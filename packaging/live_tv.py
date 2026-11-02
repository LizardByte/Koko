"""Build and bundle Koko's native TV library against a pinned TSDuck SDK."""

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import urllib.request

DISABLED = [
    "NODEKTEC", "NODTAPI", "NOHIDES", "NOVATEK", "NOCURL", "NOPCSC", "NOOPENSSL", "NOZLIB", "NOSRT",
    "NORIST", "NOJAVA", "NOPYTHON", "NOEDITLINE", "NOGITHUB", "NODOC", "NOTEST", "NOSTATIC",
]


@dataclass(frozen=True)
class BuildOptions:
    """Build concurrency and whether the package runtime already supplies the C++ libraries."""

    jobs: int
    system_cpp_runtime: bool = False


def sdk_version(checkout):
    """Read release identifiers from the pinned source, including shallow and offline checkouts."""
    header = (checkout / "src/libtscore/tsVersion.h").read_text(encoding="utf-8")
    values = dict(re.findall(r"^#define\s+(TS_VERSION_MAJOR|TS_VERSION_MINOR|TS_COMMIT)\s+(\d+)\s*$", header, re.M))
    try:
        return f"{int(values['TS_VERSION_MAJOR'])}.{int(values['TS_VERSION_MINOR'])}-{int(values['TS_COMMIT'])}"
    except KeyError as error:
        raise RuntimeError("The TSDuck submodule has incomplete version metadata") from error


def visual_studio_path():
    """Locate the selected Visual Studio installation using its supported discovery tool."""
    vswhere = (Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)"))
               / "Microsoft Visual Studio/Installer/vswhere.exe")
    installation = subprocess.check_output(
        [str(vswhere), "-latest", "-products", "*", "-property", "installationPath"], text=True,
    ).strip()
    return Path(installation)


def run(*args, **kwargs):
    """Run a build command with argument boundaries intact and fail on errors."""
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def cmake_program():
    """Find CMake in PATH or the existing Visual Studio C++ tool installation."""
    found = shutil.which("cmake")
    if found:
        return found
    if platform.system() == "Windows":
        bundled = visual_studio_path() / "Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe"
        if bundled.is_file():
            return str(bundled)
    raise RuntimeError("CMake is required to build Koko's native TV component; install the C++ CMake tools")


