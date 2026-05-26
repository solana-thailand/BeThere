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
            <div style="display:flex;flex-direction:column;gap:0.5rem;">
                {if has_ip_cap {
                    let remaining = in_person_remaining.unwrap_or(0);
                    let (color, label) = if remaining > 0 {
                        ("#34d399", format!("In-Person: {remaining} spots left"))
                    } else {
                        ("#f87171", "In-Person: FULL".to_string())
                    };
                    view! {
                        <div class="pe-capacity-row">
                            <span style=format!("width:8px;height:8px;border-radius:50%;background:{color};flex-shrink:0;")></span>
                            <span style=format!("color:{color};font-size:0.9rem;font-weight:500;")>{label}</span>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }}
                {if has_on_cap {
                    let remaining = online_remaining.unwrap_or(0);
                    let (color, label) = if remaining > 0 {
                        ("#34d399", format!("Online: {remaining} spots left"))
                    } else {
                        ("#f87171", "Online: FULL".to_string())
                    };
                    view! {
                        <div class="pe-capacity-row">
                            <span style=format!("width:8px;height:8px;border-radius:50%;background:{color};flex-shrink:0;")></span>
                            <span style=format!("color:{color};font-size:0.9rem;font-weight:500;")>{label}</span>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
        </div>
    }.into_any()
}
