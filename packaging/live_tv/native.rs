//! Invoke platform SDK builds and assemble the native TV component directory.

use super::{
    BuildOptions,
    BuildResult,
    library,
    sdk,
    tools,
};
use std::{
    env,
    fs,
    path::{
        Path,
        PathBuf,
    },
    process::{
        Command,
        Stdio,
    },
};

const DISABLED: &[&str] = &[
    "NODEKTEC",
    "NODTAPI",
    "NOHIDES",
    "NOVATEK",
    "NOCURL",
    "NOPCSC",
    "NOOPENSSL",
    "NOZLIB",
    "NOSRT",
    "NORIST",
    "NOJAVA",
    "NOPYTHON",
    "NOEDITLINE",
    "NOGITHUB",
    "NODOC",
    "NOTEST",
    "NOSTATIC",
];

pub(super) struct Context<'a> {
    pub(super) workspace: &'a Path,
    pub(super) options: &'a BuildOptions,
    pub(super) source: PathBuf,
    pub(super) cache: PathBuf,
    pub(super) destination: PathBuf,
    pub(super) build_dir: PathBuf,
    pub(super) cmake: PathBuf,
    pub(super) version: String,
}

fn resolve(
    workspace: &Path,
    path: &Path,
) -> PathBuf {
    if path.is_absolute() { path.to_owned() } else { workspace.join(path) }
}

pub(super) fn build(
    workspace: &Path,
    options: &BuildOptions,
) -> BuildResult<()> {
    if options.jobs == 0 {
        return Err("Native build jobs must be positive".into());
    }
    sdk::validate_cache_id(&options.target)?;
    let destination = resolve(workspace, &options.destination);
    let cache = resolve(workspace, &options.cache);
    fs::create_dir_all(&destination)?;
    fs::create_dir_all(&cache)?;
    let checkout = resolve(
        workspace,
        options
            .source
            .as_deref()
            .unwrap_or(Path::new("third-party/tsduck")),
    )
    .canonicalize()?;
    let version = sdk::sdk_version(&checkout)?;
    let revision = options.revision.as_deref().unwrap_or(&version);
    let context = Context {
        workspace,
        options,
        source: sdk::source_tree(&checkout, &cache, revision)?,
        build_dir: cache.join(format!("bridge-{revision}-{}", options.target)),
        cache,
        destination,
        cmake: cmake_program()?,
        version,
    };
    if options.target.contains("windows") {
        build_windows(&context)?;
    } else {
        build_unix(&context)?;
    }
    for name in ["LICENSE.txt", "OTHERS.txt"] {
        let license = context.source.join(name);
        if license.is_file() {
            fs::copy(license, context.destination.join(name))?;
        }
    }
    if !context.destination.join(library(&options.target)).is_file() {
        return Err("Native TV compilation did not produce the expected bridge library".into());
    }
    Ok(())
}

fn visual_studio_path() -> BuildResult<PathBuf> {
    let root = env::var_os("ProgramFiles(x86)")
        .map_or_else(|| PathBuf::from("C:/Program Files (x86)"), PathBuf::from);
    let output = Command::new(root.join("Microsoft Visual Studio/Installer/vswhere.exe"))
        .args([
            "-latest",
            "-products",
            "*",
            "-property",
            "installationPath",
        ])
        .output()?;
    let path = String::from_utf8(output.stdout)?;
    if !output.status.success() || path.trim().is_empty() {
        return Err("Unable to locate Visual Studio C++ tools".into());
    }
    Ok(PathBuf::from(path.trim()))
}

fn cmake_program() -> BuildResult<PathBuf> {
    if let Some(path) = tools::find_program("cmake") {
        return Ok(path);
    }
    if cfg!(windows) {
        let bundled = visual_studio_path()?
            .join("Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe");
        if bundled.is_file() {
            return Ok(bundled);
        }
    }
    Err("CMake is required to build Koko's native TV component; install the C++ CMake tools".into())
}

fn windows_architecture(target: &str) -> (&'static str, &'static str, &'static str) {
    if target.starts_with("aarch64") {
        ("Arm64", "ARM64", "arm64")
    } else {
        ("Win64", "x64", "x64")
    }
}

fn runtime(context: &Context<'_>) -> PathBuf {
    context
        .source
        .join("bin")
        .join(format!("koko-{}", context.options.target))
}

pub(super) fn configure_command(
    context: &Context<'_>,
    sdk_root: Option<&Path>,
) -> Command {
    let mut command = Command::new(&context.cmake);
    command
        .arg("-S")
        .arg(context.workspace.join("native/live-tv"))
        .arg("-B")
        .arg(&context.build_dir);
    if let Some(root) = sdk_root {
        command
            .args([
                "-A",
                windows_architecture(&context.options.target).1,
            ])
            .arg(format!("-DTSDUCK_ROOT={}", root.display()));
    } else {
        command
            .arg(format!("-DTSDUCK_SOURCE={}", context.source.display()))
            .arg(format!("-DTSDUCK_RUNTIME={}", runtime(context).display()))
            .arg("-DCMAKE_BUILD_TYPE=Release");
        if context.options.target.contains("apple") {
            command.arg(format!(
                "-DCMAKE_OSX_ARCHITECTURES={}",
                apple_architecture(&context.options.target)
            ));
        }
    }
    command
}

