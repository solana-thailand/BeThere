//! Where a sponsor's logo goes (.plans/043 L6, ASKS-4 §10): six placements
//! as wireframes, filtered by what a sponsor gives, each tagged LIVE (ships
//! today) or PROPOSED (designed, not built; build plan rule 2). Tiers carry
//! no prices (rule: pricing by conversation). No other organisation's logo
//! appears anywhere (rule 5): the wireframes show empty dashed slots.
//!
//! The contact card has the name, role and links the owner gave for the
//! prototype. The prototype's photo is a person's picture in a public repo,
//! so it waits for the owner; email, phone and LINE were never given.

use leptos::prelude::*;

use crate::i18n::{Locale, td_string};

use super::hero::Markup;

type Catalog = fn(Locale) -> &'static str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    All,
    Venue,
    Food,
    Stream,
    Ecosystem,
}

impl Category {
    const ALL: [Category; 5] = [
        Category::All,
        Category::Venue,
        Category::Food,
        Category::Stream,
        Category::Ecosystem,
    ];

    fn label(self) -> Catalog {
        match self {
            Category::All => |l| td_string!(l, landing.sponsors.cat_all),
            Category::Venue => |l| td_string!(l, landing.sponsors.cat_venue),
            Category::Food => |l| td_string!(l, landing.sponsors.cat_food),
            Category::Stream => |l| td_string!(l, landing.sponsors.cat_stream),
            Category::Ecosystem => |l| td_string!(l, landing.sponsors.cat_eco),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    EventPage,
    Poster,
    GroupPhoto,
    Recording,
    Ticket,
    Badge,
}

impl Placement {
    pub const ALL: [Placement; 6] = [
        Placement::EventPage,
        Placement::Poster,
        Placement::GroupPhoto,
        Placement::Recording,
        Placement::Ticket,
        Placement::Badge,
    ];

    /// Shipping today (the event-page sponsor row is migration 0057; the
    /// poster line and group-photo row are made for every event).
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Placement::EventPage | Placement::Poster | Placement::GroupPhoto
        )
    }

    /// Which sponsors this placement suits (ASKS-4 §10).
    pub fn suits(self, cat: Category) -> bool {
        use Category::*;
        let cats: &[Category] = match self {
            Placement::EventPage => &[Venue, Food, Stream, Ecosystem],
            Placement::Poster => &[Venue, Food, Ecosystem],
            Placement::GroupPhoto => &[Venue, Ecosystem],
            Placement::Recording => &[Stream, Ecosystem],
            Placement::Ticket => &[Food, Ecosystem],
            Placement::Badge => &[Ecosystem],
        };
        cat == All || cats.contains(&cat)
    }

    fn copy(self) -> (Catalog, Catalog) {
        match self {
            Placement::EventPage => (
                |l| td_string!(l, landing.sponsors.page_t),
                |l| td_string!(l, landing.sponsors.page_d),
            ),
            Placement::Poster => (
                |l| td_string!(l, landing.sponsors.poster_t),
                |l| td_string!(l, landing.sponsors.poster_d),
            ),
            Placement::GroupPhoto => (
                |l| td_string!(l, landing.sponsors.photo_t),
                |l| td_string!(l, landing.sponsors.photo_d),
            ),
            Placement::Recording => (
                |l| td_string!(l, landing.sponsors.rec_t),
                |l| td_string!(l, landing.sponsors.rec_d),
            ),
            Placement::Ticket => (
                |l| td_string!(l, landing.sponsors.ticket_t),
                |l| td_string!(l, landing.sponsors.ticket_d),
            ),
            Placement::Badge => (
                |l| td_string!(l, landing.sponsors.badge_t),
                |l| td_string!(l, landing.sponsors.badge_d),
            ),
        }
    }
}

