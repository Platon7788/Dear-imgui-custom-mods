//! Window icon (taskbar / Alt-Tab thumbnail).

use std::path::Path;

/// Raw RGBA8 icon for the window's taskbar / Alt-Tab thumbnail.
///
/// Construct with [`WindowIcon::from_rgba`] (raw pixels in memory) or
/// [`WindowIcon::from_rgba_file`] (raw pixels read from disk).
///
/// PNG / JPEG decoding is intentionally **not** part of this crate — pull
/// in the `image` crate yourself if you need it, decode to `Vec<u8>` (RGBA8)
/// and feed the buffer to [`WindowIcon::from_rgba`].
#[derive(Debug, Clone)]
pub struct WindowIcon {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl WindowIcon {
    /// Build from a tightly-packed RGBA byte slice (`width * height * 4` bytes).
    /// Returns `Err` if the buffer length doesn't match the dimensions.
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, &'static str> {
        if (width as usize) * (height as usize) * 4 != rgba.len() {
            return Err("rgba length must equal width * height * 4");
        }
        Ok(Self {
            rgba,
            width,
            height,
        })
    }

    /// Build from raw RGBA bytes loaded from disk.
    pub fn from_rgba_file(
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
    ) -> Result<Self, std::io::Error> {
        let rgba = std::fs::read(path)?;
        Self::from_rgba(rgba, width, height)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}
