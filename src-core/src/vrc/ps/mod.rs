use std::{fmt, str::FromStr};
use anyhow::Result;
use thiserror::Error;
use crate::{mapping::input_node::InputNode, state::VrcSettings};

pub enum OGBType {
    Orf,
    Pen,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct ParseOGBContactsError(pub String);

impl fmt::Display for ParseOGBContactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown OGB contact tag: {}", self.0)
    }
}

pub enum OGBContacts {
    TouchSelf,
    TouchSelfClose,
    TouchOthers,
    TouchOthersClose,
    PenSelf,
    PenSelfNewRoot,
    PenSelfNewTip,
    PenOthers,
    PenOthersClose,
    PenOthersNewRoot,
    PenOthersNewTip,
    FrotOthers,
}

impl FromStr for OGBContacts {
    type Err = ParseOGBContactsError;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        Ok(match val {
            "TouchSelf" => Self::TouchSelf,
            "TouchSelfClose" => Self::TouchSelfClose,
            "TouchOthers" => Self::TouchOthers,
            "TouchOthersClose" => Self::TouchOthersClose,
            "PenSelf" => Self::PenSelf,
            "PenSelfNewRoot" => Self::PenSelfNewRoot,
            "PenSelfNewTip" => Self::PenSelfNewTip,
            "PenOthers" => Self::PenOthers,
            "PenOthersClose" => Self::PenOthersClose,
            "PenOthersNewRoot" => Self::PenOthersNewRoot,
            "PenOthersNewTip" => Self::PenOthersNewTip,
            "FrotOthers" => Self::FrotOthers,
            other => return Err(ParseOGBContactsError(other.to_string())),
        })
    }
}

/// Takes in a string following the form:
/// 
/// `/avatar/parameters/OGB/<Type>/<Id>/<Contact>`
/// Returns a list of nodes. 
/// 
/// each node has a list of (addr, slots), these will be handled so that each parameter drives this value when a message arrives.
// pub fn create_ogb_nodes(cfg: &VrcSettings, over: &AviOverride, params: Vec<String>) -> Result<Vec<(Vec<(String, u8)>, Box<InputNode>)>, > {
//     let new = param.replace("/avatar/parameters/OGB/", "");
//     let trimmed: Vec<_> = new.split(|v| v == '/').collect();
//     let ogb = trimmed.first().with_context(|| "OGB Parameter: {param}; Doesn't have first")?;
//     let id = trimmed.get(1).with_context(|| "OGB Parameter: {param}; doesn't have id")?;
//     let spec = trimmed.get(2).with_context(|| "OGB Parameter: {param}; doesn't have type")?;

//     let _type = if *ogb == "Orf" {
//         OGBType::Orf
//     } else if *ogb == "Pen" {
//         OGBType::Pen
//     } else {
//         return Err(anyhow!("Unable to build type for: {ogb}"));
//     };

//     let contact = spec.parse::<OGBContacts>()?;

//     Ok(match over.ps.get(*id) {
//         Some(new) => {
//             let mut nodes = vec![];
//             for entry in new.outputs {
//                 let mut slots = SmallVec::default();

//                 let sl = Slot {
//                     muted: false,
//                     source: super::config::InputType::Weight,
//                     layer: I
//                 }

//                 let node = InputNode::new(entry.0, NodeGroup::All, entry.2, slots, entry.1)
//             }
//             nodes
//         }
//         None => {

//         }
//     })
// }

/// Takes in a string following the form:
/// 
/// `/avatar/parameters/VFH/<Type>/<Id>/<Contact>`
pub fn create_vfh_nodes(_cfg: &VrcSettings, _param: String) -> Vec<(Vec<(String, u8)>, Box<InputNode>)> {
    vec![]
}
