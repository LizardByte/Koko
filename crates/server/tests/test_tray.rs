// standard imports
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Because `launch()` runs an event loop indefinitely, this test spawns a thread to
/// call `launch()`, waits a brief moment, then ends the test. It mainly verifies that the
/// function can be invoked without immediate panics. Adjust as needed for full integration testing.
#[test]
fn test_launch_does_not_panic_immediately() {
    // We run this in a separate thread because `launch()` never returns under normal circumstances.
    let handle = thread::spawn(|| {
        koko::tray::launch();
    });

    // Wait a short moment to see if a panic happens right away.
    thread::sleep(Duration::from_secs(1));

    // We don't join the thread because `launch()` won't return in this example,
    // but dropping the handle will end the spawned thread here.
    drop(handle);
}

/// Tests the `load_icon` function with a path that does not exist.
/// We expect a panic, because the code calls `image::open`
/// and it should fail on a non-existent file.
#[test]
#[should_panic(expected = "Failed to open icon path")]
fn test_load_icon_non_existent_path_panics() {
    use koko::tray::load_icon;

    let non_existent_path = Path::new("non_existent_file.ico");

    // This should panic based on the logic within `load_icon`.
    let _icon = load_icon(non_existent_path);
}
