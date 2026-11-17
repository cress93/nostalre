//! The frame every menu screen shares: the veil over the world, the top line, the commands on the right,
//! the help line and key hints at the bottom, and the pieces screens build with (tabs, rows, keys).
use super::{CMD_BASE_ID, CRUMB_ID, HINT_BASE_ID, PULL_ID, ROW_BASE_ID, TAB_BASE_ID};
use crate::hud::style;
use nostalre_ui::context::PhysicalKeyCode as KeyCode;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{Response, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;
use std::borrow::Cow;
use std::collections::HashSet;

/// The world's shade under the menu: the HUD's dark blue, nearly opaque
const VEIL: [f32; 4] = style::shade(0.84);
const TOP: f32 = 28.0;
const CMD_W: f32 = 300.0;
const CMD_GAP: f32 = 48.0;
const MAX_CMD_ROW: f32 = 34.0;
const SECTION_GAP: f32 = 20.0;
/// The text of a row, from its left: room for the caret
pub(super) const ROW_TEXT: f32 = 24.0;
/// A list row's height, the tabs' strip, an icon in a row, the gap between a screen's columns
pub(super) const ROW_H: f32 = 36.0;
pub(super) const TABS_H: f32 = 44.0;
pub(super) const ICON: f32 = 24.0;
pub(super) const COLUMN_GAP: f32 = 36.0;
/// An item not appraised yet, greyed
pub(super) const NOT_APPRAISED: [f32; 4] = [0.67, 0.67, 0.67, 1.0];
/// A handheld (Android): Escape is its Back button
pub(crate) const HANDHELD: bool = cfg!(target_os = "android");
const HINT_GAP: f32 = 18.0;
const CAP_GAP: f32 = 6.0;
const TAB_GAP: f32 = 26.0;

/// Where the frame's parts go on this screen
pub(super) struct Layout {
    margin: f32,
    rule_y: f32,
    cmd: Rect,
    footer_y: f32,
    /// What a screen may fill: left of the commands, between the top line and the footer
    pub content: Rect,
}

impl Layout {
    pub fn new(ui: &UiFrame) -> Self {
        let (w, h) = (ui.ctx.screen_width, ui.ctx.screen_height);
        let line = style::line(ui);
        let margin = (w * 0.05).clamp(16.0, 64.0).floor();
        let rule_y = TOP + line + 6.0;
        let footer_y = h - TOP - line - 14.0;
        let top = rule_y + SECTION_GAP;
        let cmd = Rect::new(w - margin - CMD_W, top, CMD_W, footer_y - SECTION_GAP - top);
        let content = Rect::new(margin, top, (cmd.x - CMD_GAP - margin).max(0.0), cmd.h);
        Self {
            margin,
            rule_y,
            cmd,
            footer_y,
            content,
        }
    }
}

pub(super) fn veil(ui: &mut UiFrame) {
    let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
    style::fill(ui, screen, VEIL);
}

/// "MENU", the screen's name after it, `info` on the right; true when "MENU" was clicked to go back
pub(super) fn top_line(ui: &mut UiFrame, l: &Layout, crumb: Option<&str>, info: &str) -> bool {
    let baseline = style::baseline(ui, TOP);
    let mut back = false;
    match crumb {
        Some(name) => {
            let menu_w = style::heading_width(ui, "Menu");
            let rect = Rect::new(l.margin, TOP, menu_w, style::line(ui));
            let resp = ui.interact(CRUMB_ID, rect);
            let color = if resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::heading_in(ui, l.margin, baseline, "Menu", color);
            let sep_x = l.margin + menu_w + 10.0;
            style::text(ui, sep_x, baseline, "»", style::DIM);
            style::heading(ui, sep_x + style::measure(ui, "»") + 10.0, baseline, name);
            back = resp.clicked();
        }
        None => style::heading(ui, l.margin, baseline, "Menu"),
    }
    let right = ui.ctx.screen_width - l.margin;
    style::text_right(ui, right, baseline, info, style::DIM);
    style::rule(ui, l.margin, l.rule_y, right - l.margin);
    back
}

/// The command column: `labels` (a name and its key), a gap before `first_system`; `picked` highlighted,
/// bold while its screen is open. Returns the one clicked
pub(super) fn commands(
    ui: &mut UiFrame,
    l: &Layout,
    labels: &[(&str, Option<&str>)],
    first_system: usize,
    picked: usize,
    screen_open: bool,
) -> Option<usize> {
    let line = style::line(ui);
    let gap = SECTION_GAP;
    let row_h = ((l.cmd.h - gap) / labels.len() as f32)
        .min(MAX_CMD_ROW)
        .floor();
    let mut y = l.cmd.y;
    let mut clicked = None;
    for (i, (label, key)) in labels.iter().enumerate() {
        if i == first_system {
            style::rule(ui, l.cmd.x, y + (gap * 0.5).floor(), l.cmd.w);
            y += gap;
        }
        let rect = Rect::new(l.cmd.x, y, l.cmd.w, row_h);
        let resp = ui.interact(WidgetId(CMD_BASE_ID + i as u32), rect);
        if i == picked {
            style::highlight(ui, rect);
        }
        let baseline = style::baseline(ui, y + ((row_h - line) / 2.0).floor());
        let x = rect.x + ROW_TEXT;
        if i == picked && screen_open {
            style::text_bold(ui, x, baseline, label, style::TEXT);
        } else {
            let color = if i == picked || resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::text(ui, x, baseline, label, color);
        }
        if let Some(key) = key {
            let w = ui.atlas.measure_text(key);
            style::outlined(ui, rect.x + rect.w - 8.0 - w, baseline, key, style::DIM);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        y += row_h;
    }
    clicked
}

/// The keys a screen reads, or the same press from a hint tapped on a touch screen
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Keys {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub enter: bool,
    pub back: bool,
    /// Delete or Backspace: take off, drop, the second action
    pub remove: bool,
    /// S: onto the skill bar
    pub to_bar: bool,
    pub next_tab: bool,
    pub prev_tab: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Enter,
    Back,
    Remove,
    ToBar,
}

impl Keys {
    /// The menu owns the keyboard while it is open, so it reads the keys as they came
    pub fn read(ui: &UiFrame) -> Self {
        let c = &ui.ctx;
        let pressed = |code| c.pressed_codes.contains(&code);
        Self {
            up: c.key_up,
            down: c.key_down,
            left: c.key_left,
            right: c.key_right,
            enter: c.key_enter,
            back: false,
            remove: c.key_delete || c.key_backspace,
            // not Alt+S, the skill tree's window key
            to_bar: pressed(KeyCode::KeyS) && !c.alt_pressed && !c.ctrl_pressed,
            next_tab: pressed(KeyCode::PageDown) || (c.key_tab && !c.shift_pressed),
            prev_tab: pressed(KeyCode::PageUp) || (c.key_tab && c.shift_pressed),
        }
    }

    pub fn press(&mut self, key: Key) {
        match key {
            Key::Enter => self.enter = true,
            Key::Back => self.back = true,
            Key::Remove => self.remove = true,
            Key::ToBar => self.to_bar = true,
        }
    }
}

/// The controller's name for a hint that is not a key
#[derive(Debug, Clone, Copy)]
enum PadCap {
    /// Named as it is: the D-pad
    Fixed(&'static str),
    /// The in-game L1 and R1, wherever the Controller page put them
    Shoulders,
}

/// A key and what it does, at the bottom right; one with a `key` can be tapped
#[derive(Debug, Clone, Copy)]
pub(super) struct Hint {
    pub cap: &'static str,
    pad: PadCap,
    pub label: &'static str,
    pub key: Option<Key>,
}

impl Hint {
    pub const MOVE: Hint = Hint::shown("Arrows", "D-pad", "choose");
    pub const TABS: Hint = Hint::shoulders("PgUp PgDn", "tab");
    pub const MOVE_CHANGE: Hint = Hint::shown("Arrows", "D-pad", "choose, change");
    pub const PAGES: Hint = Hint::shoulders("PgUp PgDn", "page");
    pub const OPEN: Hint = Hint::tap("Enter", "open", Key::Enter);
    pub const CLOSE: Hint = Hint::tap("Esc", "close", Key::Back);
    pub const BACK: Hint = Hint::tap("Esc", "back", Key::Back);
    pub const ENTER_YES: Hint = Hint::tap("Enter", "yes", Key::Enter);
    pub const ESC_NO: Hint = Hint::tap("Esc", "no", Key::Back);
    pub const TO_BAR: Hint = Hint::tap("S", "to bar", Key::ToBar);
    pub const CANCEL: Hint = Hint::tap("Esc", "cancel", Key::Back);

    const fn shown(cap: &'static str, pad: &'static str, label: &'static str) -> Self {
        Self {
            cap,
            pad: PadCap::Fixed(pad),
            label,
            key: None,
        }
    }

    const fn shoulders(cap: &'static str, label: &'static str) -> Self {
        Self {
            cap,
            pad: PadCap::Shoulders,
            label,
            key: None,
        }
    }

    pub const fn tap(cap: &'static str, label: &'static str, key: Key) -> Self {
        Self {
            cap,
            pad: PadCap::Fixed(""),
            label,
            key: Some(key),
        }
    }

    /// The cap as the player's hands know it: the controller's button after a pad press (`gamepad.rs`
    /// `menu_press`, by the physical button the Controller page put there), Android's Back for Escape
    fn cap(&self, ui: &UiFrame) -> Cow<'static, str> {
        let pad = &ui.ctx.pad_caps;
        match (ui.ctx.pad_labels, self.key) {
            (true, Some(Key::Enter)) => pad.a.into(),
            (true, Some(Key::Back)) => pad.b.into(),
            (true, Some(Key::Remove)) => pad.y.into(),
            (true, Some(Key::ToBar)) => pad.x.into(),
            (true, None) => match self.pad {
                PadCap::Fixed(name) => name.into(),
                PadCap::Shoulders => format!("{} {}", pad.l1, pad.r1).into(),
            },
            (false, Some(Key::Back)) => back_key().into(),
            _ => self.cap.into(),
        }
    }
}

/// What the device calls Escape
fn back_key() -> &'static str {
    if HANDHELD { "Back" } else { "Esc" }
}

/// The help line and the hints under the last rule; returns the key of a hint tapped
pub(super) fn footer(ui: &mut UiFrame, l: &Layout, help: &str, hints: &[Hint]) -> Option<Key> {
    let right = ui.ctx.screen_width - l.margin;
    style::rule(ui, l.margin, l.footer_y, right - l.margin);
    let top = l.footer_y + 14.0;
    let baseline = style::baseline(ui, top);
    let mut x = right;
    let mut tapped = None;
    for (i, hint) in hints.iter().enumerate().rev() {
        let label_w = style::measure(ui, hint.label);
        let cap = hint.cap(ui);
        let cap_w = style::key_cap_width(ui, &cap);
        let w = cap_w + CAP_GAP + label_w;
        x -= w;
        style::key_cap(ui, x, top, &cap);
        style::text(ui, x + cap_w + CAP_GAP, baseline, hint.label, style::DIM);
        if let Some(key) = hint.key {
            let rect = Rect::new(x, top, w, style::line(ui));
            if ui
                .interact(WidgetId(HINT_BASE_ID + i as u32), rect)
                .clicked()
            {
                tapped = Some(key);
            }
        }
        x -= HINT_GAP;
    }
    let help = style::fit(ui, help, (x - l.margin - HINT_GAP).max(0.0));
    style::text(ui, l.margin, baseline, &help, style::TEXT);
    tapped
}

/// A row of tabs at `top`, `active` underlined; returns the one clicked
pub(super) fn tabs(
    ui: &mut UiFrame,
    x: f32,
    top: f32,
    labels: &[&str],
    active: usize,
) -> Option<usize> {
    let line = style::line(ui);
    let baseline = style::baseline(ui, top);
    let mut tx = x;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let w = style::measure_bold(ui, label);
        let rect = Rect::new(tx, top, w, line + 6.0);
        let resp = ui.interact(WidgetId(TAB_BASE_ID + i as u32), rect);
        if i == active {
            style::text_bold(ui, tx, baseline, label, style::TEXT);
            style::underline_accent(ui, Rect::new(tx, top, w, line + 5.0));
        } else {
            let color = if resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::text(ui, tx, baseline, label, color);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        tx += w + TAB_GAP;
    }
    clicked
}

/// The tab after or before `active` for the tab keys, else the one clicked
pub(super) fn switch_tab(
    keys: &Keys,
    active: usize,
    count: usize,
    clicked: Option<usize>,
) -> usize {
    if count == 0 {
        return 0;
    }
    if let Some(i) = clicked {
        i
    } else if keys.next_tab {
        (active + 1) % count
    } else if keys.prev_tab {
        (active + count - 1) % count
    } else {
        active
    }
}

/// A list row, highlighted when picked
pub(super) fn row(ui: &mut UiFrame, id: WidgetId, rect: Rect, picked: bool) -> Response {
    let resp = ui.interact(id, rect);
    if picked {
        style::highlight(ui, rect);
    }
    resp
}

/// The baseline of one line of text in the middle of `rect`
pub(super) fn row_baseline(ui: &UiFrame, rect: Rect) -> f32 {
    style::baseline(ui, rect.y + ((rect.h - style::line(ui)) / 2.0).floor())
}

/// A list of names from `area`'s top, `w` wide, the arrows moving `picked`; the one Enter or a tap opens
pub(super) fn list(
    ui: &mut UiFrame,
    area: Rect,
    w: f32,
    names: &[&str],
    picked: &mut usize,
    keys: &Keys,
) -> Option<usize> {
    *picked = step(keys, *picked, names.len());
    let mut open = keys.enter.then_some(*picked);
    for (i, name) in names.iter().enumerate() {
        let rect = Rect::new(area.x, area.y + i as f32 * ROW_H, w.min(area.w), ROW_H);
        let resp = row(ui, WidgetId(ROW_BASE_ID + i as u32), rect, i == *picked);
        let baseline = row_baseline(ui, rect);
        style::text(ui, rect.x + ROW_TEXT, baseline, name, style::TEXT);
        if resp.clicked() {
            *picked = i;
            open = Some(i);
        }
    }
    open
}

/// A texture drawn over `r`, tinted
pub(super) fn image(ui: &mut UiFrame, r: Rect, path: String, tint: [f32; 4]) {
    let (v, i) = draw::quad_vertices(r.x, r.y, r.w, r.h, tint);
    ui.draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: TextureRef::Named(path.into()),
    });
}

