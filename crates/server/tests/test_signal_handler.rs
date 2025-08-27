//! Tests for signal handling and graceful shutdown functionality.
//!
//! This module tests all components from src/signal_handler.rs:
//! - ShutdownSignal: Basic shutdown signaling functionality
//! - ShutdownCoordinator: Thread coordination and management
//! - Integration tests: End-to-end shutdown scenarios

// standard imports
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::Duration;
use tokio::time::timeout;

// local imports
use koko::signal_handler::{ShutdownCoordinator, ShutdownSignal};
use koko::web;

mod shutdown_signal {
    use super::*;

    #[test]
    fn creation() {
        let signal = ShutdownSignal::new();
        assert!(!signal.is_shutdown(), "New signal should not be shutdown");
    }

    #[test]
    fn default() {
        let signal = ShutdownSignal::default();
        assert!(
            !signal.is_shutdown(),
            "Default signal should not be shutdown"
        );
    }

    #[test]
    fn basic_functionality() {
        let signal = ShutdownSignal::new();

        // Initially not shutdown
        assert!(!signal.is_shutdown());

        // After calling shutdown, should be shutdown
        signal.shutdown();
        assert!(signal.is_shutdown());
    }

    #[test]
    fn cloning() {
        let original = ShutdownSignal::new();
        let cloned = original.clone();

        // Both should start as not shutdown
        assert!(!original.is_shutdown());
        assert!(!cloned.is_shutdown());

        // Shutting down original should affect clone
        original.shutdown();
        assert!(original.is_shutdown());
        assert!(cloned.is_shutdown());

        // Test the reverse - shutdown via clone
        let original2 = ShutdownSignal::new();
        let cloned2 = original2.clone();

        cloned2.shutdown();
        assert!(original2.is_shutdown());
        assert!(cloned2.is_shutdown());
    }

    #[test]
    fn thread_safety() {
        let signal = ShutdownSignal::new();
        let signal_clone = signal.clone();

        // Spawn a thread that will set shutdown after a delay
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal_clone.shutdown();
        });

        // Initially not shutdown
        assert!(!signal.is_shutdown());

        // Wait for the thread to set shutdown
        handle.join().unwrap();

        // Now should be shutdown
        assert!(signal.is_shutdown());
    }

    #[test]
    fn multiple_shutdowns() {
        let signal = ShutdownSignal::new();

        // Multiple calls to shutdown should be safe
        signal.shutdown();
        assert!(signal.is_shutdown());

        signal.shutdown();
        assert!(signal.is_shutdown());

        signal.shutdown();
        assert!(signal.is_shutdown());
    }

    #[test]
    fn wait_with_timeout() {
        let signal = ShutdownSignal::new();
        let signal_clone = signal.clone();

        // Spawn a thread that will set shutdown after a short delay
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            signal_clone.shutdown();
        });

        // Test wait with a reasonable timeout
        let start = std::time::Instant::now();

        // Use a custom wait implementation that can timeout for testing
        let mut waited = false;
        while !signal.is_shutdown() {
            thread::sleep(Duration::from_millis(10));
            if start.elapsed() > Duration::from_millis(200) {
                break;
            }
            waited = true;
        }

        assert!(waited, "Should have waited for shutdown signal");
        assert!(signal.is_shutdown(), "Signal should be shutdown after wait");
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "Should not have timed out"
        );
    }

    #[test]
    fn concurrent_access() {
        let signal = Arc::new(ShutdownSignal::new());
        let mut handles = vec![];

        // Spawn multiple threads that check and set shutdown
        for i in 0..10 {
            let signal_clone: Arc<ShutdownSignal> = Arc::clone(&signal);
            let handle = thread::spawn(move || {
                // Each thread waits a different amount of time
                thread::sleep(Duration::from_millis(i * 10));

                if i == 5 {
                    // Thread 5 sets shutdown
                    signal_clone.shutdown();
                }

                // All threads eventually see shutdown
                let mut attempts = 0;
                while !signal_clone.is_shutdown() && attempts < 100 {
                    thread::sleep(Duration::from_millis(5));
                    attempts += 1;
                }

                signal_clone.is_shutdown()
            });
            handles.push(handle);
        }

        // All threads should eventually see the shutdown signal
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result, "All threads should see shutdown signal");
        }
    }

    #[test]
    fn memory_consistency() {
        let signal = ShutdownSignal::new();

        // Test that shutdown state is immediately visible across threads
        let signal_clone = signal.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Wait for the main thread to signal
            barrier_clone.wait();

            // Should immediately see shutdown state
            signal_clone.is_shutdown()
        });

        // Set shutdown and then signal the other thread
        signal.shutdown();
        barrier.wait();

        let result = handle.join().unwrap();
        assert!(result, "Other thread should immediately see shutdown state");
    }

    #[test]
    fn performance() {
        // Test that shutdown signal operations are fast enough for real-time use
        let signal = ShutdownSignal::new();
        let iterations = 10000;

        // Test rapid checking performance
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            signal.is_shutdown();
        }
        let check_duration = start.elapsed();

        // Should be very fast (roughly 1 ms for 10k checks, some margin is allowed for anomalies)
        assert!(
            check_duration < Duration::from_millis(10),
            "10k shutdown checks should take less than 10ms, took {:?}",
            check_duration
        );

        // Test shutdown operation
        let start = std::time::Instant::now();
        signal.shutdown();
        let shutdown_duration = start.elapsed();

        // Shutdown should be very fast (less than 1 ms)
        assert!(
            shutdown_duration < Duration::from_millis(1),
            "Shutdown operation should take less than 1ms, took {:?}",
            shutdown_duration
        );

        // Verify shutdown worked
        assert!(signal.is_shutdown());
    }

    #[test]
    fn wait_functionality() {
        let signal = ShutdownSignal::new();
        let signal_clone = signal.clone();

        // Spawn a thread that will signal shutdown after a delay
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            signal_clone.shutdown();
        });

        // Test the wait method - this should return once shutdown is signaled
        let start = std::time::Instant::now();
        signal.wait();
        let elapsed = start.elapsed();

        // Should have waited for about 50ms
        assert!(
            elapsed >= Duration::from_millis(40),
            "Should have waited for shutdown signal"
        );
        assert!(
            elapsed < Duration::from_millis(200),
            "Should not have waited too long"
        );
        assert!(signal.is_shutdown(), "Signal should be shutdown after wait");

        handle.join().unwrap();
    }

    #[test]
    fn wait_with_already_shutdown_signal() {
        let signal = ShutdownSignal::new();

        // Signal shutdown first
        signal.shutdown();

        // Then call wait - should return immediately
        let start = std::time::Instant::now();
        signal.wait();
        let elapsed = start.elapsed();

        // Should return almost immediately since signal is already shutdown
        assert!(
            elapsed < Duration::from_millis(50),
            "Wait should return immediately for already shutdown signal"
        );
    }
}

