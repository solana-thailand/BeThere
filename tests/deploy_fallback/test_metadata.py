"""PUT API metadata completeness, checked against the real wrangler.toml.

The fallback used to carry a hardcoded copy of `[vars]` and three bindings.
That copy drifted: six variables wrangler.toml declares were never sent, and
four wrangler.toml had commented out still were. A fallback deploy therefore
shipped a different production configuration than `wrangler deploy`.

These tests fail the moment wrangler.toml grows a variable or binding the
fallback cannot carry, so the drift cannot come back silently.
"""

import pathlib
import unittest

from deploy_fallback.metadata import build_metadata, load_config, unsupported_config

WRANGLER_TOML = pathlib.Path(__file__).resolve().parents[2] / "worker" / "wrangler.toml"

MINIMAL_CONFIG = {
    "compatibility_date": "2024-09-23",
    "compatibility_flags": ["nodejs_compat"],
    "vars": {"A": "1"},
    "kv_namespaces": [{"binding": "EVENTS", "id": "kv-id"}],
    "d1_databases": [{"binding": "DB", "database_id": "d1-id"}],
    "r2_buckets": [{"binding": "ASSETS_BUCKET", "bucket_name": "bucket"}],
    "ratelimits": [
        {"name": "AUTH_RATE_LIMITER", "namespace_id": "63201",
         "simple": {"limit": 20, "period": 60}},
    ],
    "assets": {"not_found_handling": "single-page-application"},
}


def names_of(metadata, binding_type):
    return {b["name"] for b in metadata["bindings"] if b["type"] == binding_type}


class RealConfigCompletenessTest(unittest.TestCase):
    """The production wrangler.toml must be fully representable."""

    @classmethod
    def setUpClass(cls):
        cls.config = load_config(str(WRANGLER_TOML))
        cls.metadata = build_metadata(cls.config, "jwt", "shim.js")

    def test_every_declared_var_becomes_a_plain_text_binding(self):
        # Cloudflare's PUT API ignores a top-level `vars` dict; anything missing
        # from `bindings` silently reverts to the worker's hardcoded default.
        self.assertEqual(names_of(self.metadata, "plain_text"), set(self.config["vars"]))

    def test_no_variable_is_invented_beyond_wrangler_toml(self):
        for binding in self.metadata["bindings"]:
            if binding["type"] == "plain_text":
                with self.subTest(name=binding["name"]):
                    self.assertEqual(binding["text"], self.config["vars"][binding["name"]])

    def test_every_kv_d1_and_r2_binding_is_carried(self):
        self.assertEqual(
            names_of(self.metadata, "kv_namespace"),
            {e["binding"] for e in self.config["kv_namespaces"]},
        )
        self.assertEqual(
            names_of(self.metadata, "d1"),
            {e["binding"] for e in self.config["d1_databases"]},
        )
        self.assertEqual(
            names_of(self.metadata, "r2_bucket"),
            {e["binding"] for e in self.config["r2_buckets"]},
        )

    def test_compatibility_settings_come_from_the_config(self):
        self.assertEqual(
            self.metadata["compatibility_date"], self.config["compatibility_date"]
        )
        self.assertEqual(
            self.metadata["compatibility_flags"], self.config["compatibility_flags"]
        )

    def test_rate_limiters_are_reported_as_not_carried(self):
        # They are declared in wrangler.toml but withheld until the binding type
        # is verified against the PUT API (Issue #065 step 5). Silence would be
        # a security downgrade nobody notices.
        warnings = " ".join(unsupported_config(self.config, include_ratelimits=False))
        for entry in self.config["ratelimits"]:
            with self.subTest(limiter=entry["name"]):
                self.assertIn(entry["name"], warnings)


class BindingShapeTest(unittest.TestCase):
    """Field names the API requires, which differ from wrangler.toml's."""

    def setUp(self):
        self.metadata = build_metadata(MINIMAL_CONFIG, "the-jwt", "shim.js")
        self.by_name = {b["name"]: b for b in self.metadata["bindings"]}

    def test_kv_uses_namespace_id_not_id(self):
        self.assertEqual(
            self.by_name["EVENTS"], {"type": "kv_namespace", "name": "EVENTS", "namespace_id": "kv-id"}
        )

    def test_d1_uses_id_not_database_id(self):
        self.assertEqual(self.by_name["DB"], {"type": "d1", "name": "DB", "id": "d1-id"})

    def test_r2_keeps_bucket_name(self):
        self.assertEqual(
            self.by_name["ASSETS_BUCKET"],
            {"type": "r2_bucket", "name": "ASSETS_BUCKET", "bucket_name": "bucket"},
        )

    def test_assets_block_carries_the_jwt_and_spa_handling(self):
        self.assertEqual(self.metadata["assets"]["jwt"], "the-jwt")
        self.assertTrue(self.metadata["assets"]["router_config"]["has_user_worker"])
        self.assertEqual(
            self.metadata["assets"]["asset_config"]["not_found_handling"],
            "single-page-application",
        )

    def test_rate_limiters_are_excluded_by_default(self):
        self.assertEqual(names_of(self.metadata, "ratelimit"), set())

    def test_rate_limiters_are_included_on_request(self):
        metadata = build_metadata(MINIMAL_CONFIG, "jwt", "shim.js", include_ratelimits=True)
        limiter = next(b for b in metadata["bindings"] if b["type"] == "ratelimit")
        self.assertEqual(limiter["namespace_id"], "63201")
        self.assertEqual(limiter["simple"], {"limit": 20, "period": 60})

    def test_a_non_string_var_is_rejected_loudly(self):
        config = dict(MINIMAL_CONFIG, vars={"DEV_MODE": 0})
        with self.assertRaises(TypeError):
            build_metadata(config, "jwt", "shim.js")


class UnsupportedConfigTest(unittest.TestCase):
    def test_durable_objects_are_reported(self):
        config = dict(MINIMAL_CONFIG, durable_objects={"bindings": [{"name": "EVENT_DO"}]})
        self.assertTrue(
            any("durable_objects" in w for w in unsupported_config(config, False))
        )

    def test_headers_limitation_is_always_reported(self):
        self.assertTrue(any("_headers" in w for w in unsupported_config({}, False)))


if __name__ == "__main__":
    unittest.main()
