//! The site's pages, and the doors at the foot of each one (.plans/045 R4.0).
//!
//! Owner, 7 Oct (`bethere-ux/site/site.js`, `DOORS`): every page ends on the
//! other pages, one card each, so there is no "next page" link to follow and
//! no chapter order to learn. The try band (one wide card under the doors)
//! sends anyone to the devnet sandbox; it stays off until that sandbox ships
//! (Super GOAT SG3), so no page links to a route that does not exist.

use leptos::prelude::*;

use crate::i18n::{Locale, td_string};
use crate::locale::tr;

/// Whether the devnet sandbox (`/sandbox`, SG3) is live: the try band under
/// the doors and the try lines link to it. On since the owner put the
/// sandbox on prod (2026-10-11); the route answers on every deploy, and says
/// so where the sandbox keys are not set.
pub const TRY_LIVE: bool = true;

/// Where the try band and lines go.
pub const TRY_PATH: &str = "/sandbox";

/// One page of the site: the landing and the pages Release 4 adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitePage {
    Home,
    Events,
    Organizers,
    Sponsors,
}

/// What a door says: title, one line, call to action.
struct DoorCopy {
    title: fn(Locale) -> &'static str,
    line: fn(Locale) -> &'static str,
    cta: fn(Locale) -> &'static str,
}

impl SitePage {
    /// Every page, in door order.
    pub const ALL: [SitePage; 4] = [
        SitePage::Home,
        SitePage::Events,
        SitePage::Organizers,
        SitePage::Sponsors,
    ];

    /// The route path. `tests/site_pages.rs` checks each one against the router.
    pub const fn path(self) -> &'static str {
        match self {
            SitePage::Home => "/",
            SitePage::Events => "/events",
            SitePage::Organizers => "/organizers",
            SitePage::Sponsors => "/sponsors",
        }
    }

    /// The page's name in the door counters (`worker/src/door_clicks.rs`).
    pub const fn key(self) -> &'static str {
        match self {
            SitePage::Home => "home",
            SitePage::Events => "events",
            SitePage::Organizers => "organizers",
            SitePage::Sponsors => "sponsors",
        }
    }

    /// The header link label. The landing has none: the logo is its link.
    pub fn nav_label(self) -> Option<fn(Locale) -> &'static str> {
        match self {
            SitePage::Home => None,
            SitePage::Events => Some(|l| td_string!(l, landing.site.nav_events)),
            SitePage::Organizers => Some(|l| td_string!(l, landing.site.nav_organizers)),
            SitePage::Sponsors => Some(|l| td_string!(l, landing.site.nav_sponsors)),
        }
    }

    fn door(self) -> DoorCopy {
        match self {
            SitePage::Home => DoorCopy {
                title: |l| td_string!(l, landing.site.door_home_title),
                line: |l| td_string!(l, landing.site.door_home_line),
                cta: |l| td_string!(l, landing.site.door_home_cta),
            },
            SitePage::Events => DoorCopy {
                title: |l| td_string!(l, landing.site.door_events_title),
                line: |l| td_string!(l, landing.site.door_events_line),
                cta: |l| td_string!(l, landing.site.door_events_cta),
            },
            SitePage::Organizers => DoorCopy {
                title: |l| td_string!(l, landing.site.door_organizers_title),
                line: |l| td_string!(l, landing.site.door_organizers_line),
                cta: |l| td_string!(l, landing.site.door_organizers_cta),
            },
            SitePage::Sponsors => DoorCopy {
                title: |l| td_string!(l, landing.site.door_sponsors_title),
                line: |l| td_string!(l, landing.site.door_sponsors_line),
                cta: |l| td_string!(l, landing.site.door_sponsors_cta),
            },
        }
    }
}

/// The doors shown on `here`: every other page, in [`SitePage::ALL`] order.
pub fn doors_from(here: SitePage) -> impl Iterator<Item = SitePage> {
    SitePage::ALL.into_iter().filter(move |p| *p != here)
}

/// Count a door click (.plans/045 R4.10): page × door × day, no cookies, no
/// id. `sendBeacon` survives the navigation the click starts.
pub fn count_click(page: SitePage, door: &'static str) {
    if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
        let body = format!(r#"{{"page":"{}","door":"{door}"}}"#, page.key());
        let _ = nav.send_beacon_with_opt_str("/api/public/click", Some(&body));
    }
}

/// The doors section at the foot of a page.
#[component]
pub fn Doors(here: SitePage) -> impl IntoView {
    let heading = match here {
        SitePage::Home => tr(|l| td_string!(l, landing.site.doors_home)),
        _ => tr(|l| td_string!(l, landing.site.doors_other)),
    };
    let cards = doors_from(here)
        .map(|page| {
            let copy = page.door();
            view! {
                <a class="lp-door" href=page.path() on:click=move |_| count_click(here, page.key())>
                    <b>{tr(copy.title)}</b>
                    <span>{tr(copy.line)}</span>
                    <i>{tr(copy.cta)}</i>
                </a>
            }
        })
        .collect::<Vec<_>>();
    view! {
        <section id="doors" class="lp-doors-sec">
            <div class="lp-wrap">
                <h2 class="lp-h2">{heading}</h2>
                <div class="lp-doors">
                    {cards}
                    {try_band(TRY_LIVE, here)}
                </div>
            </div>
        </section>
    }
}

/// The try band, or nothing while the sandbox is not live.
pub fn try_band(live: bool, here: SitePage) -> Option<AnyView> {
    if !live {
        return None;
    }
    Some(
        view! {
            <a class="lp-door lp-door-try" href=TRY_PATH on:click=move |_| count_click(here, "try")>
                <b>{tr(|l| td_string!(l, landing.site.try_title))}</b>
                <span>{tr(|l| td_string!(l, landing.site.try_line))}</span>
                <i>{tr(|l| td_string!(l, landing.site.try_cta))}</i>
            </a>
        }
        .into_any(),
    )
}

/// The one-line try link under the organizers' swimlane, or nothing while
/// the sandbox is not live.
pub fn try_line(live: bool) -> Option<AnyView> {
    if !live {
        return None;
    }
    Some(
        view! {
            <p class="lp-try-link">
                <a href=TRY_PATH>{tr(|l| td_string!(l, landing.site.try_usdc_line))}</a>
            </p>
        }
        .into_any(),
    )
}
