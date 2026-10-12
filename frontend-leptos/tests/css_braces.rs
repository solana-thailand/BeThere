//! Every stylesheet's braces balance (comments ignored).
//!
//! A missing `}` does not fail the build: the CSS minifier and the browser
//! both swallow every rule after it into the unclosed block, so whole pages
//! silently lose their styling. A merge-conflict resolution dropped one in
//! `style-24-attendee-site.css` (.plans/046, the claim page kept its old
//! gradient cards); nothing else caught it.

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// The brace depth after `css` with `/* … */` comments removed, or the byte
/// offset where it first went negative.
fn brace_depth(css: &str) -> Result<i64, usize> {
    let mut depth = 0i64;
    let mut rest = css;
    let mut offset = 0;
    while let Some(i) = rest.find(['{', '}', '/']) {
        let at = offset + i;
        match rest.as_bytes()[i] {
            b'/' if rest[i..].starts_with("/*") => {
                let close = rest[i + 2..]
                    .find("*/")
                    .map_or(rest.len(), |j| i + 2 + j + 2);
                offset += close;
                rest = &rest[close..];
                continue;
            }
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err(at);
        }
        offset += i + 1;
        rest = &rest[i + 1..];
    }
    Ok(depth)
}

#[test]
fn every_stylesheet_closes_its_braces() {
    let mut checked = 0;
    for entry in std::fs::read_dir(format!("{ROOT}/styles")).expect("styles/") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "css") {
            continue;
        }
        let css = std::fs::read_to_string(&path).expect("read css");
        assert_eq!(brace_depth(&css), Ok(0), "{}", path.display());
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} stylesheets found");
}

#[test]
fn the_check_catches_a_missing_brace() {
    assert_eq!(brace_depth(".a { b: c; }"), Ok(0));
    assert_eq!(brace_depth(".a { b: c;\n.d { e: f; }"), Ok(1));
    assert_eq!(brace_depth("}"), Err(0));
    // braces inside comments do not count
    assert_eq!(brace_depth("/* { */ .a { }"), Ok(0));
}
