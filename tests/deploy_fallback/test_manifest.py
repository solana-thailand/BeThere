"""Asset manifest contract with the assets-upload-session API."""

import base64
import pathlib
import tempfile
import unittest

import blake3

from deploy_fallback.manifest import CONFIG_FILES, asset_hash, build_manifest


class AssetHashTest(unittest.TestCase):
    def test_hash_matches_wranglers_documented_recipe(self):
        # Spelled out independently: BLAKE3 over base64(contents) + bare extension,
        # truncated to 32 hex chars. A mismatch makes the API request buckets the
        # uploader cannot satisfy, and the deploy stalls with no completion JWT.
        contents = b"console.log(1)\n"
        expected = blake3.blake3(
            (base64.b64encode(contents).decode() + "js").encode()
        ).hexdigest()[:32]
        self.assertEqual(asset_hash(contents, "js"), expected)

    def test_hash_is_32_hex_characters(self):
        digest = asset_hash(b"x", "png")
        self.assertEqual(len(digest), 32)
        self.assertTrue(all(c in "0123456789abcdef" for c in digest))

    def test_extension_is_part_of_the_hash(self):
        self.assertNotEqual(asset_hash(b"same", "js"), asset_hash(b"same", "css"))


class BuildManifestTest(unittest.TestCase):
    def setUp(self):
        self.dist = tempfile.TemporaryDirectory()
        self.addCleanup(self.dist.cleanup)
        self.root = pathlib.Path(self.dist.name)
        (self.root / "index.html").write_bytes(b"<html></html>")
        (self.root / "nested").mkdir()
        (self.root / "nested" / "app.js").write_bytes(b"export {}")
        for name in CONFIG_FILES:
            (self.root / name).write_bytes(b"/* cloudflare config */")

    def test_cloudflare_config_files_are_never_uploaded_as_assets(self):
        # Issue #057: the PUT path cannot apply _headers/_redirects rules, so
        # uploading them only exposes them as fetchable blobs.
        manifest = build_manifest(str(self.root))
        for name in CONFIG_FILES:
            with self.subTest(name=name):
                self.assertNotIn(f"/{name}", manifest)

    def test_nested_files_use_root_relative_slash_paths(self):
        manifest = build_manifest(str(self.root))
        self.assertEqual(set(manifest), {"/index.html", "/nested/app.js"})

    def test_size_is_the_byte_length_of_the_file(self):
        manifest = build_manifest(str(self.root))
        self.assertEqual(manifest["/index.html"]["size"], len(b"<html></html>"))

    def test_entries_carry_the_content_hash(self):
        manifest = build_manifest(str(self.root))
        self.assertEqual(
            manifest["/nested/app.js"]["hash"], asset_hash(b"export {}", "js")
        )


if __name__ == "__main__":
    unittest.main()