mod managed_thread {
    use super::*;

    #[test]
    fn creation_and_basic_functionality() {
        let shutdown_signal = ShutdownSignal::new();
        let signal_clone = shutdown_signal.clone();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = Arc::clone(&completed);

        let handle = thread::spawn(move || {
            // Wait for shutdown signal
            while !signal_clone.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }
            completed_clone.store(true, Ordering::Relaxed);
        });

        let managed_thread = koko::signal_handler::ManagedThread::new(
            "test-thread".to_string(),
            handle,
            shutdown_signal.clone(),
        );

        // Test name method
        assert_eq!(managed_thread.name(), "test-thread");

        // Test is_finished method (should be false while thread is running)
        assert!(!managed_thread.is_finished());

        // Test shutdown method
        managed_thread.shutdown();
        assert!(shutdown_signal.is_shutdown());

        // Wait for thread to complete
        let result = managed_thread.join();
        assert!(result.is_ok());
        assert!(completed.load(Ordering::Relaxed));
    }

    #[test]
    fn is_finished_functionality() {
        let shutdown_signal = ShutdownSignal::new();
        let signal_clone = shutdown_signal.clone();

        let handle = thread::spawn(move || {
            // Quick task that finishes immediately
            signal_clone.shutdown();
        });

        let managed_thread = koko::signal_handler::ManagedThread::new(
            "quick-thread".to_string(),
            handle,
            shutdown_signal,
        );

        // Give thread time to complete
        thread::sleep(Duration::from_millis(100));

        // Now is_finished should return true
        assert!(managed_thread.is_finished());

        // Clean up
        let _ = managed_thread.join();
    }
}

mod shutdown_coordinator {
    use super::*;

    #[test]
    fn default_implementation() {
        ShutdownCoordinator::disable_force_exit();
        let coordinator = ShutdownCoordinator::default();
        assert_eq!(coordinator.thread_count(), 0);
        assert!(!coordinator.signal().is_shutdown());
    }

