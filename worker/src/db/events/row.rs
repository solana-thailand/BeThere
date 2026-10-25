//! D1 event row: the full `events` row and its conversion to `EventConfig`.

// ---------------------------------------------------------------------------
// D1 row type matching the full events table
// ---------------------------------------------------------------------------

/// Full event row from D1, used for reconstructing EventConfig when KV is empty.
///
/// Uses `#[serde(default)]` so that columns added by future migrations deserialize as
/// `None` / zero values even when the migration hasn't been applied to production yet.
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct D1EventRow {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub status: Option<String>,
    pub event_format: Option<String>,
    pub event_start_ms: Option<i64>,
    pub event_end_ms: Option<i64>,
    pub deposit_enabled: Option<i64>,
    pub deposit_amount_usdc: Option<i64>,
    pub deposit_amount_thb: Option<i64>,
    pub escrow_status: Option<String>,
    pub escrow_pda: Option<String>,
    pub location: Option<String>,
    pub tagline: Option<String>,
    pub organizer_emails: Option<String>,
    pub organization_id: Option<String>,
    pub video_url: Option<String>,
    pub sheet_id: Option<String>,
    pub sheet_name: Option<String>,
    pub staff_sheet_name: Option<String>,
    pub capacity: Option<i64>,
    pub total_attendees: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Columns added by migration 0003
    pub link: Option<String>,
    pub time_tba: Option<i64>,
    pub quiz_enabled: Option<i64>,
    pub nft_collection_mint: Option<String>,
    pub nft_metadata_uri: Option<String>,
    pub nft_image_url: Option<String>,
    pub nft_name_template: Option<String>,
    pub nft_symbol: Option<String>,
    pub nft_description_template: Option<String>,
    pub merkle_tree: Option<String>,
    pub staff_emails: Option<String>,
    pub claim_base_url: Option<String>,
    pub promptpay_id: Option<String>,
    pub escrow_address: Option<String>,
    pub organizer_wallet: Option<String>,
    // Legacy numeric column. **Do not read this for PDA derivation** — it
    // arrives via `JSON.stringify`, so any value above 2^53 is already rounded
    // by the time serde sees it, and a value above i64::MAX was stored as a
    // SQLite REAL and lost its low digits on the way in (Issue 085).
    //
    // The stale example in the old comment here — 15159210065911600000 — was
    // itself the corrupted form of a real event's id, which is how long this
    // went unnoticed.
    pub on_chain_event_id: Option<u64>,
    /// Exact u64, stored and read as TEXT (migration 0034). This is the value
    /// `to_event_config` trusts; `on_chain_event_id` above is kept only so old
    /// rows still parse before the repair script has run.
    #[serde(default)]
    pub on_chain_event_id_text: Option<String>,
    pub refund_deadline_hours: Option<i64>,
    pub max_refundable_deposits: Option<i64>,
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub require_contact_info: Option<i64>,
    pub require_photo_consent: Option<i64>,
    pub in_person_capacity: Option<i64>,
    pub online_capacity: Option<i64>,
    pub online_open_mode: Option<String>,
    pub online_registration_open: Option<i64>,
    pub deposit_deadline_hours: Option<i64>,
    pub updated_by: Option<String>,
    // Columns added for Issue #049
    pub dev_profile_enabled: Option<i64>,
    // Columns added for community links
    pub community_links: Option<String>,
    // Columns added by migration 0049 (ticket announcements)
    pub ticket_note_in_person: Option<String>,
    pub ticket_note_online: Option<String>,
    // Column added by migration 0053 (postponed notice)
    pub postponed_note: Option<String>,
    // Column added by migration 0057 (sponsors, JSON array)
    pub sponsors_json: Option<String>,
    // Columns added for Issue #053 Phase 3f
    pub form_config: Option<String>,
    // Columns added for organization calendar subscribe
    pub calendar_subscribe_url: Option<String>,
    // Columns added by migration 0041 (venue map link)
    pub location_map_url: Option<String>,
    // Columns added by migration 0019 (event poster)
    pub poster_url: Option<String>,
    // Columns added by migration 0020 (Plan 008 — denormalized recap flag).
    // Canonical source is `event_summaries.recap_published_at`; this flag is
    // duplicated onto the events row so `GET /api/public/events/past` can
    // filter without a join.
    pub recap_published: Option<i64>,
    // Columns added by migration 0020 (Plan 008 — Phase 3 post-event registration).
    pub post_event_registration_open: Option<i64>,
    pub post_event_registration_until_ms: Option<i64>,
}

