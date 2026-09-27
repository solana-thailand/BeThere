//! Admin dashboard sidebar: event selector, section navigation and the
//! Alt+digit shortcuts it advertises.

use leptos::prelude::*;

use super::types::{AdminSection, DashboardTab};
use crate::api::{self, AttendeeListItem, EventFormat};
use crate::icons::{Icon, IconName};

/// Signals the sidebar reads and writes; all owned by `Admin`.
#[derive(Clone, Copy)]
pub(super) struct SidebarState {
    pub(super) events_loading: ReadSignal<bool>,
    pub(super) events_list: ReadSignal<Vec<api::EventMeta>>,
    pub(super) active_event_id: ReadSignal<Option<String>>,
    pub(super) set_active_event_id: WriteSignal<Option<String>>,
    pub(super) set_refresh_counter: WriteSignal<u32>,
    pub(super) active_section: ReadSignal<AdminSection>,
    pub(super) set_active_section: WriteSignal<AdminSection>,
    pub(super) active_tab: ReadSignal<DashboardTab>,
    pub(super) set_active_tab: WriteSignal<DashboardTab>,
    pub(super) current_event_format: Memo<EventFormat>,
    pub(super) current_sheet_id: Memo<String>,
    pub(super) attendees: ReadSignal<Vec<AttendeeListItem>>,
}

/// Keyboard shortcuts for sidebar navigation (Alt+1…Alt+0).
pub(super) fn install_section_shortcuts(
    set_active_section: WriteSignal<AdminSection>,
    set_active_tab: WriteSignal<DashboardTab>,
) {
    Effect::new(move |_| {
        let handler = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                if ev.alt_key() {
                    match ev.key().as_str() {
                        "1" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Events);
                        }
                        "2" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Campaigns);
                        }
                        "3" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Quiz);
                        }
                        "4" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::FormBuilder);
                        }
                        "5" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Adventure);
                        }
                        "6" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Attendance);
                            set_active_tab.set(DashboardTab::InPerson);
                        }
                        "7" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Attendance);
                            set_active_tab.set(DashboardTab::Online);
                        }
                        "8" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Deposits);
                        }
                        "9" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Escrow);
                        }
                        "0" => {
                            ev.prevent_default();
                            set_active_section.set(AdminSection::Cancellation);
                        }
                        _ => {}
                    }
                }
            },
        );
        let window = web_sys::window().expect("no window");
        use wasm_bindgen::JsCast;
        let _ =
            window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        handler.forget();
    });
}