/// The wheel over `area`: 1 a row down, -1 up, 0 for none
pub(super) fn wheel_step(ui: &mut UiFrame, area: Rect) -> isize {
    let wheel = ui.take_scroll(area);
    if wheel > 0.0 {
        -1
    } else if wheel < 0.0 {
        1
    } else {
        0
    }
}

/// How far a finger moves before its press counts as pulling the list rather than a tap
const PULL_THRESHOLD: f32 = 8.0;

/// A finger (or the mouse) held down on a list: where it landed, the list's first row and the pick then, and
/// whether it has moved far enough to be pulling the list
struct Pull {
    x: f32,
    y: f32,
    offset: usize,
    picked: usize,
    pulling: bool,
}

/// A touch screen has no wheel: a finger held on the list in `area` and moved up or down drags the rows along,
/// `row_h` apart. The press that started it already picked the row it landed on, so on the frame it turns into
/// a pull this returns the pick from before, to put back. The gesture belongs to the area it landed in, so a
/// screen with two scrolled areas (a list and a description) moves only the one under the finger. A list that
/// fits (`max` 0) is never pulled: a finger sliding over it stays a tap, its pick kept
fn pull(
    ui: &mut UiFrame,
    area: Rect,
    offset: &mut usize,
    max: usize,
    row_h: f32,
    picked: usize,
) -> Option<usize> {
    let (x, y) = (ui.ctx.mouse_x, ui.ctx.mouse_y);
    let (pressed, down) = (ui.ctx.mouse_pressed, ui.ctx.mouse_down);
    if pressed && down && area.contains(x, y) && ui.pointer_available() {
        let start = Pull {
            x,
            y,
            offset: *offset,
            picked,
            pulling: false,
        };
        ui.state.set(PULL_ID, Some(start));
        return None;
    }
    let state = ui.state.get_or_default::<Option<Pull>>(PULL_ID);
    let pull = state.as_mut().filter(|p| area.contains(p.x, p.y))?;
    // lifted, a new press somewhere else, or nothing to scroll: this gesture is over
    if !down || pressed || row_h <= 0.0 || max == 0 {
        *state = None;
        return None;
    }
    let dy = pull.y - y;
    let mut put_back = None;
    if !pull.pulling && dy.abs() >= PULL_THRESHOLD {
        pull.pulling = true;
        put_back = Some(pull.picked);
    }
    if pull.pulling {
        let rows = (dy / row_h).round() as isize;
        *offset = (pull.offset as isize + rows).max(0) as usize;
    }
    put_back
}