/// The wireframe for a placement: page shapes in ink, the logo slots dashed.
fn wireframe(p: Placement) -> AnyView {
    match p {
        Placement::EventPage => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <rect class="lp-wf-ink" x="20" y="14" width="200" height="12" />
                <rect class="lp-wf-dim" x="20" y="34" width="150" height="6" />
                <rect class="lp-wf-dim" x="20" y="46" width="170" height="6" />
                <rect class="lp-wf-line" x="20" y="70" width="200" height="24" />
                <g class="lp-wf-slot"><rect x="20" y="104" width="60" height="22" /><rect x="90" y="104" width="60" height="22" /><rect x="160" y="104" width="60" height="22" /></g>
            </svg>
        }.into_any(),
        Placement::Poster => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <rect class="lp-wf-line" x="70" y="8" width="100" height="124" />
                <rect class="lp-wf-field" x="82" y="22" width="76" height="40" />
                <rect class="lp-wf-ink" x="82" y="72" width="60" height="7" />
                <rect class="lp-wf-dim" x="82" y="85" width="44" height="5" />
                <g class="lp-wf-slot"><rect x="82" y="104" width="34" height="18" /><rect x="124" y="104" width="34" height="18" /></g>
            </svg>
        }.into_any(),
        Placement::GroupPhoto => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <rect class="lp-wf-dim" x="20" y="10" width="200" height="120" />
                <g class="lp-wf-ink">
                    {(0..9).map(|i| {
                        let x = 40 + i * 20;
                        let y = match i % 2 { 0 => 78, _ => 86 };
                        view! { <circle cx=x cy=y r="7" /><rect x=x - 7 y=y + 8 width="14" height="22" /> }
                    }).collect::<Vec<_>>()}
                </g>
                <g class="lp-wf-slot"><rect x="60" y="18" width="34" height="16" /><rect x="103" y="18" width="34" height="16" /><rect x="146" y="18" width="34" height="16" /></g>
            </svg>
        }.into_any(),
        Placement::Recording => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <rect class="lp-wf-ink" x="20" y="14" width="200" height="112" />
                <polygon class="lp-wf-paper" points="110,52 110,88 140,70" />
                <rect class="lp-wf-field" x="20" y="126" width="200" height="4" />
                <g class="lp-wf-slot"><rect x="150" y="92" width="58" height="24" /></g>
            </svg>
        }.into_any(),
        Placement::Ticket => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <rect class="lp-wf-line" x="80" y="8" width="80" height="124" rx="8" />
                <rect class="lp-wf-ink" x="96" y="28" width="48" height="48" />
                <rect class="lp-wf-paper2" x="102" y="34" width="12" height="12" />
                <rect class="lp-wf-paper2" x="126" y="34" width="12" height="12" />
                <rect class="lp-wf-paper2" x="102" y="58" width="12" height="12" />
                <g class="lp-wf-slot"><rect x="92" y="96" width="56" height="20" /></g>
            </svg>
        }.into_any(),
        Placement::Badge => view! {
            <svg viewBox="0 0 240 140" aria-hidden="true">
                <circle class="lp-wf-line" cx="120" cy="66" r="50" />
                <circle class="lp-wf-ink" cx="120" cy="66" r="38" />
                <text class="lp-wf-paper" x="120" y="72" text-anchor="middle" font-family="Inter" font-weight="800" font-size="18">"Be"</text>
                <g class="lp-wf-slot"><rect x="94" y="110" width="52" height="20" /></g>
            </svg>
        }.into_any(),
    }
}

/// The contact the owner gave for the prototype's business card.
const FACEBOOK_URL: &str = "https://www.facebook.com/ozoneRatchapon";
const DISCORD_URL: &str = "https://discord.gg/PGbUgNmsns";

/// A vCard 3.0 of the same contact, as a `data:` link.
pub fn vcard_href() -> String {
    let card = [
        "BEGIN:VCARD",
        "VERSION:3.0",
        "FN:Ozone",
        "ORG:Solana Developer Thailand;BeThere",
        "TITLE:Founder, BeThere",
        &format!("URL:{FACEBOOK_URL}"),
        &format!("X-SOCIALPROFILE;TYPE=discord:{DISCORD_URL}"),
        "END:VCARD",
    ]
    .join("\r\n");
    format!(
        "data:text/vcard;charset=utf-8,{}",
        js_sys::encode_uri_component(&card)
    )
}

