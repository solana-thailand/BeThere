# 062 — Wallet-linked email lacks verification provenance

Priority: P1 security review. Status: resolved locally; awaiting migration/deploy.

## Evidence

`worker/src/handlers/register/signup.rs` auto-links a wallet to a newly typed email
when `email_is_new` is true. This proves wallet possession, not email ownership.
`worker/src/handlers/auth.rs::wallet_verify` subsequently looks up that link and
issues a session whose `email` is the linked address. Other handlers can no longer
infer authentication provenance from the presence/absence of a `wallet:` prefix.

The notification implementation previously reused `credit_identity_ok`, which can
be true for such links. It now requires a separate signed `Claims.email_verified`
attestation, minted only by the verified Google callback. Wallet-issued and legacy
sessions default false. This closes notification enrollment through that path.

## Resolution

- Migration `0031_wallet_email_provenance.sql` records whether both sides of a
  wallet/email binding were proved, when that happened, and the email issuer.
  Existing bindings default to unknown and are never silently upgraded.
- Wallet login resolves only verified `developer_profiles` bindings. Deposit
  recipient wallets and typed registration emails remain contact/payment data.
- Wallet binding requires a Google-verified JWT plus the existing SIWS proof.
  A partial unique index enforces one verified email per wallet under concurrency.
- Wallet registration no longer auto-binds a typed email. The UI directs the
  attendee to sign in with Google and connect the wallet from Profile.
- Stored-credit ownership resolution uses only a verified wallet/email binding.
- Executed migration tests cover legacy, recipient, verified, case-insensitive,
  and conflicting bindings. CI now runs this security SQL suite.

## Acceptance criteria

- Store link provenance (`email_verified`, verification time, issuer) separately
  from a self-declared contact address. Wallet possession must not upgrade it.
- Preserve legitimate sign-in with wallets linked by authenticated email owners.
- Treat legacy provenance as unknown, with a re-verification path rather than
  silently upgrading or irreversibly deleting links.
- Attacker wallet + typed third-party email must not gain email-scoped privileges,
  including on a subsequent login or second-event registration.
- Verified email owner can link a wallet and continue intended wallet-only access.
- Test both first-use and returning-user flows, not just duplicate registration.

Local implementation and checks satisfy these criteria. Production remains on
the old behavior until migration 0031 and the Worker/frontend release are deployed.
