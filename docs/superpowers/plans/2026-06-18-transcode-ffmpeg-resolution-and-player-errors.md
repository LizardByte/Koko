# Transcode FFmpeg Resolution & Player Error Surfacing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix transcoding failures (ffmpeg not resolvable from GUI-launched processes; player shows black screen instead of an error) by adding a robust ffmpeg resolver, a discovery/settings UI, structured stream errors, and a layered client error UX — all timer-free.

**Architecture:** A new `ffmpeg_resolve` module is the single source of truth for locating ffmpeg/ffprobe, used by both capability detection and the actual spawn (so "available in UI" ⟺ "actually launches"). Spawn failures return a structured `TranscodeErrorBody` instead of a bare 500. The client does a deterministic capabilities preflight before playback and surfaces failures through an existing in-player overlay plus the global banner. Two inert seams (per-session error store + shared error-mapper) are laid for a deferred lifecycle phase. Timed/lifecycle work is out of scope (see the companion discovery/plan docs).

**Tech Stack:** Rust + Rocket 0.5 + tokio + rocket_okapi + serde (server); TypeScript SPA (client-web). Tests: inline `#[cfg(test)]` + integration tests via `create_test_client`. `tempfile` added as a dev-dependency for hermetic resolver tests. No new realtime transport.

**Spec:** `docs/superpowers/specs/2026-06-18-transcode-ffmpeg-resolution-and-player-errors-design.md` (read it first).

---

## Conventions for this plan

- **Formatting:** always `cargo +nightly fmt` (per `AGENTS.md`). Run after every code change.
- **Testing:** pure logic → inline `#[cfg(test)] mod tests` (pattern: `crates/server/src/media.rs`); HTTP routes → integration tests in `crates/server/tests/test_web/routes/` using `create_test_client` + `make_request` (pattern: `tests/test_web/routes/settings.rs`). The project uses `#[rocket::async_test]` for async integration tests and `#[test]` for inline.
- **Package name:** the server crate is `koko` (not `koko-server`). Use `cargo test -p koko ...` and `cargo build -p koko`. The test binary is `main` (`cargo test -p koko --test main ...`).
- **Route style:** `#[openapi(tag = "Media")]` + snake_case fn, registered in `crates/server/src/web/routes/mod.rs` `api_routes()` via `openapi_get_routes!`. Admin routes take `_admin_guard: AdminGuard` (`crates/server/src/auth.rs:90`).
- **Type style:** response types derive `(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)` with a doc comment per field (pattern: `BinaryCapability` at `media.rs:115`).
- **Commits:** one logical change per commit, frequent. This plan uses `feat:`/`test:`/`refactor:` prefixes.
- **The shell-path framing is a red herring** (spec §1.1): `Command::args` bypasses the shell; spaces/brackets are passed verbatim. Do NOT add quoting anywhere — it would be dead code.

## Security decision (spec §4.1 Note 4): binary-name allow-list

The resolver accepts a configured path/name from settings. To prevent path-lookup abuse (a future vulnerability leaking an attacker-controlled path into an `exec`), `resolve_binary` enforces a **hardcoded allow-list of binary base-names** (`ffmpeg`, `ffprobe`). Behavior:

- If the configured value is a **bare name**: it must be in the allow-list; otherwise `Missing`. (A bare name is only ever looked up as that name.)
- If the configured value is a **path** (absolute or relative with a separator): the **base name** (`file_name()`) must be in the allow-list; otherwise `Missing`. The path itself can point anywhere the admin chose — we only constrain that it *is* an ffmpeg/ffprobe binary by name.

This blocks `exec("/some/path/anything")` while still allowing absolute paths to ffmpeg installed anywhere. It is documented in `ffmpeg_resolve.rs` and asserted by tests.

## Resolver path lists (spec §4.1 Notes 2–3): package-manager coverage

Well-known search directories, in priority order (first hit wins). Lower-priority managers are simply listed further down — no priority property.

- **macOS:** `/opt/homebrew/bin` (Apple Silicon), `/usr/local/bin` (Intel Homebrew + manual), `/opt/homebrew/sbin`, `/usr/local/sbin`, `/usr/bin`, `/bin`
- **Linux:** `/usr/local/bin`, `/usr/bin`, `/bin`, `/usr/sbin`, `/opt/ffmpeg/bin`, `/snap/bin` (Snap), `/home/.nix-profile/bin` is user-specific so **excluded** (Nix users set an absolute path)
- **Windows:** `C:\Program Files\ffmpeg\bin`, `C:\Program Files (x86)\ffmpeg\bin`, `%USERPROFILE%\bin`, `C:\ffmpeg\bin`, plus `%LOCALAPPDATA%\Microsoft\WinGet\Packages\*\ffmpeg*\bin` is glob-based and **excluded** for simplicity (WinGet users set an absolute path or rely on PATH)

Bare-name-vs-path distinction (Note 1): meaningful only for the allow-list check and for whether we attempt PATH lookup vs absolute-path validation. The resolver handles both uniformly via `Path::new(configured)` — see Task 1.

---

## File structure

**New files:**
- `crates/server/src/ffmpeg_resolve.rs` — the resolver module (logic + inline tests).
- `crates/server/tests/test_web/routes/tools.rs` — integration tests for the discover + status endpoints.

**Modified files (server):**
- `crates/server/src/lib.rs` — declare `pub mod ffmpeg_resolve;`
- `crates/server/Cargo.toml` — add `tempfile` dev-dependency + `real-ffmpeg-tests` feature.
- `crates/server/src/transcode.rs` — resolved-path spawn, `SpawnTranscodeError`, `TranscodeErrorBody`, `map_transcode_error`, inline tests.
- `crates/server/src/media.rs` — `detect_binary` delegates to resolver.
- `crates/server/src/web/routes/media.rs` — `ToolDiscoveryResponse`/`ToolCandidate`/`BinaryProbe`/`SessionStatusResponse` types, `discover_transcoding_tools` route, `get_session_status` route, `ACTIVE_SESSION_ERRORS` + `record_session_error` (inert), structured stream-error in `get_session_stream`.
- `crates/server/src/web/routes/mod.rs` — register the two new routes.
- `crates/server/tests/test_web/routes/mod.rs` — declare `pub mod tools;`.

**Modified files (client):**
- `crates/client-web/src/api.ts` — `discoverTranscodingTools()`, `getSessionStatus()`, `ToolDiscoveryResponse`/`TranscodeErrorBody`/`SessionStatusResponse` types.
- `crates/client-web/src/app/types.ts` — `playbackError` on `AppState`.
- `crates/client-web/src/app/settingsView.ts` — detect button + directory radios.
- `crates/client-web/src/app/playbackController.ts` — preflight in `startPlayback`, status-read recovery in the `<video>` error handler, dynamic overlay/banner copy.

---

## Task 1: `ffmpeg_resolve` module — resolution logic

**Files:**
- Create: `crates/server/src/ffmpeg_resolve.rs`
- Modify: `crates/server/src/lib.rs` (add module declaration)
- Modify: `crates/server/Cargo.toml` (add `tempfile` dev-dependency)

- [ ] **Step 1: Add `tempfile` dev-dependency**

Modify `crates/server/Cargo.toml` `[dev-dependencies]` (currently lines 102–104):

```toml
[dev-dependencies]
async-std.workspace = true
rstest.workspace = true
tempfile = "3.13"
```

- [ ] **Step 2: Declare the module**

In `crates/server/src/lib.rs`, add alongside the other `pub mod` declarations (match existing style):

```rust
pub mod ffmpeg_resolve;
```

- [ ] **Step 3: Write the failing tests**

Create `crates/server/src/ffmpeg_resolve.rs` with only the test module first (the implementation doesn't exist yet, so this fails to compile — that's the "red"):

