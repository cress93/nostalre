//! Monsters: the three capture slots (nostalRE's capture balls, `nostalre_game::capture`), each with the caught
//! monster's portrait; the picked one's stats, Call (or Recall while it fights) and Release.
use std::collections::HashMap;

use super::frame::{self, COLUMN_GAP, Hint, Key, Keys, Textures};
use super::{ACTION_BASE_ID, CARD_BASE_ID};
use crate::BuildCtx;
use crate::hud::style;
use nostalre_game::capture::{CaptureSlots, CapturedMonster, SLOTS, portrait_key};
use nostalre_game::event::GameEvent;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const CARD_W: f32 = 220.0;
const CARD_H: f32 = 200.0;
const PORTRAIT_H: f32 = 120.0;
const ROW_H: f32 = 30.0;
const BUTTON_W: f32 = 150.0;
const BUTTON_H: f32 = 40.0;
const STATS: [&str; 9] = [
    "Level", "EXP", "HP", "ATK", "DEF", "MDEF", "Race", "Element", "Size",
];

#[derive(Default)]
pub(super) struct Monsters {
    picked: usize,
    /// Release was pressed on this slot: a second press lets it go
    releasing: Option<usize>,
    /// The server was asked for the slots since the screen opened
    asked: bool,
    /// The portraits' sizes, as the client built them
    portraits: HashMap<String, (u32, u32)>,
}

/// What the screen does with a slot: call it out (or back), or let it go
fn command(what: &str, slot: usize) -> GameEvent {
    GameEvent::RequestServerCommand {
        line: format!("@capture {what} {}", slot + 1),
    }
}

impl Monsters {
    /// The screen was opened: the slots are asked for again, and nothing is half released
    pub fn enter(&mut self) {
        self.asked = false;
        self.releasing = None;
    }

    pub fn portrait_built(&mut self, key: &str, w: u32, h: u32) {
        self.portraits.insert(key.to_string(), (w, h));
    }

    pub fn footer(&self, slots: &CaptureSlots) -> (String, Vec<Hint>) {
        let Some(monster) = slots.slots[self.picked].as_ref() else {
            return (
                "Catch a monster with a capture ball (the Cash Shop sells them, Capture Chests drop them).".into(),
                vec![Hint::BACK],
            );
        };
        let call = if slots.out == Some(self.picked) {
            "recall"
        } else {
            "call"
        };
        let help = if self.releasing == Some(self.picked) {
            format!("Release {} for good? Press Release again.", monster.name)
        } else {
            format!(
                "{}: call it out to fight beside you, or let it go.",
                monster.name
            )
        };
        (
            help,
            vec![
                Hint::tap("Enter", call, Key::Enter),
                Hint::tap("Del", "release", Key::Remove),
                Hint::BACK,
            ],
        )
    }

    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        events: &mut Vec<GameEvent>,
    ) {
        if !self.asked {
            self.asked = true;
            events.push(GameEvent::RequestServerCommand {
                line: "@capture list".into(),
            });
        }
        let slots = &ctx.character.captures;
        let before = self.picked;
        if keys.left {
            self.picked = self.picked.checked_sub(1).unwrap_or(SLOTS - 1);
        }
        if keys.right {
            self.picked = (self.picked + 1) % SLOTS;
        }

        let card_w = CARD_W.min(((area.w - 2.0 * COLUMN_GAP) / SLOTS as f32).floor());
        let y = frame::section(ui, area.x, area.y, area.w, "Monster slots");
        for i in 0..SLOTS {
            let card = Rect::new(area.x + i as f32 * (card_w + COLUMN_GAP), y, card_w, CARD_H);
            if self.card(ui, card, i, slots, textures) {
                self.picked = i;
            }
        }
        if self.picked != before {
            self.releasing = None;
        }

        let top = y + CARD_H + 24.0;
        let Some(monster) = slots.slots[self.picked].as_ref() else {
            let line = "This slot is empty.";
            style::text(ui, area.x, style::baseline(ui, top), line, style::DIM);
            return;
        };
        let stats_w = (area.w * 0.55).floor();
        let sy = frame::section(ui, area.x, top, stats_w, &monster.name);
        stat_grid(ui, area.x, sy, stats_w, monster);

        let bx = area.x + stats_w + COLUMN_GAP;
        let out = slots.out == Some(self.picked);
        let recovering = slots.recovering(self.picked);
        let call_label = if out {
            "Recall".to_string()
        } else if recovering > 0 {
            format!("Recovering {recovering} s")
        } else {
            "Call".to_string()
        };
        let mut call = keys.enter;
        let mut release = keys.remove;
        if button(
            ui,
            ACTION_BASE_ID,
            Rect::new(bx, sy, BUTTON_W, BUTTON_H),
            &call_label,
            out,
        ) {
            call = true;
        }
        let release_label = if self.releasing == Some(self.picked) {
            "Sure? Release"
        } else {
            "Release"
        };
        let release_rect = Rect::new(bx, sy + BUTTON_H + 12.0, BUTTON_W, BUTTON_H);
        if button(ui, ACTION_BASE_ID + 1, release_rect, release_label, false) {
            release = true;
        }
        if call && (out || recovering == 0) {
            events.push(command("call", self.picked));
        }
        if release {
            if self.releasing == Some(self.picked) {
                events.push(command("release", self.picked));
                self.releasing = None;
            } else {
                self.releasing = Some(self.picked);
            }
        }
    }

    /// One slot: the portrait, the name and level, and what it is up to; true when tapped
    fn card(
        &self,
        ui: &mut UiFrame,
        r: Rect,
        i: usize,
        slots: &CaptureSlots,
        textures: &mut Textures,
    ) -> bool {
        let resp = ui.interact(WidgetId(CARD_BASE_ID + i as u32), r);
        style::slot(ui, r, resp.hovered());
        if i == self.picked {
            style::highlight(ui, r);
        }
        let line = style::line(ui);
        let Some(monster) = slots.slots[i].as_ref() else {
            let text = format!("Slot {} · empty", i + 1);
            let w = style::measure(ui, &text);
            let baseline = style::baseline(ui, r.y + (r.h - line) / 2.0);
            style::text(
                ui,
                r.x + ((r.w - w) / 2.0).floor(),
                baseline,
                &text,
                style::DIM,
            );
            return resp.clicked();
        };
        let key = portrait_key(monster.mob_id);
        textures.want(&key);
        if let Some(&(w, h)) = self.portraits.get(&key) {
            // as big as fits, never stretched
            let area = Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, PORTRAIT_H);
            let scale = (area.w / w as f32).min(area.h / h as f32).min(2.0);
            let (pw, ph) = (w as f32 * scale, h as f32 * scale);
            let at = Rect::new(
                area.x + ((area.w - pw) / 2.0).floor(),
                area.y + (area.h - ph).floor(),
                pw,
                ph,
            );
            frame::image(ui, at, key, [1.0; 4]);
        }
        let name = style::fit(ui, &monster.name, r.w - 16.0);
        let ny = r.y + 8.0 + PORTRAIT_H + 6.0;
        style::text_bold(ui, r.x + 8.0, style::baseline(ui, ny), &name, style::TEXT);
        let recovering = slots.recovering(i);
        let (state, color) = if slots.out == Some(i) {
            ("fighting".to_string(), style::ACCENT)
        } else if recovering > 0 {
            (format!("recovering {recovering} s"), style::DIM)
        } else {
            ("ready".to_string(), style::GOOD)
        };
        let sub = format!("Lv {} · {state}", monster.level);
        style::text(
            ui,
            r.x + 8.0,
            style::baseline(ui, ny + line + 2.0),
            &sub,
            color,
        );
        resp.clicked()
    }
}

