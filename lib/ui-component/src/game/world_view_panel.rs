//! The world map's newer view (`nostalre_game::world_view`): a tab for each world, its picture with every field's
//! suggested level and every dungeon's marker with its level range on it, and a panel for the map or dungeon
//! tapped: its level, its floors (a dungeon), its monsters (`nostalre_game::map_monster_tables`).
use std::collections::HashMap;

use crate::helper::window_chrome::fill;
use nostalre_game::map_monster_tables::{map_monsters, monster_info};
use nostalre_game::world_view::World;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The world tabs 5810 to 5829, the panel's floors 5830 to 5869, the picture 5870
const TAB_BASE_ID: u32 = 5810;
const FLOOR_BASE_ID: u32 = 5830;
const VIEW_ID: WidgetId = WidgetId(5870);
const NAVIGATE_ID: WidgetId = WidgetId(5871);
pub const TABS_H: f32 = 20.0;
const PANEL_W: f32 = 250.0;
const LINE: f32 = 16.0;
const MAX_MONSTERS: usize = 12;

const LABEL: [f32; 4] = [1.0, 1.0, 0.85, 1.0];
const SHADE: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const DUNGEON: [f32; 4] = [0.75, 0.2, 0.2, 0.85];
const CURRENT: [f32; 4] = [1.0, 0.85, 0.2, 1.0];
const PANEL_BG: [f32; 4] = [0.04, 0.04, 0.06, 0.92];
const DIM: [f32; 4] = [0.7, 0.7, 0.75, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BOSS: [f32; 4] = [1.0, 0.5, 0.4, 1.0];

#[derive(Clone, PartialEq, Debug)]
enum Picked {
    Field(String),
    /// A dungeon's marker, and the floor picked in its panel
    Dungeon(usize, usize),
}

#[derive(Default)]
pub struct WorldView {
    world: Option<usize>,
    picked: Option<Picked>,
    sizes: HashMap<String, (f32, f32)>,
    /// The map whose minimap the panel wants shown
    wanted: Option<String>,
    /// The map the panel's Navigate was tapped for
    navigate: Option<String>,
}

impl WorldView {
    pub fn image_sized(&mut self, path: &str, w: u32, h: u32) {
        self.sizes.insert(path.to_string(), (w as f32, h as f32));
    }

    /// The map whose minimap the panel showed (or wanted to) this frame
    pub fn take_wanted(&mut self) -> Option<String> {
        self.wanted.take()
    }

    /// The map to navigate to, once its panel's Navigate was tapped
    pub fn take_navigate(&mut self) -> Option<String> {
        self.navigate.take()
    }

    pub fn reset(&mut self) {
        self.world = None;
        self.picked = None;
    }

    /// The world shown: the one picked, else the one the player is on, else the first
    pub fn world(&self, worlds: &[World], current: Option<&str>) -> usize {
        self.world
            .or_else(|| current.and_then(|m| worlds.iter().position(|w| w.has_map(m))))
            .unwrap_or(0)
            .min(worlds.len().saturating_sub(1))
    }

    /// The picture's size, until it is known kRO's 1280 by 1024
    pub fn size_of(&self, world: &World) -> (f32, f32) {
        self.sizes
            .get(&world.image)
            .copied()
            .unwrap_or((1280.0, 1024.0))
    }

    /// The tabs over the picture, `w` wide from `x`
    pub fn tabs(
        &mut self,
        ui: &mut UiFrame,
        worlds: &[World],
        shown: usize,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let tab_w = (w / worlds.len().max(1) as f32).floor();
        for (i, world) in worlds.iter().enumerate() {
            let r = Rect::new(x + i as f32 * tab_w, y, tab_w, TABS_H);
            let resp = ui.interact(WidgetId(TAB_BASE_ID + i as u32), r);
            let bg = if i == shown {
                [0.25, 0.3, 0.45, 1.0]
            } else if resp.hovered() {
                [0.18, 0.2, 0.28, 1.0]
            } else {
                [0.1, 0.11, 0.15, 1.0]
            };
            fill(ui, r.x, r.y, r.w - 1.0, r.h, bg);
            let name = fit(ui, &world.name, tab_w - 6.0);
            ui.text_centered(r.x, r.y + TABS_H - 5.0, r.w, &name, WHITE);
            if resp.clicked() && i != shown {
                self.world = Some(i);
                self.picked = None;
            }
        }
    }

    /// The picture, its labels and the panel, in `area`; `current` the player's map, `name` a map's display name
    #[allow(clippy::too_many_arguments)]
    pub fn view(
        &mut self,
        ui: &mut UiFrame,
        world: &World,
        area: Rect,
        image_loaded: bool,
        current: Option<&str>,
        player: Option<(f32, f32, i32, i32)>,
        name: &dyn Fn(&str) -> String,
        preview: &dyn Fn(&str) -> Option<String>,
    ) {
        let (iw, ih) = self.size_of(world);
        let scale = (area.w / iw).min(area.h / ih);
        let (vw, vh) = ((iw * scale).floor(), (ih * scale).floor());
        let (ox, oy) = (area.x + ((area.w - vw) / 2.0).floor(), area.y);
        fill(ui, area.x, area.y, area.w, area.h, [0.05, 0.06, 0.09, 1.0]);
        if image_loaded {
            let (v, i) = draw::quad_vertices_uv(ox, oy, vw, vh, [0.0, 0.0], [1.0, 1.0], WHITE);
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::Named(world.image.clone().into()),
            });
        }
        let to_screen = |(l, t, r, b): (u16, u16, u16, u16)| {
            Rect::new(
                ox + l as f32 * scale,
                oy + t as f32 * scale,
                ((r - l) as f32 * scale).max(2.0),
                ((b - t) as f32 * scale).max(2.0),
            )
        };
        let current_key = current.map(nostalre_game::map_key);

        // every field's level, and where the player is
        for field in &world.fields {
            let r = to_screen(field.rect);
            if current_key.as_deref() == Some(field.map.as_str()) {
                outline(ui, r, CURRENT);
                if let Some((x, y, w, h)) = player {
                    let px = r.x + (x / w.max(1) as f32).clamp(0.0, 1.0) * r.w;
                    let py = r.y + (1.0 - (y / h.max(1) as f32).clamp(0.0, 1.0)) * r.h;
                    fill(ui, px - 3.0, py - 3.0, 6.0, 6.0, CURRENT);
                }
            }
            if !field.level.is_empty() && r.w >= 24.0 {
                let label = format!("Lv {}", field.level);
                let w = ui.atlas.measure_text(&label) + 4.0;
                let (lx, ly) = (
                    r.x + ((r.w - w) / 2.0).floor(),
                    r.y + ((r.h - LINE) / 2.0).floor(),
                );
                fill(ui, lx, ly, w, LINE - 2.0, SHADE);
                ui.text(lx + 2.0, ly + LINE - 5.0, &label, LABEL);
            }
        }
        // the dungeons' markers and level ranges
        for (i, dungeon) in world.dungeons.iter().enumerate() {
            let r = to_screen(dungeon.rect);
            let here = current_key
                .as_deref()
                .is_some_and(|c| dungeon.floors.iter().any(|(m, _)| m == c));
            fill(ui, r.x, r.y, r.w, r.h, if here { CURRENT } else { DUNGEON });
            outline(ui, r, WHITE);
            if !dungeon.level.is_empty() {
                let label = dungeon.level.replace('~', "-");
                let w = ui.atlas.measure_text(&label) + 4.0;
                fill(ui, r.x + r.w + 2.0, r.y, w, LINE - 2.0, SHADE);
                ui.text(r.x + r.w + 4.0, r.y + LINE - 5.0, &label, LABEL);
            }
            if matches!(self.picked, Some(Picked::Dungeon(d, _)) if d == i) {
                outline(
                    ui,
                    Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0),
                    CURRENT,
                );
            }
        }

        // hover and tap: a dungeon's marker first, it sits over fields
        let (mx, my) = (ui.ctx.mouse_x, ui.ctx.mouse_y);
        let resp = ui.interact(VIEW_ID, Rect::new(ox, oy, vw, vh));
        // the panel's own: a tap on it is not for the map under it
        let panel = Rect::new(ox + vw - PANEL_W - 6.0, oy + 6.0, PANEL_W, vh - 12.0);
        let on_panel = self.picked.is_some() && panel.contains(mx, my);
        if resp.hovered() && !on_panel {
            let (px, py) = (((mx - ox) / scale) as u16, ((my - oy) / scale) as u16);
            let hit = world
                .dungeon_at(px, py)
                .map(|d| Picked::Dungeon(d, 0))
                .or_else(|| world.field_at(px, py).map(|f| Picked::Field(f.map.clone())));
            if let Some(hit) = &hit {
                let tip = match hit {
                    Picked::Field(map) => {
                        let f = world.fields.iter().find(|f| &f.map == map);
                        let lv = f
                            .filter(|f| !f.level.is_empty())
                            .map(|f| format!(" · Lv {}", f.level));
                        format!("{}{}", name(map), lv.unwrap_or_default())
                    }
                    Picked::Dungeon(d, _) => {
                        let dungeon = &world.dungeons[*d];
                        format!(
                            "{} · Lv {}",
                            dungeon_name(dungeon.floors.first().map_or("", |f| &f.0), name),
                            dungeon.level
                        )
                    }
                };
                ui.tooltip(mx, my, &tip);
            }
            if resp.clicked() {
                self.picked = hit;
            }
        }

        if let Some(picked) = self.picked.clone() {
            self.panel(
                ui,
                world,
                picked,
                panel,
                current,
                name,
                preview,
            );
        }
    }

    /// The picked map's or dungeon's panel: its level, a dungeon's floors, the monsters
    fn panel(
        &mut self,
        ui: &mut UiFrame,
        world: &World,
        picked: Picked,
        r: Rect,
        current: Option<&str>,
        name: &dyn Fn(&str) -> String,
        preview: &dyn Fn(&str) -> Option<String>,
    ) {
        fill(ui, r.x, r.y, r.w, r.h, PANEL_BG);
        outline(ui, r, [0.55, 0.5, 0.4, 1.0]);
        let x = r.x + 8.0;
        let w = r.w - 16.0;
        let mut y = r.y + 6.0;
        let map = match &picked {
            Picked::Field(map) => {
                let level = world
                    .fields
                    .iter()
                    .find(|f| &f.map == map)
                    .map(|f| f.level.clone())
                    .unwrap_or_default();
                ui.text(x, y + LINE - 4.0, &fit(ui, &name(map), w), WHITE);
                y += LINE;
                if !level.is_empty() {
                    ui.text(x, y + LINE - 4.0, &format!("Suggested level {level}"), DIM);
                    y += LINE;
                }
                map.clone()
            }
            Picked::Dungeon(d, floor) => {
                let dungeon = &world.dungeons[*d];
                let title = dungeon_name(dungeon.floors.first().map_or("", |f| &f.0), name);
                ui.text(x, y + LINE - 4.0, &fit(ui, &title, w), WHITE);
                y += LINE;
                ui.text(
                    x,
                    y + LINE - 4.0,
                    &format!("Levels {}", dungeon.level.replace('~', "-")),
                    DIM,
                );
                y += LINE + 4.0;
                // the floor shown, ◀ ▶ step through them
                let count = dungeon.floors.len();
                let arrow = LINE + 6.0;
                let back = Rect::new(x - 2.0, y, arrow, LINE + 2.0);
                let next = Rect::new(x + w - arrow + 2.0, y, arrow, LINE + 2.0);
                let mut step = 0isize;
                for (k, (rect, glyph, by)) in [(back, "<", -1isize), (next, ">", 1)]
                    .into_iter()
                    .enumerate()
                {
                    let resp = ui.interact(WidgetId(FLOOR_BASE_ID + k as u32), rect);
                    let bg = if resp.hovered() {
                        [0.3, 0.35, 0.5, 1.0]
                    } else {
                        [0.18, 0.2, 0.28, 1.0]
                    };
                    fill(ui, rect.x, rect.y, rect.w, rect.h, bg);
                    ui.text_centered(rect.x, rect.y + LINE - 3.0, rect.w, glyph, WHITE);
                    if resp.clicked() {
                        step = by;
                    }
                }
                let floor = (*floor as isize + step).rem_euclid(count.max(1) as isize) as usize;
                if step != 0 {
                    self.picked = Some(Picked::Dungeon(*d, floor));
                }
                if let Some((map, level)) = dungeon.floors.get(floor) {
                    let label = if level.is_empty() {
                        format!("{} ({}/{count})", name(map), floor + 1)
                    } else {
                        format!("{} · Lv {level} ({}/{count})", name(map), floor + 1)
                    };
                    let inner = w - 2.0 * arrow;
                    ui.text_centered(
                        x + arrow,
                        y + LINE - 3.0,
                        inner,
                        &fit(ui, &label, inner - 4.0),
                        WHITE,
                    );
                }
                y += LINE + 6.0;
                dungeon
                    .floors
                    .get(floor)
                    .map(|f| f.0.clone())
                    .unwrap_or_default()
            }
        };
        // the way there, from wherever the player is
        let button = Rect::new(x, y + 2.0, w, LINE + 6.0);
        let resp = ui.interact(NAVIGATE_ID, button);
        let here = current.is_some_and(|c| nostalre_game::same_map(c, &map));
        let bg = if here {
            [0.12, 0.13, 0.16, 1.0]
        } else if resp.hovered() {
            [0.3, 0.45, 0.35, 1.0]
        } else {
            [0.18, 0.3, 0.22, 1.0]
        };
        fill(ui, button.x, button.y, button.w, button.h, bg);
        let label = if here { "You are here" } else { "Navigate" };
        ui.text_centered(button.x, button.y + LINE, button.w, label, if here { DIM } else { WHITE });
        if resp.clicked() && !here {
            self.navigate = Some(map.clone());
        }
        y += LINE + 10.0;
        // the map's own minimap, under the information; the monsters below it
        let side = w.min(170.0).min((r.y + r.h - y - 4.0 * LINE).max(64.0));
        self.wanted = Some(map.clone());
        if let Some(path) = preview(&map) {
            let at = Rect::new(x + ((w - side) / 2.0).floor(), y + 4.0, side, side);
            let (v, i) = draw::quad_vertices(at.x, at.y, at.w, at.h, WHITE);
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::Named(path.into()),
            });
            outline(ui, at, [0.55, 0.5, 0.4, 1.0]);
        } else {
            ui.text_centered(x, y + side / 2.0, w, "no map picture", DIM);
        }
        y += side + 8.0;
        let bottom = r.y + r.h - 4.0;
        // the monsters
        y += 4.0;
        let monsters = map_monsters(&map);
        if monsters.is_empty() {
            ui.text(x, y + LINE - 4.0, "No monsters", DIM);
            return;
        }
        ui.text(x, y + LINE - 4.0, "Monsters", WHITE);
        y += LINE;
        let top = y;
        for &(id, count) in monsters.iter().rev().take(MAX_MONSTERS) {
            if y + 2.0 * LINE > bottom {
                break;
            }
            let Some((mob, level, boss)) = monster_info(id) else {
                continue;
            };
            let right = format!("Lv {level}  ×{count}");
            let rw = ui.atlas.measure_text(&right);
            let label = fit(ui, mob, w - rw - 8.0);
            ui.text(x, y + LINE - 4.0, &label, if boss { BOSS } else { WHITE });
            ui.text(x + w - rw, y + LINE - 4.0, &right, DIM);
            y += LINE;
        }
        let shown = monsters
            .len()
            .min(MAX_MONSTERS)
            .min(((y - top) / LINE) as usize);
        if monsters.len() > shown {
            ui.text(
                x,
                y + LINE - 4.0,
                &format!("and {} more", monsters.len() - shown),
                DIM,
            );
        }
    }
}

