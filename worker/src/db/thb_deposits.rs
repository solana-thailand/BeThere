//! D1 THB deposit query helpers.
//!
//! THB deposits stored exclusively in D1 (Phase 3d complete).

use worker::D1Database;
use worker::d1::D1Type;

use event_checkin_domain::models::deposit::{DepositSource, ThbDeposit};

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Read a single THB deposit by event + attendee. Returns `None` if not found.
pub async fn get_thb_deposit(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<ThbDeposit>, String> {
    let stmt = db.prepare("SELECT * FROM thb_deposits WHERE event_id = ?1 AND attendee_id = ?2");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_thb_deposit bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_thb_deposit first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_thb_deposit first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_thb_deposit: deserialize failed"
        );
        format!("D1 get_thb_deposit deserialize: {e}")
    })?;

    Ok(Some(row_to_thb_deposit(row)?))
}

/// Find another attendee's deposit in the same event whose slip is byte-identical.
///
/// The lookup behind the duplicate-slip check (`.issues/129`): anyone can
/// upload any image to the deposit page, so the only thing distinguishing a
/// real payer from someone forwarding a friend's slip was the organizer
/// recognising them.
///
/// Three narrowings, each load-bearing:
///
/// * `event_id` — scoped to one event, which keeps this on
///   `idx_thb_deposits_event` and matches what is actually being policed. The
///   same person paying ฿500 to two events sends two different transfers.
/// * `attendee_id <> ?` — an attendee re-uploading their own slip after a
///   rejection is legitimate and must never be flagged.
/// * `slip_blake3 <> ''` — THE dangerous one. This table stores the empty
///   string rather than SQL NULL for absent text (see `insert_thb_deposit`),
///   and `'' = ''` matches. Without this clause, every row with no hash would
///   be a duplicate of every other row with no hash. The caller also refuses to
///   run with an empty fingerprint; both guards are deliberate, because either
///   one alone is a single edit away from matching everything.
pub async fn find_slip_hash_collision(
    db: &D1Database,
    event_id: &str,
    slip_blake3: &str,
    excluding_attendee_id: &str,
) -> Result<Option<ThbDeposit>, String> {
    if slip_blake3.is_empty() {
        return Ok(None);
    }

    let stmt = db
        .prepare(
            "SELECT * FROM thb_deposits \
             WHERE event_id = ?1 AND slip_blake3 = ?2 AND slip_blake3 <> '' AND attendee_id <> ?3 \
             ORDER BY uploaded_at ASC LIMIT 1",
        )
        .bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Text(slip_blake3),
            D1Type::Text(excluding_attendee_id),
        ])
        .map_err(|e| format!("D1 find_slip_hash_collision bind: {e:?}"))?;

    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 find_slip_hash_collision: {e}"))?;

    match rows.into_iter().next() {
        None => Ok(None),
        Some(row) => row_to_thb_deposit(row).map(Some),
    }
}

/// List all THB deposits for an event (newest first).
///
/// Uses `d1_safe::safe_all_rows` — the worker crate's `results()` panics on
/// SQL NULL column values (e.g. rows inserted before the empty-string convention).
pub async fn list_thb_deposits(db: &D1Database, event_id: &str) -> Result<Vec<ThbDeposit>, String> {
    let stmt = db
        .prepare("SELECT * FROM thb_deposits WHERE event_id = ?1 ORDER BY uploaded_at ASC")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 list_thb_deposits bind: {e:?}"))?;

    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 list_thb_deposits: {e}"))?;

    rows.into_iter()
        .map(row_to_thb_deposit)
        .collect::<Result<Vec<_>, _>>()
}

