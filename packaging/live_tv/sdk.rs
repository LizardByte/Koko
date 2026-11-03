//! Resolve pinned SDK metadata and collect verified, reusable native build inputs.

use super::{
    BuildResult,
    tools,
};
use serde_json::Value;
use sha2::{
    Digest,
    Sha256,
};
use std::{
    collections::BTreeMap,
    env,
    fs::{
        self,
        File,
    },
    io::{
        BufWriter,
        Read,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
    process::{
        Command,
        Stdio,
    },
};

pub(super) fn sdk_version(checkout: &Path) -> BuildResult<String> {
    let header = fs::read_to_string(checkout.join("src/libtscore/tsVersion.h"))?;
    let mut values = [None; 3];
    for line in header.lines() {
        let words = line.split_whitespace().collect::<Vec<_>>();
        if let ["#define", name, value] = words.as_slice() {
            let index = match *name {
                "TS_VERSION_MAJOR" => 0,
                "TS_VERSION_MINOR" => 1,
                "TS_COMMIT" => 2,
                _ => continue,
            };
            values[index] = value.parse::<u32>().ok();
        }
    }
    let [
        Some(major),
        Some(minor),
        Some(commit),
    ] = values
    else {
        return Err("The TSDuck submodule has incomplete version metadata".into());
    };
    Ok(format!("{major}.{minor}-{commit}"))
}

pub(super) fn validate_cache_id(value: &str) -> BuildResult<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
    {
        return Err(
            "Native build target and revision must be nonempty identifiers without path separators"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn source_tree(
    checkout: &Path,
    cache: &Path,
    revision: &str,
) -> BuildResult<PathBuf> {
    validate_cache_id(revision)?;
    if !checkout.join("src/libtscore/tsVersion.h").is_file() {
        return Err(
            "Initialize TSDuck with: git submodule update --init third-party/tsduck".into(),
        );
    }
    let source = cache.join(format!("tsduck-{revision}"));
    copy_missing_tree(checkout, &source)?;
    Ok(source)
}

fn copy_missing_tree(
    source: &Path,
    destination: &Path,
) -> BuildResult<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if matches!(entry.file_name().to_str(), Some(".git" | "bin")) {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_missing_tree(&entry.path(), &target)?;
        } else if !target.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn sha256(path: &Path) -> BuildResult<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn verify_archive(
    path: &Path,
    expected: &str,
) -> BuildResult<()> {
    if sha256(path)? != expected {
        return Err(format!("TSDuck checksum mismatch: {}", path.display()).into());
    }
    Ok(())
}

pub(super) fn download_url(
    url: &str,
    expected: &str,
    archive: &Path,
) -> BuildResult<PathBuf> {
    if archive.is_file() {
        verify_archive(archive, expected)?;
        return Ok(archive.to_owned());
    }
    let temporary = archive.with_extension("part");
    tools::run(
        tools::tool_command("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--retry",
                "3",
                "--max-time",
                "300",
                url,
                "--output",
            ])
            .arg(&temporary),
    )?;
    verify_archive(&temporary, expected)?;
    fs::rename(temporary, archive)?;
    Ok(archive.to_owned())
}

pub(super) fn metadata_header(token: Option<&str>) -> BuildResult<String> {
    match token.filter(|value| !value.is_empty()) {
        Some(value) if value.contains(['\r', '\n']) => {
            Err("GH_TOKEN must not contain line breaks".into())
        }
        Some(value) => Ok(format!("Authorization: Bearer {value}\n")),
        None => Ok(String::new()),
    }
}

fn fetch_release(
    url: &str,
    destination: &Path,
) -> BuildResult<()> {
    let token = env::var("GH_TOKEN").ok();
    fetch_release_with_token(url, destination, token.as_deref())
}

pub(super) fn fetch_release_with_token(
    url: &str,
    destination: &Path,
    token: Option<&str>,
) -> BuildResult<()> {
    let header = metadata_header(token)?;
    // Read authorization from stdin so credentials never appear in process arguments or caches.
    let mut child = tools::tool_command("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--retry",
            "3",
            "--max-time",
            "30",
            "--user-agent",
            "Koko-native-build",
            "--header",
            "@-",
            url,
            "--output",
        ])
        .arg(destination)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Could not open metadata request input")?;
    let sent = stdin.write_all(header.as_bytes());
    drop(stdin);
    let status = child.wait()?;
    sent?;
    if !status.success() {
        return Err("Unable to download TSDuck release metadata".into());
    }
    Ok(())
}

pub(super) fn release_metadata_with(
    version: &str,
    cache: &Path,
    mut fetch: impl FnMut(&str, &Path) -> BuildResult<()>,
) -> BuildResult<Value> {
    let metadata = cache.join("tsduck-releases.json");
    let mut releases: BTreeMap<String, Value> = if metadata.is_file() {
        serde_json::from_reader(File::open(&metadata)?)?
    } else {
        BTreeMap::new()
    };
    if !releases.contains_key(version) {
        let response = cache.join("tsduck-release-response.part");
        fetch(
            &format!("https://api.github.com/repos/tsduck/tsduck/releases/tags/v{version}"),
            &response,
        )?;
        releases.insert(
            version.to_owned(),
            serde_json::from_reader(File::open(&response)?)?,
        );
        let temporary = metadata.with_extension("part");
        let mut stream = BufWriter::new(File::create(&temporary)?);
        serde_json::to_writer(&mut stream, &releases)?;
        stream.flush()?;
        drop(stream);
        fs::rename(temporary, metadata)?;
        fs::remove_file(response)?;
    }
    releases
        .remove(version)
        .ok_or_else(|| "Missing cached TSDuck release".into())
}

pub(super) fn windows_asset(
    release: &Value,
    version: &str,
    architecture: &str,
) -> BuildResult<(String, String, String)> {
    let filename = format!("TSDuck-{architecture}-{version}.exe");
    let digest = release
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|asset| asset.get("name").and_then(Value::as_str) == Some(filename.as_str()))
        .and_then(|asset| asset.get("digest"))
        .and_then(Value::as_str)
        .and_then(|value| value.strip_prefix("sha256:"));
    let digest = digest
        .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            format!("The TSDuck release does not publish a SHA-256 digest for {filename}")
        })?;
    Ok((
        format!("https://github.com/tsduck/tsduck/releases/download/v{version}/{filename}"),
        digest.to_ascii_lowercase(),
        filename,
    ))
}