/// A block of wrapped text the wheel scrolls; wrapped once per text (`key`) and width, not every frame
#[derive(Default)]
pub(super) struct ScrolledText {
    key: Option<(u64, u32)>,
    rows: Vec<String>,
    offset: usize,
}

impl ScrolledText {
    /// Draws the rows of `text` that fit `area`, the last one cut with "..." while more follow; `text` is only
    /// built when `key` or the width changes
    pub fn build(&mut self, ui: &mut UiFrame, key: u64, area: Rect, text: impl FnOnce() -> String) {
        let width = area.w.round() as u32;
        if self.key != Some((key, width)) {
            self.key = Some((key, width));
            self.rows = style::wrap(ui, &text(), area.w);
            self.offset = 0;
        }
        let line = style::line(ui);
        let visible = (area.h / line).floor().max(0.0) as usize;
        let max = self.rows.len().saturating_sub(visible);
        pull(ui, area, &mut self.offset, max, line, 0);
        let moved = self.offset as isize + wheel_step(ui, area);
        self.offset = (moved.max(0) as usize).min(max);
        let more = self.offset < max;
        for (i, row) in self.rows.iter().skip(self.offset).take(visible).enumerate() {
            let baseline = style::baseline(ui, area.y + i as f32 * line);
            if more && i + 1 == visible {
                let cut = format!("{}...", row.trim_end());
                style::text(ui, area.x, baseline, &cut, style::DIM);
            } else {
                style::text(ui, area.x, baseline, row, style::DIM);
            }
        }
    }
}

