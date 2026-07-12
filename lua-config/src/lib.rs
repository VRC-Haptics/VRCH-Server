mod v0;
mod ffi;
mod params;

#[cfg(test)]
mod testing;

use std::{collections::HashMap, fmt::Display};

use serde::de::DeserializeOwned;
use v0::{NodeV0};
use haptic_core::{glam::{Vec3, Vec4}, mapping::{Nodes, groups::NodeGroup, input_node::*}};
use mlua::prelude::*;

use crate::{params::{ParamDef, ParamValue, discover_params, install_params}, v0::LocationV0};

type Address = String;
struct ScriptSource(String);

/// create a new lua runtime containing the file.
pub fn make_runtime(file: &str) -> LuaResult<Lua> {
    let rt = Lua::new();
    rt.set_app_data(ScriptSource(file.to_owned()));
    install_params(&rt, HashMap::new())?;
    rt.load(file).exec()?;
    Ok(rt)
}

/// returns available parameters for this script.
pub fn get_params(rt: &Lua) -> Vec<ParamDef> {
    discover_params(rt)
}

/// run lua script to extract the list of nodes.
pub fn build_nodes(rt: &Lua, params: Option<HashMap<String, ParamValue>>) -> LuaResult<Vec<Node>> {
    // reinstall params
    if let Some(overrides) = params {
        let src = rt
            .app_data_ref::<ScriptSource>()
            .ok_or_else(|| LuaError::runtime("runtime not created via make_runtime"))?
            .0
            .clone();
        install_params(rt, overrides)?; // resets registry + overrides
        rt.load(&src).exec()?;
    };

    let this = run_function::<NodeList>("build_nodes", &rt)?;
    return Ok(this.0);
}

pub fn build_locations(rt: Lua) -> LuaResult<(Vec<Location>, Vec<TagDef>)> {
    let this = run_function::<LocationList>("build_locations", &rt)?;
    Ok((this.0, this.1))
}

#[derive(serde::Deserialize)]
pub struct NodeList(Vec<Node>);

impl Versioned for NodeList {
    fn from_versioned(version: &str, rt: &mlua::Lua, tbl: &mlua::Table) -> mlua::Result<Self> {
        let Some(nodes) = tbl.get::<Option<LuaValue>>("nodes")? else {
            return Err(LuaError::external(FunctionError::VersionParse("No nodes object".to_owned())));
        };
        
        match version {
            "v0" => {
                let parsed: Vec<NodeV0> = rt.from_value(nodes)?;
                Ok(Self(parsed.into_iter().map(Node::from).collect()))
            }
            other => Err(LuaError::runtime(format!("unsupported node version: {other:?}"))),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct LocationList(Vec<Location>, Vec<TagDef>);

impl Versioned for LocationList {
    fn from_versioned(version: &str, rt: &Lua, tbl: &LuaTable) -> LuaResult<Self> {
        let Some(locs) = tbl.get::<Option<LuaValue>>("locations")? else {
            return Err(LuaError::external(FunctionError::VersionParse("No locations object".to_owned())));
        };
        let Some(tags) = tbl.get::<Option<LuaValue>>("tags")? else {
            return Err(LuaError::external(FunctionError::VersionParse("No locations object".to_owned())));
        };

        match version {
            "v0" => {
                let locs: Vec<LocationV0> = rt.from_value(locs)?;
                let tags: Vec<TagDef> = rt.from_value(tags)?;
                Ok(Self(locs.into_iter().map(Location::from).collect(), tags.into_iter().map(TagDef::from).collect()))
            }
            other => Err(LuaError::runtime(format!("unsupported node version: {other:?}"))),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct Node {
    pub loc: Vec3,
    /// list of address, slot pairs.
    pub slots: Vec<(Address, Slot)>,
    pub muted: bool,
    pub radius: f32,
    pub layer: InterpolationLayer,
    pub groups: NodeGroup,
    pub default: f32,
}

impl From<NodeV0> for Node {
    fn from(file: NodeV0) -> Self {
        Self { loc: file.loc, slots: file.slots, muted: file.muted, radius: file.radius, layer: file.layer, groups: file.groups, default: file.default }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
/// An in-game location, loosely based on a sever Node.
pub struct Location {
    pub loc: Vec3,
    pub rot: Vec4,
    pub shapes: Vec<(Address, Shape)>,
    pub tags: Vec<String>
}

impl From<LocationV0> for Location {
    fn from(value: LocationV0) -> Self {
        Self {
            loc: value.loc,
            rot: value.rot,
            shapes: value.shapes,
            tags: value.tags,

        }
    }
}

impl Location {
    pub fn from_nodes(nodes: Vec<Node>) -> Vec<Location> {
        // shape based on _Weight post-fix
        unimplemented!()
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct TagDef {
    pub name: String, 
    pub effect: Effect,
    pub menu: Option<MenuItem>,
} 

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct MenuItem {
    pub menu_type: MenuType,
    pub param: String,
    pub default: f32,
    pub path: String,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub enum MenuType {
    Slider,
    Boolean,
    Puppet,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
#[serde(tag = "effect")]
pub enum Effect {
    InGameVelocity,
    EnableToggle,
    // in percentage of original radius, and length
    CustomSize{start: f32, end: f32, include_len: bool},
    // base position + start and base position + end (global means global coordinates)
    CustomOffset{start: Vec3, end: Vec3, global: bool},
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
#[serde(tag = "kind")]
pub enum Shape {
    Sphere{ radius: f32 },
    Capsule{ radius: f32, len: f32, rot: Vec4, offset: Vec3 },
    Ray{ len: f32, rot: Vec4, offset: Vec3 },
}

#[derive(Debug)]
pub enum FunctionError {
    MissingVersionKey,
    MissingVersion(String),
    MissingData,
    MissingFunction,
    VersionParse(String)
}

impl Display for FunctionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LuaFunctionError: {:?}", self)
    }
}

impl std::error::Error for FunctionError {}

/// A canonical output type that knows how to parse its own versioned payloads.
/// MUST Return a Err(LuaError::external(FunctionError::VersionParse("custom_error"))) for error cases.
pub trait Versioned: Sized {
    fn from_versioned(version: &str, rt: &Lua, tbl: &LuaTable) -> LuaResult<Self>;
}

/// ERROR: FunctionError
pub fn run_function<T>(name: &str, rt: &Lua) -> LuaResult<T> 
where
    T: Versioned + DeserializeOwned,
{
    let build: LuaFunction = match rt.globals().get::<Option<LuaFunction>>(name)? {
        Some(f) => f,
        None => {
            return Err(LuaError::external(FunctionError::MissingFunction));
        }
    };

    let value: LuaValue = build.call(())?;
    let root: &LuaTable = value.as_table().ok_or_else(|| LuaError::runtime("config script must return a table"))?;
    let Some(version) = root.get::<Option<String>>("version")? else {
        return Err(LuaError::external(FunctionError::MissingVersionKey));
    };

    T::from_versioned(&version, rt, root)
}