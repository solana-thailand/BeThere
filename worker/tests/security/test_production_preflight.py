"""Regression guards for the production deployment preflight policy."""

from pathlib import Path
import unittest


DEPLOY = (Path(__file__).parents[2] / "deploy.sh").read_text()


class ProductionPreflightTests(unittest.TestCase):
    def test_production_gate_is_not_environment_opt_in(self):
        self.assertNotIn('BETHERE_PREFLIGHT_GATE:-0', DEPLOY)
        self.assertIn('run_preflight_gate ||', DEPLOY)

    def test_staging_and_dev_skip_the_production_gate(self):
        self.assertIn('if [ "$DEPLOY_ENV" != "production" ]; then', DEPLOY)

    def test_force_requires_reason_and_is_audited(self):
        self.assertIn('if [ -z "$DEPLOY_FORCE_REASON" ]; then', DEPLOY)
        self.assertIn('log_preflight_bypass', DEPLOY)
        self.assertIn('>> "$PREFLIGHT_AUDIT_LOG"', DEPLOY)


if __name__ == "__main__":
    unittest.main()
