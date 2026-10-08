//! Event form — shared by Create and Edit modes on the Events page.
//!
//! `types` holds `EventForm` and its helpers, `component` the
//! `<EventFormComponent>` that composes one component per form section, and
//! `save` the validate + create/update (+ escrow init) handler.

mod actions;
mod basic;
mod community;
mod component;
mod ctx;
mod deposit;
mod nft;
mod og_card;
mod people;
mod poster;
mod save;
mod section;
mod settings;
mod sheets;
mod sponsors;
mod types;

pub use component::EventFormComponent;
pub use types::{
    EventForm, OnDone, default_form, form_from_detail, format_date_display, status_badge_class,
    status_label,
};
