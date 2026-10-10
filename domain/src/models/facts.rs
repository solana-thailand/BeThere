//! Hand-recorded facts the site shows next to the live stats (.plans/045
//! R4.7): the commitment ladder (RTM #1–#6 payers and who came) and RTM #1's
//! room. One typed set, read by the frontend and by any worker page, so a
//! number has one home and one definition.
//!
//! These are historical records, not live figures, and several were never in
//! D1 (RTM #1 predates BeThere; RTM #3's deposits were purged before the
//! archive existed; one RTM #2 payer kept a standing balance outside D1). So
//! every row carries its source and the date it was measured, and totals are
//! summed from the rows, never typed in. Definition (owner, 6 Oct 2026):
//! in-person registrants who paid cash or credit; comp, staff and online
//! payers are out; came = checked in. The live per-event payers
//! (`public_stats::EventPayers`) use the same definition on what the system
//! recorded, and the `System` rows here must match them.

/// Where a row's numbers come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadderSource {
    /// The organizer's sheet, before BeThere took deposits (confirmed 3 Oct).
    HandRecord,
    /// BeThere's deposit archive, plus one payer the owner records by hand.
    ArchiveAndOwnerRecord,
    /// The digest-chained deposit statement, paired 1:1 to attendees.
    Statement,
    /// BeThere's live tables.
    System,
}

/// One event's payers and how many of them came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LadderRow {
    pub event: &'static str,
    pub paid: u32,
    pub came: u32,
    pub source: LadderSource,
    /// `YYYY-MM-DD`: when this row was last checked against its source.
    pub measured_at: &'static str,
}

/// RTM #1–#6 under the owner's definition (`.plans/043`, 6 Oct 2026; RTM #6
/// added 8 Oct, `.plans/044` item 4).
pub const LADDER: [LadderRow; 6] = [
    LadderRow {
        event: "RTM #1",
        paid: 16,
        came: 16,
        source: LadderSource::HandRecord,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #2",
        paid: 15,
        came: 15,
        source: LadderSource::ArchiveAndOwnerRecord,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #3",
        paid: 12,
        came: 12,
        source: LadderSource::Statement,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #4",
        paid: 16,
        came: 14,
        source: LadderSource::System,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #5",
        paid: 14,
        came: 13,
        source: LadderSource::System,
        measured_at: "2026-10-06",
    },
    LadderRow {
        event: "RTM #6",
        paid: 21,
        came: 20,
        source: LadderSource::System,
        measured_at: "2026-10-08",
    },
];

/// `(paid, came)` summed over [`LADDER`].
pub fn ladder_total() -> (u32, u32) {
    LADDER
        .iter()
        .fold((0, 0), |(p, c), r| (p + r.paid, c + r.came))
}

/// `came` of `n` as a whole percent, rounded half up; 0 when nobody counted.
pub fn percent(came: u32, n: u32) -> u32 {
    match n {
        0 => 0,
        _ => (200 * came + n) / (2 * n),
    }
}

/// One group of RTM #1's room (hand record, confirmed 3 Oct 2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomGroup {
    pub n: u32,
    pub came: u32,
}

/// Paid a deposit; confirmed without paying; never confirmed.
pub const ROOM: [RoomGroup; 3] = [
    RoomGroup { n: 16, came: 16 },
    RoomGroup { n: 14, came: 9 },
    RoomGroup { n: 15, came: 0 },
];
