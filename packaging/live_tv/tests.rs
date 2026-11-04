//! Exercise SDK integrity, offline cache reuse, platform commands, and redistributable notices.

use super::{
    BuildOptions,
    native,
    sdk,
};
use serde_json::json;
use std::{
    fs,
    io::{
        Read,
        Write,
    },
    net::TcpListener,
    path::{
        Path,
        PathBuf,
    },
    thread,
};
use tempfile::TempDir;

const ABC_DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn checkout(root: &Path) -> PathBuf {
    let source = root.join("checkout");
    fs::create_dir_all(source.join("src/libtscore")).unwrap();
    fs::write(
        source.join("src/libtscore/tsVersion.h"),
        "#define TS_VERSION_MAJOR 9\n#define TS_VERSION_MINOR 12\n#define TS_COMMIT 345\n",
    )
    .unwrap();
    source
}

#[test]
fn version_follows_the_pinned_source_and_rejects_incomplete_headers() {
    let root = TempDir::new().unwrap();
    let source = checkout(root.path());
    assert_eq!(sdk::sdk_version(&source).unwrap(), "9.12-345");
    for header in [
        "#define TS_VERSION_MAJOR 9\n",
        "#define TS_VERSION_MAJOR -1\n#define TS_VERSION_MINOR 2\n#define TS_COMMIT 3\n",
    ] {
        fs::write(source.join("src/libtscore/tsVersion.h"), header).unwrap();
        assert!(
            sdk::sdk_version(&source)
                .unwrap_err()
                .to_string()
                .contains("incomplete version metadata")
        );
    }
}

#[test]
fn cache_restores_pruned_files_without_overwriting_backports_or_copying_git_and_build_outputs() {
    let root = TempDir::new().unwrap();
    let source = checkout(root.path());
    fs::write(source.join(".git"), "gitdir: checkout-metadata").unwrap();
    fs::write(source.join("source.cpp"), "original").unwrap();
    fs::create_dir(source.join("bin")).unwrap();
    fs::write(source.join("bin/stale.dll"), "old output").unwrap();
    let cache = sdk::source_tree(&source, &root.path().join("cache"), "revision-a").unwrap();
    fs::write(cache.join("source.cpp"), "local backport").unwrap();
    fs::remove_file(cache.join("src/libtscore/tsVersion.h")).unwrap();
    sdk::source_tree(&source, &root.path().join("cache"), "revision-a").unwrap();
    assert_eq!(
        fs::read_to_string(cache.join("source.cpp")).unwrap(),
        "local backport"
    );
    assert!(cache.join("src/libtscore/tsVersion.h").is_file());
    assert!(!cache.join(".git").exists());
    assert!(!cache.join("bin").exists());
    assert!(sdk::source_tree(&source, root.path(), "../outside").is_err());
}

fn serve(body: &'static [u8]) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/sdk.exe", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let mut bytes = [0; 1024];
            let count = stream.read(&mut bytes).unwrap();
            assert!(count > 0 && request.len() + count <= 16 * 1024);
            request.extend_from_slice(&bytes[..count]);
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, worker)
}

#[test]
fn downloaded_and_cached_archives_are_verified_before_use() {
    let root = TempDir::new().unwrap();
    let archive = root.path().join("sdk.exe");
    let (url, worker) = serve(b"abc");
    assert_eq!(
        sdk::download_url(&url, ABC_DIGEST, &archive).unwrap(),
        archive
    );
    worker.join().unwrap();
    assert_eq!(fs::read(&archive).unwrap(), b"abc");
    // The server has closed; valid cached content must not require a network request.
    sdk::download_url(&url, ABC_DIGEST, &archive).unwrap();
    fs::write(&archive, "corrupt cache").unwrap();
    assert!(
        sdk::download_url(&url, ABC_DIGEST, &archive)
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch")
    );
}

#[test]
fn a_corrupt_download_never_becomes_an_extractable_cached_sdk() {
    let root = TempDir::new().unwrap();
    let archive = root.path().join("sdk.exe");
    let (url, worker) = serve(b"corrupt upstream content");
    assert!(
        sdk::download_url(&url, ABC_DIGEST, &archive)
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch")
    );
    worker.join().unwrap();
    assert!(!archive.exists());
}

