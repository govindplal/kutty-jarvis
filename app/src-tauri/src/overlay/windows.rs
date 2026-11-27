//! Win32 details of the overlay window that Tauri does not expose.
//!
//! - `WS_EX_TOOLWINDOW` keeps it out of Alt+Tab (Tauri's `skipTaskbar` only
//!   removes the taskbar button).
//! - `WS_EX_NOACTIVATE` and `SWP_NOACTIVATE` mean showing it never takes
//!   focus from the app the user is working in.
//! - `WS_EX_TRANSPARENT` makes it click-through (Tauri's
//!   `set_ignore_cursor_events` sets it too; we re-assert it).
//!
//! The styles are re-applied before every show, in case the windowing layer
//! rewrote them.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_HIDE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crate::uia::Rect;

/// Extended style bits the overlay must have, and the ones it must not.
pub fn overlay_ex_style(current: u32) -> u32 {
    // Topmost is set through SetWindowPos; SetWindowLongPtr ignores it.
    let wanted = WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TRANSPARENT.0;
    (current | wanted) & !WS_EX_APPWINDOW.0
}

/// Applies the overlay styles and covers `bounds` (physical pixels),
/// optionally showing the window without activating it.
pub fn place(hwnd: HWND, bounds: Rect, show: bool) -> windows::core::Result<()> {
    // SAFETY: `hwnd` is the overlay's live top-level window handle.
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let wanted = overlay_ex_style(current);
        if wanted != current {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
        }
        let mut flags = SWP_NOACTIVATE | SWP_FRAMECHANGED;
        if show {
            flags |= SWP_SHOWWINDOW;
        }
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            flags,
        )
    }
}

pub fn hide(hwnd: HWND) {
    // SAFETY: hiding a live window; the return value is the previous state.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_tool_window_and_click_through_bits_and_drops_app_window() {
        let style = overlay_ex_style(WS_EX_APPWINDOW.0 | 0x0008_0000 /* WS_EX_LAYERED */);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(style & WS_EX_TRANSPARENT.0, 0);
        assert_eq!(style & WS_EX_APPWINDOW.0, 0);
        assert_ne!(style & 0x0008_0000, 0, "keeps bits it does not manage");
    }
}