#[component]
pub fn Sponsors() -> impl IntoView {
    let picked = RwSignal::new(Category::All);
    let tr = crate::locale::tr;
    let tiers: [(&str, Catalog, &str); 3] = [
        (
            "Bronze",
            |l| td_string!(l, landing.sponsors.bronze),
            "lp-card lp-tier",
        ),
        (
            "Silver",
            |l| td_string!(l, landing.sponsors.silver),
            "lp-card lp-tier",
        ),
        (
            "Gold",
            |l| td_string!(l, landing.sponsors.gold),
            "lp-card lp-tier lp-gold",
        ),
    ];
    view! {
        <section id="sponsors" class="lp-sponsors">
            <div class="lp-wrap">
                <p class="lp-kicker">{tr(|l| td_string!(l, landing.sponsors.kicker))}</p>
                <h2 class="lp-h2">{tr(|l| td_string!(l, landing.sponsors.title))}</h2>
                <p class="lp-lede"><Markup text=tr(|l| td_string!(l, landing.sponsors.lede)) /></p>
                <div class="lp-cats" role="group" aria-label=tr(|l| td_string!(l, landing.sponsors.cats_label))>
                    {Category::ALL.into_iter().map(|cat| view! {
                        <button
                            type="button"
                            class="lp-cat"
                            aria-pressed=move || (picked.get() == cat).to_string()
                            on:click=move |_| picked.set(cat)
                        >
                            {tr(cat.label())}
                        </button>
                    }).collect::<Vec<_>>()}
                </div>
                <div class="lp-spots">
                    {Placement::ALL.into_iter().map(|p| {
                        let (title, desc) = p.copy();
                        let (st_class, st_label): (&str, Catalog) = match p.is_live() {
                            true => ("lp-st lp-st-yes", |l| td_string!(l, landing.sponsors.live)),
                            false => ("lp-st lp-st-prop", |l| td_string!(l, landing.sponsors.proposed)),
                        };
                        let class = move || match p.suits(picked.get()) {
                            true => "lp-card lp-spot",
                            false => "lp-card lp-spot lp-dim",
                        };
                        view! {
                            <article class=class tabindex="0">
                                {wireframe(p)}
                                <span class=st_class>{tr(st_label)}</span>
                                <h3>{tr(title)}</h3>
                                <p>{tr(desc)}</p>
                            </article>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                <div class="lp-card lp-contact">
                    <h3>{tr(|l| td_string!(l, landing.sponsors.contact_title))}</h3>
                    <div class="lp-who-row">
                        <img class="lp-avatar" src="/ozone-avatar-56.jpg" srcset="/ozone-avatar-112.jpg 2x"
                            alt="Ozone" width="56" height="56" loading="lazy" decoding="async" />
                        <p class="lp-who"><strong>"Ozone"</strong>" · "{tr(|l| td_string!(l, landing.sponsors.contact_role))}</p>
                    </div>
                    <div class="lp-row">
                        <a class="lp-btn lp-btn-primary" href=vcard_href() download="ozone-bethere.vcf">
                            {tr(|l| td_string!(l, landing.sponsors.save_contact))}
                        </a>
                        <a class="lp-btn" href=FACEBOOK_URL target="_blank" rel="noopener noreferrer">"Facebook"</a>
                        <a class="lp-btn" href=DISCORD_URL target="_blank" rel="noopener noreferrer">"Discord"</a>
                    </div>
                    <p class="lp-fineprint">{tr(|l| td_string!(l, landing.sponsors.contact_note))}</p>
                </div>
                <div class="lp-tiers">
                    {tiers.into_iter().map(|(name, what, class)| view! {
                        <div class=class><b>{name}</b><span>{tr(what)}</span></div>
                    }).collect::<Vec<_>>()}
                </div>
                <p class="lp-fineprint">{tr(|l| td_string!(l, landing.sponsors.pricing))}</p>
            </div>
        </section>
    }
}
