#![cfg(feature = "tray")]

// standard imports
use std::path::Path;

/// Tests that an existing source icon can be decoded for the tray.
#[test]
fn test_load_icon_source_icon() {
    use koko::tray::load_icon;

    let icon_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("icon.ico");

    let _icon = load_icon(&icon_path);
}

/// Tests that the tray event loop honors shutdown without creating a long-running test process.
#[cfg(target_os = "windows")]
#[test]
fn test_launch_with_shutdown_exits() {
    let shutdown_signal = koko::signal_handler::ShutdownSignal::new();
    shutdown_signal.shutdown();

    koko::tray::launch_with_shutdown(shutdown_signal);
}

/// Tests the `load_icon` function with a path that does not exist.
/// We expect a panic, because the code calls `image::open`
/// and it should fail on a non-existent file.
#[test]
fn test_load_icon_non_existent_path_panics() {
    use koko::tray::load_icon;

    let non_existent_path = Path::new("non_existent_file.ico");

    // This should panic based on the logic within `load_icon`.
    let result = std::panic::catch_unwind(|| load_icon(non_existent_path));
    let panic = result.expect_err("Missing icon path should panic");
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic>");

    assert!(
        message.contains("Failed to open icon path"),
        "unexpected panic message: {}",
        message
    );
}
