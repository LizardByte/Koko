"""Validate pinned SDK metadata, archive integrity, and clean reusable native build copies."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("koko_native_build", Path(__file__).parents[1] / "live_tv.py")
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Unable to load native packaging helpers")
native = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = native
SPEC.loader.exec_module(native)


class NativePackagingTests(unittest.TestCase):
    """Keep source versioning and binary verification independent of a particular release."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="koko-sdk-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def checkout(self, header):
        checkout = self.root / "checkout"
        version = checkout / "src/libtscore/tsVersion.h"
        version.parent.mkdir(parents=True)
        version.write_text(header, encoding="utf-8")
        return checkout

    def test_version_follows_the_pinned_source_and_rejects_incomplete_headers(self):
        checkout = self.checkout("#define TS_VERSION_MAJOR 9\n#define TS_VERSION_MINOR 12\n#define TS_COMMIT 345\n")
        self.assertEqual(native.sdk_version(checkout), "9.12-345")
        (checkout / "src/libtscore/tsVersion.h").write_text("#define TS_VERSION_MAJOR 9\n", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "incomplete version metadata"):
            native.sdk_version(checkout)

    def test_missing_cache_files_are_restored_without_overwriting_patches_or_copying_git_metadata(self):
        checkout = self.checkout("version fixture")
        (checkout / ".git").write_text("gitdir: private-checkout", encoding="utf-8")
        source = checkout / "source.cpp"
        source.write_text("upstream", encoding="utf-8")
        cache = native.source_tree(checkout, self.root / "cache", "revision-a")
        (cache / "source.cpp").write_text("local backport", encoding="utf-8")
        (cache / "src/libtscore/tsVersion.h").unlink()
        native.source_tree(checkout, self.root / "cache", "revision-a")
        self.assertTrue((cache / "src/libtscore/tsVersion.h").is_file())
        self.assertEqual((cache / "source.cpp").read_text(encoding="utf-8"), "local backport")
        self.assertFalse((cache / ".git").exists())

    def test_corrupted_cached_archives_are_rejected_before_extraction(self):
        original = self.root / "sdk.bin"
        original.write_bytes(b"verified upstream archive")
        digest = hashlib.sha256(original.read_bytes()).hexdigest()
        cache = self.root / "downloads"
        cache.mkdir()
        downloaded = native.download_url(original.as_uri(), digest, cache)
        self.assertEqual(downloaded.read_bytes(), original.read_bytes())
        downloaded.write_bytes(b"corrupt cache")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            native.download_url(original.as_uri(), digest, cache)

    def test_windows_sdk_matches_the_release_version_architecture_and_published_digest(self):
        version = "9.12-345"
        digest = "a" * 64
        release = {"assets": [{"name": f"TSDuck-Arm64-{version}.exe", "digest": f"sha256:{digest}"}]}
        with patch.object(native.urllib.request, "urlopen", return_value=io.BytesIO(json.dumps(release).encode())):
            with patch.object(native, "download_url", return_value=self.root / "sdk.exe") as download:
                native.windows_sdk(version, "Arm64", self.root)
        download.assert_called_once_with(
            f"https://github.com/tsduck/tsduck/releases/download/v{version}/TSDuck-Arm64-{version}.exe",
            digest, self.root,
        )
        # A missing or unverified asset must never reach the downloader.
        with patch.object(native, "download_url") as download:
            with self.assertRaisesRegex(RuntimeError, "does not publish a SHA-256 digest"):
                native.windows_sdk(version, "Win64", self.root)
            download.assert_not_called()


if __name__ == "__main__":
    unittest.main()