/// How one attendee's THB deposit for an event was settled.
///
/// Deliberately narrow. The admin roster needs three facts to draw a badge and
/// nothing else, and `thb_deposits.slip_url` can hold a multi-megabyte base64
/// data URL — pulling whole rows for a 500-person roster to read three booleans
/// is the kind of thing that only shows up as a slow page at the door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThbSettlement {
    /// An organizer (or the system, for a comp) has accepted this deposit.
    pub verified: bool,
    /// The money has gone back.
    pub refunded: bool,
    /// Cash, rolling credit, or a comp. Reads the `deposit_source` column added
    /// by migration 0047, falling back to the legacy sentinels for rows written
    /// before it — the same order `ThbDeposit::source()` uses.
    pub source: DepositSource,
}

/// Settlement state for every attendee with a THB deposit at one event.
///
/// **Why this exists.** The admin roster's deposit badge used to read
/// `attendees.deposit_status` and the USDC amount columns — and nothing in the
/// THB flow writes either. `save_deposit_status_to_d1` is dead code
/// (`#[allow(dead_code)]`, zero callers), so a staff comp and a
/// credit-covered registration both looked identical to an unpaid attendee:
/// the roster said **"Deposit pending"** for people who owed nothing
/// (`.issues/137`). This is the missing read.
///
/// One query per roster page, batched like the credit-ledger annotations
/// beside it. Attendees with no THB deposit are simply absent from the map.
pub async fn settlement_by_attendee(
    db: &D1Database,
    event_id: &str,
) -> Result<std::collections::HashMap<String, ThbSettlement>, String> {
    // Named columns, not `SELECT *`: see the doc comment on ThbSettlement.
    let stmt = db
        .prepare(
            "SELECT attendee_id, verified, refunded, amount_thb, slip_url, verified_by,              deposit_source              FROM thb_deposits WHERE event_id = ?1",
        )
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 settlement_by_attendee bind: {e:?}"))?;

    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 settlement_by_attendee: {e}"))?;

    let truthy = |v: Option<&serde_json::Value>| match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(serde_json::Value::String(s)) => {
            s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
        }
        _ => false,
    };

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let Some(attendee_id) = row.get("attendee_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let source = classify_source(
            row.get("deposit_source").and_then(|v| v.as_str()),
            row.get("verified_by").and_then(|v| v.as_str()),
            row.get("slip_url").and_then(|v| v.as_str()),
            row.get("amount_thb").and_then(serde_json::Value::as_i64),
        );
        map.insert(
            attendee_id.to_string(),
            ThbSettlement {
                verified: truthy(row.get("verified")),
                refunded: truthy(row.get("refunded")),
                source,
            },
        );
    }
    Ok(map)
}

/// Classify a deposit's source from the stored column, falling back to the
/// legacy sentinels.
///
/// The arm ORDER is load-bearing and is a transcription of
/// `ThbDeposit::source()` in `domain/src/models/deposit.rs`: a row that is both
/// credit-applied AND ฿0 is Credit, not Comp. Migration 0047's backfill is a
/// transcription of the same function in the same order, which is why the
/// backfill and this agree on every production row.
fn classify_source(
    stored: Option<&str>,
    verified_by: Option<&str>,
    slip_url: Option<&str>,
    amount_thb: Option<i64>,
) -> DepositSource {
    match stored {
        Some("credit") => return DepositSource::Credit,
        Some("comp") => return DepositSource::Comp,
        Some("cash") => return DepositSource::Cash,
        _ => {}
    }
    let vb = verified_by.unwrap_or("");
    let slip = slip_url.unwrap_or("");
    if vb == "SYSTEM_ROLLING_CREDIT" || slip == "ROLLING_CREDIT_AUTO_APPLIED" {
        DepositSource::Credit
    } else if vb == "SYSTEM_STAFF_WAIVE" || slip == "STAFF_COMP_WAIVED" || amount_thb == Some(0) {
        DepositSource::Comp
    } else {
        DepositSource::Cash
    }
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Insert a new THB deposit into D1.
///
/// Uses parameterized `bind_refs` — required because `slip_url` may contain a
/// large base64 data URL (several MB) that `db.exec()` cannot inline into a
/// single SQL string. D1 is Cloudflare SQLite, not PostgreSQL/PgCat, so the
/// `raw_sql` convention does not apply here.
pub async fn insert_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, slip_url, verified, verified_by, verified_at, uploaded_at, refunded, refunded_at, attendee_name, bank_account, bank_name, account_name, refund_proof_url, held_as_credit, held_as_credit_at, slip_blake3, deposit_source) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    );
    stmt.bind_refs(&[
        D1Type::Text(&deposit.attendee_id),
        D1Type::Text(&deposit.event_id),
        D1Type::Integer(deposit.amount_thb as i32),
        D1Type::Text(deposit.slip_url.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.verified as i32),
        D1Type::Text(deposit.verified_by.as_deref().unwrap_or("")),
        D1Type::Text(deposit.verified_at.as_deref().unwrap_or("")),
        D1Type::Text(&deposit.uploaded_at),
        D1Type::Integer(deposit.refunded as i32),
        D1Type::Text(deposit.refunded_at.as_deref().unwrap_or("")),
        D1Type::Text(deposit.attendee_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_account.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.account_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.refund_proof_url.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.held_as_credit as i32),
        D1Type::Text(deposit.held_as_credit_at.as_deref().unwrap_or("")),
        D1Type::Text(deposit.slip_blake3.as_deref().unwrap_or("")),
        deposit_source_bind(deposit.deposit_source),
    ])
    .map_err(|e| format!("D1 insert_thb_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 insert_thb_deposit run: {e:?}"))?;

    Ok(())
}

