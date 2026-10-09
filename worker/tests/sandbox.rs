//! Devnet sandbox (plan 042 0.4): the Worker-held keys, the one place they
//! sign, the two transactions the escrow builders do not cover, the
//! confirmation reader, and the rule that keeps the sandbox off prod.

use ed25519_dalek::{Signature, SigningKey, Verifier};
use event_checkin_worker::sandbox::config::{Off, decide};
use event_checkin_worker::sandbox::keys::{KeyError, SandboxSigner, signer_slot};
use event_checkin_worker::sandbox::send::{SignatureState, signature_state};
use event_checkin_worker::sandbox::tx::{USDC_DECIMALS, faucet_grant_tx, token_return_tx};
use event_checkin_worker::solana_escrow::{get_associated_token_address, pubkey_from_base58};
use serde_json::json;

const USDC_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

/// The keypair file `solana-keygen` would write for `seed`.
fn keypair_json(seed: u8) -> String {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let mut all = vec![seed; 32];
    all.extend_from_slice(&key.verifying_key().to_bytes());
    serde_json::to_string(&all).unwrap()
}

fn signer(seed: u8) -> SandboxSigner {
    SandboxSigner::from_keypair_json(&keypair_json(seed)).unwrap()
}

/// The message bytes of a one-signer legacy transaction.
fn message(tx: &[u8]) -> &[u8] {
    &tx[1 + 64 * usize::from(tx[0])..]
}

/// Account keys of a message whose key count fits one byte.
fn keys(msg: &[u8]) -> Vec<[u8; 32]> {
    let count = usize::from(msg[3]);
    msg[4..4 + 32 * count].as_chunks::<32>().0.to_vec()
}

#[test]
fn keypair_file_loads_and_shows_only_its_address() {
    let loaded = signer(7);
    let public = SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes();
    assert_eq!(loaded.pubkey(), &public);
    assert_eq!(
        pubkey_from_base58(loaded.address()).unwrap(),
        public,
        "address is the base58 of the public key"
    );
    let shown = format!("{loaded:?}");
    assert!(shown.contains(loaded.address()));
    assert!(
        !shown.contains("7, 7, 7"),
        "Debug must not print the seed: {shown}"
    );
}

#[test]
fn keypair_file_whose_halves_disagree_is_refused() {
    let mut all: Vec<u8> = serde_json::from_str(&keypair_json(7)).unwrap();
    all[63] ^= 1;
    let raw = serde_json::to_string(&all).unwrap();
    assert_eq!(
        SandboxSigner::from_keypair_json(&raw).unwrap_err(),
        KeyError::PubkeyMismatch
    );
}

#[test]
fn anything_but_64_json_bytes_is_refused() {
    for raw in [
        "",
        "[1,2,3]",
        "not json",
        "\"base58string\"",
        &"[1]".repeat(2),
    ] {
        assert_eq!(
            SandboxSigner::from_keypair_json(raw).unwrap_err(),
            KeyError::NotKeypairJson,
            "{raw:?}"
        );
    }
    let mut all: Vec<u8> = serde_json::from_str(&keypair_json(7)).unwrap();
    all.push(0);
    assert_eq!(
        SandboxSigner::from_keypair_json(&serde_json::to_string(&all).unwrap()).unwrap_err(),
        KeyError::NotKeypairJson
    );
}

#[tokio::test]
async fn faucet_grant_is_one_signer_three_instructions_and_signs_verifiably() {
    let faucet = signer(1);
    let visitor = signer(2);
    let mint = pubkey_from_base58(USDC_DEVNET).unwrap();
    let blockhash = [9u8; 32];
    let mut tx = faucet_grant_tx(
        faucet.pubkey(),
        visitor.pubkey(),
        &mint,
        3_000_000,
        1_000_000,
        &blockhash,
    )
    .await
    .unwrap();

    assert_eq!(tx[0], 1, "one signature slot");
    let msg = message(&tx).to_vec();
    assert_eq!(
        &msg[..3],
        &[1, 0, 4],
        "1 signer; readonly: mint + System, Token, ATA programs"
    );
    let account_keys = keys(&msg);
    assert_eq!(account_keys[0], *faucet.pubkey(), "the faucet pays");
    let visitor_ata = get_associated_token_address(visitor.pubkey(), &mint)
        .await
        .unwrap();
    assert!(account_keys.contains(&visitor_ata));
    let after_keys = 4 + 32 * account_keys.len();
    assert_eq!(&msg[after_keys..after_keys + 32], &blockhash);
    assert_eq!(msg[after_keys + 32], 3, "SOL, create ATA, USDC");

    let mut checked = vec![12u8];
    checked.extend_from_slice(&1_000_000u64.to_le_bytes());
    checked.push(USDC_DECIMALS);
    assert!(
        msg.windows(checked.len()).any(|w| w == checked.as_slice()),
        "TransferChecked of 1 USDC at 6 decimals"
    );

    assert_eq!(signer_slot(&tx, faucet.pubkey()).unwrap(), 0);
    assert_eq!(
        signer_slot(&tx, visitor.pubkey()).unwrap_err(),
        KeyError::NotASigner,
        "a writable non-signer is not a slot"
    );
    assert_eq!(
        visitor.sign_transaction(&mut tx).unwrap_err(),
        KeyError::NotASigner
    );
    faucet.sign_transaction(&mut tx).unwrap();
    let signature = Signature::from_bytes(tx[1..65].try_into().unwrap());
    SigningKey::from_bytes(&[1; 32])
        .verifying_key()
        .verify(&msg, &signature)
        .expect("the slot holds the faucet's signature over the message");
}

