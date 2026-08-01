//! The bound state of one camera: the ABI field table, the scale of each
//! field, and the route of each field into the input map.
//!
//! A session is immutable. The decode thread reads it. A new avatar or a new
//! schema builds a new session and swaps it in.

use std::fmt;
use std::sync::Arc;

use smallvec::SmallVec;

use crate::api::{DataRefKind, DataRefSchema};
use crate::vrc::AddrInfo;

use super::ffi::{DrdField, DRD_KIND_BOOL, DRD_KIND_FLOAT, DRD_KIND_INT, DRD_KIND_UINT};

/// The targets that one field drives. Most fields drive one slot.
pub type FieldRoute = SmallVec<[AddrInfo; 2]>;

/// The scale of one field, resolved at bind time.
///
/// The decode loop reads this instead of the schema. It holds no branch on a
/// string and no division.
#[derive(Copy, Clone, Debug)]
pub struct FieldSpec {
    kind: u32,
    /// The factor that maps the decoded number into the unit range.
    scale: f32,
}

impl FieldSpec {
    fn new(kind: DataRefKind, bits: u32) -> Self {
        let bits = bits.clamp(1, 32);
        let scale = match kind {
            // Kind 0 is a clamped float. The decoder already returns it in the
            // unit range, so it needs no scale. The OSC path passes a float
            // through in the same way.
            DataRefKind::Float => 1.0,
            DataRefKind::Bool => 1.0,
            // 8 unsigned bits map 0..255 onto 0.0..1.0.
            DataRefKind::Uint => 1.0 / ((1u64 << bits) - 1) as f32,
            // 8 signed bits map -128..127 onto -1.008..1.0.
            DataRefKind::Int => {
                if bits == 1 {
                    1.0
                } else {
                    1.0 / ((1u64 << (bits - 1)) - 1) as f32
                }
            }
        };
        Self {
            kind: kind.abi_code(),
            scale,
        }
    }

    /// Maps one decoded value onto the range that the input map expects.
    ///
    /// A float field passes through. An int, uint, or bool field scales by the
    /// bit width. The caller clamps.
    #[inline(always)]
    pub fn to_scalar(&self, raw: u32, number: f64) -> f32 {
        match self.kind {
            DRD_KIND_FLOAT => number as f32,
            DRD_KIND_BOOL => f32::from(raw != 0),
            DRD_KIND_UINT | DRD_KIND_INT => number as f32 * self.scale,
            _ => 0.0,
        }
    }
}

/// One camera, bound to one avatar.
pub struct CameraSession {
    /// The ID that the avatar reports over OSC.
    pub id: Box<str>,
    /// The schema version that this session uses.
    pub version: u32,
    /// The field table in ABI order. `drd_open` reads it.
    pub fields: Box<[DrdField]>,
    /// The scale of each field, in the same order.
    pub specs: Box<[FieldSpec]>,
    /// The route of each field, in the same order. An empty route drops the
    /// field.
    pub routes: Box<[FieldRoute]>,
    pub total_bits: u32,
    pub min_confidence: f32,
    /// The Spout sender name as UTF-16. `None` selects the active sender.
    pub sender_name: Option<Box<[u16]>>,
    /// True when the library must call back only on a bit change.
    pub only_on_change: bool,
    /// The count of fields with at least one target.
    pub mapped_fields: u32,
}

impl fmt::Debug for CameraSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CameraSession")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("fields", &self.fields.len())
            .field("mapped_fields", &self.mapped_fields)
            .field("total_bits", &self.total_bits)
            .finish()
    }
}

impl CameraSession {
    /// Builds a session from a schema and a route lookup.
    ///
    /// The lookup receives the field name of the schema. It returns the input
    /// map targets for that name. An empty return drops the field.
    pub fn build<F>(
        schema: &DataRefSchema,
        min_confidence: f32,
        only_on_change: bool,
        sender_name: Option<&str>,
        mut lookup: F,
    ) -> Result<Arc<CameraSession>, SessionError>
    where
        F: FnMut(&str) -> FieldRoute,
    {
        schema.validate().map_err(SessionError::Schema)?;

        let count = schema.fields.len();
        let mut fields = Vec::with_capacity(count);
        let mut specs = Vec::with_capacity(count);
        let mut routes = Vec::with_capacity(count);
        let mut mapped_fields = 0u32;

        for field in &schema.fields {
            fields.push(DrdField {
                bits: field.bits,
                kind: field.kind.abi_code(),
            });
            specs.push(FieldSpec::new(field.kind, field.bits));

            let route = lookup(&field.osc);
            if !route.is_empty() {
                mapped_fields += 1;
            }
            routes.push(route);
        }

        if mapped_fields == 0 {
            return Err(SessionError::NoRoutes);
        }

        Ok(Arc::new(CameraSession {
            id: schema.id.as_str().into(),
            version: schema.version,
            fields: fields.into_boxed_slice(),
            specs: specs.into_boxed_slice(),
            routes: routes.into_boxed_slice(),
            total_bits: schema.bit_total(),
            min_confidence: min_confidence.clamp(0.0, 1.0),
            sender_name: sender_name.map(|name| name.encode_utf16().collect()),
            only_on_change,
            mapped_fields,
        }))
    }

    /// The count of fields in the schema.
    #[inline]
    pub fn field_count(&self) -> u32 {
        self.fields.len() as u32
    }
}

/// The reason that a session did not build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The schema broke a rule of the ABI.
    Schema(crate::api::SchemaError),
    /// No field of the schema names a watched parameter.
    NoRoutes,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(err) => write!(f, "{}", err),
            Self::NoRoutes => write!(f, "no field of the schema drives a watched parameter"),
        }
    }
}

impl std::error::Error for SessionError {}