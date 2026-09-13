//! Serde `default = "..."` helpers shared across the event submodules.

/// Helper for serde default = true.
pub(super) fn default_true() -> bool {
    true
}
