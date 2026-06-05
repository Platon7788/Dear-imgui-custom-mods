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
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Bool(v) => if *v { "true" } else { "false" }.to_string(),
            Self::I32(v) => v.to_string(),
            Self::I64(v) => v.to_string(),
            Self::F32(v) => format!("{v:.3}"),
            Self::F64(v) => format!("{v:.6}"),
            Self::String(v) => v.clone(),
            Self::Color3(c) => format!("[{:.2}, {:.2}, {:.2}]", c[0], c[1], c[2]),
            Self::Color4(c) => format!("[{:.2}, {:.2}, {:.2}, {:.2}]", c[0], c[1], c[2], c[3]),
            Self::Vec2(v) => format!("[{:.2}, {:.2}]", v[0], v[1]),
            Self::Vec3(v) => format!("[{:.2}, {:.2}, {:.2}]", v[0], v[1], v[2]),
            Self::Vec4(v) => format!("[{:.2}, {:.2}, {:.2}, {:.2}]", v[0], v[1], v[2], v[3]),
            Self::Enum(idx, opts) => opts.get(*idx).cloned().unwrap_or_else(|| format!("#{idx}")),
            Self::Flags(val, _names) => format!("0x{val:X}"),
            Self::Object => "{...}".to_string(),
            Self::Array(n) => format!("[{n} items]"),
        }
    }

    /// Type name for display.
    #[must_use]
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

    /// `true` for container variants whose children are rendered as
    /// indented child rows (`Object`, `Array`).
    #[must_use]
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Object | Self::Array(_))
    }

    /// Clamp internal indices/components into a valid, displayable range
    /// **in place**. Returns `true` when a field was actually adjusted.
    ///
    /// - `Enum` selection index is clamped into `0..options.len()`
    ///   (when there is at least one option; an empty option list is
    ///   left untouched).
    /// - Color / vector float components are clamped into `0.0..=1.0`
    ///   for colors and left as-is for vectors (vectors are unbounded).
    /// - `Color3` / `Color4` channels are clamped to `0.0..=1.0`.
    ///
    /// This is host-facing input validation: a caller building an
    /// `Enum` from an out-of-range selection can call this to avoid the
    /// `#idx` fallback in [`Self::display`].
    pub fn clamp_in_place(&mut self) -> bool {
        match self {
            Self::Enum(idx, opts) => {
                if !opts.is_empty() {
                    let max = opts.len() - 1;
                    if *idx > max {
                        *idx = max;
                        return true;
                    }
                }
                false
            }
            Self::Color3(c) => clamp_channels(c),
            Self::Color4(c) => clamp_channels(c),
            _ => false,
        }
    }

    /// Parse `text` into a value of the **same variant** as `self`,
    /// returning the parsed value on success.
    ///
    /// Used by inline-edit widgets: the variant of `self` selects the
    /// parser, so a `F32` field only accepts a float, an `I32` field an
    /// integer, etc. Returns `None` on a malformed input (leaving the
    /// caller free to keep the previous value), and is a no-op `Some`
    /// echo for variants without a textual editor (`Object`, `Array`,
    /// `Color*`, `Vec*`, `Enum`, `Flags`).
    #[must_use]
    pub fn parse_like(&self, text: &str) -> Option<Self> {
        let t = text.trim();
        match self {
            Self::Bool(_) => match t {
                "true" | "1" => Some(Self::Bool(true)),
                "false" | "0" => Some(Self::Bool(false)),
                _ => None,
            },
            Self::I32(_) => t.parse::<i32>().ok().map(Self::I32),
            Self::I64(_) => t.parse::<i64>().ok().map(Self::I64),
            Self::F32(_) => t.parse::<f32>().ok().map(Self::F32),
            Self::F64(_) => t.parse::<f64>().ok().map(Self::F64),
            Self::String(_) => Some(Self::String(text.to_string())),
            // No textual editor — echo the current value unchanged.
            _ => Some(self.clone()),
        }
    }
}

