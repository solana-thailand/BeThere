# 140 — An issue's Status is prose that goes stale, and nothing records what prod runs

**Status:** mechanism built 2026-09-23 in `scripts/verify/issue_ledger.py` and
`worker/deploy.sh` (provenance). No prod deploy has used the new
`deploy.sh` yet, so no `deploy/production/*` tag exists and the deploy-state
checks report UNKNOWN until the next prod deploy.
**Found:** 2026-09-23, owner question: "how can we trust the existing issues
and docs — should there be a verdict before building a solution?"
**Severity:** medium, process. Nothing is broken in prod. The cost is sessions
planning on claims that stopped being true.

## 1. Evidence

- **Prod does not know its commit.** `wrangler deployments list` for the
  production worker, 2026-09-23: the five most recent deployments
  (`f5b99ef0` … `70989bdb`) have **no message**. The only record of which
  commit a version is lives in hand-written prose
  (`docs/deploy_20260923_runbook.md`). The commit behind `70989bdb` can only be
  inferred from timing (deployed 20:54 +07, between `5f2f4d1` at 20:50 and
  `737df43` at 20:55). It is not recorded.
- **Status claims can't be checked.** Of 136 issues, 24 have no Status line and
  27 have one no classifier can read. 26 issues claim a fix but name no commit,
  either in their Status or from any code commit message.
- **"Not deployed" is written once and never updated.** 18 issues say
  "not deployed". Several are from 2026-09-14, and prod has been deployed from
  `develop` at least five times since. With prod assumed to be `5f2f4d1`, 9 of
  those 18 claims are contradicted by git.
- **"Deployed" hides later work.** 136's Status says "fully fixed and
  DEPLOYED", but its §6.6 fix (`7d6c691`) is not in prod. The ledger flags
  this as `DEPLOY_CONTRADICTED`.
- **Branches are cited after they are gone.** 2 issues name a branch that no
  longer exists (`BRANCH_GONE`).

## 2. What was built

1. **`worker/deploy.sh` records provenance both ways.**
   - Every `wrangler deploy` carries `--message git:<sha>[+dirty]`, so the
     link from a prod version to its commit is on Cloudflare.
   - Every successful deploy (wrangler or PUT fallback) makes a local annotated
     tag `deploy/<env>/<UTC ts>`, so the link from a commit to prod is in git.
   - Tags are **not pushed**. The repo is public; pushing is the owner's call.
   - A failed tag write never fails the deploy.
2. **`scripts/verify/issue_ledger.py`** (read-only) gives each Status claim a
   verdict. Its evidence is:
   - commits cited in the Status block,
   - code-touching commits whose message names the issue (`.issues/NNN`,
     `#NNN`, `(NNN)`, `issue NNN`; doc-only commits excluded), and
   - the newest `deploy/production/*` tag, or `--prod-ref <commit>` for a
     what-if.

   Flags: `STALE_UNDEPLOYED`, `DEPLOY_CONTRADICTED`, `NOT_ON_HEAD`,
   `BRANCH_GONE`, `UNVERIFIABLE`.

## 3. The convention (verdict before solution)

The ledger answers "is the commit there / is it in prod". It **cannot** answer
"is the behaviour still fixed". So before acting on any issue, plan, or doc
claim:

1. Run `python3 scripts/verify/issue_ledger.py`. Treat a flagged claim as
   unknown, not as fact.
2. Re-run the issue's own reproduction or probe against the current code or
   prod, and record the result as `Verified: <date> — <command>`. Only then
   build on it.
3. Write Status as facts that don't decay:
   - `fixed in \`<sha>\`` — the ledger derives deployed or not.
   - Don't write "not deployed". It is true until the next deploy and then
     silently false.
4. Every fix commit names its issue in the message (`.issues/NNN`). This is
   already the common habit (80 `.issues/NNN` and 199 `#NNN` references in
   the log). The ledger turns it into evidence.

## 4. Remaining

- [ ] First prod deploy with the new `deploy.sh`. Then check that
      `wrangler deployments list --json` shows `workers/message = git:…` and
      that `git tag -l 'deploy/*'` shows the tag. The flag is in
      `wrangler deploy --help` on 4.99.0, but no real deploy has used it yet.
- [ ] Owner: push `deploy/*` tags or keep them local? Keeping them local means
      only this machine has the git → prod link. The Cloudflare message covers
      the other direction from anywhere.
- [ ] Correct the flagged Statuses **after** the first tagged deploy, from the
      ledger's evidence, not by hand from inference.
- [ ] Optional: run the ledger in CI in report mode (it exits 0 without
      `--strict`).

## สรุปภาษาไทย

- สถานะใน issue เป็นข้อความที่เขียนครั้งเดียวแล้วเก่าลงเรื่อย ๆ และ prod ไม่เคยบันทึกว่ารัน commit ไหน (deployment 5 ครั้งล่าสุดไม่มี message เลย)
- ทำแล้ว:
  - `deploy.sh` ใส่ commit ลงใน message ของ version และสร้าง git tag `deploy/<env>/<เวลา>` ทุกครั้งที่ deploy
  - `issue_ledger.py` ตรวจสถานะทุก issue เทียบกับ git
- หลักการ: ก่อนลงมือแก้ ให้ตรวจก่อน (verdict) ด้วยสคริปต์ แล้วรัน reproduction ของ issue นั้นซ้ำ · เขียนสถานะเป็น "fixed in `<sha>`" แทน "not deployed"
