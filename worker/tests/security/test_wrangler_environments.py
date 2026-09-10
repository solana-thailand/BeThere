"""Guard non-inheritable staging bindings and fail-closed service defaults."""

from pathlib import Path
import tomllib
import unittest


CONFIG = Path(__file__).resolve().parents[2] / "wrangler.toml"


class WranglerEnvironmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config = tomllib.loads(CONFIG.read_text())
        cls.production = cls.config
        cls.staging = cls.config["env"]["staging"]

    def test_staging_redeclares_every_production_var(self):
        self.assertEqual(
            set(self.production["vars"]),
            set(self.staging["vars"]),
            "Wrangler vars are non-inheritable; staging must redeclare the full set",
        )

    def test_staging_redeclares_rate_limits_with_distinct_namespaces(self):
        production = {item["name"]: item for item in self.production["ratelimits"]}
        staging = {item["name"]: item for item in self.staging["ratelimits"]}
        self.assertEqual(set(production), set(staging))
        self.assertTrue(
            set(item["namespace_id"] for item in production.values()).isdisjoint(
                item["namespace_id"] for item in staging.values()
            )
        )
        for name in production:
            self.assertEqual(production[name]["simple"], staging[name]["simple"])

    def test_staging_external_services_default_to_safe_modes(self):
        variables = self.staging["vars"]
        self.assertEqual(variables["NOTIFICATIONS_ENABLED"], "0")
        self.assertEqual(variables["NOTIFICATION_FROM"], "")
        self.assertIn("devnet", variables["HELIUS_RPC_URL"])
        self.assertEqual(variables["CROSSMINT_HOST"], "staging.crossmint.com")
        self.assertEqual(variables["CROSSMINT_COLLECTION_ID"], "")


if __name__ == "__main__":
    unittest.main()
