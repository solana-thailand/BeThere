//! Fonts are self-hosted (plan 046 Phase 2): no request leaves for Google, and
//! every font file the page names exists, so a renamed or missing file is a
//! test failure, not text quietly falling back to the system face.

use std::collections::BTreeSet;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every `/fonts/<file>` named in `text`.
fn font_files(text: &str) -> BTreeSet<String> {
    text.split("/fonts/")
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(|c| !matches!(c, '"' | ')' | '\'' | ' '))
                .collect()
        })
        .collect()
}

#[test]
fn every_named_font_file_exists() {
    let css = read("styles/style-00-fonts.css");
    let html = read("index.html");
    let named: BTreeSet<String> = font_files(&css)
        .union(&font_files(&html))
        .cloned()
        .collect();
    assert!(
        named.len() >= 7,
        "expected the seven slices, found {named:?}"
    );
    for file in &named {
        assert!(file.ends_with(".woff2"), "{file}");
        let bytes = std::fs::read(format!("{ROOT}/fonts/{file}"))
            .unwrap_or_else(|e| panic!("fonts/{file}: {e}"));
        assert_eq!(&bytes[..4], b"wOF2", "fonts/{file} is not WOFF2");
    }
    // the preload must be one of the faces the stylesheet declares, and the
    // Basic Latin slice of Inter: a glob once picked `inter-latin-ext-…`
    // (85 KB, the "e" of "ext" is a hex digit) instead of `inter-latin-…` (48 KB)
    for file in font_files(&html) {
        assert!(
            font_files(&css).contains(&file),
            "index.html preloads {file}, which no @font-face uses"
        );
        let hash = file
            .strip_prefix("inter-latin-")
            .and_then(|r| r.strip_suffix(".woff2"));
        assert!(
            hash.is_some_and(|h| h.len() == 12 && h.bytes().all(|b| b.is_ascii_hexdigit())),
            "index.html should preload Inter's Basic Latin slice, not {file}"
        );
    }
}

#[test]
fn nothing_loads_from_google_fonts() {
    for path in ["index.html", "_headers", "styles/style-00-fonts.css"] {
        let text = read(path);
        assert!(!text.contains("fonts.googleapis.com"), "{path}");
        assert!(!text.contains("fonts.gstatic.com"), "{path}");
    }
}

#[test]
fn each_family_ships_its_licence() {
    let css = read("styles/style-00-fonts.css");
    for (family, licence) in [
        ("Inter", "LICENSE-Inter-OFL.txt"),
        ("Anuphan", "Anuphan-OFL.txt"),
        ("Bricolage Grotesque", "LICENSE-BricolageGrotesque-OFL.txt"),
        ("JetBrains Mono", "LICENSE-JetBrainsMono-OFL.txt"),
    ] {
        assert!(
            css.contains(&format!("font-family: \"{family}\"")),
            "{family} has no @font-face"
        );
        assert!(
            read(&format!("fonts/{licence}")).contains("SIL Open Font License"),
            "{licence}"
        );
    }
}
