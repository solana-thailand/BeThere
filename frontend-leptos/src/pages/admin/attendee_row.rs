//! One row of the admin attendee roster: badges, meta line and the
//! per-attendee actions.
//!
//! A plain function, not a component, on purpose: it runs inside the list's
//! reactive closure, so the signals it reads while building (selection,
//! in-flight ids, delete confirmation) re-render the list as they did when
//! this was inline.

use std::collections::HashSet;

use leptos::prelude::*;

use super::actions::{spawn_apply_credit, spawn_delete_attendee, spawn_participation_switch};
use super::types::{AdminSection, Notify, deposit_badge_for, is_vip_ticket};
use crate::api::AttendeeListItem;
use crate::components::{self, ToastType};
use crate::pages::admin_attendance_answer::AnswerPicker;
use crate::pages::admin_duplicate_hint::duplicate_hint;
use crate::utils;

/// Signals a roster row reads and writes; all owned by `Admin`.
#[derive(Clone, Copy)]
pub(super) struct RowCtx {
    pub(super) active_event_id: ReadSignal<Option<String>>,
    pub(super) deposit_enabled: Memo<bool>,
    pub(super) set_selected_ids: WriteSignal<HashSet<String>>,
    pub(super) switching_ids: ReadSignal<HashSet<String>>,
    pub(super) set_switching_ids: WriteSignal<HashSet<String>>,
    pub(super) confirm_delete_id: ReadSignal<Option<String>>,
    pub(super) set_confirm_delete_id: WriteSignal<Option<String>>,
    pub(super) deleting_ids: ReadSignal<HashSet<String>>,
    pub(super) set_deleting_ids: WriteSignal<HashSet<String>>,
    pub(super) set_pending_record_slip: WriteSignal<Option<String>>,
    pub(super) set_active_section: WriteSignal<AdminSection>,
    pub(super) notify: Notify,
}