```rust
//! Locating the ffmpeg/ffprobe executables used for media processing.
//!
//! GUI-launched processes (macOS `.app` bundles, some Linux desktop launchers)
//! do not inherit the user's shell PATH, so a bare `ffmpeg` lookup can fail
//! even when the binary is installed under e.g. `/opt/homebrew/bin`. This
//! module is the single source of truth for resolution and is used by BOTH
//! capability display and the actual transcode spawn, so "available in UI"
//! is equivalent to "actually launches".

// standard imports
use std::path::PathBuf;

// lib imports
use once_cell::sync::Lazy;

/// Binary base-names the resolver is allowed to resolve. Anything else is
/// rejected as `Missing`. This is a security boundary: it prevents a
/// future bug or compromised setting from causing an arbitrary `exec`.
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
        resolved_path: PathBuf,
        via: ResolveSource,
    },
    /// The binary could not be found. `checked_paths` lists every location
    /// inspected, for UI display.
    Missing {
        configured: String,
        checked_paths: Vec<PathBuf>,
    },
}

/// Platform-specific well-known directories, highest priority first.
static WELL_KNOWN_DIRS: Lazy<Vec<PathBuf>> = Lazy::new(well_known_dirs);

#[cfg(not(target_os = "windows"))]
fn well_known_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/sbin"),
            PathBuf::from("/usr/local/sbin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/opt/ffmpeg/bin"),
            PathBuf::from("/snap/bin"),
        ]
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // A binary is "executable enough" for these tests if it exists on disk;
    // we create real (empty) files rather than mocking stat calls.
    fn write_shim(dir: &std::path::Path, name: &str) -> PathBuf {
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

    #[cfg(unix)]
    #[test]
    fn absolute_configured_path_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let ffmpeg = write_shim(dir.path(), "ffmpeg");
        let resolved = resolve_binary(ffmpeg.to_str().unwrap(), "ffmpeg");
        assert!(matches!(
            resolved,
            ResolvedBinary::Found { via: ResolveSource::ConfiguredAbsolute, .. }
        ));
        if let ResolvedBinary::Found { resolved_path, .. } = resolved {
            assert_eq!(resolved_path, ffmpeg);
        }
    }

    #[cfg(unix)]
    #[test]
    fn disallowed_base_name_is_rejected_even_as_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let rogue = write_shim(dir.path(), "not-ffmpeg");
        let resolved = resolve_binary(rogue.to_str().unwrap(), "ffmpeg");
        // The requested binary_name itself must be allowed; a rogue name is never resolved.
        assert!(matches!(resolve_binary(rogue.to_str().unwrap(), "rogue"), ResolvedBinary::Missing { .. }));
        // Asking for an allowed name via a path whose base name is rogue is also rejected.
        let _ = resolved; // (configured-absolute path with wrong base name -> Missing; see impl)
        assert!(matches!(
            resolve_binary(rogue.to_str().unwrap(), "ffmpeg"),
            ResolvedBinary::Missing { .. }
        ));
    }

    #[test]
    fn missing_returns_checked_paths() {
        let resolved = resolve_binary("/definitely/does/not/exist/ffmpeg", "ffmpeg");
        match resolved {
            ResolvedBinary::Missing { configured, checked_paths } => {
                assert_eq!(configured, "/definitely/does/not/exist/ffmpeg");
                assert!(!checked_paths.is_empty());
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn allowed_binaries_constant_is_what_we_expect() {
        assert_eq!(ALLOWED_BINARIES, &["ffmpeg", "ffprobe"]);
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail (compile error)**

Run: `cargo test -p koko --lib ffmpeg_resolve 2>&1 | head -30`
Expected: FAIL — the tests reference `resolve_binary`, which is not yet defined. (The `#[cfg(unix)]` shims also need `tempfile`, added in Step 1.)

- [ ] **Step 5: Write the minimal implementation**

Append to `crates/server/src/ffmpeg_resolve.rs` (above the test module — move the test module to the bottom of the file):

```rust
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
/// 4. Else `Missing` with every checked location.
pub fn resolve_binary(configured: &str, binary_name: &str) -> ResolvedBinary {
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
        if base_name_is_allowed(&configured_path, binary_name) && is_executable(&configured_path) {
            return ResolvedBinary::Found {
                resolved_path: canonicalize(&configured_path),
                via: ResolveSource::ConfiguredAbsolute,
            };
        }
        checked.push(configured_path.clone());
    } else {
        // 2. Bare name: must be the allowed name itself.
        if configured == binary_name {
            if let Some(found) = lookup_on_path(binary_name) {
                return ResolvedBinary::Found {
                    resolved_path: found,
                    via: ResolveSource::PathLookup,
                };
            }
            checked.push(PathBuf::from(configured));
        }
    }

    // 3. Well-known directories.
    for dir in WELL_KNOWN_DIRS.iter() {
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

fn base_name_is_allowed(path: &std::path::Path, _binary_name: &str) -> bool {
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
    #[cfg(windows)]
    {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
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

#[cfg(test)]
mod tests {
    // ... (the test module from Step 3 goes here, unchanged)
}
```

