-- A wallet address stored on a developer profile is an identity binding only
-- after both sides were proved: Google verified the email and SIWS verified the
-- wallet. Existing rows have unknown provenance and therefore stay untrusted.
ALTER TABLE developer_profiles ADD COLUMN wallet_email_verified INTEGER NOT NULL DEFAULT 0 CHECK(wallet_email_verified IN (0,1));
ALTER TABLE developer_profiles ADD COLUMN wallet_email_verified_at TEXT;
ALTER TABLE developer_profiles ADD COLUMN wallet_email_issuer TEXT CHECK(wallet_email_issuer IS NULL OR wallet_email_issuer IN ('google'));

CREATE UNIQUE INDEX idx_dev_profiles_verified_wallet
ON developer_profiles(LOWER(wallet_address))
WHERE wallet_email_verified = 1 AND wallet_address IS NOT NULL;