#[tokio::test]
async fn token_return_is_signed_and_paid_by_the_visitor() {
    let faucet = signer(1);
    let visitor = signer(2);
    let mint = pubkey_from_base58(USDC_DEVNET).unwrap();
    let tx = token_return_tx(
        visitor.pubkey(),
        faucet.pubkey(),
        &mint,
        1_000_000,
        &[3; 32],
    )
    .await
    .unwrap();
    let account_keys = keys(message(&tx));
    assert_eq!(account_keys[0], *visitor.pubkey());
    let faucet_ata = get_associated_token_address(faucet.pubkey(), &mint)
        .await
        .unwrap();
    assert!(account_keys.contains(&faucet_ata));
    assert!(
        !account_keys.contains(faucet.pubkey()),
        "the faucet does not sign"
    );
}

#[test]
fn malformed_transactions_are_refused_not_indexed() {
    let key = [1u8; 32];
    assert_eq!(signer_slot(&[], &key), Err(KeyError::MalformedTransaction));
    assert_eq!(signer_slot(&[0], &key), Err(KeyError::MalformedTransaction));
    assert_eq!(
        signer_slot(&[1; 40], &key),
        Err(KeyError::MalformedTransaction)
    );
}

#[test]
fn signature_states_read_from_rpc() {
    let state = |value| signature_state(&json!({"result": {"value": [value]}}));
    assert_eq!(state(json!(null)), Ok(SignatureState::Pending));
    assert_eq!(
        state(json!({"err": null, "confirmationStatus": "processed"})),
        Ok(SignatureState::Pending)
    );
    assert_eq!(
        state(json!({"err": null, "confirmationStatus": "confirmed"})),
        Ok(SignatureState::Confirmed)
    );
    assert_eq!(
        state(json!({"err": null, "confirmationStatus": "finalized"})),
        Ok(SignatureState::Confirmed)
    );
    assert!(matches!(
        state(
            json!({"err": {"InstructionError": [0, {"Custom": 1}]}, "confirmationStatus": "confirmed"})
        ),
        Ok(SignatureState::Failed(_))
    ));
    assert!(signature_state(&json!({"error": {"code": -32005}})).is_err());
}

#[test]
fn sandbox_is_off_unless_staging_devnet_and_two_distinct_keys() {
    let organizer = keypair_json(1);
    let faucet = keypair_json(2);
    let on = |dev, cluster: &str, o: Option<&str>, f: Option<&str>| decide(dev, cluster, o, f);
    assert_eq!(
        on(false, "devnet", Some(&organizer), Some(&faucet)).unwrap_err(),
        Off::NotStaging,
        "prod (DEV_MODE off) never runs the sandbox, keys or not"
    );
    assert_eq!(
        on(true, "mainnet-beta", Some(&organizer), Some(&faucet)).unwrap_err(),
        Off::NotDevnet
    );
    assert_eq!(
        on(true, "devnet", None, Some(&faucet)).unwrap_err(),
        Off::KeysMissing
    );
    assert_eq!(
        on(true, "devnet", Some(&organizer), None).unwrap_err(),
        Off::KeysMissing
    );
    assert_eq!(
        on(true, "devnet", Some("junk"), Some(&faucet)).unwrap_err(),
        Off::KeysInvalid
    );
    assert_eq!(
        on(true, "devnet", Some(&organizer), Some(&organizer)).unwrap_err(),
        Off::KeysInvalid,
        "one key may not be both organizer and faucet"
    );
    let keys = on(true, "devnet", Some(&organizer), Some(&faucet)).unwrap();
    assert_eq!(keys.organizer.pubkey(), signer(1).pubkey());
    assert_eq!(keys.faucet.pubkey(), signer(2).pubkey());
}
