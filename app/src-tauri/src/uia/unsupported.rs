//! Placeholder backend for platforms without an accessibility walker yet.
//! macOS (AXUIElement) and Linux (AT-SPI) backends would replace this.

use std::sync::Mutex;
use std::time::Instant;

use super::{ListOptions, RawSnapshot};

pub fn walk(
    _options: &ListOptions,
    _sink: &Mutex<RawSnapshot>,
    _deadline: Instant,
) -> Result<(), String> {
    Err("listing UI elements is only implemented on Windows so far".into())
}
