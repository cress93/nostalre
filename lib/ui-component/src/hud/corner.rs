//! Top right: ping, the round minimap, where you are, the route the navigation follows, the quests you follow.
use super::{HudInput, NavTap};
use super::style;
use crate::BuildCtx;
use crate::helper::format::format_thousands;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const RIGHT: f32 = 16.0;
const TOP: f32 = 14.0;
const MAP: f32 = 190.0;
const QUESTS_W: f32 = 270.0;
const MAX_QUESTS: usize = 3;
const QUEST_GAP: f32 = 10.0;
const OBJECTIVE_INDENT: f32 = 16.0;
const DOT: f32 = 6.0;
const MENU_BTN_ID: WidgetId = WidgetId(7000);
const NAV_WALK_ID: WidgetId = WidgetId(7001);
const NAV_END_ID: WidgetId = WidgetId(7002);

/// Draws the corner and returns the minimap's square, and a tap on the navigation's buttons
pub fn build(ui: &mut UiFrame, ctx: &BuildCtx, input: &HudInput) -> (Rect, Option<NavTap>) {
    let line = style::line(ui);
    let right = ui.ctx.screen_width - RIGHT;
    let mut y = TOP;
    if let Some(ms) = input.ping_ms {
        style::text_right(
            ui,
            right,
            style::baseline(ui, y),
            &format!("ping {ms} ms"),
            style::DIM,
        );
    }
    y += line + 4.0;

    let map = Rect::new(right - MAP, y, MAP, MAP);
    y += MAP + 4.0;
    if let Some(place) = &input.place {
        let name = super::place_name(ctx.data, &place.map);
        let coords = format!("{}, {}", place.x, place.y);
        let baseline = style::baseline(ui, y);
        let coords_w = style::measure(ui, &coords);
        style::text_right(ui, right, baseline, &coords, style::DIM);
        let name = style::fit(ui, name, MAP.max(QUESTS_W) - coords_w - 8.0);
        style::text_right(ui, right - coords_w - 8.0, baseline, &name, style::TEXT);
        y += line;
    }

    let mut tap = None;
    if let Some(nav) = &input.navigation {
        y += 16.0;
        let left = right - QUESTS_W;
        style::heading_rule(ui, left, y, QUESTS_W, "Navigation");
        y += line + 6.0;
        let dest = style::fit(ui, &nav.dest, QUESTS_W);
        style::text_bold(ui, left, style::baseline(ui, y), &dest, style::ACCENT);
        y += line + 2.0;
        for row in style::wrap(ui, &nav.step, QUESTS_W).iter().take(2) {
            style::text(ui, left, style::baseline(ui, y), row, style::DIM);
            y += line;
        }
        // Walk (or Stop) and End, pills a finger can hit
        y += 4.0;
        let h = line + 10.0;
        let walk = if nav.walking { "Stop" } else { "Walk there" };
        let mut x = left;
        for (id, label, what) in [
            (NAV_WALK_ID, walk, NavTap::Walk(!nav.walking)),
            (NAV_END_ID, "End", NavTap::End),
        ] {
            let w = style::measure(ui, label) + 28.0;
            let r = Rect::new(x, y, w, h);
            let resp = ui.interact(id, r);
            style::pill_button(ui, r);
            let color = if resp.hovered() { style::ACCENT } else { style::TEXT };
            style::text_centered(ui, r.x + r.w / 2.0, style::baseline(ui, y + 5.0), label, color);
            if resp.clicked() {
                tap = Some(what);
            }
            x += w + 10.0;
        }
        y += h + 4.0;
    }

    let active: Vec<_> = ctx.quest_log.quests.iter().filter(|q| q.active).collect();
    if active.is_empty() {
        return (map, tap);
    }
    y += 16.0;
    let left = right - QUESTS_W;
    style::heading_rule(ui, left, y, QUESTS_W, "Quests");
    y += line + 6.0;
    for quest in active.iter().take(MAX_QUESTS) {
        let title = ctx
            .data
            .quest_display
            .as_ref()
            .map(|t| t.title(quest.id))
            .unwrap_or_else(|| format!("Quest {}", quest.id));
        let title = style::fit(ui, &title, QUESTS_W);
        style::text_bold(ui, left, style::baseline(ui, y), &title, style::ACCENT);
        y += line + 2.0;
        for o in &quest.objectives {
            let done = o.required > 0 && o.current >= o.required;
            let count = if o.required > 0 {
                format!("{} / {}", o.current, o.required)
            } else {
                o.current.to_string()
            };
            let baseline = style::baseline(ui, y);
            let dot_y = y + ((line - DOT) / 2.0).floor();
            style::dot(ui, left + OBJECTIVE_INDENT - DOT - 6.0, dot_y, DOT, done);
            let count_w = style::measure(ui, &count);
            let (name_color, count_color) = if done {
                (style::DIM_MORE, style::GOOD)
            } else {
                (style::DIM, style::TEXT)
            };
            style::text_right(ui, right, baseline, &count, count_color);
            let name_w = QUESTS_W - OBJECTIVE_INDENT - count_w - 10.0;
            let name = style::fit(ui, &o.name, name_w);
            style::text(ui, left + OBJECTIVE_INDENT, baseline, &name, name_color);
            y += line;
        }
        y += QUEST_GAP;
    }
    (map, tap)
}

const BOTTOM: f32 = 10.0;

/// Bottom right: zeny, weight, slots and the menu's key, which a tap opens too; true when tapped
pub fn wallet(ui: &mut UiFrame, ctx: &BuildCtx) -> bool {
    let inv = &ctx.character.inventory;
    let line = style::line(ui);
    // a phone's rounded corner would cut into the line
    let pad = if crate::menu::HANDHELD { 45.0 } else { 0.0 };
    let right = ui.ctx.screen_width - RIGHT - pad;
    let mut y = ui.ctx.screen_height - BOTTOM - line;
    // a handheld's Back button opens it beside the pad
    let menu_line = if ui.ctx.pad_labels || crate::menu::HANDHELD {
        "Back  Menu"
    } else {
        "Esc  Menu"
    };
    let menu_w = style::measure(ui, menu_line);
    let menu = ui.interact(MENU_BTN_ID, Rect::new(right - menu_w, y, menu_w, line));
    let color = if menu.hovered() {
        style::TEXT
    } else {
        style::DIM
    };
    style::text_right(ui, right, style::baseline(ui, y), menu_line, color);
    y -= line + 4.0;
    let percent = if inv.max_weight > 0 {
        inv.weight * 100 / inv.max_weight
    } else {
        0
    };
    let weight = format!(
        "Weight {percent}% · {} / {}",
        inv.all_items().len(),
        inv.slots
    );
    let color = if inv.is_overweight() {
        style::WARN
    } else {
        style::DIM
    };
    style::text_right(ui, right, style::baseline(ui, y), &weight, color);
    y -= line;
    let zeny = format!("{} z", format_thousands(i64::from(inv.zeny)));
    style::text_right(ui, right, style::baseline(ui, y), &zeny, style::TEXT);
    menu.clicked()
}
