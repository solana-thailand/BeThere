//! KV-based event storage for multi-event / organizer support (Issue 004).
//!
//! Events are stored in a Cloudflare KV namespace bound as `EVENTS`:
//!
//!   "events"                         → EventIndex (JSON) — list of EventMeta
//!   "event:{id}"                     → EventConfig (JSON) — full per-event config
//!
//! Per-event quiz data uses the same namespace with prefixed keys:
//!   "event:{id}:quiz:questions"      → QuizConfig (JSON)
//!   "event:{id}:quiz:progress:{tok}" → QuizProgress (JSON)

// Phase 2 helpers used when event-scoping existing handlers.
use worker::KvStore;

use event_checkin_domain::models::event::{
    CreateEventRequest, EventConfig, EventIndex, EventMeta, EventStatus, UpdateEventRequest,
};

// ---------------------------------------------------------------------------
// Event index (list of all events)
// ---------------------------------------------------------------------------

/// Read the event index from KV.
/// Returns an empty index if the key doesn't exist yet (first run).
pub async fn get_event_index(kv: &KvStore) -> Result<EventIndex, String> {
    let raw: Option<String> = kv
        .get("events")
        .text()
        .await
        .map_err(|e| format!("failed to read event index from KV: {e:?}"))?;

    match raw {
        None => Ok(EventIndex::default()),
        Some(json_str) => {
            serde_json::from_str(&json_str).map_err(|e| format!("failed to parse event index: {e}"))
        }
    }
}

/// Write the event index to KV.
async fn save_event_index(kv: &KvStore, index: &EventIndex) -> Result<(), String> {
    let json_str = serde_json::to_string(index)
        .map_err(|e| format!("failed to serialize event index: {e:?}"))?;
    kv.put("events", &json_str)
        .map_err(|e| format!("failed to build event index put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event index to KV: {e:?}"))
}

// ---------------------------------------------------------------------------
// Per-event config
// ---------------------------------------------------------------------------

/// KV key for a specific event's full configuration.
fn event_config_key(id: &str) -> String {
    format!("event:{id}")
}

/// Read a single event's full configuration.
/// Returns `None` if the event ID doesn't exist.
pub async fn get_event_config(kv: &KvStore, id: &str) -> Result<Option<EventConfig>, String> {
    let key = event_config_key(id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read event config '{id}' from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse event config '{id}': {e}")),
    }
}

