//! Developer profile editor page.
//!
//! Allows authenticated attendees to view and edit their developer profile:
//! display name, social handles, interests, tech stack, and role.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::api::{
    self, DeveloperProfile, UpdateProfileBody, INTEREST_OPTIONS, ROLE_OPTIONS,
};
use crate::icons::{Icon, IconName};

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
        let set_state = set_state;
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
                let discord = profile.discord_handle.clone().unwrap_or_default();
                let twitter = profile.twitter_handle.clone().unwrap_or_default();
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
                            <h3 class="dev-profile-section-title">"Social Links"</h3>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"GitHub"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="github_username"
                                    prop:value=github.clone()
                                    on:input=move |ev| {
                                        update_field("github_handle", event_target_value(&ev));
                                    }
                                />
                            </div>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Discord"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="discord_username"
                                    prop:value=discord.clone()
                                    on:input=move |ev| {
                                        update_field("discord_handle", event_target_value(&ev));
                                    }
                                />
                            </div>

                            <div class="dev-profile-field">
                                <label class="dev-profile-label">"Twitter / X"</label>
                                <input
                                    class="dev-profile-input"
                                    type="text"
                                    placeholder="twitter_handle"
                                    prop:value=twitter.clone()
                                    on:input=move |ev| {
                                        update_field("twitter_handle", event_target_value(&ev));
                                    }
                                />
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
