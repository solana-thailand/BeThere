//! The collapsible section frame every form section shares.

use leptos::prelude::*;

/// The badge beside a section title.
#[derive(Clone, Copy)]
pub(super) enum SectionBadge {
    Required,
    Recommended,
    Optional,
}

impl SectionBadge {
    fn class(self) -> &'static str {
        match self {
            Self::Required => "form-section-badge form-section-badge-required",
            Self::Recommended => "form-section-badge form-section-badge-recommended",
            Self::Optional => "form-section-badge form-section-badge-optional",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Required => "Required",
            Self::Recommended => "Recommended",
            Self::Optional => "Optional",
        }
    }
}

/// A form section with a keyboard-operable header that collapses its body.
///
/// `icon_class` is the full modifier class (`form-section-icon-basic`), never a
/// fragment. tests/css_class_audit.rs reads `icon_class="…"` as a class position.
#[component]
pub(super) fn FormSection(
    icon_class: &'static str,
    title: &'static str,
    badge: SectionBadge,
    #[prop(default = true)] open: bool,
    children: Children,
) -> impl IntoView {
    let (is_open, set_open) = signal(open);
    let toggle = move || set_open.update(|v| *v = !*v);
    view! {
        <div class="form-section">
            <div
                class="form-section-header"
                role="button"
                tabindex="0"
                aria-expanded=move || is_open.get().to_string()
                on:click=move |_| toggle()
                on:keydown=move |ev| {
                    if crate::utils::is_activation_key(&ev) {
                        ev.prevent_default();
                        toggle();
                    }
                }
            >
                <span class=format!("form-section-icon {icon_class}")></span>
                <span class="form-section-title">{title}</span>
                <span class=badge.class()>{badge.label()}</span>
                <span class="form-section-toggle" class:form-section-toggle-open=move || is_open.get()>"▼"</span>
            </div>
            <div class="form-section-body" class:form-section-body-hidden=move || !is_open.get()>
                {children()}
            </div>
        </div>
    }
}
