//! Every `var(--token)` the frontend reads is defined somewhere (.plans/037 §5).
//!
//! An undefined custom property does not fail loudly: the declaration is
//! invalid at computed-value time and the property falls back to inherited or
//! initial. The claim quiz read `--fg`, `--muted`, `--primary` and `--bg`,
//! none of which existed, so its heading colour, icon colour and
//! selected-option border silently vanished; admin feedback used an indigo
//! `--color-primary` fallback that was never on the palette.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Read by keyframes in `style-08-core.css` that nothing sets;
/// `js/confetti.js` injects its own keyframes. Remove the entries (or the
/// dead keyframes) when that is cleaned up.
const KNOWN_UNDEFINED: &[&str] = &["--drift", "--rotation", "--sway"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => files(&path, ext, out),
            false => {
                if path.extension().is_some_and(|e| e == ext) {
                    out.push(path);
                }
            }
        }
    }
}

/// The custom-property name starting at `bytes[at]` (`--` included).
fn token_at(text: &str, at: usize) -> Option<&str> {
    let rest = &text[at..];
    if !rest.starts_with("--") {
        return None;
    }
    let len = rest
        .char_indices()
        .skip(2)
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    (len > 2).then(|| &rest[..len])
}

/// (tokens read with `var(`, tokens defined with `--name:`), by file.
fn scan() -> (BTreeMap<String, BTreeSet<String>>, BTreeSet<String>) {
    let mut sources = Vec::new();
    files(&root().join("styles"), "css", &mut sources);
    files(&root().join("src"), "rs", &mut sources);
    sources.push(root().join("index.html"));

    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut defined = BTreeSet::new();
    for path in sources {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = path
            .strip_prefix(root())
            .unwrap_or(&path)
            .display()
            .to_string();
        for (i, _) in text.match_indices("var(") {
            let after = text[i + 4..].trim_start();
            let at = text.len() - after.len();
            if let Some(token) = token_at(&text, at) {
                used.entry(token.to_string())
                    .or_default()
                    .insert(name.clone());
            }
        }
        for (i, _) in text.match_indices("--") {
            if let Some(token) = token_at(&text, i)
                && text[i + token.len()..].trim_start().starts_with(':')
            {
                defined.insert(token.to_string());
            }
        }
    }
    (used, defined)
}

#[test]
fn every_token_read_is_defined() {
    let (used, defined) = scan();
    assert!(used.len() > 30, "the scan must see the stylesheets");
    let missing: Vec<String> = used
        .iter()
        .filter(|(token, _)| {
            !defined.contains(*token) && !KNOWN_UNDEFINED.contains(&token.as_str())
        })
        .map(|(token, where_)| format!("{token} in {where_:?}"))
        .collect();
    assert!(
        missing.is_empty(),
        "undefined CSS custom properties; use a :root token instead:\n{}",
        missing.join("\n")
    );
}

#[test]
fn known_undefined_list_has_no_stale_entries() {
    let (used, defined) = scan();
    let stale: Vec<&&str> = KNOWN_UNDEFINED
        .iter()
        .filter(|t| defined.contains(**t) || !used.contains_key(**t))
        .collect();
    assert!(stale.is_empty(), "remove from KNOWN_UNDEFINED: {stale:?}");
}