/// Parse an enum column stored as its `as_str()` snake_case spelling.
///
/// Three cases, deliberately kept distinct — the previous
/// `from_value(...).unwrap_or_default()` collapsed them into one and logged
/// nothing, so a corrupt column was indistinguishable from a fresh one:
///
/// - **absent or empty** — `NULL` on a row written before the column existed.
///   Returns `legacy`, the value that row semantically had.
/// - **parses** — use it. Surrounding whitespace is tolerated.
/// - **present but unrecognised** — every writer goes through `as_str()`, so
///   this is only reachable through an out-of-band D1 edit or a migration
///   default. Returns `corrupt`, chosen per column to fail closed, and warns:
///   the fallback is a guess, and the row needs a human.
pub(super) fn parse_enum_column<T: serde::de::DeserializeOwned>(
    event_id: &str,
    column: &str,
    stored: Option<&str>,
    legacy: T,
    corrupt: T,
) -> T {
    let raw = stored.unwrap_or("").trim();
    match raw.is_empty() {
        true => legacy,
        false => match serde_json::from_value(serde_json::Value::String(raw.to_string())) {
            Ok(parsed) => parsed,
            Err(_) => {
                tracing::warn!(
                    event_id = %event_id,
                    column = %column,
                    value = %raw,
                    "unrecognised enum value in D1 — falling back to the fail-closed variant"
                );
                corrupt
            }
        },
    }
}

impl D1EventRow {
    /// The on-chain event id, taken from the exact TEXT column (Issue 085).
    ///
    /// The escrow PDA is derived from this value, so a rounded one silently
    /// addresses an account that does not exist. The numeric column cannot be
    /// trusted: it reaches Rust through `JSON.stringify`, which rounds anything
    /// above 2^53, and a value above `i64::MAX` was already destroyed on write.
    ///
    /// Falling back to the numeric column keeps un-repaired rows working
    /// exactly as badly as before rather than breaking them further, but a
    /// value that cannot be exact is flagged so the corruption is visible in
    /// the log instead of only in a failed transaction simulation.
    fn exact_on_chain_event_id(&self) -> u64 {
        if let Some(text) = self.on_chain_event_id_text.as_deref()
            && !text.is_empty()
            && let Ok(parsed) = text.parse::<u64>()
        {
            return parsed;
        }

        let numeric = self.on_chain_event_id.unwrap_or(0);
        // 2^53 — above this, `JSON.stringify` has already rounded the value.
        if numeric > 9_007_199_254_740_992 {
            tracing::warn!(
                event_id = %self.id.as_deref().unwrap_or("?"),
                "on_chain_event_id read from the legacy numeric column and is \
                 above 2^53, so it is rounded and the derived escrow PDA will \
                 be wrong — run scripts/verify/onchain_event_id_audit.py (085)"
            );
        }
        numeric
    }