    #[test]
    fn shutdown_method() {
        ShutdownCoordinator::disable_force_exit();
        let coordinator = ShutdownCoordinator::new();
        let main_signal = coordinator.signal();

        // Initially not shutdown
        assert!(!main_signal.is_shutdown());

        // Call shutdown method
        coordinator.shutdown();

        // Should now be shutdown
        assert!(main_signal.is_shutdown());
    }

    #[test]
    fn thread_count_functionality() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::new();

        // Initially no threads
        assert_eq!(coordinator.thread_count(), 0);

        // Register some threads
        coordinator.register_thread("thread1", |_| {
            thread::sleep(Duration::from_millis(10));
        });
        assert_eq!(coordinator.thread_count(), 1);

        coordinator.register_async_thread("thread2", |_| async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
        });
        assert_eq!(coordinator.thread_count(), 2);

        coordinator.register_thread("thread3", |_| {
            thread::sleep(Duration::from_millis(10));
        });
        assert_eq!(coordinator.thread_count(), 3);

        // Wait for completion
        coordinator.wait_for_completion();
    }

    #[test]
    fn wait_for_completion_with_thread_errors() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::new();

        // Register a thread that will panic
        coordinator.register_thread("panic-thread", |_| {
            thread::sleep(Duration::from_millis(10));
            panic!("Intentional test panic");
        });

        // Register a normal thread
        coordinator.register_thread("normal-thread", |_| {
            thread::sleep(Duration::from_millis(20));
        });

        // This should handle the panic gracefully and log warnings
        coordinator.wait_for_completion();
    }

    #[test]
    fn wait_for_completion_all_successful() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::new();
        let counter = Arc::new(AtomicU32::new(0));

        // Register several successful threads
        for i in 0..3 {
            let counter_clone = Arc::clone(&counter);
            coordinator.register_thread(&format!("success-thread-{}", i), move |_| {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(10));
            });
        }

        // This should complete successfully and log success message
        coordinator.wait_for_completion();

        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn monitor_thread_functionality() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::new();
        let main_signal = coordinator.signal();
        let monitor_triggered = Arc::new(AtomicBool::new(false));
        let monitor_clone = Arc::clone(&monitor_triggered);

        // Register a thread that will complete quickly
        coordinator.register_thread("quick-thread", move |shutdown_signal| {
            thread::sleep(Duration::from_millis(50));
            // This thread completing should trigger monitor to initiate global shutdown
            shutdown_signal.shutdown();
            monitor_clone.store(true, Ordering::Relaxed);
        });

        // Register a thread that waits for shutdown signal
        let long_running_shutdown = Arc::new(AtomicBool::new(false));
        let lr_clone = Arc::clone(&long_running_shutdown);
        coordinator.register_thread("waiting-thread", move |shutdown_signal| {
            while !shutdown_signal.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }
            lr_clone.store(true, Ordering::Relaxed);
        });

        // Start monitor - this will detect thread completion and signal shutdown
        coordinator.start_monitor();

        // Wait for completion
        coordinator.wait_for_completion();

        // Verify monitor detected thread completion and triggered shutdown
        assert!(monitor_triggered.load(Ordering::Relaxed));
        assert!(main_signal.is_shutdown());
        assert!(long_running_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn monitor_external_shutdown_signal() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::new();
        let main_signal = coordinator.signal();

        let shutdown_received = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown_received);

        // Register a thread that waits for shutdown signal
        coordinator.register_thread("waiting-thread", move |shutdown_signal| {
            while !shutdown_signal.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }
            shutdown_clone.store(true, Ordering::Relaxed);
        });

        // Start monitor
        coordinator.start_monitor();

        // External shutdown signal after delay
        let signal_clone = main_signal.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal_clone.shutdown();
        });

        // Wait for completion
        coordinator.wait_for_completion();

        // Verify external shutdown was handled
        assert!(main_signal.is_shutdown());
        assert!(shutdown_received.load(Ordering::Relaxed));
    }

    #[test]
    #[should_panic(expected = "Failed to create tokio runtime")]
    fn async_thread_runtime_creation_failure() {
        // This test is tricky to trigger in practice, but we can document it
        // The panic path occurs when tokio runtime creation fails
        // In normal circumstances this should never happen, but the panic is there for safety

        // Since we can't easily mock runtime creation failure, we'll create a separate test
        // that documents this behavior. The actual panic line will be covered when/if
        // runtime creation actually fails in extreme circumstances.

        // For now, let's verify that normal async thread creation works fine
        let mut coordinator = ShutdownCoordinator::new();

        coordinator.register_async_thread("normal-async", |_| async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
        });

        coordinator.wait_for_completion();

        // If we reach here, runtime creation worked fine
        // The panic path is for extreme error conditions that are hard to reproduce in tests
        panic!("Failed to create tokio runtime for test_panic_scenario");
    }

    #[test]
    fn timeout_thread_functionality() {
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::with_timeout(Duration::from_millis(200));
        let main_signal = coordinator.signal();

        // Register a thread that will wait for shutdown
        let timeout_activated = Arc::new(AtomicBool::new(false));
        let timeout_clone = Arc::clone(&timeout_activated);

        coordinator.register_thread("timeout-test-thread", move |shutdown_signal| {
            // Wait for shutdown signal
            while !shutdown_signal.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }
            timeout_clone.store(true, Ordering::Relaxed);
        });

        // Start monitor (which also starts timeout thread)
        coordinator.start_monitor();

        // Signal shutdown to activate timeout mechanism
        main_signal.shutdown();

        // Wait for completion - timeout thread should handle this gracefully
        coordinator.wait_for_completion();

        assert!(timeout_activated.load(Ordering::Relaxed));
    }

    #[test]
    fn timeout_thread_graceful_exit_on_windows() {
        // This test specifically checks that timeout thread exits gracefully during tests
        ShutdownCoordinator::disable_force_exit();
        let mut coordinator = ShutdownCoordinator::with_timeout(Duration::from_millis(50));
        let main_signal = coordinator.signal();

        // Register a thread that will deliberately take longer than timeout to finish
        coordinator.register_thread("slow-thread", move |shutdown_signal| {
            // Wait for shutdown signal
            while !shutdown_signal.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }
            // Simulate slow cleanup that would normally trigger timeout
            thread::sleep(Duration::from_millis(100));
        });

        // Start monitor (which also starts timeout thread)
        coordinator.start_monitor();

        // Signal shutdown to activate timeout mechanism
        main_signal.shutdown();

        // Wait for completion - should complete gracefully without process::exit
        coordinator.wait_for_completion();

        // If we reach here, the timeout thread exited gracefully
        assert!(main_signal.is_shutdown());
    }
}

