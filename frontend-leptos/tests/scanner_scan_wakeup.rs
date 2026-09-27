//! Plan 028 F3: a detected QR reaches Rust without a second poll tick.
//!
//! The JS detector already runs every 300 ms. Rust used to poll its result
//! on its own 300 ms tick as well, so a scan waited on both. Rust now awaits
//! `waitForScanEvent`, which the JS side resolves wherever it publishes a
//! result or an error, or stops. A publish site that forgets to notify
//! would fall back to the 1 s safety timeout: slower than before.

const JS: &str = include_str!("../js/scanner.js");
const PAGE: &str = include_str!("../src/pages/scanner/page.rs");

/// Lines that publish a result or an error (not the `= null` resets).
fn publish_sites() -> Vec<usize> {
    JS.lines()
        .enumerate()
        .filter(|(_, l)| {
            let l = l.trim();
            (l.starts_with("window.__qrResult =") || l.starts_with("window.__cameraError ="))
                && !l.ends_with("= null;")
        })
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn every_publish_wakes_the_waiters() {
    let lines: Vec<&str> = JS.lines().collect();
    let sites = publish_sites();
    assert!(sites.len() >= 4, "publish sites moved; update this guard");
    for i in sites {
        let window = &lines[i..(i + 5).min(lines.len())];
        assert!(
            window.iter().any(|l| l.contains("_notifyScanWaiters();")),
            "scanner.js line {} publishes without waking the Rust side",
            i + 1
        );
    }
}

#[test]
fn stopping_wakes_the_waiters() {
    let start = JS
        .find("export function stopCamera()")
        .expect("stopCamera moved");
    let body = &JS[start..];
    let body = &body[..body.find("\n}\n").expect("stopCamera end")];
    assert!(body.contains("_notifyScanWaiters();"));
}

#[test]
fn the_rust_loop_awaits_the_event_not_a_tick() {
    let start = PAGE
        .find("wait_for_scan_event_js(")
        .expect("scan loop moved; update this guard");
    let loop_body = &PAGE[start..start + 2000];
    assert!(!loop_body.contains("TimeoutFuture::new(300)"));
}
