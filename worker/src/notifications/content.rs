use super::{Notification, policy::NotificationKind};
use event_checkin_domain::models::event::EventConfig;
use serde::Serialize;

#[derive(Serialize)]
pub struct Message {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn clean(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

/// Everything a message body is rendered from. Grouped rather than passed
/// positionally so a new field cannot be silently swapped with `online` or
/// `needs_deposit` at a call site.
pub struct Render<'a> {
    pub job: &'a Notification,
    pub kind: NotificationKind,
    pub event: &'a EventConfig,
    pub email: &'a str,
    pub name: &'a str,
    pub online: bool,
    pub needs_deposit: bool,
    pub base: &'a str,
}

pub fn render(
    Render {
        job,
        kind,
        event,
        email,
        name,
        online,
        needs_deposit,
        base,
    }: &Render<'_>,
) -> Message {
    let title = match *kind {
        NotificationKind::Registration => "Your registration is saved",
        NotificationKind::Reminder => "Your event is coming up",
        NotificationKind::DepositConfirmed => "Your deposit is confirmed",
        NotificationKind::DepositRejected => "Your payment slip needs attention",
        NotificationKind::Survey => "How were the sessions?",
    };

    // The survey is the one kind whose row no longer stands for an event.
    // `dedup_key` is per person (migration 0038), so the event this row happens
    // to be filed under is whichever one enrolled them first — of up to twelve.
    // Every event-shaped part of the message below is therefore wrong for it:
    // the subject would name one of twelve, When/Where would describe a session
    // that is over, and "Your ticket" would link to a past event's ticket.
    //
    // So it gets its own message rather than the shared one with a different
    // title. `/feedback` enumerates the sessions; the mail does not try to.
    if *kind == NotificationKind::Survey {
        let feedback = format!("{base}/feedback");
        let name = clean(name);
        return Message {
            to: (*email).into(),
            subject: format!("{title} — Solana Developer Thailand"),
            text: format!(
                "Hi {name},\n\nThank you for coming to the sessions you attended this year.\n\nWe are writing up what to run next, and the one thing we do not have is what you thought. It takes about two minutes, every question is skippable, and answers go to the organising team — nothing is attributed by name.\n\nShare your feedback: {feedback}\n\nThe page lists every session you attended, so you only need to open it once.\n\nThis is an event service message from BeThere."
            ),
            html: format!(
                "<h1>{}</h1><p>Hi {},</p><p>Thank you for coming to the sessions you attended this year.</p><p>We are writing up what to run next, and the one thing we do not have is what you thought. It takes about two minutes, every question is skippable, and answers go to the organising team — nothing is attributed by name.</p><p><a href=\"{}\">Share your feedback</a></p><p>The page lists every session you attended, so you only need to open it once.</p><p>This is an event service message from BeThere.</p>",
                escape(title),
                escape(&name),
                escape(&feedback)
            ),
        };
    }
    let id = urlencoding::encode(&job.attendee_id);
    let event_id = urlencoding::encode(&job.event_id);
    let ticket = format!("{base}/ticket/{id}?event_id={event_id}");
    // The survey's questions are asked by the post-event registration form, not
    // by the mail — sending someone to their ticket would be a dead end, and
    // there is nothing left to deposit for an event that is already over.
    let (action, action_label) = match *kind {
        // One page for every event this person still owes feedback on, so a
        // regular attendee follows one link instead of one per event
        // (`.issues/091`). The single-event route is unchanged and still
        // serves the QR codes on the recap posters.
        NotificationKind::Survey => (format!("{base}/feedback"), "Share your feedback"),
        NotificationKind::DepositRejected => (
            format!("{base}/deposit/{id}?event_id={event_id}"),
            "Review your payment and upload a new slip",
        ),
        _ if *needs_deposit => (
            format!("{base}/deposit/{id}?event_id={event_id}"),
            "Complete your deposit to secure your spot",
        ),
        _ => (ticket.clone(), "Open your ticket"),
    };
    let start = chrono::DateTime::from_timestamp_millis(event.event_start_ms);
    let end = chrono::DateTime::from_timestamp_millis(event.event_end_ms);
    let when = if event.time_tba {
        "Time to be announced".into()
    } else {
        start
            .map(|d| d.format("%a, %d %b %Y at %H:%M UTC").to_string())
            .unwrap_or_else(|| "See the event page for the time".into())
    };
    let location = if *online {
        "Online — open your ticket for the session link"
    } else {
        &event.location
    };
    let mut text = format!(
        "Hi {},\n\n{title}: {}\nWhen: {when}\nWhere: {location}\n\n{action_label}: {action}\nYour ticket: {ticket}\n",
        clean(name),
        clean(&event.name)
    );
    let mut html = format!(
        "<h1>{}</h1><p>Hi {},</p><h2>{}</h2><p><strong>When:</strong> {}<br><strong>Where:</strong> {}</p><p><a href=\"{}\">{}</a></p><p><a href=\"{}\">Your ticket</a></p>",
        escape(title),
        escape(name),
        escape(&event.name),
        escape(&when),
        escape(location),
        escape(&action),
        escape(action_label),
        escape(&ticket)
    );
    if *kind != NotificationKind::Survey
        && !event.time_tba
        && let (Some(start), Some(end)) = (start, end)
        && end > start
    {
        let dates = format!(
            "{}/{}",
            start.format("%Y%m%dT%H%M%SZ"),
            end.format("%Y%m%dT%H%M%SZ")
        );
        let calendar = format!(
            "https://calendar.google.com/calendar/render?action=TEMPLATE&text={}&dates={}&details={}&location={}",
            urlencoding::encode(&event.name),
            urlencoding::encode(&dates),
            urlencoding::encode(&ticket),
            urlencoding::encode(location)
        );
        text.push_str(&format!("Add to calendar: {calendar}\n"));
        html.push_str(&format!(
            "<p><a href=\"{}\">Add to calendar</a></p>",
            escape(&calendar)
        ));
    }
    text.push_str("\nThis is an event service message from BeThere. Sign in with your registration account to open your ticket.");
    html.push_str("<p>This is an event service message from BeThere. Sign in with your registration account to open your ticket.</p>");
    Message {
        to: (*email).into(),
        subject: format!("{title} — {}", clean(&event.name)),
        text,
        html,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_untrusted_html_and_headers() {
        assert_eq!(escape("<script>\"&"), "&lt;script&gt;&quot;&amp;");
        assert_eq!(clean("hello\r\nBcc: victim"), "hello  Bcc: victim");
    }
    fn fixture() -> (Notification, EventConfig) {
        let job = serde_json::from_value(serde_json::json!({
            "id":1,"event_id":"event&one","attendee_id":"a/one","kind":"registration",
            "version":"","status":"pending","attempts":0,"due_at":0,
            "attempted_at":null,"message_id":null,"error_code":null
        }))
        .unwrap();
        let event = crate::db::events::D1EventRow {
            name: Some("<Builders>\r\nBcc: test".into()),
            slug: Some("builders/one".into()),
            event_start_ms: Some(1_800_000_000_000),
            event_end_ms: Some(1_800_003_600_000),
            location: Some("Venue & friends".into()),
            ..Default::default()
        }
        .to_event_config();
        (job, event)
    }
    #[test]
    fn confirmation_has_safe_links_calendar_and_both_bodies() {
        let (job, event) = fixture();
        let message = render(&Render {
            job: &job,
            kind: NotificationKind::Registration,
            event: &event,
            email: "a@example.com",
            name: "<img src=x>",
            online: false,
            needs_deposit: true,
            base: "https://bethere.example",
        });
        assert!(!message.subject.contains(['\r', '\n']));
        assert!(!message.html.contains("<img"));
        assert!(message.html.contains("&lt;Builders&gt;"));
        assert!(
            message
                .text
                .contains("/deposit/a%2Fone?event_id=event%26one")
        );
        assert!(message.html.contains("Add to calendar"));
        assert!(message.text.contains("UTC"));
        assert!(message.text.contains("Your ticket:"));
    }
    #[test]
    fn online_ticket_and_unknown_time_do_not_offer_payment_or_calendar() {
        let (job, mut event) = fixture();
        event.time_tba = true;
        let message = render(&Render {
            job: &job,
            kind: NotificationKind::Registration,
            event: &event,
            email: "a@example.com",
            name: "Builder",
            online: true,
            needs_deposit: false,
            base: "https://bethere.example",
        });
        assert!(!message.text.contains("/deposit/"));
        assert!(!message.text.contains("Add to calendar"));
        assert!(message.text.contains("Time to be announced"));
        assert!(message.text.contains("session link"));
    }
    /// The survey is the only kind sent after the event: it must point at the
    /// page that carries the questions, never at a deposit that can no longer be
    /// paid or a calendar entry for a date that has passed.
    ///
    /// That page is `/feedback`, which collects every event this person still
    /// owes feedback on (`.issues/091`) — deliberately *not* the single-event
    /// route, so one message does not become one message per event attended.
    #[test]
    fn survey_points_at_the_combined_feedback_page_not_a_deposit_or_calendar() {
        let (job, event) = fixture();
        let message = render(&Render {
            job: &job,
            kind: NotificationKind::Survey,
            event: &event,
            email: "a@example.com",
            name: "Builder",
            online: false,
            needs_deposit: true, // an unpaid deposit must not hijack a post-event message
            base: "https://bethere.example",
        });
        assert!(message.text.contains("https://bethere.example/feedback"));
        // The single-event route is still live for the recap QR codes, but a
        // notification must not send anyone down it — that is the regression
        // this whole page exists to prevent.
        assert!(!message.text.contains("/post-event-register"));
        assert!(!message.text.contains("/deposit/"));
        assert!(!message.text.contains("Add to calendar"));
        assert!(message.subject.starts_with("How were the sessions?"));
        // One row now stands for every session this person attended (migration
        // 0038), so the message must not claim to be about one of them — not in
        // the subject, and not as a When/Where/ticket for an event that is over.
        assert!(!message.subject.contains(&event.name));
        assert!(!message.text.contains(&event.name));
        assert!(!message.text.contains("When:"));
        assert!(!message.text.contains("Where:"));
        assert!(!message.text.contains("Your ticket:"));
    }
}
