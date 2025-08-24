//! Tests for signal handling and graceful shutdown functionality.

use koko::signal_handler::ShutdownSignal;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_shutdown_signal_creation() {
    let signal = ShutdownSignal::new();
    assert!(!signal.is_shutdown(), "New signal should not be shutdown");
}

#[test]
fn test_shutdown_signal_default() {
    let signal = ShutdownSignal::default();
    assert!(
        !signal.is_shutdown(),
        "Default signal should not be shutdown"
    );
}

#[test]
fn test_shutdown_signal_basic_functionality() {
    let signal = ShutdownSignal::new();

    // Initially not shutdown
    assert!(!signal.is_shutdown());

    // After calling shutdown, should be shutdown
    signal.shutdown();
    assert!(signal.is_shutdown());
}

#[test]
fn test_shutdown_signal_cloning() {
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
fn test_shutdown_signal_thread_safety() {
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
fn test_shutdown_signal_multiple_shutdowns() {
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
fn test_shutdown_signal_wait_with_timeout() {
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
fn test_shutdown_signal_concurrent_access() {
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
fn test_shutdown_signal_memory_consistency() {
    let signal = ShutdownSignal::new();

    // Test that shutdown state is immediately visible across threads
    let signal_clone = signal.clone();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let barrier_clone = Arc::clone(&barrier);

    let handle = thread::spawn(move || {
        // Wait for main thread to signal
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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_shutdown_coordination_pattern() {
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

        // Verify final state
        assert!(shutdown_signal.is_shutdown());
        assert!(web_completed.load(Ordering::Relaxed));
    }
}
