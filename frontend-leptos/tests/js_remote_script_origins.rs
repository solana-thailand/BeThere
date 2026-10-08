//! Every script a `js/*.js` file loads from another origin must be allowed by
//! the CSP `script-src` the Worker sends (`worker/src/middleware/headers.rs`).
//!
//! `mobile_wallet.js` imported the Mobile Wallet Adapter from esm.sh, which
//! `script-src` never listed. Chrome refused the import and the catch block
//! logged it, so Android wallets never registered in prod and nothing failed
//! (`.issues/189`). The MWA bundle is now self-hosted; this test fails the next
//! time a loader points at an origin the policy does not grant.
//!
//! A load is a string literal `http(s)://…` that reaches a script sink:
//! `import(…)`, `.src = …` or a `…Script(…)` helper, either on the same line or
//! through a `var|let|const NAME =` that a sink later names.

use std::fs;
use std::path::Path;

const HEADERS_RS: &str = include_str!("../../worker/src/middleware/headers.rs");
const MOBILE_WALLET: &str = include_str!("../js/mobile_wallet.js");
const INDEX: &str = include_str!("../index.html");
const MWA_BUNDLE: &str = include_str!("../vendor/mwa-wallet-standard-mobile-0.5.3.js");

/// The source tokens of `script-src` in the Worker's CSP.
fn script_src_sources(headers_rs: &str) -> Vec<String> {
    let start = headers_rs
        .find("script-src ")
        .expect("headers.rs has no script-src directive");
    let directive = &headers_rs[start..];
    let end = directive.find(';').expect("script-src is not terminated");
    directive[..end]
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect()
}

/// `scheme://host[:port]` of an absolute URL.
fn origin_of(url: &str) -> String {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let end = url[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |i| after_scheme + i);
    url[..end].to_string()
}

/// Absolute URLs inside string literals on one line.
fn url_literals(line: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for quote in ['"', '\'', '`'] {
        let mut parts = line.split(quote).skip(1).step_by(2);
        for literal in parts.by_ref() {
            if literal.starts_with("https://") || literal.starts_with("http://") {
                urls.push(literal.to_string());
            }
        }
    }
    urls
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with('*') || t.starts_with("/*")
}

fn has_sink(line: &str) -> bool {
    line.contains("import(") || line.contains(".src =") || line.contains("Script(")
}

/// The binding a line declares (`var NAME =`, `const NAME =`, `let NAME =`).
fn declared_name(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = ["var ", "let ", "const "]
        .iter()
        .find_map(|kw| t.strip_prefix(kw))?;
    let name = rest.split(|c: char| c == '=' || c.is_whitespace()).next()?;
    match name.is_empty() {
        true => None,
        false => Some(name),
    }
}

/// True if a sink in `src` is fed the binding `name`.
fn sink_uses(src: &str, name: &str) -> bool {
    src.lines().filter(|l| !is_comment(l)).any(|l| {
        has_sink(l)
            && (l.contains(&format!("import({name}"))
                || l.contains(&format!("= {name}"))
                || l.contains(&format!("Script({name}")))
    })
}

/// Every remote URL `src` loads as a script.
fn remote_script_urls(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if is_comment(line) {
            continue;
        }
        let urls = url_literals(line);
        if urls.is_empty() {
            continue;
        }
        // `var NAME =` may sit on this line or, when prettier wrapped a long
        // literal, on the line above.
        let name = declared_name(line).or_else(|| {
            i.checked_sub(1)
                .map(|p| lines[p])
                .filter(|p| p.trim_end().ends_with('='))
                .and_then(declared_name)
        });
        let loaded = has_sink(line) || name.is_some_and(|n| sink_uses(src, n));
        if loaded {
            found.extend(urls);
        }
    }
    found
}

