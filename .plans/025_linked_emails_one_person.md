# 025 — Linked emails: several emails, one person

Status: **Phase 1 built locally (2026-09-18), not deployed.** §6.1–6.3 are
still open, and only the tasks they gate are left undone.
Origin: [#122](../.issues/122_credit_invisible_when_registered_with_other_email.md).
The owner chose option C (2026-09-18) and asked that one person with several emails
be treated as one person for money and, where possible, for NFT claims.

Deadline: the #122 attendee is registered for RTM #6, which ends
**2026-09-27 16:00 +07**. Phase 1 must be deployed before then, or that case falls
back to #122 option B.

## 1. Today (facts, 2026-09-18)

- There is no person id. The lowercased email is the identity key everywhere:
  `developer_profiles`, `contacts`, `staff`, `credit_ledger`, attendee rows
  (unique on `event_id, LOWER(email)`), and JWT `Claims.email`.
- The only cross-identity link is **one wallet → one verified email**
  (`0031_wallet_email_provenance.sql`, unique on the wallet).
- Registration dedup compares emails only (`register/signup.rs:205-223`, plus the
  D1 unique index).
- NFT claims are one per **attendee row** (`claimed_at` + `claim_locks` on
  `(event_id, token)`). Two emails give two rows, which allows two claims, even
  to the same wallet.
- GitHub and Telegram handles have no unique index, so one handle can sit on many
  emails.

## 2. Model

New table `person_emails` (migration `0043`):

| column | notes |
|---|---|
| `email` TEXT PRIMARY KEY | lowercased; an email belongs to at most one person |
| `person_id` TEXT NOT NULL | UUIDv7; indexed |
| `is_primary` INTEGER | exactly one per person (partial unique index) |
| `proof` TEXT NOT NULL | CHECK `google` \| `admin`; only `google` is written today |
| `linked_at` TEXT NOT NULL | |

`admin` is already allowed by the CHECK because widening one later means
rebuilding the table in SQLite. The `linked_by` / `note` columns an admin link
needs are a plain `ALTER TABLE ADD COLUMN` once §6.1 is answered.

- An email with no row is implicitly its own person. There's no backfill, and
  nothing changes until someone links.
- There is one resolver, `db::person::emails_of(db, email) -> Vec<String>`
  (always includes `email` itself). Every person-aware call site goes through it.
  This is the DRY seam; nothing else touches `person_emails`.
- **Ledger history is not rewritten.** Rows stay per email (append-only). Only
  reads and the spend guard aggregate over the person.

## 3. Linking — proof of ownership of both emails

### 3.1 Why the callback checks the cookie as well as the HMAC

The signed state alone proves only that *someone's* session asked for a link. An
attacker could mint a state for their own email and send the URL to a victim;
the victim's Google sign-in would then join the victim's email — and credit — to
the attacker's person. So the callback also requires the request to carry the
session cookie of the email in the state. `SameSite=Lax` sends it on Google's
top-level GET redirect.

- **Self-service (default):** signed in with Google as A → Profile → "Add another
  email" → Google OAuth for B. The state is HMAC-signed and carries A; reuse
  `sign_link_state` / `verify_link_state` from `social_link.rs` with a new tag,
  10-minute TTL. The callback requires `email_verified` for B, then links B into
  A's person.
  - Google login covers Gmail and any Google Workspace domain. The #122 work
    domain has Google MX (`aspmx.l.google.com`), so this attendee can self-link.
- **Admin link (super-admin only):** for non-Google emails or unreachable
  attendees. Needs a reason; writes `audit_log`. The risk is merging two
  *different* people's credit, so it's narrow on purpose.
- **Conflicts (v1):**
  - B already linked to another person that has other emails → reject and ask an
    admin. No automatic multi-email merges.
  - B is someone's verified wallet email → allowed; the wallet stays bound to B.
- **Unlink:** admin only, audited. Refused if the move would leave any email's
  per-email ledger sum negative, which would strand money on the wrong side.
- **Sessions do not change in v1.** Logging in with B still gives
  `Claims.email = B`. Profile shows the linked set. Making sessions canonical
  (always the primary) touches every email-keyed table and is Phase 3, only if
  needed.

## 4. Effects (Phase 1 — money first)

1. **Credit balance:** `credit_ledger::balance`, `positive_balances` and
   `try_spend` sum over `person_emails_of!`; `thb_balances_by_email` and
   `liability` group by person. The roster badge and Apply Credit therefore show
   on either email's row, including one with no ledger rows of its own.
2. **Credit spend:** the balance subquery in `credit_coverage::apply`'s guard
   becomes `WHERE email IN person_emails_of!("?1")`, the same fragment every
   reader uses. The `apply` row is still written under the **registered** email, so the return
   (`return:{event}:{email}`) pairs with it exactly as today. Per-email sums can
   go negative; the person sum is the truth.
3. **Liability and payout queue:** these group by person, so one person's
   withdrawal request is one row, not one per email.
4. **Signup auto-apply** uses the same resolver. `credit_identity_ok` is
   unchanged: the session must still prove the *typed* email.

## 5. Effects (Phase 2 — duplicates and NFTs)

1. **Registration dedup:** signup rejects when any email of the person already
   has a row in this event ("you're already registered as a…@…"). Walk-ins are
   exempt, as they are today.
2. **Claim:** `claim/execute.rs` refuses when another row in the same event whose
   email is in the person's set already has `claimed_at`.
3. **Recipient wallet, one badge per event (recommended, independent of
   linking):** refuse a second claim in the same event to a wallet that already
   received one. Cheap: a unique partial index on
   `(event_id, LOWER(recipient_wallet))`.

### What linking can and cannot stop (be explicit)

Linking is voluntary. Someone farming NFTs with throwaway emails will simply
**not link**, so linking alone does **not** prevent multi-email farming. It
covers honest duplicates and cases an admin has linked. The real limits are:

- in-person check-in (already required), which is physical;
- the per-event recipient-wallet rule (§5.3), which raises the cost (wallets are
  free, but each needs its own claim and its own gas-less mint);
- a **possible-duplicate flag** on the roster (same normalized name, same wallet,
  or same GitHub/Telegram handle across emails) so staff can link, or reject,
  before check-in.

Nothing short of KYC stops a determined farmer on online events. We don't claim
otherwise.

## 6. Owner decisions

- [x] 6.0 **Login model:** shared credit only (owner, 2026-09-18). Sessions keep
      the email that was used; credit, dedup and claims resolve over the linked set.
      Phase 3 (canonical session) stays out of scope.

- [ ] 6.1 **Admin link:** allow super-admin manual linking with a mandatory reason
      and audit? (Recommended: yes. It's needed for non-Google emails.)
- [ ] 6.2 **One badge per wallet per event** (§5.3)? (Recommended: yes.)
- [ ] 6.3 **Possible-duplicate roster flag** (§5)? (Recommended: yes, but after
      Phase 1.)

## 7. Tasks

Phase 1 (target: deployed before 2026-09-27)
- [x] 7.1 Migration `0043_person_emails.sql` + `db::person` (the
      `person_emails_of!` fragment, `link_google`, `emails_of`) + SQLite
      behaviour tests in `worker/tests/security/test_person_emails.py`
      (implicit person, symmetric link, no merge of two linked people, one
      primary per person).
- [x] 7.2 Person-aware `balance`, `positive_balances`, `try_spend`,
      `thb_balances_by_email`, `liability` holders, `reconcile`'s
      negative-balance alarm, `credit_coverage::apply`'s guard and the payout
      queue (`contacts::credit_refund_requests`). Tested: spend from a linked
      email once, per-email negative with a healthy person sum, the roster map
      covering an email with no ledger rows, unlinked behaviour unchanged.
      Reverse-patched to a single-email fragment: exactly the four
      person-sharing tests fail, the unlinked ones stay green.
- [x] 7.3 Self-service Google link flow: `GET /api/auth/email-link` →
      Google account chooser → the existing `/api/auth/callback` branches on the
      `email-link:` state (`handlers::email_link`), so **no new redirect URI**
      is needed in the Google console. Profile page lists the linked emails and
      has "Add another email". The callback requires both the signed state and
      the matching session cookie — see §3.1.
- [ ] 7.4 Admin link/unlink endpoint (gated on 6.1) + audit_log.
- [ ] 7.5 Staging verification: two Google accounts, hold on A, register with B,
      Apply Credit shows on B's roster row and spends once. **Needs the owner** —
      it requires signing in to two real Google accounts in a browser, which no
      agent can do. Everything below it is automated instead: the SQL runs
      against the production migrations in `test_person_emails.py`.

Phase 2
- [x] 7.6 Person-aware registration dedup (`register/signup.rs`): the person's
      own email is matched first, a linked sibling row second, so registering
      with the second email returns the existing registration instead of opening
      a duplicate. A D1 failure degrades to exact-email dedup, never a block.
- [x] 7.7 Person-aware claim guard (`db::person::claimed_elsewhere`, wired into
      `claim/mint/execute.rs` between the per-row `claimed_at` check and the
      mint): one badge per person per event. Best-effort — it reads the D1
      attendee mirror, so a missing row means no extra block, never a false one.
      Guards in `worker/tests/linked_email_guards.rs`; SQL behaviour in
      `test_person_emails.py`.
- [ ] 7.8 Per-event recipient-wallet uniqueness (gated on 6.2). This is the
      part that bites a farmer who never links; 7.6/7.7 only bind linked emails.
- [ ] 7.9 Possible-duplicate roster flag (gated on 6.3).

Phase 3 (only if needed)
- [ ] 7.10 Canonical session email (primary) across email-keyed tables.

## 8. Definition of done

- The #122 attendee's ฿500 is spendable from their RTM #6 row without any
  hand-written ledger row.
- An unlinked email behaves exactly as before (regression tests).
- Every link has proof (`google`) or an admin, reason and audit row.
