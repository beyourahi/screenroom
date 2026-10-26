//! One decoded video frame, in a renderer-neutral form.
//!
//! The desktop client draws this through wgpu; a future zero-copy adapter can
//! produce the same type after importing a platform texture, so the presentation
//! layer never learns which bridge produced the picture.

use std::sync::Arc;

/// One frame in tightly packed RGBA8, top row first.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// RGBA bytes, exactly `width * height * 4` long.
    pub rgba: Arc<Vec<u8>>,
    /// Monotonic frame number; the presentation layer skips re-uploads.
    pub generation: u64,
}

impl Frame {
    /// True when the buffer length matches the declared size.
    pub fn is_well_formed(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.rgba.len() == self.width as usize * self.height as usize * 4
    }

    /// Reads one pixel, or `None` when out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height || !self.is_well_formed() {
            return None;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        Some([
            self.rgba[offset],
            self.rgba[offset + 1],
            self.rgba[offset + 2],
            self.rgba[offset + 3],
        ])
    }

    /// True when every pixel is fully transparent black — an empty render.
    pub fn is_blank(&self) -> bool {
        let (pixels, _) = self.rgba.as_chunks::<4>();
        pixels
            .iter()
            .all(|pixel| pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 && pixel[3] == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32, bytes: Vec<u8>) -> Frame {
        Frame {
            width,
            height,
            rgba: Arc::new(bytes),
            generation: 1,
        }
    }

    #[test]
    fn well_formed_checks_buffer_length() {
        assert!(frame(2, 2, vec![0; 16]).is_well_formed());
        assert!(!frame(2, 2, vec![0; 15]).is_well_formed());
        assert!(!frame(0, 2, vec![]).is_well_formed());
    }

    #[test]
    fn pixel_reads_rgba_in_row_major_order() {
        let mut bytes = vec![0u8; 2 * 2 * 4];
        bytes[4..8].copy_from_slice(&[10, 20, 30, 255]);
        let frame = frame(2, 2, bytes);
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 0]));
        assert_eq!(frame.pixel(1, 0), Some([10, 20, 30, 255]));
        assert_eq!(frame.pixel(2, 0), None);
        assert_eq!(frame.pixel(0, 2), None);
    }

    #[test]
    fn blank_means_all_zero_bytes() {
        assert!(frame(2, 2, vec![0; 16]).is_blank());
        let mut bytes = vec![0u8; 16];
        bytes[3] = 255;
        assert!(!frame(2, 2, bytes).is_blank());
    }
}
