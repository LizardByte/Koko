use std::env;
use std::fs;
use std::path::Path;

#[path = "packaging/ffmpeg.rs"]
mod ffmpeg;

fn main() {
    // Get the workspace root directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(&manifest_dir)
        .ancestors()
        .nth(2) // Go up two levels from crates/<crate> to reach the workspace root
        .expect("Failed to find workspace root");

    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("assets").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("packaging/ffmpeg.rs").display()
    );
    let out_dir = env::var_os("OUT_DIR").expect("Failed to get OUT_DIR");
    let target = env::var("TARGET").expect("Failed to get TARGET");
    let prepared = ffmpeg::prepare(workspace_root, Path::new(&out_dir), &target)
        .expect("Failed to prepare build-deps media tools");
    let prepared = prepared
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=KOKO_BUILD_FFMPEG_ROOT={prepared}");
    fs::write(Path::new(&out_dir).join("ffmpeg-root.txt"), prepared)
        .expect("Failed to record the prepared FFmpeg path");

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