/// Render one roster row.
pub(super) fn attendee_row(
    attendee: &AttendeeListItem,
    is_selected: bool,
    ctx: RowCtx,
) -> impl IntoView {
    let RowCtx {
        active_event_id,
        deposit_enabled: current_deposit_enabled,
        set_selected_ids,
        switching_ids,
        set_switching_ids,
        confirm_delete_id,
        set_confirm_delete_id,
        deleting_ids,
        set_deleting_ids,
        set_pending_record_slip: set_pending_record_slip_attendee,
        set_active_section,
        notify,
    } = ctx;
    let set_toast = notify.set_toast;
    let set_refresh_counter = notify.set_refresh_counter;
    let event_id_for_delete = active_event_id;

    let is_checked_in = attendee.checked_in_at.is_some();
    let is_attendee_in_person = utils::is_in_person(&attendee.participation_type);
    let is_vip = is_vip_ticket(&attendee.ticket_name);
    let is_walkin = attendee.ticket_name.eq_ignore_ascii_case("Walk-in");
    let api_id = attendee.api_id.clone();
    let delete_id = api_id.clone();
    // Dedicated clone for the participation-toggle children
    // closure (Fn) — avoids moving api_id out of the
    // environment, which would break other closures.
    let switch_display_id = api_id.clone();
    // Owned id for the attendance-answer picker (migration 0052).
    let answer_id = api_id.clone();
    let switch_click_id = api_id.clone();
    // Clone for the deep-link "Record slip" button — fires
    // the cross-section modal trigger. Same Fn-closure
    // reasoning as the participation-toggle clones above.
    let record_slip_id = api_id.clone();
    let badge_class = if is_checked_in {
        "badge badge-success"
    } else {
        "badge badge-warning"
    };
    let badge_text = if is_checked_in {
        "Checked In"
    } else {
        "Pending"
    };
    let participation = utils::get_participation_badge(&attendee.participation_type);
    let p_class = participation.css_class.to_string();
    let p_label = participation.label;
    let name = attendee.name.clone();
    let email = attendee.email.clone();
    let ticket = attendee.ticket_name.clone();
    let has_ticket = !ticket.is_empty();
    let time_ago_str = attendee
        .checked_in_at
        .as_deref()
        .map(utils::time_ago)
        .unwrap_or_default();
    let has_time_ago = is_checked_in && !time_ago_str.is_empty();
    let checked_in_by_suffix = attendee.checked_in_by.as_ref().map_or(String::new(), |by| {
        if by.is_empty() {
            String::new()
        } else {
            format!(" by {by}")
        }
    });
    let deposit_link = match active_event_id.get() {
        Some(ref eid) => format!("/deposit/{api_id}?event_id={eid}"),
        None => format!("/deposit/{api_id}"),
    };
    let ticket_link = match active_event_id.get() {
        Some(ref eid) => format!("/ticket/{api_id}?event_id={eid}"),
        None => format!("/ticket/{api_id}"),
    };
    // Participation-aware status badges
    let has_nft = attendee.nft_proof_url.is_some();
    let nft_url = attendee.nft_proof_url.clone();

    // For in-person: deposit/refund badges.
    //
    // Order matters and is settled-first: a
    // deposit that is refunded, comped or
    // credit-covered is DONE, and saying
    // "pending" about any of them sends an
    // organizer chasing money nobody owes.
    //
    // `.issues/137`: this used to read only
    // `deposit_amount` (USDC) and
    // `deposit_verified` (derived from
    // `attendees.deposit_status`) — and nothing
    // in the THB flow writes either, so staff
    // comps and credit registrations showed
    // "Deposit pending" forever. The `thb_*`
    // fields are the roster's first look at
    // `thb_deposits`.
    let deposit_badge = deposit_badge_for(
        attendee,
        is_attendee_in_person,
        current_deposit_enabled.get(),
    );

    // For online: show claim status when no deposit flow
    let claim_badge = if !is_attendee_in_person && has_nft {
        Some(("badge badge-success", "Claimed \u{2713}"))
    } else if !is_attendee_in_person && !is_checked_in {
        Some(("badge badge-info", "Registered"))
    } else {
        None
    };

    // "Apply Credit" is offered when an in-person attendee holds rolling
    // THB credit and has not completed/refunded a deposit — the backend
    // spends it (only if sufficient) and writes a covered deposit.
    // NOTE: deposit_amount is USDC-only, so it can't detect THB-stuck rows;
    // credit_thb (annotated by the list handler) is the correct gate.
    let credit_thb = attendee.credit_thb;
    let has_credit = credit_thb > 0;
    let can_apply_credit = is_attendee_in_person
        && has_credit
        && attendee.deposit_verified.as_deref() != Some("true")
        && attendee.refund_status.is_none();
    let apply_credit_id_click = attendee.api_id.clone();
    let apply_credit_id_disabled = attendee.api_id.clone();
    let duplicate_badge = duplicate_hint(&attendee.possible_duplicates);

    view! {
        <div class="attendee-item" class:vip=is_vip class:selected=is_selected>
            // Row 1: checkbox + name + badges + status indicators
            <div class="attendee-row-top">
                <button
                    class=format!("attendee-checkbox{}", if is_selected { " checked" } else { "" })
                    on:click=move |_| set_selected_ids.update(|ids| {
                        if ids.contains(&api_id) { ids.remove(&api_id); } else { ids.insert(api_id.clone()); }
                    })
                    disabled=is_checked_in
                >
                    {if is_selected { "✓" } else { "" }}
                </button>
                <div class="attendee-name">{name.clone()}</div>
                <span class=p_class.clone()>{p_label.clone()}</span>
                <span class=badge_class>{badge_text}</span>
                <Show
                    when=move || has_ticket && is_vip
                    fallback=|| view! { <span></span> }
                >
                    <span class="vip-badge">"VIP"</span>
                </Show>
                <Show
                    when=move || has_ticket && is_walkin
                    fallback=|| view! { <span></span> }
                >
                    <span class="walkin-badge">"Walk-in"</span>
                </Show>
                <Show
                    when=move || deposit_badge.is_some()
                    fallback=|| view! { <span></span> }
                >
                    {
                        let (cls, txt) = deposit_badge.unwrap_or(("", ""));
                        view! { <span class=cls.to_string()>{txt}</span> }
                    }
                </Show>
                <Show
                    when=move || claim_badge.is_some()
                    fallback=|| view! { <span></span> }
                >
                    {
                        let (cls, txt) = claim_badge.unwrap_or(("", ""));
                        view! { <span class=cls.to_string()>{txt}</span> }
                    }
                </Show>
                // Rolling deposit credit the attendee holds — makes credit
                // visible at the row level (previously only on the Held tab).
                <Show
                    when=move || has_credit
                    fallback=|| view! { <span></span> }
                >
                    <span class="badge badge-info" title="Rolling deposit credit available — use the Apply Credit button to cover this event">
                        {format!("\u{0e3f}{credit_thb} credit")}
                    </span>
                </Show>
                <Show
                    when=move || has_nft
                    fallback=|| view! { <span></span> }
                >
                    {
                        let nft_href = nft_url.clone().unwrap_or_default();
                        view! {
                            <a
                                href=nft_href
                                target="_blank"
                                rel="noopener noreferrer"
                                class="badge badge-nft"
                                title="View NFT"
                            >
                                "NFT ✦"
                            </a>
                        }
                    }
                </Show>
                {duplicate_badge}
            </div>
            // Row 2: email + ticket + time ago + action buttons
            <div class="attendee-row-bottom">
                <div class="attendee-meta">
                    <span class="attendee-email-inline">{email.clone()}</span>
                    <Show
                        when=move || has_ticket
                        fallback=|| view! { <span></span> }
                    >
                        <span class="admin-ticket-tag">{ticket.clone()}</span>
                    </Show>
                    <Show
                        when=move || has_time_ago
                        fallback=|| view! { <span></span> }
                    >
                        <span class="admin-time-ago-inline">
                            {time_ago_str.clone()}{checked_in_by_suffix.clone()}
                        </span>
                    </Show>
                </div>
                <div class="attendee-actions">
                    // Deposit button — only for in-person attendees
                    <Show
                        when=move || is_attendee_in_person
                        fallback=|| view! { <span></span> }
                    >
                        <a
                            href=deposit_link.clone()
                            class="btn btn-outline btn-xs btn-xs-override"
                            title="Deposit page"
                        >
                            "Deposit"
                        </a>
                    </Show>
                    // Record slip on behalf of attendee — deep-links into
                    // the Deposits section's Record-Slip modal. Use case:
                    // attendee sent the slip via LINE/email and cannot upload
                    // themselves (JWT expired, browser bug, etc.).
                    // Gated on deposit_enabled for the current event.
                    <Show
                        when=move || current_deposit_enabled.get()
                        fallback=|| view! { <span></span> }
                    >
                        <button
                            class="btn btn-outline btn-xs btn-xs-override"
                            title="Record a slip on behalf of this attendee (they sent it via LINE/email and cannot upload themselves). Opens the deposit modal pre-filled."
                            on:click={
                                let id = record_slip_id.clone();
                                let set_pending = set_pending_record_slip_attendee;
                                let set_section = set_active_section;
                                move |_| {
                                    set_pending.set(Some(id.clone()));
                                    set_section.set(AdminSection::Deposits);
                                }
                            }
                        >
                            "Record Slip"
                        </button>
                    </Show>
                    // Apply Credit — complete a registration stuck at the
                    // deposit step by spending the attendee's rolling credit.
                    // Shown only for the stuck state (in-person, registered,
                    // no deposit). The backend spends credit only if it covers
                    // the deposit, else surfaces a toast error.
                    <Show
                        when=move || current_deposit_enabled.get() && can_apply_credit
                        fallback=|| view! { <span></span> }
                    >
                        <button
                            class="btn btn-outline btn-xs btn-xs-override"
                            disabled=switching_ids.get().contains(&apply_credit_id_disabled)
                            title="Apply this attendee's rolling deposit credit to cover this event (completes a registration stuck at the deposit step). No effect if they have insufficient credit."
                            on:click={
                                let aid = apply_credit_id_click.clone();
                                let eid = event_id_for_delete.get();
                                move |_| {
                                    spawn_apply_credit(aid.clone(), eid.clone(), set_switching_ids, notify);
                                }
                            }
                        >
                            "Apply Credit"
                        </button>
                    </Show>
                    <AnswerPicker
                        attendee_id=answer_id.clone()
                        event_id=event_id_for_delete.get_untracked()
                        current=attendee.attendance_answer
                        set_toast=set_toast
                        set_refresh_counter=set_refresh_counter
                    />
                    // Participation-type toggle — flip In-Person ⇄ Online.
                    // Use case: attendee chose deposit/in-person but confirmed
                    // out-of-band they'll attend online (or vice-versa).
                    // Hidden for walk-ins (their participation_type is 'walkin').
                    <Show
                        when=move || !is_walkin
                        fallback=|| view! { <span></span> }
                    >
                        <button
                            class="btn btn-outline btn-xs btn-xs-override admin-participation-toggle"
                            disabled=switching_ids.get().contains(&switch_display_id)
                            title=if is_attendee_in_person {
                                "Switch to Online (confirmed out-of-band)"
                            } else {
                                "Switch to In-Person"
                            }
                            on:click={
                                let switch_id = switch_click_id.clone();
                                let target_mode = if is_attendee_in_person { "Online" } else { "In-Person" };
                                let eid = event_id_for_delete.get();
                                move |_| {
                                    spawn_participation_switch(
                                        switch_id.clone(),
                                        eid.clone(),
                                        target_mode,
                                        set_switching_ids,
                                        notify,
                                    );
                                }
                            }
                        >
                            // Static label (computed once, like the Delete button).
                            // The whole list re-renders via refresh_counter after success,
                            // so no reactive closure is needed here.
                            {if is_attendee_in_person { "→ Online" } else { "→ In-Person" }}
                        </button>
                    </Show>
                    <button
                        class="btn btn-outline btn-xs btn-xs-override"
                        title="Copy ticket link"
                        on:click={
                            let ticket_link = ticket_link.clone();
                            move |_| {
                                let full_url = format!("{}{}",
                                    web_sys::window()
                                        .and_then(|w| w.location().origin().ok())
                                        .unwrap_or_default(),
                                    ticket_link
                                );
                                let clipboard = web_sys::window()
                                    .unwrap()
                                    .navigator()
                                    .clipboard();
                                let _ = clipboard.write_text(&full_url);
                                components::show_toast(
                                    &set_toast,
                                    "Ticket link copied!",
                                    ToastType::Success,
                                );
                            }
                        }
                    >
                        "Ticket"
                    </button>
                    <button
                        class={
                            let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                            let is_deleting = deleting_ids.get().contains(&delete_id);
                            if is_deleting {
                                "btn btn-xs btn-xs-override".to_string()
                            } else if is_confirming {
                                "btn btn-confirm-danger btn-xs btn-xs-override".to_string()
                            } else {
                                "btn btn-danger btn-xs btn-xs-override".to_string()
                            }
                        }
                        disabled=deleting_ids.get().contains(&delete_id)
                        title="Delete attendee"
                        on:click={
                            let delete_id = delete_id.clone();
                            move |_| {
                                let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                                if is_confirming {
                                    set_confirm_delete_id.set(None);
                                    spawn_delete_attendee(
                                        delete_id.clone(),
                                        event_id_for_delete.get(),
                                        set_deleting_ids,
                                        notify,
                                    );
                                } else {
                                    set_confirm_delete_id.set(Some(delete_id.clone()));
                                    let set_confirm = set_confirm_delete_id;
                                    gloo_timers::callback::Timeout::new(3000, move || {
                                        set_confirm.set(None);
                                    }).forget();
                                }
                            }
                        }
                    >
                        {
                            let is_confirming = confirm_delete_id.get().as_deref() == Some(&delete_id);
                            let is_deleting = deleting_ids.get().contains(&delete_id);
                            if is_deleting { "Deleting..." } else if is_confirming { "⚠ Confirm?" } else { "Delete" }
                        }
                    </button>
                </div>
            </div>
        </div>
    }
}
