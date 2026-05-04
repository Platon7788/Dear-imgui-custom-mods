//! Property value types and display.

/// A typed property value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    /// RGB color `[r, g, b]` in `0.0..=1.0`.
    Color3([f32; 3]),
    /// RGBA color `[r, g, b, a]` in `0.0..=1.0`.
    Color4([f32; 4]),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    /// Enum / dropdown: (selected index, options).
    Enum(usize, Vec<String>),
    /// Bitflags: (value, flag names).
    Flags(u64, Vec<String>),
    /// Nested object (children stored as properties).
    Object,
    /// Array (children stored as indexed properties).
    Array(usize),
}

impl Default for PropertyValue {
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl PropertyValue {
    /// Display the value as a string.
    pub fn display(&self) -> String {
        match self {
            Self::Bool(v) => if *v { "true" } else { "false" }.to_string(),
            Self::I32(v) => v.to_string(),
            Self::I64(v) => v.to_string(),
            Self::F32(v) => format!("{:.3}", v),
            Self::F64(v) => format!("{:.6}", v),
            Self::String(v) => v.clone(),
            Self::Color3(c) => format!("[{:.2}, {:.2}, {:.2}]", c[0], c[1], c[2]),
            Self::Color4(c) => format!("[{:.2}, {:.2}, {:.2}, {:.2}]", c[0], c[1], c[2], c[3]),
            Self::Vec2(v) => format!("[{:.2}, {:.2}]", v[0], v[1]),
            Self::Vec3(v) => format!("[{:.2}, {:.2}, {:.2}]", v[0], v[1], v[2]),
            Self::Vec4(v) => format!("[{:.2}, {:.2}, {:.2}, {:.2}]", v[0], v[1], v[2], v[3]),
            Self::Enum(idx, opts) => opts
                .get(*idx)
                .cloned()
                .unwrap_or_else(|| format!("#{}", idx)),
            Self::Flags(val, _names) => format!("0x{:X}", val),
            Self::Object => "{...}".to_string(),
            Self::Array(n) => format!("[{} items]", n),
        }
    }

    /// Type name for display.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::I32(_) => "i32",
            Self::I64(_) => "i64",
            Self::F32(_) => "f32",
            Self::F64(_) => "f64",
            Self::String(_) => "string",
            Self::Color3(_) => "color3",
            Self::Color4(_) => "color4",
            Self::Vec2(_) => "vec2",
            Self::Vec3(_) => "vec3",
            Self::Vec4(_) => "vec4",
            Self::Enum(_, _) => "enum",
            Self::Flags(_, _) => "flags",
            Self::Object => "object",
            Self::Array(_) => "array",
        }
    }
}
