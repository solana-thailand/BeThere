//! Developer profile editor page.
//!
//! Allows authenticated attendees to view and edit their developer profile:
//! display name, social handles, interests, tech stack, and role.

use leptos::prelude::*;
use leptos_meta::Title;
use wasm_bindgen::prelude::*;

use crate::api::{
    self, DeveloperProfile, UpdateProfileBody, INTEREST_OPTIONS, ROLE_OPTIONS,
};
use crate::icons::{Icon, IconName};

// ---------------------------------------------------------------------------
// JS Interop
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum ProfileState {
    /// Checking auth state.
    LoadingAuth,
    /// Fetching profile from API.
    LoadingProfile,
    /// Profile loaded, ready to edit.
    Editing(DeveloperProfile),
    /// Saving profile to API.
    Saving(DeveloperProfile),
    /// Save succeeded.
    Saved(DeveloperProfile),
    /// Error state.
    Error(String),
}

impl ProfileState {
    fn profile(&self) -> Option<&DeveloperProfile> {
        match self {
            Self::Editing(p) | Self::Saving(p) | Self::Saved(p) => Some(p),
            _ => None,
        }
    }

    fn is_saving(&self) -> bool {
        matches!(self, Self::Saving(_))
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn DevProfile() -> impl IntoView {
    let (state, set_state) = signal(ProfileState::LoadingAuth);
    let (dirty, set_dirty) = signal(false);

    // Auth check + profile fetch on mount
    // Use API-based auth check (GET /api/auth/me) instead of localStorage-only check.
    // The localStorage token may be missing/expired while the HttpOnly cookie is still
    // valid — using the API avoids a false redirect to /login → /admin for staff users.
    {
        leptos::task::spawn_local(async move {
            // Verify auth via cookie-based API call
            match crate::api::get_me().await {
                Ok(_) => {}
                Err(_) => {
                    let _ =
                        web_sys::window().map(|w| w.location().set_href("/login"));
                    return;
                }
            }

            set_state.set(ProfileState::LoadingProfile);

            match api::get_my_profile().await {
                Ok(profile) => {
                    set_state.set(ProfileState::Editing(profile));
                }
                Err(e) => {
                    set_state.set(ProfileState::Error(format!(
                        "Failed to load profile: {}",
                        e.message
                    )));
                }
            }
        });
    }

    // Save handler
    let on_save = move || {
        let current = state.get();
        let profile = match &current {
            ProfileState::Editing(p) | ProfileState::Saved(p) => p.clone(),
            _ => return,
        };

        let body: UpdateProfileBody = (&profile).into();
        set_state.set(ProfileState::Saving(profile));
        set_dirty.set(false);

        leptos::task::spawn_local(async move {
            match api::update_my_profile(&body).await {
                Ok(updated) => {
                    set_state.set(ProfileState::Saved(updated));
                }
                Err(e) => {
                    set_state.set(ProfileState::Error(format!(
                        "Failed to save: {}",
                        e.message
                    )));
                }
            }
        });
    };

    // Toggle a tag in a vec
    let toggle_tag = move |field: &str, tag: &str| {
        let current = state.get();
        let mut profile = match current.profile() {
            Some(p) => p.clone(),
            None => return,
        };

        let tags = match field {
            "interests" => &mut profile.interests,
            "tech_stack" => &mut profile.tech_stack,
            _ => return,
        };

        if let Some(pos) = tags.iter().position(|t| t == tag) {
            tags.remove(pos);
        } else {
            tags.push(tag.to_string());
        }

        set_state.set(ProfileState::Editing(profile));
        set_dirty.set(true);
    };

    // Update a text field
    let update_field = move |field: &str, value: String| {
        let current = state.get();
        let mut profile = match current.profile() {
            Some(p) => p.clone(),
            None => return,
        };

        match field {
            "display_name" => profile.display_name = value,
            "github_handle" => profile.github_handle = Some(value),
            "discord_handle" => profile.discord_handle = Some(value),
            "twitter_handle" => profile.twitter_handle = Some(value),
            "telegram_handle" => profile.telegram_handle = Some(value),
            "primary_role" => profile.primary_role = if value.is_empty() { None } else { Some(value) },
            "learning_goals" => profile.learning_goals = value,
            "company_org" => profile.company_org = value,
            "location_city" => profile.location_city = value,
            _ => {}
        }

        set_state.set(ProfileState::Editing(profile));
        set_dirty.set(true);
    };

    // Toggle consent
    let toggle_consent = move || {
        let current = state.get();
        let mut profile = match current.profile() {
            Some(p) => p.clone(),
            None => return,
        };
        profile.consent_outreach = !profile.consent_outreach;
        set_state.set(ProfileState::Editing(profile));
        set_dirty.set(true);
    };

    view! {
        <Title text="Developer Profile — BeThere" />
        <div class="dev-profile-page">
            <div class="dev-profile-header">
                <a href="/" class="btn btn-outline btn-sm dev-profile-back-btn">
                    "← Back"
                </a>
                <h1 class="dev-profile-title">
                    <Icon icon=IconName::Star class="icon-lg" />
                    " Developer Profile"
                </h1>
                <p class="dev-profile-subtitle">
                    "Tell us about yourself — your interests and skills help us improve events."
                </p>
            </div>

            {move || {
                let current = state.get();
                let d = dirty.get();
                match current {
                    ProfileState::LoadingAuth | ProfileState::LoadingProfile => {
                        view! {
                            <div class="dev-loading">
                                <div class="spinner"></div>
                                <p>"Loading profile..."</p>
                            </div>
                        }.into_any()
                    }
                    ProfileState::Error(msg) => {
                        view! {
                            <div class="dev-error">
                                <Icon icon=IconName::Warning class="icon-lg icon-warning" />
                                <p>{msg}</p>
                                <a href="/login" class="btn btn-primary btn-sm">"Sign In"</a>
                            </div>
                        }.into_any()
                    }
                    ProfileState::Saved(_) if !d => {
                        view! {
                            <div class="dev-profile-saved-banner">
                                <Icon icon=IconName::Check class="icon-sm icon-success" />
                                " Profile saved!"
                            </div>
                        }.into_any()
                    }
                    _ => view! { <div></div> }.into_any(),
                }
            }}

            {move || {
                let current = state.get();
                let profile = match current.profile() {
                    Some(p) => p.clone(),
                    None => return view! { <div></div> }.into_any(),
                };
                let is_saving = current.is_saving();
                let is_dirty = dirty.get();
                let email = profile.email.clone();
                let display_name = profile.display_name.clone();
                let github = profile.github_handle.clone().unwrap_or_default();
                let github_verified = profile.github_verified;
                let discord = profile.discord_handle.clone().unwrap_or_default();
                let discord_verified = profile.discord_verified;
                let twitter = profile.twitter_handle.clone().unwrap_or_default();
                let telegram = profile.telegram_handle.clone().unwrap_or_default();
                let telegram_verified = profile.telegram_verified;
                let role = profile.primary_role.clone().unwrap_or_default();
                let company = profile.company_org.clone();
                let city = profile.location_city.clone();
                let goals = profile.learning_goals.clone();
                let consent = profile.consent_outreach;
                let interests = profile.interests.clone();
                let tech_stack = profile.tech_stack.clone();
                let events = profile.total_events;

                view! {
                    <div class="dev-profile-form">

                        // Email (read-only)
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"Email"</label>
                            <div class="dev-profile-readonly">{email}</div>
                        </div>

                        // Display Name
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"Display Name"</label>
                            <input
                                class="dev-profile-input"
                                type="text"
                                placeholder="How should we call you?"
                                prop:value=display_name.clone()
                                on:input=move |ev| {
                                    update_field("display_name", event_target_value(&ev));
                                }
                            />
                        </div>

                        // Role
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"Primary Role"</label>
                            <select
                                class="dev-profile-select"
                                prop:value=role.clone()
                                on:change=move |ev| {
                                    update_field("primary_role", event_target_value(&ev));
                                }
                            >
                                <option value="">"— Select role —"</option>
                                {ROLE_OPTIONS.iter().map(|r| {
                                    view! {
                                        <option value={*r}>{*r}</option>
                                    }
                                }).collect::<Vec<_>>()}
                            </select>
                        </div>

                        // Company/Org
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"Company / Organization"</label>
                            <input
                                class="dev-profile-input"
                                type="text"
                                placeholder="Where do you work?"
                                prop:value=company.clone()
                                on:input=move |ev| {
                                    update_field("company_org", event_target_value(&ev));
                                }
                            />
                        </div>

                        // City
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"City"</label>
                            <input
                                class="dev-profile-input"
                                type="text"
                                placeholder="e.g. Bangkok"
                                prop:value=city.clone()
                                on:input=move |ev| {
                                    update_field("location_city", event_target_value(&ev));
                                }
                            />
                        </div>

                        // Social handles section
                        <div class="dev-profile-section">
                            <h3 class="dev-profile-section-title">"Social Links"
                                <span style="font-size:0.7rem;font-weight:400;color:#94a3b8;margin-left:8px;">"— Connect accounts to verify them"</span>
                            </h3>

                            // GitHub
                            <div class="dev-profile-social-row">
                                <div class="dev-profile-social-info">
                                    <span class="dev-profile-social-icon">"🐙"</span>
                                    <span class="dev-profile-label">"GitHub"</span>
                                    {if github_verified {
                                        view! {
                                            <span class="dev-profile-verified-badge">"✓ Verified"</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>
                                {if github_verified {
                                    let gh = github.clone();
                                    let clean = gh.trim_start_matches('@').trim_start_matches("https://github.com/").trim().to_string();
                                    let gh_url = format!("https://github.com/{clean}");
                                    view! {
                                        <div class="dev-profile-social-actions">
                                            <a href=gh_url target="_blank" rel="noopener noreferrer" class="dev-profile-social-link-btn">
                                                "@" {clean} " ↗"
                                            </a>
                                            <a href="/api/auth/social/unlink" class="dev-profile-social-unlink-btn"
                                               on:click=move |ev| {
                                                   ev.prevent_default();
                                                   leptos::task::spawn_local(async move {
                                                       let _ = api::social_unlink("github").await;
                                                       web_sys::window().unwrap().location().reload().unwrap();
                                                   });
                                               }
                                            >
                                                "Unlink"
                                            </a>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="dev-profile-social-actions">
                                            <a href="/api/auth/github" rel="external" class="dev-profile-social-connect-btn"
                                                on:click=move |ev| {
                                                    ev.prevent_default();
                                                    if let Some(win) = web_sys::window() {
                                                        let _ = win.location().set_href("/api/auth/github");
                                                    }
                                                }
                                            >
                                                "Connect GitHub →"
                                            </a>
                                        </div>
                                    }.into_any()
                                }}
                            </div>

                            // Telegram
                            <div class="dev-profile-social-row">
                                <div class="dev-profile-social-info">
                                    <span class="dev-profile-social-icon">"✈️"</span>
                                    <span class="dev-profile-label">"Telegram"</span>
                                    {if telegram_verified {
                                        view! {
                                            <span class="dev-profile-verified-badge">"✓ Verified"</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>
                                {if telegram_verified {
                                    let tg = telegram.clone();
                                    let tg_url = format!("https://t.me/{}", tg.trim_start_matches('@'));
                                    view! {
                                        <div class="dev-profile-social-actions">
                                            <a href=tg_url target="_blank" rel="noopener noreferrer" class="dev-profile-social-link-btn">
                                                "@" {tg} " ↗"
                                            </a>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="dev-profile-social-actions">
                                            // Telegram Login Widget — renders a button that opens Telegram auth
                                            <div id="telegram-login-widget" class="dev-profile-telegram-widget">
                                                "📱 Use Telegram Login below:"
                                            </div>
                                        </div>
                                    }.into_any()
                                }}
                            </div>

                            // Discord
                            <div class="dev-profile-social-row">
                                <div class="dev-profile-social-info">
                                    <span class="dev-profile-social-icon">"🎮"</span>
                                    <span class="dev-profile-label">"Discord"</span>
                                    {if discord_verified {
                                        view! {
                                            <span class="dev-profile-verified-badge">"✓ Verified"</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>
                                <div class="dev-profile-social-actions">
                                    {if discord_verified {
                                        let dc = discord.clone();
                                        view! {
                                            <span class="dev-profile-social-handle">"@"{dc}</span>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <input
                                                class="dev-profile-input dev-profile-social-input"
                                                type="text"
                                                placeholder="@username (manual)"
                                                prop:value=discord.clone()
                                                on:input=move |ev| {
                                                    update_field("discord_handle", event_target_value(&ev));
                                                }
                                            />
                                        }.into_any()
                                    }}
                                </div>
                            </div>

                            // Twitter / X (manual — Twitter OAuth is expensive)
                            <div class="dev-profile-social-row">
                                <div class="dev-profile-social-info">
                                    <span class="dev-profile-social-icon">"𝕏"</span>
                                    <span class="dev-profile-label">"Twitter / X"</span>
                                </div>
                                <div class="dev-profile-social-actions">
                                    {if !twitter.is_empty() {
                                        let clean = twitter.trim_start_matches('@').trim_start_matches("https://x.com/").trim_start_matches("https://twitter.com/").trim().to_string();
                                        let x_url = format!("https://x.com/{clean}");
                                        view! {
                                            <div style="display:flex;gap:8px;align-items:center;">
                                                <input
                                                    class="dev-profile-input dev-profile-social-input"
                                                    type="text"
                                                    placeholder="@handle"
                                                    prop:value=twitter.clone()
                                                    on:input=move |ev| {
                                                        update_field("twitter_handle", event_target_value(&ev));
                                                    }
                                                />
                                                <a href=x_url target="_blank" rel="noopener noreferrer" class="dev-profile-social-link-btn" style="white-space:nowrap;">
                                                    "↗"
                                                </a>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <input
                                                class="dev-profile-input dev-profile-social-input"
                                                type="text"
                                                placeholder="@handle"
                                                prop:value=twitter.clone()
                                                on:input=move |ev| {
                                                    update_field("twitter_handle", event_target_value(&ev));
                                                }
                                            />
                                        }.into_any()
                                    }}
                                </div>
                            </div>
                        </div>

                        // Interests
                        <div class="dev-profile-section">
                            <h3 class="dev-profile-section-title">"Interests"</h3>
                            <p class="dev-profile-hint">"Select topics you're interested in."</p>
                            <div class="dev-profile-tags">
                                {INTEREST_OPTIONS.iter().map(|tag| {
                                    let tag_str = tag.to_string();
                                    let tag_str_click = tag_str.clone();
                                    let is_selected = interests.contains(&tag_str);
                                    let class = if is_selected {
                                        "dev-profile-tag selected"
                                    } else {
                                        "dev-profile-tag"
                                    };
                                    view! {
                                        <button
                                            class={class}
                                            on:click=move |_| {
                                                toggle_tag("interests", &tag_str_click);
                                            }
                                        >
                                            {tag_str}
                                        </button>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>

                        // Tech Stack (free text, comma-separated)
                        <div class="dev-profile-section">
                            <h3 class="dev-profile-section-title">"Tech Stack"</h3>
                            <p class="dev-profile-hint">
                                "Technologies you use (comma-separated)."
                            </p>
                            <input
                                class="dev-profile-input"
                                type="text"
                                placeholder="Rust, TypeScript, React, Solidity..."
                                prop:value={tech_stack.join(", ")}
                                on:input=move |ev| {
                                    let val = event_target_value(&ev);
                                    let tags: Vec<String> = val
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    let current = state.get();
                                    let mut p = match current.profile() {
                                        Some(p) => p.clone(),
                                        None => return,
                                    };
                                    p.tech_stack = tags;
                                    set_state.set(ProfileState::Editing(p));
                                    set_dirty.set(true);
                                }
                            />
                        </div>

                        // Learning Goals
                        <div class="dev-profile-field">
                            <label class="dev-profile-label">"Learning Goals"</label>
                            <textarea
                                class="dev-profile-textarea"
                                placeholder="What do you want to learn?"
                                rows="3"
                                prop:value=goals.clone()
                                on:input=move |ev| {
                                    update_field("learning_goals", event_target_value(&ev));
                                }
                            />
                        </div>

                        // Events attended
                        {if events > 0 {
                            view! {
                                <div class="dev-profile-section">
                                    <h3 class="dev-profile-section-title">"Your Activity"</h3>
                                    <div class="dev-profile-stat-card">
                                        <span class="dev-profile-stat-number">{format!("{events}")}</span>
                                        <span class="dev-profile-stat-label">{if events == 1 { "Event Attended" } else { "Events Attended" }}</span>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="dev-profile-section">
                                    <h3 class="dev-profile-section-title">"Your Activity"</h3>
                                    <p class="dev-profile-hint">
                                        "You haven't attended any events yet. Join an event to see your stats here!"
                                    </p>
                                </div>
                            }.into_any()
                        }}

                        // Consent
                        <div class="dev-profile-field">
                            <label class="dev-profile-checkbox-label">
                                <input
                                    type="checkbox"
                                    prop:checked=consent
                                    on:change=move |_| {
                                        toggle_consent();
                                    }
                                />
                                " I consent to being contacted about future events and opportunities"
                            </label>
                        </div>

                        // Save button
                        <div class="dev-profile-actions">
                            <button
                                class="btn btn-primary"
                                disabled={move || !is_dirty || is_saving}
                                on:click=move |_| on_save()
                            >
                                {if is_saving {
                                    "Saving..."
                                } else {
                                    "Save Profile"
                                }}
                            </button>
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}
