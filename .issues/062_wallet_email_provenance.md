# 062 — Wallet-linked email lacks verification provenance

Priority: P1 security review. Status: open; notification enrollment isolated from it.

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

## Remaining review (do not infer an exploit from link existence alone)

Audit every operation relying on a wallet-linked email as authorization, especially
role resolution, stored-credit access, wallet binding/unbinding, and profile writes.
Determine which links originated from authenticated email owners and which from
first-time typed emails. Existing rows do not record this provenance.

## Proposed fix and acceptance criteria

- Store link provenance (`email_verified`, verification time, issuer) separately
  from a self-declared contact address. Wallet possession must not upgrade it.
- Preserve legitimate sign-in with wallets linked by authenticated email owners.
- Treat legacy provenance as unknown, with a re-verification path rather than
  silently upgrading or irreversibly deleting links.
- Attacker wallet + typed third-party email must not gain email-scoped privileges,
  including on a subsequent login or second-event registration.
- Verified email owner can link a wallet and continue intended wallet-only access.
- Test both first-use and returning-user flows, not just duplicate registration.

This is broader than email delivery; resolve it before claiming the platform's
wallet/email authorization has been fully reviewed for production.