/// The first row a list of `len` shows, `visible` at a time and `row_h` apart: the wheel or a finger pulling
/// (see [`pull`]) over `area` moves it, and after a key moved `picked` (`follow`) it keeps that row in sight.
/// Returns the pick to put back when a press on a row turns out to be a pull
#[allow(clippy::too_many_arguments)]
pub(super) fn scroll(
    ui: &mut UiFrame,
    area: Rect,
    offset: &mut usize,
    picked: usize,
    len: usize,
    visible: usize,
    row_h: f32,
    follow: bool,
) -> Option<usize> {
    let max = len.saturating_sub(visible);
    let put_back = pull(ui, area, offset, max, row_h, picked);
    *offset = (*offset as isize + wheel_step(ui, area)).max(0) as usize;
    if follow && picked < *offset {
        *offset = picked;
    } else if follow && visible > 0 && picked >= *offset + visible {
        *offset = picked + 1 - visible;
    }
    *offset = (*offset).min(max);
    put_back
}

/// The picked index after the up and down keys, wrapping
pub(super) fn step(keys: &Keys, picked: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let picked = picked.min(len - 1);
    if keys.up {
        picked.checked_sub(1).unwrap_or(len - 1)
    } else if keys.down {
        (picked + 1) % len
    } else {
        picked
    }
}

