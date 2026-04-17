use rosc::OscType;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SpectaOscType {
    Int(i32),
    Float(f32),
    String(String),
    Long(i64),
    Double(f64),
    Char(char),
    Bool(bool),
    Nil,
    Inf,
    Blob(Vec<u8>),
    // Flatten complex inner types to something serializable
    Time { seconds: u32, fractional: u32 },
    Color { r: u8, g: u8, b: u8, a: u8 },
    Midi { port: u8, status: u8, data1: u8, data2: u8 },
    Array(Vec<SpectaOscType>),
}

impl SpectaOscType {
    pub fn float(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn int(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Convert to the underlying rosc type for full API access
    pub fn as_osc(&self) -> OscType {
        self.clone().into()
    }

    pub fn double(&self) -> Option<f64> {
        match self {
            Self::Double(v) => Some(*v),
            _ => None,
        }
    }

    pub fn long(&self) -> Option<i64> {
        match self {
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<OscType> for SpectaOscType {
    fn from(osc: OscType) -> Self {
        match osc {
            OscType::Int(v) => Self::Int(v),
            OscType::Float(v) => Self::Float(v),
            OscType::String(v) => Self::String(v),
            OscType::Long(v) => Self::Long(v),
            OscType::Double(v) => Self::Double(v),
            OscType::Char(v) => Self::Char(v),
            OscType::Bool(v) => Self::Bool(v),
            OscType::Nil => Self::Nil,
            OscType::Inf => Self::Inf,
            OscType::Blob(v) => Self::Blob(v),
            OscType::Time(t) => Self::Time {
                seconds: t.seconds,
                fractional: t.fractional,
            },
            OscType::Color(c) => Self::Color {
                r: c.red, g: c.green, b: c.blue, a: c.alpha,
            },
            OscType::Midi(m) => Self::Midi {
                port: m.port, status: m.status, data1: m.data1, data2: m.data2,
            },
            OscType::Array(a) => Self::Array(
                a.content.into_iter().map(SpectaOscType::from).collect(),
            ),
        }
    }
}

impl From<SpectaOscType> for OscType {
    fn from(s: SpectaOscType) -> Self {
        match s {
            SpectaOscType::Int(v) => Self::Int(v),
            SpectaOscType::Float(v) => Self::Float(v),
            SpectaOscType::String(v) => Self::String(v),
            SpectaOscType::Long(v) => Self::Long(v),
            SpectaOscType::Double(v) => Self::Double(v),
            SpectaOscType::Char(v) => Self::Char(v),
            SpectaOscType::Bool(v) => Self::Bool(v),
            SpectaOscType::Nil => Self::Nil,
            SpectaOscType::Inf => Self::Inf,
            SpectaOscType::Blob(v) => Self::Blob(v),
            SpectaOscType::Time { seconds, fractional } => {
                Self::Time(rosc::OscTime { seconds, fractional })
            }
            SpectaOscType::Color { r, g, b, a } => {
                Self::Color(rosc::OscColor { red: r, green: g, blue: b, alpha: a })
            }
            SpectaOscType::Midi { port, status, data1, data2 } => {
                Self::Midi(rosc::OscMidiMessage { port, status, data1, data2 })
            }
            SpectaOscType::Array(v) => {
                Self::Array(rosc::OscArray {
                    content: v.into_iter().map(OscType::from).collect(),
                })
            }
        }
    }
}