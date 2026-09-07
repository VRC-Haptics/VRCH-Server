use haptic_core::vrc::config::{InputLayer, InputType};
use crate::*;

#[test]
fn load_param() {
    let rt = make_runtime(r#"
        local user_param = param("param_name", 1.0, {min = 0.0, max = 1.0, label = "Some Custom Parameter"})
        function build_nodes()
            return { version = "v0", nodes = {}}
        end

    "#).unwrap();
    let params = get_params(&rt);
    assert_eq!(params.len(), 1);

    let p = params.first().unwrap();
    assert_eq!(p.name, "param_name");                 // the string arg, not the Lua binding
    assert_eq!(p.default, ParamValue::Int(1));
    assert_eq!(p.min, Some(0.0));
    assert_eq!(p.max, Some(1.0));
    assert_eq!(p.label.as_deref(), Some("Some Custom Parameter"));
}

#[test]
fn edit_param() {
    let rt = make_runtime(r#"
        local user_param = param("unique_name", 1.0, {min = 0.0, max = 1.0, label = "Some Custom Parameter"})
        function build_nodes()
            return { 
                version = "v0", 
                nodes = {
                    {
                        loc = {0.0, 0.0, 0.0},
                        slots = { 
                            {  "SomeAddress",
                                {   
                                    muted = false,
                                    source = "weight",
                                    layer = "additive",
                                    weight = 1.0, 
                                }
                            }
                        },
                        muted = false,
                        radius = user_param,
                        layer = "default",
                        groups = {"All"},
                        default = 0.0,
                    }
                }
            }
        end

    "#).unwrap();
    let params = get_params(&rt);
    assert_eq!(params.len(), 1);

    let p = params.first().unwrap();
    let mut mods = HashMap::new();
    mods.insert(p.name.to_owned(), ParamValue::Float(3.0));
    let nodes = build_nodes(&rt, Some(mods)).unwrap();

    let node = nodes.first().unwrap();
    assert!((node.radius - 3.0).abs() < 0.0001, "Edited value isn't: {}", node.radius);
}

#[test]
fn edit_enum_param() {
    let rt = make_runtime(r#"
        local layer_new = param("enum", "default", {opt = {"default", "greedy", "linear", "area"}})
        function build_nodes()
            return { 
                version = "v0", 
                nodes = {
                    {
                        loc = {0.0, 0.0, 0.0},
                        slots = { 
                            {  "SomeAddress",
                                {   
                                    muted = false,
                                    source = "weight",
                                    layer = "additive",
                                    weight = 1.0, 
                                }
                            }
                        },
                        muted = false,
                        radius = 1.0,
                        layer = layer_new,
                        groups = {"All"},
                        default = 0.0,
                    }
                }
            }
        end

    "#).unwrap();
    let params = get_params(&rt);
    assert_eq!(params.len(), 1);

    let p = params.first().unwrap();
    let mut mods = HashMap::new();
    mods.insert(p.name.to_owned(), ParamValue::Str("linear".to_owned()));
    let nodes = build_nodes(&rt, Some(mods)).unwrap();

    let node = nodes.first().unwrap();
    assert!(node.layer == InterpolationLayer::Linear, "Edited value isn't: {:?}", node.layer);
}

#[test]
fn load_v0() {
    let rt = make_runtime(r#"
        function build_nodes()
            return {
                version = "v0",
                nodes = {
                    {
                        loc = {0.0, 0.0, 0.0},
                        slots = { 
                            {  "SomeAddress",
                                {   
                                    muted = false,
                                    source = "weight",
                                    layer = "additive",
                                    weight = 1.0, 
                                }
                            }
                        },
                        muted = false,
                        radius = 1.0,
                        layer = "default",
                        groups = {"All"},
                        default = 0.0,
                    }
                }
            }
        end
    "#).unwrap();
    let actual = Node {
        loc: Vec3::ZERO,
        slots: vec![("SomeAddress".into(), Slot { muted: false, source: InputType::Weight, layer: InputLayer::Additive, weight: 1.0, history: Default::default()})],
        muted: false,
        radius: 1.0,
        layer: InterpolationLayer::Default,
        groups: NodeGroup::All,
        default: 0.0,
    };
    let nodes = build_nodes(&rt, None).unwrap();

    let first = nodes.first().unwrap();
    assert_eq!(*first, actual);
}