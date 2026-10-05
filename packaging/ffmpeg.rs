//! Prepare the media tools associated with the pinned build-deps submodule.

use std::{
    env,
    error::Error,
    fs,
    path::{
        Path,
        PathBuf,
    },
    process::Command,
};

type BuildResult<T> = Result<T, Box<dyn Error>>;

pub fn package_platform(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-musl" => Some("Alpine-x86_64"),
        "aarch64-unknown-linux-musl" => Some("Alpine-aarch64"),
        "x86_64-unknown-linux-gnu" => Some("Linux-x86_64"),
        "aarch64-unknown-linux-gnu" => Some("Linux-aarch64"),
        "powerpc64le-unknown-linux-gnu" => Some("Linux-ppc64le"),
        "x86_64-unknown-freebsd" => Some("FreeBSD-amd64"),
        "x86_64-apple-darwin" => Some("Darwin-x86_64"),
        "aarch64-apple-darwin" => Some("Darwin-arm64"),
        "x86_64-pc-windows-msvc" | "x86_64-pc-windows-gnu" => Some("Windows-AMD64"),
        "aarch64-pc-windows-msvc" | "aarch64-pc-windows-gnullvm" => Some("Windows-ARM64"),
        _ => None,
    }
}

// IDEs can inherit PATH before Git or Windows system tools were added to it.
fn tool_command(program: &str) -> Command {
    #[cfg(windows)]
    {
        let executable = format!("{program}.exe");
        // Git Bash tar treats drive-letter archive paths as remote hosts.
        if matches!(program, "curl" | "tar") {
            if let Some(directory) = env::var_os("SystemRoot") {
                let path = Path::new(&directory).join("System32").join(&executable);
                if path.is_file() {
                    return Command::new(path);
                }
            }
        }
        if let Some(path) = env::var_os("PATH").and_then(|path| {
            env::split_paths(&path)
                .map(|directory| directory.join(&executable))
                .find(|path| path.is_file())
        }) {
            return Command::new(path);
        }
        if program == "git" {
            for variable in [
                "ProgramW6432",
                "ProgramFiles",
                "ProgramFiles(x86)",
                "LOCALAPPDATA",
            ] {
                if let Some(directory) = env::var_os(variable) {
                    let directory = PathBuf::from(directory);
                    let directory = if variable == "LOCALAPPDATA" {
                        directory.join("Programs")
                    } else {
                        directory
                    };
                    let path = directory.join("Git/cmd/git.exe");
                    if path.is_file() {
                        return Command::new(path);
                    }
                }
            }
        }
    }
    Command::new(program)
}

