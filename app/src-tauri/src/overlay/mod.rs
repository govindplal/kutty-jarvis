//! The pointing overlay (ADR 0013).
//!
//! A second Tauri window (`overlay`, see tauri.conf.json) covers the primary
//! monitor. It is transparent, always on top, click-through, never takes
//! focus and stays out of the taskbar and Alt+Tab. It is hidden while idle.
//!
//! [`point_at`] converts the cursor position and the target rectangle from
//! physical screen pixels to the overlay's CSS pixels, shows the window
//! without activating it, and sends a [`PointRequest`]. The overlay page
//! (`src/overlay/`) animates a cursor along an eased arc to the centre of the
//! target, highlights it briefly and fades. We hide the window again once the
//! animation has had time to finish.

#[cfg(windows)]
mod windows;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{App, AppHandle, Emitter, Manager, WebviewWindow};

use crate::uia::Rect;

pub const OVERLAY_LABEL: &str = "overlay";
pub const POINT_EVENT: &str = "overlay-point";
pub const DEFAULT_HOLD_MS: u32 = 900;
pub const FADE_MS: u32 = 300;
/// Slack after the animation before the window is hidden.
const HIDE_MARGIN_MS: u32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CssPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CssRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A monitor in physical pixels plus its scale factor (DPI / 96).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MonitorGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl MonitorGeometry {
    pub fn bounds(&self) -> Rect {
        Rect::new(
            self.x,
            self.y,
            i32::try_from(self.width).unwrap_or(i32::MAX),
            i32::try_from(self.height).unwrap_or(i32::MAX),
        )
    }

    fn scale(&self) -> f64 {
        if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        }
    }

    /// Physical screen point to CSS pixels inside an overlay that exactly
    /// covers this monitor.
    pub fn to_css_point(&self, x: f64, y: f64) -> CssPoint {
        CssPoint {
            x: (x - f64::from(self.x)) / self.scale(),
            y: (y - f64::from(self.y)) / self.scale(),
        }
    }

    pub fn to_css_rect(&self, rect: Rect) -> CssRect {
        let origin = self.to_css_point(f64::from(rect.x), f64::from(rect.y));
        CssRect {
            x: origin.x,
            y: origin.y,
            width: f64::from(rect.width) / self.scale(),
            height: f64::from(rect.height) / self.scale(),
        }
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        let bounds = self.bounds();
        x >= f64::from(bounds.x)
            && y >= f64::from(bounds.y)
            && x < bounds.right() as f64
            && y < bounds.bottom() as f64
    }

    /// Nearest point on this monitor, for a cursor sitting on another one.
    pub fn clamp_point(&self, x: f64, y: f64) -> (f64, f64) {
        let bounds = self.bounds();
        let max_x = (bounds.right() - 1) as f64;
        let max_y = (bounds.bottom() - 1) as f64;
        (
            x.clamp(f64::from(bounds.x), max_x.max(f64::from(bounds.x))),
            y.clamp(f64::from(bounds.y), max_y.max(f64::from(bounds.y))),
        )
    }
}

/// Flight time grows with distance but stays between 0.45 and 1 s, so short
/// hops are not sluggish and long ones are still easy to follow.
pub fn flight_ms(distance_css: f64) -> u32 {
    let distance = if distance_css.is_finite() {
        distance_css.max(0.0)
    } else {
        0.0
    };
    (450.0 + distance * 0.35).clamp(450.0, 1000.0).round() as u32
}

/// What the overlay page needs to animate one pointing gesture. All
/// positions are CSS pixels relative to the overlay's top-left corner.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointRequest {
    pub sequence: u64,
    pub from: CssPoint,
    pub target: CssRect,
    /// The scale factor used for the conversion; the page compares it with
    /// its own `devicePixelRatio`.
    pub scale: f64,
    pub flight_ms: u32,
    pub hold_ms: u32,
    pub fade_ms: u32,
}

impl PointRequest {
    pub fn total_ms(&self) -> u32 {
        self.flight_ms + self.hold_ms + self.fade_ms
    }
}

