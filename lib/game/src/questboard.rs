//! The quest board (nostalRE's server, npc/custom/questboard.txt): three cards of quests on maps of the character's
//! level, the up to three taken, and the waits. The server sends them as `[[qbc]]`, `[[qba]]` and `[[qb]]` lines of
//! its own chat, which never reach the chat window; the menu's Board screen shows them and answers with `@board`.

use std::time::Instant;

/// The cards, and the quests one may have at once
pub const SLOTS: usize = 3;

/// The cards on the board: three quests (now and then a Grand Hunt), and a fourth that is always an instance
pub const CARDS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestKind {
    Hunt,
    Cull,
    Elite,
    Gather,
    Capture,
    /// Beat a boss in one of rAthena's instances; the card says how to get there
    Instance,
    /// Monsters of one race or element, within ten levels, anywhere
    Combo,
    /// A great hunt: any monsters within ten levels, anywhere
    Grand,
}

impl QuestKind {
    fn from_number(n: i64) -> Option<Self> {
        Some(match n {
            1 => Self::Hunt,
            2 => Self::Cull,
            3 => Self::Elite,
            4 => Self::Gather,
            5 => Self::Capture,
            6 => Self::Instance,
            7 => Self::Combo,
            8 => Self::Grand,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Hunt => "Hunt",
            Self::Cull => "Cull",
            Self::Elite => "Elite",
            Self::Gather => "Gather",
            Self::Capture => "Capture",
            Self::Instance => "Instance",
            Self::Combo => "Combo Hunt",
            Self::Grand => "Grand Hunt",
        }
    }
}

/// A reward besides the EXP: an item and how many, or zeny
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reward {
    Item { id: u32, amount: u32 },
    Zeny(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quest {
    pub kind: QuestKind,
    /// 1 Easy, 2 Normal, 3 Hard
    pub difficulty: u8,
    pub map: String,
    pub map_name: String,
    pub map_level: u32,
    /// The monster (an item for Gather), 0 for a Cull
    pub target: u32,
    pub target_name: String,
    pub count: u32,
    pub base_exp: u64,
    pub rewards: Vec<Reward>,
}

impl Quest {
    /// What it asks, in a few words
    pub fn goal(&self) -> String {
        let (n, what) = (self.count, &self.target_name);
        match self.kind {
            QuestKind::Hunt => format!("Kill {n} {what}"),
            QuestKind::Cull => format!("Kill {n} monsters"),
            QuestKind::Elite if n == 1 => format!("Beat an Elite {what}"),
            QuestKind::Elite => format!("Beat {n} Elite {what}"),
            QuestKind::Gather => format!("Bring {n} {what}"),
            QuestKind::Capture => format!("Catch a {what}"),
            QuestKind::Instance => format!("Clear {what}"),
            QuestKind::Combo | QuestKind::Grand => format!("Kill {n} {what}"),
        }
    }

    pub fn difficulty_name(&self) -> &'static str {
        match self.difficulty {
            1 => "Easy",
            2 => "Normal",
            _ => "Hard",
        }
    }

    /// kind|difficulty|map|map name|map level|target|target name|count|base exp|reward 1|amount|reward 2|amount
    fn parse(fields: &[&str]) -> Option<Self> {
        let num = |i: usize| fields.get(i)?.trim().parse::<i64>().ok();
        let kind = QuestKind::from_number(num(0)?)?;
        let mut rewards = Vec::new();
        for i in [9, 11] {
            let (id, amount) = (num(i)?, num(i + 1)?.max(0) as u32);
            match id {
                -1 => rewards.push(Reward::Zeny(amount)),
                id if id > 0 && amount > 0 => rewards.push(Reward::Item {
                    id: id as u32,
                    amount,
                }),
                _ => {}
            }
        }
        Some(Self {
            kind,
            difficulty: num(1)?.clamp(1, 3) as u8,
            map: fields.get(2)?.to_string(),
            map_name: fields.get(3)?.to_string(),
            map_level: num(4)?.max(0) as u32,
            target: num(5)?.max(0) as u32,
            target_name: fields.get(6)?.to_string(),
            count: num(7)?.max(1) as u32,
            base_exp: num(8)?.max(0) as u64,
            rewards,
        })
    }
}

/// The board, as the server last told it
#[derive(Debug, Clone, Default)]
pub struct QuestBoard {
    pub cards: [Option<Quest>; CARDS],
    /// The quests taken, with how far along each is
    pub active: [Option<(Quest, u32)>; SLOTS],
    renew_wait: u32,
    reroll_wait: u32,
    received: Option<Instant>,
    /// A board was talked to: the Board screen opens
    pub open_asked: bool,
}

impl QuestBoard {
    /// One of the server's lines: true when it was one ([[qbc]], [[qba]] or [[qb]]), which the chat never shows
    pub fn apply(&mut self, line: &str) -> bool {
        if let Some(rest) = line.strip_prefix("[[qbc]]") {
            let fields: Vec<&str> = rest.split('|').collect();
            if let Some(slot) = slot_of(&fields, CARDS) {
                self.cards[slot] = Quest::parse(&fields[1..]);
            }
            return true;
        }
        if let Some(rest) = line.strip_prefix("[[qba]]") {
            let fields: Vec<&str> = rest.split('|').collect();
            if let Some(slot) = slot_of(&fields, SLOTS) {
                let progress = fields
                    .get(14)
                    .and_then(|p| p.trim().parse().ok())
                    .unwrap_or(0);
                self.active[slot] = Quest::parse(&fields[1..]).map(|q| (q, progress));
            }
            return true;
        }
        if let Some(rest) = line.strip_prefix("[[qb]]") {
            let mut fields = rest
                .split('|')
                .map(|f| f.trim().parse::<u32>().unwrap_or(0));
            self.renew_wait = fields.next().unwrap_or(0);
            self.reroll_wait = fields.next().unwrap_or(0);
            self.open_asked |= fields.next() == Some(1);
            self.received = Some(Instant::now());
            return true;
        }
        false
    }

    fn since(&self) -> u32 {
        self.received.map_or(0, |at| at.elapsed().as_secs()) as u32
    }

    /// Seconds until the cards renew by themselves
    pub fn renew_wait(&self) -> u32 {
        self.renew_wait.saturating_sub(self.since())
    }

    /// Seconds until the cards can be rerolled
    pub fn reroll_wait(&self) -> u32 {
        self.reroll_wait.saturating_sub(self.since())
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().flatten().count()
    }
}

fn slot_of(fields: &[&str], slots: usize) -> Option<usize> {
    fields
        .first()?
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&s| s < slots)
}

/// The quest log's ids for the quests taken, one per slot: far past kRO's own
pub const QUEST_ID: u32 = 9_900_000;

/// Where a quest takes the player, for the navigation: its map, a cell there when there is one (an instance's
/// entry NPC), and what to call it
pub type Place = (String, Option<(u16, u16)>, String);

impl Quest {
    /// Where this quest is done; `None` for one done anywhere
    pub fn place(&self) -> Option<Place> {
        match self.kind {
            QuestKind::Combo | QuestKind::Grand => None,
            // "Talk to Emily in Prontera Field (145, 235)"
            QuestKind::Instance => {
                let (how, cell) = self.map_name.rsplit_once(" (")?;
                let (x, y) = cell.trim_end_matches(')').split_once(',')?;
                let cell = (x.trim().parse().ok()?, y.trim().parse().ok()?);
                let who = how.split(" in ").next().unwrap_or(how);
                Some((self.map.clone(), Some(cell), who.to_string()))
            }
            _ if self.map.is_empty() => None,
            _ => Some((self.map.clone(), None, self.map_name.clone())),
        }
    }
}

impl QuestBoard {
    /// Where the quest log's quest `id` (one taken from the board) is done
    pub fn place_of(&self, id: u32) -> Option<Place> {
        let slot = id.checked_sub(QUEST_ID)? as usize;
        self.active.get(slot)?.as_ref()?.0.place()
    }

