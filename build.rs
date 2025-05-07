use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets");

    // Get the workspace root directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(&manifest_dir)
        .ancestors()
        .nth(2) // Go up two levels from crates/<crate> to reach the workspace root
        .expect("Failed to find workspace root");

    // Create target directory in workspace root
    let target_dir = workspace_root.join("target/doc/assets");
    fs::create_dir_all(&target_dir).expect("Failed to create target/doc/assets directory");

    // Copy assets from workspace root
    let assets_dir = workspace_root.join("assets");
    fs::copy(assets_dir.join("icon.ico"), target_dir.join("icon.ico"))
        .expect("Failed to copy crate favicon");
    fs::copy(assets_dir.join("icon.png"), target_dir.join("icon.png"))
        .expect("Failed to copy crate logo");
}