/// Clamp every channel of a float-color array into `0.0..=1.0`.
/// Returns `true` if any channel changed.
fn clamp_channels<const N: usize>(c: &mut [f32; N]) -> bool {
    let mut changed = false;
    for ch in c.iter_mut() {
        let clamped = ch.clamp(0.0, 1.0);
        if clamped != *ch {
            *ch = clamped;
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_bool_false() {
        assert_eq!(PropertyValue::default(), PropertyValue::Bool(false));
    }

    #[test]
    fn is_container_only_object_and_array() {
        assert!(PropertyValue::Object.is_container());
        assert!(PropertyValue::Array(0).is_container());
        assert!(!PropertyValue::Bool(true).is_container());
        assert!(!PropertyValue::String("x".into()).is_container());
    }

    #[test]
    fn enum_out_of_range_display_falls_back_to_index() {
        // Index past the end → `#idx`, never a panic.
        let v = PropertyValue::Enum(9, vec!["A".into(), "B".into()]);
        assert_eq!(v.display(), "#9");
    }

    #[test]
    fn enum_clamp_pulls_index_into_range() {
        let mut v = PropertyValue::Enum(9, vec!["A".into(), "B".into()]);
        assert!(v.clamp_in_place());
        assert_eq!(v.display(), "B");
        // Idempotent: already in range → no change.
        assert!(!v.clamp_in_place());
    }

    #[test]
    fn enum_clamp_empty_options_is_noop() {
        let mut v = PropertyValue::Enum(3, vec![]);
        assert!(!v.clamp_in_place());
        assert_eq!(v.display(), "#3");
    }

    #[test]
    fn color_clamp_pins_channels_to_unit_range() {
        let mut c3 = PropertyValue::Color3([2.0, -1.0, 0.5]);
        assert!(c3.clamp_in_place());
        assert_eq!(c3, PropertyValue::Color3([1.0, 0.0, 0.5]));

        let mut c4 = PropertyValue::Color4([0.2, 0.4, 0.6, 0.8]);
        assert!(!c4.clamp_in_place(), "already in range");
    }

    #[test]
    fn parse_like_respects_variant() {
        assert_eq!(
            PropertyValue::I32(0).parse_like("42"),
            Some(PropertyValue::I32(42))
        );
        assert_eq!(
            PropertyValue::F32(0.0).parse_like("3.5"),
            Some(PropertyValue::F32(3.5))
        );
        assert_eq!(
            PropertyValue::Bool(false).parse_like("true"),
            Some(PropertyValue::Bool(true))
        );
        assert_eq!(
            PropertyValue::Bool(true).parse_like("0"),
            Some(PropertyValue::Bool(false))
        );
        assert_eq!(
            PropertyValue::String(String::new()).parse_like("  keep spaces  "),
            Some(PropertyValue::String("  keep spaces  ".into()))
        );
    }

    #[test]
    fn parse_like_rejects_malformed() {
        assert_eq!(PropertyValue::I32(0).parse_like("not-a-number"), None);
        assert_eq!(PropertyValue::F64(0.0).parse_like(""), None);
        assert_eq!(PropertyValue::Bool(false).parse_like("yes"), None);
    }

    #[test]
    fn parse_like_echoes_non_textual_variants() {
        let v = PropertyValue::Vec3([1.0, 2.0, 3.0]);
        assert_eq!(v.parse_like("ignored"), Some(v.clone()));
        let obj = PropertyValue::Object;
        assert_eq!(obj.parse_like("ignored"), Some(PropertyValue::Object));
    }

    #[test]
    fn utf8_string_display_roundtrips() {
        // Multi-byte labels/values must survive display untouched.
        let v = PropertyValue::String("Привет — café 🎨".into());
        assert_eq!(v.display(), "Привет — café 🎨");
    }
}