(Note: delete the duplicate test module from Step 3's position and keep a single copy at the bottom of the file.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p koko --lib ffmpeg_resolve 2>&1 | tail -20`
Expected: PASS — all `ffmpeg_resolve::tests::*` pass. On Windows the two `#[cfg(unix)]` tests are skipped.

- [ ] **Step 7: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/src/ffmpeg_resolve.rs crates/server/src/lib.rs crates/server/Cargo.toml
git commit -m "feat(server): add ffmpeg_resolve module with allow-listed binary resolution"
```

---

## Task 2: Add version probing + thread the resolver into `detect_binary`

**Files:**
- Modify: `crates/server/src/ffmpeg_resolve.rs` (add `probe_version`)
- Modify: `crates/server/src/media.rs:5997` (`detect_binary` delegates to resolver)

- [ ] **Step 1: Write the failing test for version probing**

In `ffmpeg_resolve.rs` test module, add (Unix-only, uses a shim that prints a version line):

```rust
    #[cfg(unix)]
    #[test]
    fn probe_version_reads_first_stdout_line() {
        let dir = tempfile::tempdir().unwrap();
        // A shim that mimics `ffmpeg -version` first line.
        let shim = dir.join("ffmpeg");
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
        assert_eq!(version.as_deref(), Some("ffmpeg version 7.0.2, built locally"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p koko --lib ffmpeg_resolve::tests::probe_version 2>&1 | tail -10`
Expected: FAIL — `probe_version` undefined.

- [ ] **Step 3: Implement `probe_version`**

In `ffmpeg_resolve.rs`, add (with the other free functions):

```rust
/// Run `<binary> -version` and return the first stdout line, or None on failure.
/// Used for capability display. Uses the synchronous std Command because this
/// is called rarely and from sync contexts (detect_binary).
pub fn probe_version(path: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new(path).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p koko --lib ffmpeg_resolve::tests::probe_version 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Refactor `detect_binary` to delegate to the resolver**

Locate `detect_binary` at `crates/server/src/media.rs:5997`. Replace its body to use the resolver. First read the surrounding `BinaryCapability` struct (it's at `media.rs:115`) to preserve its shape.

Change the `detect_binary` function from a hand-rolled `Command::new(binary).arg("-version")` to:

```rust
fn detect_binary(configured: &str, binary_name: &str) -> BinaryCapability {
    use crate::ffmpeg_resolve::{
        self,
        ResolveSource,
        ResolvedBinary,
    };

    match ffmpeg_resolve::resolve_binary(configured, binary_name) {
        ResolvedBinary::Found { resolved_path, via } => {
            let version = ffmpeg_resolve::probe_version(&resolved_path);
            log::info!(
                "Resolved {} via {:?}: {}",
                binary_name,
                via,
                resolved_path.display()
            );
            BinaryCapability {
                configured_path: configured.to_string(),
                available: true,
                version,
                error: None,
            }
        }
        ResolvedBinary::Missing { configured, checked_paths } => {
            let checked_display = checked_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            log::warn!(
                "Could not resolve {}; checked: [{}]",
                binary_name,
                checked_display
            );
            BinaryCapability {
                configured_path: configured,
                available: false,
                version: None,
                error: Some(format!(
                    "Could not find {binary_name}. Checked: {checked_display}"
                )),
            }
        }
    }
}
```

Then update the two callers `inspect_transcoding_capability` (`media.rs:785`) and any other call sites that currently call `detect_binary(&settings.ffmpeg_path)` — they must pass the binary name:

```rust
pub fn inspect_transcoding_capability(settings: &FfmpegSettings) -> TranscodingCapability {
    TranscodingCapability {
        ffmpeg: detect_binary(&settings.ffmpeg_path, "ffmpeg"),
        ffprobe: detect_binary(&settings.ffprobe_path, "ffprobe"),
    }
}
```

Search for other `detect_binary(` call sites and fix them similarly: `grep -n "detect_binary(" crates/server/src/`.

- [ ] **Step 6: Build and run the existing media tests**

Run: `cargo test -p koko --lib 2>&1 | tail -20`
Expected: PASS — existing `media::tests` still pass (they don't touch `detect_binary`, but the build must succeed).

- [ ] **Step 7: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/src/ffmpeg_resolve.rs crates/server/src/media.rs
git commit -m "refactor(server): detect_binary delegates to ffmpeg_resolve with version probe"
```

---

## Task 3: `SpawnTranscodeError` + `TranscodeErrorBody` + `map_transcode_error` (seam #2)

**Files:**
- Modify: `crates/server/src/transcode.rs`

- [ ] **Step 1: Write the failing tests for the mapper**

At the bottom of `crates/server/src/transcode.rs`, add an inline test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn executable_missing_maps_to_open_settings_action() {
        let err = SpawnTranscodeError::ExecutableMissing {
            checked_paths: vec![PathBuf::from("/usr/bin/ffmpeg")],
        };
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, rocket::http::Status::ServiceUnavailable);
        assert_eq!(body.code, "transcode_executable_missing");
        assert_eq!(body.action, Some("open_settings"));
        assert!(body.message.contains("ffmpeg"));
    }

    #[test]
    fn io_error_maps_to_failed_with_no_action() {
        let err = SpawnTranscodeError::Io(std::io::Error::other("boom"));
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, rocket::http::Status::InternalServerError);
        assert_eq!(body.code, "transcode_failed");
        assert_eq!(body.action, None);
    }

    #[test]
    fn bad_input_maps_to_input_error() {
        let err = SpawnTranscodeError::BadInput {
            ffmpeg_stderr: "No such file".into(),
        };
        let (status, body) = map_transcode_error(err);
        assert_eq!(status, rocket::http::Status::UnprocessableEntity);
        assert_eq!(body.code, "transcode_input_error");
        assert_eq!(body.action, None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p koko --lib transcode::tests 2>&1 | tail -15`
Expected: FAIL — `SpawnTranscodeError`, `TranscodeErrorBody`, `map_transcode_error` undefined.

- [ ] **Step 3: Add the types and mapper**

Near the top of `crates/server/src/transcode.rs` (after the imports, before `TranscodeProcess`), add:

```rust
use rocket::http::Status;

/// A structured error from attempting to spawn a transcode.
#[derive(Debug)]
pub enum SpawnTranscodeError {
    /// ffmpeg could not be resolved/executed. `checked_paths` lists where we looked.
    ExecutableMissing { checked_paths: Vec<std::path::PathBuf> },
    /// ffmpeg started but reported its input was unusable. (Produced by the
    /// deferred lifecycle phase; declared here so the shared mapper is complete.)
    BadInput { ffmpeg_stderr: String },
    /// Any other spawn-time I/O error.
    Io(std::io::Error),
}

impl From<std::io::Error> for SpawnTranscodeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// The JSON body returned to clients when a transcode fails. The `action`
/// field lets the UI decide whether to show an actionable control (e.g.
/// "Open settings") for this kind of failure.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscodeErrorBody {
    /// Stable machine code: `transcode_executable_missing` | `transcode_input_error` | `transcode_failed`.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Optional UI action hint, e.g. `Some("open_settings")`.
    pub action: Option<&'static str>,
}

/// Map a [`SpawnTranscodeError`] to an HTTP status + body. This is the single
/// error-shaping function reused by the route handler today and the lifecycle
/// watcher in a later phase.
pub fn map_transcode_error(error: SpawnTranscodeError) -> (Status, TranscodeErrorBody) {
    match error {
        SpawnTranscodeError::ExecutableMissing { checked_paths } => {
            let checked = checked_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (
                Status::ServiceUnavailable,
                TranscodeErrorBody {
                    code: "transcode_executable_missing",
                    message: format!(
                        "FFmpeg could not be found. Install it or set its path in Settings. \
                         Checked: [{checked}]"
                    ),
                    action: Some("open_settings"),
                },
            )
        }
        SpawnTranscodeError::BadInput { ffmpeg_stderr } => (
            Status::UnprocessableEntity,
            TranscodeErrorBody {
                code: "transcode_input_error",
                message: format!(
                    "FFmpeg could not read the source media. {}",
                    ffmpeg_stderr.trim()
                ),
                action: None,
            },
        ),
        SpawnTranscodeError::Io(error) => (
            Status::InternalServerError,
            TranscodeErrorBody {
                code: "transcode_failed",
                message: format!("Transcode failed to start: {error}"),
                action: None,
            },
        ),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p koko --lib transcode::tests 2>&1 | tail -15`
Expected: PASS — all three mapper tests pass.

- [ ] **Step 5: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/src/transcode.rs
git commit -m "feat(server): add SpawnTranscodeError + shared map_transcode_error"
```

---

## Task 4: Thread the resolver into the spawn functions (THE fix for issue 1)

**Files:**
- Modify: `crates/server/src/transcode.rs` (`spawn_transcode`, `spawn_transcode_stdout`)

- [ ] **Step 1: Change `spawn_transcode` and `spawn_transcode_stdout` signatures and bodies**

These currently return `Result<Child, std::io::Error>` and call `Command::new(&settings.ffmpeg_path)`. Change them to resolve first, return `SpawnTranscodeError`, and execute the resolved path.

Locate `spawn_transcode` (`transcode.rs:184`) and `spawn_transcode_stdout` (`transcode.rs:214`). Replace `spawn_transcode_stdout` with:

```rust
/// Spawns a transcode process that writes a fragmented stream to stdout.
pub async fn spawn_transcode_stdout(
    _session_id: &str,
    spec: &TranscodeSpec,
    settings: &FfmpegSettings,
) -> Result<Child, SpawnTranscodeError> {
    let args = spec.to_ffmpeg_stdout_args();

    let resolved = crate::ffmpeg_resolve::resolve_ffmpeg(&settings.ffmpeg_path);
    let ffmpeg_path = match resolved {
        crate::ffmpeg_resolve::ResolvedBinary::Found { resolved_path, .. } => {
            log::info!(
                "Starting FFmpeg stdout stream: {} {}",
                resolved_path.display(),
                args.join(" ")
            );
            resolved_path
        }
        crate::ffmpeg_resolve::ResolvedBinary::Missing { checked_paths, .. } => {
            return Err(SpawnTranscodeError::ExecutableMissing { checked_paths });
        }
    };

    let mut command = Command::new(&ffmpeg_path);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().map_err(SpawnTranscodeError::Io)?;

    Ok(child)
}
```

Apply the equivalent change to `spawn_transcode` (`:184`) — same resolver block, but keep its existing `Stdio::null()` for stdout/stderr and `to_ffmpeg_args()` (not stdout args). Its signature becomes `Result<Child, SpawnTranscodeError>`.

- [ ] **Step 2: Build to verify the signatures compile**

Run: `cargo build -p koko 2>&1 | tail -20`
Expected: the build fails at the call site in `media.rs` (`get_session_stream` still expects `std::io::Error`). That's the next task. Note: this task intentionally leaves the build red at the route; Task 5 fixes it. Do NOT commit yet.

---

## Task 5: Structured stream error + per-session error store (seam #1) in `get_session_stream`

**Files:**
- Modify: `crates/server/src/web/routes/media.rs`

- [ ] **Step 1: Add the inert per-session error store and status types**

Near `ACTIVE_TRANSCODE_TASKS` (`media.rs:317`), add:

```rust
/// Per-session transcode errors, written by the lifecycle watcher (deferred
/// phase) and read by `get_session_status`. Inert in the current phase: nothing
/// writes to it yet.
static ACTIVE_SESSION_ERRORS: Lazy<RwLock<std::collections::HashMap<String, crate::transcode::TranscodeErrorBody>>> =
    Lazy::new(|| RwLock::new(std::collections::HashMap::new()));

/// Record a per-session transcode error. Unused in this phase; the lifecycle
/// watcher will call it.
#[allow(dead_code)]
async fn record_session_error(session_id: &str, error: crate::transcode::TranscodeErrorBody) {
    ACTIVE_SESSION_ERRORS
        .write()
        .await
        .insert(session_id.to_string(), error);
}
```

(Confirm `RwLock`/`Lazy` are imported in this file — they are, via `ACTIVE_TRANSCODE_TASKS`'s existing imports. Adjust imports if the compiler asks.)

Add the status response type near `SessionStream` (`media.rs:163`). Because the streaming route returns `Err(status)` (the `<video>` src can't read a JSON body anyway — recovery is via the status read), `TranscodeErrorBody` itself does **not** need `JsonSchema`; only the dedicated status types below do:

```rust
/// The structured transcode error as exposed to clients (owned strings, so it
/// serializes cleanly and matches the client `TranscodeErrorBody` type).
#[derive(Debug, Clone, serde::Serialize, rocket_okapi::JsonSchema)]
pub struct SessionTranscodeError {
    /// Stable machine code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// Optional UI action hint.
    pub action: Option<String>,
}

impl From<crate::transcode::TranscodeErrorBody> for SessionTranscodeError {
    fn from(body: crate::transcode::TranscodeErrorBody) -> Self {
        Self {
            code: body.code.to_string(),
            message: body.message,
            action: body.action.map(str::to_string),
        }
    }
}

/// Status of a playback session's transcode, returned to the client so it can
/// recover a structured error when the `<video>` element stalls.
#[derive(Debug, Clone, serde::Serialize, rocket_okapi::JsonSchema)]
pub struct SessionStatusResponse {
    /// A transcode error for this session, if any. `null` while the session is
    /// healthy or (in this phase) since nothing writes errors yet.
    pub error: Option<SessionTranscodeError>,
}
```

(Leave `TranscodeErrorBody`'s derives exactly as written in Task 3: `#[derive(Debug, Clone, serde::Serialize)]` — no `JsonSchema` needed.)

- [ ] **Step 2: Map spawn errors to structured JSON in `get_session_stream`**

Locate the `match crate::transcode::spawn_transcode_stdout(...)` block (`media.rs:3621-3655`). Change the `Err(e)` arm to use the mapper and return a JSON body instead of a bare 500:

```rust
        Err(error) => {
            let (status, body) = crate::transcode::map_transcode_error(error);
            log::error!(
                "Failed to spawn transcode for session {}: {} ({} — action: {:?})",
                session_id,
                body.message,
                body.code,
                body.action
            );
            // Persist for the status read, so the client can recover the detail
            // even though the stream response body is unreadable from a <video> src.
            record_session_error(&session_id, body.clone()).await;
            // Use rocket's Json to serialize. Note: TranscodeErrorBody derives Serialize.
            Err(status)
        }
```

Note: returning `Err(status)` preserves current behavior (the client can't read a media-src body anyway; it uses the status read in Task 9). The structured body is recoverable via `GET /sessions/<id>/status` (Task 6) and is also `log::error!`'d. Because the stream branch returns `Result<SessionStream, Status>`, we can't attach a JSON body to the 5xx on the streaming response without a custom Responder; the status read is the recovery channel. (Documented in spec §4.4.)

If `body.clone()` complains (TranscodeErrorBody must be `Clone`): ensure `#[derive(Debug, Clone)]` on `TranscodeErrorBody` (it already is in Task 3).

- [ ] **Step 3: Build to confirm the whole server compiles**

Run: `cargo build -p koko 2>&1 | tail -20`
Expected: PASS (green again after Task 4's intentional red).

- [ ] **Step 4: Format and commit (Tasks 4 + 5 together)**

```bash
cargo +nightly fmt
git add crates/server/src/transcode.rs crates/server/src/web/routes/media.rs
git commit -m "feat(server): resolve ffmpeg before spawn; structured transcode errors"
```

---

## Task 6: Discovery + session-status routes

**Files:**
- Modify: `crates/server/src/web/routes/media.rs`
- Modify: `crates/server/src/web/routes/mod.rs`

- [ ] **Step 1: Add the discovery response types**

In `crates/server/src/web/routes/media.rs`, near the other response structs (after `ServerCapabilitiesResponse` at `:341`), add:

```rust
/// Validation result for a single binary (the configured path or a candidate).
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct BinaryProbe {
    /// The resolved absolute path, if the binary was found.
    pub resolved_path: Option<String>,
    /// First line of `ffmpeg -version` / `ffprobe -version`, when available.
    pub version: Option<String>,
    /// Why the binary is unavailable, when applicable.
    pub error: Option<String>,
}

/// One directory discovered to contain ffmpeg and/or ffprobe.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ToolCandidate {
    /// The directory path (serialized as a string).
    pub directory: String,
    /// ffmpeg probe result; `None` if absent in this directory.
    pub ffmpeg: Option<BinaryProbe>,
    /// ffprobe probe result; `None` if absent in this directory.
    pub ffprobe: Option<BinaryProbe>,
}

/// Result of the on-demand transcoding-tools discovery call.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ToolDiscoveryResponse {
    /// Validation of the user's currently-configured ffmpeg path.
    pub configured_ffmpeg: BinaryProbe,
    /// Validation of the user's currently-configured ffprobe path.
    pub configured_ffprobe: BinaryProbe,
    /// One entry per directory containing at least one of ffmpeg/ffprobe.
    pub candidates: Vec<ToolCandidate>,
}
```

(Ensure `Serialize, JsonSchema` are imported — they are, used by `ServerCapabilitiesResponse`. If `PartialEq, Eq` conflicts with any field, drop them; they're for test assertions only.)

- [ ] **Step 2: Implement the discovery helper and route**

In `media.rs`, add a private helper that probes a single configured value:

```rust
fn probe_configured(configured: &str, binary_name: &str) -> BinaryProbe {
    use crate::ffmpeg_resolve::{self, ResolvedBinary};
    match ffmpeg_resolve::resolve_binary(configured, binary_name) {
        ResolvedBinary::Found { resolved_path, .. } => {
            let version = ffmpeg_resolve::probe_version(&resolved_path);
            BinaryProbe {
                resolved_path: Some(resolved_path.display().to_string()),
                version,
                error: None,
            }
        }
        ResolvedBinary::Missing { checked_paths, .. } => {
            let checked = checked_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            BinaryProbe {
                resolved_path: None,
                version: None,
                error: Some(format!("Not found. Checked: [{checked}]")),
            }
        }
    }
}
```

Add the route (place it right after `get_server_capabilities`, `media.rs:3049`):

```rust
/// Discover ffmpeg/ffprobe candidates and validate the configured paths.
///
/// On-demand from the settings page. Searches PATH + well-known locations and
/// also validates the user's currently-configured paths. Read-only.
#[openapi(tag = "Media")]
#[post("/api/v1/system/tools/discover")]
pub async fn discover_transcoding_tools(
    _admin_guard: AdminGuard,
) -> Result<Json<ToolDiscoveryResponse>, Status> {
    let settings = current_settings();
    let configured_ffmpeg = probe_configured(&settings.ffmpeg.ffmpeg_path, "ffmpeg");
    let configured_ffprobe = probe_configured(&settings.ffmpeg.ffprobe_path, "ffprobe");

    // Collect candidate directories from PATH + well-known, de-duplicated, preserving order.
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if let Some(s) = dir.to_str() {
                if seen.insert(s.to_string()) {
                    dirs.push(dir);
                }
            }
        }
    }
    for dir in crate::ffmpeg_resolve::well_known_dirs_public() {
        if let Some(s) = dir.to_str() {
            if seen.insert(s.to_string()) {
                dirs.push(dir);
            }
        }
    }

    let mut candidates = Vec::new();
    for dir in dirs {
        let ffmpeg = probe_in_dir(&dir, "ffmpeg");
        let ffprobe = probe_in_dir(&dir, "ffprobe");
        if ffmpeg.resolved_path.is_some() || ffprobe.resolved_path.is_some() {
            candidates.push(ToolCandidate {
                directory: dir.display().to_string(),
                ffmpeg,
                ffprobe,
            });
        }
    }

    Ok(Json(ToolDiscoveryResponse {
        configured_ffmpeg,
        configured_ffprobe,
        candidates,
    }))
}

fn probe_in_dir(dir: &std::path::Path, binary_name: &str) -> Option<BinaryProbe> {
    let candidate = dir.join(binary_name);
    if crate::ffmpeg_resolve::is_executable_public(&candidate) {
        let version = crate::ffmpeg_resolve::probe_version(&candidate);
        Some(BinaryProbe {
            resolved_path: Some(candidate.display().to_string()),
            version,
            error: None,
        })
    } else {
        None
    }
}
```

For this to compile, expose two helpers from `ffmpeg_resolve.rs`. Add to that module (they wrap the existing private items):

```rust
/// Public accessor for the well-known directory list (for discovery).
pub fn well_known_dirs_public() -> Vec<PathBuf> {
    WELL_KNOWN_DIRS.clone()
}

/// Public accessor for the executability check (for discovery).
pub fn is_executable_public(path: &std::path::Path) -> bool {
    is_executable(path)
}
```

- [ ] **Step 3: Add the session-status route**

In `media.rs`, after `delete_session` (`:3507`), add:

```rust
/// Return the transcode status for a playback session.
///
/// Used by the client to recover a structured error when the `<video>` element
/// stalls. Returns `{ error: null }` until the lifecycle watcher writes one.
#[openapi(tag = "Media")]
#[get("/api/v1/sessions/<session_id>/status")]
pub async fn get_session_status(
    session_id: String,
) -> Result<Json<SessionStatusResponse>, Status> {
    // Validate the session exists (optional but polite). If the session map
    // doesn't contain it, we still return any persisted error, else 404.
    let session_known = ACTIVE_PLAYBACK_SESSIONS.read().await.contains_key(&session_id);
    let error = ACTIVE_SESSION_ERRORS
        .read()
        .await
        .get(&session_id)
        .cloned()
        .map(SessionTranscodeError::from);
    if !session_known && error.is_none() {
        return Err(Status::NotFound);
    }
    Ok(Json(SessionStatusResponse { error }))
}
```

- [ ] **Step 4: Register both routes**

In `crates/server/src/web/routes/mod.rs`, add to `api_routes()` (the `openapi_get_routes!` list), in logical spots:

```rust
        media::get_server_capabilities,
        media::discover_transcoding_tools,
        // ...
        media::create_session,
        media::delete_session,
        media::get_session_status,
```

- [ ] **Step 5: Build to confirm compilation**

Run: `cargo build -p koko 2>&1 | tail -20`
Expected: PASS. Fix any `AdminGuard`/`current_settings` import errors (the file already imports `crate::auth::AdminGuard`? If not, add `use crate::auth::AdminGuard;` near `use crate::auth::UserGuard;` at `media.rs:51`).

- [ ] **Step 6: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/src/web/routes/media.rs crates/server/src/web/routes/mod.rs crates/server/src/ffmpeg_resolve.rs
git commit -m "feat(server): add transcoding-tools discover + session-status routes"
```

---

## Task 7: Route integration tests (discover + status)

**Files:**
- Create: `crates/server/tests/test_web/routes/tools.rs`
- Modify: `crates/server/tests/test_web/routes/mod.rs`

- [ ] **Step 1: Declare the test module**

In `crates/server/tests/test_web/routes/mod.rs`, add (match existing `pub mod` lines):

```rust
pub mod tools;
```

- [ ] **Step 2: Write the integration tests**

Create `crates/server/tests/test_web/routes/tools.rs`. First inspect how `test_auth.rs` performs an admin-authenticated request (token creation) so this matches. Read `crates/server/tests/test_auth.rs` for the `make_request` auth-header pattern, then write:

```rust
use rocket::http::Status;

use crate::test_utils::{
    create_test_client,
    make_request,
};

#[rocket::async_test]
async fn test_discover_requires_admin() {
    let client = create_test_client(Some("tools_discover_requires_admin")).await;
    // Unauthenticated -> forbidden/unauthorized, never 200.
    let response = make_request(
        Some(&client),
        "post",
        "/api/v1/system/tools/discover",
        None,
        None,
        None,
        Some(false),
    )
    .await;
    assert!(
        response.status == Status::Unauthorized || response.status == Status::Forbidden,
        "expected auth failure, got {}",
        response.status
    );
}

#[rocket::async_test]
async fn test_session_status_unknown_session_is_404() {
    let client = create_test_client(Some("tools_status_unknown")).await;
    let response = make_request(
        Some(&client),
        "get",
        "/api/v1/sessions/does-not-exist/status",
        None,
        None,
        Some(Status::NotFound),
        Some(false),
    )
    .await;
    assert_eq!(response.status, Status::NotFound);
}
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test -p koko --test main test_web::routes::tools 2>&1 | tail -20`
Expected: PASS. (The discover test asserts auth rejection, which holds regardless of whether ffmpeg is installed — hermetic. The status test asserts 404 for an unknown session.)

- [ ] **Step 4: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/tests/test_web/routes/tools.rs crates/server/tests/test_web/routes/mod.rs
git commit -m "test(server): integration tests for discover + session-status routes"
```

---

## Task 8: Transcode arg-generation tests (gap F)

**Files:**
- Modify: `crates/server/src/transcode.rs`

- [ ] **Step 1: Add arg-generation tests to the existing test module**

In `crates/server/src/transcode.rs` test module (created in Task 3), add:

```rust
    use std::path::PathBuf;

    fn spec_with_source(source: &str) -> TranscodeSpec {
        TranscodeSpec {
            source_path: PathBuf::from(source),
            output_path: PathBuf::from("/tmp/out.mp4"),
            container: "mp4".into(),
            video_codec: None,
            audio_codec: None,
            max_width: None,
            max_height: None,
            max_bitrate_kbps: None,
            start_time_ms: None,
            audio_stream_index: None,
        }
    }

    #[test]
    fn source_path_with_spaces_and_brackets_is_one_argv_entry() {
        let spec = spec_with_source(
            "/Users/hazer/Downloads/Torrents/[Group] Title With Spaces E10.mkv",
        );
        let args = spec.to_ffmpeg_args();
        // The path must appear verbatim as a single element, right after -i.
        let i_pos = args.iter().position(|a| a == "-i").expect("-i present");
        let path_arg = &args[i_pos + 1];
        assert_eq!(
            path_arg,
            "/Users/hazer/Downloads/Torrents/[Group] Title With Spaces E10.mkv"
        );
        // And it must be exactly one element (no shell splitting happened).
        assert_eq!(args.iter().filter(|a| **a == *path_arg).count(), 1);
    }

    #[test]
    fn copy_codecs_when_none_specified() {
        let spec = spec_with_source("/tmp/in.mkv");
        let args = spec.to_ffmpeg_args();
        let v_pos = args.iter().position(|a| a == "-c:v").unwrap();
        assert_eq!(args[v_pos + 1], "copy");
        let a_pos = args.iter().position(|a| a == "-c:a").unwrap();
        assert_eq!(args[a_pos + 1], "copy");
    }

    #[test]
    fn mp4_container_emits_fragmented_movflags() {
        let spec = spec_with_source("/tmp/in.mkv");
        let args = spec.to_ffmpeg_args();
        let mf_pos = args.iter().position(|a| a == "-movflags").unwrap();
        assert!(args[mf_pos + 1].contains("frag_keyframe"));
        assert!(args[mf_pos + 1].contains("empty_moov"));
    }

    #[test]
    fn stdout_args_target_pipe1() {
        let spec = spec_with_source("/tmp/in.mkv");
        let args = spec.to_ffmpeg_stdout_args();
        assert_eq!(args.last().unwrap(), "pipe:1");
    }
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p koko --lib transcode::tests 2>&1 | tail -20`
Expected: PASS — all four arg-generation tests pass, confirming the path-quoting-is-fine invariant (spec §1.1).

- [ ] **Step 3: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/src/transcode.rs
git commit -m "test(server): cover transcode arg generation, incl. spaces-in-path invariant"
```

---

## Task 9: Optional real-ffmpeg smoke test (behind feature flag)

**Files:**
- Modify: `crates/server/Cargo.toml`
- Modify: `crates/server/src/ffmpeg_resolve.rs`

- [ ] **Step 1: Add the feature flag**

In `crates/server/Cargo.toml` `[features]` (lines 46–57), add:

```toml
real-ffmpeg-tests = []
```

- [ ] **Step 2: Add the gated smoke test**

In `ffmpeg_resolve.rs` test module, add:

```rust
    #[cfg(feature = "real-ffmpeg-tests")]
    #[test]
    fn real_ffmpeg_on_path_is_found() {
        // Only runs when invoked with --features real-ffmpeg-tests AND ffmpeg
        // is actually installed on PATH. Skipped in normal CI.
        let resolved = resolve_binary("ffmpeg", "ffmpeg");
        match resolved {
            ResolvedBinary::Found { via, .. } => {
                // On most dev machines this resolves via PathLookup or WellKnown.
                println!("resolved ffmpeg via {via:?}");
            }
            ResolvedBinary::Missing { checked_paths, .. } => {
                panic!(
                    "real-ffmpeg-tests feature is enabled but ffmpeg was not found; checked: {:?}",
                    checked_paths
                );
            }
        }
    }
```

- [ ] **Step 3: Verify default (flag OFF) and opt-in (flag ON) behavior**

Run (default, must not run the gated test): `cargo test -p koko --lib ffmpeg_resolve 2>&1 | grep real_ffmpeg`
Expected: no output (test excluded).

Run (opt-in, requires local ffmpeg): `cargo test -p koko --lib ffmpeg_resolve --features real-ffmpeg-tests real_ffmpeg 2>&1 | tail -10`
Expected: PASS if ffmpeg is installed locally; otherwise a clear panic listing checked paths. (Locally the developer decides.)

- [ ] **Step 4: Format and commit**

```bash
cargo +nightly fmt
git add crates/server/Cargo.toml crates/server/src/ffmpeg_resolve.rs
git commit -m "test(server): add opt-in real-ffmpeg smoke test behind feature flag"
```

---

## Task 10: Client API types and request functions

**Files:**
- Modify: `crates/client-web/src/api.ts`

- [ ] **Step 1: Add the types and request functions**

In `crates/client-web/src/api.ts`, near the other response interfaces (after `ServerCapabilities`, ~line 54) and the request functions (near `createPlaybackSession`, ~line 1270), add the types:

```ts
export interface BinaryProbe {
  resolved_path?: string;
  version?: string;
  error?: string;
}

export interface ToolCandidate {
  directory: string;
  ffmpeg?: BinaryProbe;
  ffprobe?: BinaryProbe;
}

export interface ToolDiscoveryResponse {
  configured_ffmpeg: BinaryProbe;
  configured_ffprobe: BinaryProbe;
  candidates: ToolCandidate[];
}

export interface TranscodeErrorBody {
  code: string;
  message: string;
  action?: string;
}

export interface SessionStatusResponse {
  error?: TranscodeErrorBody;
}
```

And the request functions (place with the other POST/GET helpers):

```ts
export function discoverTranscodingTools(): Promise<ToolDiscoveryResponse> {
  return requestJson<ToolDiscoveryResponse>('POST', '/api/v1/system/tools/discover');
}

export function getSessionStatus(sessionId: string): Promise<SessionStatusResponse> {
  return requestJson<SessionStatusResponse>('GET', `/api/v1/sessions/${encodeURIComponent(sessionId)}/status`);
}
```

(Confirm `requestJson`'s signature by reading the existing `createPlaybackSession` at `api.ts:1270` — match its pattern exactly, including how auth headers are attached. If `requestJson` requires an admin token for POST, it already handles it; the discover route is admin-gated server-side.)

- [ ] **Step 2: Verify the client type-checks**

Run: `cd crates/client-web && npx tsc --noEmit 2>&1 | tail -20` (then return to repo root).
Expected: PASS — no type errors. (If `tsc` isn't the project's checker, run whatever `crates/client-web/package.json`'s `build`/`typecheck` script is.)

- [ ] **Step 3: Commit**

```bash
git add crates/client-web/src/api.ts
git commit -m "feat(client): add transcoding discovery + session status API helpers"
```

---

## Task 11: Client `playbackError` state + dynamic overlay/banner

**Files:**
- Modify: `crates/client-web/src/app/types.ts`
- Modify: `crates/client-web/src/app/playbackController.ts`

- [ ] **Step 1: Add `playbackError` to `AppState`**

In `crates/client-web/src/app/types.ts`, add to `AppState` (near `error?: string;` at line 142):

```ts
  playbackError?: { code: string; message: string; action?: string };
```

- [ ] **Step 2: Make the in-player overlay copy dynamic**

In `crates/client-web/src/app/playbackController.ts`, the overlay markup at lines 324–327 currently has hardcoded strings:

```ts
        <div class="player-error-indicator" aria-live="polite">
          <strong>Playback could not start</strong>
          <span>Try another audio track or start playback again.</span>
        </div>
```

Replace with dynamic copy driven by `state.playbackError`:

```ts
        <div class="player-error-indicator" aria-live="polite">
          <strong>${escapeHtml(state.playbackError?.message ? 'Playback failed' : 'Playback could not start')}</strong>
          <span>${escapeHtml(state.playbackError?.message ?? 'Try another audio track or start playback again.')}</span>
        </div>
```

- [ ] **Step 3: Update `setPlayerError` to set `state.playbackError`**

In `playbackController.ts`, the `setPlayerError` closure (~line 1053) currently only toggles classes:

```ts
  const setPlayerError = (): void => {
    shell?.classList.remove('is-media-loading');
    shell?.classList.add('has-media-error');
  };
```

Change it to also reset the loading state (the structured detail is set by the caller / status-read):

```ts
  const setPlayerError = (): void => {
    shell?.classList.remove('is-media-loading');
    shell?.classList.add('has-media-error');
  };
```

(No change needed to the function body itself; `state.playbackError` is set at the call sites in Tasks 12–13. Keep this step as a confirmation read — if the body differs in the current tree, leave the class toggles intact and ensure it doesn't clear `state.playbackError`.)

- [ ] **Step 4: Type-check**

Run: `cd crates/client-web && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/client-web/src/app/types.ts crates/client-web/src/app/playbackController.ts
git commit -m "feat(client): dynamic playback error copy via state.playbackError"
```

---

## Task 12: Client capabilities preflight in `startPlayback`

**Files:**
- Modify: `crates/client-web/src/app/playbackController.ts`

- [ ] **Step 1: Add the preflight in `startPlayback`**

`startPlayback` is at `playbackController.ts:610`. It currently sets `state.isPlayerOpen = true` unconditionally. Add a preflight that blocks opening the player when transcode is required but ffmpeg is unavailable. Insert after `state.activePlaybackItem = item;` (line 613) and before `state.isPlayerOpen = true;` (line 615):

```ts
  // Preflight: if this item needs transcoding but ffmpeg is unavailable on the
  // server, don't open the player on a doomed stream — surface the error now.
  const ffmpegAvailable = state.capabilities?.transcoding.ffmpeg.available === true;
  // We need the decision to know whether transcode is required; create the
  // session first (cheap) only if we can't tell from existing state. The
  // session is created below regardless, so just guard the open.
```

Then, immediately after the session is created (`state.activePlaybackSession = await createPlaybackSession(...)` at line 626, before the final `render()`), insert the check:

```ts
  const decision = state.activePlaybackSession.decision;
  const needsTranscode = decision.transcode_required;
  if (needsTranscode && !ffmpegAvailable) {
    state.playbackError = {
      code: 'transcode_executable_missing',
      message: state.capabilities?.transcoding.ffmpeg.error
        ?? 'Transcoding is required but FFmpeg is not available on the server.',
      action: 'open_settings',
    };
    state.error = 'Transcoding requires FFmpeg, which the server could not find. Set its path in Settings.';
    state.isPlayerOpen = true; // open so the overlay/banner are visible
    render();
    return;
  }
  // Healthy path: clear any prior error and proceed.
  state.playbackError = undefined;
```

(Confirm `PlaybackSession.decision.transcode_required` exists on the client type — read `api.ts` `PlaybackSession` interface. It does, per the server `PlaybackDecision`.)

- [ ] **Step 2: Wire the global banner's "Open settings" action**

The global `error-panel` is rendered in `app.ts` / `auth.ts`. Locate where `state.error` is rendered as `.error-panel` (read `crates/client-web/src/app.ts` and `auth.ts:73`). Add a conditional button when `state.playbackError?.action === 'open_settings'`. Since the banner markup is shared, the cleanest minimal change is to render the button inside the same `error-panel` block when the action is present. Read the exact markup first, then add (adapting to the actual surrounding code):

```ts
${state.playbackError?.action === 'open_settings' ? `<button class="button-link" type="button" data-action="open-settings">Open settings</button>` : ''}
```

And bind the click handler in `eventBindings.ts` (read it for the existing click-delegation pattern, e.g. how `close-player` is bound) to navigate to the settings view. Match the existing settings-navigation call used elsewhere in that file.

- [ ] **Step 3: Type-check**

Run: `cd crates/client-web && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/client-web/src/app/playbackController.ts crates/client-web/src/app/app.ts crates/client-web/src/app/eventBindings.ts
git commit -m "feat(client): preflight ffmpeg availability before transcode playback"
```

---

## Task 13: Client status-read recovery in the `<video>` error handler

**Files:**
- Modify: `crates/client-web/src/app/playbackController.ts`

- [ ] **Step 1: Make the `<video>` error handler fetch session status**

The error handler is at `playbackController.ts:1330`:

```ts
  player.addEventListener('error', () => {
    setPlayerError();
    console.error('Media playback failed', player.error);
  });
```

Replace with a version that recovers structured detail from the status endpoint (map lookup, not a spawn):

```ts
  player.addEventListener('error', () => {
    setPlayerError();
    console.error('Media playback failed', player.error);
    const sessionId = state.activePlaybackSession?.session_id;
    if (!sessionId) {
      return;
    }
    // Best-effort: recover a structured error from the per-session store. This
    // is a cheap map lookup on the server, not a transcode spawn. It only helps
    // when the browser actually fires `error` (HTTP failures are unreliable).
    void getSessionStatus(sessionId)
      .then((status) => {
        if (status.error) {
          state.playbackError = status.error;
          render();
        }
      })
      .catch((error) => {
        console.warn('Failed to fetch session status after playback error', error);
      });
  });
```

Add `getSessionStatus` to the imports at the top of `playbackController.ts` (it's already imported alongside `createPlaybackSession` etc.; if not, add it to the `from '../api'` import block).

- [ ] **Step 2: Type-check**

Run: `cd crates/client-web && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/client-web/src/app/playbackController.ts
git commit -m "feat(client): recover structured transcode error via session status read"
```

---

## Task 14: Settings UI — "Detect ffmpeg" + directory radios

**Files:**
- Modify: `crates/client-web/src/app/settingsView.ts`

- [ ] **Step 1: Read the existing ffmpeg inputs and form-save flow**

Read `crates/client-web/src/app/settingsView.ts` lines 545–560 (the ffmpeg/ffprobe inputs) and 770–790 (the form `submit`/save handler that reads `formData`). The new UI must (a) add a detect button + radio container after the inputs, (b) call `discoverTranscodingTools()` on click, (c) render radios grouped by directory, (d) selecting a directory radio writes both paths into the form fields.

- [ ] **Step 2: Add the detect button + results container markup**

After the existing ffprobe input (`settingsView.ts:555`), add:

```ts
            <div class="ffmpeg-discover">
              <button id="detect-ffmpeg" class="secondary-button" type="button">Detect ffmpeg</button>
              <div id="ffmpeg-discover-results" class="ffmpeg-discover-results" hidden></div>
            </div>
```

- [ ] **Step 3: Add the click binding and rendering**

In the event-binding section of `settingsView.ts` (where other settings buttons are wired), add:

```ts
const detectButton = root.querySelector<HTMLButtonElement>('#detect-ffmpeg');
const resultsContainer = root.querySelector<HTMLElement>('#ffmpeg-discover-results');
const ffmpegInput = root.querySelector<HTMLInputElement>('input[name="ffmpeg_path"]');
const ffprobeInput = root.querySelector<HTMLInputElement>('input[name="ffprobe_path"]');

detectButton?.addEventListener('click', async () => {
  if (!resultsContainer || !ffmpegInput || !ffprobeInput) {
    return;
  }
  detectButton.disabled = true;
  detectButton.textContent = 'Detecting…';
  try {
    const discovery = await discoverTranscodingTools();
    renderDiscoverResults(resultsContainer, ffmpegInput, ffprobeInput, discovery);
  } catch (error) {
    resultsContainer.hidden = false;
    resultsContainer.innerHTML = `<p class="muted">Detection failed: ${escapeHtml(error instanceof Error ? error.message : 'unknown error')}</p>`;
  } finally {
    detectButton.disabled = false;
    detectButton.textContent = 'Detect ffmpeg';
  }
});
```

Add the `renderDiscoverResults` helper (in the same file):

```ts
function renderDiscoverResults(
  container: HTMLElement,
  ffmpegInput: HTMLInputElement,
  ffprobeInput: HTMLInputElement,
  discovery: ToolDiscoveryResponse,
): void {
  const parts: string[] = [];

  // Configured-path option (paired validation).
  const configuredOk = discovery.configured_ffmpeg.resolved_path && discovery.configured_ffprobe.resolved_path;
  const configuredLabel = configuredOk
    ? `Current (${discovery.configured_ffmpeg.version ?? 'ffmpeg'} · ${discovery.configured_ffprobe.version ?? 'ffprobe'})`
    : `Current — ${discovery.configured_ffmpeg.error ?? discovery.configured_ffprobe.error ?? 'not found'}`;
  parts.push(`
    <label class="ffmpeg-discover-option">
      <input type="radio" name="ffmpeg-discover-choice" value="current" ${configuredOk ? 'checked' : ''} />
      <span>${escapeHtml(configuredLabel)}</span>
    </label>
  `);

  // Candidate directories, paired.
  for (const candidate of discovery.candidates) {
    const both = candidate.ffmpeg && candidate.ffprobe;
    const label = `${candidate.directory} — ${candidate.ffmpeg?.version ?? 'ffmpeg missing'} · ${candidate.ffprobe?.version ?? 'ffprobe missing'}`;
    parts.push(`
      <label class="ffmpeg-discover-option${both ? '' : ' is-disabled'}">
        <input type="radio" name="ffmpeg-discover-choice" value="${escapeHtml(candidate.directory)}" ${both ? '' : 'disabled'} />
        <span>${escapeHtml(label)}</span>
      </label>
    `);
  }

  // Manual option (keep the text inputs authoritative).
  parts.push(`
    <label class="ffmpeg-discover-option">
      <input type="radio" name="ffmpeg-discover-choice" value="manual" />
      <span>Use the manual paths above</span>
    </label>
  `);

  container.innerHTML = parts.join('');
  container.hidden = false;

  container.querySelectorAll<HTMLInputElement>('input[name="ffmpeg-discover-choice"]').forEach((radio) => {
    radio.addEventListener('change', () => {
      if (radio.value === 'manual') {
        return; // leave the text inputs as-is
      }
      if (radio.value === 'current') {
        return; // leave as-is; configured path already in the inputs
      }
      // A directory: set both inputs to <dir>/ffmpeg and <dir>/ffprobe.
      const dir = radio.value;
      ffmpegInput.value = `${dir}/ffmpeg`;
      ffprobeInput.value = `${dir}/ffprobe`;
    });
  });
}
```

Ensure imports at the top of `settingsView.ts` include `discoverTranscodingTools`, `ToolDiscoveryResponse` (from `../api`) and `escapeHtml` (from `./format`).

- [ ] **Step 4: Add minimal CSS for the new controls**

In `crates/client-web/src/style.css`, near the settings styles, add:

```css
.ffmpeg-discover {
  margin-top: 0.5rem;
}
.ffmpeg-discover-results {
  margin-top: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.ffmpeg-discover-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.ffmpeg-discover-option.is-disabled {
  opacity: 0.5;
}
```

- [ ] **Step 5: Type-check and commit**

Run: `cd crates/client-web && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS.

```bash
cargo +nightly fmt
git add crates/client-web/src/app/settingsView.ts crates/client-web/src/style.css
git commit -m "feat(client): Detect ffmpeg button with directory-paired radio picker"
```

---

## Task 15: Full build + test sweep + final verification

- [ ] **Step 1: Full server test suite**

Run: `cargo test -p koko 2>&1 | tail -30`
Expected: PASS — all existing tests plus the new `ffmpeg_resolve`, `transcode::tests`, and `tools` integration tests. (Existing tests must not regress; the `detect_binary` signature change is the main risk — verify all call sites were updated in Task 2 Step 5.)

- [ ] **Step 2: Full client type-check/build**

Run: `cd crates/client-web && npm run build 2>&1 | tail -20` (or `npm run typecheck` if present; then return to repo root).
Expected: PASS.

- [ ] **Step 3: Clippy (if the project uses it)**

Run: `cargo clippy -p koko --all-targets 2>&1 | tail -30`
Expected: no new warnings from the added code. Fix any the plan's code introduced (common: unused imports, needless `clone`).

- [ ] **Step 4: Format the whole tree**

Run: `cargo +nightly fmt`

- [ ] **Step 5: Manual smoke check (document the steps, don't automate)**

Since the frontend has no test harness, verify by hand:
1. Start the server, open the web client, go to Settings.
2. Click "Detect ffmpeg" — confirm the configured path + candidate directories appear as radios with versions.
3. Pick a directory radio — confirm both ffmpeg/ffprobe inputs update; save.
4. Play an HEVC item — confirm it transcodes (issue 1 fixed).
5. Break ffmpeg_path in settings (set to `ffmpeg-does-not-exist`), save, play an HEVC item — confirm the in-player overlay AND the global banner with "Open settings" appear (issue 2 fixed), instead of a black screen.

- [ ] **Step 6: Final commit (if any formatting/test fixes)**

```bash
git add -A
git commit -m "chore: final formatting and verification pass" || echo "nothing to commit"
```

---

## Spec coverage self-review

| Spec item | Task(s) |
|-----------|---------|
| §4.1 ffmpeg resolver (allow-list, well-known dirs, package managers) | Task 1 (+ "Security decision" + "Resolver path lists" preambles) |
| §4.1 `detect_binary` delegates to resolver | Task 2 |
| §4.1 logging resolved path / checked paths | Task 2 (info/warn logs) |
| §4.2 discover endpoint (AdminGuard, validates configured path) | Task 6 |
| §4.2 settings UI (detect button, directory radios) | Task 14 |
| §4.3 `SpawnTranscodeError` + `TranscodeErrorBody` + `map_transcode_error` (seam #2) | Task 3 |
| §4.3 resolved-path spawn (THE issue-1 fix) | Task 4 |
| §4.3 per-session error store + `record_session_error` (seam #1, inert) | Task 5 |
| §4.3 structured stream-error in route handler | Task 5 |
| §4.3 `get_session_status` route (live reader) | Task 6 |
| §4.4 `playbackError` state + dynamic overlay/banner | Task 11 |
| §4.4 capabilities preflight in `startPlayback` | Task 12 |
| §4.4 status-read recovery in `<video>` error handler | Task 13 |
| §5 testing: resolver inline tests (shim-based) | Task 1 |
| §5 testing: transcode arg-generation tests (gap F) | Task 8 |
| §5 testing: real-ffmpeg smoke behind feature flag | Task 9 |
| §5 testing: route integration tests (tools.rs) | Task 7 |
| §5 testing: frontend test-light (manual) | Task 15 Step 5 (documented) |

**Gaps:** none. Every spec section §4.1–§4.4 and every testing bullet in §5 maps to at least one task. Deferred items (§3, Appendix B) are intentionally not covered — they belong to the lifecycle phase docs.

**Type consistency check:** `TranscodeErrorBody` (Task 3) is referenced by `SessionTranscodeError::from` (Task 5), `map_transcode_error` (Task 3, used Task 5), and the client `TranscodeErrorBody` (Task 10) — shapes align (`code`/`message`/`action`). `resolve_binary`/`resolve_ffmpeg`/`resolve_ffprobe` (Task 1) used by `detect_binary` (Task 2), spawn (Task 4), and `probe_configured`/`probe_in_dir` (Task 6). `discoverTranscodingTools`/`getSessionStatus` (Task 10) consumed by Tasks 12–14. Names match across tasks.
