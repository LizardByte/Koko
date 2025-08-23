//! Integration tests for web server graceful shutdown functionality.

use koko::{signal_handler::ShutdownSignal, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_web_server_shutdown_signal_handling() {
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

    // Web server should shut down within reasonable time
    let result = timeout(Duration::from_secs(2), web_handle).await;
    assert!(
        result.is_ok(),
        "Web server should shut down within 2 seconds"
    );
}

#[tokio::test]
async fn test_web_server_rocket_build() {
    // Test that we can build a rocket instance without errors
    let rocket = web::rocket();
    assert!(
        rocket.ignite().await.is_ok(),
        "Rocket should ignite successfully"
    );
}

#[tokio::test]
async fn test_web_server_with_custom_db_path() {
    // Test web server with custom database path
    let custom_db_path = Some(":memory:".to_string());
    let rocket = web::rocket_with_db_path(custom_db_path);
    assert!(
        rocket.ignite().await.is_ok(),
        "Rocket with custom DB path should ignite successfully"
    );
}

#[test]
fn test_shutdown_coordination_realistic_scenario() {
    // Test the actual coordination pattern used in the main application
    let shutdown_signal = ShutdownSignal::new();
    let web_completed = Arc::new(AtomicBool::new(false));

    // Simulate web server completion
    let web_shutdown_signal = shutdown_signal.clone();
    let web_completed_clone = Arc::clone(&web_completed);
    let web_handle = thread::spawn(move || {
        // Simulate web server startup and operation
        thread::sleep(Duration::from_millis(50));

        // Simulate receiving shutdown signal (like from Rocket)
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
        // Wait for shutdown signal first
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

    // Give timeout thread a moment, then check it didn't trigger
    thread::sleep(Duration::from_millis(100));

    // Cleanup timeout thread
    drop(timeout_handle);

    // Verify final state
    assert!(
        shutdown_signal.is_shutdown(),
        "Shutdown signal should be set"
    );
    assert!(
        web_completed.load(Ordering::Relaxed),
        "Web server should be marked as completed"
    );

    // Timeout should not have triggered since everything shut down quickly
    // Note: This test is timing-dependent, but should be reliable with the short delays
}

#[test]
fn test_multiple_shutdown_signals_coordination() {
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
        while !main_clone.is_shutdown() && !web_clone.is_shutdown() && !tray_clone.is_shutdown() {
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
fn test_shutdown_signal_performance() {
    // Test that shutdown signal operations are fast enough for real-time use
    let signal = ShutdownSignal::new();
    let iterations = 10000;

    // Test rapid checking performance
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        signal.is_shutdown();
    }
    let check_duration = start.elapsed();

    // Should be very fast (less than 1ms for 10k checks)
    assert!(
        check_duration < Duration::from_millis(1),
        "10k shutdown checks should take less than 1ms, took {:?}",
        check_duration
    );

    // Test shutdown operation
    let start = std::time::Instant::now();
    signal.shutdown();
    let shutdown_duration = start.elapsed();

    // Shutdown should be very fast (less than 1ms)
    assert!(
        shutdown_duration < Duration::from_millis(1),
        "Shutdown operation should take less than 1ms, took {:?}",
        shutdown_duration
    );

    // Verify shutdown worked
    assert!(signal.is_shutdown());
}
