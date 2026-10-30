"""Build and bundle Koko's native TV library against a pinned TSDuck SDK."""

import argparse
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import urllib.request

VERSION = "3.45-4798"
ARCHIVES = {
    "gcc-license": ("https://www.gnu.org/licenses/gpl-3.0.txt", "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"),
    "gcc-exception": ("https://raw.githubusercontent.com/gcc-mirror/gcc/releases/gcc-15.2.0/COPYING.RUNTIME", "9d6b43ce4d8de0c878bf16b54d8e7a10d9bd42b75178153e3af6a815bdc90f74"),
    "source": (f"https://github.com/tsduck/tsduck/archive/refs/tags/v{VERSION}.tar.gz", "a35845430fff1385cf1cda9645bbfd0ec887ed440137fc6c26863c624c24eb63"),
    "Win64": (f"https://github.com/tsduck/tsduck/releases/download/v{VERSION}/TSDuck-Win64-{VERSION}.exe", "d1d1dae29e82d9c9dde8380fd562d2995e6cb702706031fa9574ebc06092bc57"),
    "Arm64": (f"https://github.com/tsduck/tsduck/releases/download/v{VERSION}/TSDuck-Arm64-{VERSION}.exe", "d32122992fe88f91054a32da1c945382de1b741d0912512f5e60d7a571c26ea6"),
}
DISABLED = ["NODEKTEC", "NODTAPI", "NOHIDES", "NOVATEK", "NOCURL", "NOPCSC", "NOOPENSSL", "NOZLIB", "NOSRT", "NORIST", "NOJAVA", "NOPYTHON", "NOEDITLINE", "NOGITHUB", "NODOC", "NOTEST", "NOSTATIC"]


def run(*args, **kwargs):
    """Run a build command with argument boundaries intact and fail on errors."""
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def cmake_program():
    """Find CMake in PATH or the existing Visual Studio C++ tool installation."""
    found = shutil.which("cmake")
    if found:
        return found
    if platform.system() == "Windows":
        vswhere = Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Microsoft Visual Studio/Installer/vswhere.exe"
        if vswhere.is_file():
            installation = subprocess.check_output([str(vswhere), "-latest", "-products", "*", "-property", "installationPath"], text=True).strip()
            bundled = Path(installation) / "Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe"
            if bundled.is_file():
                return str(bundled)
    raise RuntimeError("CMake is required to build Koko's native TV component; install the C++ CMake tools")