#[test]
fn release_metadata_is_cached_by_version_without_requiring_network_access() {
    let root = TempDir::new().unwrap();
    let release = json!({"assets": [{
        "name": "TSDuck-Arm64-9.12-345.exe",
        "digest": format!("sha256:{ABC_DIGEST}")
    }]});
    let loaded = sdk::release_metadata_with("9.12-345", root.path(), |url, file| {
        assert_eq!(
            url,
            "https://api.github.com/repos/tsduck/tsduck/releases/tags/v9.12-345"
        );
        serde_json::to_writer(fs::File::create(file)?, &release)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(loaded, release);
    assert_eq!(
        sdk::release_metadata_with("9.12-345", root.path(), |_, _| panic!(
            "unexpected network request"
        ))
        .unwrap(),
        release
    );
    assert!(!root.path().join("tsduck-release-response.part").exists());
    fs::write(root.path().join("tsduck-releases.json"), "invalid JSON").unwrap();
    assert!(
        sdk::release_metadata_with("9.12-345", root.path(), |_, _| panic!(
            "unexpected network request"
        ))
        .is_err()
    );
}

#[test]
fn windows_sdk_selection_requires_a_matching_architecture_and_published_sha256() {
    let release = json!({"assets": [{
        "name": "TSDuck-Arm64-9.12-345.exe",
        "digest": format!("sha256:{}", ABC_DIGEST.to_uppercase()),
        "browser_download_url": "https://untrusted.example/sdk.exe"
    }]});
    let (url, digest, filename) = sdk::windows_asset(&release, "9.12-345", "Arm64").unwrap();
    assert_eq!(
        url,
        "https://github.com/tsduck/tsduck/releases/download/v9.12-345/TSDuck-Arm64-9.12-345.exe"
    );
    assert_eq!(digest, ABC_DIGEST);
    assert_eq!(filename, "TSDuck-Arm64-9.12-345.exe");
    assert!(sdk::windows_asset(&release, "9.12-345", "Win64").is_err());
    for digest in [
        json!(null),
        json!("sha256:bad"),
        json!(format!("sha256:{}", "g".repeat(64))),
        json!(format!("sha512:{ABC_DIGEST}")),
    ] {
        let release = json!({"assets":[{"name":"TSDuck-Arm64-9.12-345.exe","digest":digest}]});
        assert!(
            sdk::windows_asset(&release, "9.12-345", "Arm64")
                .unwrap_err()
                .to_string()
                .contains("does not publish a SHA-256 digest")
        );
    }
}

#[test]
fn metadata_authorization_handles_anonymous_pulls_and_rejects_header_injection() {
    assert_eq!(sdk::metadata_header(None).unwrap(), "");
    assert_eq!(sdk::metadata_header(Some("")).unwrap(), "");
    let token = std::process::id().to_string();
    assert_eq!(
        sdk::metadata_header(Some(&token)).unwrap(),
        format!("Authorization: Bearer {token}\n")
    );
    assert!(sdk::metadata_header(Some("first\r\nsecond")).is_err());
}

#[test]
fn release_requests_deliver_authentication_through_stdin_and_support_anonymous_access() {
    let root = TempDir::new().unwrap();
    let token = std::process::id().to_string();
    for authorization in [None, Some(token.as_str())] {
        let (url, worker) = serve(b"{}");
        let response = root.path().join("response.json");
        sdk::fetch_release_with_token(&url, &response, authorization).unwrap();
        let request = worker.join().unwrap();
        assert!(request.contains("User-Agent: Koko-native-build\r\n"));
        if authorization.is_some() {
            assert!(request.contains(&format!("Authorization: Bearer {token}\r\n")));
        } else {
            assert!(!request.contains("Authorization:"));
        }
        assert_eq!(fs::read(&response).unwrap(), b"{}");
    }
}

#[test]
fn runtime_copy_omits_command_plugins_and_keeps_shared_libraries_and_broadcast_data() {
    let root = TempDir::new().unwrap();
    let output = root.path().join("bundle");
    fs::create_dir(&output).unwrap();
    for file in [
        "koko_tv.dll",
        "tsduck.DLL",
        "libtsduck.so.3",
        "libtscore.so",
        "libkoko_tv.dylib",
        "tables.xml",
        "dtv.names",
        "tsplugin_http.dll",
        "tsdektec.dll",
        "tool.exe",
        "ignored.txt",
    ] {
        fs::write(root.path().join(file), file).unwrap();
    }
    sdk::copy_runtime(root.path(), &output).unwrap();
    let mut names = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [
            "dtv.names",
            "koko_tv.dll",
            "libkoko_tv.dylib",
            "libtscore.so",
            "libtsduck.so.3",
            "tables.xml",
            "tsduck.DLL"
        ]
    );
}

#[test]
fn cpp_runtime_bundling_includes_the_matching_libraries_and_local_license_notices() {
    let root = TempDir::new().unwrap();
    let output = root.path().join("bundle");
    fs::create_dir(&output).unwrap();
    for file in [
        "libstdc++.so.6",
        "libgcc_s.so.1",
    ] {
        fs::write(root.path().join(file), file).unwrap();
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    sdk::bundle_cpp_runtime_with(workspace, &output, |name| Ok(root.path().join(name))).unwrap();
    assert_eq!(
        fs::read(output.join("libstdc++.so.6")).unwrap(),
        b"libstdc++.so.6"
    );
    assert_eq!(
        fs::read(output.join("libgcc_s.so.1")).unwrap(),
        b"libgcc_s.so.1"
    );
    assert!(
        fs::read_to_string(output.join("gcc-runtime-GPL-3.0.txt"))
            .unwrap()
            .contains("GNU GENERAL PUBLIC LICENSE")
    );
    assert!(
        fs::read_to_string(output.join("gcc-runtime-exception-3.1.txt"))
            .unwrap()
            .contains("GCC RUNTIME LIBRARY EXCEPTION")
    );
    assert!(
        sdk::bundle_cpp_runtime_with(workspace, &output, |_| Ok(root.path().join("missing")))
            .is_err()
    );
}

#[test]
fn source_backport_is_applied_once_without_dirtying_the_checkout() {
    let root = TempDir::new().unwrap();
    let checkout = checkout(root.path());
    fs::write(checkout.join("value.txt"), "original\n").unwrap();
    let source = sdk::source_tree(&checkout, &root.path().join("cache"), "fixture").unwrap();
    fs::create_dir_all(root.path().join("packaging/patches")).unwrap();
    fs::write(
        root.path().join("packaging/patches/tsduck-no-web.patch"),
        "--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-original\n+backport\n",
    )
    .unwrap();
    native::patch_source(root.path(), &source).unwrap();
    native::patch_source(root.path(), &source).unwrap();
    assert_eq!(
        fs::read_to_string(source.join("value.txt")).unwrap(),
        "backport\n"
    );
    assert_eq!(
        fs::read_to_string(checkout.join("value.txt")).unwrap(),
        "original\n"
    );
}

#[test]
fn build_commands_preserve_target_architecture_parallelism_and_sdk_make_flags() {
    let root = TempDir::new().unwrap();
    for (target, expected) in [
        ("aarch64-apple-darwin", "arm64"),
        ("x86_64-apple-darwin", "x86_64"),
    ] {
        let mut options = BuildOptions::new(target.to_owned(), root.path().join("bundle"));
        options.jobs = 3;
        let context = context(root.path(), &options);
        let make = native::make_command(&context, "libtsduck");
        let args = make
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        for required in [
            "NOPYTHON=1",
            "NOCURL=1",
            "NOSTATIC=1",
            "DEBUG=",
            "-j",
            "3",
        ] {
            assert!(args.iter().any(|arg| arg == required));
        }
        assert!(args.contains(&format!("LOCAL_ARCH={expected}")));
        assert!(!args.iter().any(|arg| arg.starts_with("CXXFLAGS_EXTRA=")));
        for (_, value) in make
            .get_envs()
            .filter(|(name, _)| matches!(name.to_str(), Some("CXXFLAGS_EXTRA" | "LDFLAGS_EXTRA")))
        {
            assert!(
                value
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(&format!("-arch {expected}"))
            );
        }
        let cmake = native::configure_command(&context, None);
        assert!(
            cmake
                .get_args()
                .any(|arg| arg == format!("-DCMAKE_OSX_ARCHITECTURES={expected}").as_str())
        );
    }
}

fn context<'a>(
    root: &'a Path,
    options: &'a BuildOptions,
) -> native::Context<'a> {
    native::Context {
        workspace: root,
        options,
        source: root.join("source folder"),
        cache: root.join("cache"),
        destination: options.destination.clone(),
        build_dir: root.join("bridge"),
        cmake: "cmake".into(),
        version: "9.12-345".to_owned(),
    }
}

