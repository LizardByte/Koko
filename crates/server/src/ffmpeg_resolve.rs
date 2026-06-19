//! Locating the ffmpeg/ffprobe executables used for media processing.
//!
//! GUI-launched processes (macOS `.app` bundles, some Linux desktop launchers)
//! do not inherit the user's shell PATH, so a bare `ffmpeg` lookup can fail
//! even when the binary is installed under e.g. `/opt/homebrew/bin`. This
//! module is the single source of truth for resolution and is used by BOTH
//! capability display and the actual transcode spawn, so "available in UI"
//! is equivalent to "actually launches".

use std::path::PathBuf;

use once_cell::sync::Lazy;

/// Binary base-names the resolver is allowed to resolve. Anything else is
/// rejected as [`ResolvedBinary::Missing`]. This is a security boundary: it
/// prevents a future bug or compromised setting from causing an arbitrary
/// `exec`.
pub const ALLOWED_BINARIES: &[&str] = &["ffmpeg", "ffprobe"];

/// How a binary was located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveSource {
    /// The configured value was an absolute path that exists and is executable.
    ConfiguredAbsolute,
    /// The configured bare name resolved on PATH.
    PathLookup,
    /// Found in a well-known install directory (not the configured path).
    WellKnown,
}

/// Result of resolving a single binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedBinary {
    /// The binary was found.
    Found {
        /// The absolute path to use when spawning the binary.
        resolved_path: PathBuf,
        /// How the binary was located.
        via: ResolveSource,
    },
    /// The binary could not be found. `checked_paths` lists every location
    /// inspected, for UI display.
    Missing {
        /// The configured value that failed to resolve.
        configured: String,
        /// Every location the resolver inspected, in order.
        checked_paths: Vec<PathBuf>,
    },
}

/// Platform-specific well-known directories, highest priority first.
static WELL_KNOWN_DIRS: Lazy<Vec<PathBuf>> = Lazy::new(well_known_dirs);

#[cfg(target_os = "macos")]
fn well_known_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/sbin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn well_known_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/usr/sbin"),
        PathBuf::from("/opt/ffmpeg/bin"),
        PathBuf::from("/snap/bin"),
    ]
}

#[cfg(target_os = "windows")]
fn well_known_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from(r"C:\Program Files\ffmpeg\bin"),
        PathBuf::from(r"C:\Program Files (x86)\ffmpeg\bin"),
        PathBuf::from(r"C:\ffmpeg\bin"),
    ];
    if let Ok(home) = std::env::var("USERPROFILE") {
        dirs.push(PathBuf::from(home).join("bin"));
    }
    dirs
}

/// Resolve a single binary.
///
/// `configured` is what the admin set (a bare name like `ffmpeg`, or an
/// absolute/relative path). `binary_name` is the canonical name being looked
/// up and MUST be in [`ALLOWED_BINARIES`].
///
/// Resolution order:
/// 1. If `configured` looks like a path (contains a separator) or is absolute,
///    validate its base name is allowed and it exists + is executable.
/// 2. Else treat `configured` as a bare name: require it to be allowed, then
///    look it up on PATH.
/// 3. Else fall back to the well-known directory list for `binary_name`.
/// 4. Else [`ResolvedBinary::Missing`] with every checked location.
pub fn resolve_binary(
    configured: &str,
    binary_name: &str,
) -> ResolvedBinary {
    resolve_binary_with(configured, binary_name, &WELL_KNOWN_DIRS, lookup_on_path)
}

/// Internal resolver that accepts an explicit well-known directory list and a
/// PATH-lookup function so tests can run hermetically (the static
/// [`WELL_KNOWN_DIRS`] and the real `PATH` reflect the actual machine, where
/// ffmpeg may be installed).
fn resolve_binary_with(
    configured: &str,
    binary_name: &str,
    well_known_dirs: &[PathBuf],
    path_lookup: impl Fn(&str) -> Option<PathBuf>,
) -> ResolvedBinary {
    if !ALLOWED_BINARIES.contains(&binary_name) {
        return ResolvedBinary::Missing {
            configured: configured.to_string(),
            checked_paths: Vec::new(),
        };
    }

    let mut checked: Vec<PathBuf> = Vec::new();
    let configured_path = PathBuf::from(configured);

    // 1. Configured-as-path: only if it actually has a path component, or is absolute.
    let is_path_form = configured_path
        .parent()
        .map(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(false)
        || configured_path.is_absolute();

    if is_path_form {
        if base_name_is_allowed(&configured_path) && is_executable(&configured_path) {
            return ResolvedBinary::Found {
                resolved_path: canonicalize(&configured_path),
                via: ResolveSource::ConfiguredAbsolute,
            };
        }
        checked.push(configured_path.clone());
    } else {
        // 2. Bare name: must be the allowed name itself.
        if configured == binary_name {
            if let Some(found) = path_lookup(binary_name) {
                return ResolvedBinary::Found {
                    resolved_path: found,
                    via: ResolveSource::PathLookup,
                };
            }
            checked.push(PathBuf::from(configured));
        }
    }

    // 3. Well-known directories.
    for dir in well_known_dirs.iter() {
        let candidate = dir.join(binary_name);
        if is_executable(&candidate) {
            return ResolvedBinary::Found {
                resolved_path: canonicalize(&candidate),
                via: ResolveSource::WellKnown,
            };
        }
        checked.push(candidate);
    }

    ResolvedBinary::Missing {
        configured: configured.to_string(),
        checked_paths: checked,
    }
}

/// Convenience: resolve ffmpeg using the configured value.
pub fn resolve_ffmpeg(configured: &str) -> ResolvedBinary {
    resolve_binary(configured, "ffmpeg")
}

/// Convenience: resolve ffprobe using the configured value.
pub fn resolve_ffprobe(configured: &str) -> ResolvedBinary {
    resolve_binary(configured, "ffprobe")
}

fn base_name_is_allowed(path: &std::path::Path) -> bool {
    // The base name of the configured path must itself be an allowed binary.
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| ALLOWED_BINARIES.contains(&name))
        .unwrap_or(false)
}

fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if !metadata.is_file() {
            return false;
        }
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }
}

fn canonicalize(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn lookup_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(canonicalize(&candidate));
        }
    }
    None
}

/// Run `<binary> -version` and return the first stdout line, or [`None`] on
/// failure. Used for capability display. Uses the synchronous std [`Command`]
/// because this is called rarely and from sync contexts (`detect_binary`).
pub fn probe_version(path: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new(path)
        .arg("-version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A binary is "executable enough" for these tests if it exists on disk;
    // we create real (empty) files rather than mocking stat calls.
    fn write_shim(
        dir: &std::path::Path,
        name: &str,
    ) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    // The public `resolve_binary` searches the real machine's well-known dirs
    // and PATH (where ffmpeg may actually be installed), so these tests use the
    // internal `resolve_binary_with` with controlled inputs to stay hermetic.
    const NO_WELL_KNOWN: &[PathBuf] = &[];
    fn no_path_lookup(_: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(unix)]
    #[test]
    fn absolute_configured_path_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = write_shim(dir.path(), "ffmpeg");
        let resolved = resolve_binary_with(
            ffmpeg.to_str().unwrap(),
            "ffmpeg",
            NO_WELL_KNOWN,
            no_path_lookup,
        );
        assert!(matches!(
            resolved,
            ResolvedBinary::Found {
                via: ResolveSource::ConfiguredAbsolute,
                ..
            }
        ));
        if let ResolvedBinary::Found { resolved_path, .. } = resolved {
            // macOS canonicalizes /var -> /private/var; compare canonical forms.
            assert_eq!(resolved_path, canonicalize(&ffmpeg));
        }
    }

    #[cfg(unix)]
    #[test]
    fn disallowed_base_name_is_rejected_even_as_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let rogue = write_shim(dir.path(), "not-ffmpeg");
        // The requested binary_name itself must be allowed; a rogue name is never resolved.
        assert!(matches!(
            resolve_binary_with(
                rogue.to_str().unwrap(),
                "rogue",
                NO_WELL_KNOWN,
                no_path_lookup
            ),
            ResolvedBinary::Missing { .. }
        ));
        // Asking for an allowed name via a path whose base name is rogue is also rejected.
        assert!(matches!(
            resolve_binary_with(
                rogue.to_str().unwrap(),
                "ffmpeg",
                NO_WELL_KNOWN,
                no_path_lookup
            ),
            ResolvedBinary::Missing { .. }
        ));
    }

    #[test]
    fn missing_returns_checked_paths() {
        // Empty well-known list + no PATH -> the only checked path is the configured one.
        let resolved = resolve_binary_with(
            "/definitely/does/not/exist/ffmpeg",
            "ffmpeg",
            NO_WELL_KNOWN,
            no_path_lookup,
        );
        match resolved {
            ResolvedBinary::Missing {
                configured,
                checked_paths,
            } => {
                assert_eq!(configured, "/definitely/does/not/exist/ffmpeg");
                assert_eq!(
                    checked_paths,
                    vec![PathBuf::from(
                        "/definitely/does/not/exist/ffmpeg"
                    )]
                );
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn well_known_dir_hit_reports_wellknown_source() {
        // A well-known dir containing the shim resolves via WellKnown, with PATH
        // lookup disabled so the real machine's ffmpeg can't interfere.
        let dir = tempfile::tempdir().unwrap();
        write_shim(dir.path(), "ffmpeg");
        let resolved = resolve_binary_with(
            "ffmpeg",
            "ffmpeg",
            &[dir.path().to_path_buf()],
            no_path_lookup,
        );
        match resolved {
            ResolvedBinary::Found {
                via: ResolveSource::WellKnown,
                ..
            } => {}
            other => panic!("expected Found via WellKnown, got {other:?}"),
        }
    }

    #[test]
    fn path_lookup_hit_reports_pathlookup_source() {
        // A bare name found via PATH lookup reports PathLookup, not WellKnown.
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = write_shim(dir.path(), "ffmpeg");
        let via_path = move |name: &str| {
            if name == "ffmpeg" { Some(ffmpeg.clone()) } else { None }
        };
        let resolved = resolve_binary_with("ffmpeg", "ffmpeg", NO_WELL_KNOWN, via_path);
        assert!(matches!(
            resolved,
            ResolvedBinary::Found {
                via: ResolveSource::PathLookup,
                ..
            }
        ));
    }

    #[test]
    fn allowed_binaries_constant_is_what_we_expect() {
        assert_eq!(ALLOWED_BINARIES, &["ffmpeg", "ffprobe"]);
    }

    #[cfg(unix)]
    #[test]
    fn probe_version_reads_first_stdout_line() {
        let dir = tempfile::tempdir().unwrap();
        // A shim that mimics `ffmpeg -version` first line.
        let shim = dir.path().join("ffmpeg");
        std::fs::write(
            &shim,
            "#!/bin/sh\necho 'ffmpeg version 7.0.2, built locally'\nexit 0\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&shim).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&shim, perms).unwrap();

        let version = probe_version(&shim);
        assert_eq!(
            version.as_deref(),
            Some("ffmpeg version 7.0.2, built locally")
        );
    }

    #[test]
    fn probe_version_returns_none_for_missing_binary() {
        assert!(probe_version(std::path::Path::new("/definitely/does/not/exist/ffmpeg")).is_none());
    }
}
