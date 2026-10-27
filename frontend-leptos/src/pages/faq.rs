//! `/faq` (.plans/038 P3-a): questions for attendees and for organizers, in
//! two tabs of native `<details>` accordions (keyboard and screen-reader
//! support for free, no JS). Linked from the landing footer only.
//!
//! The deposit promises are not written here: they come from
//! `utils::deposit_copy`, like everywhere else
//! (`tests/deposit_promise_has_one_home.rs`).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::{Locale, t, td_string, use_i18n};
use crate::utils::deposit_copy::thb_refund_window;

#[derive(Clone, Copy, PartialEq)]
enum Audience {
    Attendees,
    Organizers,
}

/// A question and its answer, both from the catalog.
type Qa = (fn(Locale) -> &'static str, fn(Locale) -> &'static str);

const ATTENDEE_QA: [Qa; 7] = [
    (
        |l| td_string!(l, landing.faq.what_q),
        |l| td_string!(l, landing.faq.what_a),
    ),
    (
        |l| td_string!(l, landing.faq.account_q),
        |l| td_string!(l, landing.faq.account_a),
    ),
    (
        |l| td_string!(l, landing.faq.wallet_q),
        |l| td_string!(l, landing.faq.wallet_a),
    ),
    (
        |l| td_string!(l, landing.faq.ticket_q),
        |l| td_string!(l, landing.faq.ticket_a),
    ),
    (
        |l| td_string!(l, landing.faq.claim_q),
        |l| td_string!(l, landing.faq.claim_a),
    ),
    (
        |l| td_string!(l, landing.faq.cancel_q),
        |l| td_string!(l, landing.faq.cancel_a),
    ),
    (
        |l| td_string!(l, landing.faq.crypto_q),
        |l| td_string!(l, landing.faq.crypto_a),
    ),
];

const ORGANIZER_QA: [Qa; 3] = [
    (
        |l| td_string!(l, landing.faq.org_create_q),
        |l| td_string!(l, landing.faq.org_create_a),
    ),
    (
        |l| td_string!(l, landing.faq.org_capacity_q),
        |l| td_string!(l, landing.faq.org_capacity_a),
    ),
    (
        |l| td_string!(l, landing.faq.org_checkin_q),
        |l| td_string!(l, landing.faq.org_checkin_a),
    ),
];

fn qa_item((q, a): Qa) -> impl IntoView {
    view! {
        <details class="faq-item">
            <summary class="faq-q">{crate::locale::tr(q)}</summary>
            <p class="faq-a">{crate::locale::tr(a)}</p>
        </details>
    }
}

