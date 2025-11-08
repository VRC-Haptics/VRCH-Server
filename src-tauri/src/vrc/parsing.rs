use super::OscPath;

use regex::Regex;
use rosc::OscType;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::vec;

static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"VF\d+_").unwrap());

/// Removes the VRC Fury naming from the parameters
pub fn remove_version(path: &str) -> String {
    // Replace all matches with the captured slash "$1" to avoid producing a double slash.
    REGEX.replace_all(path, "$1").to_string()
}

/// convenience function for parsing returned HTTP OSCQuery messages
pub fn parse_incoming(input: &str) -> Vec<OscInfo> {
    let recursive_nodes: OscQueryNode = serde_json::from_str(input).expect("couldn't parse json");
    return recursive_nodes.to_info();
}

// Represents the raw JSON structure from the OSCQuery server.
#[derive(Debug, serde::Deserialize)]
pub struct OscQueryNode {
    // REQUIRED: every node must have a FULL_PATH.
    #[serde(rename = "FULL_PATH")]
    full_path: String,

    // OPTIONAL: ACCESS (if missing, we assume full read/write if a VALUE is supported)
    #[serde(rename = "ACCESS", default)]
    access: Option<u8>,

    // OPTIONAL: human-readable description.
    #[serde(rename = "DESCRIPTION", default)]
    description: Option<String>,

    // OPTIONAL: the OSC type tag string (present if this node is an OSC method).
    #[serde(rename = "TYPE", default)]
    osc_type: Option<String>,

    // OPTIONAL: the value(s) associated with an OSC method.
    #[serde(rename = "VALUE", default)]
    value: Option<Vec<serde_json::Value>>,

    // REQUIRED for containers: the sub-node hierarchy.
    // If omitted, the node should be considered an OSC method.
    #[serde(rename = "CONTENTS", default)]
    contents: Option<HashMap<String, OscQueryNode>>,
}

impl OscQueryNode {
    /// Turns a tree of QueryNodes into a list of OscInfo
    pub fn to_info(&self) -> Vec<OscInfo> {
        let mut fill: Vec<OscInfo> = vec![];

        // if has children nodes
        if let Some(children) = &self.contents {
            for (_key, node) in children {
                node.recurse(&mut fill);
            }
        }
        fill.push(OscInfo::from_node(self));

        return fill;
    }

