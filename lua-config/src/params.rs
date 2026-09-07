use mlua::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ParamDef {
    pub name: String,
    pub default: ParamValue,
    pub min: Option<f64>,
    pub max: Option<f64>,
    // options, if list of strings and other stepped values.
    pub opt: Option<Vec<ParamValue>>,
    pub label: Option<String>,
    pub desc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl FromLua for ParamValue {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        Ok(match value {
            LuaValue::Boolean(b) => ParamValue::Bool(b),
            LuaValue::Integer(i) => ParamValue::Int(i),
            LuaValue::Number(n)  => ParamValue::Float(n),
            LuaValue::String(s)  => ParamValue::Str(s.to_string_lossy()),
            other => {
                return Err(LuaError::runtime(format!(
                    "cannot convert {} to ParamValue (expected boolean, number, or string)",
                    other.type_name()
                )))
            }
        })
    }
}

impl IntoLua for ParamValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        Ok(match self {
            ParamValue::Bool(b)  => LuaValue::Boolean(b),
            ParamValue::Int(i)   => LuaValue::Integer(i),
            ParamValue::Float(n) => LuaValue::Number(n),
            ParamValue::Str(s)   => LuaValue::String(lua.create_string(s)?),
        })
    }
}

#[derive(Default)]
struct ParamRegistry(Vec<ParamDef>);
struct ParamOverrides(HashMap<String, ParamValue>);

/// Installs the global `param(name, default, opts?)` function.
/// Call this on a fresh runtime BEFORE `load(...).exec()`.
/// `overrides` is empty on the discovery pass, populated on the run pass.
pub fn install_params(rt: &Lua, overrides: HashMap<String, ParamValue>) -> LuaResult<()> {
    rt.set_app_data(ParamRegistry::default());
    rt.set_app_data(ParamOverrides(overrides));

    let param = rt.create_function(
        |lua, (name, default, opts): (String, ParamValue, Option<LuaTable>)| {
            let (min, max, opt, label, desc) = match &opts {
                Some(t) => (
                    t.get::<Option<f64>>("min")?,
                    t.get::<Option<f64>>("max")?,
                    t.get::<Option<Vec<ParamValue>>>("opt")?,
                    t.get::<Option<String>>("label")?,
                    t.get::<Option<String>>("desc")?,
                ),
                None => (None, None, None, None, None),
            };

            // Record the declaration in call order.
            {
                let mut reg = lua
                    .app_data_mut::<ParamRegistry>()
                    .ok_or_else(|| LuaError::runtime("param registry not installed"))?;
                reg.0.push(ParamDef { name: name.clone(), default: default.clone(), min, max, opt, label, desc });
            }

            // Resolve host override, else the declared default. Script binds the result.
            let resolved: ParamValue = match lua.app_data_ref::<ParamOverrides>() {
                Some(over) => over.0.get(&name).cloned().unwrap_or(default),
                None => default,
            };

            Ok(resolved)
        },
    )?;

    rt.globals().set("param", param)?;
    Ok(())
}

/// Reads back every param the script declared during exec (declaration order).
pub fn discover_params(rt: &Lua) -> Vec<ParamDef> {
    rt.app_data_ref::<ParamRegistry>()
        .map(|r| r.0.clone())
        .unwrap_or_default()
}