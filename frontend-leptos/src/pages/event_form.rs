//! Event form component — shared by Create and Edit modes on the Events page.
//!
//! Contains the `EventForm` struct, all form helpers, and the `<EventFormComponent>`
//! that renders the full form UI with validation, save, and escrow init logic.

use std::sync::Arc;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::api;
use crate::components;
use crate::icons::{Icon, IconName};

// ===== Form State =====

/// Form state for creating/editing events.
#[derive(Debug, Clone, Default)]
pub struct EventForm {
    pub name: String,
    pub slug: String,
    pub tagline: String,
    pub link: String,
    pub event_start: String,
    pub event_end: String,
    pub time_tba: bool,
    pub sheet_id: String,
    pub sheet_name: String,
    pub staff_sheet_name: String,
    pub quiz_enabled: bool,
    pub nft_collection_mint: String,
    pub nft_metadata_uri: String,
    pub nft_image_url: String,
    pub poster_url: String,
    pub nft_name_template: String,
    pub nft_symbol: String,
    pub nft_description_template: String,
    pub merkle_tree: String,
    pub claim_base_url: String,
    pub organizer_emails: String,
    pub staff_emails: String,
    pub status: api::EventStatus,
    pub event_format: api::EventFormat,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: String,
    pub deposit_amount_thb: String,
    pub require_contact_info: bool,
    pub require_photo_consent: bool,
    pub promptpay_id: String,
    pub escrow_address: String,
    pub escrow_status: api::EscrowStatus,
    pub organizer_wallet: String,
    pub on_chain_event_id: String,
    pub refund_deadline_hours: String,
    pub max_refundable_deposits: String,
    pub location: String,
    pub video_url: String,
    pub in_person_capacity: String,
    pub online_capacity: String,
    pub online_open_mode: api::OnlineOpenMode,
    pub online_registration_open: bool,
    pub deposit_deadline_hours: String,
    pub visibility: api::EventVisibility,
    pub updated_at: String,
    pub community_links: Vec<crate::api::CommunityLink>,
    pub calendar_subscribe_url: String,
}

// ===== Helpers =====

/// Auto-generate a URL-safe slug from a name.
fn generate_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Parse an ISO date string or epoch ms string to epoch milliseconds.
fn parse_date_to_ms(date_str: &str) -> Option<i64> {
    if date_str.is_empty() {
        return None;
    }
    // Try parsing as epoch ms first
    if let Ok(ms) = date_str.parse::<i64>() {
        if ms > 1_000_000_000_000 {
            return Some(ms);
        }
    }
    // Parse as ISO datetime-local format (YYYY-MM-DDTHH:MM)
    let cleaned = date_str.replace('T', " ");
    let parsed = js_sys::Date::parse(&cleaned);
    if parsed.is_nan() {
        // Fallback: try original string
        let parsed2 = js_sys::Date::parse(date_str);
        if parsed2.is_nan() {
            return None;
        }
        Some(parsed2 as i64)
    } else {
        Some(parsed as i64)
    }
}

/// Format epoch milliseconds to a short readable date string.
pub fn format_date_display(ms: i64) -> String {
    if ms == 0 {
        return "\u{2014}".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let seconds = date.get_seconds();
    let tz_offset_min = date.get_timezone_offset(); // minutes behind UTC (positive = west)
    let tz_sign = if tz_offset_min >= 0.0 { '-' } else { '+' };
    let tz_abs = tz_offset_min.abs() as i32;
    let tz_h = tz_abs / 60;
    let tz_m = tz_abs % 60;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC{}{:02}:{:02}",
        year, month, day, hours, minutes, seconds, tz_sign, tz_h, tz_m
    )
}

/// Format epoch milliseconds to datetime-local input format (YYYY-MM-DDTHH:MM).
fn format_datetime_local(ms: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        year, month, day, hours, minutes
    )
}

/// Build self-hosted NFT badge URLs. No Arweave/IPFS needed.
fn get_self_hosted_nft_urls() -> (String, String) {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "https://bethere.solana-thailand.workers.dev".to_string());
    let image_url = format!("{origin}/api/badge-hd.svg");
    let metadata_uri_prefix = format!("{origin}/api/metadata/");
    (image_url, metadata_uri_prefix)
}

/// Create a default form state with sensible defaults.
pub fn default_form() -> EventForm {
    EventForm {
        name: String::new(),
        slug: String::new(),
        tagline: String::new(),
        link: String::new(),
        event_start: String::new(),
        event_end: String::new(),
        time_tba: false,
        sheet_id: String::new(),
        sheet_name: "Attendees".to_string(),
        staff_sheet_name: "staff".to_string(),
        quiz_enabled: false,
        nft_collection_mint: String::new(),
        nft_metadata_uri: String::new(),
        nft_image_url: String::new(),
        poster_url: String::new(),
        nft_name_template: String::new(),
        nft_symbol: String::new(),
        nft_description_template: String::new(),
        merkle_tree: String::new(),
        claim_base_url: String::new(),
        organizer_emails: String::new(),
        staff_emails: String::new(),
        status: api::EventStatus::Draft,
        event_format: api::EventFormat::InPerson,
        deposit_enabled: true,
        deposit_amount_usdc: String::new(),
        deposit_amount_thb: String::new(),
        require_contact_info: true,
        require_photo_consent: false,
        promptpay_id: String::new(),
        escrow_address: String::new(),
        escrow_status: api::EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: String::new(),
        refund_deadline_hours: String::new(),
        max_refundable_deposits: String::new(),
        location: String::new(),
        video_url: String::new(),
        in_person_capacity: String::new(),
        online_capacity: String::new(),
        online_open_mode: api::OnlineOpenMode::default(),
        online_registration_open: false,
        deposit_deadline_hours: String::new(),
        visibility: api::EventVisibility::default(),
        updated_at: String::new(),
        community_links: vec![],
        calendar_subscribe_url: String::new(),
    }
}

/// Populate form state from an event detail API response.
pub fn form_from_detail(detail: &api::EventDetail) -> EventForm {
    EventForm {
        name: detail.name.clone(),
        slug: detail.slug.clone(),
        tagline: detail.tagline.clone(),
        link: detail.link.clone(),
        event_start: if detail.event_start_ms > 0 {
            format_datetime_local(detail.event_start_ms)
        } else {
            String::new()
        },
        event_end: if detail.event_end_ms > 0 {
            format_datetime_local(detail.event_end_ms)
        } else {
            String::new()
        },
        time_tba: detail.time_tba,
        sheet_id: detail.sheet_id.clone(),
        sheet_name: if detail.sheet_name.is_empty() {
            "Attendees".to_string()
        } else {
            detail.sheet_name.clone()
        },
        staff_sheet_name: if detail.staff_sheet_name.is_empty() {
            "staff".to_string()
        } else {
            detail.staff_sheet_name.clone()
        },
        quiz_enabled: detail.quiz_enabled,
        nft_collection_mint: detail.nft_collection_mint.clone(),
        nft_metadata_uri: detail.nft_metadata_uri.clone(),
        nft_image_url: detail.nft_image_url.clone(),
        poster_url: detail.poster_url.clone(),
        nft_name_template: detail.nft_name_template.clone(),
        nft_symbol: detail.nft_symbol.clone(),
        nft_description_template: detail.nft_description_template.clone(),
        merkle_tree: detail.merkle_tree.clone(),
        claim_base_url: detail.claim_base_url.clone(),
        organizer_emails: detail.organizer_emails.join(", "),
        staff_emails: detail.staff_emails.join(", "),
        status: detail.status.clone(),
        event_format: detail.event_format.clone(),
        deposit_enabled: detail.deposit_enabled,
        deposit_amount_usdc: if detail.deposit_amount_usdc > 0 { format!("{:.6}", detail.deposit_amount_usdc as f64 / 1_000_000.0).trim_end_matches('0').trim_end_matches('.').to_string() } else { String::new() },
        deposit_amount_thb: if detail.deposit_amount_thb > 0 { detail.deposit_amount_thb.to_string() } else { String::new() },
        require_contact_info: detail.require_contact_info,
        require_photo_consent: detail.require_photo_consent,
        promptpay_id: detail.promptpay_id.clone(),
        escrow_address: detail.escrow_address.clone(),
        escrow_status: detail.escrow_status.clone(),
        organizer_wallet: detail.organizer_wallet.clone(),
        on_chain_event_id: if detail.on_chain_event_id > 0 {
            format!("{}", detail.on_chain_event_id)
        } else {
            String::new()
        },
        refund_deadline_hours: if detail.refund_deadline_hours > 0 {
            format!("{}", detail.refund_deadline_hours)
        } else {
            String::new()
        },
        max_refundable_deposits: if detail.max_refundable_deposits > 0 {
            format!("{}", detail.max_refundable_deposits)
        } else {
            String::new()
        },
        location: detail.location.clone(),
        video_url: detail.video_url.clone(),
        in_person_capacity: detail.in_person_capacity.map(|v| v.to_string()).unwrap_or_default(),
        online_capacity: detail.online_capacity.map(|v| v.to_string()).unwrap_or_default(),
        online_open_mode: detail.online_open_mode.clone(),
        online_registration_open: detail.online_registration_open,
        deposit_deadline_hours: detail.deposit_deadline_hours.map(|h| h.to_string()).unwrap_or_default(),
        visibility: detail.visibility.clone(),
        updated_at: detail.updated_at.clone(),
        community_links: detail.community_links.clone(),
        calendar_subscribe_url: detail.calendar_subscribe_url.clone(),
    }
}

/// Parse comma-separated emails into a Vec.
fn parse_emails(s: &str) -> Vec<String> {
    s.split(',')
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect()
}

