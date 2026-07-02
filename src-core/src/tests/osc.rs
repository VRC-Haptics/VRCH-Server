use glam::Vec3;

use crate::{osc::parse::{MsgType, RefMessage}, *}; 

fn bytes_of(v: &[f32]) -> Vec<u8> { v.iter().flat_map(|f| f.to_be_bytes()).collect() }
 
#[test] fn f32_roundtrip() {
    assert_eq!(MsgType::F32.try_f32(&1.5f32.to_be_bytes(), true).unwrap(), 1.5);
}
#[test] fn bool_consts_as_f32() {
    assert_eq!(MsgType::BoolTrue.try_f32(&[], true).unwrap(), 1.0);
    assert_eq!(MsgType::BoolFalse.try_f32(&[], true).unwrap(), 0.0);
}
#[test] fn i32_scaled() {
    assert_eq!(MsgType::I32.try_f32(&255i32.to_be_bytes(), true).unwrap(), 1.0);
    assert_eq!(MsgType::I32.try_f32(&0i32.to_be_bytes(), true).unwrap(), 0.0);
    let mid = MsgType::I32.try_f32(&128i32.to_be_bytes(), true).unwrap();
    assert!((mid - 128.0/255.0).abs() < 1e-6);
}
#[test] fn i32_unscaled_raw() {
    assert_eq!(MsgType::I32.try_f32(&7i32.to_be_bytes(), false).unwrap(), 7.0);
}
#[test] fn f32_short_buffer_errors() {
    assert!(MsgType::F32.try_f32(&[0,0,0], true).is_err());
}
#[test] fn vec3_ok() {
    let b = bytes_of(&[1.0, 2.0, 3.0]);
    assert_eq!(MsgType::ArrayF32X3.try_vec3(&b, true).unwrap(), Vec3::new(1.0,2.0,3.0));
}
#[test] fn vec3_x4_reads_first_three() {
    let b = bytes_of(&[1.0, 2.0, 3.0, 4.0]);
    assert_eq!(MsgType::ArrayF32X4.try_vec3(&b, true).unwrap(), Vec3::new(1.0,2.0,3.0));
}
#[test] fn vec3_rejects_scalar() {
    assert!(MsgType::F32.try_vec3(&bytes_of(&[1.0]), true).is_err());
}
#[test] fn vec3_short_buffer_errors() {
    assert!(MsgType::ArrayF32X3.try_vec3(&bytes_of(&[1.0, 2.0]), true).is_err());
}
#[test] fn bool_from_f32() {
    assert_eq!(MsgType::F32.parse_bool(&0.7f32.to_be_bytes(), true), Some(true));
    assert_eq!(MsgType::F32.parse_bool(&0.2f32.to_be_bytes(), true), Some(false));
}
#[test] fn bool_from_i32() {
    assert_eq!(MsgType::I32.parse_bool(&200i32.to_be_bytes(), true), Some(true));
    assert_eq!(MsgType::I32.parse_bool(&100i32.to_be_bytes(), true), Some(false));
    assert_eq!(MsgType::I32.parse_bool(&1i32.to_be_bytes(), false), Some(true));
}
#[test] fn bool_from_string() {
    assert_eq!(MsgType::String.parse_bool(b"true\0\0\0\0", true), Some(true));
    assert_eq!(MsgType::String.parse_bool(b"TRUE", true), Some(true));
    assert_eq!(MsgType::String.parse_bool(b"false", true), Some(false));
    assert_eq!(MsgType::String.parse_bool(b"nope", true), None);
}
#[test] fn via_refmessage() {
    let b = 2.5f32.to_be_bytes();
    let m = RefMessage { addr: "/x", t: MsgType::F32, contents: &b };
    assert_eq!(m.into_f32().unwrap(), 2.5);
    assert_eq!(m.into_bool(), Some(true));
}