/// Update an existing THB deposit (for verify / slip-upload operations).
///
/// Uses parameterized `bind_refs` — see `insert_thb_deposit` for rationale.
///
/// **The five settlement columns are deliberately absent from the `SET` list**:
/// `refunded`, `refunded_at`, `held_as_credit`, `held_as_credit_at` and
/// `refund_proof_url` are owned exclusively by [`try_settle_refund`] and
/// [`try_settle_hold_credit`] (and, for the proof URL, by
/// [`set_refund_proof_url`]). This is a blanket read-modify-write of every other
/// column, so including them would let any caller holding a row it read earlier
/// retract a settlement that landed in between — resetting `refunded` to 0 on a
/// deposit whose cash has already gone out, which re-arms the refund CAS for a
/// second payout. The callers that do set them in memory (`refund.rs`,
/// `hold_credit.rs`, `hold_admin.rs`) all do so *after* their CAS has already
/// written D1, so dropping them here changes nothing on the intended paths; it
/// only removes the clobber. The KV fallback in `save_thb_deposit` still
/// serialises the whole struct, which is correct — there is no CAS there.
pub async fn update_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET amount_thb = ?1, slip_url = ?2, verified = ?3, verified_by = ?4, verified_at = ?5, attendee_name = ?6, bank_account = ?7, bank_name = ?8, account_name = ?9, slip_blake3 = ?10, deposit_source = ?11 \
         WHERE event_id = ?12 AND attendee_id = ?13",
    );
    stmt.bind_refs(&[
        D1Type::Integer(deposit.amount_thb as i32),
        D1Type::Text(deposit.slip_url.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.verified as i32),
        D1Type::Text(deposit.verified_by.as_deref().unwrap_or("")),
        D1Type::Text(deposit.verified_at.as_deref().unwrap_or("")),
        D1Type::Text(deposit.attendee_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_account.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.account_name.as_deref().unwrap_or("")),
        // Travels with slip_url: a new slip means a new hash, and updating one
        // without the other would leave the previous image's fingerprint
        // attached to the current image.
        D1Type::Text(deposit.slip_blake3.as_deref().unwrap_or("")),
        // The comp decision is a blanket-update column on purpose: it is set by
        // the admin comp action through the same read-modify-write every other
        // non-settlement column uses. The five settlement columns stay out of
        // this SET list for the reason documented above.
        deposit_source_bind(deposit.deposit_source),
        D1Type::Text(&deposit.event_id),
        D1Type::Text(&deposit.attendee_id),
    ])
    .map_err(|e| format!("D1 update_thb_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 update_thb_deposit run: {e:?}"))?;

    Ok(())
}

