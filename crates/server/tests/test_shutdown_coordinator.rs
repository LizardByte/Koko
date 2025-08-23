//! Tests for the ShutdownCoordinator and improved shutdown system.

use koko::signal_handler::ShutdownCoordinator;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

#[test]
fn test_shutdown_coordinator_creation() {
    let coordinator = ShutdownCoordinator::new();
    assert_eq!(coordinator.thread_count(), 0);
    assert!(!coordinator.signal().is_shutdown());
}

#[test]
fn test_shutdown_coordinator_with_custom_timeout() {
    let timeout = Duration::from_millis(500);
    let coordinator = ShutdownCoordinator::with_timeout(timeout);
    assert_eq!(coordinator.thread_count(), 0);
}

#[test]
fn test_register_sync_thread() {
    let mut coordinator = ShutdownCoordinator::new();
    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = Arc::clone(&completed);

    let _signal = coordinator.register_thread("test-thread", move |shutdown_signal| {
        // Simulate some work
        std::thread::sleep(Duration::from_millis(50));
        completed_clone.store(true, Ordering::Relaxed);

        // Check shutdown signal
        assert!(!shutdown_signal.is_shutdown());
    });

    assert_eq!(coordinator.thread_count(), 1);

    // Wait for completion
    coordinator.wait_for_completion();

    assert!(completed.load(Ordering::Relaxed));
}

#[test]
fn test_register_async_thread() {
    let mut coordinator = ShutdownCoordinator::new();
    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = Arc::clone(&completed);

    let _signal =
        coordinator.register_async_thread("async-test", move |shutdown_signal| async move {
            // Simulate async work
            tokio::time::sleep(Duration::from_millis(50)).await;
            completed_clone.store(true, Ordering::Relaxed);

            // Check shutdown signal
            assert!(!shutdown_signal.is_shutdown());
        });

    assert_eq!(coordinator.thread_count(), 1);

    // Wait for completion
    coordinator.wait_for_completion();

    assert!(completed.load(Ordering::Relaxed));
}

#[test]
fn test_multiple_threads_registration() {
    let mut coordinator = ShutdownCoordinator::new();
    let counter = Arc::new(AtomicU32::new(0));

    // Register multiple sync threads
    for i in 0..3 {
        let counter_clone = Arc::clone(&counter);
        coordinator.register_thread(&format!("sync-{}", i), move |_| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(10));
        });
    }

    // Register multiple async threads
    for i in 0..2 {
        let counter_clone = Arc::clone(&counter);
        coordinator.register_async_thread(&format!("async-{}", i), move |_| async move {
            counter_clone.fetch_add(10, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(10)).await;
        });
    }

    assert_eq!(coordinator.thread_count(), 5);

    // Wait for all to complete
    coordinator.wait_for_completion();

    // 3 sync threads (1 each) + 2 async threads (10 each) = 23
    assert_eq!(counter.load(Ordering::Relaxed), 23);
}

#[test]
fn test_coordinated_shutdown() {
    let mut coordinator = ShutdownCoordinator::new();
    let main_signal = coordinator.signal();

    let thread1_shutdown = Arc::new(AtomicBool::new(false));
    let thread2_shutdown = Arc::new(AtomicBool::new(false));

    let t1_clone = Arc::clone(&thread1_shutdown);
    coordinator.register_thread("worker-1", move |shutdown_signal| {
        // Wait for shutdown signal
        while !shutdown_signal.is_shutdown() {
            std::thread::sleep(Duration::from_millis(10));
        }
        t1_clone.store(true, Ordering::Relaxed);
    });

    let t2_clone = Arc::clone(&thread2_shutdown);
    coordinator.register_async_thread("worker-2", move |shutdown_signal| async move {
        // Wait for shutdown signal
        while !shutdown_signal.is_shutdown() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        t2_clone.store(true, Ordering::Relaxed);
    });

    // Start monitor to coordinate shutdown
    coordinator.start_monitor();

    // Signal shutdown after a delay
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        main_signal.shutdown();
    });

    // Wait for completion
    coordinator.wait_for_completion();

    // Both threads should have received shutdown signals
    assert!(thread1_shutdown.load(Ordering::Relaxed));
    assert!(thread2_shutdown.load(Ordering::Relaxed));
}

#[test]
fn test_thread_completion_triggers_shutdown() {
    let mut coordinator = ShutdownCoordinator::new();
    let main_signal = coordinator.signal();

    let long_running_shutdown = Arc::new(AtomicBool::new(false));
    let lr_clone = Arc::clone(&long_running_shutdown);

    // Register a short-lived thread that completes quickly
    coordinator.register_thread("short-lived", |_| {
        std::thread::sleep(Duration::from_millis(50));
        // This thread completes, which should trigger global shutdown
    });

    // Register a long-running thread that waits for shutdown
    coordinator.register_thread("long-running", move |shutdown_signal| {
        while !shutdown_signal.is_shutdown() {
            std::thread::sleep(Duration::from_millis(10));
        }
        lr_clone.store(true, Ordering::Relaxed);
    });

    // Start monitor
    coordinator.start_monitor();

    // Wait for completion
    coordinator.wait_for_completion();

    // Main signal should be shutdown due to short-lived thread completion
    assert!(main_signal.is_shutdown());

    // Long-running thread should have been signaled to shutdown
    assert!(long_running_shutdown.load(Ordering::Relaxed));
}

#[test]
fn test_shutdown_coordinator_thread_names() {
    let mut coordinator = ShutdownCoordinator::new();

    coordinator.register_thread("database-manager", |_| {
        std::thread::sleep(Duration::from_millis(10));
    });

    coordinator.register_async_thread("file-watcher", |_| async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
    });

    coordinator.register_thread("background-processor", |_| {
        std::thread::sleep(Duration::from_millis(10));
    });

    assert_eq!(coordinator.thread_count(), 3);

    coordinator.wait_for_completion();
}

#[test]
fn test_shutdown_signal_propagation() {
    let mut coordinator = ShutdownCoordinator::new();
    let received_signals = Arc::new(AtomicU32::new(0));

    // Create multiple threads that all check for shutdown
    for i in 0..5 {
        let counter = Arc::clone(&received_signals);
        coordinator.register_thread(&format!("checker-{}", i), move |shutdown_signal| {
            // Wait for shutdown signal
            while !shutdown_signal.is_shutdown() {
                std::thread::sleep(Duration::from_millis(5));
            }
            counter.fetch_add(1, Ordering::Relaxed);
        });
    }

    coordinator.start_monitor();

    // Trigger shutdown
    let main_signal = coordinator.signal();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        main_signal.shutdown();
    });

    coordinator.wait_for_completion();

    // All threads should have received the shutdown signal
    assert_eq!(received_signals.load(Ordering::Relaxed), 5);
}

#[test]
fn test_performance_with_many_threads() {
    let mut coordinator = ShutdownCoordinator::new();
    let start_time = std::time::Instant::now();

    // Register many threads
    for i in 0..20 {
        coordinator.register_thread(&format!("perf-{}", i), |_| {
            // Minimal work
            std::thread::sleep(Duration::from_millis(1));
        });
    }

    let registration_time = start_time.elapsed();

    let completion_start = std::time::Instant::now();
    coordinator.wait_for_completion();
    let completion_time = completion_start.elapsed();

    // Performance assertions - these are quite generous
    assert!(
        registration_time < Duration::from_millis(100),
        "Registration took too long: {:?}",
        registration_time
    );
    assert!(
        completion_time < Duration::from_millis(500),
        "Completion took too long: {:?}",
        completion_time
    );
}