pub(super) fn windows_sdk(
    version: &str,
    architecture: &str,
    cache: &Path,
) -> BuildResult<PathBuf> {
    let release = release_metadata_with(version, cache, fetch_release)?;
    let (url, digest, filename) = windows_asset(&release, version, architecture)?;
    download_url(&url, &digest, &cache.join(filename))
}

pub(super) fn copy_runtime(
    directory: &Path,
    destination: &Path,
) -> BuildResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.starts_with("tsplugin_") || name == "tsdektec.dll" || !entry.path().is_file() {
            continue;
        }
        if [
            ".dll", ".so", ".dylib", ".xml", ".names",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
            || name.contains(".so.")
        {
            fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub(super) fn bundle_cpp_runtime_with(
    workspace: &Path,
    destination: &Path,
    mut resolve: impl FnMut(&str) -> BuildResult<PathBuf>,
) -> BuildResult<()> {
    for name in [
        "libstdc++.so.6",
        "libgcc_s.so.1",
    ] {
        let library = resolve(name)?;
        if !library.is_file() {
            return Err(format!("Unable to locate the C++ runtime {name}").into());
        }
        fs::copy(library, destination.join(name))?;
    }
    for name in [
        "gcc-runtime-GPL-3.0.txt",
        "gcc-runtime-exception-3.1.txt",
    ] {
        fs::copy(
            workspace.join("packaging/licenses").join(name),
            destination.join(name),
        )?;
    }
    Ok(())
}

pub(super) fn bundle_cpp_runtime(
    workspace: &Path,
    destination: &Path,
) -> BuildResult<()> {
    bundle_cpp_runtime_with(workspace, destination, |name| {
        let output = Command::new("c++")
            .arg(format!("-print-file-name={name}"))
            .output()?;
        if !output.status.success() {
            return Err(format!("Unable to locate the C++ runtime {name}").into());
        }
        Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
    })
}