/// Art a screen draws that may not be loaded yet (an item's picture): asked for once, loaded by the client
#[derive(Default)]
pub(super) struct Textures {
    asked: HashSet<String>,
    wanted: Vec<String>,
}

impl Textures {
    pub fn want(&mut self, path: &str) {
        if !self.asked.contains(path) {
            self.asked.insert(path.to_string());
            self.wanted.push(path.to_string());
        }
    }

    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.wanted)
    }
}

/// A section heading with its rule, `w` wide; returns the y under it
pub(super) fn section(ui: &mut UiFrame, x: f32, top: f32, w: f32, heading: &str) -> f32 {
    style::heading_rule(ui, x, top, w, heading);
    top + style::line(ui) + 8.0
}

#[cfg(test)]
mod tests {
    use super::{ROW_H, scroll};
    use nostalre_ui::context::UiContext;
    use nostalre_ui::rect::Rect;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    const AREA: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 200.0,
        h: ROW_H * 5.0,
    };

    /// One frame of a 20-row list showing 5 with the pointer at `y`: `pressed` this frame, `down` held
    fn frame(
        state: &mut StateCache,
        offset: &mut usize,
        y: f32,
        pressed: bool,
        down: bool,
    ) -> Option<usize> {
        frame_of(state, offset, 20, y, pressed, down)
    }

    /// The same for a list of `len` rows
    fn frame_of(
        state: &mut StateCache,
        offset: &mut usize,
        len: usize,
        y: f32,
        pressed: bool,
        down: bool,
    ) -> Option<usize> {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 50.0;
        ctx.mouse_y = y;
        ctx.mouse_pressed = pressed;
        ctx.mouse_clicked = pressed;
        ctx.mouse_down = down;
        let mut ui = test_frame(&mut ctx, state);
        scroll(&mut ui, AREA, offset, 3, len, 5, ROW_H, false)
    }

    #[test]
    fn a_finger_pulls_the_list_row_by_row_and_puts_the_pick_back() {
        let mut state = StateCache::new();
        let mut offset = 2;
        assert_eq!(frame(&mut state, &mut offset, 150.0, true, true), None);
        // under the threshold it is still a tap
        assert_eq!(frame(&mut state, &mut offset, 146.0, false, true), None);
        assert_eq!(offset, 2);
        // pulled up two rows: the list moves down two, and the pick from before the press comes back once
        assert_eq!(
            frame(&mut state, &mut offset, 150.0 - 2.0 * ROW_H, false, true),
            Some(3)
        );
        assert_eq!(offset, 4);
        assert_eq!(
            frame(&mut state, &mut offset, 150.0 - 3.0 * ROW_H, false, true),
            None
        );
        assert_eq!(offset, 5);
        // past the end it stops at the last page
        frame(&mut state, &mut offset, 150.0 - 40.0 * ROW_H, false, true);
        assert_eq!(offset, 15);
        // lifted: the list stays where it was left
        frame(&mut state, &mut offset, 20.0, false, false);
        frame(&mut state, &mut offset, 150.0, false, false);
        assert_eq!(offset, 15);
    }

    #[test]
    fn a_press_outside_the_list_does_not_pull_it() {
        let mut state = StateCache::new();
        let mut offset = 0;
        frame(&mut state, &mut offset, 400.0, true, true);
        frame(&mut state, &mut offset, 50.0, false, true);
        assert_eq!(offset, 0);
    }

    #[test]
    fn a_list_that_fits_is_not_pulled_and_keeps_the_tapped_pick() {
        let mut state = StateCache::new();
        let mut offset = 0;
        assert_eq!(
            frame_of(&mut state, &mut offset, 4, 150.0, true, true),
            None
        );
        assert_eq!(
            frame_of(&mut state, &mut offset, 4, 150.0 - 3.0 * ROW_H, false, true),
            None
        );
        assert_eq!(offset, 0);
    }
}
