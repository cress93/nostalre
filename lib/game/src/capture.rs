//! Captured monsters (nostalRE's server, npc/custom/capture.txt): capture balls catch any monster into one of
//! three slots, which the menu's Monsters screen shows and calls out or releases. The server tells the client
//! the slots in a `[[capture]]` line of its own chat, which never reaches the chat window, and takes the
//! screen's choices as `@capture` commands.

use std::time::Instant;

use crate::data_table::DataTable;

/// The slots a character has
pub const SLOTS: usize = 3;

/// What starts the server's line about the slots
pub const SYNC_PREFIX: &str = "[[capture]]";

/// The capture balls, Basic to Master: id, name, what it holds best, and the item whose icon it borrows (the
/// game's own tables do not know them)
pub const BALLS: [(u32, &str, &str, u32); 4] = [
    (
        99901,
        "Basic Capture Ball",
        "Catches monsters up to level 60 at its best.",
        12408,
    ),
    (
        99902,
        "Great Capture Ball",
        "Catches monsters up to level 130 at its best.",
        102202,
    ),
    (
        99903,
        "Ultra Capture Ball",
        "Catches monsters up to level 200 at its best.",
        23135,
    ),
    (
        99904,
        "Master Capture Ball",
        "Catches almost any monster, and is the only ball that can hold a boss.",
        12248,
    ),
];

/// The balls into the item tables: their names, descriptions and icons
pub fn add_ball_items(data: &mut DataTable) {
    for (id, name, holds, icon_of) in BALLS {
        if let Some(names) = data.item_name.as_mut() {
            names.insert(id, name);
        }
        if let Some(resources) = data.item_resource.as_mut() {
            resources.borrow_icon(id, icon_of);
        }
        if let Some(descriptions) = data.item_description.as_mut() {
            descriptions.insert(
                id,
                vec![
                    "Throw it at a monster to catch it into one of your three monster slots (Menu > Monsters)."
                        .into(),
                    holds.into(),
                    "Past its level it gets less sure, and a hurt monster is easier to catch.".into(),
                    "Weight: 1".into(),
                ],
            );
        }
    }
}

/// The menu's texture key for a monster's portrait, which the client builds from its sprite
pub fn portrait_key(mob_id: u16) -> String {
    format!("nostalre:mob/{mob_id}")
}

/// The monster a portrait key is of
pub fn portrait_mob(key: &str) -> Option<u16> {
    key.strip_prefix("nostalre:mob/")?.parse().ok()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedMonster {
    pub mob_id: u16,
    pub name: String,
    pub level: i32,
    pub hp: i32,
    pub atk_min: i32,
    pub atk_max: i32,
    pub def: i32,
    pub mdef: i32,
    pub race: u8,
    pub element: u8,
    pub element_level: u8,
    pub size: u8,
    /// Seconds it needed to recover when the line came
    pub recover_secs: u32,
    /// Its EXP toward the next level, and what that level needs
    pub exp: u32,
    pub exp_next: u32,
}

impl CapturedMonster {
    pub fn race_name(&self) -> &'static str {
        const RACES: [&str; 10] = [
            "Formless",
            "Undead",
            "Brute",
            "Plant",
            "Insect",
            "Fish",
            "Demon",
            "Demi-Human",
            "Angel",
            "Dragon",
        ];
        RACES.get(usize::from(self.race)).copied().unwrap_or("?")
    }

    pub fn element_name(&self) -> &'static str {
        const ELEMENTS: [&str; 10] = [
            "Neutral", "Water", "Earth", "Fire", "Wind", "Poison", "Holy", "Shadow", "Ghost",
            "Undead",
        ];
        ELEMENTS
            .get(usize::from(self.element))
            .copied()
            .unwrap_or("?")
    }

    pub fn size_name(&self) -> &'static str {
        ["Small", "Medium", "Large"]
            .get(usize::from(self.size))
            .copied()
            .unwrap_or("?")
    }

    /// id,level,hp,atk min,atk max,def,mdef,race,element,element level,size,seconds to recover,exp,exp to next,name
    fn parse(slot: &str) -> Option<Self> {
        let mut fields = slot.splitn(15, ',');
        let mut num = || fields.next()?.trim().parse::<i64>().ok();
        let mob_id = u16::try_from(num()?).ok().filter(|&id| id != 0)?;
        let (level, hp, atk_min, atk_max, def, mdef) =
            (num()?, num()?, num()?, num()?, num()?, num()?);
        let (race, element, element_level, size, recover) =
            (num()?, num()?, num()?, num()?, num()?);
        let (exp, exp_next) = (num()?, num()?);
        let name = fields.next()?.to_string();
        Some(Self {
            mob_id,
            name,
            level: level as i32,
            hp: hp as i32,
            atk_min: atk_min as i32,
            atk_max: atk_max as i32,
            def: def as i32,
            mdef: mdef as i32,
            race: race as u8,
            element: element as u8,
            element_level: element_level as u8,
            size: size as u8,
            recover_secs: recover.max(0) as u32,
            exp: exp.max(0) as u32,
            exp_next: exp_next.max(1) as u32,
        })
    }
}

