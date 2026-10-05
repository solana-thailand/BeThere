//! "Refund Bank Info" block shared by the admin deposit tabs (refund queue,
//! refunded, held as credit), so the three cannot drift apart.

use leptos::prelude::*;

/// The attendee's refund bank details, or `missing` as a warning badge when
/// any of the three parts is absent. The server maps empty columns to `None`.
pub fn refund_bank_info(
    bank_account: Option<String>,
    bank_name: Option<String>,
    account_name: Option<String>,
    missing: &'static str,
) -> AnyView {
    let body = match (bank_account, bank_name, account_name) {
        (Some(account), Some(bank), Some(name)) => view! {
            <div class="panel-hint">{format!("Account: {account}")}</div>
            <div class="panel-hint">{format!("Bank: {bank}")}</div>
            <div class="panel-hint">{format!("Name: {name}")}</div>
        }
        .into_any(),
        _ => view! {
            <div class="badge badge-warning admin-dep-badge-row">{missing}</div>
        }
        .into_any(),
    };
    view! {
        <div class="admin-dep-bank-section">
            <div class="panel-hint admin-dep-bank-label">"Refund Bank Info"</div>
            {body}
        </div>
    }
    .into_any()
}

/// The account number as a bank app's transfer field takes it: digits only,
/// without the dashes and spaces attendees type.
pub fn account_digits(account: &str) -> String {
    account.chars().filter(char::is_ascii_digit).collect()
}

/// Longest transfer note the organizer's bank app accepts (owner, 2026-10-05).
pub const MAX_REFUND_NOTE_CHARS: usize = 40;

/// The default transfer note, e.g. "คืนค่างาน Solana x AI Builders #6": the
/// event name shortened by [`short_event_name`], then cut to
/// [`MAX_REFUND_NOTE_CHARS`]. The organizer can edit it above the queue.
pub fn refund_note(event_name: &str) -> String {
    const PREFIX: &str = "คืนค่างาน ";
    let name = short_event_name(event_name);
    if name.is_empty() {
        return "คืนค่ามัดจำ".to_string();
    }
    let note = format!("{PREFIX}{name}");
    if note.chars().count() <= MAX_REFUND_NOTE_CHARS {
        return note;
    }
    // Too long: cut the name, never the edition marker, which goes last.
    match edition_marker(&name) {
        Some(marker) => {
            let rest = name.replacen(&marker, "", 1);
            let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
            let budget = MAX_REFUND_NOTE_CHARS
                .saturating_sub(PREFIX.chars().count() + 1 + marker.chars().count());
            format!("{PREFIX}{} {marker}", fit_chars(&rest, budget))
        }
        None => fit_chars(&note, MAX_REFUND_NOTE_CHARS),
    }
}

/// An event name short enough for a transfer note that still tells editions
/// apart. Series names put the edition near the end, e.g.
/// "Solana x AI Builders: The Road to Mainnet #6 (Bangkok)", so cutting the
/// tail loses "#6". Instead: drop trailing "(…)" groups, keep the part before
/// a ":" as the series name, and append the edition marker ("#6",
/// "Part 7", "EP 3", "ครั้งที่ 2") if the kept part lost it.
/// → "Solana x AI Builders #6".
pub fn short_event_name(name: &str) -> String {
    let mut base = name.trim();
    while base.ends_with(')') {
        match base.rfind('(') {
            Some(i) => base = base[..i].trim_end(),
            None => break,
        }
    }
    let marker = edition_marker(base);
    let head = base.split_once(':').map_or(base, |(head, _)| head).trim();
    match marker {
        Some(m) if !head.contains(&m) => format!("{head} {m}"),
        _ => head.to_string(),
    }
}

/// The last edition marker in `name`: "#6", or a number after "Part", "EP"
/// or "ครั้งที่".
fn edition_marker(name: &str) -> Option<String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    let is_number = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit());
    (0..words.len()).rev().find_map(|i| {
        let w = words[i];
        if w.len() > 1 && w.starts_with('#') && is_number(&w[1..]) {
            return Some(w.to_string());
        }
        let prev = i.checked_sub(1).map(|j| words[j]);
        match prev {
            Some(p)
                if is_number(w)
                    && matches!(
                        p.to_lowercase().trim_end_matches('.'),
                        "part" | "ep" | "ครั้งที่"
                    ) =>
            {
                Some(format!("{p} {w}"))
            }
            _ => None,
        }
    })
}