/// Plans a gesture from the cursor to the centre of `target`, all inputs in
/// physical screen pixels.
pub fn plan(
    monitor: &MonitorGeometry,
    cursor: (f64, f64),
    target: Rect,
    hold_ms: u32,
    sequence: u64,
) -> Result<PointRequest, String> {
    if target.is_empty() {
        return Err("the target has no size".into());
    }
    let (center_x, center_y) = target.center();
    if !monitor.contains_point(center_x, center_y) {
        return Err(
            "the target is not on the primary monitor (other monitors are not supported yet)"
                .into(),
        );
    }
    let (cursor_x, cursor_y) = monitor.clamp_point(cursor.0, cursor.1);
    let from = monitor.to_css_point(cursor_x, cursor_y);
    let to = monitor.to_css_point(center_x, center_y);
    let distance = (to.x - from.x).hypot(to.y - from.y);
    Ok(PointRequest {
        sequence,
        from,
        target: monitor.to_css_rect(target),
        scale: monitor.scale(),
        flight_ms: flight_ms(distance),
        hold_ms,
        fade_ms: FADE_MS,
    })
}

/// Counts gestures so a late hide from an earlier one cannot cut a newer one.
#[derive(Default)]
pub struct OverlaySequence(AtomicU64);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointReport {
    pub request: PointRequest,
    pub monitor: MonitorGeometry,
    /// Cursor position the flight started from, physical pixels.
    pub cursor: (f64, f64),
    /// Target rectangle, physical pixels.
    pub target: Rect,
}

/// Makes the overlay click-through and keeps it out of the taskbar and
/// Alt+Tab. Called once at start-up; the window stays hidden.
pub fn init(app: &App) -> tauri::Result<()> {
    app.manage(OverlaySequence::default());
    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };
    window.set_ignore_cursor_events(true)?;
    if let Ok(Some(monitor)) = app.primary_monitor() {
        place(&window, &geometry(&monitor), false)?;
    }
    Ok(())
}

/// Flies the overlay cursor from the mouse position to `target` (physical
/// screen pixels) and highlights it.
pub fn point_at(app: &AppHandle, target: Rect, hold_ms: u32) -> Result<PointReport, String> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or("the overlay window is missing")?;
    let monitor = app
        .primary_monitor()
        .map_err(|error| format!("could not read the monitors: {error}"))?
        .ok_or("no primary monitor found")?;
    let monitor = geometry(&monitor);
    let cursor = app
        .cursor_position()
        .map(|position| (position.x, position.y))
        .unwrap_or((
            f64::from(monitor.x) + f64::from(monitor.width) / 2.0,
            f64::from(monitor.y) + f64::from(monitor.height) / 2.0,
        ));
    let sequence = app
        .state::<OverlaySequence>()
        .0
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let request = plan(&monitor, cursor, target, hold_ms, sequence)?;

    place(&window, &monitor, true)
        .map_err(|error| format!("could not show the overlay: {error}"))?;
    window
        .emit_to(OVERLAY_LABEL, POINT_EVENT, request.clone())
        .map_err(|error| format!("could not reach the overlay: {error}"))?;
    hide_later(app.clone(), sequence, request.total_ms() + HIDE_MARGIN_MS);

    Ok(PointReport {
        request,
        monitor,
        cursor,
        target,
    })
}

fn geometry(monitor: &tauri::Monitor) -> MonitorGeometry {
    MonitorGeometry {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
        scale: monitor.scale_factor(),
    }
}

fn hide_later(app: AppHandle, sequence: u64, after_ms: u32) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(u64::from(after_ms)));
        if app.state::<OverlaySequence>().0.load(Ordering::SeqCst) != sequence {
            return; // a newer gesture owns the overlay now
        }
        if let Some(window) = app.get_webview_window(OVERLAY_LABEL)
            && let Err(error) = hide(&window)
        {
            eprintln!("could not hide the overlay: {error}");
        }
    });
}

/// Moves the overlay over `monitor`, showing it without activation if asked.
#[cfg(windows)]
fn place(window: &WebviewWindow, monitor: &MonitorGeometry, show: bool) -> tauri::Result<()> {
    windows::place(window.hwnd()?, monitor.bounds(), show)
        .map_err(|error| tauri::Error::Anyhow(error.into()))
}

#[cfg(windows)]
fn hide(window: &WebviewWindow) -> tauri::Result<()> {
    windows::hide(window.hwnd()?);
    Ok(())
}