/// Atomically claim the "hold as credit" settlement for a verified THB deposit.
///
/// Flips `held_as_credit` 0→1 in a single conditional UPDATE that only matches
/// when the deposit is verified, not already held, and not refunded. Returns
/// `Ok(true)` iff THIS call performed the flip — the caller may then increment
/// rolling credit exactly once. `Ok(false)` means it was already settled (a
/// concurrent or prior hold/refund won), so the caller must NOT grant credit.
/// This closes the check-then-write race where two concurrent `/hold` requests
/// both pass the in-memory guard and double-increment credit.
pub async fn try_settle_hold_credit(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
    held_at: &str,
) -> Result<bool, String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET held_as_credit = 1, held_as_credit_at = ?1 \
         WHERE event_id = ?2 AND attendee_id = ?3 \
           AND verified = 1 AND held_as_credit = 0 AND refunded = 0",
    );
    let result = stmt
        .bind_refs(&[
            D1Type::Text(held_at),
            D1Type::Text(event_id),
            D1Type::Text(attendee_id),
        ])
        .map_err(|e| format!("D1 try_settle_hold_credit bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 try_settle_hold_credit run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Atomically claim the "refund" settlement for a verified THB deposit.
///
/// Flips `refunded` 0→1 only when the deposit is verified, not already refunded,
/// and NOT held as credit (so a deposit converted to credit can never also be
/// cash-refunded — and vice versa, since the hold CAS requires `refunded = 0`).
/// Returns `Ok(true)` iff THIS call performed the flip. Mutually exclusive with
/// `try_settle_hold_credit` on the same row, so hold and refund can't both win.
pub async fn try_settle_refund(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
    refunded_at: &str,
    refund_proof_url: &str,
) -> Result<bool, String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET refunded = 1, refunded_at = ?1, refund_proof_url = ?2 \
         WHERE event_id = ?3 AND attendee_id = ?4 \
           AND verified = 1 AND refunded = 0 AND held_as_credit = 0",
    );
    let result = stmt
        .bind_refs(&[
            D1Type::Text(refunded_at),
            D1Type::Text(refund_proof_url),
            D1Type::Text(event_id),
            D1Type::Text(attendee_id),
        ])
        .map_err(|e| format!("D1 try_settle_refund bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 try_settle_refund run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Rewrite `refund_proof_url` in place, without touching any other column.
///
/// The only writer of a settlement column outside the CAS pair. It exists for
/// the `data:` URL → R2 migration (`handlers/deposit/thb/handlers/mod.rs`),
/// which replaces a multi-MB inline base64 blob with a compact serving path for
/// the *same* proof. It cannot retract a settlement: it never writes `refunded`.
pub async fn set_refund_proof_url(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
    refund_proof_url: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET refund_proof_url = ?1 \
         WHERE event_id = ?2 AND attendee_id = ?3",
    );
    stmt.bind_refs(&[
        D1Type::Text(refund_proof_url),
        D1Type::Text(event_id),
        D1Type::Text(attendee_id),
    ])
    .map_err(|e| format!("D1 set_refund_proof_url bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 set_refund_proof_url run: {e:?}"))?;

    Ok(())
}

/// Delete all THB deposits for an event (cleanup).
/// How much of an event's deposits the archive covers, as of right now.
///
/// `unarchived` is the one the delete turns on, and it is counted by matching
/// ids — not by comparing totals. An earlier version compared
/// `archived >= live`, which is a different question: it asks whether the
/// archive is *big enough*, and passes whenever `archived` is inflated by rows
/// that correspond to nothing currently live (0044-era rows carrying a NULL
/// `source_deposit_id`, or rows kept from an earlier purge of the same event
/// that has since taken new deposits). It happened to be safe — the
/// `INSERT … SELECT` in [`archive_thb_deposits_for_event`] is what guarantees
/// every live id is archived, and the gate was only a backstop — but a check
/// whose stated meaning is not the one it computes rots the moment someone
/// changes what else the archive holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveCoverage {
    /// Rows the archive holds for this event. Reported, not decided on.
    pub archived: i64,
    /// Live deposits with no archive row carrying their id. **Must be 0 before
    /// anything is deleted.**
    pub unarchived: i64,
}

impl ArchiveCoverage {
    /// Every live deposit has an archived counterpart, so deleting loses nothing.
    ///
    /// Naturally safe to retry: an event already purged has no live rows, so
    /// nothing can be unmatched.
    pub fn is_complete(&self) -> bool {
        self.unarchived == 0
    }
}

/// Copy an event's deposits into `thb_deposit_archive`, keeping the money and
/// dropping the people, and report how much of the event the archive now covers.
///
/// This is the half of the retention policy that was missing. The nightly cron
/// deletes `thb_deposits` 90 days after the refund window closes — correct for
/// `bank_account`, `attendee_name` and the slip URL, and destructive for the
/// amounts, which are the only record that the deposits balanced. RTM#3's 14
/// rows (7,000 THB) went that way on 2026-09-19, one of them neither refunded
/// nor held (`.issues/126`).
///
/// **Idempotent per source row**, not per attendee: `ON CONFLICT
/// (source_deposit_id) DO NOTHING`. `thb_deposits` has no UNIQUE on
/// `(event_id, attendee_id)` and its save path is check-then-act, so one
/// attendee really can hold two deposit rows — and keying on the pair archived
/// one of them and deleted both (`.issues/127`). The money is per row, so the
/// key is per row.
///
/// `event_slug` is denormalised from `events` because the archive outlives the
/// event row. It falls back to the id, which for a duplicated event is the slug
/// of its source event (`.issues/079`) — wrong-looking but never empty.
pub async fn archive_thb_deposits_for_event(
    db: &D1Database,
    event_id: &str,
) -> Result<ArchiveCoverage, String> {
    let insert = db
        .prepare(
            "INSERT INTO thb_deposit_archive \
             (source_deposit_id, event_id, event_slug, attendee_id, amount_thb, verified, \
              verified_at, uploaded_at, refunded, refunded_at, held_as_credit, \
              held_as_credit_at, had_slip, had_refund_proof) \
             SELECT d.id, d.event_id, \
                    COALESCE((SELECT e.slug FROM events e WHERE e.id = d.event_id), d.event_id), \
                    d.attendee_id, d.amount_thb, d.verified, d.verified_at, \
                    COALESCE(d.uploaded_at, ''), d.refunded, d.refunded_at, \
                    COALESCE(d.held_as_credit, 0), d.held_as_credit_at, \
                    CASE WHEN COALESCE(d.slip_url, '') <> '' THEN 1 ELSE 0 END, \
                    CASE WHEN COALESCE(d.refund_proof_url, '') <> '' THEN 1 ELSE 0 END \
             FROM thb_deposits d WHERE d.event_id = ?1 \
             ON CONFLICT (source_deposit_id) DO NOTHING",
        )
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 archive_thb_deposits bind: {e:?}"))?;
    insert
        .run()
        .await
        .map_err(|e| format!("D1 archive_thb_deposits run: {e:?}"))?;

    let stmt = db
        .prepare(
            "SELECT (SELECT COUNT(*) FROM thb_deposit_archive WHERE event_id = ?1) AS archived, \
                    (SELECT COUNT(*) FROM thb_deposits d WHERE d.event_id = ?1 \
                       AND NOT EXISTS (SELECT 1 FROM thb_deposit_archive a \
                                       WHERE a.source_deposit_id = d.id)) AS unarchived",
        )
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 archive_thb_deposits count bind: {e:?}"))?;
    let rows = super::d1_safe::safe_all_rows(&stmt).await?;
    let row = rows
        .first()
        .ok_or_else(|| "D1 archive_thb_deposits count returned no row".to_string())?;
    let read = |key: &str| -> Result<i64, String> {
        row.get(key)
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| format!("D1 archive_thb_deposits count missing `{key}`"))
    };
    Ok(ArchiveCoverage {
        archived: read("archived")?,
        unarchived: read("unarchived")?,
    })
}

