use super::Notification;
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

pub fn render(
    job: &Notification,
    event: &EventConfig,
    email: &str,
    name: &str,
    online: bool,
    needs_deposit: bool,
    base: &str,
) -> Result<Message, String> {
    let title = match job.kind.as_str() {
        "registration" => "Your registration is saved",
        "reminder" => "Your event is coming up",
        "deposit_confirmed" => "Your deposit is confirmed",
        "deposit_rejected" => "Your payment slip needs attention",
        _ => return Err("unknown notification kind".into()),
    };
    let id = urlencoding::encode(&job.attendee_id);
    let event_id = urlencoding::encode(&job.event_id);
    let ticket = format!("{base}/ticket/{id}?event_id={event_id}");
    let action = if needs_deposit {
        format!("{base}/deposit/{id}?event_id={event_id}")
    } else {
        ticket.clone()
    };
    let action_label = if job.kind == "deposit_rejected" {
        "Review your payment and upload a new slip"
    } else if needs_deposit {
        "Complete your deposit to secure your spot"
    } else {
        "Open your ticket"
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
    let location = if online {
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
    if !event.time_tba
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
    Ok(Message {
        to: email.into(),
        subject: format!("{title} — {}", clean(&event.name)),
        text,
        html,
    })
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
        let message = render(
            &job,
            &event,
            "a@example.com",
            "<img src=x>",
            false,
            true,
            "https://bethere.example",
        )
        .unwrap();
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
        let message = render(
            &job,
            &event,
            "a@example.com",
            "Builder",
            true,
            false,
            "https://bethere.example",
        )
        .unwrap();
        assert!(!message.text.contains("/deposit/"));
        assert!(!message.text.contains("Add to calendar"));
        assert!(message.text.contains("Time to be announced"));
        assert!(message.text.contains("session link"));
    }
    #[test]
    fn reject_unknown_message_kind() {
        let (mut job, event) = fixture();
        job.kind = "marketing".into();
        assert!(
            render(
                &job,
                &event,
                "a@example.com",
                "Builder",
                false,
                false,
                "https://bethere.example"
            )
            .is_err()
        );
    }
}
