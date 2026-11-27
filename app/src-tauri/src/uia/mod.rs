//! On-screen UI elements of the foreground window, as pointing targets.
//!
//! ADR 0005: the accessibility tree supplies exact element rectangles and the
//! model picks an element by number from a list. This module produces that
//! numbered list ([`list_elements`]) and its compact text form for a prompt
//! ([`render_for_prompt`]). It never calls a model.
//!
//! Layering: the platform backend (Windows UI Automation in `windows.rs`)
//! only walks the tree and reports raw nodes. Everything else (filtering,
//! de-duplication, capping, numbering, rendering) is plain Rust in this file,
//! so it is unit-tested and shared by future macOS/Linux backends.
//!
//! Coordinates are physical screen pixels (the process is per-monitor DPI
//! aware), the same space as the cursor position and the monitor bounds.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use self::windows as backend;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
use self::unsupported as backend;

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Default number of elements handed to the model.
pub const DEFAULT_CAP: usize = 150;
/// Default wall-clock budget for one enumeration.
pub const DEFAULT_TIME_BUDGET: Duration = Duration::from_millis(1500);
/// Extra time the caller waits for a stuck backend call before giving up.
const HARD_GRACE: Duration = Duration::from_millis(500);
/// Elements smaller than this on either side are treated as decoration.
const MIN_SIDE: i32 = 2;
/// Longest name shown in a prompt line, in characters.
const PROMPT_NAME_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && i64::from(self.x) < other.right()
            && i64::from(other.x) < self.right()
            && i64::from(self.y) < other.bottom()
            && i64::from(other.y) < self.bottom()
    }

    pub fn contains(&self, other: &Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    /// Centre in (possibly fractional) pixels.
    pub fn center(&self) -> (f64, f64) {
        (
            f64::from(self.x) + f64::from(self.width) / 2.0,
            f64::from(self.y) + f64::from(self.height) / 2.0,
        )
    }
}

/// Element role, mirroring the UI Automation control types. Other platforms
/// map their roles onto the same set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    AppBar,
    Button,
    Calendar,
    CheckBox,
    ComboBox,
    Custom,
    DataGrid,
    DataItem,
    Document,
    Edit,
    Group,
    Header,
    HeaderItem,
    Hyperlink,
    Image,
    List,
    ListItem,
    Menu,
    MenuBar,
    MenuItem,
    Pane,
    ProgressBar,
    RadioButton,
    ScrollBar,
    SemanticZoom,
    Separator,
    Slider,
    Spinner,
    SplitButton,
    StatusBar,
    Tab,
    TabItem,
    Table,
    Text,
    Thumb,
    TitleBar,
    ToolBar,
    ToolTip,
    Tree,
    TreeItem,
    Window,
    Unknown,
}

impl Role {
    /// Short lowercase name used in JSON and in the prompt.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::AppBar => "appbar",
            Role::Button => "button",
            Role::Calendar => "calendar",
            Role::CheckBox => "checkbox",
            Role::ComboBox => "combobox",
            Role::Custom => "custom",
            Role::DataGrid => "datagrid",
            Role::DataItem => "dataitem",
            Role::Document => "document",
            Role::Edit => "edit",
            Role::Group => "group",
            Role::Header => "header",
            Role::HeaderItem => "headeritem",
            Role::Hyperlink => "link",
            Role::Image => "image",
            Role::List => "list",
            Role::ListItem => "listitem",
            Role::Menu => "menu",
            Role::MenuBar => "menubar",
            Role::MenuItem => "menuitem",
            Role::Pane => "pane",
            Role::ProgressBar => "progressbar",
            Role::RadioButton => "radio",
            Role::ScrollBar => "scrollbar",
            Role::SemanticZoom => "semanticzoom",
            Role::Separator => "separator",
            Role::Slider => "slider",
            Role::Spinner => "spinner",
            Role::SplitButton => "splitbutton",
            Role::StatusBar => "statusbar",
            Role::Tab => "tablist",
            Role::TabItem => "tab",
            Role::Table => "table",
            Role::Text => "text",
            Role::Thumb => "thumb",
            Role::TitleBar => "titlebar",
            Role::ToolBar => "toolbar",
            Role::ToolTip => "tooltip",
            Role::Tree => "tree",
            Role::TreeItem => "treeitem",
            Role::Window => "window",
            Role::Unknown => "unknown",
        }
    }

    /// Things a person clicks, types into or selects. These win when the list
    /// has to be cut down to the cap.
    pub fn is_interactive(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::CheckBox
                | Role::ComboBox
                | Role::DataItem
                | Role::Document
                | Role::Edit
                | Role::HeaderItem
                | Role::Hyperlink
                | Role::ListItem
                | Role::MenuItem
                | Role::RadioButton
                | Role::Slider
                | Role::Spinner
                | Role::SplitButton
                | Role::TabItem
                | Role::TreeItem
        )
    }

    /// Roles that are meaningful even without a name (an unlabelled text box
    /// is still somewhere you can type).
    fn meaningful_without_name(self) -> bool {
        matches!(self, Role::Edit | Role::Document | Role::ComboBox)
    }

    /// Purely visual parts that never make a useful target.
    fn is_decoration(self) -> bool {
        matches!(self, Role::Separator | Role::Thumb | Role::ScrollBar)
    }
}

