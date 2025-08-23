//! Signal handling utilities for graceful shutdown.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// A thread-safe shutdown signal that can be shared across threads.
#[derive(Clone)]
pub struct ShutdownSignal {
    /// Atomic boolean indicating whether shutdown has been requested.
    shutdown: Arc<AtomicBool>,
}

impl ShutdownSignal {
    /// Create a new shutdown signal.
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal that shutdown has been requested.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    /// Wait for shutdown signal.
    pub fn wait(&self) {
        while !self.is_shutdown() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a managed thread that can be gracefully shut down.
pub struct ManagedThread {
    name: String,
    handle: JoinHandle<()>,
    shutdown_signal: ShutdownSignal,
}

impl ManagedThread {
    /// Create a new managed thread.
    pub fn new(
        name: String,
        handle: JoinHandle<()>,
        shutdown_signal: ShutdownSignal,
    ) -> Self {
        Self {
            name,
            handle,
            shutdown_signal,
        }
    }

    /// Get the thread name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Signal this thread to shut down.
    pub fn shutdown(&self) {
        self.shutdown_signal.shutdown();
    }

    /// Check if this thread is finished.
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Wait for this thread to complete.
    pub fn join(self) -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
        log::info!("Waiting for {} thread to complete", self.name);
        self.handle.join()
    }
}

/// Coordinates graceful shutdown across multiple threads.
pub struct ShutdownCoordinator {
    main_signal: ShutdownSignal,
    threads: Vec<ManagedThread>,
    timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator with default 5-second timeout.
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(5))
    }

    /// Create a new shutdown coordinator with custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            main_signal: ShutdownSignal::new(),
            threads: Vec::new(),
            timeout,
        }
    }

    /// Get the main shutdown signal.
    pub fn signal(&self) -> ShutdownSignal {
        self.main_signal.clone()
    }

    /// Register a new thread for shutdown coordination.
    pub fn register_thread<F>(
        &mut self,
        name: &str,
        thread_fn: F,
    ) -> ShutdownSignal
    where
        F: FnOnce(ShutdownSignal) + Send + 'static,
    {
        let shutdown_signal = ShutdownSignal::new();
        let signal_clone = shutdown_signal.clone();

        let handle = std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                thread_fn(signal_clone);
            })
            .unwrap_or_else(|_| panic!("Failed to spawn {} thread", name));

        let managed_thread = ManagedThread::new(name.to_string(), handle, shutdown_signal.clone());
        self.threads.push(managed_thread);

        shutdown_signal
    }

    /// Register an async thread for shutdown coordination.
    pub fn register_async_thread<F, Fut>(
        &mut self,
        name: &str,
        thread_fn: F,
    ) -> ShutdownSignal
    where
        F: FnOnce(ShutdownSignal) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let shutdown_signal = ShutdownSignal::new();
        let signal_clone = shutdown_signal.clone();
        let name_owned = name.to_string(); // Convert to owned string

        let handle = std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap_or_else(|_| {
                    panic!("Failed to create tokio runtime for {}", name_owned)
                });
                rt.block_on(thread_fn(signal_clone));
            })
            .unwrap_or_else(|_| panic!("Failed to spawn {} async thread", name));

        let managed_thread = ManagedThread::new(name.to_string(), handle, shutdown_signal.clone());
        self.threads.push(managed_thread);

        shutdown_signal
    }

    /// Start a monitor thread that watches for thread completion or external shutdown.
    pub fn start_monitor(&mut self) {
        let main_signal = self.main_signal.clone();
        let timeout = self.timeout;

        // Create signals for the monitor to watch
        let thread_signals: Vec<_> = self
            .threads
            .iter()
            .map(|t| (t.name().to_string(), t.shutdown_signal.clone()))
            .collect();

        self.register_thread("monitor", move |monitor_signal| {
            loop {
                // Check if main shutdown was signaled
                if main_signal.is_shutdown() {
                    log::info!(
                        "Monitor detected main shutdown signal, signaling all threads to exit"
                    );
                    for (name, signal) in &thread_signals {
                        log::debug!("Signaling {} thread to shutdown", name);
                        signal.shutdown();
                    }
                    break;
                }

                // Check if any thread completed (which should trigger shutdown)
                for (name, signal) in &thread_signals {
                    if signal.is_shutdown() && !main_signal.is_shutdown() {
                        log::info!(
                            "Monitor detected {} thread completed, initiating global shutdown",
                            name
                        );
                        main_signal.shutdown();
                        break;
                    }
                }

                // Check if monitor itself should shut down
                if monitor_signal.is_shutdown() {
                    break;
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        });

        // Start timeout thread
        let timeout_signal = self.main_signal.clone();
        self.register_thread("timeout", move |_| {
            // Wait for shutdown signal to be received first
            while !timeout_signal.is_shutdown() {
                std::thread::sleep(Duration::from_millis(100));
            }

            log::info!(
                "Timeout thread: shutdown signal received, starting {:?} timeout",
                timeout
            );
            let timeout_start = std::time::Instant::now();

            loop {
                let elapsed = timeout_start.elapsed();
                if elapsed > timeout {
                    log::warn!(
                        "Application did not exit within {:?}, forcing exit",
                        timeout
                    );
                    std::process::exit(0);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    /// Trigger shutdown of all threads.
    pub fn shutdown(&self) {
        log::info!("Initiating coordinated shutdown of all threads");
        self.main_signal.shutdown();
    }

    /// Wait for all threads to complete.
    pub fn wait_for_completion(self) {
        log::info!("Waiting for all threads to complete");

        let mut failed_threads = Vec::new();
        for thread in self.threads {
            let thread_name = thread.name().to_string();
            match thread.join() {
                Ok(_) => log::debug!("{} thread completed successfully", thread_name),
                Err(_) => {
                    log::warn!("{} thread completed with error", thread_name);
                    failed_threads.push(thread_name);
                }
            }
        }

        if failed_threads.is_empty() {
            log::info!("All threads completed successfully");
        } else {
            log::warn!("Some threads failed: {:?}", failed_threads);
        }
    }

    /// Get the number of registered threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
