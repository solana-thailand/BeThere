//! Staff scanner page — fullscreen camera QR scanning with slide-up bottom sheet.
//!
//! The camera fills the entire screen. A bottom sheet slides up with session info
//! and a manual entry toggle. Scan results appear as glass panel overlays on top
//! of the camera view.
//!
//! The video element is always present in the DOM (never conditionally rendered)
//! to avoid race conditions between the reactive Effect and DOM mounting.
//!
//! Requires being wrapped in `<ProtectedRoute>` to provide
//! `ReadSignal<String>` (user email) via context.

mod event_bar;
mod interop;
mod logic;
mod page;
mod state;
mod views;
mod walk_in;

pub use page::Scanner;