/// Write a single event's full configuration.
async fn save_event_config(kv: &KvStore, config: &EventConfig) -> Result<(), String> {
    let key = event_config_key(&config.id);
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize event config: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build event config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event config to KV: {e:?}"))
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// List all events (metadata only, no full config).
/// Returns events sorted by creation date (newest first).
pub async fn list_events(kv: &KvStore) -> Result<Vec<EventMeta>, String> {
    let index = get_event_index(kv).await?;
    Ok(index.events)
}

/// Get a single event's full configuration by ID.
pub async fn get_event(kv: &KvStore, id: &str) -> Result<Option<EventConfig>, String> {
    get_event_config(kv, id).await
}

/// Create a new event.
///
/// Generates a unique ID from the slug, validates required fields,
/// saves the full config, and updates the event index.
pub async fn create_event(kv: &KvStore, req: &CreateEventRequest) -> Result<EventConfig, String> {
    // Validate required fields
    if req.name.trim().is_empty() {
        return Err("event name is required".to_string());
    }
    if req.sheet_id.trim().is_empty() {
        return Err("google sheet_id is required".to_string());
    }
    if req.event_start_ms <= 0 {
        return Err("event_start_ms must be a positive Unix epoch millisecond".to_string());
    }
    if req.event_end_ms <= req.event_start_ms {
        return Err("event_end_ms must be after event_start_ms".to_string());
    }

    // Generate slug from name if not provided
    let slug = if req.slug.trim().is_empty() {
        slugify(&req.name)
    } else {
        slugify(&req.slug)
    };

    // Generate ID from slug
    let id = slug.clone();

    // Check for duplicate ID
    let index = get_event_index(kv).await?;
    if index.events.iter().any(|e| e.id == id) {
        return Err(format!(
            "event with id '{id}' already exists — use a different name or slug"
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();

    let config = EventConfig {
        id: id.clone(),
        name: req.name.trim().to_string(),
        slug: slug.clone(),
        tagline: req.tagline.trim().to_string(),
        link: req.link.trim().to_string(),
        status: EventStatus::Draft,
        event_start_ms: req.event_start_ms,
        event_end_ms: req.event_end_ms,
        sheet_id: req.sheet_id.trim().to_string(),
        sheet_name: if req.sheet_name.is_empty() {
            "checkin".to_string()
        } else {
            req.sheet_name.clone()
        },
        staff_sheet_name: if req.staff_sheet_name.is_empty() {
            "staff".to_string()
        } else {
            req.staff_sheet_name.clone()
        },
        quiz_enabled: req.quiz_enabled,
        nft_collection_mint: req.nft_collection_mint.trim().to_string(),
        nft_metadata_uri: req.nft_metadata_uri.trim().to_string(),
        nft_image_url: req.nft_image_url.trim().to_string(),
        nft_name_template: req.nft_name_template.trim().to_string(),
        nft_symbol: req.nft_symbol.trim().to_string(),
        nft_description_template: req.nft_description_template.trim().to_string(),
        organizer_emails: req
            .organizer_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect(),
        staff_emails: req
            .staff_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect(),
        claim_base_url: req.claim_base_url.trim().to_string(),
        merkle_tree: req.merkle_tree.trim().to_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    // Save full config
    save_event_config(kv, &config).await?;

    // Update index
    let mut index = index;
    index.events.insert(0, config.to_meta());
    save_event_index(kv, &index).await?;

    tracing::info!(
        "event created: id={id} name='{}' sheet_id='{}'",
        config.name,
        config.sheet_id,
    );

    Ok(config)
}

/// Update an existing event's configuration.
///
/// Only provided (non-None) fields are updated.
/// Returns the updated EventConfig.
pub async fn update_event(
    kv: &KvStore,
    id: &str,
    req: &UpdateEventRequest,
) -> Result<EventConfig, String> {
    let mut config = get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    // Apply partial updates
    if let Some(ref name) = req.name {
        config.name = name.trim().to_string();
    }
    if let Some(ref slug) = req.slug {
        config.slug = slugify(slug);
    }
    if let Some(ref tagline) = req.tagline {
        config.tagline = tagline.trim().to_string();
    }
    if let Some(ref link) = req.link {
        config.link = link.trim().to_string();
    }
    if let Some(ref status) = req.status {
        config.status = status.clone();
    }
    if let Some(ms) = req.event_start_ms {
        if ms <= 0 {
            return Err("event_start_ms must be positive".to_string());
        }
        config.event_start_ms = ms;
    }
    if let Some(ms) = req.event_end_ms {
        if ms <= config.event_start_ms {
            return Err("event_end_ms must be after event_start_ms".to_string());
        }
        config.event_end_ms = ms;
    }
    if let Some(ref sheet_id) = req.sheet_id {
        if sheet_id.trim().is_empty() {
            return Err("sheet_id cannot be empty".to_string());
        }
        config.sheet_id = sheet_id.trim().to_string();
    }
    if let Some(ref sheet_name) = req.sheet_name {
        config.sheet_name = sheet_name.clone();
    }
    if let Some(ref staff_sheet_name) = req.staff_sheet_name {
        config.staff_sheet_name = staff_sheet_name.clone();
    }
    if let Some(enabled) = req.quiz_enabled {
        config.quiz_enabled = enabled;
    }
    if let Some(ref v) = req.nft_collection_mint {
        config.nft_collection_mint = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_metadata_uri {
        config.nft_metadata_uri = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_image_url {
        config.nft_image_url = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_name_template {
        config.nft_name_template = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_symbol {
        config.nft_symbol = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_description_template {
        config.nft_description_template = v.trim().to_string();
    }
    if let Some(ref emails) = req.organizer_emails {
        config.organizer_emails = emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
    }
    if let Some(ref emails) = req.staff_emails {
        config.staff_emails = emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
    }
    if let Some(ref url) = req.claim_base_url {
        config.claim_base_url = url.trim().to_string();
    }
    if let Some(ref v) = req.merkle_tree {
        config.merkle_tree = v.trim().to_string();
    }

    config.updated_at = chrono::Utc::now().to_rfc3339();

    // Save updated config
    save_event_config(kv, &config).await?;

    // Update index entry
    let mut index = get_event_index(kv).await?;
    if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
        *entry = config.to_meta();
    }
    save_event_index(kv, &index).await?;

    tracing::info!("event updated: id={id}");

    Ok(config)
}

/// Archive (soft-delete) an event by setting its status to Archived.
pub async fn archive_event(kv: &KvStore, id: &str) -> Result<(), String> {
    let mut config = get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    config.status = EventStatus::Archived;
    config.updated_at = chrono::Utc::now().to_rfc3339();

    save_event_config(kv, &config).await?;

    // Update index
    let mut index = get_event_index(kv).await?;
    if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
        entry.status = EventStatus::Archived;
    }
    save_event_index(kv, &index).await?;

    tracing::info!("event archived: id={id}");

    Ok(())
}

// ---------------------------------------------------------------------------
// Seed from global config
// ---------------------------------------------------------------------------

/// Seed the first event from global AppConfig (env vars).
///
/// Idempotent: if the event index already has an active event, returns it.
/// Otherwise builds an EventConfig with id="default" and status=Active,
/// saves it to KV, and updates the index.
pub async fn seed_from_config(
    kv: &KvStore,
    global: &event_checkin_domain::config::AppConfig,
    state: &crate::state::AppState,
) -> Result<EventConfig, String> {
    // Idempotent: return existing active event if any
    let index = get_event_index(kv).await?;
    if let Some(meta) = index
        .events
        .iter()
        .find(|e| e.status == EventStatus::Active)
        && let Some(config) = get_event_config(kv, &meta.id).await?
    {
        tracing::info!("seed: already have active event id={}", config.id);
        return Ok(config);
    }

    let now = chrono::Utc::now().to_rfc3339();

    let config = EventConfig {
        id: "default".to_string(),
        name: global.event_name.clone(),
        slug: "default".to_string(),
        tagline: global.event_tagline.clone(),
        link: global.event_link.clone(),
        status: EventStatus::Active,
        event_start_ms: global.event_start_ms,
        event_end_ms: global.event_end_ms,
        sheet_id: global.sheets.sheet_id.clone(),
        sheet_name: global.sheets.sheet_name.clone(),
        staff_sheet_name: global.sheets.staff_sheet_name.clone(),
        quiz_enabled: true,
        nft_collection_mint: global.nft_collection_mint.clone(),
        nft_metadata_uri: global.nft_metadata_uri.clone(),
        nft_image_url: global.nft_image_url.clone(),
        nft_name_template: "BeThere - {event_name}".to_string(),
        nft_symbol: "BETHERE".to_string(),
        nft_description_template: "Proof of attendance at {event_name}".to_string(),
        organizer_emails: {
            let mut emails = global.super_admin_emails.clone();
            // Merge organizers from Google Sheet staff tab (role "admin" or "organizer")
            if let Ok(members) = crate::sheets::get_staff_members(
                state,
                &global.sheets.sheet_id,
                &global.sheets.staff_sheet_name,
            )
            .await
            {
                for m in &members {
                    if matches!(m.role.as_str(), "admin" | "organizer")
                        && !emails.iter().any(|e| e.eq_ignore_ascii_case(&m.email))
                    {
                        emails.push(m.email.clone());
                    }
                }
            }
            emails
        },
        staff_emails: {
            let mut emails = global.staff_emails.clone();
            // Merge staff from Google Sheet staff tab (all members)
            if let Ok(members) = crate::sheets::get_staff_members(
                state,
                &global.sheets.sheet_id,
                &global.sheets.staff_sheet_name,
            )
            .await
            {
                for m in &members {
                    if !emails.iter().any(|e| e.eq_ignore_ascii_case(&m.email)) {
                        emails.push(m.email.clone());
                    }
                }
            }
            emails
        },
        claim_base_url: global.claim_base_url.clone(),
        merkle_tree: String::new(), // not in global config — per-event only
        created_at: now.clone(),
        updated_at: now,
    };

    // Save full config
    save_event_config(kv, &config).await?;

    // Update index
    let mut index = index;
    index.events.insert(0, config.to_meta());
    save_event_index(kv, &index).await?;

    tracing::info!(
        "seed: created default event from config name='{}'",
        config.name,
    );

    Ok(config)
}

// ---------------------------------------------------------------------------
// Event resolution helpers
// ---------------------------------------------------------------------------

/// Find the first active event.
///
/// Used for backward compatibility: legacy API routes that don't specify
/// an event_id resolve to the first active event.
pub async fn get_active_event(kv: &KvStore) -> Result<Option<EventConfig>, String> {
    let index = get_event_index(kv).await?;
    for meta in &index.events {
        if meta.status == EventStatus::Active
            && let Some(config) = get_event_config(kv, &meta.id).await?
        {
            return Ok(Some(config));
        }
    }
    Ok(None)
}

/// Resolve an event ID to its full configuration.
///
/// Falls back to the first active event if `event_id` is empty or "default".
/// Returns an error if no matching event is found.
pub async fn resolve_event(kv: &KvStore, event_id: Option<&str>) -> Result<EventConfig, String> {
    match event_id {
        Some(id) if !id.is_empty() && id != "default" => get_event_config(kv, id)
            .await?
            .ok_or_else(|| format!("event '{id}' not found")),
        _ => {
            // Fall back to first active event
            get_active_event(kv)
                .await?
                .ok_or_else(|| "no active event found — create an event first".to_string())
        }
    }
}

/// Check if a user is an organizer for a specific event.
pub fn is_event_organizer(config: &EventConfig, email: &str) -> bool {
    config
        .organizer_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
}

/// Check if a user is staff for a specific event.
pub fn is_event_staff(config: &EventConfig, email: &str) -> bool {
    config
        .staff_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
}

/// Check if a user has any access (organizer or staff) to a specific event.
pub fn has_event_access(config: &EventConfig, email: &str) -> bool {
    is_event_organizer(config, email) || is_event_staff(config, email)
}

/// Build the event-scoped KV key for quiz questions.
///
/// When `event_id` is "default" (legacy mode), returns `"questions"` for
/// backward compatibility with the old QUIZ KV namespace.
/// Otherwise returns `"event:{id}:quiz:questions"` for the EVENTS KV namespace.
pub fn quiz_questions_key(event_id: &str) -> String {
    format!("event:{event_id}:quiz:questions")
}

/// Build the event-scoped KV key for quiz progress.
///
/// When `event_id` is "default" (legacy mode), returns `"progress:{token}"`
/// for backward compatibility with the old QUIZ KV namespace.
/// Otherwise returns `"event:{id}:quiz:progress:{token}"`.
pub fn quiz_progress_key(event_id: &str, claim_token: &str) -> String {
    format!("event:{event_id}:quiz:progress:{claim_token}")
}

/// Resolve an event, falling back to global config if EVENTS KV is not available.
///
/// This is the main entry point for handlers:
/// - If `events_kv` is `Some` → resolve event from KV (by ID or first active)
/// - If `events_kv` is `None` → build synthetic EventConfig from global env vars
pub async fn resolve_event_or_fallback(
    events_kv: Option<&KvStore>,
    event_id: Option<&str>,
    global: &event_checkin_domain::config::AppConfig,
) -> Result<EventConfig, String> {
    match events_kv {
        Some(kv) => resolve_event(kv, event_id).await,
        None => Ok(EventConfig::from_global_config(
            &global.event_name,
            &global.event_tagline,
            &global.event_link,
            global.event_start_ms,
            global.event_end_ms,
            &global.sheets.sheet_id,
            &global.sheets.sheet_name,
            &global.sheets.staff_sheet_name,
            &global.nft_collection_mint,
            &global.nft_metadata_uri,
            &global.nft_image_url,
            "",                          // nft_symbol — not in global config
            global.staff_emails.clone(), // organizer_emails — use staff_emails for legacy
            Vec::new(),                  // staff_emails
            &global.claim_base_url,
            "", // merkle_tree — not in global config
        )),
    }
}

// ---------------------------------------------------------------------------
// Quiz migration (QUIZ → EVENTS namespace)
// ---------------------------------------------------------------------------

/// Result of a quiz migration operation.
pub struct MigrationResult {
    /// `true` if data was copied, `false` if destination already existed.
    pub migrated: bool,
    /// The event ID that was the migration target.
    pub event_id: String,
    /// Human-readable status message.
    pub message: String,
}

/// Migrate quiz config from legacy QUIZ namespace to event-scoped EVENTS namespace.
/// Idempotent — skips if destination key already exists.
pub async fn migrate_quiz_to_event(
    events_kv: &KvStore,
    quiz_kv: &KvStore,
    event_id: &str,
) -> Result<MigrationResult, String> {
    let dest_key = format!("event:{event_id}:quiz:questions");

    // Idempotent: skip if destination already exists
    let existing: Option<String> = events_kv
        .get(&dest_key)
        .text()
        .await
        .map_err(|e| format!("failed to check destination key '{dest_key}': {e:?}"))?;

    match existing {
        Some(_) => {
            tracing::info!("migrate: destination '{dest_key}' already exists, skipping");
            Ok(MigrationResult {
                migrated: false,
                event_id: event_id.to_string(),
                message: format!("quiz data already migrated to event '{event_id}'"),
            })
        }
        None => {
            // Read source from legacy QUIZ namespace
            let raw: Option<String> =
                quiz_kv.get("questions").text().await.map_err(|e| {
                    format!("failed to read 'questions' from QUIZ namespace: {e:?}")
                })?;

            let source: serde_json::Value =
                serde_json::from_str(raw.as_deref().ok_or_else(|| {
                    "no quiz data found in QUIZ namespace (key 'questions' is empty)".to_string()
                })?)
                .map_err(|e| format!("failed to parse quiz data from QUIZ namespace: {e}"))?;

            // Write to EVENTS namespace
            let json_str = serde_json::to_string(&source)
                .map_err(|e| format!("failed to serialize quiz data: {e:?}"))?;
            events_kv
                .put(&dest_key, &json_str)
                .map_err(|e| format!("failed to build quiz migration put: {e:?}"))?
                .execute()
                .await
                .map_err(|e| format!("failed to write quiz data to '{dest_key}': {e:?}"))?;

            tracing::info!("migrate: copied quiz data to '{dest_key}'");
            Ok(MigrationResult {
                migrated: true,
                event_id: event_id.to_string(),
                message: format!("quiz data migrated to event '{event_id}'"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a string to a URL-friendly slug.
///
/// Lowercases, replaces non-alphanumeric runs with hyphens,
/// strips leading/trailing hyphens.
fn slugify(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