/// Render the admin sidebar.
pub(super) fn admin_sidebar(state: SidebarState) -> impl IntoView {
    let SidebarState {
        events_loading,
        events_list,
        active_event_id,
        set_active_event_id,
        set_refresh_counter,
        active_section,
        set_active_section,
        active_tab,
        set_active_tab,
        current_event_format,
        current_sheet_id,
        attendees,
    } = state;

    view! {
        <aside class="admin-sidebar">
            // Quick nav — Home link at top of sidebar for easy exit
            <div class="admin-sidebar-section admin-sidebar-topnav">
                <a href="/" class="admin-sidebar-item admin-sidebar-home" title="Back to home">
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
                            <polyline points="9 22 9 12 15 12 15 22"></polyline>
                        </svg>
                    </span>
                    "Home"
                </a>
            </div>

            // Event selector dropdown (always visible at top of sidebar)
            <Show when=move || !events_loading.get() && !events_list.get().is_empty() fallback=|| view! { <div></div> }>
                <div class="admin-sidebar-section">
                    <div class="admin-sidebar-heading">"Event"</div>
                    <crate::pages::admin_event_selector::AdminEventSelector
                        events_list=events_list
                        active_event_id=active_event_id
                        set_active_event_id=set_active_event_id
                        set_refresh_counter=set_refresh_counter
                    />
                </div>
            </Show>

            // ── Group 1: Event Setup (most important) ──
            <div class="admin-sidebar-group">
                <div class="admin-sidebar-group-label">"Event Setup"</div>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Events
                    on:click=move |_| set_active_section.set(AdminSection::Events)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect>
                            <line x1="16" y1="2" x2="16" y2="6"></line>
                            <line x1="8" y1="2" x2="8" y2="6"></line>
                            <line x1="3" y1="10" x2="21" y2="10"></line>
                        </svg>
                    </span>
                    "Manage Events"
                    <span class="admin-sidebar-kbd">"Alt+1"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Quiz
                    on:click=move |_| set_active_section.set(AdminSection::Quiz)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"></path>
                            <rect x="8" y="2" width="8" height="4" rx="1" ry="1"></rect>
                        </svg>
                    </span>
                    "Quiz"
                    <span class="admin-sidebar-kbd">"Alt+3"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::FormBuilder
                    on:click=move |_| set_active_section.set(AdminSection::FormBuilder)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                        </svg>
                    </span>
                    "Form Builder"
                    <span class="admin-sidebar-kbd">"Alt+4"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Adventure
                    on:click=move |_| set_active_section.set(AdminSection::Adventure)
                >
                    <span class="admin-sidebar-icon"><Icon icon=IconName::Crab class="icon-sm"/></span>
                    "Adventure"
                    <span class="admin-sidebar-kbd">"Alt+5"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Campaigns
                    on:click=move |_| set_active_section.set(AdminSection::Campaigns)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M12 2L2 7l10 5 10-5-10-5z"></path>
                            <path d="M2 17l10 5 10-5"></path>
                            <path d="M2 12l10 5 10-5"></path>
                        </svg>
                    </span>
                    "Campaigns"
                    <span class="admin-sidebar-kbd">"Alt+2"</span>
                </button>
            </div>

            // ── Group 2: Check-in (day-of-event) ──
            <div class="admin-sidebar-group">
                <div class="admin-sidebar-group-label">"Check-in"</div>
                <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <div></div> }>
                    <button
                        class="admin-sidebar-item"
                        class:active=move || active_section.get() == AdminSection::Attendance && active_tab.get() == DashboardTab::InPerson
                        on:click=move |_| {
                            set_active_section.set(AdminSection::Attendance);
                            set_active_tab.set(DashboardTab::InPerson);
                        }
                    >
                        <span class="admin-sidebar-icon">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
                                <circle cx="9" cy="7" r="4"></circle>
                                <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
                                <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
                            </svg>
                        </span>
                        "In-Person"
                        <span class="admin-sidebar-kbd">"Alt+6"</span>
                    </button>
                </Show>
                <Show when=move || current_event_format.get().has_online() fallback=|| view! { <div></div> }>
                    <button
                        class="admin-sidebar-item"
                        class:active=move || active_section.get() == AdminSection::Attendance && active_tab.get() == DashboardTab::Online
                        on:click=move |_| {
                            set_active_section.set(AdminSection::Attendance);
                            set_active_tab.set(DashboardTab::Online);
                        }
                    >
                        <span class="admin-sidebar-icon">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <circle cx="12" cy="12" r="10"></circle>
                                <line x1="2" y1="12" x2="22" y2="12"></line>
                                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                            </svg>
                        </span>
                        "Online"
                        <span class="admin-sidebar-kbd">"Alt+7"</span>
                    </button>
                </Show>
                // Google Sheet link — one per event (Sheet is shared by both tabs).
                // Hidden when no event selected or sheet_id is empty.
                <Show
                    when=move || !current_sheet_id.get().trim().is_empty()
                    fallback=|| view! { <span></span> }
                >
                    <a
                        class="admin-sidebar-item admin-sidebar-link-external"
                        href=move || crate::utils::google_sheet_url(&current_sheet_id.get())
                        target="_blank"
                        rel="noopener noreferrer"
                        title="Open this event's Google Sheet in a new tab"
                    >
                        <span class="admin-sidebar-icon">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                                <polyline points="14 2 14 8 20 8"></polyline>
                                <line x1="18" y1="13" x2="6" y2="13"></line>
                                <line x1="18" y1="17" x2="6" y2="17"></line>
                                <line x1="8" y1="9" x2="6" y2="9"></line>
                            </svg>
                        </span>
                        "View Google Sheet"
                        <span class="admin-sidebar-external-arrow">"↗"</span>
                    </a>
                </Show>
            </div>

            // ── Group 3: Payments (deposit & escrow) — only for events with in-person track ──
            <Show when=move || current_event_format.get().has_in_person() fallback=|| view! { <div></div> }>
            <div class="admin-sidebar-group">
                <div class="admin-sidebar-group-label">"Payments"</div>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Deposits
                    on:click=move |_| set_active_section.set(AdminSection::Deposits)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <line x1="12" y1="1" x2="12" y2="23"></line>
                            <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"></path>
                        </svg>
                    </span>
                    "Deposits & Refunds"
                    <span class="admin-sidebar-kbd">"Alt+8"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Escrow
                    on:click=move |_| set_active_section.set(AdminSection::Escrow)
                >
                    <span class="admin-sidebar-icon"><Icon icon=IconName::Lock class="icon-sm"/></span>
                    "Escrow"
                    <span class="admin-sidebar-kbd">"Alt+9"</span>
                </button>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Cancellation
                    on:click=move |_| set_active_section.set(AdminSection::Cancellation)
                >
                    <span class="admin-sidebar-icon">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="12" cy="12" r="10"></circle>
                            <line x1="15" y1="9" x2="9" y2="15"></line>
                            <line x1="9" y1="9" x2="15" y2="15"></line>
                        </svg>
                    </span>
                    "Cancellation"
                    <span class="admin-sidebar-kbd">"Alt+0"</span>
                </button>
            </div>
            </Show>

            // ── Group 4: Post-Event ──
            <div class="admin-sidebar-group">
                <div class="admin-sidebar-group-label">"Post-Event"</div>
                <button
                    class="admin-sidebar-item"
                    class:active=move || active_section.get() == AdminSection::Feedback
                    on:click=move |_| set_active_section.set(AdminSection::Feedback)
                >
                    <span class="admin-sidebar-icon"><Icon icon=IconName::Star class="icon-sm"/></span>
                    "Feedback & Survey"
                </button>
            </div>

            // Quick stats at bottom of sidebar
            <div class="admin-sidebar-stats">
                {move || {
                    if active_event_id.get().is_none() {
                        view! {
                            <div class="admin-sidebar-stats-empty">
                                "Select an event to see stats"
                            </div>
                        }.into_any()
                    } else {
                        let attendees_list = attendees.get();
                        let tab_attendees: Vec<_> = attendees_list.iter()
                            .filter(|a| active_tab.get().matches(&a.participation_type))
                            .collect();
                        let total = tab_attendees.len();
                        let checked_in = tab_attendees.iter().filter(|a| a.checked_in_at.is_some()).count();
                        let remaining = total.saturating_sub(checked_in);
                        view! {
                            <div class="admin-sidebar-stat">
                                <span class="admin-sidebar-stat-value">{total}</span>
                                <span class="admin-sidebar-stat-label">"Total"</span>
                            </div>
                            <div class="admin-sidebar-stat">
                                <span class="admin-sidebar-stat-value admin-stat-value-success">{checked_in}</span>
                                <span class="admin-sidebar-stat-label">"Checked In"</span>
                            </div>
                            <div class="admin-sidebar-stat">
                                <span class="admin-sidebar-stat-value admin-stat-value-warning">{remaining}</span>
                                <span class="admin-sidebar-stat-label">"Remaining"</span>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </aside>
    }
}