pub async fn delete_thb_deposits_for_event(db: &D1Database, event_id: &str) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM thb_deposits WHERE event_id = ?1");
    stmt.bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 delete_thb_deposits bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_thb_deposits run: {e:?}"))?;

    Ok(())
}

/// Delete a single THB deposit by event + attendee (attendee deletion).
pub async fn delete_thb_deposit(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM thb_deposits WHERE event_id = ?1 AND attendee_id = ?2");
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 delete_thb_deposit bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_thb_deposit run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Row → Domain conversion
// ---------------------------------------------------------------------------

fn row_to_thb_deposit(row: serde_json::Value) -> Result<ThbDeposit, String> {
    let get_str = |field: &str| -> String {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_opt_str = |field: &str| -> Option<String> {
        row.get(field)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    let get_bool =
        |field: &str| -> bool { row.get(field).and_then(|v| v.as_i64()).unwrap_or(0) != 0 };

    Ok(ThbDeposit {
        attendee_id: get_str("attendee_id"),
        event_id: get_str("event_id"),
        amount_thb: row.get("amount_thb").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
        slip_url: get_opt_str("slip_url"),
        verified: get_bool("verified"),
        verified_by: get_opt_str("verified_by"),
        verified_at: get_opt_str("verified_at"),
        uploaded_at: get_str("uploaded_at"),
        refunded: get_bool("refunded"),
        refunded_at: get_opt_str("refunded_at"),
        held_as_credit: get_bool("held_as_credit"),
        held_as_credit_at: get_opt_str("held_as_credit_at"),
        attendee_name: get_opt_str("attendee_name"),
        bank_account: get_opt_str("bank_account"),
        bank_name: get_opt_str("bank_name"),
        account_name: get_opt_str("account_name"),
        refund_proof_url: get_opt_str("refund_proof_url"),
        // Absent on every row uploaded before migration 0046, and `get_opt_str`
        // maps both SQL NULL and '' to None. That is the intended reading:
        // "not known", never "not a duplicate".
        slip_blake3: get_opt_str("slip_blake3"),
        // NULL / '' means "never recorded", and `source()` then falls back to
        // the legacy sentinels. An unrecognised string is treated the same way
        // rather than panicking: the CHECK constraint already makes one
        // impossible, and a read path is the wrong place to discover it.
        deposit_source: get_opt_str("deposit_source").and_then(|s| match s.as_str() {
            "cash" => Some(DepositSource::Cash),
            "credit" => Some(DepositSource::Credit),
            "comp" => Some(DepositSource::Comp),
            _ => None,
        }),
    })
}

/// The wire/DB spelling of a [`DepositSource`] — the same three strings the
/// `deposit_source` CHECK constraint allows.
/// Bind `deposit_source` for D1.
///
/// **Returns `D1Type::Null` for `None`, NOT an empty string**, and that is the
/// whole point of this function existing (`.issues/138`).
///
/// Every other optional column in this module binds `""` for absent, because
/// they are plain `TEXT` with no constraint. `deposit_source` is different:
/// migration 0047 gave it
/// `CHECK (deposit_source IS NULL OR deposit_source IN ('cash','credit','comp'))`,
/// and `''` is neither NULL nor a member of that list. Binding the module's
/// usual empty string therefore **fails the CHECK and aborts the whole
/// statement** — which is exactly what happened in production on 2026-09-23:
/// every attendee slip upload sets `deposit_source: None`
/// (`slip_upload.rs`), so every upload returned
/// `500 internal error` from the moment 0047 was applied.
///
/// `D1Type::Null` binds correctly on worker 0.8.1 — `db/audit.rs`,
/// `db/credit_ledger.rs` and `db/attendees/writes.rs` all rely on it.
fn deposit_source_bind(source: Option<DepositSource>) -> D1Type<'static> {
    match source {
        Some(s) => D1Type::Text(s.as_str()),
        None => D1Type::Null,
    }
}
