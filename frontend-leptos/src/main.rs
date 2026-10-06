#![forbid(unsafe_code)]

use event_checkin_frontend::App;

fn main() {
    #[cfg(feature = "console_log")]
    console_log::init_with_level(log::Level::Debug).expect("could not init logger");
    event_checkin_frontend::remove_boot_summary();
    leptos::mount::mount_to_body(App);
}
