//! Album: the Card Album (`nostalre_game::card_album`). On top the count, the next reward and everything earned;
//! under it the regions, each with its cards in and its reward, and the picked region's cards: the ones in the
//! album bright, the missing ones dim with the monster dropping them and where it is. A card tapped twice
//! navigates to that map.
use super::ROW_BASE_ID;
use super::frame::{self, COLUMN_GAP, Hint, Keys, ROW_H};
use crate::BuildCtx;
use crate::hud::style;
use nostalre_game::card_album::{self, AlbumCard, CardAlbum};
use nostalre_game::event::GameEvent;
use nostalre_game::map_monster_tables::monster_info;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The card rows' ids, after the regions'
const CARD_ROW_BASE: u32 = ROW_BASE_ID + 100;
const REGIONS_W: f32 = 0.36;

#[derive(Default)]
pub(super) struct Album {
    region: usize,
    region_offset: usize,
    card: Option<usize>,
    card_offset: usize,
    /// The server was asked for the album since the screen opened
    asked: bool,
}

impl Album {
    pub fn enter(&mut self) {
        self.asked = false;
    }

    pub fn summary(album: &CardAlbum) -> String {
        format!("{} of {} cards", album.count(), card_album::total())
    }

    pub fn footer(&self, album: &CardAlbum) -> (String, Vec<Hint>) {
        let help = match card_album::regions().nth(self.region) {
            Some(r) => format!(
                "{}: {} of {} cards (bosses' cards count toward the total only). Completing it gives {}. \
                 Tap a card twice to navigate to its monster.",
                r.name,
                album.region_count(self.region),
                r.need,
                r.perk.text(r.amount).to_lowercase()
            ),
            None => String::new(),
        };
        (help, vec![Hint::MOVE, Hint::BACK])
    }

    /// Draws the screen; true when a card's map was picked to navigate to (the menu closes)
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        if !self.asked {
            self.asked = true;
            events.push(GameEvent::RequestServerCommand {
                line: "@album sync".into(),
            });
        }
        let album = &ctx.character.album;
        let line = style::line(ui);
        let mut y = area.y;