fn js_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("js");
    let mut out: Vec<(String, String)> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "js"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_remote_script_origin_is_in_csp_script_src() {
    let allowed = script_src_sources(HEADERS_RS);
    assert!(allowed.iter().any(|s| s == "'self'"), "parsed {allowed:?}");
    let sources = js_sources();
    assert!(
        sources.len() >= 10,
        "walked too few files: {}",
        sources.len()
    );

    let mut loads = Vec::new();
    for (file, src) in &sources {
        for url in remote_script_urls(src) {
            loads.push((file.clone(), origin_of(&url)));
        }
    }
    // Blindness floor: the web3 CDN pair, Telegram and Turnstile are real
    // remote loads today. Finding fewer means the scanner went blind.
    assert!(loads.len() >= 4, "scanner found only {loads:?}");

    let blocked: Vec<_> = loads
        .iter()
        .filter(|(_, origin)| !allowed.contains(origin))
        .collect();
    assert!(
        blocked.is_empty(),
        "these js/ loaders fetch scripts the CSP script-src blocks \
         (self-host them, like vendor/): {blocked:?}; script-src = {allowed:?}"
    );
}

#[test]
fn scanner_flags_the_esm_sh_import_that_shipped() {
    // The exact shape that reached prod (.issues/189), wrapped by prettier.
    let shipped = "  var MWA_LIB_URL =\n    \"https://esm.sh/@solana-mobile/wallet-standard-mobile@0.5.3\";\n  var mod = await import(MWA_LIB_URL);\n";
    let urls = remote_script_urls(shipped);
    assert_eq!(
        urls,
        ["https://esm.sh/@solana-mobile/wallet-standard-mobile@0.5.3"]
    );
    let allowed = script_src_sources(HEADERS_RS);
    assert!(!allowed.contains(&origin_of(&urls[0])));

    for direct in [
        "const m = await import(\"https://esm.sh/x\");",
        "s.src = \"https://evil.example/a.js\";",
        "var script = web3Script(\"https://cdn.example/\");",
    ] {
        assert_eq!(remote_script_urls(direct).len(), 1, "{direct}");
    }
    // An RPC endpoint is a connect-src fetch, not a script load.
    assert!(remote_script_urls("    return \"https://api.devnet.solana.com\";").is_empty());
}

#[test]
fn mwa_loads_the_same_origin_bundle_that_the_build_ships() {
    assert!(
        MOBILE_WALLET.contains("var MWA_LIB_URL = \"/mwa-wallet-standard-mobile-0.5.3.js\";"),
        "MWA_LIB_URL must be the same-origin vendored bundle"
    );
    assert!(remote_script_urls(MOBILE_WALLET).is_empty());
    assert!(INDEX.contains(
        r#"<link data-trunk rel="copy-file" href="vendor/mwa-wallet-standard-mobile-0.5.3.js" />"#
    ));
    assert!(
        INDEX.contains("vendor/mwa-wallet-standard-mobile-0.5.3.LICENSE.txt"),
        "Apache-2.0 and MIT require shipping the licences with the bundle"
    );
}

#[test]
fn mwa_bundle_is_self_contained_and_eval_free() {
    assert!(MWA_BUNDLE.len() > 100_000, "bundle looks truncated");
    // The exports mobile_wallet.js reads.
    for export in [
        "as registerMwa",
        "as createDefaultAuthorizationCache",
        "as createDefaultChainSelector",
        "as createDefaultWalletNotFoundHandler",
    ] {
        assert!(MWA_BUNDLE.contains(export), "bundle lost `{export}`");
    }
    // The CSP has no 'unsafe-eval'; a bundled eval would throw at runtime.
    for banned in ["eval(", "new Function(", "Function(\""] {
        assert!(!MWA_BUNDLE.contains(banned), "bundle contains `{banned}`");
    }
    // One file: no static or dynamic import of anything else.
    assert!(!MWA_BUNDLE.contains("import("), "bundle imports at runtime");
    for static_import in ["import{", "import\"", "import*", "import \"", "import {"] {
        assert!(
            !MWA_BUNDLE.contains(static_import),
            "bundle has a static import (`{static_import}`)"
        );
    }
    // The Local Network Access mitigation (>= 0.5.0) that pinned 0.5.3.
    assert!(MWA_BUNDLE.contains("loopback-network"));
}
