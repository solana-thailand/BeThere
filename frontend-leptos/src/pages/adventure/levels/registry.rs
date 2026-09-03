//! The built-in level registry.

use crate::pages::adventure::types::*;

use super::advanced::*;
use super::basics::*;

/// All built-in levels (for development — production loads from KV).
pub fn default_levels() -> Vec<LevelData> {
    vec![
        level_01_hello_world(),
        level_02_variables(),
        level_03_types(),
        level_04_control_flow(),
        level_05_functions(),
        level_06_ownership(),
        level_07_structs_enums(),
        level_08_pattern_matching(),
        level_09_error_handling(),
        level_10_traits(),
    ]
}
