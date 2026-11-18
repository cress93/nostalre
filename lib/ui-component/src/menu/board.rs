//! Board: the quest board's three cards (`nostalre_game::questboard`), each with its rewards under it, the quests
//! taken with how far along they are, Reroll and Abort Current. A tap picks a card, a second tap takes it.
use super::frame::{self, COLUMN_GAP, Hint, Key, Keys, Textures};
use super::{ACTION_BASE_ID, CARD_BASE_ID, ROW_BASE_ID};
use crate::BuildCtx;
use crate::hud::style;
use nostalre_game::event::GameEvent;
use nostalre_game::questboard::{CARDS, QuestBoard, QuestKind, Reward, SLOTS, wait_text};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const CARD_H: f32 = 170.0;
const SLOT: f32 = 48.0;
const SLOT_GAP: f32 = 10.0;
const ROW_H: f32 = 30.0;
const BUTTON_W: f32 = 170.0;
const BUTTON_H: f32 = 38.0;
const ICON: f32 = 24.0;

#[derive(Default)]
pub(super) struct Board {
    /// The card picked: a second tap takes it
    card: usize,
    /// The quest taken that Abort Current would drop
    active: usize,
    /// The server was asked for the board since the screen opened
    asked: bool,
}

fn command(line: String) -> GameEvent {
    GameEvent::RequestServerCommand { line }
}

impl Board {
    pub fn enter(&mut self) {
        self.asked = false;
    }

