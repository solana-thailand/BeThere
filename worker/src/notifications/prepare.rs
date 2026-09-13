//! Re-read eligibility and render immediately before transport.
use super::{
    Notification, content,
    policy::{
        DeliveryWindow, NotificationKind, ReminderTiming, in_delivery_window, reminder_timing,
    },
};
use crate::db::d1_safe::safe_all_rows;
use worker::{D1Database, d1::D1Type};

pub(super) enum Prepared {
    Ready(content::Message),
    Cancel,
    Defer(i64),
}

pub(super) async fn prepare(
    db: &D1Database,
    job: &Notification,
    base: &str,
) -> Result<Prepared, String> {
    // A kind the build does not know cannot be rendered or reasoned about.
    let Some(kind) = NotificationKind::parse(&job.kind) else {
        return Ok(Prepared::Cancel);
    };
    let Some(row) = crate::db::events::get_event(db, &job.event_id).await? else {
        return Ok(Prepared::Cancel);
    };
    let event = row.to_event_config();
    let now = chrono::Utc::now().timestamp_millis();
    if !in_delivery_window(
        kind.delivery_window(),
        &event.status,
        event.event_end_ms,
        now,
    ) {
        return Ok(Prepared::Cancel);
    }
    if kind == NotificationKind::Reminder {
        match reminder_timing(event.time_tba, event.event_start_ms, now) {
            ReminderTiming::Cancel => return Ok(Prepared::Cancel),
            ReminderTiming::Defer(until) => return Ok(Prepared::Defer(until)),
            ReminderTiming::Send => {}
        }
    }
    let stmt=db.prepare("SELECT a.email,a.name,a.participation_type,a.checked_in_at FROM attendees a JOIN notification_enrollments n ON n.attendee_id=a.id AND n.event_id=a.event_id WHERE a.id=?1 AND a.event_id=?2 AND a.approval_status='approved'")
        .bind_refs(&[D1Type::Text(&job.attendee_id),D1Type::Text(&job.event_id)]).map_err(|e|e.to_string())?;
    let Some(a) = safe_all_rows(&stmt).await?.into_iter().next() else {
        return Ok(Prepared::Cancel);
    };
    let email = a["email"].as_str().ok_or("missing email")?;
    if !event_checkin_domain::validation::is_plausible_email(email) {
        return Ok(Prepared::Cancel);
    }
    let checked_in = a["checked_in_at"].as_str().is_some_and(|s| !s.is_empty());
    if kind == NotificationKind::Reminder && checked_in {
        return Ok(Prepared::Cancel);
    }
    if kind.requires_check_in() && !checked_in {
        return Ok(Prepared::Cancel);
    }
    // The survey's questions live on the post-event registration form, so the
    // message is only honest while that form still accepts an answer — the
    // organizer can close it, or its deadline can lapse, after the job is queued.
    if kind == NotificationKind::Survey && !event.post_event_registration_accepting(now) {
        return Ok(Prepared::Cancel);
    }
    // Nothing sent after an event offers a deposit, so it does not pay for the
    // read either — one D1 round trip per recipient on a whole-event send.
    let pre_event = kind.delivery_window() == DeliveryWindow::BeforeEventEnds;
    let deposit = match pre_event {
        true => {
            crate::db::deposit_statuses::get_deposit_status(db, &job.event_id, &job.attendee_id)
                .await?
        }
        false => None,
    };
    let online = a["participation_type"].as_str() == Some("online");
    if matches!(
        kind,
        NotificationKind::DepositConfirmed | NotificationKind::DepositRejected
    ) {
        if !event.deposit_enabled || online {
            return Ok(Prepared::Cancel);
        }
        let Some(ref d) = deposit else {
            return Ok(Prepared::Cancel);
        };
        if d.deposited_at != job.version
            || (kind == NotificationKind::DepositConfirmed && (!d.verified || d.rejected))
            || (kind == NotificationKind::DepositRejected && !d.rejected)
        {
            return Ok(Prepared::Cancel);
        }
    }
    let needs_deposit = pre_event
        && event.deposit_enabled
        && !online
        && !deposit.as_ref().is_some_and(|d| d.verified && !d.rejected);
    Ok(Prepared::Ready(content::render(&content::Render {
        job,
        kind,
        event: &event,
        email,
        name: a["name"].as_str().unwrap_or(""),
        online,
        needs_deposit,
        base,
    })))
}
