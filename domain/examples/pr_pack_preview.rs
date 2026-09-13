//! Print a PR pack for two realistic events so the generated copy can be read.
//!
//! `.plans/008` Phase 4 leaves one validation open: *"copy a generated social
//! post → post to a test account → confirm readability."* Posting is a manual,
//! outward-facing step, but the readability half needs only the text — and
//! reading it out of a browser card is not required to spot doubled punctuation,
//! an awkward truncation, a wrong date format or a broken link.
//!
//! The two fixtures mirror real events from `.plans/018` §8: the recurring
//! hybrid with a THB 500 deposit, and a no-deposit online session.
//!
//! Run: `cargo run -p event-checkin-domain --example pr_pack_preview`

use event_checkin_domain::models::event::EventConfig;
use event_checkin_domain::pr_pack;

/// Build from JSON rather than a struct literal: `EventConfig` has ~50 fields
/// and no `Default`, so a literal would bury the handful the copy actually
/// depends on. Most fields carry `#[serde(default)]`; the seven that do not
/// (`link`, `sheet_id`, `sheet_name`, `staff_sheet_name`, `created_at`,
/// `updated_at`) are supplied by `REQUIRED` below; `tagline` is also required
/// but each fixture states it, since an empty tagline changes the copy.
fn event(fields: &str) -> EventConfig {
    const REQUIRED: &str = r#""link":"","sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":"""#;
    serde_json::from_str(&format!("{{{REQUIRED},{fields}}}")).expect("fixture must deserialize")
}

fn hybrid_with_deposit() -> EventConfig {
    event(
        r#"
        "id": "evt-road-to-mainnet",
        "name": "Solana x AI Builders: Road to Mainnet",
        "slug": "solana-ai-builders-road-to-mainnet",
        "tagline": "Ship an on-chain AI app in one afternoon",
        "status": "active",
        "event_start_ms": 1787486400000,
        "event_end_ms": 1787500800000,
        "location": "Bangkok, Thailand",
        "claim_base_url": "https://bethere.app/claim",
        "calendar_subscribe_url": "https://bethere.app/calendar.ics",
        "organizer_emails": ["hello@solanathailand.org"],
        "deposit_enabled": true,
        "deposit_amount_thb": 500,
        "refund_deadline_hours": 168,
        "max_refundable_deposits": 50,
        "event_format": "hybrid",
        "require_contact_info": true
    "#,
    )
}

fn online_no_deposit() -> EventConfig {
    event(
        r#"
        "id": "evt-latent-space-7",
        "name": "Solana in Latent Space Part 7",
        "slug": "solana-in-latent-space-7",
        "tagline": "",
        "status": "active",
        "event_start_ms": 1785330000000,
        "event_end_ms": 1785333600000,
        "location": "Online",
        "claim_base_url": "https://bethere.app/claim",
        "event_format": "online"
    "#,
    )
}

fn show(label: &str, event: &EventConfig) {
    let pack = pr_pack::generate(event);
    println!("\n{:=<78}", "");
    println!("{label}");
    println!("{:=<78}\n", "");
    println!("HEADLINE\n  {}\n", pack.headline);
    println!("SHORT BLURB\n  {}\n", pack.short_blurb);
    println!(
        "SOCIAL POST  ({} chars — Twitter/X budget is 280)\n{}\n",
        pack.social_post.chars().count(),
        indent(&pack.social_post)
    );
    println!("CALENDAR\n{}\n", indent(&pack.calendar_text));
    println!("EMAIL SNIPPET\n{}\n", indent(&pack.email_snippet));
    println!("DEPOSIT TERMS\n{}\n", indent(&pack.deposit_terms));
    println!("ORGANIZERS\n  {:?}", pack.organizers);
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    show(
        "HYBRID · THB 500 deposit · recurring flagship",
        &hybrid_with_deposit(),
    );
    show("ONLINE · no deposit · no tagline", &online_no_deposit());
}