impl Serialize for Role {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One node as the platform backend saw it, before any filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct RawElement {
    pub name: String,
    pub role: Role,
    pub rect: Rect,
    pub automation_id: String,
    pub offscreen: bool,
    /// Index of the parent in the same snapshot; `None` for children of the
    /// window itself. Parents always come before their children.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    TimeBudget,
    NodeLimit,
}

/// What the backend collected. Filled incrementally so a walk cut short by
/// the time budget still yields what it found.
#[derive(Debug, Clone, Default)]
pub struct RawSnapshot {
    pub window_found: bool,
    pub window_title: String,
    pub process_id: u32,
    pub window_rect: Rect,
    pub elements: Vec<RawElement>,
    /// Nodes read from the tree, including the ones filtered out later.
    pub visited: usize,
    /// Subtrees skipped because their root was offscreen.
    pub pruned_offscreen: usize,
    pub stop: Option<StopReason>,
}

/// Which window to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowTarget {
    /// The window the user is working in (the buddy's normal case).
    #[default]
    Foreground,
    /// The first visible, unowned, titled top-level window of a process.
    /// Used by tests and debugging, where focus cannot be arranged.
    Process(u32),
}

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub target: WindowTarget,
    pub cap: usize,
    pub time_budget: Duration,
    /// Upper bound on nodes read, whatever the time.
    pub max_visited: usize,
    pub max_depth: usize,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            target: WindowTarget::Foreground,
            cap: DEFAULT_CAP,
            time_budget: DEFAULT_TIME_BUDGET,
            max_visited: 5000,
            max_depth: 64,
        }
    }
}

/// A numbered pointing target.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    /// 1-based number the model answers with.
    pub id: usize,
    pub role: Role,
    pub name: String,
    /// Physical screen pixels.
    pub rect: Rect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementList {
    pub window_title: String,
    pub process_id: u32,
    pub window_rect: Rect,
    pub elements: Vec<Element>,
    /// Elements that passed the filters, before the cap.
    pub candidates: usize,
    pub visited: usize,
    pub pruned_offscreen: usize,
    pub elapsed_ms: u64,
    /// True when more candidates existed than the cap allowed.
    pub capped: bool,
    /// True when the walk stopped early; the list is then incomplete.
    pub partial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
}

impl ElementList {
    pub fn get(&self, id: usize) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }
}

/// Lists the foreground window's elements with the default cap and budget.
pub fn list_elements() -> Result<ElementList, String> {
    list_elements_with(&ListOptions::default())
}

/// Lists the foreground window's elements. The walk runs on its own thread;
/// if it overruns the budget we return what it found so far, flagged partial.
pub fn list_elements_with(options: &ListOptions) -> Result<ElementList, String> {
    let started = Instant::now();
    let deadline = started + options.time_budget;
    let sink = Arc::new(Mutex::new(RawSnapshot::default()));
    let (done_tx, done_rx) = mpsc::channel();

    let worker_sink = Arc::clone(&sink);
    let worker_options = options.clone();
    thread::Builder::new()
        .name("ui-elements".into())
        .spawn(move || {
            let outcome = backend::walk(&worker_options, &worker_sink, deadline);
            let _ = done_tx.send(outcome);
        })
        .map_err(|error| format!("could not start the element walker: {error}"))?;

    let outcome = done_rx.recv_timeout(options.time_budget + HARD_GRACE);
    let mut snapshot = std::mem::take(&mut *lock(&sink));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(message)) if !snapshot.window_found => return Err(message),
        // The walk failed part-way: keep what it found, flagged partial.
        Ok(Err(_)) => snapshot.stop = snapshot.stop.or(Some(StopReason::TimeBudget)),
        Err(RecvTimeoutError::Timeout) => snapshot.stop = Some(StopReason::TimeBudget),
        Err(RecvTimeoutError::Disconnected) => {
            return Err("the element walker stopped unexpectedly".into());
        }
    }
    if !snapshot.window_found {
        return Err("no foreground window could be read in time".into());
    }
    Ok(build_list(snapshot, options.cap, started.elapsed()))
}