/// The stats in two columns
fn stat_grid(ui: &mut UiFrame, x: f32, top: f32, w: f32, m: &CapturedMonster) {
    let pct = u64::from(m.exp) * 100 / u64::from(m.exp_next.max(1));
    let values = [
        m.level.to_string(),
        format!("{} / {} ({pct}%)", m.exp, m.exp_next),
        m.hp.to_string(),
        format!("{}-{}", m.atk_min, m.atk_max),
        m.def.to_string(),
        m.mdef.to_string(),
        m.race_name().to_string(),
        format!("{} {}", m.element_name(), m.element_level),
        m.size_name().to_string(),
    ];
    let line = style::line(ui);
    let col_w = ((w - COLUMN_GAP) / 2.0).floor();
    let rows = STATS.len().div_ceil(2);
    for (i, (label, value)) in STATS.iter().zip(&values).enumerate() {
        let (col, row) = (i / rows, i % rows);
        let cx = x + col as f32 * (col_w + COLUMN_GAP);
        let baseline = style::baseline(
            ui,
            top + row as f32 * ROW_H + ((ROW_H - line) / 2.0).floor(),
        );
        style::text(ui, cx, baseline, label, style::DIM);
        style::text_right(ui, cx + col_w, baseline, value, style::TEXT);
    }
}

/// A labelled button; true when tapped
fn button(ui: &mut UiFrame, id: u32, r: Rect, label: &str, lit: bool) -> bool {
    let resp = ui.interact(WidgetId(id), r);
    style::slot(ui, r, resp.hovered() || lit);
    let line = style::line(ui);
    let w = style::measure_bold(ui, label);
    let color = if lit { style::ACCENT } else { style::TEXT };
    let baseline = style::baseline(ui, r.y + ((r.h - line) / 2.0).floor());
    style::text_bold(ui, r.x + ((r.w - w) / 2.0).floor(), baseline, label, color);
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_and_release_name_their_slot_from_one() {
        assert!(matches!(
            command("call", 0),
            GameEvent::RequestServerCommand { line } if line == "@capture call 1"
        ));
        assert!(matches!(
            command("release", 2),
            GameEvent::RequestServerCommand { line } if line == "@capture release 3"
        ));
    }
}
