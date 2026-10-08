//! Save handler: validate, then create or update (plus escrow init).

use event_checkin_domain::models::event::{
    DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME, normalize_sheet_name, normalize_sponsors,
};
use event_checkin_domain::money::{check_usdc_deposit, parse_usdc_atomic};
use leptos::prelude::*;

use super::ctx::FormCtx;
use super::types::{OnDone, parse_date_to_ms, parse_emails};
use crate::{api, components};

/// Validates the form and creates or updates the event.
pub(super) fn save_event(ctx: FormCtx, on_done: OnDone) {
    let FormCtx {
        form,
        set_toast,
        editing_id,
        is_create,
        set_saving,
        slug_taken,
        cl_links,
        sponsors,
        create_wallet_name,
        create_wallet_pk,
        ..
    } = ctx;
    let current_form = form.get();

    // Validate required fields
    if current_form.name.trim().is_empty() {
        components::show_toast(
            &set_toast,
            "Event name is required",
            components::ToastType::Error,
        );
        return;
    }
    if current_form.slug.trim().is_empty() {
        components::show_toast(
            &set_toast,
            "Event slug is required",
            components::ToastType::Error,
        );
        return;
    }
    // Check slug availability (client-side against loaded events)
    if slug_taken.get() {
        components::show_toast(
            &set_toast,
            "This slug is already taken by another event",
            components::ToastType::Error,
        );
        return;
    }
    if current_form.sheet_id.trim().is_empty() {
        components::show_toast(
            &set_toast,
            "Google Sheet ID is required",
            components::ToastType::Error,
        );
        return;
    }
    // Tab names reach a Google Sheets A1 range on every sync. The backend
    // rejects a name Google would refuse to create; check here too so the
    // organiser sees which field is wrong without a round trip.
    for (label, raw, fallback) in [
        (
            "Attendee tab name",
            &current_form.sheet_name,
            DEFAULT_ATTENDEE_SHEET_NAME,
        ),
        (
            "Staff tab name",
            &current_form.staff_sheet_name,
            DEFAULT_STAFF_SHEET_NAME,
        ),
    ] {
        if let Err(e) = normalize_sheet_name(raw, fallback) {
            components::show_toast(
                &set_toast,
                &format!("{label}: {e}"),
                components::ToastType::Error,
            );
            return;
        }
    }
    // Same check the backend runs; the normalised list (trimmed, blank rows
    // dropped) is what gets sent.
    let sponsor_list = match normalize_sponsors(&sponsors.get()) {
        Ok(list) => list,
        Err(e) => {
            components::show_toast(
                &set_toast,
                &format!("Sponsors: {e}"),
                components::ToastType::Error,
            );
            return;
        }
    };

    // Validate schedule — backend requires positive start_ms and end > start
    let time_tba = current_form.time_tba;
    let start_ms = parse_date_to_ms(&current_form.event_start).unwrap_or(0);
    let end_ms = parse_date_to_ms(&current_form.event_end).unwrap_or(0);
    if start_ms <= 0 {
        components::show_toast(
            &set_toast,
            "Event start date is required",
            components::ToastType::Error,
        );
        return;
    }
    if !time_tba && end_ms <= 0 {
        components::show_toast(
            &set_toast,
            "Event end date is required",
            components::ToastType::Error,
        );
        return;
    }
    if !time_tba && end_ms <= start_ms {
        components::show_toast(
            &set_toast,
            "Event end must be after event start",
            components::ToastType::Error,
        );
        return;
    }
    // TBA mode: default end_ms to start_ms + 24h if not set
    let end_ms = if time_tba && end_ms <= 0 {
        start_ms + 86_400_000
    } else {
        end_ms
    };

    // Validate deposit fields when deposit is enabled
    if current_form.deposit_enabled {
        let usdc_check = check_usdc_deposit(&current_form.deposit_amount_usdc);
        let Some(usdc_atomic) = usdc_check.atomic() else {
            let message = usdc_check.error_message().unwrap_or_default();
            components::show_toast(&set_toast, message, components::ToastType::Error);
            return;
        };
        let thb_val = current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0);

        // At least one deposit amount must be set
        if usdc_atomic == 0 && thb_val == 0 {
            components::show_toast(
                &set_toast,
                "At least one deposit amount (USDC or THB) is required when deposit is enabled",
                components::ToastType::Error,
            );
            return;
        }

        // THB amount set but no PromptPay ID — QR generation will fail
        if thb_val > 0 && current_form.promptpay_id.trim().is_empty() {
            components::show_toast(
                &set_toast,
                "PromptPay ID is required when THB amount is set",
                components::ToastType::Error,
            );
            return;
        }

        // In Create mode with USDC deposit > 0, wallet connection is required
        if is_create && usdc_atomic > 0 && create_wallet_pk.get().is_empty() {
            components::show_toast(
                &set_toast,
                "Connect your Solana wallet to create event with USDC deposit escrow",
                components::ToastType::Error,
            );
            return;
        }

        // Escrow init requires USDC amount — check early when wallet is connected
        let do_escrow_init =
            !create_wallet_pk.get().is_empty() && !create_wallet_name.get().is_empty();
        if do_escrow_init && usdc_atomic == 0 {
            components::show_toast(
                &set_toast,
                "USDC deposit amount is required to initialize on-chain escrow",
                components::ToastType::Error,
            );
            return;
        }

        let deadline_hrs = current_form
            .refund_deadline_hours
            .parse::<u32>()
            .unwrap_or(0);
        if deadline_hrs == 0 {
            components::show_toast(
                &set_toast,
                "Refund deadline must be at least 1 hour",
                components::ToastType::Error,
            );
            return;
        }
    }

    // The share card (`.issues/183`), drawn after a successful save.
    let card = crate::utils::og_card::CardEvent {
        name: current_form.name.trim().to_string(),
        slug: current_form.slug.trim().to_string(),
        start_ms,
        time_tba,
        location: current_form.location.trim().to_string(),
        poster_url: current_form.poster_url.trim().to_string(),
    };

    set_saving.set(true);

    if is_create {
        let body = api::CreateEventBody {
            name: current_form.name.trim().to_string(),
            slug: current_form.slug.trim().to_string(),
            tagline: current_form.tagline.trim().to_string(),
            description: current_form.description.trim().to_string(),
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
            deposit_amount_usdc: parse_usdc_atomic(&current_form.deposit_amount_usdc).unwrap_or(0),
            deposit_amount_thb: current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0),
            promptpay_id: current_form.promptpay_id.trim().to_string(),
            escrow_address: current_form.escrow_address.trim().to_string(),
            organizer_wallet: if create_wallet_pk.get().is_empty() {
                current_form.organizer_wallet.trim().to_string()
            } else {
                create_wallet_pk.get()
            },
            on_chain_event_id: current_form.on_chain_event_id.parse::<u64>().unwrap_or(0),
            refund_deadline_hours: current_form
                .refund_deadline_hours
                .parse::<u32>()
                .unwrap_or(0),
            max_refundable_deposits: current_form
                .max_refundable_deposits
                .parse::<u32>()
                .unwrap_or(0),
            event_format: current_form.event_format.clone(),
            require_contact_info: current_form.require_contact_info,
            require_photo_consent: current_form.require_photo_consent,
            time_tba,
            location: if current_form.location.trim().is_empty() {
                None
            } else {
                Some(current_form.location.trim().to_string())
            },
            location_map_url: current_form.location_map_url.trim().to_string(),
            // A brand-new event starts with nobody waived; the list is
            // managed from the edit form once the event exists.
            comp_emails: Vec::new(),
            video_url: current_form.video_url.trim().to_string(),
            in_person_capacity: current_form.in_person_capacity.trim().parse::<u32>().ok(),
            online_capacity: current_form.online_capacity.trim().parse::<u32>().ok(),
            online_open_mode: current_form.online_open_mode.clone(),
            online_registration_open: current_form.online_registration_open,
            deposit_deadline_hours: current_form
                .deposit_deadline_hours
                .trim()
                .parse::<u32>()
                .ok(),
            visibility: current_form.visibility.clone(),
            community_links: cl_links.get(),
            sponsors: sponsor_list.clone(),
            ticket_note_in_person: current_form.ticket_note_in_person.clone(),
            ticket_note_online: current_form.ticket_note_online.clone(),
            postponed_note: current_form.postponed_note.clone(),
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

            // Never blocks the save: drawn and uploaded in the background.
            super::og_card::refresh(created.id.clone(), card);

            // Step 2: Initialize escrow on-chain (if wallet connected + deposit enabled)
            if do_escrow_init {
                log::info!(
                    "[event-form] initializing escrow for event {}...",
                    created.id
                );
                let req = api::InitEscrowRequest {
                    event_id: created.id.clone(),
                };
                match api::init_escrow(&req).await {
                    Ok(resp) => {
                        // SEC-014: Verify wallet cluster matches expected network.
                        let expected_cluster = crate::utils::get_cluster();
                        if let Err(cluster_err) =
                            crate::pages::escrow_init::check_wallet_cluster(&wn, &expected_cluster)
                                .await
                        {
                            let cluster_err = cluster_err.message(crate::i18n::Locale::en);
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
                        match crate::pages::escrow_init::simulate_transaction_js(
                            &wn,
                            &resp.transaction,
                        )
                        .await
                        {
                            Ok(sim) if sim.ok => {}
                            Ok(sim) => {
                                let err_msg =
                                    sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                log::error!("[event-form] escrow simulation failed: {err_msg}");
                                components::show_toast(
                                    &set_toast,
                                    &format!("Transaction would fail: {err_msg}"),
                                    components::ToastType::Error,
                                );
                                set_saving.set(false);
                                return;
                            }
                            Err(e) => {
                                log::warn!("[event-form] simulate error (not blocking): {e}");
                            }
                        }

                        match crate::pages::escrow_init::sign_and_send_tx_js(&wn, &resp.transaction)
                            .await
                        {
                            crate::wallet_error::WalletResult::Success(signature) => {
                                log::info!("[event-form] escrow TX confirmed: {}", signature);
                                // Update the event with escrow fields from the response
                                let update_body = api::UpdateEventBody {
                                    escrow_address: Some(resp.escrow_address.clone()),
                                    escrow_status: Some(api::EscrowStatus::Initialized),
                                    organizer_wallet: Some(pk.clone()),
                                    on_chain_event_id: Some(resp.on_chain_event_id),
                                    expected_updated_at: if created.updated_at.is_empty() {
                                        None
                                    } else {
                                        Some(created.updated_at.clone())
                                    },
                                    ..Default::default()
                                };
                                if let Err(e) = api::update_event(&created.id, &update_body).await {
                                    log::warn!("[event-form] failed to save escrow fields: {e}");
                                }
                                components::show_mutation_toast(
                                    &set_toast,
                                    &format!(
                                        "Event '{}' created + escrow initialized",
                                        created.name
                                    ),
                                    &created.warnings,
                                );
                            }
                            crate::wallet_error::WalletResult::Error(e) => {
                                let msg = crate::wallet_error::user_friendly_message(
                                    &e,
                                    crate::i18n::Locale::en,
                                );
                                log::error!(
                                    "[event-form] escrow TX error: code={:?} msg={}",
                                    e.code,
                                    e.raw_message
                                );
                                components::show_toast(
                                    &set_toast,
                                    &format!(
                                        "Event '{}' created, but escrow failed: {}. Edit event to retry.",
                                        created.name, msg
                                    ),
                                    components::ToastType::Warning,
                                );
                            }
                            crate::wallet_error::WalletResult::UnknownFailure => {
                                log::error!("[event-form] escrow TX rejected by wallet");
                                components::show_toast(
                                    &set_toast,
                                    &format!(
                                        "Event '{}' created, but escrow TX failed. Edit event to retry.",
                                        created.name
                                    ),
                                    components::ToastType::Warning,
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[event-form] init_escrow failed: {e}");
                        components::show_toast(
                            &set_toast,
                            &format!(
                                "Event '{}' created, but escrow init failed: {e}. Edit event to retry.",
                                created.name
                            ),
                            components::ToastType::Warning,
                        );
                    }
                }
            } else {
                components::show_mutation_toast(
                    &set_toast,
                    &format!("Event '{}' created", created.name),
                    &created.warnings,
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
            description: Some(current_form.description.trim().to_string()),
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
            nft_description_template: Some(
                current_form.nft_description_template.trim().to_string(),
            ),
            merkle_tree: Some(current_form.merkle_tree.trim().to_string()),
            claim_base_url: Some(current_form.claim_base_url.trim().to_string()),
            organizer_emails: Some(parse_emails(&current_form.organizer_emails)),
            staff_emails: Some(parse_emails(&current_form.staff_emails)),
            deposit_enabled: Some(current_form.deposit_enabled),
            deposit_amount_usdc: Some(
                parse_usdc_atomic(&current_form.deposit_amount_usdc).unwrap_or(0),
            ),
            deposit_amount_thb: Some(current_form.deposit_amount_thb.parse::<u64>().unwrap_or(0)),
            promptpay_id: Some(current_form.promptpay_id.trim().to_string()),
            escrow_address: Some(current_form.escrow_address.trim().to_string()),
            escrow_status: None, // not updated via general form — escrow panel manages this
            organizer_wallet: Some(current_form.organizer_wallet.trim().to_string()),
            on_chain_event_id: Some(current_form.on_chain_event_id.parse::<u64>().unwrap_or(0)),
            refund_deadline_hours: Some(
                current_form
                    .refund_deadline_hours
                    .parse::<u32>()
                    .unwrap_or(0),
            ),
            max_refundable_deposits: Some(
                current_form
                    .max_refundable_deposits
                    .parse::<u32>()
                    .unwrap_or(0),
            ),
            expected_updated_at: if current_form.updated_at.is_empty() {
                None
            } else {
                Some(current_form.updated_at.clone())
            },
            event_format: Some(current_form.event_format.clone()),
            require_contact_info: Some(current_form.require_contact_info),
            require_photo_consent: Some(current_form.require_photo_consent),
            time_tba: Some(time_tba),
            location: if current_form.location.trim().is_empty() {
                None
            } else {
                Some(current_form.location.trim().to_string())
            },
            // Always sent so clearing the field removes the link.
            location_map_url: Some(current_form.location_map_url.trim().to_string()),
            // Always sent, so emptying the box really does clear the list.
            // Split on newlines AND commas because people paste both.
            comp_emails: Some(
                current_form
                    .comp_emails
                    .split(['\n', ','])
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            video_url: Some(current_form.video_url.trim().to_string()),
            in_person_capacity: Some(current_form.in_person_capacity.trim().parse::<u32>().ok()),
            online_capacity: Some(current_form.online_capacity.trim().parse::<u32>().ok()),
            online_open_mode: Some(current_form.online_open_mode.clone()),
            online_registration_open: Some(current_form.online_registration_open),
            deposit_deadline_hours: Some(
                current_form
                    .deposit_deadline_hours
                    .trim()
                    .parse::<u32>()
                    .ok(),
            ),
            visibility: Some(current_form.visibility.clone()),
            community_links: Some(cl_links.get()),
            // Always sent, so removing every row removes the logo row.
            sponsors: Some(sponsor_list.clone()),
            ticket_note_in_person: Some(current_form.ticket_note_in_person.clone()),
            ticket_note_online: Some(current_form.ticket_note_online.clone()),
            // Always sent, so emptying the box un-postpones the event.
            postponed_note: Some(current_form.postponed_note.clone()),
            calendar_subscribe_url: Some(current_form.calendar_subscribe_url.trim().to_string()),
        };

        leptos::task::spawn_local(async move {
            match api::update_event(&eid, &body).await {
                Ok(data) => {
                    // Never blocks the save: drawn and uploaded in the background.
                    super::og_card::refresh(eid.clone(), card);
                    components::show_mutation_toast(
                        &set_toast,
                        &format!("Event '{}' updated", data.name),
                        &data.warnings,
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
}
