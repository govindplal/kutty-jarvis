//! Windows backend: walks a window's UI Automation control view (normally
//! the foreground window).
//!
//! One cache request per node fetches every property we need in a single
//! cross-process call. Offscreen subtrees are skipped. The walk checks the
//! deadline before each node, and UIA's own connection and transaction
//! timeouts bound any single call, so a hung app cannot hang the buddy.

use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest, IUIAutomationElement,
    IUIAutomationTreeWalker, UIA_AutomationIdPropertyId, UIA_BoundingRectanglePropertyId,
    UIA_CONTROLTYPE_ID, UIA_ControlTypePropertyId, UIA_IsOffscreenPropertyId, UIA_NamePropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::{BOOL, Interface};

use super::{ListOptions, RawElement, RawSnapshot, Rect, Role, StopReason, WindowTarget};

/// Longest a single UIA call may block, in milliseconds.
const CALL_TIMEOUT_MS: u32 = 1000;

pub fn walk(
    options: &ListOptions,
    sink: &Mutex<RawSnapshot>,
    deadline: Instant,
) -> Result<(), String> {
    let _com = ComApartment::enter()?;
    // SAFETY: plain COM calls on a thread that has entered the MTA above.
    unsafe {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("UI Automation is unavailable: {error}"))?;
        if let Ok(automation2) = automation.cast::<IUIAutomation2>() {
            let _ = automation2.SetConnectionTimeout(CALL_TIMEOUT_MS);
            let _ = automation2.SetTransactionTimeout(CALL_TIMEOUT_MS);
        }
        let cache = automation
            .CreateCacheRequest()
            .map_err(|error| format!("could not create a cache request: {error}"))?;
        for property in [
            UIA_NamePropertyId,
            UIA_ControlTypePropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_IsOffscreenPropertyId,
            UIA_AutomationIdPropertyId,
        ] {
            cache
                .AddProperty(property)
                .map_err(|error| format!("could not configure the cache request: {error}"))?;
        }

        let hwnd = target_window(options.target)?;
        let root = automation
            .ElementFromHandleBuildCache(hwnd, &cache)
            .map_err(|error| format!("could not read the window: {error}"))?;
        {
            let mut snapshot = lock(sink);
            snapshot.window_found = true;
            snapshot.window_title = cached_name(&root);
            snapshot.process_id = process_id(hwnd);
            snapshot.window_rect = cached_rect(&root);
        }

        let walker = automation
            .ControlViewWalker()
            .map_err(|error| format!("could not create a tree walker: {error}"))?;
        let mut context = Walk {
            walker,
            cache,
            sink,
            options,
            deadline,
        };
        context.children(&root, None, 1);
    }
    Ok(())
}

struct Walk<'a> {
    walker: IUIAutomationTreeWalker,
    cache: IUIAutomationCacheRequest,
    sink: &'a Mutex<RawSnapshot>,
    options: &'a ListOptions,
    deadline: Instant,
}

impl Walk<'_> {
    /// Visits the children of `parent` depth first, in tree order. Returns
    /// false once the walk must stop.
    unsafe fn children(
        &mut self,
        parent: &IUIAutomationElement,
        parent_index: Option<usize>,
        depth: usize,
    ) -> bool {
        // SAFETY: COM calls on the walker's MTA thread.
        let mut next = unsafe {
            self.walker
                .GetFirstChildElementBuildCache(parent, &self.cache)
                .ok()
        };
        while let Some(element) = next {
            if !self.may_continue() {
                return false;
            }
            let raw = unsafe { read(&element, parent_index) };
            let offscreen = raw.offscreen;
            let index = {
                let mut snapshot = lock(self.sink);
                snapshot.visited += 1;
                snapshot.elements.push(raw);
                if offscreen {
                    snapshot.pruned_offscreen += 1;
                }
                snapshot.elements.len() - 1
            };
            if !offscreen
                && depth < self.options.max_depth
                && !unsafe { self.children(&element, Some(index), depth + 1) }
            {
                return false;
            }
            next = unsafe {
                self.walker
                    .GetNextSiblingElementBuildCache(&element, &self.cache)
                    .ok()
            };
        }
        true
    }

    fn may_continue(&self) -> bool {
        let mut snapshot = lock(self.sink);
        if snapshot.stop.is_some() {
            return false;
        }
        if Instant::now() >= self.deadline {
            snapshot.stop = Some(StopReason::TimeBudget);
            return false;
        }
        if snapshot.visited >= self.options.max_visited {
            snapshot.stop = Some(StopReason::NodeLimit);
            return false;
        }
        true
    }
}

unsafe fn read(element: &IUIAutomationElement, parent: Option<usize>) -> RawElement {
    // SAFETY: cached property reads; no cross-process call.
    unsafe {
        RawElement {
            name: cached_name(element),
            role: role(element.CachedControlType().unwrap_or(UIA_CONTROLTYPE_ID(0))),
            rect: cached_rect(element),
            automation_id: element
                .CachedAutomationId()
                .map(|id| id.to_string())
                .unwrap_or_default(),
            offscreen: element
                .CachedIsOffscreen()
                .map(|value| value.as_bool())
                .unwrap_or(false),
            parent,
        }
    }
}

