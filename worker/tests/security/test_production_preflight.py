"""Regression guards for the production deploy gate (staging parity).

The flow-harness preflight could never pass (.issues/141), so every prod
deploy used --force. It was replaced by scripts/verify/staging_parity.sh:
prod only gets a tree staging is running now. These tests pin the policy in
deploy.sh and prove the parity script both passes and fails, against a
throwaway git repository.
"""

from pathlib import Path
import subprocess
import tempfile
import unittest


WORKER = Path(__file__).parents[2]
DEPLOY = (WORKER / "deploy.sh").read_text()
PARITY = WORKER.parent / "scripts/verify/staging_parity.sh"


class ProductionGatePolicyTests(unittest.TestCase):
    def test_production_gate_is_not_environment_opt_in(self):
        self.assertNotIn('BETHERE_PREFLIGHT_GATE:-0', DEPLOY)
        self.assertIn('run_preflight_gate ||', DEPLOY)

    def test_staging_and_dev_skip_the_production_gate(self):
        self.assertIn('if [ "$DEPLOY_ENV" != "production" ]; then', DEPLOY)

    def test_force_requires_reason_and_is_audited(self):
        self.assertIn('if [ -z "$DEPLOY_FORCE_REASON" ]; then', DEPLOY)
        self.assertIn('log_preflight_bypass', DEPLOY)
        self.assertIn('>> "$PREFLIGHT_AUDIT_LOG"', DEPLOY)

    def test_the_gate_is_staging_parity_not_the_unsatisfiable_harness(self):
        self.assertIn('npx wrangler deployments status --env staging --json', DEPLOY)
        self.assertIn('bash "$parity_script" "$message"', DEPLOY)
        self.assertNotIn('PREFLIGHT_SCRIPT', DEPLOY)


class StagingParityScriptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "t@example.invalid")
        self.git("config", "user.name", "t")
        (self.repo / "a.txt").write_text("one\n")
        self.git("add", "a.txt")
        self.git("commit", "-q", "-m", "one")
        self.staged_sha = self.git("rev-parse", "HEAD").strip()

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.repo, check=True, capture_output=True, text=True).stdout

    def parity(self, message):
        return subprocess.run(["bash", str(PARITY), message], cwd=self.repo, capture_output=True, text=True)

    def test_same_commit_passes(self):
        self.assertEqual(self.parity(f"git:{self.staged_sha}").returncode, 0)

    def test_a_different_commit_with_the_same_tree_passes(self):
        # A release merge on main: new commit, identical content.
        self.git("commit", "-q", "--allow-empty", "-m", "release merge")
        self.assertEqual(self.parity(f"git:{self.staged_sha}").returncode, 0)

    def test_a_different_tree_fails(self):
        (self.repo / "a.txt").write_text("two\n")
        self.git("commit", "-q", "-am", "two")
        result = self.parity(f"git:{self.staged_sha}")
        self.assertEqual(result.returncode, 1)
        self.assertIn("tree differs", result.stderr)

    def test_uncommitted_changes_fail(self):
        (self.repo / "a.txt").write_text("dirty\n")
        result = self.parity(f"git:{self.staged_sha}")
        self.assertEqual(result.returncode, 1)
        self.assertIn("uncommitted", result.stderr)

    def test_a_dirty_staging_deploy_fails(self):
        # deploy.sh records `git:<sha>+dirty` when it deployed uncommitted
        # changes; no commit matches that code. Seen live on 2026-09-28.
        result = self.parity(f"git:{self.staged_sha}+dirty")
        self.assertEqual(result.returncode, 1)
        self.assertIn("uncommitted changes", result.stderr)

    def test_no_git_provenance_fails(self):
        self.assertEqual(self.parity("manual upload").returncode, 1)

    def test_an_unknown_commit_fails(self):
        result = self.parity("git:" + "0" * 40)
        self.assertEqual(result.returncode, 1)
        self.assertIn("git fetch", result.stderr)

    def test_usage_error(self):
        self.assertEqual(subprocess.run(["bash", str(PARITY)], cwd=self.repo, capture_output=True).returncode, 2)


if __name__ == "__main__":
    unittest.main()
