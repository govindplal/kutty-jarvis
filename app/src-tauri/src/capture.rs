//! Screen capture for the buddy.
//!
//! Privacy rule (see ADR 0009): a capture happens only when the owner presses
//! the push-to-talk hotkey. The image is kept in memory for the next model call
//! and is never written to disk.
//!
//! Frames are downscaled so the longest edge is at most [`MAX_EDGE`]. Local
//! vision models are several times faster on smaller frames (finding 0003).

use std::io::Cursor;
use std::sync::Mutex;

use xcap::Monitor;
use xcap::image::{ImageFormat, RgbaImage, imageops::FilterType};

pub const MAX_EDGE: u32 = 1280;

/// The most recent capture, held in memory only.
#[derive(Default)]
pub struct LatestScreenshot(pub Mutex<Option<Screenshot>>);

#[derive(Debug, Clone)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

/// Size that fits within `max_edge` on the longest side, keeping aspect ratio.
/// Never scales up and never returns a zero dimension.
pub fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let (width, height) = (width.max(1), height.max(1));
    let longest = width.max(height);
    if longest <= max_edge {
        return (width, height);
    }
    let scale = f64::from(max_edge) / f64::from(longest);
    let scaled_w = ((f64::from(width) * scale).round() as u32).max(1);
    let scaled_h = ((f64::from(height) * scale).round() as u32).max(1);
    (scaled_w, scaled_h)
}

pub fn downscale(image: &RgbaImage, max_edge: u32) -> RgbaImage {
    let (width, height) = fit_within(image.width(), image.height(), max_edge);
    if (width, height) == (image.width(), image.height()) {
        return image.clone();
    }
    xcap::image::imageops::resize(image, width, height, FilterType::Triangle)
}

pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| format!("could not encode screenshot: {error}"))?;
    Ok(bytes.into_inner())
}

/// Captures the primary monitor (or the first one if none is marked primary).
pub fn capture_primary() -> Result<Screenshot, String> {
    let monitors = Monitor::all().map_err(|error| format!("could not list displays: {error}"))?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| "no display found".to_string())?;
    let raw = monitor
        .capture_image()
        .map_err(|error| format!("screen capture failed: {error}"))?;
    let frame = downscale(&raw, MAX_EDGE);
    Ok(Screenshot {
        width: frame.width(),
        height: frame.height(),
        png: encode_png(&frame)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_small_frames_alone() {
        assert_eq!(fit_within(800, 600, 1280), (800, 600));
    }

    #[test]
    fn scales_landscape_to_longest_edge() {
        assert_eq!(fit_within(2560, 1440, 1280), (1280, 720));
    }

    #[test]
    fn scales_portrait_to_longest_edge() {
        assert_eq!(fit_within(1080, 2400, 1280), (576, 1280));
    }

    #[test]
    fn never_returns_a_zero_dimension() {
        assert_eq!(fit_within(10_000, 1, 1280), (1280, 1));
        assert_eq!(fit_within(0, 0, 1280), (1, 1));
    }

    #[test]
    fn downscale_produces_the_fitted_size() {
        let image = RgbaImage::new(2560, 1440);
        let small = downscale(&image, 1280);
        assert_eq!((small.width(), small.height()), (1280, 720));
    }

    #[test]
    fn png_round_trips_pixels() {
        let mut image = RgbaImage::new(4, 2);
        image.put_pixel(1, 1, xcap::image::Rgba([10, 20, 30, 255]));
        let bytes = encode_png(&image).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = xcap::image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(1, 1).0, [10, 20, 30, 255]);
    }
}
