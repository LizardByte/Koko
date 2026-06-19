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
/// Resolution is literal — the resolver runs exactly what is configured and
/// never silently substitutes a binary found elsewhere. Smart discovery
/// (PATH + well-known directories) is a separate concern handled on demand by
/// the Detect UI; the runtime just executes the configured value.
///
/// Resolution order:
/// 1. If `configured` looks like a path (contains a separator) or is absolute,
///    validate its base name is allowed and it exists + is executable.
/// 2. Else treat `configured` as a bare name: require it to be allowed, then
///    look it up on PATH.
/// 3. Else [`ResolvedBinary::Missing`] with every checked location.
pub fn resolve_binary(
    configured: &str,
    binary_name: &str,
) -> ResolvedBinary {
    resolve_binary_with(configured, binary_name, lookup_on_path)
}

/// Internal resolver that accepts a PATH-lookup function so tests can run
/// hermetically (the real `PATH` reflects the actual machine, where ffmpeg may
/// be installed).
fn resolve_binary_with(
    configured: &str,
    binary_name: &str,
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

/// Encoders relevant to media transcoding that we surface in the discovery UI.
/// Anything else ffmpeg reports is ignored to keep the display focused.
const RELEVANT_ENCODERS: &[&str] = &[
    "libx264",
    "libx265",
    "libvpx-vp9",
    "libvpx",
    "libsvtav1",
    "libopus",
    "libmp3lame",
    "aac",
];

/// Run `ffmpeg -hide_banner -encoders` and return the subset of
/// [`RELEVANT_ENCODERS`] that this build supports, preserving the allow-list
/// order. Empty if the binary can't be probed. Only called during the on-demand
/// Detect action, so the one ffmpeg invocation is acceptable.
pub fn probe_encoders(path: &std::path::Path) -> Vec<String> {
    let output = match std::process::Command::new(path)
        .args(["-hide_banner", "-encoders"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    // Encoder data rows look like: ` V....D libx264   ... (codec h264)`.
    // Collect the encoder names that appear, then filter to the allow-list in
    // a stable order so the UI is deterministic.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let present: std::collections::HashSet<&str> =
        stdout.lines().filter_map(parse_encoder_name).collect();
    RELEVANT_ENCODERS
        .iter()
        .filter(|name| present.contains(**name))
        .map(|name| (*name).to_string())
        .collect()
}

/// Extract the encoder name from a `-encoders` data row. Rows begin with a
/// 6-char flags column (` V....D `, ` A..... `, etc.) followed by the name.
/// Legend rows like ` V..... = Video` are rejected because the "name" starts
/// with `=` rather than a letter.
fn parse_encoder_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(' ')?;
    // The flags column is exactly 6 chars (type + 5 flag dots/letters).
    let flags = rest.get(0..6)?;
    if !matches!(flags.as_bytes()[0], b'V' | b'A' | b'S') {
        return None;
    }
    let after_flags = rest.get(6..)?;
    let name = after_flags.trim_start();
    // Real encoder names start with a letter; legend rows yield `=` here.
    let first = name.as_bytes().first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let end = name.find(char::is_whitespace).unwrap_or(name.len());
    if end == 0 {
        return None;
    }
    Some(&name[..end])
}

/// Public accessor for the platform well-known directory list, for discovery.
pub fn well_known_dirs_public() -> Vec<PathBuf> {
    WELL_KNOWN_DIRS.clone()
}

/// Public accessor for the executability check, for discovery.
pub fn is_executable_public(path: &std::path::Path) -> bool {
    is_executable(path)
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

    // The public `resolve_binary` consults the real machine's PATH (where
    // ffmpeg may actually be installed), so these tests use the internal
    // `resolve_binary_with` with a controlled PATH-lookup function to stay
    // hermetic. The resolver no longer searches well-known directories — that
    // is the Detect UI's job, not the runtime's.
    fn no_path_lookup(_: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(unix)]
    #[test]
    fn absolute_configured_path_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = write_shim(dir.path(), "ffmpeg");
        let resolved = resolve_binary_with(ffmpeg.to_str().unwrap(), "ffmpeg", no_path_lookup);
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
            resolve_binary_with(rogue.to_str().unwrap(), "rogue", no_path_lookup),
            ResolvedBinary::Missing { .. }
        ));
        // Asking for an allowed name via a path whose base name is rogue is also rejected.
        assert!(matches!(
            resolve_binary_with(rogue.to_str().unwrap(), "ffmpeg", no_path_lookup),
            ResolvedBinary::Missing { .. }
        ));
    }

    #[test]
    fn missing_returns_checked_paths() {
        // No PATH hit -> the only checked path is the configured one.
        let resolved = resolve_binary_with(
            "/definitely/does/not/exist/ffmpeg",
            "ffmpeg",
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
    fn path_lookup_hit_reports_pathlookup_source() {
        // A bare name found via PATH lookup reports PathLookup.
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = write_shim(dir.path(), "ffmpeg");
        let via_path = move |name: &str| {
            if name == "ffmpeg" { Some(ffmpeg.clone()) } else { None }
        };
        let resolved = resolve_binary_with("ffmpeg", "ffmpeg", via_path);
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

    #[test]
    fn parse_encoder_name_extracts_from_data_row() {
        assert_eq!(
            parse_encoder_name(" V....D libx264              libx264 H.264 (codec h264)"),
            Some("libx264")
        );
        assert_eq!(
            parse_encoder_name(" A....D aac                  AAC (Advanced Audio Coding)"),
            Some("aac")
        );
    }

    #[test]
    fn parse_encoder_name_rejects_legend_and_blank() {
        assert_eq!(parse_encoder_name(" V..... = Video"), None);
        assert_eq!(parse_encoder_name("Encoders:"), None);
        assert_eq!(parse_encoder_name(""), None);
        assert_eq!(parse_encoder_name("not a row"), None);
    }

    #[cfg(unix)]
    #[test]
    fn probe_encoders_returns_relevant_subset_in_allow_list_order() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("ffmpeg");
        // A shim that emits a few -encoders rows mixing relevant + irrelevant names.
        // Recognized: libx264, libx265, aac. Irrelevant/ignored: foo, alias_pix.
        // Order in output is intentionally not allow-list order.
        let script = "#!/bin/sh\n\
            echo ' V....D libx265              H.265 (codec hevc)'\n\
            echo ' V....D alias_pix           Alias PIX (codec alias_pix)'\n\
            echo ' V....D libx264              H.264 (codec h264)'\n\
            echo ' V....D foo                 not relevant (codec foo)'\n\
            echo ' A....D aac                 AAC (codec aac)'\n\
            exit 0\n";
        std::fs::write(&shim, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&shim).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&shim, perms).unwrap();

        let encoders = probe_encoders(&shim);
        // Subset of the allow-list, in allow-list order (libx264 before libx265
        // even though libx265 appeared first in the output).
        assert_eq!(
            encoders,
            vec![
                "libx264".to_string(),
                "libx265".to_string(),
                "aac".to_string()
            ]
        );
    }

    #[test]
    fn probe_encoders_returns_empty_for_missing_binary() {
        assert!(
            probe_encoders(std::path::Path::new("/definitely/does/not/exist/ffmpeg")).is_empty()
        );
    }

    /// Smoke test that only runs when invoked with `--features real-ffmpeg-tests`
    /// AND ffmpeg is actually installed. Skipped in normal CI so the suite stays
    /// hermetic; locally, a developer with ffmpeg can opt in.
    #[cfg(feature = "real-ffmpeg-tests")]
    #[test]
    fn real_ffmpeg_on_path_is_found() {
        match resolve_binary("ffmpeg", "ffmpeg") {
            ResolvedBinary::Found { via, .. } => {
                println!("resolved ffmpeg via {via:?}");
            }
            ResolvedBinary::Missing { checked_paths, .. } => {
                panic!(
                    "real-ffmpeg-tests feature is enabled but ffmpeg was not found; checked: {checked_paths:?}"
                );
            }
        }
    }
}