#[cfg(not(windows))]
fn place(window: &WebviewWindow, monitor: &MonitorGeometry, show: bool) -> tauri::Result<()> {
    window.set_position(tauri::PhysicalPosition::new(monitor.x, monitor.y))?;
    window.set_size(tauri::PhysicalSize::new(monitor.width, monitor.height))?;
    if show {
        window.show()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn hide(window: &WebviewWindow) -> tauri::Result<()> {
    window.hide()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_HD_150: MonitorGeometry = MonitorGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.5,
    };

    #[test]
    fn converts_physical_pixels_to_css_at_150_percent() {
        assert_eq!(
            FULL_HD_150.to_css_point(960.0, 540.0),
            CssPoint { x: 640.0, y: 360.0 }
        );
        assert_eq!(
            FULL_HD_150.to_css_rect(Rect::new(300, 150, 120, 45)),
            CssRect {
                x: 200.0,
                y: 100.0,
                width: 80.0,
                height: 30.0
            }
        );
    }

    #[test]
    fn subtracts_the_monitor_origin() {
        let left_of_primary = MonitorGeometry {
            x: -1280,
            y: -200,
            width: 1280,
            height: 1024,
            scale: 1.25,
        };
        assert_eq!(
            left_of_primary.to_css_point(-1280.0 + 250.0, -200.0 + 125.0),
            CssPoint { x: 200.0, y: 100.0 }
        );
    }

    #[test]
    fn a_broken_scale_factor_falls_back_to_one() {
        let broken = MonitorGeometry {
            scale: 0.0,
            ..FULL_HD_150
        };
        assert_eq!(
            broken.to_css_point(10.0, 20.0),
            CssPoint { x: 10.0, y: 20.0 }
        );
    }

    #[test]
    fn contains_and_clamps_points() {
        assert!(FULL_HD_150.contains_point(0.0, 0.0));
        assert!(FULL_HD_150.contains_point(1919.5, 1079.0));
        assert!(!FULL_HD_150.contains_point(1920.0, 10.0));
        assert!(!FULL_HD_150.contains_point(-1.0, 10.0));
        assert_eq!(FULL_HD_150.clamp_point(2500.0, -40.0), (1919.0, 0.0));
        assert_eq!(FULL_HD_150.clamp_point(100.0, 200.0), (100.0, 200.0));
    }

    #[test]
    fn flight_time_grows_with_distance_within_bounds() {
        assert_eq!(flight_ms(0.0), 450);
        assert_eq!(flight_ms(400.0), 590);
        assert_eq!(flight_ms(5000.0), 1000);
        assert_eq!(flight_ms(f64::NAN), 450);
        assert!(flight_ms(300.0) < flight_ms(600.0));
    }

    #[test]
    fn plans_a_gesture_in_css_pixels() {
        let request = plan(
            &FULL_HD_150,
            (0.0, 0.0),
            Rect::new(600, 300, 120, 60),
            900,
            7,
        )
        .unwrap();
        assert_eq!(request.from, CssPoint { x: 0.0, y: 0.0 });
        assert_eq!(
            request.target,
            CssRect {
                x: 400.0,
                y: 200.0,
                width: 80.0,
                height: 40.0
            }
        );
        // Target centre (440, 220) is about 492 CSS px away.
        assert_eq!(request.flight_ms, 622);
        assert_eq!(request.total_ms(), 622 + 900 + FADE_MS);
        assert_eq!(request.sequence, 7);
        assert_eq!(request.scale, 1.5);
    }

    #[test]
    fn plan_clamps_a_cursor_on_another_monitor() {
        let request = plan(
            &FULL_HD_150,
            (-500.0, 300.0),
            Rect::new(600, 300, 120, 60),
            0,
            1,
        )
        .unwrap();
        assert_eq!(request.from, CssPoint { x: 0.0, y: 200.0 });
    }

    #[test]
    fn plan_rejects_targets_off_the_monitor_or_empty() {
        assert!(plan(&FULL_HD_150, (0.0, 0.0), Rect::new(2000, 10, 50, 50), 0, 1).is_err());
        assert!(plan(&FULL_HD_150, (0.0, 0.0), Rect::new(10, 10, 0, 50), 0, 1).is_err());
    }

    #[test]
    fn request_serialises_in_camel_case() {
        let request = plan(&FULL_HD_150, (0.0, 0.0), Rect::new(0, 0, 30, 30), 100, 1).unwrap();
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["flightMs"], 455);
        assert_eq!(json["holdMs"], 100);
        assert_eq!(json["fadeMs"], FADE_MS);
        assert_eq!(json["target"]["width"], 20.0);
    }
}
