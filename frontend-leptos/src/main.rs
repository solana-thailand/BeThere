use event_checkin_frontend::App;

fn main() {
    console_log::init_with_level(log::Level::Debug).expect("could not init logger");
    leptos::mount::mount_to_body(App);
}
