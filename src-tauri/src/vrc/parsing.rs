use std::vec;
use std::collections::HashMap;

use rosc::OscType;

/// convenience function for parsing returned HTTP OSCQuery messages
pub fn parse_incoming(input: &str) -> Vec<OscInfo> {
    let recursive_nodes: OscQueryNode = serde_json::from_str(input)
    .expect("couldn't parse json");
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
        let mut fill:Vec<OscInfo>  = vec![];

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

#[derive(Debug, Clone, PartialEq)]
pub enum OscAccessLevel {
    Refused,    // 0 – no value associated
    OnlyRead,   // 1 – value may only be retrieved
    OnlyWrite,  // 2 – value may only be set
    Full,       // 3 – value may be both retrieved and set
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

#[derive(Debug, Clone)]
pub struct OscInfo {
    pub full_path: String,
    pub access: OscAccessLevel,
    pub value: Option<Vec<OscType>>,
    pub description: Option<String>,
}

impl OscInfo {
    pub fn from_node(node: &OscQueryNode) -> OscInfo {
        let mut access = OscAccessLevel::Full;
        if let Some(acc) = node.access {
            access = OscAccessLevel::from(acc);
        }

        // If both tags and contents exist
        let mut types: Option<Vec<OscType>> = Option::None;
        if let Some(type_tags) = &node.osc_type {
            if let Some(contents) = &node.value {
                let mut things = vec![];
                // Iterate over each tag for the types
                for (i, tag) in type_tags.chars().enumerate() {
                    let finished = match tag {
                        's' => {OscType::String(
                                contents[i].as_str()
                                    .expect("Couldn't coerce string type")
                                    .to_string()
                            )
                        },
                        'i' => {OscType::Int(
                            contents[i].as_i64()
                                .expect("Couldn't coerce integer type") 
                                as i32
                            )
                        },
                        'f' => {OscType::Float(
                            contents[i].as_f64()
                                .expect("Couldn't coerce integer type") 
                                as f32
                            )
                        },
                        tag => {
                            log::error!("Unsupported OSC Type tag: {}", tag);
                            OscType::Nil
                        }
                    };

                    things.push(finished);
                }
                things.reverse();
                types = Some(things);
            }
        }

        OscInfo { 
            full_path: node.full_path.clone(), 
            access: access, 
            value: types, 
            description: node.description.clone(),
        }
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