        // the count, the next reward, what was earned
        let count = album.count();
        let total = card_album::total();
        let head = format!("{count} / {total} cards");
        style::text_bold(ui, area.x, style::baseline(ui, y), &head, style::ACCENT);
        let next = match album.next_milestone() {
            Some((at, perk, amount)) => format!("Next at {at}: {}", perk.text(amount)),
            None => "Every count reward earned".into(),
        };
        style::text_right(ui, area.x + area.w, style::baseline(ui, y), &next, style::DIM);
        y += line + 6.0;
        let earned = album.earned();
        let earned = if earned.is_empty() {
            "No rewards yet: find cards to fill the album.".to_string()
        } else {
            earned
                .iter()
                .map(|&(perk, amount)| perk.text(amount))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        for row in style::wrap(ui, &earned, area.w).iter().take(2) {
            style::text(ui, area.x, style::baseline(ui, y), row, style::GOOD);
            y += line;
        }
        y += 10.0;

        // the regions
        let regions: Vec<_> = card_album::regions().collect();
        let list_w = (area.w * REGIONS_W).floor();
        let list = Rect::new(area.x, y, list_w, area.y + area.h - y);
        let visible = (list.h / ROW_H).floor().max(1.0) as usize;
        let before = self.region;
        self.region = frame::step(keys, self.region, regions.len());
        let follow = self.region != before;
        if let Some(back) = frame::scroll(
            ui,
            list,
            &mut self.region_offset,
            self.region,
            regions.len(),
            visible,
            ROW_H,
            follow,
        ) {
            self.region = back;
        }
        for (k, (i, region)) in regions
            .iter()
            .enumerate()
            .skip(self.region_offset)
            .take(visible)
            .enumerate()
        {
            let r = Rect::new(list.x, list.y + k as f32 * ROW_H, list.w, ROW_H);
            let resp = frame::row(ui, WidgetId(ROW_BASE_ID + k as u32), r, i == self.region);
            if resp.clicked() && i != self.region {
                self.region = i;
            }
            let have = album.region_count(i);
            let done = have >= region.need;
            let baseline = frame::row_baseline(ui, r);
            let progress = if done {
                "✓".to_string()
            } else {
                format!("{have}/{}", region.need)
            };
            let pw = style::measure(ui, &progress);
            let name = style::fit(ui, region.name, r.w - pw - frame::ROW_TEXT - 16.0);
            style::text(ui, r.x + frame::ROW_TEXT, baseline, &name, style::TEXT);
            let color = if done { style::GOOD } else { style::DIM };
            style::text_right(ui, r.x + r.w - 8.0, baseline, &progress, color);
        }
        if self.region != before {
            self.card = None;
            self.card_offset = 0;
        }

        // the picked region: its reward, its cards
        let Some(region) = regions.get(self.region) else {
            return false;
        };
        let x = area.x + list_w + COLUMN_GAP / 2.0;
        let w = area.x + area.w - x;
        let mut cy = y;
        let done = album.region_done(region, self.region);
        let reward = format!(
            "{}: {}",
            if done { "Earned" } else { "Completing it" },
            region.perk.text(region.amount)
        );
        style::text_bold(ui, x, style::baseline(ui, cy), region.name, style::TEXT);
        cy += line + 2.0;
        style::text(
            ui,
            x,
            style::baseline(ui, cy),
            &style::fit(ui, &reward, w),
            if done { style::GOOD } else { style::DIM },
        );
        cy += line + 8.0;
        let cards: Vec<AlbumCard> = card_album::cards_of(self.region).collect();
        let rows = Rect::new(x, cy, w, area.y + area.h - cy);
        let row_h = line + 10.0;
        let shown = (rows.h / row_h).floor().max(1.0) as usize;
        let picked = self.card.unwrap_or(usize::MAX);
        frame::scroll(
            ui,
            rows,
            &mut self.card_offset,
            picked.min(cards.len()),
            cards.len(),
            shown,
            row_h,
            false,
        );
        let mut navigate = None;
        for (k, (i, card)) in cards
            .iter()
            .enumerate()
            .skip(self.card_offset)
            .take(shown)
            .enumerate()
        {
            let r = Rect::new(rows.x, rows.y + k as f32 * row_h, rows.w, row_h);
            let resp = frame::row(ui, WidgetId(CARD_ROW_BASE + k as u32), r, self.card == Some(i));
            if resp.clicked() {
                if self.card == Some(i) {
                    navigate = Some(*card);
                }
                self.card = Some(i);
            }
            let owned = album.has(card.id);
            let baseline = frame::row_baseline(ui, r);
            let monster = monster_info(card.mob).map_or("?", |(name, ..)| name);
            let place = crate::hud::place_name(ctx.data, card.map);
            let source = if card.boss {
                format!("{monster} (boss) · {place}")
            } else {
                format!("{monster} · {place}")
            };
            let source = style::fit(ui, &source, r.w * 0.5);
            let sw = style::measure(ui, &source);
            let name = ctx.data.item_name_for(card.id, true);
            let name = style::fit(ui, &name, r.w - sw - 24.0);
            let (name_color, mark) = if owned {
                (style::TEXT, "● ")
            } else {
                (style::DIM_MORE, "○ ")
            };
            style::text(ui, r.x + 4.0, baseline, &format!("{mark}{name}"), name_color);
            style::text_right(ui, r.x + r.w - 8.0, baseline, &source, style::DIM);
        }
        if let Some(card) = navigate {
            let monster = monster_info(card.mob).map_or("?", |(name, ..)| name);
            events.push(GameEvent::Navigate {
                map: card.map.to_string(),
                cell: None,
                label: format!("{monster} · {}", crate::hud::place_name(ctx.data, card.map)),
            });
            return true;
        }
        false
    }
}