    pub fn footer(&self, board: &QuestBoard) -> (String, Vec<Hint>) {
        let help = match board.cards[self.card].as_ref() {
            Some(q) => format!(
                "{} · {} · {} (Lv {}). Tap the card again to take it.",
                q.goal(),
                q.difficulty_name(),
                q.map_name,
                q.map_level
            ),
            None => {
                "This card was taken: new quests come when the board renews. Up to three at once."
                    .into()
            }
        };
        (
            help,
            vec![
                Hint::MOVE,
                Hint::tap("Enter", "take", Key::Enter),
                Hint::tap("Del", "abort", Key::Remove),
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
            events.push(command("@board list".into()));
        }
        let board = &ctx.character.board;
        if keys.left {
            self.card = self.card.checked_sub(1).unwrap_or(CARDS - 1);
        }
        if keys.right {
            self.card = (self.card + 1) % CARDS;
        }
        let mut take = keys.enter;

        let gap = COLUMN_GAP / 2.0;
        let card_w = ((area.w - (CARDS - 1) as f32 * gap) / CARDS as f32).floor();
        for i in 0..CARDS {
            let x = area.x + i as f32 * (card_w + gap);
            let card = Rect::new(x, area.y, card_w, CARD_H);
            if self.card(ui, card, i, board) {
                // a second tap on the picked card takes it
                take |= self.card == i;
                self.card = i;
            }
            let quest = board.cards[i].as_ref();
            rewards(ui, ctx, x, area.y + CARD_H + 8.0, card_w, quest, textures);
        }
        if take && board.cards[self.card].is_some() {
            events.push(command(format!("@board take {}", self.card + 1)));
        }

        // the quests taken
        let top = area.y + CARD_H + SLOT + 26.0;
        let ay = frame::section(
            ui,
            area.x,
            top,
            area.w,
            &format!("Active quests {}/{SLOTS}", board.active_count()),
        );
        let line = style::line(ui);
        for i in 0..SLOTS {
            let r = Rect::new(area.x, ay + i as f32 * ROW_H, area.w, ROW_H);
            let baseline = style::baseline(ui, r.y + ((ROW_H - line) / 2.0).floor());
            let Some((q, progress)) = board.active[i].as_ref() else {
                style::text(ui, r.x + frame::ROW_TEXT, baseline, "—", style::DIM);
                continue;
            };
            if frame::row(ui, WidgetId(ROW_BASE_ID + i as u32), r, i == self.active).clicked() {
                self.active = i;
            }
            let text = format!("{} · {}", q.goal(), q.map_name);
            let text = style::fit(ui, &text, r.w - 140.0);
            style::text(ui, r.x + frame::ROW_TEXT, baseline, &text, style::TEXT);
            let done = format!("{} / {}", progress.min(&q.count), q.count);
            style::text_right(ui, r.x + r.w - 8.0, baseline, &done, style::ACCENT);
        }

        // the picked active quest's rewards (the first one taken when the picked row is empty)
        if board.active[self.active].is_none()
            && let Some(first) = board.active.iter().position(Option::is_some)
        {
            self.active = first;
        }
        let ry = ay + SLOTS as f32 * ROW_H + 10.0;
        let picked = board.active[self.active].as_ref().map(|(q, _)| q);
        if picked.is_some() {
            let label_baseline = style::baseline(ui, ry + ((SLOT - line) / 2.0).floor());
            style::text(
                ui,
                area.x + frame::ROW_TEXT,
                label_baseline,
                "Rewards",
                style::DIM,
            );
            rewards(ui, ctx, area.x, ry, area.w, picked, textures);
        }

        // Reroll bottom left, Abort Current bottom right
        let below = if picked.is_some() {
            ry + SLOT + 10.0
        } else {
            ry
        };
        let by = (area.y + area.h - BUTTON_H).max(below);
        let wait = board.reroll_wait();
        let reroll = if wait > 0 {
            format!("Reroll {}", wait_text(wait))
        } else {
            "Reroll".to_string()
        };
        if button(
            ui,
            ACTION_BASE_ID,
            Rect::new(area.x, by, BUTTON_W, BUTTON_H),
            &reroll,
        ) && wait == 0
        {
            events.push(command("@board reroll".into()));
        }
        let renew_wait = board.renew_wait();
        if renew_wait > 0 {
            let note = format!("New quests in {}", wait_text(renew_wait));
            let w = style::measure(ui, &note);
            let baseline = style::baseline(ui, by + ((BUTTON_H - line) / 2.0).floor());
            style::text(
                ui,
                area.x + ((area.w - w) / 2.0).floor(),
                baseline,
                &note,
                style::DIM,
            );
        }
        let abort = Rect::new(area.x + area.w - BUTTON_W, by, BUTTON_W, BUTTON_H);
        let aborted = button(ui, ACTION_BASE_ID + 1, abort, "Abort Current") || keys.remove;
        if aborted && board.active[self.active].is_some() {
            events.push(command(format!("@board abort {}", self.active + 1)));
        }
    }

    /// One card: kind and stars, map and level, the goal, its EXP; true when tapped
    fn card(&self, ui: &mut UiFrame, r: Rect, i: usize, board: &QuestBoard) -> bool {
        let resp = ui.interact(WidgetId(CARD_BASE_ID + i as u32), r);
        style::slot(ui, r, resp.hovered());
        if i == self.card {
            style::highlight(ui, r);
        }
        let line = style::line(ui);
        let Some(q) = board.cards[i].as_ref() else {
            let baseline = style::baseline(ui, r.y + (r.h - line) / 2.0);
            style::text(ui, r.x + 12.0, baseline, "—", style::DIM);
            return resp.clicked();
        };
        let x = r.x + 12.0;
        let w = r.w - 24.0;
        let mut y = r.y + 10.0;
        let stars =
            "★".repeat(usize::from(q.difficulty)) + &"☆".repeat(3 - usize::from(q.difficulty));
        style::text_bold(ui, x, style::baseline(ui, y), q.kind.name(), style::ACCENT);
        style::text_right(
            ui,
            r.x + r.w - 12.0,
            style::baseline(ui, y),
            &stars,
            style::ACCENT,
        );
        y += line + 8.0;
        let goal = style::wrap(ui, &q.goal(), w);
        for row in goal.iter().take(2) {
            style::text_bold(ui, x, style::baseline(ui, y), row, style::TEXT);
            y += line;
        }
        y += 6.0;
        // an instance's says where to go, over two lines
        let rows = if matches!(
            q.kind,
            QuestKind::Instance | QuestKind::Combo | QuestKind::Grand
        ) {
            2
        } else {
            1
        };
        let wrapped = style::wrap(ui, &q.map_name, w);
        for (n, row) in wrapped.iter().take(rows).enumerate() {
            let text = if n + 1 == rows {
                style::fit(ui, row, w)
            } else {
                row.clone()
            };
            style::text(ui, x, style::baseline(ui, y), &text, style::DIM);
            y += line;
        }
        let lv = format!("Lv {} · {}", q.map_level, q.difficulty_name());
        style::text(ui, x, style::baseline(ui, y), &lv, style::DIM);
        resp.clicked()
    }
}

/// A quest's three reward slots, centred in `w` from `x`: its EXP, then up to two items or zeny
fn rewards(
    ui: &mut UiFrame,
    ctx: &BuildCtx,
    x: f32,
    y: f32,
    w: f32,
    quest: Option<&nostalre_game::questboard::Quest>,
    textures: &mut Textures,
) {
    {
        let row_w = 3.0 * SLOT + 2.0 * SLOT_GAP;
        let x0 = x + ((w - row_w) / 2.0).floor();
        let small = style::line(ui);
        for k in 0..3usize {
            let r = Rect::new(x0 + k as f32 * (SLOT + SLOT_GAP), y, SLOT, SLOT);
            style::slot(ui, r, false);
            let Some(q) = quest else {
                continue;
            };
            let baseline = style::baseline(ui, r.y + r.h - small - 2.0);
            match (k, q.rewards.get(k.wrapping_sub(1))) {
                (0, _) => {
                    let top = style::baseline(ui, r.y + 4.0);
                    style::text_bold(ui, r.x + 6.0, top, "EXP", style::GOOD);
                    let amount = short(q.base_exp);
                    style::text_right(ui, r.x + r.w - 4.0, baseline, &amount, style::TEXT);
                }
                (_, Some(Reward::Zeny(z))) => {
                    let top = style::baseline(ui, r.y + 4.0);
                    style::text_bold(ui, r.x + 6.0, top, "z", style::ACCENT);
                    style::text_right(
                        ui,
                        r.x + r.w - 4.0,
                        baseline,
                        &short(u64::from(*z)),
                        style::TEXT,
                    );
                }
                (_, Some(Reward::Item { id, amount })) => {
                    if let Some(path) = ctx.data.item_icon_path(*id) {
                        textures.want(&path);
                        let icon =
                            Rect::new(r.x + ((r.w - ICON) / 2.0).floor(), r.y + 4.0, ICON, ICON);
                        frame::image(ui, icon, path, [1.0; 4]);
                    }
                    if *amount > 1 {
                        style::text_right(
                            ui,
                            r.x + r.w - 4.0,
                            baseline,
                            &format!("×{amount}"),
                            style::TEXT,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// 1234 as 1.2k, 1234567 as 1.2M
fn short(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.1}B", n as f64 / 1e9),
    }
}

/// A labelled button; true when tapped
fn button(ui: &mut UiFrame, id: u32, r: Rect, label: &str) -> bool {
    let resp = ui.interact(WidgetId(id), r);
    style::slot(ui, r, resp.hovered());
    let line = style::line(ui);
    let w = style::measure_bold(ui, label);
    let baseline = style::baseline(ui, r.y + ((r.h - line) / 2.0).floor());
    style::text_bold(
        ui,
        r.x + ((r.w - w) / 2.0).floor(),
        baseline,
        label,
        style::TEXT,
    );
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn big_numbers_are_short() {
        assert_eq!(short(950), "950");
        assert_eq!(short(12_340), "12.3k");
        assert_eq!(short(4_500_000), "4.5M");
    }
}
