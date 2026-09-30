//! Sponsors section: name + logo URL + link rows (migration 0057).
//!
//! Logo upload to R2 is a follow-up; for now the organizer pastes an
//! `https://` image URL. Validation is `normalize_sponsors`, run at save.

use event_checkin_domain::models::event::{MAX_SPONSORS, Sponsor};
use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};

/// Which field of a sponsor row an input edits.
#[derive(Clone, Copy)]
enum SponsorField {
    Name,
    LogoUrl,
    Link,
}

impl SponsorField {
    fn get(self, s: &Sponsor) -> String {
        match self {
            Self::Name => s.name.clone(),
            Self::LogoUrl => s.logo_url.clone(),
            Self::Link => s.link.clone(),
        }
    }

    fn set(self, s: &mut Sponsor, val: String) {
        match self {
            Self::Name => s.name = val,
            Self::LogoUrl => s.logo_url = val,
            Self::Link => s.link = val,
        }
    }
}

/// Optional sponsor list shown as a logo row on the public event page.
#[component]
pub(super) fn SponsorsSection(ctx: FormCtx) -> impl IntoView {
    let sponsors = ctx.sponsors;
    // Rows re-render only when a row is added or removed, not on every
    // keystroke, so an input keeps focus while the organizer types.
    let row_count = Memo::new(move |_| sponsors.with(Vec::len));

    let field_input = move |idx: usize,
                            field: SponsorField,
                            kind: &'static str,
                            class: &'static str,
                            placeholder: &'static str| {
        let value =
            sponsors.with_untracked(|v| v.get(idx).map(|s| field.get(s)).unwrap_or_default());
        view! {
            <input
                type=kind
                class=format!("quiz-number-input {class}")
                placeholder=placeholder
                prop:value=value
                on:input=move |ev| {
                    let val = event_target_value(&ev);
                    sponsors.update(|v| {
                        if let Some(s) = v.get_mut(idx) {
                            field.set(s, val);
                        }
                    });
                }
            />
        }
    };

    view! {
        <FormSection icon_class="form-section-icon-community" title="Sponsors" badge=SectionBadge::Optional open=false>
            <p class="quiz-setting-hint">
                "Shown as a logo row near the bottom of the public event page, in this order. Logos are shown in grey until hovered. Leave the logo empty to show the name instead. Addresses must start with https."
            </p>
            {move || {
                (0..row_count.get()).map(|idx| view! {
                    <div class="community-link-row sponsor-row">
                        {field_input(idx, SponsorField::Name, "text", "sponsor-name", "Sponsor name")}
                        {field_input(idx, SponsorField::LogoUrl, "url", "sponsor-logo-url", "Logo image URL (https)")}
                        {field_input(idx, SponsorField::Link, "url", "sponsor-link", "Website (optional)")}
                        <button
                            type="button"
                            class="btn btn-outline btn-xs community-link-remove"
                            title="Remove sponsor"
                            aria-label=format!("Remove sponsor {}", idx + 1)
                            on:click=move |_| {
                                sponsors.update(|v| {
                                    if idx < v.len() {
                                        v.remove(idx);
                                    }
                                });
                            }
                        >
                            "×"
                        </button>
                    </div>
                }).collect::<Vec<_>>()
            }}
            {move || {
                (row_count.get() < MAX_SPONSORS).then(|| view! {
                    <button
                        type="button"
                        class="btn btn-outline btn-sm community-link-add"
                        on:click=move |_| sponsors.update(|v| v.push(Sponsor::default()))
                    >
                        "+ Add Sponsor"
                    </button>
                })
            }}
        </FormSection>
    }
}
