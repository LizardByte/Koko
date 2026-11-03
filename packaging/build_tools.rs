//! Locate and invoke native build tools consistently across Cargo and xtask.

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

pub(crate) fn find_program(program: &str) -> Option<PathBuf> {
    let executable = if cfg!(windows) && Path::new(program).extension().is_none() {
        format!("{program}.exe")
    } else {
        program.to_owned()
    };
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(&executable))
            .find(|path| is_executable(path))
    })
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

// GUI-launched builds can inherit PATH before Git or Windows system tools were added.
pub(crate) fn tool_command(program: &str) -> Command {
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
        if let Some(path) = find_program(program) {
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
    Command::new(find_program(program).unwrap_or_else(|| PathBuf::from(program)))
}

pub(crate) fn git_output(
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

pub(crate) fn run(command: &mut Command) -> BuildResult<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    if !command
        .status()
        .map_err(|error| format!("Could not execute {program}: {error}"))?
        .success()
    {
        return Err(format!("{program} failed while preparing native dependencies").into());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::is_executable;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
    };

    #[test]
    fn program_lookup_rejects_nonexecutables_directories_and_missing_paths() {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("tool");
        fs::write(&program, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_executable(&program));
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&program));
        assert!(!is_executable(root.path()));
        assert!(!is_executable(&root.path().join("missing")));
    }
}
