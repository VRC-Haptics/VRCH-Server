//! Data reference schemas.
//!
//! A schema describes the packed bit code that an avatar draws into its camera.
//! The file is either plain JSON or a markdown file with a fenced JSON block.
//! Both forms hold the same object:
//!
//! ```json
//! {
//!   "id": "Testing",
//!   "version": 1,
//!   "shader": "Custom/DataReference",
//!   "totalBits": 97,
//!   "fields": [
//!     { "name": "/avatar/parameters/haptics/nodes/h15k", "type": "float", "bits": 32 }
//!   ]
//! }
//! ```
//!
//! The field order gives the bit offsets, the same way the native ABI does. A
//! name is an OSC path without the VRC Fury version prefix.

use std::fmt;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// The value type of one field. The codes match the native ABI.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CamType {
    Float,
    Int,
    Uint,
    Bool,
}

/// One field of the packed code.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CamField {
    /// The OSC parameter that this field drives.
    pub osc: String,
    #[serde(rename = "type")]
    pub kind: CamType,
    /// The width of the field, 1 to 32 bits.
    pub bits: u32,
}

/// The full description of one camera code.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CamConfig {
    /// The avatar id that this config relates to
    pub id: String,
    /// The schema version. The loader takes the highest version of an ID.
    pub version: u32,
    /// The shader that draws the code.
    pub shader: String,
    /// The total width of the code. 0 means the sum of the field widths.
    #[serde(default)]
    pub total_bits: u32,
    pub fields: Vec<CamField>,
}

impl CamConfig {
    /// The sum of the field widths.
    pub fn bit_sum(&self) -> u32 {
        self.fields.iter().map(|field| field.bits).sum()
    }

    /// The width that the decoder must use.
    pub fn bit_total(&self) -> u32 {
        if self.total_bits == 0 {
            self.bit_sum()
        } else {
            self.total_bits
        }
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.id.is_empty() {
            return Err(SchemaError::EmptyId);
        }
        if self.fields.is_empty() {
            return Err(SchemaError::NoFields);
        }
        for field in &self.fields {
            if field.bits < 1 || field.bits > 32 {
                return Err(SchemaError::FieldWidth {
                    name: field.osc.clone(),
                    bits: field.bits,
                });
            }
            if field.kind == CamType::Bool && field.bits != 1 {
                return Err(SchemaError::FieldWidth {
                    name: field.osc.clone(),
                    bits: field.bits,
                });
            }
        }
        let sum = self.bit_sum();
        if self.total_bits != 0 && self.total_bits != sum {
            return Err(SchemaError::BitTotal {
                stated: self.total_bits,
                sum,
            });
        }
        Ok(())
    }
}

/// The reason that a schema file is not usable.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaError {
    EmptyId,
    NoFields,
    /// A field is below 1 bit or above 32 bits, or a bool is wider than 1 bit.
    FieldWidth { name: String, bits: u32 },
    /// `totalBits` does not match the sum of the field widths.
    BitTotal { stated: u32, sum: u32 },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "the schema has an empty id"),
            Self::NoFields => write!(f, "the schema has no fields"),
            Self::FieldWidth { name, bits } => {
                write!(f, "field {} has an illegal width of {} bits", name, bits)
            }
            Self::BitTotal { stated, sum } => {
                write!(f, "totalBits is {} but the fields sum to {}", stated, sum)
            }
        }
    }
}

/// One schema file on disk.
///
/// The identity of an entry is the ID and the version. Two files that claim the
/// same pair collide in the index, and the second one drops out.
#[derive(Debug, Clone, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalAvailableSchema {
    pub id: String,
    pub version: u32,
    pub path: PathBuf,
}

impl PartialEq for LocalAvailableSchema {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.version == other.version
    }
}

impl std::hash::Hash for LocalAvailableSchema {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.version.hash(state);
    }
}

/// Reads a schema file from disk.
pub fn read_schema(path: &Path) -> Result<CamConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    parse_schema(&content)
}

/// Parses a schema out of a plain JSON file or a markdown file.
pub fn parse_schema(content: &str) -> Result<CamConfig, String> {
    serde_json::from_str::<CamConfig>(json_body(content)).map_err(|err| err.to_string())
}

/// Returns the JSON object inside a file.
///
/// The function takes the first fenced `json` block. A file without a fence
/// falls back to the text between the first brace and the last brace.
pub fn json_body(content: &str) -> &str {
    if content.trim_start().starts_with('{') {
        return content;
    }
    if let Some(body) = fenced_json(content) {
        return body;
    }
    match (content.find('{'), content.rfind('}')) {
        (Some(start), Some(end)) if end > start => &content[start..=end],
        _ => content,
    }
}

/// Finds the first fenced `json` block of a markdown file.
fn fenced_json(content: &str) -> Option<&str> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;

    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        match start {
            None => {
                if let Some(tag) = trimmed.strip_prefix("```") {
                    if tag.trim().eq_ignore_ascii_case("json") {
                        start = Some(offset + line.len());
                    }
                }
            }
            Some(begin) => {
                if trimmed.starts_with("```") {
                    return Some(&content[begin..offset]);
                }
            }
        }
        offset += line.len();
    }

    start.map(|begin| &content[begin..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKDOWN: &str = "# Camera\n\n```json\n{\n  \"id\": \"Testing\",\n  \"version\": 1,\n  \"shader\": \"Custom/DataReference\",\n  \"totalBits\": 97,\n  \"fields\": [\n    { \"name\": \"_D00\", \"type\": \"float\", \"bits\": 32 },\n    { \"name\": \"_D01\", \"type\": \"int\",   \"bits\": 32 },\n    { \"name\": \"_D02\", \"type\": \"uint\",  \"bits\": 32 },\n    { \"name\": \"_D03\", \"type\": \"bool\",  \"bits\": 1 }\n  ]\n}\n```\n";

    #[test]
    fn reads_a_fenced_schema() {
        let schema = parse_schema(MARKDOWN).expect("parse");
        assert_eq!(schema.id, "Testing");
        assert_eq!(schema.version, 1);
        assert_eq!(schema.fields.len(), 4);
        assert_eq!(schema.bit_sum(), 97);
        schema.validate().expect("valid");
    }

    #[test]
    fn reads_a_nested_fence() {
        let nested = format!("```markdown\n{}```\n", MARKDOWN);
        let schema = parse_schema(&nested).expect("parse");
        assert_eq!(schema.id, "Testing");
    }

    #[test]
    fn reads_plain_json() {
        let plain = json_body(MARKDOWN).to_string();
        let schema = parse_schema(&plain).expect("parse");
        assert_eq!(schema.fields[3].kind, CamType::Bool);
    }

    #[test]
    fn rejects_a_bad_total() {
        let mut schema = parse_schema(MARKDOWN).expect("parse");
        schema.total_bits = 96;
        assert!(matches!(
            schema.validate(),
            Err(SchemaError::BitTotal { stated: 96, sum: 97 })
        ));
    }
}
