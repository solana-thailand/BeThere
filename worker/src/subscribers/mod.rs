//! "Email me when the next event opens" (.plans/045 R4.12): who asked
//! (`db`), what they get (`copy`, pure), and the hourly job that mails each
//! newly opened public event to them (`announce`).

pub mod announce;
pub mod copy;
pub mod db;