    /// Convert D1 row to domain EventConfig.
    /// Missing columns fall back to their pre-migration value via
    /// [`parse_enum_column`] / `unwrap_or_default`, so a row written before a
    /// migration ran still converts.
    pub fn to_event_config(&self) -> event_checkin_domain::models::event::EventConfig {
        use event_checkin_domain::models::event::*;

        let event_id = self.id.as_deref().unwrap_or("<unknown>");
        let status = parse_enum_column(
            event_id,
            "status",
            self.status.as_deref(),
            EventStatus::Draft,
            // A corrupt status keeps the event out of every listing.
            EventStatus::Draft,
        );
        let escrow_status = parse_enum_column(
            event_id,
            "escrow_status",
            self.escrow_status.as_deref(),
            EscrowStatus::None,
            // Treat a corrupt escrow status as live: `is_active()` then blocks
            // archiving, deleting and repointing an escrow that may still hold
            // funds. Degrading to `None` would unlock all three.
            EscrowStatus::Initialized,
        );
        let event_format = parse_enum_column(
            event_id,
            "event_format",
            self.event_format.as_deref(),
            EventFormat::InPerson,
            EventFormat::InPerson,
        );
        let visibility = parse_enum_column(
            event_id,
            "visibility",
            self.visibility.as_deref(),
            // NULL predates migration 0016; those events were all public.
            EventVisibility::Public,
            // A corrupt visibility must not publish a private event.
            EventVisibility::Private,
        );
        let online_open_mode = parse_enum_column(
            event_id,
            "online_open_mode",
            self.online_open_mode.as_deref(),
            OnlineOpenMode::AutoOnFull,
            // A corrupt mode must not fling online registration open; `Manual`
            // waits for the organizer.
            OnlineOpenMode::Manual,
        );

        let organizer_emails: Vec<String> = self
            .organizer_emails
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let staff_emails: Vec<String> = self
            .staff_emails
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        EventConfig {
            // D1 does not carry the waived-email list; KV is its home. An empty
            // vec here means "unknown from this source", never "nobody is waived".
            // The event resolvers restore it from KV by id
            // (`event_store::read::with_kv_only_fields`, `.issues/139`).
            comp_emails: Vec::new(),
            id: self.id.clone().unwrap_or_default(),
            name: self.name.clone().unwrap_or_default(),
            slug: self.slug.clone().unwrap_or_default(),
            tagline: self.tagline.clone().unwrap_or_default(),
            link: self.link.clone().unwrap_or_default(),
            status,
            event_start_ms: self.event_start_ms.unwrap_or(0),
            event_end_ms: self.event_end_ms.unwrap_or(0),
            time_tba: self.time_tba.unwrap_or(0) == 1,
            sheet_id: self.sheet_id.clone().unwrap_or_default(),
            sheet_name: self.sheet_name.clone().unwrap_or_default(),
            staff_sheet_name: self.staff_sheet_name.clone().unwrap_or_default(),
            quiz_enabled: self.quiz_enabled.unwrap_or(0) == 1,
            nft_collection_mint: self.nft_collection_mint.clone().unwrap_or_default(),
            nft_metadata_uri: self.nft_metadata_uri.clone().unwrap_or_default(),
            nft_image_url: self.nft_image_url.clone().unwrap_or_default(),
            nft_name_template: self.nft_name_template.clone().unwrap_or_default(),
            nft_symbol: self.nft_symbol.clone().unwrap_or_default(),
            nft_description_template: self.nft_description_template.clone().unwrap_or_default(),
            merkle_tree: self.merkle_tree.clone().unwrap_or_default(),
            organization_id: self.organization_id.clone().unwrap_or_default(),
            organizer_emails,
            staff_emails,
            claim_base_url: self.claim_base_url.clone().unwrap_or_default(),
            deposit_enabled: self.deposit_enabled.unwrap_or(0) != 0,
            deposit_amount_usdc: self.deposit_amount_usdc.unwrap_or(0) as u64,
            deposit_amount_thb: self.deposit_amount_thb.unwrap_or(0) as u64,
            promptpay_id: self.promptpay_id.clone().unwrap_or_default(),
            escrow_address: self.escrow_address.clone().unwrap_or_default(),
            escrow_status,
            organizer_wallet: self.organizer_wallet.clone().unwrap_or_default(),
            on_chain_event_id: self.exact_on_chain_event_id(),
            refund_deadline_hours: self.refund_deadline_hours.unwrap_or(168) as u32,
            max_refundable_deposits: self.max_refundable_deposits.unwrap_or(0) as u32,
            description: self.description.clone().unwrap_or_default(),
            location: self.location.clone().unwrap_or_default(),
            video_url: self.video_url.clone().unwrap_or_default(),
            event_format,
            require_contact_info: self.require_contact_info.unwrap_or(1) == 1,
            require_photo_consent: self.require_photo_consent.unwrap_or(0) == 1,
            in_person_capacity: self
                .in_person_capacity
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
            online_capacity: self
                .online_capacity
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
            online_open_mode,
            online_registration_open: self.online_registration_open.unwrap_or(0) == 1,
            deposit_deadline_hours: self
                .deposit_deadline_hours
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
            visibility,
            created_at: self.created_at.clone().unwrap_or_default(),
            updated_at: self.updated_at.clone().unwrap_or_default(),
            updated_by: self.updated_by.clone().unwrap_or_default(),
            dev_profile_enabled: self.dev_profile_enabled.unwrap_or(0) == 1,
            community_links: serde_json::from_str(
                &self
                    .community_links
                    .clone()
                    .unwrap_or_else(|| "[]".to_string()),
            )
            .unwrap_or_default(),
            ticket_note_in_person: self.ticket_note_in_person.clone().unwrap_or_default(),
            ticket_note_online: self.ticket_note_online.clone().unwrap_or_default(),
            postponed_note: self.postponed_note.clone().unwrap_or_default(),
            sponsors: self
                .sponsors_json
                .as_deref()
                .and_then(|j| serde_json::from_str(j).ok())
                .unwrap_or_default(),
            calendar_subscribe_url: self.calendar_subscribe_url.clone().unwrap_or_default(),
            location_map_url: self.location_map_url.clone().unwrap_or_default(),
            poster_url: self.poster_url.clone().unwrap_or_default(),
            recap_published: self.recap_published.unwrap_or(0) != 0,
            post_event_registration_open: self.post_event_registration_open.unwrap_or(0) != 0,
            post_event_registration_until_ms: self.post_event_registration_until_ms,
        }
    }
}