fn lock(sink: &Mutex<RawSnapshot>) -> std::sync::MutexGuard<'_, RawSnapshot> {
    sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Turns a raw snapshot into the numbered list.
pub fn build_list(snapshot: RawSnapshot, cap: usize, elapsed: Duration) -> ElementList {
    let kept = filter(&snapshot.elements, &snapshot.window_rect);
    let candidates = kept.len();
    let chosen = apply_cap(&snapshot.elements, kept, cap);
    let elements = number(&snapshot.elements, &chosen);
    ElementList {
        window_title: clean_name(&snapshot.window_title),
        process_id: snapshot.process_id,
        window_rect: snapshot.window_rect,
        elements,
        candidates,
        visited: snapshot.visited,
        pruned_offscreen: snapshot.pruned_offscreen,
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        capped: candidates > chosen.len(),
        partial: snapshot.stop.is_some(),
        stop_reason: snapshot.stop,
    }
}

/// Indices of raw elements worth showing, in tree order.
pub fn filter(raw: &[RawElement], window: &Rect) -> Vec<usize> {
    let mut kept = vec![false; raw.len()];
    for (index, element) in raw.iter().enumerate() {
        kept[index] = is_meaningful(element, window)
            && !repeats_ancestor(raw, &kept, index)
            && !is_cell_of_kept_item(raw, &kept, index);
    }
    (0..raw.len()).filter(|&index| kept[index]).collect()
}

/// Text, edit, image and group parts inside a kept list, data, tree or tab
/// item (the Name / Date modified / Size cells of an Explorer row, the path
/// group inside a VS Code tab) belong to one target: the item. Listing them
/// would crowd real targets out of the cap.
fn is_cell_of_kept_item(raw: &[RawElement], kept: &[bool], index: usize) -> bool {
    if !matches!(
        raw[index].role,
        Role::Text | Role::Edit | Role::Image | Role::Group
    ) {
        return false;
    }
    let mut parent = raw[index].parent;
    for _ in 0..2 {
        let Some(ancestor_index) = parent else {
            return false;
        };
        let ancestor = &raw[ancestor_index];
        if kept[ancestor_index]
            && matches!(
                ancestor.role,
                Role::ListItem | Role::DataItem | Role::TreeItem | Role::TabItem
            )
        {
            return true;
        }
        parent = ancestor.parent;
    }
    false
}

fn is_meaningful(element: &RawElement, window: &Rect) -> bool {
    if element.offscreen || element.role.is_decoration() {
        return false;
    }
    if element.rect.width < MIN_SIDE || element.rect.height < MIN_SIDE {
        return false;
    }
    if !window.is_empty() && !element.rect.intersects(window) {
        return false;
    }
    !clean_name(&element.name).is_empty() || element.role.meaningful_without_name()
}

/// A text or image label inside a kept ancestor with the same name (the
/// "Save" text inside the "Save" button) adds nothing; drop it.
fn repeats_ancestor(raw: &[RawElement], kept: &[bool], index: usize) -> bool {
    let element = &raw[index];
    if !matches!(element.role, Role::Text | Role::Image) {
        return false;
    }
    let name = clean_name(&element.name);
    let mut parent = element.parent;
    for _ in 0..3 {
        let Some(ancestor_index) = parent else {
            return false;
        };
        let ancestor = &raw[ancestor_index];
        if kept[ancestor_index]
            && clean_name(&ancestor.name) == name
            && ancestor.rect.contains(&element.rect)
        {
            return true;
        }
        parent = ancestor.parent;
    }
    false
}