def download(kind, cache):
    """Verify every cached or downloaded upstream archive before extraction."""
    url, digest = ARCHIVES[kind]
    archive = cache / url.rsplit("/", 1)[1]
    if not archive.exists():
        temporary = archive.with_suffix(archive.suffix + ".part")
        urllib.request.urlretrieve(url, temporary)
        temporary.replace(archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
        raise RuntimeError(f"TSDuck checksum mismatch: {archive}")
    return archive


def source_tree(cache):
    archive = download("source", cache)
    source = cache / f"tsduck-{VERSION}"
    if not source.exists():
        with tarfile.open(archive) as package:
            package.extractall(cache, filter="data")
    return source


def copy_runtime(directory, destination):
    """Copy just shared libraries and broadcast data, excluding TSDuck CLI programs."""
    for pattern in ("*.dll", "*.so", "*.so.*", "*.dylib", "*.xml", "*.names"):
        for path in directory.glob(pattern):
            if path.name.startswith("tsplugin_") or path.name == "tsdektec.dll":
                continue
            shutil.copy2(path, destination / path.name)


def build(target, destination, cache, offline_source=None):
    """Produce a self-contained native component directory for the selected Rust target."""
    root = Path(__file__).resolve().parents[1]
    destination.mkdir(parents=True, exist_ok=True)
    cache.mkdir(parents=True, exist_ok=True)
    source = offline_source.resolve() if offline_source else source_tree(cache)
    build_dir = cache / f"bridge-{VERSION}-{target}"
    cmake_tool = cmake_program()
    cmake = [cmake_tool, "-S", root / "native/live-tv", "-B", build_dir]
    if "windows" in target:
        architecture = "Arm64" if target.startswith("aarch64") else "Win64"
        archive = download(architecture, cache)
        sdk = cache / f"sdk-{VERSION}-{architecture}"
        sevenzip = shutil.which("7z") or str(Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "7-Zip/7z.exe")
        if not (sdk / f"lib/Release-{architecture}/tsduck.lib").is_file():
            run(sevenzip, "x", archive, f"-o{sdk}", "-y", "include/*", f"lib/Release-{architecture}/*", "bin/*.dll", "bin/*.xml", "bin/*.names")
        run(*cmake, "-A", "ARM64" if architecture == "Arm64" else "x64", f"-DTSDUCK_ROOT={sdk}")
        run(cmake_tool, "--build", build_dir, "--config", "Release", "--parallel", "2")
        copy_runtime(sdk / "bin", destination)
        shutil.copy2(build_dir / "Release/koko_tv.dll", destination / "koko_tv.dll")
        # TSDuck's MSVC SDK needs the redistributable CRT even when Rust links its CRT statically.
        vswhere = Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Microsoft Visual Studio/Installer/vswhere.exe"
        visual_studio = Path(subprocess.check_output([str(vswhere), "-latest", "-products", "*", "-property", "installationPath"], text=True).strip())
        crt_arch = "arm64" if architecture == "Arm64" else "x64"
        crt_directories = sorted((visual_studio / "VC/Redist/MSVC").glob(f"*/{crt_arch}/Microsoft.VC*.CRT"))
        if not crt_directories:
            raise RuntimeError("The MSVC redistributable CRT is required to package native TV support")
        copy_runtime(crt_directories[-1], destination)
    else:
        # Build only the two libraries; the bridge uses the public C++ SDK directly.
        flags = [f"{name}=1" for name in DISABLED]
        flags.append(f"BINDIR_SUFFIX=-koko-{target}")
        if "apple" in target:
            architecture = 'arm64' if target.startswith('aarch64') else 'x86_64'
            flags.extend([f"LOCAL_ARCH={architecture}", f"CXXFLAGS_EXTRA=-arch {architecture}", f"LDFLAGS_EXTRA=-arch {architecture}"])
        make = "gmake" if platform.system() == "FreeBSD" else "make"
        for component in ("libtscore", "libtsduck"):
            run(make, "-C", source / f"src/{component}", "-j", str(min(os.cpu_count() or 2, 4)), *flags)
        # Use generated headers and the public source include directories, with no SDK install.
        runtime = next(p for p in (source / "bin").iterdir() if p.is_dir() and p.name.startswith("release-") and p.name.endswith(f"-koko-{target}") and any(p.glob("libtsduck.*")))
        run(*cmake, f"-DTSDUCK_SOURCE={source}", f"-DTSDUCK_RUNTIME={runtime}", "-DCMAKE_BUILD_TYPE=Release", *([f"-DCMAKE_OSX_ARCHITECTURES={'arm64' if target.startswith('aarch64') else 'x86_64'}"] if "apple" in target else []))
        run(cmake_tool, "--build", build_dir, "--parallel", "2")
        copy_runtime(runtime, destination)
        for config_dir in (source / "src/libtsduck/config", source / "src/libtscore/config"):
            copy_runtime(config_dir, destination)
        copy_runtime(build_dir, destination)
        if "linux" in target:
            # Include matching C++ runtimes so the native component works on minimal installs.
            for name in ("libstdc++.so.6", "libgcc_s.so.1"):
                runtime_library = Path(subprocess.check_output(["c++", f"-print-file-name={name}"], text=True).strip())
                if not runtime_library.is_file():
                    raise RuntimeError(f"Unable to locate the C++ runtime {name}")
                shutil.copy2(runtime_library, destination / name)
            shutil.copy2(download("gcc-license", cache), destination / "gcc-runtime-GPL-3.0.txt")
            shutil.copy2(download("gcc-exception", cache), destination / "gcc-runtime-exception-3.1.txt")
    for name in ("LICENSE.txt", "OTHERS.txt"):
        if (source / name).exists():
            shutil.copy2(source / name, destination / name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--destination", required=True, type=Path)
    parser.add_argument("--cache", default=Path("target/live-tv-deps"), type=Path)
    parser.add_argument("--source", type=Path, help="Pinned source already extracted by an offline package builder")
    options = parser.parse_args()
    build(options.target, options.destination.resolve(), options.cache.resolve(), options.source)