/// A dungeon's name: its first floor's, without the floor ("Payon Cave F1" is Payon Cave)
fn dungeon_name(first_floor: &str, name: &dyn Fn(&str) -> String) -> String {
    let full = name(first_floor);
    let words: Vec<&str> = full.split_whitespace().collect();
    match words.split_last() {
        Some((last, rest))
            if !rest.is_empty()
                && (last.starts_with(['F', 'B'])
                    && last[1..].chars().all(|c| c.is_ascii_digit())
                    || last.ends_with('F')
                        && last[..last.len() - 1].chars().all(|c| c.is_ascii_digit())) =>
        {
            rest.join(" ")
        }
        _ => full,
    }
}

fn fit(ui: &UiFrame, text: &str, w: f32) -> String {
    if ui.atlas.measure_text(text) <= w {
        return text.to_string();
    }
    let mut out: String = text.to_string();
    while !out.is_empty() && ui.atlas.measure_text(&format!("{out}…")) > w {
        out.pop();
    }
    format!("{out}…")
}

fn outline(ui: &mut UiFrame, r: Rect, color: [f32; 4]) {
    fill(ui, r.x, r.y, r.w, 1.0, color);
    fill(ui, r.x, r.y + r.h - 1.0, r.w, 1.0, color);
    fill(ui, r.x, r.y, 1.0, r.h, color);
    fill(ui, r.x + r.w - 1.0, r.y, 1.0, r.h, color);
}

#[cfg(test)]
mod tests {
    use super::dungeon_name;

    #[test]
    fn a_dungeon_is_named_without_its_floor() {
        let name = |m: &str| m.to_string();
        assert_eq!(dungeon_name("Payon Cave F1", &name), "Payon Cave");
        assert_eq!(dungeon_name("Clock Tower B2", &name), "Clock Tower");
        assert_eq!(dungeon_name("Bifrost Tower 1F", &name), "Bifrost Tower");
        assert_eq!(dungeon_name("Sunken Ship", &name), "Sunken Ship");
    }
}