/// `text` cut to at most `max` chars. Counts Unicode scalar values, so a Thai
/// vowel or tone mark counts on its own: the result fits whether the bank app
/// counts marks or not. Never ends on a consonant whose mark was cut off.
pub fn fit_chars(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    // A mark belongs to the char before it: drop that char rather than split them.
    while end > 0 && is_thai_combining_mark(chars[end]) {
        end -= 1;
    }
    chars[..end]
        .iter()
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Thai above/below vowels and tone marks (they render on the char before).
fn is_thai_combining_mark(c: char) -> bool {
    matches!(c, '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}')
}

/// One-tap copy of the refund row's account number, amount and transfer note,
/// so a refund is copy → paste in the bank app → transfer, without retyping.
pub fn refund_copy_buttons(
    bank_account: Option<String>,
    amount_thb: u64,
    note: Signal<String>,
    set_toast: WriteSignal<Option<crate::components::ToastMessage>>,
) -> AnyView {
    use crate::components::{ToastType, show_toast};
    use crate::pages::deposit::js_interop::copy_to_clipboard;

    let digits = bank_account
        .as_deref()
        .map(account_digits)
        .unwrap_or_default();
    let copy = move |text: String, what: &'static str| {
        let (msg, kind) = match copy_to_clipboard(&text) {
            true => (format!("Copied {what}: {text}"), ToastType::Success),
            false => (format!("Could not copy {what}"), ToastType::Error),
        };
        show_toast(&set_toast, &msg, kind);
    };
    view! {
        <div class="admin-dep-confirm-row">
            {(!digits.is_empty()).then(|| {
                let digits = digits.clone();
                view! {
                    <button class="btn btn-outline btn-sm" on:click=move |_| copy(digits.clone(), "account")>
                        "Copy account"
                    </button>
                }
            })}
            <button class="btn btn-outline btn-sm" on:click=move |_| copy(amount_thb.to_string(), "amount")>
                {format!("Copy {amount_thb} THB")}
            </button>
            <button class="btn btn-outline btn-sm" title=move || note.get() on:click=move |_| copy(note.get_untracked(), "note")>
                "Copy note"
            </button>
        </div>
    }
    .into_any()
}

/// localStorage key of the organizer's edited refund note for one event.
fn note_key(event_id: &str) -> String {
    format!("bethere.refund_note.{event_id}")
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// The refund note for `event_id`: the organizer's saved edit, else the
/// default from the event name.
pub fn load_refund_note(event_id: &str, event_name: &str) -> String {
    local_storage()
        .and_then(|s| s.get_item(&note_key(event_id)).ok().flatten())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| refund_note(event_name))
}

/// Remember the organizer's edit for this event, on this device.
pub fn save_refund_note(event_id: &str, note: &str) {
    if let Some(s) = local_storage() {
        let _ = s.set_item(&note_key(event_id), note);
    }
}

/// The editable note above the refund queue, with a live x/40 count.
pub fn refund_note_editor(
    note: ReadSignal<String>,
    set_note: WriteSignal<String>,
    event_id: Signal<Option<String>>,
) -> AnyView {
    let count = move || note.with(|n| n.chars().count());
    view! {
        <div class="admin-dep-bank-section">
            <label class="form-label">"Transfer note (copied by Copy note)"</label>
            <input
                type="text"
                class="form-input dep-input"
                maxlength=MAX_REFUND_NOTE_CHARS.to_string()
                prop:value=move || note.get()
                on:input=move |ev| {
                    let value = fit_chars(&event_target_value(&ev), MAX_REFUND_NOTE_CHARS);
                    if let Some(id) = event_id.get_untracked() {
                        save_refund_note(&id, &value);
                    }
                    set_note.set(value);
                }
            />
            <div class="panel-hint">{move || format!("{} / {MAX_REFUND_NOTE_CHARS} characters", count())}</div>
        </div>
    }
    .into_any()
}
