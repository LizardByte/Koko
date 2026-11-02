//! Prepare the pinned native TV runtime for Cargo builds and release packaging.

use std::{
    env,
    error::Error,
    path::{
        Path,
        PathBuf,
    },
    process::Command,
};
type BuildResult<T> = Result<T, Box<dyn Error>>;

fn library(target: &str) -> &'static str {
    if target.contains("windows") {
        "koko_tv.dll"
    } else if target.contains("apple") {
        "libkoko_tv.dylib"
    } else {
        "libkoko_tv.so"
    }
}

fn python() -> BuildResult<Command> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("KOKO_PYTHON") {
        candidates.push(PathBuf::from(path));
    }
    candidates.extend([
        PathBuf::from("python3"),
        PathBuf::from("python"),
    ]);
    #[cfg(windows)]
    if let Some(home) = env::var_os("USERPROFILE") {
        // A GUI-launched Cargo may inherit an incomplete PATH for an existing pyenv install.
        let versions = PathBuf::from(home).join(".pyenv/pyenv-win/versions");
        if let Ok(entries) = std::fs::read_dir(versions) {
            candidates.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path().join("python.exe")),
            );
        }
    }
    for path in candidates {
        let mut probe = Command::new(&path);
        if probe
            .args([
                "-c",
                "import sys; sys.exit(0 if sys.version_info >= (3, 12) else 1)",
            ])
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Ok(Command::new(path));
        }
    }
    Err(
        "Python 3.12 or newer is required to prepare native TV support; set KOKO_PYTHON to its \
         executable"
            .into(),
    )
}

pub fn prepare(
    workspace: &Path,
    out: &Path,
    target: &str,
) -> BuildResult<Option<PathBuf>> {
    for variable in [
        "KOKO_TSDUCK_ROOT",
        "KOKO_SKIP_TSDUCK_BUILD",
        "KOKO_PYTHON",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    for file in [
        "native/live-tv/bridge.cpp",
        "native/live-tv/CMakeLists.txt",
        "packaging/live_tv.py",
        "packaging/live_tv.rs",
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
    let git_dir = super::ffmpeg::git_output(
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
    let revision = super::ffmpeg::git_output(&source, &["rev-parse", "HEAD"])?;
    let root = out.join("live-tv");
    let status = python()?
        .arg(workspace.join("packaging/live_tv.py"))
        .arg("--target")
        .arg(target)
        .arg("--destination")
        .arg(&root)
        .arg("--cache")
        .arg(workspace.join("target/live-tv-deps"))
        .arg("--source")
        .arg(source)
        .arg("--revision")
        .arg(revision)
        .arg("--jobs")
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "2".to_owned()))
        .status()?;
    if !status.success() || !root.join(library(target)).is_file() {
        return Err(
            "Unable to prepare Koko's native TV component; check the native build tools and the \
             build output above"
                .into(),
        );
    }
    Ok(Some(root))
}