unsafe fn cached_name(element: &IUIAutomationElement) -> String {
    unsafe { element.CachedName() }
        .map(|name| name.to_string())
        .unwrap_or_default()
}

unsafe fn cached_rect(element: &IUIAutomationElement) -> Rect {
    match unsafe { element.CachedBoundingRectangle() } {
        Ok(r) => Rect::new(
            r.left,
            r.top,
            r.right.saturating_sub(r.left),
            r.bottom.saturating_sub(r.top),
        ),
        Err(_) => Rect::default(),
    }
}

fn target_window(target: WindowTarget) -> Result<HWND, String> {
    match target {
        WindowTarget::Foreground => {
            // SAFETY: no arguments; returns a null handle when there is none.
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd.is_invalid() {
                Err("there is no foreground window".into())
            } else {
                Ok(hwnd)
            }
        }
        WindowTarget::Process(pid) => {
            let mut search = ProcessWindowSearch { pid, found: None };
            // SAFETY: the callback only runs during this call and `search`
            // outlives it. EnumWindows reports an error when the callback
            // stops early, which is how a match ends the search.
            let _ = unsafe {
                EnumWindows(
                    Some(match_process_window),
                    LPARAM(&mut search as *mut ProcessWindowSearch as isize),
                )
            };
            search
                .found
                .ok_or_else(|| format!("process {pid} has no visible main window"))
        }
    }
}

struct ProcessWindowSearch {
    pid: u32,
    found: Option<HWND>,
}

unsafe extern "system" fn match_process_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `ProcessWindowSearch` passed by `target_window`.
    unsafe {
        let search = &mut *(lparam.0 as *mut ProcessWindowSearch);
        let is_main = process_id(hwnd) == search.pid
            && IsWindowVisible(hwnd).as_bool()
            && GetWindow(hwnd, GW_OWNER).is_err()
            && GetWindowTextLengthW(hwnd) > 0;
        if is_main {
            search.found = Some(hwnd);
        }
        BOOL::from(!is_main)
    }
}

unsafe fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

fn lock(sink: &Mutex<RawSnapshot>) -> std::sync::MutexGuard<'_, RawSnapshot> {
    sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Maps a UIA control type id (UIA_*ControlTypeId, 50000 to 50040).
fn role(control_type: UIA_CONTROLTYPE_ID) -> Role {
    match control_type.0 {
        50000 => Role::Button,
        50001 => Role::Calendar,
        50002 => Role::CheckBox,
        50003 => Role::ComboBox,
        50004 => Role::Edit,
        50005 => Role::Hyperlink,
        50006 => Role::Image,
        50007 => Role::ListItem,
        50008 => Role::List,
        50009 => Role::Menu,
        50010 => Role::MenuBar,
        50011 => Role::MenuItem,
        50012 => Role::ProgressBar,
        50013 => Role::RadioButton,
        50014 => Role::ScrollBar,
        50015 => Role::Slider,
        50016 => Role::Spinner,
        50017 => Role::StatusBar,
        50018 => Role::Tab,
        50019 => Role::TabItem,
        50020 => Role::Text,
        50021 => Role::ToolBar,
        50022 => Role::ToolTip,
        50023 => Role::Tree,
        50024 => Role::TreeItem,
        50025 => Role::Custom,
        50026 => Role::Group,
        50027 => Role::Thumb,
        50028 => Role::DataGrid,
        50029 => Role::DataItem,
        50030 => Role::Document,
        50031 => Role::SplitButton,
        50032 => Role::Window,
        50033 => Role::Pane,
        50034 => Role::Header,
        50035 => Role::HeaderItem,
        50036 => Role::Table,
        50037 => Role::TitleBar,
        50038 => Role::Separator,
        50039 => Role::SemanticZoom,
        50040 => Role::AppBar,
        _ => Role::Unknown,
    }
}

/// Joins the multithreaded COM apartment for the life of the walk.
struct ComApartment;

impl ComApartment {
    fn enter() -> Result<Self, String> {
        // SAFETY: balanced by CoUninitialize in Drop, which only runs when
        // this succeeded.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_err() {
            return Err(format!("could not initialise COM: {result:?}"));
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: matches the successful CoInitializeEx in `enter`.
        unsafe { CoUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Accessibility::{
        UIA_AppBarControlTypeId, UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId,
        UIA_TabItemControlTypeId,
    };

    #[test]
    fn maps_control_type_ids_from_the_sdk() {
        assert_eq!(role(UIA_ButtonControlTypeId), Role::Button);
        assert_eq!(role(UIA_HyperlinkControlTypeId), Role::Hyperlink);
        assert_eq!(role(UIA_TabItemControlTypeId), Role::TabItem);
        assert_eq!(role(UIA_AppBarControlTypeId), Role::AppBar);
        assert_eq!(role(UIA_CONTROLTYPE_ID(1)), Role::Unknown);
    }
}