#[test]
fn windows_configuration_and_crt_bundling_follow_the_target_instead_of_the_host() {
    let root = TempDir::new().unwrap();
    for (target, expected, architecture) in [
        ("aarch64-pc-windows-msvc", "ARM64", "arm64"),
        ("x86_64-pc-windows-msvc", "x64", "x64"),
    ] {
        let options = BuildOptions::new(target.to_owned(), root.path().join(architecture));
        let context = context(root.path(), &options);
        let command = native::configure_command(&context, Some(&context.source));
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.windows(2).any(|pair| pair == ["-A", expected]));
        assert!(args.contains(&format!("-DTSDUCK_ROOT={}", context.source.display())));
        for version in ["14.40", "14.50"] {
            let crt = root
                .path()
                .join("VC/Redist/MSVC")
                .join(version)
                .join(architecture)
                .join("Microsoft.VC143.CRT");
            fs::create_dir_all(&crt).unwrap();
            fs::write(
                crt.join("vcruntime140.dll"),
                format!("{architecture}-{version}"),
            )
            .unwrap();
        }
        fs::create_dir(&options.destination).unwrap();
        native::bundle_msvc_runtime(root.path(), architecture, &options.destination).unwrap();
        assert_eq!(
            fs::read_to_string(options.destination.join("vcruntime140.dll")).unwrap(),
            format!("{architecture}-14.50")
        );
    }
}
