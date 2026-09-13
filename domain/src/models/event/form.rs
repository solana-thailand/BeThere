//! Registration form config (Issue #049).

use serde::{Deserialize, Serialize};

/// Per-event registration form configuration, stored in KV under `event:{id}:form:config`.
///
/// Defines which developer profile fields appear on the public registration form,
/// their labels, options, and whether they're required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationFormConfig {
    /// Human-readable section header (e.g. "About You").
    #[serde(default = "default_section_label")]
    pub section_label: String,
    /// Ordered list of form fields to render.
    #[serde(default = "default_form_fields")]
    pub fields: Vec<FormFieldConfig>,
}

fn default_section_label() -> String {
    "About You (optional \u{2014} helps us plan better events)".to_string()
}

fn default_form_fields() -> Vec<FormFieldConfig> {
    vec![
        FormFieldConfig {
            key: "experience_level".to_string(),
            label: "Experience level".to_string(),
            field_type: FormFieldType::Select,
            options: Some(vec![
                "Beginner".to_string(),
                "Intermediate".to_string(),
                "Senior".to_string(),
                "Tech Lead".to_string(),
            ]),
            required: false,
            profile_field: true,
        },
        FormFieldConfig {
            key: "tech_stack".to_string(),
            label: "Technologies you use".to_string(),
            field_type: FormFieldType::Multiselect,
            options: Some(vec![
                "Rust".to_string(),
                "TypeScript".to_string(),
                "Python".to_string(),
                "Solidity".to_string(),
                "Move".to_string(),
                "Go".to_string(),
                "C++".to_string(),
            ]),
            required: false,
            profile_field: true,
        },
        FormFieldConfig {
            key: "interests".to_string(),
            label: "Topics that interest you".to_string(),
            field_type: FormFieldType::Multiselect,
            options: Some(vec![
                "DeFi".to_string(),
                "NFT".to_string(),
                "ZK Proofs".to_string(),
                "Infrastructure".to_string(),
                "Gaming".to_string(),
                "AI/ML".to_string(),
                "Mobile".to_string(),
            ]),
            required: false,
            profile_field: true,
        },
    ]
}

impl Default for RegistrationFormConfig {
    fn default() -> Self {
        Self {
            section_label: default_section_label(),
            fields: default_form_fields(),
        }
    }
}

impl RegistrationFormConfig {
    /// Returns the default form config.
    pub fn default_config() -> Self {
        Self::default()
    }
}

/// Configuration for a single form field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFieldConfig {
    /// Unique field key matching `developer_profiles` column or custom key.
    pub key: String,
    /// Human-readable label shown above the field.
    pub label: String,
    /// Field input type.
    #[serde(rename = "type")]
    pub field_type: FormFieldType,
    /// Options for Select/Multiselect fields. Ignored for Text/Textarea.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    /// Whether the field is required for submission.
    #[serde(default)]
    pub required: bool,
    /// Whether this field should update the developer profile.
    #[serde(default)]
    pub profile_field: bool,
}

/// Supported form field types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FormFieldType {
    /// Single-line text input.
    Text,
    /// Multi-line text area.
    Textarea,
    /// Single-select dropdown.
    Select,
    /// Multi-select (checkboxes).
    Multiselect,
}
