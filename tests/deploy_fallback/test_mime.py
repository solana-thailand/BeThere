"""Content types declared for assets uploaded by the PUT API fallback.

A wrong Content-Type here is not cosmetic: the upload endpoint stores the type
declared on the multipart part and serves it verbatim. On 2026-07-26 a
hardcoded application/octet-stream made the browser download the JS glue
instead of executing it, taking the whole frontend down.
"""

import unittest

from deploy_fallback.mime import MIME_BY_EXTENSION, UNKNOWN_TYPE, mime_for

# Extensions the Leptos frontend actually ships. Each one MUST be mapped
# explicitly — never left to the platform's mimetypes database.
SHIPPED_EXTENSIONS = ("html", "js", "css", "wasm", "json", "svg", "png", "ico", "woff2")


class MimeMappingTest(unittest.TestCase):
    def test_every_shipped_extension_is_mapped_explicitly(self):
        for extension in SHIPPED_EXTENSIONS:
            with self.subTest(extension=extension):
                self.assertIn(extension, MIME_BY_EXTENSION)

    def test_no_shipped_asset_is_served_as_a_download(self):
        for extension in SHIPPED_EXTENSIONS:
            with self.subTest(extension=extension):
                self.assertNotIn("octet-stream", mime_for(f"/app.{extension}"))

    def test_executable_and_text_types_declare_utf8(self):
        for extension in ("html", "js", "mjs", "css", "txt"):
            with self.subTest(extension=extension):
                self.assertIn("charset=utf-8", mime_for(f"/a.{extension}"))

    def test_javascript_and_wasm_types_are_the_ones_browsers_execute(self):
        self.assertEqual(mime_for("/index-abc123.js"), "text/javascript; charset=utf-8")
        self.assertEqual(mime_for("/app_bg.wasm"), "application/wasm")

    def test_extension_match_is_case_insensitive(self):
        self.assertEqual(mime_for("/LOGO.PNG"), mime_for("/logo.png"))

    def test_unmapped_extension_falls_back_to_the_no_type_sentinel(self):
        # Not octet-stream: that forces a download. The sentinel lets the edge decide.
        self.assertEqual(mime_for("/weird.zzzunknown"), UNKNOWN_TYPE)

    def test_file_without_extension_is_not_forced_to_a_type(self):
        self.assertEqual(mime_for("/LICENSE"), UNKNOWN_TYPE)


if __name__ == "__main__":
    unittest.main()