#[component]
pub fn Faq() -> impl IntoView {
    let i18n = use_i18n();
    let (audience, set_audience) = signal(Audience::Attendees);
    let tab = move |which: Audience, label: fn(Locale) -> &'static str| {
        view! {
            <button
                class="faq-tab"
                role="tab"
                aria-selected=move || (audience.get() == which).to_string()
                class:faq-tab--active=move || audience.get() == which
                on:click=move |_| set_audience.set(which)
            >
                {crate::locale::tr(label)}
            </button>
        }
    };
    view! {
        <Title text=crate::locale::tr(|l| td_string!(l, landing.faq.page_title)) />
        <div class="faq-page">
            <a href="/" class="faq-back">{crate::locale::tr(|l| td_string!(l, landing.faq.back_home))}</a>
            <h1 class="faq-title">{crate::locale::tr(|l| td_string!(l, landing.faq.title))}</h1>
            <div class="faq-tabs" role="tablist">
                {tab(Audience::Attendees, |l| td_string!(l, landing.faq.tab_attendees))}
                {tab(Audience::Organizers, |l| td_string!(l, landing.faq.tab_organizers))}
            </div>
            <div class="faq-panel" role="tabpanel">
                {move || match audience.get() {
                    Audience::Attendees => view! {
                        {ATTENDEE_QA.into_iter().take(3).map(qa_item).collect::<Vec<_>>()}
                        // Deposit: the promises come from deposit_copy.
                        <details class="faq-item">
                            <summary class="faq-q">{crate::locale::tr(|l| td_string!(l, landing.faq.deposit_q))}</summary>
                            <p class="faq-a">
                                {t!(
                                    i18n,
                                    landing.faq.deposit_a,
                                    refund_window = move || thb_refund_window(i18n.get_locale())
                                )}
                            </p>
                        </details>
                        {ATTENDEE_QA.into_iter().skip(3).map(qa_item).collect::<Vec<_>>()}
                        <details class="faq-item">
                            <summary class="faq-q">{crate::locale::tr(|l| td_string!(l, landing.faq.privacy_q))}</summary>
                            <p class="faq-a">
                                {crate::locale::tr(|l| td_string!(l, landing.faq.privacy_a))}
                                " "
                                <a href="/data-privacy">{crate::locale::tr(|l| td_string!(l, landing.faq.privacy_manage))}</a>
                                " · "
                                <a href="/privacy">{crate::locale::tr(|l| td_string!(l, landing.faq.privacy_policy))}</a>
                            </p>
                        </details>
                    }.into_any(),
                    Audience::Organizers => view! {
                        {ORGANIZER_QA.into_iter().map(qa_item).collect::<Vec<_>>()}
                        // Settlement: the THB window comes from deposit_copy.
                        <details class="faq-item">
                            <summary class="faq-q">{crate::locale::tr(|l| td_string!(l, landing.faq.org_settle_q))}</summary>
                            <p class="faq-a">
                                {t!(
                                    i18n,
                                    landing.faq.org_settle_a,
                                    refund_window = move || thb_refund_window(i18n.get_locale())
                                )}
                            </p>
                        </details>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

/// The FAQ as Markdown, for AI readers (.plans/049 item 4). `/llms.txt`
/// links pages that render only with wasm, so a crawler reads nothing behind
/// them; this is the same catalog text, in the same order as the page, served
/// as a static file. Native only: the wasm never calls it.
/// `tests/faq_markdown.rs` fails when the committed files drift from it.
#[cfg(not(target_arch = "wasm32"))]
pub fn faq_markdown(locale: Locale) -> String {
    let qa = |out: &mut String, q: &str, a: &str| {
        out.push_str(&format!("### {q}\n\n{a}\n\n"));
    };
    // `td_string!` cannot interpolate (no `interpolate_display`), so the two
    // answers that carry `{{ refund_window }}` are read raw from the same
    // catalog JSON and filled with the same window the page uses.
    let catalog: serde_json::Value = serde_json::from_str(match locale {
        Locale::en => include_str!("../../locales/en/landing.json"),
        Locale::th => include_str!("../../locales/th/landing.json"),
    })
    .expect("landing catalog is JSON");
    let with_window = |key: &str| {
        catalog["faq"][key]
            .as_str()
            .expect("faq key in the catalog")
            .replace("{{ refund_window }}", thb_refund_window(locale))
    };
    let mut out = format!(
        "# {}\n\n## {}\n\n",
        td_string!(locale, landing.faq.title),
        td_string!(locale, landing.faq.tab_attendees)
    );
    for (q, a) in ATTENDEE_QA.into_iter().take(3) {
        qa(&mut out, q(locale), a(locale));
    }
    qa(
        &mut out,
        td_string!(locale, landing.faq.deposit_q),
        &with_window("deposit_a"),
    );
    for (q, a) in ATTENDEE_QA.into_iter().skip(3) {
        qa(&mut out, q(locale), a(locale));
    }
    let privacy = format!(
        "{} [{}](/data-privacy) · [{}](/privacy)",
        td_string!(locale, landing.faq.privacy_a),
        td_string!(locale, landing.faq.privacy_manage),
        td_string!(locale, landing.faq.privacy_policy)
    );
    qa(
        &mut out,
        td_string!(locale, landing.faq.privacy_q),
        &privacy,
    );
    out.push_str(&format!(
        "## {}\n\n",
        td_string!(locale, landing.faq.tab_organizers)
    ));
    for (q, a) in ORGANIZER_QA {
        qa(&mut out, q(locale), a(locale));
    }
    qa(
        &mut out,
        td_string!(locale, landing.faq.org_settle_q),
        &with_window("org_settle_a"),
    );
    out.trim_end().to_string() + "\n"
}
