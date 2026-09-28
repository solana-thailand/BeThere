use crate::i18n::{t_string, use_i18n};
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn capacity_indicator(
    in_person_capacity: Option<u32>,
    online_remaining: Option<u32>,
    in_person_remaining: Option<u32>,
) -> AnyView {
    let i18n = use_i18n();
    let has_ip_cap = in_person_capacity.is_some();
    let has_on_cap = online_remaining.is_some();

    if !has_ip_cap && !has_on_cap {
        return ().into_any();
    }

    view! {
        <div class="pe-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Ticket class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.capacity_title))}
            </h2>
            <div class="pe-capacity-grid">
                {if has_ip_cap {
                    let remaining = in_person_remaining.unwrap_or(0);
                    let is_full = remaining == 0;
                    let color = if is_full { "#f87171" } else { "#34d399" };
                    let label = move || match is_full {
                        false => t_string!(i18n, event.capacity_in_person_left),
                        true => t_string!(i18n, event.capacity_in_person_full),
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
                    let is_full = remaining == 0;
                    let color = if is_full { "#f87171" } else { "#34d399" };
                    let label = move || match is_full {
                        false => t_string!(i18n, event.capacity_online_left),
                        true => t_string!(i18n, event.capacity_online_full),
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