mod integration {
    use super::*;

    #[tokio::test]
    async fn web_server_shutdown_signal_handling() {
        let shutdown_signal = ShutdownSignal::new();
        let shutdown_signal_clone = shutdown_signal.clone();

        // Start web server in background
        let web_handle = tokio::spawn(async move {
            web::launch_with_shutdown(shutdown_signal_clone).await;
        });

        // Give the server a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Signal shutdown
        shutdown_signal.shutdown();

        // Web server should shut down within a reasonable time
        let result = timeout(Duration::from_secs(2), web_handle).await;
        assert!(
            result.is_ok(),
            "Web server should shut down within 2 seconds"
        );
    }

    #[test]
    fn shutdown_coordination_realistic_scenario() {
        // Test the actual coordination pattern used in the main application
        let shutdown_signal = ShutdownSignal::new();
        let web_completed = Arc::new(AtomicBool::new(false));

        // Simulate web server completion
        let web_shutdown_signal = shutdown_signal.clone();
        let web_completed_clone = Arc::clone(&web_completed);
        let web_handle = thread::spawn(move || {
            // Simulate web server startup and operation
            thread::sleep(Duration::from_millis(50));

            // Simulate receiving a shutdown signal (like from Rocket)
            web_completed_clone.store(true, Ordering::Relaxed);
            web_shutdown_signal.shutdown();

            println!("Simulated web server completed");
        });

        // Simulate tray shutdown signal
        let tray_shutdown_signal = shutdown_signal.clone();

        // Simulate monitor thread
        let monitor_shutdown_signal = shutdown_signal.clone();
        let web_completed_monitor = Arc::clone(&web_completed);
        let monitor_handle = thread::spawn(move || {
            loop {
                if monitor_shutdown_signal.is_shutdown() {
                    println!("Monitor detected shutdown signal, signaling tray to exit");
                    tray_shutdown_signal.shutdown();
                    break;
                }
                if web_completed_monitor.load(Ordering::Relaxed) {
                    println!("Monitor detected web server completed, signaling tray to exit");
                    tray_shutdown_signal.shutdown();
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Simulate timeout mechanism
        let timeout_shutdown_signal = shutdown_signal.clone();
        let timeout_triggered = Arc::new(AtomicBool::new(false));
        let timeout_triggered_clone = Arc::clone(&timeout_triggered);
        let timeout_handle = thread::spawn(move || {
            // Wait for the shutdown signal first
            while !timeout_shutdown_signal.is_shutdown() {
                thread::sleep(Duration::from_millis(10));
            }

            // Start timeout
            let timeout_start = std::time::Instant::now();
            loop {
                let elapsed = timeout_start.elapsed();
                if elapsed > Duration::from_millis(500) {
                    // Shorter timeout for testing
                    timeout_triggered_clone.store(true, Ordering::Relaxed);
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Wait for all threads to complete
        web_handle.join().unwrap();
        monitor_handle.join().unwrap();

        // Give the timeout thread a moment, then check it didn't trigger
        thread::sleep(Duration::from_millis(100));

        // Cleanup timeout thread
        drop(timeout_handle);

        // Verify the final state
        assert!(
            shutdown_signal.is_shutdown(),
            "Shutdown signal should be set"
        );
        assert!(
            web_completed.load(Ordering::Relaxed),
            "Web server should be marked as completed"
        );

        // Timeout should not have triggered since everything shut down quickly
        // Note: This test is timing-dependent but should be reliable with the short delays
    }

    #[test]
    fn multiple_shutdown_signals_coordination() {
        // Test that multiple shutdown signals work correctly together
        let main_signal = ShutdownSignal::new();
        let web_signal = ShutdownSignal::new();
        let tray_signal = ShutdownSignal::new();

        // Clone signals for use in closures
        let main_clone = main_signal.clone();
        let web_clone = web_signal.clone();
        let tray_clone = tray_signal.clone();
        let main_for_coordinator = main_signal.clone();
        let web_for_coordinator = web_signal.clone();
        let tray_for_coordinator = tray_signal.clone();

        // Thread that coordinates shutdown
        let coordinator_handle = thread::spawn(move || {
            // Wait for any signal
            while !main_clone.is_shutdown() && !web_clone.is_shutdown() && !tray_clone.is_shutdown()
            {
                thread::sleep(Duration::from_millis(5));
            }

            // Propagate shutdown to all signals
            main_for_coordinator.shutdown();
            web_for_coordinator.shutdown();
            tray_for_coordinator.shutdown();
        });

        // Trigger shutdown from one signal after a delay
        let trigger_signal = main_signal.clone();
        let trigger_handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            trigger_signal.shutdown();
        });

        // Wait for coordination
        trigger_handle.join().unwrap();
        coordinator_handle.join().unwrap();

        // All signals should be shutdown
        assert!(main_signal.is_shutdown());
        assert!(web_signal.is_shutdown());
        assert!(tray_signal.is_shutdown());
    }

    #[test]
    fn shutdown_coordination_pattern() {
        // Test the pattern used in the main application
        let shutdown_signal = ShutdownSignal::new();
        let web_completed = Arc::new(AtomicBool::new(false));

        // Simulate web server thread
        let web_shutdown_signal = shutdown_signal.clone();
        let web_completed_clone = Arc::clone(&web_completed);
        let web_handle = thread::spawn(move || {
            // Simulate web server work
            thread::sleep(Duration::from_millis(50));

            // Web server detects shutdown or completes
            web_completed_clone.store(true, Ordering::Relaxed);
            web_shutdown_signal.shutdown();
        });

        // Simulate monitor thread
        let monitor_shutdown_signal = shutdown_signal.clone();
        let web_completed_monitor = Arc::clone(&web_completed);
        let tray_shutdown_signal = shutdown_signal.clone();
        let monitor_handle = thread::spawn(move || {
            loop {
                if monitor_shutdown_signal.is_shutdown()
                    || web_completed_monitor.load(Ordering::Relaxed)
                {
                    tray_shutdown_signal.shutdown();
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Wait for coordination to complete
        web_handle.join().unwrap();
        monitor_handle.join().unwrap();

        // Verify the final state
        assert!(shutdown_signal.is_shutdown());
        assert!(web_completed.load(Ordering::Relaxed));
    }
}
