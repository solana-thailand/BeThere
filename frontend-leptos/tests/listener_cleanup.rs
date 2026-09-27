//! Plan 028 F5/F8: window listeners and timers must end with the component
//! that started them.
//!
//! Leptos 0.8's `WindowListenerHandle` has no `Drop` impl, so `let _ =` or
//! `drop(handle)` leaves the listener installed; only `.remove()` takes it
//! off. A forgotten raw `Closure` does the same. An `on_cleanup` inside a
//! `spawn_local` task runs after `.await`, where there is no owner, so it is
//! never called.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        match path.is_dir() {
            true => rust_sources(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => out.push(path),
            false => {}
        }
    }
}

fn sources() -> Vec<(PathBuf, String)> {
    let mut paths = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut paths,
    );
    paths
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).expect("read source");
            (p, text)
        })
        .collect()
}

/// The binding name in `let <name> =` directly before `window_event_listener(`.
fn bound_name(before: &str) -> Option<&str> {
    let rest = before.trim_end().strip_suffix('=')?.trim_end();
    let start = rest.rfind("let ")? + 4;
    let name = rest[start..].trim();
    match name.is_empty() || name == "_" || name.contains(char::is_whitespace) {
        true => None,
        false => Some(name),
    }
}

#[test]
fn every_window_listener_is_removed_by_name() {
    let mut bad = Vec::new();
    for (path, text) in sources() {
        for (at, _) in text.match_indices("window_event_listener(") {
            let before = &text[..at];
            let removed =
                bound_name(before).is_some_and(|name| text.contains(&format!("{name}.remove()")));
            if !removed {
                let line = text[..at].lines().count();
                bad.push(format!("{}:{line}", path.display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "bind the handle and call `.remove()` in on_cleanup: {bad:?}"
    );
}

#[test]
fn no_forgotten_keydown_closures() {
    let bad: Vec<_> = sources()
        .into_iter()
        .filter(|(_, text)| text.contains("add_event_listener_with_callback(\"keydown\""))
        .map(|(p, _)| p.display().to_string())
        .collect();
    assert!(
        bad.is_empty(),
        "use window_event_listener + remove(): {bad:?}"
    );
}

#[test]
fn public_event_countdown_interval_is_owned() {
    const PAGE: &str = include_str!("../src/pages/public_event/page.rs");
    let interval = PAGE
        .find("set_interval_with_handle(")
        .expect("countdown interval moved; update this guard");
    let first_task = PAGE
        .find("spawn_local(")
        .expect("fetch task moved; update this guard");
    assert!(
        interval < first_task,
        "the countdown interval must be set up outside the fetch task"
    );
    assert_eq!(PAGE.matches("set_interval_with_handle(").count(), 1);
}

#[test]
fn session_timer_stops_when_unmounted() {
    const WIDGETS: &str = include_str!("../src/pages/claim/widgets.rs");
    let start = WIDGETS
        .find("fn SessionTimer(")
        .expect("SessionTimer moved; update this guard");
    let body = &WIDGETS[start..];
    let body = &body[..body[1..]
        .find("\n#[component]")
        .map_or(body.len(), |i| i + 1)];
    assert!(body.contains(".try_set(label.to_string()).is_some()"));
}
