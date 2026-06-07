use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn capacity_indicator(
    in_person_capacity: Option<u32>,
    online_remaining: Option<u32>,
    in_person_remaining: Option<u32>,
) -> AnyView {
    let has_ip_cap = in_person_capacity.is_some();
    let has_on_cap = online_remaining.is_some();

    if !has_ip_cap && !has_on_cap {
        return ().into_any();
    }

    view! {
        <div class="pe-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Ticket class="icon-md" />" Capacity"
            </h2>
            <div class="pe-capacity-grid">
                {if has_ip_cap {
                    let remaining = in_person_remaining.unwrap_or(0);
                    let (color, label, is_full) = if remaining > 0 {
                        ("#34d399", "In-Person Spots Remaining", false)
                    } else {
                        ("#f87171", "In-Person — FULL", true)
                    };
                    view! {
                        <div class="pe-capacity-tile">
                            <span class="pe-capacity-number" style=format!("color:{color};{}", if is_full { "text-decoration:line-through;opacity:0.6" } else { "" })>{remaining}</span>
                            <span class="pe-capacity-label">{label}</span>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }}
                {if has_on_cap {
                    let remaining = online_remaining.unwrap_or(0);
                    let (color, label, is_full) = if remaining > 0 {
                        ("#34d399", "Online Spots Remaining", false)
                    } else {
                        ("#f87171", "Online — FULL", true)
                    };
                    view! {
                        <div class="pe-capacity-tile">
                            <span class="pe-capacity-number" style=format!("color:{color};{}", if is_full { "text-decoration:line-through;opacity:0.6" } else { "" })>{remaining}</span>
                            <span class="pe-capacity-label">{label}</span>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
        </div>
    }.into_any()
}
