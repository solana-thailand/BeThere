//! Generative event posters (F4, `src/utils/poster.rs`): the properties the
//! event hero and /discover rely on.

use event_checkin_frontend::utils::poster::{INKS, PAPER, poster_data_url, poster_svg};

const SLUGS: [&str; 6] = [
    "e2e-builders-night",
    "e2e-free-workshop",
    "rust-bangkok-meetup-7",
    "solana-hacker-house-bkk",
    "a",
    "",
];

/// Same slug, same poster: on every device and every render, with no storage.
#[test]
fn same_slug_draws_the_same_poster() {
    for slug in SLUGS {
        assert_eq!(poster_svg(slug), poster_svg(slug), "{slug:?}");
    }
}

/// The point of F4: poster-less events stop looking identical.
#[test]
fn different_slugs_draw_different_posters() {
    let posters: std::collections::BTreeSet<String> = SLUGS.iter().map(|s| poster_svg(s)).collect();
    assert_eq!(posters.len(), SLUGS.len());
}

/// No new colours: every fill is the paper or one of the brand inks.
#[test]
fn only_palette_colours() {
    for slug in SLUGS {
        let svg = poster_svg(slug);
        for fill in svg.split("fill=\"").skip(1) {
            let colour = fill.split('"').next().unwrap_or_default();
            assert!(
                colour == PAPER || INKS.contains(&colour),
                "{slug:?} uses {colour:?}"
            );
        }
    }
}

#[test]
fn is_one_svg_document() {
    for slug in SLUGS {
        let svg = poster_svg(slug);
        assert!(
            svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
            "{slug:?}"
        );
        assert!(svg.ends_with("</svg>"), "{slug:?}");
        assert_eq!(svg.matches("<svg").count(), 1, "{slug:?}");
    }
}

/// The data URL must survive as an `<img src>`: no raw `#` (it would start a
/// fragment), `<`, `>` or `"`, and it stays small enough to inline.
#[test]
fn data_url_is_url_safe_and_small() {
    for slug in SLUGS {
        let url = poster_data_url(slug);
        assert!(
            url.starts_with("data:image/svg+xml;charset=utf-8,"),
            "{slug:?}"
        );
        for bad in ['#', '<', '>', '"'] {
            assert!(!url.contains(bad), "{slug:?} contains {bad:?}");
        }
        assert!(url.len() < 4096, "{slug:?} is {} bytes", url.len());
    }
}
