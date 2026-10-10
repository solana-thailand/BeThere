//! Which job a cron tick runs (.plans/045 R4.12). One trigger, hourly at
//! :17: a second cron on prod was refused by the schedules API (400,
//! 2026-10-10) while staging took it, so each worker keeps the one trigger
//! it always had. The 03:xx UTC tick runs the daily jobs (cleanup and the
//! nightly reconciles) and nothing else, so its D1 work is not added to the
//! announcer's; every other hour runs the subscriber announcer.

/// The only cron in `wrangler.toml` `[triggers]`.
pub const HOURLY_CRON: &str = "17 * * * *";
/// The UTC hour whose tick runs the daily jobs (was `0 3 * * *`).
pub const DAILY_HOUR_UTC: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduledJob {
    Daily,
    Announce,
}

/// The job for a tick scheduled at `scheduled_ms` (epoch ms, UTC).
pub fn job_for(scheduled_ms: f64) -> ScheduledJob {
    let hour = (scheduled_ms / 3_600_000.0).floor().rem_euclid(24.0) as u32;
    match hour == DAILY_HOUR_UTC {
        true => ScheduledJob::Daily,
        false => ScheduledJob::Announce,
    }
}