fn git_output(
    directory: &Path,
    args: &[&str],
) -> BuildResult<String> {
    let output = tool_command("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .map_err(|error| format!("Could not execute git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn release_tag(directory: &Path) -> BuildResult<String> {
    if !directory.join(".git").exists() {
        return Err("Initialize build-deps with: git submodule update --init \
                    third-party/build-deps"
            .into());
    }
    // Keep Cargo aware of detached HEAD changes even when source file contents are identical.
    let git_dir = git_output(
        directory,
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
    let tag = git_output(
        directory,
        &[
            "describe",
            "--tags",
            "--exact-match",
        ],
    )?;
    if !tag.starts_with('v') || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
        return Err(format!("Invalid build-deps release tag: {tag}").into());
    }
    Ok(tag)
}

fn run(command: &mut Command) -> BuildResult<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    if !command
        .status()
        .map_err(|error| format!("Could not execute {program}: {error}"))?
        .success()
    {
        return Err(
            format!("{program} failed while preparing the build-deps media package").into(),
        );
    }
    Ok(())
}

fn has_tools(
    directory: &Path,
    extension: &str,
) -> bool {
    ["ffmpeg", "ffprobe"].iter().all(|name| {
        directory
            .join("bin")
            .join(format!("{name}{extension}"))
            .is_file()
    }) && directory.join("share/licenses").is_dir()
}

pub fn prepare(
    workspace: &Path,
    out_dir: &Path,
    target: &str,
) -> BuildResult<Option<PathBuf>> {
    for variable in [
        "KOKO_FFMPEG_ROOT",
        "KOKO_SKIP_FFMPEG_DOWNLOAD",
        "CARGO_NET_OFFLINE",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    if env::var("KOKO_SKIP_FFMPEG_DOWNLOAD").as_deref() == Ok("1") {
        return Ok(None);
    }
    let extension = if target.contains("windows") { ".exe" } else { "" };
    if let Some(directory) = env::var_os("KOKO_FFMPEG_ROOT") {
        let directory = fs::canonicalize(directory)?;
        if !has_tools(&directory, extension) {
            return Err(
                "KOKO_FFMPEG_ROOT must contain bin/ffmpeg, bin/ffprobe, and share/licenses for \
                 the target"
                    .into(),
            );
        }
        return Ok(Some(directory));
    }
    let platform = package_platform(target).ok_or_else(|| {
        format!(
            "No build-deps media package for {target}; set KOKO_FFMPEG_ROOT or \
             KOKO_SKIP_FFMPEG_DOWNLOAD=1"
        )
    })?;
    let submodule = workspace.join("third-party/build-deps");
    let offline = env::var("CARGO_NET_OFFLINE").as_deref() == Ok("true");
    let tag = match release_tag(&submodule) {
        Ok(tag) => tag,
        Err(error) => {
            if offline || !submodule.join(".git").exists() {
                return Err(error);
            }
            // Shallow submodule checkouts may not contain the release tag yet.
            run(tool_command("git").arg("-C").arg(&submodule).args([
                "fetch",
                "--tags",
                "--depth=1",
            ]))?;
            release_tag(&submodule).map_err(|_| {
                "The build-deps submodule must be checked out at a published release tag"
            })?
        }
    };
    let package_dir = out_dir.join(format!("ffmpeg-{tag}")).join(platform);
    let root = package_dir.join("ffmpeg");
    if !has_tools(&root, extension) {
        fs::create_dir_all(&package_dir)?;
        let archive_name = format!("{platform}-ffmpeg.tar.gz");
        let archive = package_dir.join(&archive_name);
        if !archive.is_file() {
            if offline {
                return Err(format!(
                    "No cached build-deps package for {tag}/{platform}; set KOKO_FFMPEG_ROOT for \
                     an offline build"
                )
                .into());
            }
            let url = format!(
                "https://github.com/LizardByte/build-deps/releases/download/{tag}/{archive_name}"
            );
            let partial = package_dir.join(format!("{archive_name}.part"));
            eprintln!("Downloading media tools from build-deps tag {tag} for {platform}");
            run(tool_command("curl")
                .args([
                    "--fail",
                    "--location",
                    "--retry",
                    "3",
                    "--max-time",
                    "300",
                    &url,
                    "--output",
                ])
                .arg(&partial))?;
            fs::rename(partial, &archive)?;
        }
        run(tool_command("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&package_dir))?;
        if !has_tools(&root, extension) {
            return Err(
                "The build-deps archive did not contain the expected media tools and licenses"
                    .into(),
            );
        }
    }
    Ok(Some(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };

    #[test]
    fn selects_target_packages_independently_of_the_host() {
        for (target, expected) in [
            ("x86_64-unknown-linux-musl", "Alpine-x86_64"),
            ("aarch64-unknown-linux-musl", "Alpine-aarch64"),
            ("x86_64-unknown-linux-gnu", "Linux-x86_64"),
            ("aarch64-unknown-linux-gnu", "Linux-aarch64"),
            ("powerpc64le-unknown-linux-gnu", "Linux-ppc64le"),
            ("x86_64-unknown-freebsd", "FreeBSD-amd64"),
            ("x86_64-apple-darwin", "Darwin-x86_64"),
            ("aarch64-apple-darwin", "Darwin-arm64"),
            ("x86_64-pc-windows-msvc", "Windows-AMD64"),
            ("aarch64-pc-windows-msvc", "Windows-ARM64"),
        ] {
            assert_eq!(package_platform(target), Some(expected));
        }
        assert_eq!(package_platform("aarch64-linux-android"), None);
    }

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> PathBuf {
        let directory = env::temp_dir().join(format!(
            "koko-build-deps-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        run(tool_command("git")
            .arg("init")
            .arg("--quiet")
            .arg(&directory))
        .unwrap();
        run(tool_command("git").arg("-C").arg(&directory).args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "fixture",
        ]))
        .unwrap();
        directory
    }

    #[test]
    fn uses_only_a_tag_on_the_current_submodule_commit() {
        let directory = fixture();
        run(tool_command("git")
            .arg("-C")
            .arg(&directory)
            .args(["tag", "v2026.1004.34232"]))
        .unwrap();
        assert_eq!(release_tag(&directory).unwrap(), "v2026.1004.34232");
        run(tool_command("git").arg("-C").arg(&directory).args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "untagged",
        ]))
        .unwrap();
        assert!(release_tag(&directory).is_err());
    }

    #[test]
    fn refuses_an_uninitialized_submodule() {
        assert!(
            release_tag(Path::new("not-a-build-deps-checkout"))
                .unwrap_err()
                .to_string()
                .contains("git submodule update --init")
        );
    }

    #[cfg(windows)]
    #[test]
    fn finds_windows_build_tools_with_incomplete_path() {
        const CHILD_ENV: &str = "KOKO_TEST_WINDOWS_TOOLS_WITHOUT_PATH";
        if env::var(CHILD_ENV).as_deref() != Ok("1") {
            let directory = env::temp_dir().join(format!(
                "koko-tool-path-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            for program in ["curl", "tar"] {
                fs::write(directory.join(format!("{program}.exe")), b"PATH fixture").unwrap();
            }
            for path in [
                "",
                directory.to_str().unwrap(),
            ] {
                let output = Command::new(env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "tests::finds_windows_build_tools_with_incomplete_path",
                        "--nocapture",
                    ])
                    .env("PATH", path)
                    .env(CHILD_ENV, "1")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            fs::remove_dir_all(directory).unwrap();
            return;
        }
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(
            release_tag(&workspace.join("third-party/build-deps"))
                .unwrap()
                .starts_with('v')
        );
        for program in ["curl", "tar"] {
            let native = PathBuf::from(env::var_os("SystemRoot").unwrap())
                .join("System32")
                .join(format!("{program}.exe"));
            let mut command = tool_command(program);
            assert_eq!(command.get_program(), native.as_os_str());
            run(command.arg("--version")).unwrap();
        }
    }
}
