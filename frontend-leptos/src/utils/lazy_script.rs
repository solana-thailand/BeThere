//! Off-first-load scripts (the goal globe, the lit room): a file copied to
//! `dist/` with no tag, injected on demand, which then exposes
//! `window.<global>.mount`. One loader, so each bridge only says what to mount.

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

/// `window.<global>.mount`, once the script that defines it has run.
pub fn mount_fn(global: &str) -> Option<js_sys::Function> {
    let window = web_sys::window()?;
    let obj = js_sys::Reflect::get(&window, &global.into()).ok()?;
    js_sys::Reflect::get(&obj, &"mount".into())
        .ok()?
        .dyn_into()
        .ok()
}

/// Hand `window.<global>.mount` to `then`, injecting `src` first if it has
/// not run yet. Logs, and does nothing else, if the script fails to define it.
pub fn with_mount(
    src: &'static str,
    global: &'static str,
    then: impl FnOnce(js_sys::Function) + 'static,
) {
    if let Some(mount) = mount_fn(global) {
        then(mount);
        return;
    }
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(script) = doc
        .create_element("script")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let _ = script.set_attribute("src", src);
    let onload = Closure::once_into_js(move || match mount_fn(global) {
        Some(mount) => then(mount),
        None => log::warn!("[lazy_script] {src} loaded without {global}.mount"),
    });
    script.set_onload(Some(onload.unchecked_ref()));
    if let Some(head) = doc.head() {
        let _ = head.append_child(&script);
    }
}
