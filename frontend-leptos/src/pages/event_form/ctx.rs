//! Signals shared by every section of the event form.

use leptos::prelude::*;

use super::types::EventForm;
use crate::{api, components};

/// Shared form state, built once by `EventFormComponent` and handed to each
/// section. Every field is a signal (or `Copy`), so the struct is `Copy`.
#[derive(Clone, Copy)]
pub(super) struct FormCtx {
    pub form: ReadSignal<EventForm>,
    pub set_form: WriteSignal<EventForm>,
    pub set_toast: WriteSignal<Option<components::ToastMessage>>,
    pub editing_id: ReadSignal<Option<String>>,
    pub is_create: bool,
    pub events: ReadSignal<Vec<api::EventMeta>>,
    pub saving: ReadSignal<bool>,
    pub set_saving: WriteSignal<bool>,
    pub poster_busy: ReadSignal<bool>,
    pub set_poster_busy: WriteSignal<bool>,
    pub slug_taken: ReadSignal<bool>,
    pub set_slug_taken: WriteSignal<bool>,
    /// Community links, kept apart from `form` for row-level editing.
    pub cl_links: ReadSignal<Vec<api::CommunityLink>>,
    pub set_cl_links: WriteSignal<Vec<api::CommunityLink>>,
    /// Wallet state for the combined Create Event + Escrow Init flow.
    pub create_wallet_name: ReadSignal<String>,
    pub set_create_wallet_name: WriteSignal<String>,
    pub create_wallet_pk: ReadSignal<String>,
    pub set_create_wallet_pk: WriteSignal<String>,
    pub detected_wallets: ReadSignal<Vec<String>>,
}