    /// DO NOT USE: to_info is teh correct api
    pub fn recurse(&self, fill: &mut Vec<OscInfo>) {
        if let Some(children) = &self.contents {
            for (_key, node) in children {
                node.recurse(fill);
            }
        }
        fill.push(OscInfo::from_node(self));
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub enum OscAccessLevel {
    Refused,   // 0 – no value associated
    OnlyRead,  // 1 – value may only be retrieved
    OnlyWrite, // 2 – value may only be set
    Full,      // 3 – value may be both retrieved and set
}

impl OscAccessLevel {
    /// if this access level represents a readable state
    fn is_readable(&self) -> bool {
        match *self {
            OscAccessLevel::Full | OscAccessLevel::OnlyRead => true,
            _ => false,
        }
    }
}

// Conversion from the optional u8 (from the JSON) into our enum.
// If ACCESS is missing (None) then per protocol we assume that if VALUE is supported, it’s readable,
impl From<u8> for OscAccessLevel {
    fn from(opt: u8) -> Self {
        match opt {
            0 => OscAccessLevel::Refused,
            1 => OscAccessLevel::OnlyRead,
            2 => OscAccessLevel::OnlyWrite,
            3 => OscAccessLevel::Full,
            _ => OscAccessLevel::Refused,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OscInfo {
    pub full_path: OscPath,
    pub access: OscAccessLevel,
    pub value: Vec<OscType>,
    pub description: Option<String>,
}

impl OscInfo {
    /// Creates a singular parameter (path + contents)
    pub fn from_node(node: &OscQueryNode) -> OscInfo {
        let mut access = OscAccessLevel::Full;
        if let Some(acc) = node.access {
            access = OscAccessLevel::from(acc);
        }

        // create OSCType
        let mut types: Vec<OscType> = vec![];
        if let Some(type_tags) = &node.osc_type {
            if let Some(contents) = &node.value {
                let values_to_parse = if contents.len() == 1 && contents[0].is_array() {
                    // Unwrap single nested array
                    contents[0].as_array().unwrap_or(contents)
                } else {
                    contents
                };

                // create array
                for (tag, value) in type_tags.chars().zip(values_to_parse) {
                    if !access.is_readable() && *value == Value::Null {
                        types.push(OscType::Nil);
                        continue;
                    }

                    match match_tag(tag, value) {
                        Ok(content) => types.push(content),
                        Err((fallback, msg)) => {
                            log::error!("Error parsing {}: {}", node.full_path, msg);
                            log::error!("Full Node: {node:?}");
                            types.push(fallback);
                        }
                    }
                }
            }
        }

        OscInfo {
            full_path: OscPath(node.full_path.clone()),
            access: access,
            value: types,
            description: node.description.clone(),
        }
    }
}

fn match_tag(tag: char, content: &Value) -> Result<OscType, (OscType, String)> {
    // Handle Null values first for all types
    if content.is_null() {
        return Ok(OscType::Nil);
    }

    match tag {
        's' | 'S' => {
            if let Some(s) = content.as_str() {
                Ok(OscType::String(s.to_string()))
            } else if let Some(obj) = content.as_object() {
                Ok(handle_obj(obj))
            } else {
                Err((
                    OscType::Nil,
                    format!("Couldn't coerce string: {:?}", content),
                ))
            }
        }
        'i' => {
            if let Some(num) = content.as_i64() {
                Ok(OscType::Int(num as i32))
            } else if let Some(obj) = content.as_object() {
                Ok(handle_obj(obj))
            } else {
                Err((
                    OscType::Nil,
                    format!("Couldn't coerce integer: {:?}", content),
                ))
            }
        }
        'f' => {
            if let Some(num) = content.as_f64() {
                Ok(OscType::Float(num as f32))
            } else if let Some(s) = content.as_str() {
                // Handle special float string values
                match s {
                    "NaN" => Ok(OscType::Float(f32::NAN)),
                    "Infinity" | "Inf" => Ok(OscType::Float(f32::INFINITY)),
                    "-Infinity" | "-Inf" => Ok(OscType::Float(f32::NEG_INFINITY)),
                    _ => Err((
                        OscType::Nil,
                        format!("Couldn't coerce float: {:?}", content),
                    )),
                }
            } else if let Some(obj) = content.as_object() {
                Ok(handle_obj(obj))
            } else {
                Err((
                    OscType::Nil,
                    format!("Couldn't coerce float: {:?}", content),
                ))
            }
        }
        'T' => Ok(OscType::Bool(true)),
        'F' => Ok(OscType::Bool(false)),
        'I' => Ok(OscType::Inf),
        'N' => Ok(OscType::Nil),
        't' => Err((OscType::Nil, format!("time tag types are unsupported"))),
        tag => Err((
            OscType::Nil,
            format!(
                "Unsupported OSC Type tag: {}, With Contents: {:?}",
                tag, content
            ),
        )),
    }
}

fn handle_obj(obj: &serde_json::Map<String, Value>) -> OscType {
    if obj.is_empty() {
        // if empty object we can safely skip it
        OscType::Nil
    } else {
        log::error!("Found object that was not empty");
        OscType::Nil
    }
}

use std::cmp::PartialEq;
impl PartialEq for OscInfo {
    fn eq(&self, other: &Self) -> bool {
        self.full_path == other.full_path
            && self.access == other.access
            && self.value == other.value
            && self.description == other.description
    }
}