fn build_windows(context: &Context<'_>) -> BuildResult<()> {
    let (architecture, _, crt_architecture) = windows_architecture(&context.options.target);
    let archive = sdk::windows_sdk(&context.version, architecture, &context.cache)?;
    let sdk_root = context
        .cache
        .join(format!("sdk-{}-{architecture}", context.version));
    if !sdk_root
        .join(format!("lib/Release-{architecture}/tsduck.lib"))
        .is_file()
    {
        let sevenzip = tools::find_program("7z").unwrap_or_else(|| {
            PathBuf::from(env::var_os("ProgramFiles").unwrap_or_else(|| "C:/Program Files".into()))
                .join("7-Zip/7z.exe")
        });
        tools::run(
            Command::new(sevenzip)
                .arg("x")
                .arg(archive)
                .arg(format!("-o{}", sdk_root.display()))
                .args([
                    "-y",
                    "include/*",
                    &format!("lib/Release-{architecture}/*"),
                    "bin/*.dll",
                    "bin/*.xml",
                    "bin/*.names",
                ]),
        )?;
    }
    tools::run(&mut configure_command(context, Some(&sdk_root)))?;
    tools::run(
        Command::new(&context.cmake)
            .arg("--build")
            .arg(&context.build_dir)
            .args([
                "--config",
                "Release",
                "--parallel",
            ])
            .arg(context.options.jobs.to_string()),
    )?;
    sdk::copy_runtime(&sdk_root.join("bin"), &context.destination)?;
    fs::copy(
        context.build_dir.join("Release/koko_tv.dll"),
        context.destination.join("koko_tv.dll"),
    )?;
    bundle_msvc_runtime(
        &visual_studio_path()?,
        crt_architecture,
        &context.destination,
    )
}

pub(super) fn bundle_msvc_runtime(
    installation: &Path,
    architecture: &str,
    destination: &Path,
) -> BuildResult<()> {
    let mut directories = Vec::new();
    for version in fs::read_dir(installation.join("VC/Redist/MSVC"))? {
        let root = version?.path().join(architecture);
        if !root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("Microsoft.VC") && name.ends_with(".CRT") && entry.path().is_dir() {
                directories.push(entry.path());
            }
        }
    }
    directories.sort();
    let directory = directories
        .last()
        .ok_or("The MSVC redistributable CRT is required to package native TV support")?;
    sdk::copy_runtime(directory, destination)
}

pub(super) fn patch_source(
    workspace: &Path,
    source: &Path,
) -> BuildResult<()> {
    let patch = workspace.join("packaging/patches/tsduck-no-web.patch");
    let applied = tools::tool_command("patch")
        .args([
            "-R",
            "--dry-run",
            "--force",
            "--silent",
            "-p1",
            "-i",
        ])
        .arg(&patch)
        .current_dir(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !applied.success() {
        tools::run(
            tools::tool_command("patch")
                .args([
                    "--forward",
                    "--batch",
                    "-p1",
                    "-i",
                ])
                .arg(patch)
                .current_dir(source),
        )?;
    }
    Ok(())
}

fn apple_architecture(target: &str) -> &'static str {
    if target.starts_with("aarch64") { "arm64" } else { "x86_64" }
}

pub(super) fn make_command(
    context: &Context<'_>,
    component: &str,
) -> Command {
    let make = if cfg!(any(target_os = "macos", target_os = "freebsd")) { "gmake" } else { "make" };
    let mut command = tools::tool_command(make);
    command
        .arg("-C")
        .arg(context.source.join("src").join(component))
        .arg("-j")
        .arg(context.options.jobs.to_string());
    command
        .args(DISABLED.iter().map(|name| format!("{name}=1")))
        .arg("DEBUG=")
        .arg(format!("BINDIR={}", runtime(context).display()));
    if context.options.target.contains("apple") {
        let architecture = apple_architecture(&context.options.target);
        command.arg(format!("LOCAL_ARCH={architecture}"));
        // Environment assignments let TSDuck append its own required flags.
        for variable in [
            "CXXFLAGS_EXTRA",
            "LDFLAGS_EXTRA",
        ] {
            let mut value = env::var_os(variable).unwrap_or_default();
            value.push(format!(" -arch {architecture}"));
            command.env(variable, value);
        }
        let sed = tools::find_program("gsed").unwrap_or_else(|| "gsed".into());
        command.arg(format!("SED={}", sed.display()));
    }
    command
}

fn build_unix(context: &Context<'_>) -> BuildResult<()> {
    patch_source(context.workspace, &context.source)?;
    for component in ["libtscore", "libtsduck"] {
        tools::run(&mut make_command(context, component))?;
    }
    tools::run(&mut configure_command(context, None))?;
    tools::run(
        Command::new(&context.cmake)
            .arg("--build")
            .arg(&context.build_dir)
            .arg("--parallel")
            .arg(context.options.jobs.to_string()),
    )?;
    sdk::copy_runtime(&runtime(context), &context.destination)?;
    for config in [
        "src/libtsduck/config",
        "src/libtscore/config",
    ] {
        sdk::copy_runtime(&context.source.join(config), &context.destination)?;
    }
    sdk::copy_runtime(&context.build_dir, &context.destination)?;
    if context.options.target.contains("linux") && !context.options.system_cpp_runtime {
        sdk::bundle_cpp_runtime(context.workspace, &context.destination)?;
    }
    Ok(())
}