def download_url(url, digest, cache):
    """Reject corrupted downloads and cached archives before they reach an extractor."""
    archive = cache / url.rsplit("/", 1)[1]
    if not archive.exists():
        temporary = archive.with_suffix(archive.suffix + ".part")
        urllib.request.urlretrieve(url, temporary)
        temporary.replace(archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
        raise RuntimeError(f"TSDuck checksum mismatch: {archive}")
    return archive


def release_metadata(version: str, cache: Path):
    """Cache release data by version inside a file whose path never depends on upstream content."""
    metadata = cache / "tsduck-releases.json"
    releases = json.loads(metadata.read_text(encoding="utf-8")) if metadata.is_file() else {}
    if version not in releases:
        url = f"https://api.github.com/repos/tsduck/tsduck/releases/tags/v{version}"
        request = urllib.request.Request(url, headers={"User-Agent": "Koko-native-build"})
        with urllib.request.urlopen(request, timeout=30) as response:
            releases[version] = json.load(response)
        temporary = metadata.with_suffix(".part")
        with temporary.open("w", encoding="utf-8") as stream:
            json.dump(releases, stream)
        temporary.replace(metadata)
    return releases[version]


def windows_sdk(version, architecture, cache):
    """Resolve the matching SDK's SHA-256 from its published GitHub release metadata."""
    release = release_metadata(version, cache)
    filename = f"TSDuck-{architecture}-{version}.exe"
    asset = next((item for item in release.get("assets", []) if item.get("name") == filename), None)
    digest = asset.get("digest", "") if asset else ""
    if not isinstance(digest, str) or not re.fullmatch(r"sha256:[0-9a-fA-F]{64}", digest):
        raise RuntimeError(f"The TSDuck release does not publish a SHA-256 digest for {filename}")
    url = f"https://github.com/tsduck/tsduck/releases/download/v{version}/{filename}"
    return download_url(url, digest.removeprefix("sha256:").lower(), cache)


def copy_missing(source, destination):
    """Restore pruned SDK files without overwriting patched files or invalidating compiled objects."""
    if not Path(destination).is_file():
        shutil.copy2(source, destination)
    return destination


def source_tree(checkout, cache, revision):
    """Build in a revision-specific copy so generated files and patches never dirty the submodule."""
    if not (checkout / "src/libtscore/tsVersion.h").is_file():
        raise RuntimeError("Initialize TSDuck with: git submodule update --init third-party/tsduck")
    source = cache / f"tsduck-{revision}"
    shutil.copytree(checkout, source, dirs_exist_ok=True, copy_function=copy_missing,
                    ignore=shutil.ignore_patterns(".git", "bin"))
    return source


def copy_runtime(directory, destination):
    """Copy just shared libraries and broadcast data, excluding TSDuck CLI programs."""
    for pattern in ("*.dll", "*.so", "*.so.*", "*.dylib", "*.xml", "*.names"):
        for path in directory.glob(pattern):
            if path.name.startswith("tsplugin_") or path.name == "tsdektec.dll":
                continue
            shutil.copy2(path, destination / path.name)


def build_windows(target, destination, cache, build_dir, cmake, version, options):
    """Build the bridge using the matching upstream Windows SDK and redistributable CRT."""
    architecture = "Arm64" if target.startswith("aarch64") else "Win64"
    archive = windows_sdk(version, architecture, cache)
    sdk = cache / f"sdk-{version}-{architecture}"
    sevenzip = shutil.which("7z") or str(Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "7-Zip/7z.exe")
    if not (sdk / f"lib/Release-{architecture}/tsduck.lib").is_file():
        run(sevenzip, "x", archive, f"-o{sdk}", "-y", "include/*", f"lib/Release-{architecture}/*",
            "bin/*.dll", "bin/*.xml", "bin/*.names")
    run(*cmake, "-A", "ARM64" if architecture == "Arm64" else "x64", f"-DTSDUCK_ROOT={sdk}")
    run(cmake[0], "--build", build_dir, "--config", "Release", "--parallel", options.jobs)
    copy_runtime(sdk / "bin", destination)
    shutil.copy2(build_dir / "Release/koko_tv.dll", destination / "koko_tv.dll")
    # TSDuck's MSVC SDK needs the redistributable CRT even when Rust links its CRT statically.
    crt_arch = "arm64" if architecture == "Arm64" else "x64"
    crt_directories = sorted((visual_studio_path() / "VC/Redist/MSVC").glob(f"*/{crt_arch}/Microsoft.VC*.CRT"))
    if not crt_directories:
        raise RuntimeError("The MSVC redistributable CRT is required to package native TV support")
    copy_runtime(crt_directories[-1], destination)


def patch_source(root, source):
    """Apply the pinned SDK backport once, including when reusing an extracted build cache."""
    patch = root / "packaging/patches/tsduck-no-web.patch"
    reverse = subprocess.run(
        ["patch", "-R", "--dry-run", "--force", "--silent", "-p1", "-i", str(patch)],
        cwd=source, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    if reverse.returncode:
        run("patch", "--forward", "--batch", "-p1", "-i", patch, cwd=source)


def bundle_cpp_runtime(destination):
    """Include matching C++ libraries and license notices for minimal Linux installs."""
    for name in ("libstdc++.so.6", "libgcc_s.so.1"):
        runtime_library = Path(subprocess.check_output(["c++", f"-print-file-name={name}"], text=True).strip())
        if not runtime_library.is_file():
            raise RuntimeError(f"Unable to locate the C++ runtime {name}")
        shutil.copy2(runtime_library, destination / name)
    notices = Path(__file__).resolve().parent / "licenses"
    for name in ("gcc-runtime-GPL-3.0.txt", "gcc-runtime-exception-3.1.txt"):
        shutil.copy2(notices / name, destination / name)


def build_unix(target, destination, source, build_dir, cmake, options):
    """Build only the SDK libraries and bridge without installing upstream CLI programs."""
    patch_source(Path(__file__).resolve().parents[1], source)
    flags = [f"{name}=1" for name in DISABLED]
    runtime = source / "bin" / f"koko-{target}"
    # Cargo exports DEBUG even for release builds. Keep the SDK profile and directory deterministic.
    flags.extend(["DEBUG=", f"BINDIR={runtime}"])
    environment = os.environ.copy()
    if "apple" in target:
        architecture = "arm64" if target.startswith("aarch64") else "x86_64"
        flags.append(f"LOCAL_ARCH={architecture}")
        # Command-line assignments prevent TSDuck from appending its required include paths.
        for variable in ("CXXFLAGS_EXTRA", "LDFLAGS_EXTRA"):
            environment[variable] = f"{environment.get(variable, '')} -arch {architecture}".strip()
        cmake.append(f"-DCMAKE_OSX_ARCHITECTURES={architecture}")
        # TSDuck's scripts require GNU sed's argument ordering.
        flags.append(f"SED={shutil.which('gsed') or 'gsed'}")
    # Upstream's make configuration needs GNU Make and a current Bash on macOS/BSD.
    make = "gmake" if platform.system() in ("Darwin", "FreeBSD") else "make"
    for component in ("libtscore", "libtsduck"):
        run(make, "-C", source / f"src/{component}", "-j", options.jobs, *flags, env=environment)
    run(*cmake, f"-DTSDUCK_SOURCE={source}", f"-DTSDUCK_RUNTIME={runtime}", "-DCMAKE_BUILD_TYPE=Release")
    run(cmake[0], "--build", build_dir, "--parallel", options.jobs)
    copy_runtime(runtime, destination)
    for config_dir in (source / "src/libtsduck/config", source / "src/libtscore/config"):
        copy_runtime(config_dir, destination)
    copy_runtime(build_dir, destination)
    if "linux" in target and not options.system_cpp_runtime:
        bundle_cpp_runtime(destination)


def build(target, destination, cache, offline_source=None, revision=None, options=None):
    """Produce a self-contained native component directory for the selected Rust target."""
    root = Path(__file__).resolve().parents[1]
    destination.mkdir(parents=True, exist_ok=True)
    cache.mkdir(parents=True, exist_ok=True)
    checkout = offline_source.resolve() if offline_source else root / "third-party/tsduck"
    version = sdk_version(checkout)
    revision = revision or version
    options = options or BuildOptions(os.cpu_count() or 2)
    source = source_tree(checkout, cache, revision)
    build_dir = cache / f"bridge-{revision}-{target}"
    cmake = [cmake_program(), "-S", root / "native/live-tv", "-B", build_dir]
    if "windows" in target:
        build_windows(target, destination, cache, build_dir, cmake, version, options)
    else:
        build_unix(target, destination, source, build_dir, cmake, options)
    for name in ("LICENSE.txt", "OTHERS.txt"):
        if (source / name).exists():
            shutil.copy2(source / name, destination / name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--destination", required=True, type=Path)
    parser.add_argument("--cache", default=Path("target/live-tv-deps"), type=Path)
    parser.add_argument("--source", type=Path, help="Pinned source already extracted by an offline package builder")
    parser.add_argument("--revision", help="Pinned submodule revision used to isolate build caches")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 2, help="Concurrent native compilation jobs")
    parser.add_argument("--system-cpp-runtime", action="store_true",
                        help="Use C++ runtime libraries supplied by the package")
    options = parser.parse_args()
    if options.jobs < 1:
        parser.error("--jobs must be positive")
    build(options.target, options.destination.resolve(), options.cache.resolve(), options.source, options.revision,
          BuildOptions(options.jobs, options.system_cpp_runtime))
