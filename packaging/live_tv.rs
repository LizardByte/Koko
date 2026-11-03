//! Prepare the pinned native TV runtime for Cargo, xtask, and offline packages.

#[path = "live_tv/native.rs"]
mod native;
#[path = "live_tv/sdk.rs"]
mod sdk;
#[cfg(test)]
#[path = "live_tv/tests.rs"]
mod tests;
#[path = "build_tools.rs"]
mod tools;

use std::{
    env,
    error::Error,
    path::{
        Path,
        PathBuf,
    },
};

type BuildResult<T> = Result<T, Box<dyn Error>>;

/// Inputs shared by Cargo's native build and explicit package preparation.
#[derive(Debug)]
pub struct BuildOptions {
    /// Rust target triple whose native component will be packaged.
    pub target: String,
    /// Output directory containing the bridge, SDK libraries, data, and notices.
    pub destination: PathBuf,
    /// Reusable source, download, and CMake build cache.
    pub cache: PathBuf,
    /// Optional source checkout supplied by an offline package builder.
    pub source: Option<PathBuf>,
    /// Optional pinned revision used to isolate reusable source copies.
    pub revision: Option<String>,
    /// Maximum concurrent native compilation jobs.
    pub jobs: usize,
    /// Whether the package supplies its own C++ runtime libraries.
    pub system_cpp_runtime: bool,
}

impl BuildOptions {
    /// Use the initialized submodule, repository-local cache, and available CPUs.
    pub fn new(
        target: String,
        destination: PathBuf,
    ) -> Self {
        Self {
            target,
            destination,
            cache: PathBuf::from("target/live-tv-deps"),
            source: None,
            revision: None,
            jobs: std::thread::available_parallelism().map_or(2, usize::from),
            system_cpp_runtime: false,
        }
    }
}

fn library(target: &str) -> &'static str {
    if target.contains("windows") {
        "koko_tv.dll"
    } else if target.contains("apple") {
        "libkoko_tv.dylib"
    } else {
        "libkoko_tv.so"
    }
}

/// Build and collect the native runtime without installing TSDuck's CLI programs.
pub fn build(
    workspace: &Path,
    options: &BuildOptions,
) -> BuildResult<()> {
    native::build(workspace, options)
}

/// Honor Cargo overrides or build the native component from the pinned submodule.
pub fn prepare(
    workspace: &Path,
    out: &Path,
    target: &str,
) -> BuildResult<Option<PathBuf>> {
    for variable in [
        "KOKO_TSDUCK_ROOT",
        "KOKO_SKIP_TSDUCK_BUILD",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    for file in [
        "native/live-tv/bridge.cpp",
        "native/live-tv/CMakeLists.txt",
        "packaging/live_tv.rs",
        "packaging/live_tv/native.rs",
        "packaging/live_tv/sdk.rs",
        "packaging/build_tools.rs",
        "packaging/patches/tsduck-no-web.patch",
        "packaging/licenses/gcc-runtime-GPL-3.0.txt",
        "packaging/licenses/gcc-runtime-exception-3.1.txt",
    ] {
        println!("cargo:rerun-if-changed={}", workspace.join(file).display());
    }
    if let Some(root) = env::var_os("KOKO_TSDUCK_ROOT") {
        let root = PathBuf::from(root).canonicalize()?;
        if !root.join(library(target)).is_file() {
            return Err(
                "KOKO_TSDUCK_ROOT does not contain the native TV library for this target".into(),
            );
        }
        return Ok(Some(root));
    }
    if env::var_os("KOKO_SKIP_TSDUCK_BUILD").is_some() {
        return Ok(None);
    }
    let source = workspace.join("third-party/tsduck");
    if !source.join(".git").exists() {
        return Err(
            "Initialize TSDuck with: git submodule update --init third-party/tsduck".into(),
        );
    }
    let git_dir = tools::git_output(
        &source,
        &[
            "rev-parse",
            "--absolute-git-dir",
        ],
    )?;
    for path in ["HEAD", "refs", "packed-refs"] {
        println!(
            "cargo:rerun-if-changed={}",
            Path::new(&git_dir).join(path).display()
        );
    }
    let root = out.join("live-tv");
    let mut options = BuildOptions::new(target.to_owned(), root.clone());
    options.source = Some(source.clone());
    options.revision = Some(tools::git_output(&source, &["rev-parse", "HEAD"])?);
    if let Ok(jobs) = env::var("NUM_JOBS") {
        options.jobs = jobs.parse()?;
    }
    build(workspace, &options)?;
    if !root.join(library(target)).is_file() {
        return Err(
            "Unable to prepare Koko's native TV component; the native library is missing".into(),
        );
    }
    Ok(Some(root))
}
