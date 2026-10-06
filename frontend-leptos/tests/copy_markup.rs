//! The landing copy markup (ASKS-4 §15): keyword links, highlights, and
//! plain text for anything that is not a closed marker.

use event_checkin_frontend::utils::copy_markup::{Segment, parse};

#[test]
fn hero_sub_line_links_its_keywords() {
    let got = parse("{Free|#sponsors} events. Hold your seat with a {deposit|#how}.\nShow up.");
    assert_eq!(
        got,
        vec![
            Segment::Link {
                text: "Free",
                href: "#sponsors"
            },
            Segment::Text(" events. Hold your seat with a "),
            Segment::Link {
                text: "deposit",
                href: "#how"
            },
            Segment::Text(".\nShow up."),
        ]
    );
}

#[test]
fn thai_text_and_highlights() {
    assert_eq!(
        parse("งาน{ฟรี|#sponsors} **คืนเต็ม**"),
        vec![
            Segment::Text("งาน"),
            Segment::Link {
                text: "ฟรี",
                href: "#sponsors"
            },
            Segment::Text(" "),
            Segment::Highlight("คืนเต็ม"),
        ]
    );
}

#[test]
fn plain_text_is_one_segment() {
    assert_eq!(parse("Show up."), vec![Segment::Text("Show up.")]);
    assert!(parse("").is_empty());
}

#[test]
fn unclosed_or_malformed_markers_stay_text() {
    let joined = |s: &str| {
        parse(s)
            .into_iter()
            .map(|seg| match seg {
                Segment::Text(t) => t.to_string(),
                other => panic!("{s:?} gave {other:?}"),
            })
            .collect::<String>()
    };
    for s in [
        "{open",
        "{no bar}",
        "{text|nohash}",
        "{|#x}",
        "{t|#}",
        "**open",
        "a ** b",
    ] {
        assert_eq!(joined(s), s);
    }
}

#[test]
fn nested_brace_takes_the_inner_link() {
    assert_eq!(
        parse("{a {b|#c}"),
        vec![
            Segment::Text("{"),
            Segment::Text("a "),
            Segment::Link {
                text: "b",
                href: "#c"
            }
        ]
    );
}
