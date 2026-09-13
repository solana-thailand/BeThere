//! Quiz editor component for the admin dashboard (Issue 003).
//!
//! Provides a visual interface for organizers to:
//! - Create and edit quiz questions with multiple-choice options
//! - Set correct answers and explanations
//! - Configure passing score, max attempts, optional timer
//! - Preview quiz as attendees see it
//! - Save to backend via API

mod editor;
mod helpers;
mod preview;

pub use editor::QuizEditor;