/// CSS class for event status badge.
pub fn status_badge_class(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "badge badge-success",
        api::EventStatus::Draft => "badge badge-warning",
        api::EventStatus::Completed => "badge badge-completed",
        api::EventStatus::Archived => "badge badge-archived",
    }
}

/// Human-readable status label.
pub fn status_label(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "Active",
        api::EventStatus::Draft => "Draft",
        api::EventStatus::Completed => "Completed",
        api::EventStatus::Archived => "Archived",
    }
}

/// Callback type for when the form is done (save success or cancel).
pub type OnDone = Arc<dyn Fn() + Send + Sync + 'static>;

// ===== Component =====

/// Full event form component shared by Create and Edit modes.
///
/// Handles all form fields, validation, save logic (create/update + escrow init),
/// and wallet connection for combined Create + Escrow Init flow.
#[component]
pub fn EventFormComponent(
    #[prop(name = "set_toast")] set_toast: WriteSignal<Option<components::ToastMessage>>,
    #[prop(name = "form")] form: ReadSignal<EventForm>,
    #[prop(name = "set_form")] set_form: WriteSignal<EventForm>,
    #[prop(name = "editing_id")] editing_id: ReadSignal<Option<String>>,
    #[prop(name = "is_create")] is_create: bool,
    #[prop(name = "events")] events: ReadSignal<Vec<api::EventMeta>>,
    #[prop(name = "on_done")] on_done: OnDone,
) -> impl IntoView {
    // Section collapse signals (true = expanded)
    let (sec_basic_open, set_sec_basic_open) = signal(true);
    let (sec_schedule_open, set_sec_schedule_open) = signal(true);
    let (sec_sheets_open, set_sec_sheets_open) = signal(true);
    let (sec_nft_open, set_sec_nft_open) = signal(true);
    let (sec_settings_open, set_sec_settings_open) = signal(true);
    let (sec_deposit_open, set_sec_deposit_open) = signal(true);
    let (sec_capacity_open, set_sec_capacity_open) = signal(true);
    let (sec_people_open, set_sec_people_open) = signal(true);
    let (sec_community_open, set_sec_community_open) = signal(false);
    let (sec_poster_open, set_sec_poster_open) = signal(true);

    // Community links — managed as a separate signal for easier row-level editing
    let (cl_links, set_cl_links) = signal(form.get().community_links.clone());

    let add_community_link = move || {
        set_cl_links.update(|links| {
            if links.len() < 5 {
                links.push(crate::api::CommunityLink {
                    platform: "discord".to_string(),
                    url: String::new(),
                    label: String::new(),
                });
            }
        });
    };

    let (slug_manually_edited, set_slug_manually_edited) = signal(false);
    let (slug_taken, set_slug_taken) = signal(false);
    let (saving, set_saving) = signal(false);
    let (poster_busy, set_poster_busy) = signal(false);

    // Wallet connection state for combined Create Event + Escrow Init flow
    let (create_wallet_name, set_create_wallet_name) = signal(String::new());
    let (create_wallet_pk, set_create_wallet_pk) = signal(String::new());
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Detect installed wallets on mount (poll for late-injecting extensions)
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = super::escrow_init::get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo_timers::future::TimeoutFuture::new(300).await;
                    wallets = super::escrow_init::get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[event-form] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

    // Handle name input (auto-generate slug if not manually edited)
    let handle_name_input = move |ev| {
        let name = event_target_value(&ev);
        let edited = slug_manually_edited.get();
        set_form.update(|f| {
            f.name = name.clone();
            if !edited {
                f.slug = generate_slug(&name);
            }
        });
    };

    // Handle slug input
    let handle_slug_input = move |ev| {
        set_slug_manually_edited.set(true);
        let val = event_target_value(&ev);
        set_form.update(|f| f.slug = val);
        set_slug_taken.set(false);
    };

    // Check slug availability on blur (client-side check against loaded events)
    let handle_slug_blur = move |_| {
        let current_slug = form.get().slug.trim().to_lowercase();
        if current_slug.is_empty() {
            set_slug_taken.set(false);
            return;
        }
        let editing = editing_id.get().unwrap_or_default();
        let taken = events.get().iter().any(|e| {
            e.slug.to_lowercase() == current_slug && e.id != editing
        });
        set_slug_taken.set(taken);
    };

    // Handle save (create or update)
    let on_done_save = on_done.clone();
    let on_done_view = on_done.clone();
    let handle_save = move |_: web_sys::MouseEvent| {
        let on_done = on_done_save.clone();
        let current_form = form.get();

        // Validate required fields
        if current_form.name.trim().is_empty() {
            components::show_toast(&set_toast, "Event name is required", components::ToastType::Error);
            return;
        }
        if current_form.slug.trim().is_empty() {
            components::show_toast(&set_toast, "Event slug is required", components::ToastType::Error);
            return;
        }
        // Check slug availability (client-side against loaded events)
        if slug_taken.get() {
            components::show_toast(&set_toast, "This slug is already taken by another event", components::ToastType::Error);
            return;
        }
        if current_form.sheet_id.trim().is_empty() {
            components::show_toast(&set_toast, "Google Sheet ID is required", components::ToastType::Error);
            return;
        }

        // Validate schedule — backend requires positive start_ms and end > start
        let time_tba = current_form.time_tba;
        let start_ms = parse_date_to_ms(&current_form.event_start).unwrap_or(0);
        let end_ms = parse_date_to_ms(&current_form.event_end).unwrap_or(0);
        if start_ms <= 0 {
            components::show_toast(&set_toast, "Event start date is required", components::ToastType::Error);
            return;
        }
        if !time_tba && end_ms <= 0 {
            components::show_toast(&set_toast, "Event end date is required", components::ToastType::Error);
            return;
        }
        if !time_tba && end_ms <= start_ms {
            components::show_toast(&set_toast, "Event end must be after event start", components::ToastType::Error);
            return;
        }
        // TBA mode: default end_ms to start_ms + 24h if not set
        let end_ms = if time_tba && end_ms <= 0 { start_ms + 86_400_000 } else { end_ms };

        // Validate deposit fields when deposit is enabled
        if current_form.deposit_enabled {
            let usdc_val = current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
            let thb_val = current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0);

            // At least one deposit amount must be set
            if usdc_val == 0.0 && thb_val == 0 {
                components::show_toast(&set_toast, "At least one deposit amount (USDC or THB) is required when deposit is enabled", components::ToastType::Error);
                return;
            }

            // USDC minimum precision (6 decimals → 0.01 smallest meaningful)
            if usdc_val > 0.0 && usdc_val < 0.01 {
                components::show_toast(&set_toast, "Minimum deposit is 0.01 USDC", components::ToastType::Error);
                return;
            }

            // USDC max cap (SEC-003: backend enforces $1,000 = 1,000,000,000 lamports)
            if usdc_val > 1000.0 {
                components::show_toast(&set_toast, "Maximum deposit is 1,000 USDC", components::ToastType::Error);
                return;
            }

            // THB amount set but no PromptPay ID — QR generation will fail
            if thb_val > 0 && current_form.promptpay_id.trim().is_empty() {
                components::show_toast(&set_toast, "PromptPay ID is required when THB amount is set", components::ToastType::Error);
                return;
            }

            // In Create mode with USDC deposit > 0, wallet connection is required
            if is_create && usdc_val > 0.0 && create_wallet_pk.get().is_empty() {
                components::show_toast(
                    &set_toast,
                    "Connect your Solana wallet to create event with USDC deposit escrow",
                    components::ToastType::Error,
                );
                return;
            }

            // Escrow init requires USDC amount — check early when wallet is connected
            let do_escrow_init = !create_wallet_pk.get().is_empty()
                && !create_wallet_name.get().is_empty();
            if do_escrow_init && usdc_val == 0.0 {
                components::show_toast(
                    &set_toast,
                    "USDC deposit amount is required to initialize on-chain escrow",
                    components::ToastType::Error,
                );
                return;
            }

            let deadline_hrs = current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0);
            if deadline_hrs == 0 {
                components::show_toast(&set_toast, "Refund deadline must be at least 1 hour", components::ToastType::Error);
                return;
            }
        }

        set_saving.set(true);

        if is_create {
            let body = api::CreateEventBody {
                name: current_form.name.trim().to_string(),
                slug: current_form.slug.trim().to_string(),
                tagline: current_form.tagline.trim().to_string(),
                link: current_form.link.trim().to_string(),
                event_start_ms: start_ms,
                event_end_ms: end_ms,
                sheet_id: current_form.sheet_id.trim().to_string(),
                sheet_name: current_form.sheet_name.trim().to_string(),
                staff_sheet_name: current_form.staff_sheet_name.trim().to_string(),
                quiz_enabled: current_form.quiz_enabled,
                nft_collection_mint: current_form.nft_collection_mint.trim().to_string(),
                nft_metadata_uri: current_form.nft_metadata_uri.trim().to_string(),
                nft_image_url: current_form.nft_image_url.trim().to_string(),
                poster_url: current_form.poster_url.trim().to_string(),
                nft_name_template: current_form.nft_name_template.trim().to_string(),
                nft_symbol: current_form.nft_symbol.trim().to_string(),
                nft_description_template: current_form.nft_description_template.trim().to_string(),
                merkle_tree: current_form.merkle_tree.trim().to_string(),
                claim_base_url: current_form.claim_base_url.trim().to_string(),
                organizer_emails: parse_emails(&current_form.organizer_emails),
                staff_emails: parse_emails(&current_form.staff_emails),
                deposit_enabled: current_form.deposit_enabled,
                deposit_amount_usdc: (current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64,
                deposit_amount_thb: current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0),
                promptpay_id: current_form.promptpay_id.trim().to_string(),
                escrow_address: current_form.escrow_address.trim().to_string(),
                organizer_wallet: if create_wallet_pk.get().is_empty() {
                    current_form.organizer_wallet.trim().to_string()
                } else {
                    create_wallet_pk.get()
                },
                on_chain_event_id: current_form.on_chain_event_id.parse::<u64>().unwrap_or(0),
                refund_deadline_hours: current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0),
                max_refundable_deposits: current_form.max_refundable_deposits.parse::<u32>().unwrap_or(0),
                event_format: current_form.event_format.clone(),
                require_contact_info: current_form.require_contact_info,
                require_photo_consent: current_form.require_photo_consent,
                time_tba,
                location: if current_form.location.trim().is_empty() { None } else { Some(current_form.location.trim().to_string()) },
                video_url: current_form.video_url.trim().to_string(),
                in_person_capacity: current_form.in_person_capacity.trim().parse::<u32>().ok(),
                online_capacity: current_form.online_capacity.trim().parse::<u32>().ok(),
                online_open_mode: current_form.online_open_mode.clone(),
                online_registration_open: current_form.online_registration_open,
                deposit_deadline_hours: current_form.deposit_deadline_hours.trim().parse::<u32>().ok(),
                visibility: current_form.visibility.clone(),
                community_links: cl_links.get(),
                calendar_subscribe_url: current_form.calendar_subscribe_url.trim().to_string(),
            };

            // Determine if we should also initialize escrow after creating the event.
            let do_escrow_init = current_form.deposit_enabled
                && !create_wallet_pk.get().is_empty()
                && !create_wallet_name.get().is_empty();
            let wn = create_wallet_name.get();
            let pk = create_wallet_pk.get();
            leptos::task::spawn_local(async move {
                // Step 1: Create the event
                let created = match api::create_event(&body).await {
                    Ok(data) => {
                        log::info!("[event-form] event created: id={}", data.id);
                        data
                    }
                    Err(e) => {
                        log::error!("[event-form] create failed: {e}");
                        components::show_toast(
                            &set_toast,
                            &format!("Failed to create event: {e}"),
                            components::ToastType::Error,
                        );
                        set_saving.set(false);
                        return;
                    }
                };

                // Step 2: Initialize escrow on-chain (if wallet connected + deposit enabled)
                if do_escrow_init {
                    log::info!("[event-form] initializing escrow for event {}...", created.id);
                    let req = api::InitEscrowRequest {
                        event_id: created.id.clone(),
                    };
                    match api::init_escrow(&req).await {
                        Ok(resp) => {
                            // SEC-014: Verify wallet cluster matches expected network.
                            let expected_cluster = crate::utils::get_cluster();
                            if let Err(cluster_err) = super::escrow_init::check_wallet_cluster(&wn, &expected_cluster).await {
                                log::error!("[event-form] cluster mismatch: {cluster_err}");
                                components::show_toast(
                                    &set_toast,
                                    &cluster_err,
                                    components::ToastType::Error,
                                );
                                set_saving.set(false);
                                return;
                            }
                            log::info!("[event-form] escrow TX built, signing via {wn}...");

                            // Pre-sign simulation.
                            match super::escrow_init::simulate_transaction_js(&wn, &resp.transaction).await {
                                Ok(sim) if sim.ok => {}
                                Ok(sim) => {
                                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                    log::error!("[event-form] escrow simulation failed: {err_msg}");
                                    components::show_toast(&set_toast, &format!("Transaction would fail: {err_msg}"), components::ToastType::Error);
                                    set_saving.set(false);
                                    return;
                                }
                                Err(e) => { log::warn!("[event-form] simulate error (not blocking): {e}"); }
                            }

                            match super::escrow_init::sign_and_send_tx_js(&wn, &resp.transaction).await {
                                crate::wallet_error::WalletResult::Success(signature) => {
                                    log::info!("[event-form] escrow TX confirmed: {}", signature);
                                    // Update the event with escrow fields from the response
                                    let update_body = api::UpdateEventBody {
                                        escrow_address: Some(resp.escrow_address.clone()),
                                        escrow_status: Some(api::EscrowStatus::Initialized),
                                        organizer_wallet: Some(pk.clone()),
                                        on_chain_event_id: Some(resp.on_chain_event_id),
                                        expected_updated_at: if created.updated_at.is_empty() { None } else { Some(created.updated_at.clone()) },
                                        ..Default::default()
                                    };
                                    if let Err(e) = api::update_event(&created.id, &update_body).await {
                                        log::warn!("[event-form] failed to save escrow fields: {e}");
                                    }
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Event '{}' created + escrow initialized", created.name),
                                        components::ToastType::Success,
                                    );
                                }
                                crate::wallet_error::WalletResult::Error(e) => {
                                    let msg = crate::wallet_error::user_friendly_message(&e);
                                    log::error!("[event-form] escrow TX error: code={:?} msg={}", e.code, e.raw_message);
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Event '{}' created, but escrow failed: {}. Edit event to retry.", created.name, msg),
                                        components::ToastType::Warning,
                                    );
                                }
                                crate::wallet_error::WalletResult::UnknownFailure => {
                                    log::error!("[event-form] escrow TX rejected by wallet");
                                    components::show_toast(
                                        &set_toast,
                                        &format!("Event '{}' created, but escrow TX failed. Edit event to retry.", created.name),
                                        components::ToastType::Warning,
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[event-form] init_escrow failed: {e}");
                            components::show_toast(
                                &set_toast,
                                &format!("Event '{}' created, but escrow init failed: {e}. Edit event to retry.", created.name),
                                components::ToastType::Warning,
                            );
                        }
                    }
                } else {
                    components::show_toast(
                        &set_toast,
                        &format!("Event '{}' created", created.name),
                        components::ToastType::Success,
                    );
                }

                on_done();
                set_saving.set(false);
            });
        } else {
            let eid = editing_id.get().unwrap_or_default();
            let body = api::UpdateEventBody {
                name: Some(current_form.name.trim().to_string()),
                slug: Some(current_form.slug.trim().to_string()),
                tagline: Some(current_form.tagline.trim().to_string()),
                link: Some(current_form.link.trim().to_string()),
                status: Some(current_form.status.clone()),
                event_start_ms: Some(start_ms),
                event_end_ms: Some(end_ms),
                sheet_id: Some(current_form.sheet_id.trim().to_string()),
                sheet_name: Some(current_form.sheet_name.trim().to_string()),
                staff_sheet_name: Some(current_form.staff_sheet_name.trim().to_string()),
                quiz_enabled: Some(current_form.quiz_enabled),
                nft_collection_mint: Some(current_form.nft_collection_mint.trim().to_string()),
                nft_metadata_uri: Some(current_form.nft_metadata_uri.trim().to_string()),
                nft_image_url: Some(current_form.nft_image_url.trim().to_string()),
                poster_url: Some(current_form.poster_url.trim().to_string()),
                nft_name_template: Some(current_form.nft_name_template.trim().to_string()),
                nft_symbol: Some(current_form.nft_symbol.trim().to_string()),
                nft_description_template: Some(current_form.nft_description_template.trim().to_string()),
                merkle_tree: Some(current_form.merkle_tree.trim().to_string()),
                claim_base_url: Some(current_form.claim_base_url.trim().to_string()),
                organizer_emails: Some(parse_emails(&current_form.organizer_emails)),
                staff_emails: Some(parse_emails(&current_form.staff_emails)),
                deposit_enabled: Some(current_form.deposit_enabled),
                deposit_amount_usdc: Some((current_form.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64),
                deposit_amount_thb: Some(current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0)),
                promptpay_id: Some(current_form.promptpay_id.trim().to_string()),
                escrow_address: Some(current_form.escrow_address.trim().to_string()),
                escrow_status: None, // not updated via general form — escrow panel manages this
                organizer_wallet: Some(current_form.organizer_wallet.trim().to_string()),
                on_chain_event_id: Some(current_form.on_chain_event_id.parse::<u64>().unwrap_or(0)),
                refund_deadline_hours: Some(current_form.refund_deadline_hours.parse::<u32>().unwrap_or(0)),
                max_refundable_deposits: Some(current_form.max_refundable_deposits.parse::<u32>().unwrap_or(0)),
                expected_updated_at: if current_form.updated_at.is_empty() { None } else { Some(current_form.updated_at.clone()) },
                event_format: Some(current_form.event_format.clone()),
                require_contact_info: Some(current_form.require_contact_info),
                require_photo_consent: Some(current_form.require_photo_consent),
                time_tba: Some(time_tba),
                location: if current_form.location.trim().is_empty() { None } else { Some(current_form.location.trim().to_string()) },
                video_url: Some(current_form.video_url.trim().to_string()),
                in_person_capacity: Some(current_form.in_person_capacity.trim().parse::<u32>().ok()),
                online_capacity: Some(current_form.online_capacity.trim().parse::<u32>().ok()),
                online_open_mode: Some(current_form.online_open_mode.clone()),
                online_registration_open: Some(current_form.online_registration_open),
                deposit_deadline_hours: Some(current_form.deposit_deadline_hours.trim().parse::<u32>().ok()),
                visibility: Some(current_form.visibility.clone()),
                community_links: Some(cl_links.get()),
                calendar_subscribe_url: Some(current_form.calendar_subscribe_url.trim().to_string()),
            };

            leptos::task::spawn_local(async move {
                match api::update_event(&eid, &body).await {
                    Ok(data) => {
                        components::show_toast(
                            &set_toast,
                            &format!("Event '{}' updated", data.name),
                            components::ToastType::Success,
                        );
                        on_done();
                    }
                    Err(e) => {
                        log::error!("[event-form] update failed: {e}");
                        let msg = if e.message.contains("conflict") {
                            "Event was modified by another user. Please reload the event and re-apply your changes.".to_string()
                        } else {
                            format!("Failed to update event: {e}")
                        };
                        components::show_toast(&set_toast, &msg, components::ToastType::Error);
                    }
                }
                set_saving.set(false);
            });
        }
    };

    // Wrap on_done in store_value so the view! macro's Fn closure can clone it
    // without moving the original Arc.
    let stored_on_done = leptos::prelude::StoredValue::new(on_done_view);

    view! {
        <div class="card">
            <h2 class="admin-section-heading">{if is_create { "Create Event" } else { "Edit Event" }}</h2>

            // ── Basic Info ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_basic_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-basic"></span>
                    <span class="form-section-title">"Basic Info"</span>
                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_basic_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_basic_open.get()>
                    <div class="quiz-settings-grid">
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Name"<span class="field-required-badge">"Required"</span></label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="Event Name"
                                prop:value=move || form.get().name
                                on:input=handle_name_input
                            />
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Slug"<span class="field-required-badge">"Required"</span></label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="event-slug"
                                prop:value=move || form.get().slug
                                on:input=handle_slug_input
                                on:focusout=handle_slug_blur
                            />
                            <Show
                                when=move || slug_taken.get()
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "This slug is already taken by another event"
                                </div>
                            </Show>
                            <span class="quiz-setting-hint">"Auto-generated from name"</span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Tagline"<span class="field-optional-badge">"Optional"</span></label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="A short description"
                                prop:value=move || form.get().tagline
                                on:input=move |ev| set_form.update(|f| f.tagline = event_target_value(&ev))
                            />
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Link"<span class="field-optional-badge">"Optional"</span></label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="https://example.com"
                                prop:value=move || form.get().link
                                on:input=move |ev| set_form.update(|f| f.link = event_target_value(&ev))
                            />
                        </div>
                    </div>
                </div>
            </div>

            // ── Schedule ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_schedule_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-schedule"></span>
                    <span class="form-section-title">"Schedule"</span>
                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_schedule_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_schedule_open.get()>
                    <div class="quiz-settings-grid">
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Event Start"<span class="field-required-badge">"Required"</span></label>
                        <input
                            type="datetime-local"
                            class="quiz-number-input"
                            prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_start).unwrap_or(0))
                            on:input=move |ev| {
                                let val = event_target_value(&ev);
                                let ms = parse_date_to_ms(&val).unwrap_or(0);
                                set_form.update(|f| f.event_start = if ms > 0 { format_datetime_local(ms) } else { val });
                            }
                        />
                        <span class="quiz-setting-hint">"Times in your local timezone"</span>
                    </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Event End"<span class="field-required-badge">"Required"</span></label>
                        <input
                            type="datetime-local"
                            class="quiz-number-input"
                            prop:value=move || format_datetime_local(parse_date_to_ms(&form.get().event_end).unwrap_or(0))
                            on:input=move |ev| {
                                let val = event_target_value(&ev);
                                let ms = parse_date_to_ms(&val).unwrap_or(0);
                                set_form.update(|f| f.event_end = if ms > 0 { format_datetime_local(ms) } else { val });
                            }
                        />
                        <span class="quiz-setting-hint">"Times in your local timezone"</span>
                    </div>
                    <div class="quiz-setting-item event-form-span-full">
                        <label class="event-form-tba-label">
                            <input
                                type="checkbox"
                                prop:checked=move || form.get().time_tba
                                on:change=move |ev| set_form.update(|f| f.time_tba = event_target_checked(&ev))
                            />
                            <span class="quiz-field-label event-form-tba-field-label">"Time TBA (To Be Announced)"</span>
                            <span class="form-hint event-form-tba-hint">"Show \"TBA\" instead of specific time"</span>
                        </label>
                    </div>
                    <Show
                        when=move || {
                            let start = parse_date_to_ms(&form.get().event_start).unwrap_or(0);
                            let end = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                            start > 0 && end > 0 && end <= start
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "Event end must be after event start"
                        </div>
                    </Show>
                    <div class="quiz-setting-item event-form-span-full">
                        <label class="quiz-field-label">"Location"<span class="field-optional-badge">"Optional"</span></label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="e.g. Impact Exhibition Center, Bangkok"
                            prop:value=move || form.get().location
                            on:input=move |ev| set_form.update(|f| f.location = event_target_value(&ev))
                        />
                        <span class="quiz-setting-hint">"Venue name and address for in-person events"</span>
                    </div>
                    <div class="quiz-setting-item event-form-span-full">
                        <label class="quiz-field-label">"Video / Livestream URL"<span class="field-optional-badge">"Optional"</span></label>
                        <input
                            type="url"
                            class="quiz-number-input"
                            placeholder="e.g. https://youtube.com/live/abc123"
                            prop:value=move || form.get().video_url
                            on:input=move |ev| set_form.update(|f| f.video_url = event_target_value(&ev))
                        />
                        <span class="quiz-setting-hint">"YouTube livestream or recording link — shown to attendees on the event page"</span>
                    </div>
                    <div class="quiz-setting-item event-form-span-full">
                        <label class="quiz-field-label">"Calendar Subscribe URL"<span class="field-optional-badge">"Optional"</span></label>
                        <input
                            type="url"
                            class="quiz-number-input"
                            placeholder="e.g. https://calendar.google.com/calendar/embed?src=..."
                            prop:value=move || form.get().calendar_subscribe_url
                            on:input=move |ev| set_form.update(|f| f.calendar_subscribe_url = event_target_value(&ev))
                        />
                        <span class="quiz-setting-hint">"Google Calendar embed URL — shown as 'Our Event Calendar' on ticket page"</span>
                    </div>
                </div>
                </div>
            </div>

            // ── Google Sheets ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_sheets_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-sheets"></span>
                    <span class="form-section-title">"Google Sheets"</span>
                    <span class="form-section-badge form-section-badge-required">"Required"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_sheets_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_sheets_open.get()>
                    // ── Quick Guide: How to get Google Sheet ID ──
                    <div class="sheet-guide-box event-form-sheet-guide">
                        <div class="sheet-guide-heading"><Icon icon=IconName::Copy class="icon-sm"/>" Quick Guide"</div>
                        <ol class="sheet-guide-steps">
                            <li>
                                <a
                                    href="https://docs.google.com/forms/create"
                                    target="_blank"
                                    rel="noopener"
                                    class="sheet-guide-link"
                                >
                                    "Create a Google Form"
                                </a>
                                " → add your registration fields"
                            </li>
                            <li>"In the Form editor → Responses tab → click 'Link to Sheets'"</li>
                            <li>"Copy the Sheet ID from the URL:"</li>
                        </ol>
                        <div class="sheet-guide-url">
                            <code>"docs.google.com/spreadsheets/d/"</code>
                            <code class="sheet-guide-highlight">"YOUR_SHEET_ID"</code>
                            <code>"/edit"</code>
                        </div>
                    </div>

                    <div class="quiz-settings-grid">
                        <div class="quiz-setting-item">
                                <label class="quiz-field-label">"Sheet ID"<span class="field-required-badge">"Required"</span></label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="Paste Google Sheet ID here"
                                prop:value=move || form.get().sheet_id
                                on:input=move |ev| set_form.update(|f| f.sheet_id = event_target_value(&ev))
                            />
                            <div class="quiz-setting-hint event-form-hint-row">
                                <Show
                                    when=move || !form.get().sheet_id.trim().is_empty()
                                    fallback=|| view! { <span></span> }
                                >
                                    <a
                                        href=move || crate::utils::google_sheet_url(&form.get().sheet_id)
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        class="event-form-link-sm"
                                    >
                                        "Open in Google Sheets ↗"
                                    </a>
                                </Show>
                            </div>
                        </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Sheet Name"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="Attendees"
                            prop:value=move || form.get().sheet_name
                            on:input=move |ev| set_form.update(|f| f.sheet_name = event_target_value(&ev))
                        />
                    </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Staff Sheet Name"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="staff"
                            prop:value=move || form.get().staff_sheet_name
                            on:input=move |ev| set_form.update(|f| f.staff_sheet_name = event_target_value(&ev))
                        />
                    </div>
                </div>
                </div>
            </div>

            // ── Event Poster (marketing hero image) ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_poster_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-nft"></span>
                    <span class="form-section-title">"Event Poster"</span>
                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_poster_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_poster_open.get()>
                    <div class="quiz-setting-item event-form-span-full">
                        <div class="hint-info event-form-nft-intro">
                            "Shown at the top of the public event page (/e/{{slug}}). Falls back to the NFT badge if not set."
                        </div>
                        // Live preview — mirrors the badge preview pattern
                        <div class="event-form-nft-actions">
                            <Show
                                when=move || !form.get().poster_url.is_empty()
                                fallback=|| view! { <div></div> }
                            >
                                <img
                                    src=move || form.get().poster_url
                                    alt="Event poster preview"
                                    class="event-form-badge-preview"
                                />
                                <span class="event-form-badge-type-label">
                                    {move || {
                                        let url = form.get().poster_url;
                                        if url.starts_with("/api/storage/posters") { "Uploaded poster" } else { "External poster" }.to_string()
                                    }}
                                </span>
                            </Show>
                        </div>
                    </div>
                    <div class="quiz-settings-grid">
                        // File upload — only available once the event exists (needs an event_id).
                        <div class="quiz-setting-item event-form-span-full">
                            <label class="quiz-field-label">"Upload Image"</label>
                            <Show
                                when=move || !editing_id.get().unwrap_or_default().is_empty()
                                fallback=|| view! {
                                    <div class="quiz-setting-hint">
                                        "Save the event first, then return here to upload a poster file."
                                    </div>
                                }
                            >
                                <div class="event-form-nft-actions">
                                    <input
                                        type="file"
                                        accept="image/*"
                                        prop:disabled=move || poster_busy.get()
                                        on:change=move |ev| {
                                            let target = match ev.target() {
                                                Some(t) => t,
                                                None => return,
                                            };
                                            let input: web_sys::HtmlInputElement = target.unchecked_into();
                                            let file = input.files().and_then(|fl| fl.item(0));
                                            let eid = editing_id.get().unwrap_or_default();
                                            if eid.is_empty() {
                                                components::show_toast(&set_toast, "Save the event before uploading a poster", components::ToastType::Error);
                                                return;
                                            }
                                            let Some(file) = file else { return };
                                            let content_type = file.type_();
                                            if !content_type.starts_with("image/") {
                                                components::show_toast(&set_toast, "Please choose an image file", components::ToastType::Error);
                                                return;
                                            }
                                            set_poster_busy.set(true);
                                            leptos::task::spawn_local(async move {
                                                match api::upload_poster(&eid, &file, &content_type).await {
                                                    Ok(res) => {
                                                        let url = res.poster_url.clone();
                                                        set_form.update(|f| f.poster_url = url.clone());
                                                        components::show_toast(&set_toast, "Poster uploaded", components::ToastType::Success);
                                                    }
                                                    Err(e) => {
                                                        log::error!("[event-form] poster upload failed: {e}");
                                                        components::show_toast(&set_toast, &format!("Failed to upload poster: {e}"), components::ToastType::Error);
                                                    }
                                                }
                                                set_poster_busy.set(false);
                                            });
                                        }
                                    />
                                    <Show
                                        when=move || poster_busy.get()
                                        fallback=|| view! { <span></span> }
                                    >
                                        <span class="quiz-setting-hint">"Uploading…"</span>
                                    </Show>
                                </div>
                            </Show>
                            <span class="quiz-setting-hint">"Max 5 MB. PNG / JPG / WebP / SVG."</span>
                        </div>
                        // URL override — always available (also covers pre-save case).
                        <div class="quiz-setting-item event-form-span-full">
                            <label class="quiz-field-label">"Poster URL (override)"</label>
                            <div class="event-form-nft-actions">
                                <input
                                    type="text"
                                    class="quiz-number-input"
                                    placeholder="/api/storage/posters/<id> or https://..."
                                    prop:value=move || form.get().poster_url
                                    on:input=move |ev| set_form.update(|f| f.poster_url = event_target_value(&ev))
                                />
                                <Show
                                    when=move || !form.get().poster_url.is_empty() && !editing_id.get().unwrap_or_default().is_empty()
                                    fallback=|| view! { <span></span> }
                                >
                                    <button
                                        class="btn btn-outline btn-sm"
                                        prop:disabled=move || poster_busy.get()
                                        on:click=move |_| {
                                            let eid = editing_id.get().unwrap_or_default();
                                            if eid.is_empty() {
                                                set_form.update(|f| f.poster_url = String::new());
                                                return;
                                            }
                                            set_poster_busy.set(true);
                                            leptos::task::spawn_local(async move {
                                                match api::delete_poster(&eid).await {
                                                    Ok(_) => {
                                                        set_form.update(|f| f.poster_url = String::new());
                                                        components::show_toast(&set_toast, "Poster removed", components::ToastType::Success);
                                                    }
                                                    Err(e) => {
                                                        log::error!("[event-form] poster delete failed: {e}");
                                                        components::show_toast(&set_toast, &format!("Failed to remove poster: {e}"), components::ToastType::Error);
                                                    }
                                                }
                                                set_poster_busy.set(false);
                                            });
                                        }
                                    >
                                        "✕ Remove"
                                    </button>
                                </Show>
                            </div>
                            <Show
                                when=move || {
                                    let v = form.get().poster_url.trim().to_string();
                                    !v.is_empty()
                                        && !v.starts_with("http://")
                                        && !v.starts_with("https://")
                                        && !v.starts_with("/api/storage/posters")
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "URL should start with http://, https://, or /api/storage/posters"
                                </div>
                            </Show>
                        </div>
                    </div>
                </div>
            </div>

            // ── NFT Configuration ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_nft_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-nft"></span>
                    <span class="form-section-title">"NFT Attendance Badge"</span>
                    <span class="form-section-badge form-section-badge-recommended">"Recommended"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_nft_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_nft_open.get()>
                    // ── Quick-fill default badge ──
                    <div class="quiz-setting-item event-form-span-full">
                        <div class="hint-info event-form-nft-intro">
                            "NFT badges reward attendees for showing up. Use the default BeThere badge or skip if you don't need one."
                        </div>
                        <div class="event-form-nft-actions">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| {
                                    let (img_url, meta_prefix) = get_self_hosted_nft_urls();
                                    let eid = editing_id.get().unwrap_or_default();
                                    set_form.update(|f| {
                                        f.nft_image_url = img_url;
                                        if !eid.is_empty() {
                                            f.nft_metadata_uri = format!("{meta_prefix}{eid}");
                                        }
                                        f.nft_name_template = "BeThere - {event_name}".to_string();
                                        f.nft_symbol = "BETHERE".to_string();
                                        f.nft_description_template = "Proof of attendance at {event_name}".to_string();
                                    });
                                }
                            >
                                <Icon icon=IconName::Palette class="icon-sm"/>" Use default badge"
                            </button>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| {
                                    set_form.update(|f| {
                                        f.nft_image_url = String::new();
                                        f.nft_metadata_uri = String::new();
                                        f.nft_name_template = String::new();
                                        f.nft_symbol = String::new();
                                        f.nft_description_template = String::new();
                                        f.nft_collection_mint = String::new();
                                        f.merkle_tree = String::new();
                                    });
                                }
                            >
                                "✕ Skip NFT"
                            </button>
                            // Badge preview
                            <Show
                                when=move || !form.get().nft_image_url.is_empty()
                                fallback=|| view! { <div></div> }
                            >
                                <img
                                    src=move || form.get().nft_image_url
                                    alt="NFT badge preview"
                                    class="event-form-badge-preview"
                                />
                                <span class="event-form-badge-type-label">
                                    {move || {
                                        let url = form.get().nft_image_url;
                                        if url.contains("/api/badge") { "Default badge" } else { "Custom badge" }.to_string()
                                    }}
                                </span>
                            </Show>
                        </div>
                    </div>
                    <div class="quiz-settings-grid">
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Collection Mint"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="NFT collection mint address"
                            prop:value=move || form.get().nft_collection_mint
                            on:input=move |ev| set_form.update(|f| f.nft_collection_mint = event_target_value(&ev))
                        />
                        <Show
                            when=move || {
                                let v = form.get().nft_collection_mint.trim().to_string();
                                !v.is_empty() && !v.chars().all(|c| c.is_ascii_alphanumeric() && (c.is_ascii_digit() || (c >= 'A' && c <= 'H') || (c >= 'J' && c <= 'N') || (c >= 'P' && c <= 'Z') || (c >= 'a' && c <= 'k') || (c >= 'm' && c <= 'z')))
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="hint-warning-xs">
                                "Invalid base58 characters detected (expected Solana address format)"
                            </div>
                        </Show>
                        <div class="quiz-setting-hint event-form-hint-row">
                            <span>"Solana mint address (base58)"</span>
                            <Show
                                when=move || !form.get().nft_collection_mint.trim().is_empty()
                                fallback=|| view! { <span></span> }
                            >
                                <a
                                    href=move || crate::utils::metaplex_explorer_url(&form.get().nft_collection_mint.trim(), &crate::utils::get_cluster())
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="event-form-link-sm"
                                >
                                    "Verify on Metaplex ↗"
                                </a>
                            </Show>
                        </div>
                    </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Merkle Tree"</label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="Tree address (base58) — leave empty for Helius default"
                                prop:value=move || form.get().merkle_tree
                                on:input=move |ev| set_form.update(|f| f.merkle_tree = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Reserved for future use. Helius mints to its own managed tree."</span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Metadata URI"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="https://..."
                            prop:value=move || form.get().nft_metadata_uri
                            on:input=move |ev| set_form.update(|f| f.nft_metadata_uri = event_target_value(&ev))
                        />
                        <Show
                            when=move || {
                                let v = form.get().nft_metadata_uri.trim().to_string();
                                !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="hint-warning-xs">
                                "URI must start with http:// or https://"
                            </div>
                        </Show>
                        <Show
                            when=move || editing_id.get().unwrap_or_default().is_empty()
                            fallback=|| view! { <div></div> }
                        >
                            <span class="quiz-setting-hint">"Save the event first, then edit to auto-fill this with the dynamic metadata URL."</span>
                        </Show>
                    </div>
                    <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Image URL"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="https://..."
                            prop:value=move || form.get().nft_image_url
                            on:input=move |ev| set_form.update(|f| f.nft_image_url = event_target_value(&ev))
                        />
                        <Show
                            when=move || {
                                let v = form.get().nft_image_url.trim().to_string();
                                !v.is_empty() && !v.starts_with("http://") && !v.starts_with("https://")
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="hint-warning-xs">
                                "URL must start with http:// or https://"
                            </div>
                        </Show>
                    </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Name Template"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="{event_name} #1"
                            prop:value=move || form.get().nft_name_template
                            on:input=move |ev| set_form.update(|f| f.nft_name_template = event_target_value(&ev))
                        />
                        <span class="quiz-setting-hint">"Use {event_name} placeholder"</span>
                        <Show
                            when=move || {
                                let f = form.get();
                                let resolved = if f.nft_name_template.is_empty() {
                                    format!("BeThere - {}", f.name)
                                } else {
                                    f.nft_name_template.replace("{event_name}", &f.name)
                                };
                                resolved.len() > 32
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="hint-warning-xs">
                                "Resolved name exceeds 32-char limit (Bubblegum max). Name will be truncated."
                            </div>
                        </Show>
                    </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Symbol"</label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="NFT"
                            prop:value=move || form.get().nft_symbol
                            on:input=move |ev| set_form.update(|f| f.nft_symbol = event_target_value(&ev))
                        />
                    </div>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Description Template"</label>
                        <textarea
                            class="quiz-textarea quiz-textarea-sm"
                            placeholder="NFT description..."
                            prop:value=move || form.get().nft_description_template
                            on:input=move |ev| set_form.update(|f| f.nft_description_template = event_target_value(&ev))
                        ></textarea>
                    </div>
                </div>
                </div>
            </div>

            // ── Settings ──
            <div class="form-section">
                <div class="form-section-header" on:click=move |_| set_sec_settings_open.update(|v| *v = !*v)>
                    <span class="form-section-icon form-section-icon-settings"></span>
                    <span class="form-section-title">"Settings"</span>
                    <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                    <span class="form-section-toggle" class:form-section-toggle-open=move || sec_settings_open.get()>"▼"</span>
                </div>
                <div class="form-section-body" class:form-section-body-hidden=move || !sec_settings_open.get()>
                    <div class="quiz-settings-grid">
                        <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Claim Base URL"<span class="field-optional-badge">"Auto"</span></label>
                        <input
                            type="text"
                            class="quiz-number-input"
                            placeholder="Leave empty to auto-use current domain"
                            prop:value=move || form.get().claim_base_url
                            on:input=move |ev| set_form.update(|f| f.claim_base_url = event_target_value(&ev))
                        />
                        <span class="quiz-setting-hint">
                            "The base URL for attendee claim links (e.g. "
                            <code class="event-form-code-inherit">"https://bethere.solana-thailand.workers.dev/claim"</code>
                            "). Leave empty — the system auto-generates claim links from your current domain."
                        </span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Quiz Enabled"</label>
                            <label class="quiz-toggle-label event-form-toggle-label">
                                <input
                                    type="checkbox"
                                    class="quiz-toggle-checkbox"
                                    prop:checked=move || form.get().quiz_enabled
                                    on:change=move |ev| {
                                        let checked = event_target_checked(&ev);
                                        set_form.update(|f| f.quiz_enabled = checked);
                                    }
                                />
                                <span class="quiz-toggle-switch"></span>
                                <span class="quiz-toggle-text">
                                    {move || if form.get().quiz_enabled { "Yes" } else { "No" }}
                                </span>
                            </label>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Require Contact Info"</label>
                            <label class="quiz-toggle-label event-form-toggle-label">
                                <input
                                    type="checkbox"
                                    class="quiz-toggle-checkbox"
                                    prop:checked=move || form.get().require_contact_info
                                    on:change=move |ev| {
                                        let checked = event_target_checked(&ev);
                                        set_form.update(|f| f.require_contact_info = checked);
                                    }
                                />
                                <span class="quiz-toggle-switch"></span>
                                <span class="quiz-toggle-text">
                                    {move || if form.get().require_contact_info { "Yes" } else { "No" }}
                                </span>
                            </label>
                            <span class="quiz-setting-hint">
                                "When enabled, self-registration requires attendees to provide a contact channel (Telegram/Line/Facebook/X) and username. Disable for events that don't need it."
                            </span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Require Photo Consent (PDPA)"</label>
                            <label class="quiz-toggle-label event-form-toggle-label">
                                <input
                                    type="checkbox"
                                    class="quiz-toggle-checkbox"
                                    prop:checked=move || form.get().require_photo_consent
                                    on:change=move |ev| {
                                        let checked = event_target_checked(&ev);
                                        set_form.update(|f| f.require_photo_consent = checked);
                                    }
                                />
                                <span class="quiz-toggle-switch"></span>
                                <span class="quiz-toggle-text">
                                    {move || if form.get().require_photo_consent { "Yes" } else { "No" }}
                                </span>
                            </label>
                            <span class="quiz-setting-hint">
                                "When enabled, attendees must consent to photo/video capture during the event. Required for Thai events with photography (PDPA compliance)."
                            </span>
                        </div>
                        // Status selector (edit only)
                        <Show when=move || !is_create fallback=|| view! { <div></div> }>
                            <div class="quiz-setting-item">
                                <label class="quiz-field-label">"Status"</label>
                                <select
                                    class="quiz-number-input"
                                    on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        let status = match val.as_str() {
                                            "active" => api::EventStatus::Active,
                                            "completed" => api::EventStatus::Completed,
                                            _ => api::EventStatus::Draft,
                                        };
                                        set_form.update(|f| f.status = status);
                                    }
                                    prop:value=move || {
                                        match form.get().status {
                                            api::EventStatus::Active => "active".to_string(),
                                            api::EventStatus::Completed => "completed".to_string(),
                                            api::EventStatus::Draft => "draft".to_string(),
                                            api::EventStatus::Archived => "archived".to_string(),
                                        }
                                    }
                                >
                                    <option value="draft">"Draft"</option>
                                    <option value="active">"Active"</option>
                                    <option value="completed">"Completed"</option>
                                </select>
                            </div>
                        </Show>
                    </div>
                    </div>
                </div>

                // ── Event Format ──
                <div class="dep-config-row">
                    <span class="dep-config-label">"Format"</span>
                    <select
                        class="form-select form-select-sm"
                        on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let fmt = match val.as_str() {
                                "online" => api::EventFormat::Online,
                                "hybrid" => api::EventFormat::Hybrid,
                                _ => api::EventFormat::InPerson,
                            };
                            set_form.update(|f| {
                                f.event_format = fmt.clone();
                                // Auto-sync deposit: in-person/hybrid = enabled
                                f.deposit_enabled = fmt.has_in_person();
                            });
                        }
                        prop:value=move || form.get().event_format.as_str()
                    >
                        <option value="in_person">"In-Person"</option>
                        <option value="online">"Online"</option>
                        <option value="hybrid">"Hybrid"</option>
                    </select>
                </div>
                // Format description
                <div class="dep-info-note">
                    <p class="hint-note">
                        {move || match form.get().event_format {
                            api::EventFormat::InPerson => "Physical event with deposit commitment. Attendees get 100% refund at check-in.",
                            api::EventFormat::Online => "Virtual event. No deposit — quest completion serves as virtual check-in.",
                            api::EventFormat::Hybrid => "Both in-person and online tracks. In-person attendees deposit; online attendees complete quests.",
                        }}
                    </p>
                </div>

                // ── Visibility ──
                <div class="dep-config-row">
                    <span class="dep-config-label">"Visibility"</span>
                    <div class="radio-group">
                        <label class="radio-label">
                            <input type="radio" name="visibility" value="public"
                                checked=move || form.get().visibility == api::EventVisibility::Public
                                on:change=move |_| set_form.update(|f| f.visibility = api::EventVisibility::Public)
                            />
                            <span>"🌐 Public"</span>
                            <span class="form-hint">" — visible on landing page, anyone can register"</span>
                        </label>
                        <label class="radio-label">
                            <input type="radio" name="visibility" value="private"
                                checked=move || form.get().visibility == api::EventVisibility::Private
                                on:change=move |_| set_form.update(|f| f.visibility = api::EventVisibility::Private)
                            />
                            <span>"🔒 Private"</span>
                            <span class="form-hint">" — hidden from landing, requires sign-in + access"</span>
                        </label>
                    </div>
                </div>

                // ── Capacity ──
                <Show when=move || {
                    let fmt = form.get().event_format;
                    fmt == api::EventFormat::InPerson || fmt == api::EventFormat::Hybrid
                } fallback=|| view! { <div></div> }>
                <div class="form-section">
                    <div class="form-section-header" on:click=move |_| set_sec_capacity_open.update(|v| *v = !*v)>
                        <span class="form-section-icon form-section-icon-settings"></span>
                        <span class="form-section-title">"Capacity & Registration Control"</span>
                        <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                        <span class="form-section-toggle" class:form-section-toggle-open=move || sec_capacity_open.get()>&"▼"</span>
                    </div>
                    <div class="form-section-body" class:form-section-body-hidden=move || !sec_capacity_open.get()>
                        <div class="quiz-settings-grid">
                            // In-person capacity
                            <Show when=move || form.get().event_format != api::EventFormat::Online fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-field-label">"In-Person Capacity"<span class="field-optional-badge">"Unlimited if empty"</span></label>
                                    <input
                                        type="number"
                                        class="quiz-number-input"
                                        placeholder="e.g., 150"
                                        min="1"
                                        step="1"
                                        prop:value=move || form.get().in_person_capacity
                                        on:input=move |ev| set_form.update(|f| f.in_person_capacity = event_target_value(&ev))
                                    />
                                    <span class="quiz-setting-hint">"Maximum number of on-site attendees. Leave empty for unlimited. Includes walk-ins."</span>
                                </div>
                            </Show>
                            // Online capacity (hybrid only)
                            <Show when=move || form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-field-label">"Online Capacity"<span class="field-optional-badge">"Unlimited if empty"</span></label>
                                    <input
                                        type="number"
                                        class="quiz-number-input"
                                        placeholder="e.g., 500"
                                        min="1"
                                        step="1"
                                        prop:value=move || form.get().online_capacity
                                        on:input=move |ev| set_form.update(|f| f.online_capacity = event_target_value(&ev))
                                    />
                                    <span class="quiz-setting-hint">"Maximum online attendees. Leave empty for unlimited. Prevents NFT exhaustion for large events."</span>
                                </div>
                            </Show>
                            // Online open mode (hybrid only)
                            <Show when=move || form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-field-label">"Online Registration"</label>
                                    <select
                                        class="quiz-number-input"
                                        on:change=move |ev| {
                                            let val = event_target_value(&ev);
                                            let mode = match val.as_str() {
                                                "auto_on_full" => api::OnlineOpenMode::AutoOnFull,
                                                "manual" => api::OnlineOpenMode::Manual,
                                                _ => api::OnlineOpenMode::Always,
                                            };
                                            set_form.update(|f| f.online_open_mode = mode);
                                        }
                                        prop:value=move || form.get().online_open_mode.as_str()
                                    >
                                        <option value="always">"Always Open"</option>
                                        <option value="auto_on_full">"Auto (when in-person full)"</option>
                                        <option value="manual">"Manual Toggle"</option>
                                    </select>
                                    <span class="quiz-setting-hint">
                                        {move || match form.get().online_open_mode {
                                            api::OnlineOpenMode::Always => "Both in-person and online tracks open from the start.",
                                            api::OnlineOpenMode::AutoOnFull => "Online registration opens automatically when in-person capacity is reached.",
                                            api::OnlineOpenMode::Manual => "You control when online registration opens via the toggle below.",
                                        }}
                                    </span>
                                </div>
                            </Show>
                            // Manual toggle (only when Manual mode selected)
                            <Show when=move || form.get().online_open_mode == api::OnlineOpenMode::Manual && form.get().event_format == api::EventFormat::Hybrid fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-field-label">"Online Registration Open"</label>
                                    <label class="quiz-toggle-label event-form-toggle-label">
                                        <input
                                            type="checkbox"
                                            class="quiz-toggle-checkbox"
                                            prop:checked=move || form.get().online_registration_open
                                            on:change=move |ev| {
                                                let checked = event_target_checked(&ev);
                                                set_form.update(|f| f.online_registration_open = checked);
                                            }
                                        />
                                        <span class="quiz-toggle-switch"></span>
                                        <span class="quiz-toggle-text">
                                            {move || if form.get().online_registration_open { "Open" } else { "Closed" }}
                                        </span>
                                    </label>
                                    <span class="quiz-setting-hint">"Toggle online registration on/off. Attendees see the online option only when this is enabled."</span>
                                </div>
                            </Show>
                            // Deposit deadline (in-person / hybrid only)
                            <Show when=move || form.get().deposit_enabled && form.get().event_format != api::EventFormat::Online fallback=|| view! { <div></div> }>
                                <div class="quiz-setting-item">
                                    <label class="quiz-field-label">"Deposit Deadline"<span class="field-optional-badge">"Hours"</span></label>
                                    <input
                                        type="number"
                                        class="quiz-number-input"
                                        placeholder="e.g., 24"
                                        min="1"
                                        step="1"
                                        prop:value=move || form.get().deposit_deadline_hours
                                        on:input=move |ev| set_form.update(|f| f.deposit_deadline_hours = event_target_value(&ev))
                                    />
                                    <span class="quiz-setting-hint">"Hours after registration to complete deposit. Attendees who miss the deadline are auto-switched to online track. Leave empty for no deadline."</span>
                                </div>
                            </Show>
                        </div>
                    </div>
                </div>
                </Show>

                // Deposit config section — only when enabled
                <Show when=move || form.get().deposit_enabled fallback=|| view! { <div></div> }>
                <div class="form-section">
                    <div class="form-section-header" on:click=move |_| set_sec_deposit_open.update(|v| *v = !*v)>
                        <span class="form-section-icon form-section-icon-deposit"></span>
                        <span class="form-section-title">"Deposit Details"</span>
                        <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                        <span class="form-section-toggle" class:form-section-toggle-open=move || sec_deposit_open.get()>"▼"</span>
                    </div>
                    <div class="form-section-body" class:form-section-body-hidden=move || !sec_deposit_open.get()>
                        <div class="quiz-settings-grid">
                            <div class="quiz-setting-item">
                                <label class="quiz-field-label">"USDC Amount"<span class="field-optional-badge">"Required for escrow"</span></label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g. 10 (whole USDC)"
                                step="0.01"
                                min="0.01"
                                prop:value=move || form.get().deposit_amount_usdc
                                on:input=move |ev| set_form.update(|f| f.deposit_amount_usdc = event_target_value(&ev))
                            />
                            <Show
                                when=move || {
                                    let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                    form.get().deposit_enabled && val > 0.0 && val < 0.01
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "Minimum deposit is 0.01 USDC"
                                </div>
                            </Show>
                            <Show
                                when=move || {
                                    let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                    let thb = form.get().deposit_amount_thb.parse::<u64>().unwrap_or(0);
                                    form.get().deposit_enabled && val == 0.0 && thb == 0
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "At least one deposit amount (USDC or THB) is required"
                                </div>
                            </Show>
                            <span class="quiz-setting-hint">"Amount in whole USDC (e.g. 10 = 10 USDC). Max: 1,000 USDC"</span>
                        </div>
                        <Show
                            when=move || {
                                let val = form.get().deposit_amount_usdc.parse::<f64>().unwrap_or(0.0);
                                val > 1000.0
                            }
                            fallback=|| view! { <div></div> }
                        >
                            <div class="hint-warning-xs">
                                "Maximum deposit is 1,000 USDC (SEC-003 cap)"
                            </div>
                        </Show>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"THB Amount"</label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g. 500"
                                step="1"
                                min="0"
                                prop:value=move || form.get().deposit_amount_thb
                                on:input=move |ev| set_form.update(|f| f.deposit_amount_thb = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Amount in Thai Baht"</span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"PromptPay ID"</label>
                            <input
                                type="text"
                                class="quiz-number-input"
                                placeholder="e.g. 0812345678 or 1-1001-00000-00-0"
                                prop:value=move || form.get().promptpay_id
                                on:input=move |ev| set_form.update(|f| f.promptpay_id = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"Thai phone number or national ID for PromptPay QR generation"</span>
                            <Show
                                when=move || {
                                    let thb = form.get().deposit_amount_thb.parse::<u64>().unwrap_or(0);
                                    let pp = form.get().promptpay_id.trim().to_string();
                                    thb > 0 && pp.is_empty()
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "PromptPay ID is required when THB amount is set"
                                </div>
                            </Show>
                        </div>
                        // ── On-chain escrow fields: always read-only ──
                        <Show when=move || !form.get().escrow_address.is_empty() fallback=|| view! { <div></div> }>
                            <div class="quiz-setting-item">
                                <label class="quiz-field-label">
                                    "Escrow Address"
                                    <a
                                        href=move || crate::utils::solscan_address_url(&form.get().escrow_address, &crate::utils::get_cluster())
                                        target="_blank"
                                        rel="noopener"
                                        class="escrow-solscan-link"
                                    >
                                        "Solscan"
                                    </a>
                                </label>
                                <div class="readonly-field">
                                    <span class="readonly-value-mono">{move || form.get().escrow_address}</span>
                                    <span class="readonly-badge">"Locked"</span>
                                </div>
                                <span class="quiz-setting-hint">"On-chain escrow PDA — auto-filled after on-chain init"</span>
                            </div>
                        </Show>

                        <Show when=move || !form.get().organizer_wallet.is_empty() fallback=|| view! { <div></div> }>
                            <div class="quiz-setting-item">
                                <label class="quiz-field-label">"Organizer Wallet"</label>
                                <div class="readonly-field">
                                    <span class="readonly-value-mono">{move || form.get().organizer_wallet}</span>
                                    <span class="readonly-badge">"Locked"</span>
                                </div>
                                <span class="quiz-setting-hint event-form-hint-success">"Wallet locked — set by escrow panel"</span>
                            </div>
                        </Show>

                        <Show when=move || !form.get().on_chain_event_id.is_empty() && form.get().on_chain_event_id != "0" fallback=|| view! { <div></div> }>
                            <div class="quiz-setting-item">
                                <label class="quiz-field-label">"On-Chain Event ID"<span class="field-optional-badge">"Auto"</span></label>
                                <div class="readonly-field">
                                    <span class="readonly-value">{move || form.get().on_chain_event_id}</span>
                                    <span class="readonly-badge">"Locked"</span>
                                </div>
                                <span class="quiz-setting-hint">"Numeric ID for PDA seeds — auto-derived from slug"</span>
                            </div>
                        </Show>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Refund Deadline (hours)"</label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g. 168 (= 7 days)"
                                min="1"
                                step="1"
                                prop:value=move || form.get().refund_deadline_hours
                                on:input=move |ev| set_form.update(|f| f.refund_deadline_hours = event_target_value(&ev))
                            />
                            <Show
                                when=move || {
                                    let val = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                    form.get().deposit_enabled && val == 0
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-warning-xs">
                                    "Refund deadline must be at least 1 hour"
                                </div>
                            </Show>
                            <span class="quiz-setting-hint">"Hours after event end for refund deadline (default: 168 = 7 days)"</span>
                            // Visual timeline: show computed deadline date
                            <Show
                                when=move || {
                                    let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                    let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                    end_ms > 0 && hrs > 0
                                }
                                fallback=|| view! { <div></div> }
                            >
                                <div class="hint-success-sm">
                                    {move || {
                                        let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                        let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                        let deadline_ms = end_ms + (hrs as i64 * 3_600_000);
                                        let days = hrs / 24;
                                        let day_label = if days >= 7 {
                                            format!("{} days", days)
                                        } else if days > 0 {
                                            format!("{}d {}h", days, hrs % 24)
                                        } else {
                                            format!("{}h", hrs)
                                        };
                                        format!("Refund deadline: {} ({day_label} after event end)", format_date_display(deadline_ms))
                                    }}
                                </div>
                            </Show>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Max Refundable Deposits"</label>
                            <input
                                type="number"
                                class="quiz-number-input"
                                placeholder="e.g., 18"
                                min="0"
                                step="1"
                                prop:value=move || form.get().max_refundable_deposits
                                on:input=move |ev| set_form.update(|f| f.max_refundable_deposits = event_target_value(&ev))
                            />
                            <span class="quiz-setting-hint">"First N deposits get refund on check-in. Leave 0 or empty for unlimited. Deposits beyond this count are non-refundable."</span>
                        </div>
                    </div>

                    // ── Create Mode: Wallet Connect for combined Create + Escrow Init ──
                    <Show when=move || {
                        let f = form.get();
                        f.deposit_enabled && is_create
                    }>
                        <div class="panel-box-dashed event-form-escrow-panel">
                            <div class="panel-label">
                                "Escrow Setup"
                            </div>
                            <div class="panel-hint u-mb-sm">
                                "Connect your Solana wallet to initialize escrow when the event is created."
                            </div>

                            // Wallet not yet connected — show connect buttons
                            <Show when=move || create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                                <Show when=move || !detected_wallets.get().is_empty() fallback=|| view! {
                                    <div class="panel-hint">
                                        "No Solana wallets detected. Install Phantom or another wallet extension."
                                    </div>
                                }>
                                    <div class="flex-wrap-row">
                                        {move || detected_wallets.get().iter().map(|wn| {
                                            let wn_c = wn.clone();
                                            let set_wn = set_create_wallet_name;
                                            let set_wp = set_create_wallet_pk;
                                            let set_t = set_toast;
                                            view! {
                                                <button
                                                    class="btn btn-outline btn-sm"
                                                    on:click=move |_| {
                                                        let wn = wn_c.clone();
                                                        let set_wn = set_wn;
                                                        let set_wp = set_wp;
                                                        let set_t = set_t;
                                                        leptos::task::spawn_local(async move {
                                                            match super::escrow_init::connect_wallet_js(&wn).await {
                                                                crate::wallet_error::WalletResult::Success(pk) => {
                                                                    log::info!("[event-form] wallet connected: {} ({})", wn, &pk[..8.min(pk.len())]);
                                                                    set_wn.set(wn);
                                                                    set_wp.set(pk);
                                                                }
                                                                crate::wallet_error::WalletResult::Error(e) => {
                                                                    components::show_toast(&set_t, &crate::wallet_error::user_friendly_message(&e), components::ToastType::Error);
                                                                }
                                                                crate::wallet_error::WalletResult::UnknownFailure => {
                                                                    components::show_toast(&set_t, "Wallet connection failed", components::ToastType::Error);
                                                                }
                                                            }
                                                        });
                                                    }
                                                >
                                                    {format!("Connect {}", wn)}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Show>
                            </Show>

                            // Wallet connected — show confirmation
                            <Show when=move || !create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                                <div class="wallet-connected-bar event-form-wallet-bar-no-gap">
                                    <div class="wallet-info-left">
                                        <div class="wallet-label">
                                            {move || format!("{} connected", create_wallet_name.get())}
                                        </div>
                                        <div class="wallet-address-bold">
                                            {move || create_wallet_pk.get()}
                                        </div>
                                    </div>
                                    <button
                                        class="btn btn-outline btn-xs"
                                        on:click=move |_| {
                                            set_create_wallet_name.set(String::new());
                                            set_create_wallet_pk.set(String::new());
                                        }
                                    >
                                        "Disconnect"
                                    </button>
                                </div>
                                <div class="hint-success-sm event-form-hint-after-wallet">
                                    "Creating event will also initialize escrow on-chain (wallet signature required)."
                                </div>
                            </Show>
                        </div>
                    </Show>

                    // ── Escrow Management (Edit Mode Only) ──
                    <Show when=move || {
                        let f = form.get();
                        f.deposit_enabled && !editing_id.get().unwrap_or_default().is_empty()
                    }>
                        <super::escrow_init::EscrowInitPanel
                            event_id=editing_id.get().unwrap_or_default()
                            form=form
                            set_form=set_form
                            set_toast=set_toast
                        />
                    </Show>
                    </div>
                </div>
                </Show>

                // ── People ──
                <div class="form-section">
                    <div class="form-section-header" on:click=move |_| set_sec_people_open.update(|v| *v = !*v)>
                        <span class="form-section-icon form-section-icon-people"></span>
                        <span class="form-section-title">"People"</span>
                        <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                        <span class="form-section-toggle" class:form-section-toggle-open=move || sec_people_open.get()>"▼"</span>
                    </div>
                    <div class="form-section-body" class:form-section-body-hidden=move || !sec_people_open.get()>
                        <div class="quiz-settings-grid">
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Organizer Emails"</label>
                            <textarea
                                class="quiz-textarea quiz-textarea-sm"
                                placeholder="admin@example.com, organizer@example.com"
                                prop:value=move || form.get().organizer_emails
                                on:input=move |ev| set_form.update(|f| f.organizer_emails = event_target_value(&ev))
                            ></textarea>
                            <span class="quiz-setting-hint">"Comma-separated"</span>
                        </div>
                        <div class="quiz-setting-item">
                            <label class="quiz-field-label">"Staff Emails"</label>
                            <textarea
                                class="quiz-textarea quiz-textarea-sm"
                                placeholder="staff1@example.com, staff2@example.com"
                                prop:value=move || form.get().staff_emails
                                on:input=move |ev| set_form.update(|f| f.staff_emails = event_target_value(&ev))
                            ></textarea>
                            <span class="quiz-setting-hint">"Comma-separated"</span>
                        </div>
                    </div>
                    </div>
                </div>

                // ── Community Links ──
                <div class="form-section">
                    <div class="form-section-header" on:click=move |_| set_sec_community_open.update(|v| *v = !*v)>
                        <span class="form-section-icon form-section-icon-community"></span>
                        <span class="form-section-title">"Community Links"</span>
                        <span class="form-section-badge form-section-badge-optional">"Optional"</span>
                        <span class="form-section-toggle" class:form-section-toggle-open=move || sec_community_open.get()>"▼"</span>
                    </div>
                    <div class="form-section-body" class:form-section-body-hidden=move || !sec_community_open.get()>
                        <p class="quiz-setting-hint">
                            "Add links to your community channels (Discord, Telegram, X, Facebook, LINE). These appear on the event registration and ticket pages."
                        </p>
                        {move || {
                            let links = cl_links.get();
                            links.into_iter().enumerate().map(|(i, link)| {
                                let idx = i;
                                let platform = link.platform.clone();
                                let url = link.url.clone();
                                let label = link.label.clone();
                                view! {
                                    <div class="community-link-row">
                                        <select
                                            class="quiz-number-input community-link-platform"
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                set_cl_links.update(|links| {
                                                    if idx < links.len() {
                                                        links[idx].platform = val;
                                                    }
                                                });
                                            }
                                        >
                                            <option value="discord" selected=platform == "discord">"Discord"</option>
                                            <option value="telegram" selected=platform == "telegram">"Telegram"</option>
                                            <option value="x" selected=platform == "x">"X (Twitter)"</option>
                                            <option value="facebook" selected=platform == "facebook">"Facebook"</option>
                                            <option value="line" selected=platform == "line">"LINE"</option>
                                            <option value="website" selected=platform == "website">"Website"</option>
                                        </select>
                                        <input
                                            type="url"
                                            class="quiz-number-input community-link-url"
                                            placeholder="https://..."
                                            prop:value=url
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                set_cl_links.update(|links| {
                                                    if idx < links.len() {
                                                        links[idx].url = val;
                                                    }
                                                });
                                            }
                                        />
                                        <input
                                            type="text"
                                            class="quiz-number-input community-link-label"
                                            placeholder="Label (optional)"
                                            prop:value=label
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                set_cl_links.update(|links| {
                                                    if idx < links.len() {
                                                        links[idx].label = val;
                                                    }
                                                });
                                            }
                                        />
                                        <button
                                            class="btn btn-outline btn-xs community-link-remove"
                                            on:click=move |_| {
                                                set_cl_links.update(|links| {
                                                    if idx < links.len() {
                                                        links.remove(idx);
                                                    }
                                                });
                                            }
                                        >
                                            "×"
                                        </button>
                                    </div>
                                }
                            }).collect::<Vec<_>>()
                        }}
                        {move || {
                            let count = cl_links.get().len();
                            if count < 5 {
                                view! {
                                    <button
                                        class="btn btn-outline btn-sm community-link-add"
                                        on:click=move |_| add_community_link()
                                    >
                                        "+ Add Link"
                                    </button>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        }}
                    </div>
                </div>

                // ── Action Buttons ──
                <div class="form-actions-row">
                    <button
                        class="btn btn-primary"
                        on:click=handle_save
                        disabled=move || saving.get()
                    >
                        {move || {
                            if saving.get() {
                                "Saving...".to_string()
                            } else if !is_create {
                                "Update Event".to_string()
                            } else if !create_wallet_pk.get().is_empty() && form.get().deposit_enabled {
                                "Create Event + Initialize Escrow".to_string()
                            } else {
                                "Create Event".to_string()
                            }
                        }}
                    </button>
                    <button class="btn btn-outline" on:click=move |_| { stored_on_done.get_value()(); }>
                        "Cancel"
                    </button>
                    <Show
                        when=move || {
                            !is_create && !editing_id.get().unwrap_or_default().is_empty()
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <button
                            class="btn btn-outline btn-archive"
                            on:click=move |_| {
                                let aid = editing_id.get().unwrap_or_default();
                                let set_toast = set_toast;
                                let on_done_ref = stored_on_done.get_value().clone();
                                leptos::task::spawn_local(async move {
                                    match api::archive_event(&aid).await {
                                        Ok(data) => {
                                            components::show_toast(
                                                &set_toast,
                                                &format!("Event '{}' archived", data.name),
                                                components::ToastType::Success,
                                            );
                                            on_done_ref();
                                        }
                                        Err(e) => {
                                            log::error!("[event-form] archive failed: {e}");
                                            components::show_toast(
                                                &set_toast,
                                                &format!("Failed to archive: {e}"),
                                                components::ToastType::Error,
                                            );
                                        }
                                    }
                                });
                            }
                        >
                            "Archive Event"
                        </button>
                    </Show>
                </div>
            </div>
    }
}