    /// The quests taken as quest log entries (the quest window and the HUD's list track them): id, title, what it
    /// says under it, its description, and its one goal as an objective
    pub fn log_entries(
        &self,
        items: impl Fn(u32) -> String,
    ) -> Vec<(crate::quest::Quest, String, String, String)> {
        let mut out = Vec::new();
        for (slot, entry) in self.active.iter().enumerate() {
            let Some((q, progress)) = entry else {
                continue;
            };
            let id = QUEST_ID + slot as u32;
            let what = match q.kind {
                QuestKind::Cull => "Monsters".to_string(),
                QuestKind::Elite => format!("Elite {}", q.target_name),
                _ => q.target_name.clone(),
            };
            let objective = crate::quest::QuestObjective {
                hunt_id: id,
                mob_id: if matches!(q.kind, QuestKind::Hunt | QuestKind::Elite) {
                    q.target
                } else {
                    0
                },
                name: what,
                current: (*progress).min(q.count).min(i16::MAX as u32) as i16,
                required: q.count.min(i16::MAX as u32) as i16,
            };
            let rewards: Vec<String> = std::iter::once(format!("{} base EXP", q.base_exp))
                .chain(q.rewards.iter().map(|r| match r {
                    Reward::Zeny(z) => format!("{z} zeny"),
                    Reward::Item { id, amount } => format!("{amount} x {}", items(*id)),
                }))
                .collect();
            let title = format!("Board: {}", q.goal());
            let summary = format!(
                "{} (Lv {}) · {}",
                q.map_name,
                q.map_level,
                q.difficulty_name()
            );
            let description = format!(
                "{} on {}, a {} quest of the Quest Board.\nRewards: {}.",
                q.goal(),
                q.map_name,
                q.difficulty_name().to_lowercase(),
                rewards.join(", ")
            );
            out.push((
                crate::quest::Quest {
                    id,
                    active: true,
                    end_time: None,
                    objectives: vec![objective],
                },
                title,
                summary,
                description,
            ));
        }
        out
    }
}

/// A wait as m:ss
pub fn wait_text(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quest_taken_says_where_it_is_done() {
        let mut board = QuestBoard::default();
        board.apply("[[qba]]0|1|2|prt_fild08|Prontera Field 8|3|1002|Poring|60|4500|501|50|-1|1200|4");
        board.apply("[[qba]]1|6|3|prt_fild05|Talk to Emily in Prontera Field (145, 235)|30|1|Poring Village|1|9000|0|0|0|0|0");
        board.apply("[[qba]]2|8|1||Any map, monsters Lv 20-40|30|0|monsters|600|9000|0|0|0|0|0");
        assert_eq!(
            board.place_of(QUEST_ID),
            Some(("prt_fild08".into(), None, "Prontera Field 8".into()))
        );
        assert_eq!(
            board.place_of(QUEST_ID + 1),
            Some(("prt_fild05".into(), Some((145, 235)), "Talk to Emily".into()))
        );
        assert_eq!(board.place_of(QUEST_ID + 2), None, "done anywhere");
        assert_eq!(board.place_of(12), None, "not the board's");
    }

    #[test]
    fn cards_active_quests_and_waits_read_back() {
        let mut board = QuestBoard::default();
        assert!(board.apply(
            "[[qbc]]0|1|2|prt_fild08|Prontera Field 8|3|1002|Poring|60|4500|501|50|-1|1200"
        ));
        assert!(board.apply("[[qbc]]1|0"));
        assert!(board.apply(
            "[[qba]]2|4|3|pay_dun00|Payon Cave F1|23|957|Decayed Nail|25|90000|1105|1|985|3|12"
        ));
        assert!(board.apply("[[qb]]120|0|1"));
        assert!(!board.apply("Caught Poring!"));

        let card = board.cards[0].as_ref().unwrap();
        assert_eq!(card.goal(), "Kill 60 Poring");
        assert_eq!(card.difficulty_name(), "Normal");
        assert_eq!(
            card.rewards,
            vec![
                Reward::Item {
                    id: 501,
                    amount: 50
                },
                Reward::Zeny(1200)
            ]
        );
        assert!(board.cards[1].is_none());
        let (quest, progress) = board.active[2].as_ref().unwrap();
        assert_eq!(
            (quest.goal().as_str(), *progress),
            ("Bring 25 Decayed Nail", 12)
        );
        assert_eq!(board.active_count(), 1);
        assert!(board.open_asked);
        assert!(board.renew_wait() <= 120 && board.renew_wait() >= 119);
        assert_eq!(wait_text(125), "2:05");
    }
}
