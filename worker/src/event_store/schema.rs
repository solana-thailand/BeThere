//! KV key helpers, schema constants, and slug utilities.

// ---------------------------------------------------------------------------
// Key helpers
// ---------------------------------------------------------------------------

/// KV key for a specific event's full configuration.
pub fn event_config_key(id: &str) -> String {
    format!("event:{id}")
}

/// KV key for the escrow → event reverse index.
pub fn escrow_index_key(escrow_address: &str) -> String {
    format!("escrow:{escrow_address}")
}

/// KV key for per-attendee deposit status.
/// Pattern: `event:{id}:deposit:status:{attendee_id}`
pub fn deposit_status_key(event_id: &str, attendee_id: &str) -> String {
    format!("event:{event_id}:deposit:status:{attendee_id}")
}

/// KV key for THB deposit record.
/// Pattern: `event:{id}:deposit:thb:{attendee_id}`
pub fn thb_deposit_key(event_id: &str, attendee_id: &str) -> String {
    format!("event:{event_id}:deposit:thb:{attendee_id}")
}

/// KV key for listing all THB deposits in an event.
/// Pattern: `event:{id}:deposit:thb:list`
pub fn thb_deposit_list_key(event_id: &str) -> String {
    format!("event:{event_id}:deposit:thb:list")
}

/// KV key for the deposit counter for an event.
/// Value is a u32 counter (stored as decimal string).
pub fn deposit_counter_key(event_id: &str) -> String {
    format!("event:{event_id}:deposit:counter")
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

// ---------------------------------------------------------------------------
// Slug helpers
// ---------------------------------------------------------------------------

/// Convert a string to a URL-friendly slug.
///
/// Lowercases, replaces non-alphanumeric runs with hyphens,
/// strips leading/trailing hyphens.
pub fn slugify(input: &str) -> String {
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

/// Resolve slug collisions by appending an incrementing suffix.
///
/// If `base_slug` is not in `existing_ids`, returns it unchanged.
/// Otherwise tries `base_slug-1`, `base_slug-2`, ... until a free ID is found.
/// Both `id` and `slug` are returned (they are always equal).
pub fn deduplicate_slug(base_slug: &str, existing_ids: &[&str]) -> (String, String) {
    let existing_set: std::collections::HashSet<&str> = existing_ids.iter().copied().collect();

    if !existing_set.contains(base_slug) {
        return (base_slug.to_string(), base_slug.to_string());
    }

    for i in 1..=1000u32 {
        let candidate = format!("{base_slug}-{i}");
        if !existing_set.contains(candidate.as_str()) {
            return (candidate.clone(), candidate);
        }
    }

    // Extremely unlikely fallback — use timestamp suffix
    let fallback = format!("{base_slug}-{}", chrono::Utc::now().timestamp());
    (fallback.clone(), fallback)
}