/// The three slots, as the server last told them
#[derive(Debug, Clone, Default)]
pub struct CaptureSlots {
    pub slots: [Option<CapturedMonster>; SLOTS],
    /// The slot called out to fight
    pub out: Option<usize>,
    /// When the line came, for the recovery countdowns
    pub received: Option<Instant>,
}

impl CaptureSlots {
    /// The server's `[[capture]]` line: `<slot out or -1>|<slot>|<slot>|<slot>`, a slot `0` when empty. `None`
    /// for any other line
    pub fn parse(line: &str) -> Option<Self> {
        let rest = line.strip_prefix(SYNC_PREFIX)?;
        let mut parts = rest.split('|');
        let out = parts.next()?.trim().parse::<i64>().ok()?;
        let mut slots: [Option<CapturedMonster>; SLOTS] = Default::default();
        for slot in &mut slots {
            *slot = CapturedMonster::parse(parts.next().unwrap_or("0"));
        }
        Some(Self {
            slots,
            out: usize::try_from(out).ok().filter(|&s| s < SLOTS),
            received: Some(Instant::now()),
        })
    }

    /// Seconds slot `i` still needs before it can be called again
    pub fn recovering(&self, i: usize) -> u32 {
        let Some(monster) = self.slots.get(i).and_then(Option::as_ref) else {
            return 0;
        };
        let since = self.received.map_or(0, |at| at.elapsed().as_secs()) as u32;
        monster.recover_secs.saturating_sub(since)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sync_line_reads_its_slots() {
        let line = "[[capture]]1|1002,1,55,7,10,2,5,3,3,1,1,0,4,10,Poring|0|1113,15,300,30,40,8,0,2,2,1,0,42,0,122,Drops, the Bold";
        let slots = CaptureSlots::parse(line).unwrap();
        assert_eq!(slots.out, Some(1));
        let poring = slots.slots[0].as_ref().unwrap();
        assert_eq!((poring.mob_id, poring.level, poring.hp), (1002, 1, 55));
        assert_eq!(poring.name, "Poring");
        assert_eq!(poring.element_name(), "Fire");
        assert!(slots.slots[1].is_none());
        let drops = slots.slots[2].as_ref().unwrap();
        assert_eq!(drops.name, "Drops, the Bold", "a name may hold commas");
        assert_eq!(drops.recover_secs, 42);
        assert_eq!((poring.exp, poring.exp_next), (4, 10));
        assert!(slots.recovering(2) <= 42 && slots.recovering(2) >= 41);
    }

    #[test]
    fn nothing_out_and_other_lines() {
        let slots = CaptureSlots::parse("[[capture]]-1|0|0|0").unwrap();
        assert_eq!(slots.out, None);
        assert!(slots.slots.iter().all(Option::is_none));
        assert!(CaptureSlots::parse("Caught Poring!").is_none());
    }

    #[test]
    fn portrait_keys_round_trip() {
        assert_eq!(portrait_mob(&portrait_key(1002)), Some(1002));
        assert_eq!(portrait_mob("data/texture/x.bmp"), None);
    }
}