/// Keeps at most `cap` elements: interactive ones first, then the rest, each
/// group in tree order. The result is back in tree order.
pub fn apply_cap(raw: &[RawElement], kept: Vec<usize>, cap: usize) -> Vec<usize> {
    if kept.len() <= cap {
        return kept;
    }
    let (interactive, other): (Vec<usize>, Vec<usize>) = kept
        .into_iter()
        .partition(|&index| raw[index].role.is_interactive());
    let mut chosen: Vec<usize> = interactive.into_iter().chain(other).take(cap).collect();
    chosen.sort_unstable();
    chosen
}

/// Numbers the chosen elements 1..=n in tree order.
pub fn number(raw: &[RawElement], chosen: &[usize]) -> Vec<Element> {
    chosen
        .iter()
        .enumerate()
        .map(|(position, &index)| {
            let element = &raw[index];
            let automation_id = element.automation_id.trim();
            Element {
                id: position + 1,
                role: element.role,
                name: clean_name(&element.name),
                rect: element.rect,
                automation_id: (!automation_id.is_empty()).then(|| automation_id.to_string()),
            }
        })
        .collect()
}

/// Collapses whitespace and control characters into single spaces.
/// Also drops Unicode private-use characters: icon fonts (VS Code's
/// codicons, Segoe Fluent Icons) put glyphs there, which mean nothing to a
/// model.
pub fn clean_name(name: &str) -> String {
    name.split(|c: char| c.is_whitespace() || c.is_control() || is_private_use(c))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_private_use(c: char) -> bool {
    matches!(u32::from(c), 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
}

/// Compact text form for a model prompt, one element per line:
/// `12 button "Save" (x,y,w,h)` in physical screen pixels.
pub fn render_for_prompt(list: &ElementList) -> String {
    let mut header = format!(
        "UI elements of window \"{}\" (number role \"name\" (x,y,width,height) in screen pixels)",
        prompt_name(&list.window_title)
    );
    if list.capped {
        header.push_str(&format!(
            "; showing {} of {}",
            list.elements.len(),
            list.candidates
        ));
    }
    if list.partial {
        header.push_str("; list incomplete, the app was slow to answer");
    }
    let mut out = header;
    out.push('\n');
    for element in &list.elements {
        out.push_str(&render_line(element));
        out.push('\n');
    }
    out
}

pub fn render_line(element: &Element) -> String {
    let rect = element.rect;
    let mut line = format!(
        "{} {} \"{}\" ({},{},{},{})",
        element.id,
        element.role.as_str(),
        prompt_name(&element.name),
        rect.x,
        rect.y,
        rect.width,
        rect.height
    );
    if element.name.is_empty()
        && let Some(id) = &element.automation_id
    {
        line.push_str(&format!(" [id {}]", prompt_name(id)));
    }
    line
}

/// Name as it appears between quotes: cleaned, double quotes swapped for
/// single ones, cut to [`PROMPT_NAME_CHARS`] characters.
pub fn prompt_name(name: &str) -> String {
    let cleaned = clean_name(name).replace('"', "'");
    if cleaned.chars().count() <= PROMPT_NAME_CHARS {
        return cleaned;
    }
    let mut cut: String = cleaned.chars().take(PROMPT_NAME_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rect = Rect::new(0, 0, 1000, 800);

    fn raw(name: &str, role: Role, rect: Rect, parent: Option<usize>) -> RawElement {
        RawElement {
            name: name.into(),
            role,
            rect,
            automation_id: String::new(),
            offscreen: false,
            parent,
        }
    }

    fn snapshot(elements: Vec<RawElement>) -> RawSnapshot {
        RawSnapshot {
            window_found: true,
            window_title: "Untitled - Notepad".into(),
            process_id: 42,
            window_rect: WINDOW,
            visited: elements.len(),
            elements,
            ..RawSnapshot::default()
        }
    }

    #[test]
    fn rect_geometry() {
        let a = Rect::new(10, 10, 100, 50);
        assert!(a.intersects(&Rect::new(100, 50, 10, 10)));
        assert!(!a.intersects(&Rect::new(110, 10, 10, 10)), "touching edges");
        assert!(
            !a.intersects(&Rect::new(20, 20, 0, 10)),
            "empty never intersects"
        );
        assert!(a.contains(&Rect::new(10, 10, 100, 50)));
        assert!(!a.contains(&Rect::new(9, 10, 10, 10)));
        assert_eq!(Rect::new(10, 20, 31, 10).center(), (25.5, 25.0));
    }

    #[test]
    fn keeps_named_visible_elements() {
        let elements = vec![raw("Save", Role::Button, Rect::new(10, 10, 80, 24), None)];
        assert_eq!(filter(&elements, &WINDOW), vec![0]);
    }

    #[test]
    fn drops_offscreen_zero_size_and_outside_elements() {
        let mut offscreen = raw("Hidden", Role::Button, Rect::new(10, 10, 80, 24), None);
        offscreen.offscreen = true;
        let elements = vec![
            offscreen,
            raw("Zero", Role::Button, Rect::new(10, 10, 0, 24), None),
            raw("Tiny", Role::Button, Rect::new(10, 10, 1, 1), None),
            raw("Outside", Role::Button, Rect::new(2000, 10, 80, 24), None),
            raw("Edge", Role::Button, Rect::new(990, 790, 80, 24), None),
        ];
        assert_eq!(
            filter(&elements, &WINDOW),
            vec![4],
            "only the partly visible one"
        );
    }

    #[test]
    fn drops_nameless_and_decorative_elements_but_keeps_unnamed_inputs() {
        let elements = vec![
            raw("", Role::Pane, Rect::new(0, 0, 500, 500), None),
            raw("   \n\t", Role::Group, Rect::new(0, 0, 100, 100), None),
            raw(
                "Vertical",
                Role::ScrollBar,
                Rect::new(980, 0, 20, 800),
                None,
            ),
            raw("", Role::Separator, Rect::new(0, 50, 500, 2), None),
            raw("", Role::Edit, Rect::new(0, 60, 500, 300), None),
            raw("", Role::Document, Rect::new(0, 60, 500, 300), None),
        ];
        assert_eq!(filter(&elements, &WINDOW), vec![4, 5]);
    }

    #[test]
    fn drops_a_label_that_repeats_its_button() {
        let elements = vec![
            raw("Save", Role::Button, Rect::new(10, 10, 80, 24), None),
            raw("Save", Role::Text, Rect::new(14, 12, 40, 20), Some(0)),
            raw(
                "Saved 2 min ago",
                Role::Text,
                Rect::new(14, 12, 40, 20),
                Some(0),
            ),
        ];
        assert_eq!(filter(&elements, &WINDOW), vec![0, 2]);
    }

    #[test]
    fn drops_the_cells_of_a_list_row_but_keeps_its_links() {
        let row = Rect::new(10, 100, 700, 30);
        let elements = vec![
            raw("Report.pdf", Role::ListItem, row, None),
            raw("Name", Role::Edit, Rect::new(40, 104, 200, 22), Some(0)),
            raw("Size", Role::Text, Rect::new(600, 104, 80, 22), Some(0)),
            raw(
                "Open",
                Role::Hyperlink,
                Rect::new(300, 104, 60, 22),
                Some(0),
            ),
            raw("Search", Role::Edit, Rect::new(10, 10, 300, 30), None),
        ];
        assert_eq!(filter(&elements, &WINDOW), vec![0, 3, 4]);
    }

    #[test]
    fn drops_groups_inside_tabs() {
        let elements = vec![
            raw(
                "README.md",
                Role::TabItem,
                Rect::new(500, 44, 166, 41),
                None,
            ),
            raw(
                "C:\\repo\\README.md",
                Role::Group,
                Rect::new(508, 49, 118, 31),
                Some(0),
            ),
        ];
        assert_eq!(filter(&elements, &WINDOW), vec![0]);
    }

    #[test]
    fn icon_font_glyphs_do_not_count_as_names() {
        assert_eq!(clean_name("\u{EB51}"), "");
        assert_eq!(clean_name("\u{E713} Settings"), "Settings");
        let elements = vec![raw("\u{EA60}", Role::Text, Rect::new(10, 10, 20, 20), None)];
        assert!(filter(&elements, &WINDOW).is_empty());
    }

    #[test]
    fn keeps_a_label_when_its_ancestor_was_filtered_out() {
        let mut button = raw("Save", Role::Button, Rect::new(10, 10, 80, 24), None);
        button.offscreen = true;
        let elements = vec![
            button,
            raw("Save", Role::Text, Rect::new(14, 12, 40, 20), Some(0)),
        ];
        assert_eq!(filter(&elements, &WINDOW), vec![1]);
    }

    #[test]
    fn cap_prefers_interactive_elements_and_keeps_tree_order() {
        let elements = vec![
            raw("Title", Role::Text, Rect::new(0, 0, 50, 20), None),
            raw("Open", Role::Button, Rect::new(0, 30, 50, 20), None),
            raw("Status", Role::Text, Rect::new(0, 60, 50, 20), None),
            raw("Close", Role::Button, Rect::new(0, 90, 50, 20), None),
            raw("Footer", Role::Text, Rect::new(0, 120, 50, 20), None),
        ];
        assert_eq!(apply_cap(&elements, vec![0, 1, 2, 3, 4], 3), vec![0, 1, 3]);
        assert_eq!(apply_cap(&elements, vec![0, 1, 2], 10), vec![0, 1, 2]);
    }

    #[test]
    fn numbers_from_one_in_tree_order() {
        let mut save = raw(" Save\n", Role::Button, Rect::new(10, 10, 80, 24), None);
        save.automation_id = "SaveButton".into();
        let elements = vec![
            raw("File", Role::MenuItem, Rect::new(0, 0, 40, 20), None),
            save,
        ];
        let numbered = number(&elements, &[0, 1]);
        assert_eq!(numbered[0].id, 1);
        assert_eq!(numbered[0].automation_id, None);
        assert_eq!(numbered[1].id, 2);
        assert_eq!(numbered[1].name, "Save");
        assert_eq!(numbered[1].automation_id.as_deref(), Some("SaveButton"));
    }

    #[test]
    fn build_list_reports_cap_and_partial_flags() {
        let elements = (0..5)
            .map(|i| {
                raw(
                    &format!("B{i}"),
                    Role::Button,
                    Rect::new(0, i * 30, 50, 20),
                    None,
                )
            })
            .collect();
        let mut snap = snapshot(elements);
        snap.stop = Some(StopReason::TimeBudget);
        let list = build_list(snap, 3, Duration::from_millis(12));
        assert_eq!(list.elements.len(), 3);
        assert_eq!(list.candidates, 5);
        assert!(list.capped);
        assert!(list.partial);
        assert_eq!(list.elapsed_ms, 12);
        assert_eq!(list.get(3).map(|e| e.name.as_str()), Some("B2"));
        assert!(list.get(4).is_none());
    }

    #[test]
    fn renders_a_compact_prompt() {
        let mut list = build_list(
            snapshot(vec![
                raw("Save", Role::Button, Rect::new(10, 20, 80, 24), None),
                raw(
                    "Say \"hi\"",
                    Role::Hyperlink,
                    Rect::new(10, 50, 80, 24),
                    None,
                ),
                RawElement {
                    automation_id: "SearchBox".into(),
                    ..raw("", Role::Edit, Rect::new(10, 80, 200, 24), None)
                },
            ]),
            150,
            Duration::ZERO,
        );
        let expected = "UI elements of window \"Untitled - Notepad\" (number role \"name\" (x,y,width,height) in screen pixels)\n\
            1 button \"Save\" (10,20,80,24)\n\
            2 link \"Say 'hi'\" (10,50,80,24)\n\
            3 edit \"\" (10,80,200,24) [id SearchBox]\n";
        assert_eq!(render_for_prompt(&list), expected);

        list.capped = true;
        list.candidates = 9;
        list.partial = true;
        let header = render_for_prompt(&list).lines().next().unwrap().to_string();
        assert!(header.ends_with("; showing 3 of 9; list incomplete, the app was slow to answer"));
    }

    #[test]
    fn long_names_are_cut_for_the_prompt() {
        let element = Element {
            id: 7,
            role: Role::Text,
            name: "x".repeat(200),
            rect: Rect::new(-5, 0, 10, 10),
            automation_id: None,
        };
        let line = render_line(&element);
        assert!(line.starts_with("7 text \""));
        assert!(line.contains(&format!("{}…\"", "x".repeat(59))));
        assert!(line.ends_with("(-5,0,10,10)"));
    }

    #[test]
    fn serialises_roles_as_short_names() {
        let element = Element {
            id: 1,
            role: Role::TabItem,
            name: "Home".into(),
            rect: Rect::new(1, 2, 3, 4),
            automation_id: None,
        };
        assert_eq!(
            serde_json::to_string(&element).unwrap(),
            r#"{"id":1,"role":"tab","name":"Home","rect":{"x":1,"y":2,"width":3,"height":4}}"#
        );
    }
}
