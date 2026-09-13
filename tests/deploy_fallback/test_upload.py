"""Asset upload flow — the three bugs that already cost a production deploy.

  1. Uploading every file instead of only the buckets the API requested.
  2. Sending application/json, so uploads silently failed and the *init* JWT
     was handed to the PUT API as if it were the completion JWT.
  3. Leaving the `cfwau_` prefix on the JWT, which the PUT API rejects (10021).
"""

import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

from deploy_fallback.upload import _multipart_body, strip_jwt_prefix, upload_assets


class JwtPrefixTest(unittest.TestCase):
    def test_prefix_is_stripped(self):
        self.assertEqual(strip_jwt_prefix("cfwau_abc.def"), "abc.def")

    def test_unprefixed_jwt_is_untouched(self):
        self.assertEqual(strip_jwt_prefix("abc.def"), "abc.def")

    def test_only_a_leading_prefix_is_removed(self):
        self.assertEqual(strip_jwt_prefix("x.cfwau_y"), "x.cfwau_y")


class MultipartBodyTest(unittest.TestCase):
    def test_part_declares_the_assets_real_content_type(self):
        body, boundary = _multipart_body("hash1", "YmFzZTY0", "text/javascript; charset=utf-8")
        text = body.decode()
        self.assertIn("Content-Type: text/javascript; charset=utf-8\r\n", text)
        self.assertNotIn("octet-stream", text)

    def test_field_and_filename_are_the_content_hash(self):
        body, _ = _multipart_body("hash1", "YmFzZTY0", "application/wasm")
        self.assertIn('name="hash1"; filename="hash1"', body.decode())

    def test_body_opens_and_closes_on_the_reported_boundary(self):
        body, boundary = _multipart_body("h", "v", "application/null")
        text = body.decode()
        self.assertTrue(text.startswith(f"--{boundary}\r\n"))
        self.assertTrue(text.endswith(f"--{boundary}--\r\n"))


def _response(payload):
    # BytesIO is already a context manager, which is all urlopen's caller needs.
    return io.BytesIO(json.dumps(payload).encode())


class UploadAssetsTest(unittest.TestCase):
    def setUp(self):
        self.dist = tempfile.TemporaryDirectory()
        self.addCleanup(self.dist.cleanup)
        root = pathlib.Path(self.dist.name)
        (root / "app.js").write_bytes(b"export {}")
        (root / "old.css").write_bytes(b"body{}")
        self.manifest = {
            "/app.js": {"hash": "hash-new", "size": 9},
            "/old.css": {"hash": "hash-old", "size": 6},
        }

    def _run(self, init_payload, upload_payloads):
        responses = [_response(init_payload)] + [_response(p) for p in upload_payloads]
        requests = []

        def fake_urlopen(request, *args, **kwargs):
            requests.append(request)
            return responses.pop(0)

        with (
            mock.patch("deploy_fallback.upload.urlopen", fake_urlopen),
            mock.patch("deploy_fallback.upload.status"),
        ):
            jwt = upload_assets(
                api_base="https://api.example/accounts/acct",
                script="bethere",
                oauth="oauth-token",
                dist=self.dist.name,
                manifest=self.manifest,
            )
        return jwt, requests

    def test_only_requested_hashes_are_uploaded(self):
        jwt, requests = self._run(
            {"result": {"jwt": "init-jwt", "buckets": [["hash-new"]]}},
            [{"result": {"jwt": "completion-jwt"}}],
        )
        # One session init + exactly one upload: `old.css` was not requested.
        self.assertEqual(len(requests), 2)
        self.assertIn('name="hash-new"', requests[1].data.decode())
        self.assertEqual(jwt, "completion-jwt")

    def test_uploads_are_multipart_not_json(self):
        _, requests = self._run(
            {"result": {"jwt": "init-jwt", "buckets": [["hash-new"]]}},
            [{"result": {"jwt": "completion-jwt"}}],
        )
        content_type = requests[1].get_header("Content-type")
        self.assertTrue(content_type.startswith("multipart/form-data; boundary="))

    def test_uploads_authenticate_with_the_init_jwt(self):
        _, requests = self._run(
            {"result": {"jwt": "init-jwt", "buckets": [["hash-new"]]}},
            [{"result": {"jwt": "completion-jwt"}}],
        )
        self.assertEqual(requests[1].get_header("Authorization"), "Bearer init-jwt")

    def test_the_last_completion_jwt_wins_over_earlier_ones(self):
        jwt, _ = self._run(
            {"result": {"jwt": "init-jwt", "buckets": [["hash-new", "hash-old"]]}},
            [{"result": {"jwt": "first"}}, {"result": {"jwt": "final"}}],
        )
        self.assertEqual(jwt, "final")

    def test_an_up_to_date_deploy_returns_the_init_jwt_unprefixed(self):
        jwt, requests = self._run({"result": {"jwt": "cfwau_init-jwt", "buckets": []}}, [])
        self.assertEqual(len(requests), 1)
        self.assertEqual(jwt, "init-jwt")

    def test_a_silent_upload_without_a_jwt_fails_instead_of_reusing_the_init_jwt(self):
        with self.assertRaises(RuntimeError):
            self._run(
                {"result": {"jwt": "init-jwt", "buckets": [["hash-new"]]}},
                [{"result": {}}],
            )


if __name__ == "__main__":
    unittest.main()
