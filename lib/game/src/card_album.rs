//! The Card Album (nostalRE's server, npc/custom/cardalbum.txt): every card a monster of the world drops, region by
//! region (`crate::card_album_tables`, from scripts/gen-card-album.py), the ones the account has had, and what they
//! earned: a reward at each count of cards, and one for each region whose cards (bosses' aside) are all in. The
//! server sends the album as `[[alb]]` lines of its own chat, which never reach the chat window.

use std::collections::HashSet;

use crate::card_album_tables::{CARDS, MILESTONES, REGIONS};

const PREFIX: &str = "[[alb]]";

/// A reward's kind, as the tables and the server's #ALBUM_ variables number them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perk {
    Discount = 1,
    Overcharge = 2,
    Weight = 3,
    Travel = 4,
    Board = 5,
    Refine = 6,
}

impl Perk {
    pub const ALL: [Perk; 6] = [
        Perk::Discount,
        Perk::Overcharge,
        Perk::Weight,
        Perk::Travel,
        Perk::Board,
        Perk::Refine,
    ];

    pub fn from_number(n: u8) -> Option<Self> {
        Self::ALL.get(usize::from(n).checked_sub(1)?).copied()
    }

    /// The reward in words, as the server says it
    pub fn text(self, amount: u32) -> String {
        match self {
            Self::Discount => format!("Shop discount +{amount}%"),
            Self::Overcharge => format!("Overcharge +{amount}%"),
            Self::Weight => format!("Max weight +{amount}"),
            Self::Travel => format!("Kafra teleports {amount}% cheaper"),
            Self::Board => format!("Quest board waits {amount}% shorter"),
            Self::Refine => format!("Refine chance +{amount}%"),
        }
    }

    /// The most the server gives of it (npc/custom/cardalbum.txt's caps)
    fn cap(self) -> u32 {
        match self {
            Self::Travel => 100,
            Self::Board => 90,
            _ => u32::MAX,
        }
    }
}

/// One card of the album: its id, its region (an index into `regions()`), the monster dropping it, whether only
/// bosses do, the map that monster spawns most on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlbumCard {
    pub id: u32,
    pub region: usize,
    pub mob: u16,
    pub boss: bool,
    pub map: &'static str,
}

/// A region: its name, the cards completing it, its reward
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub name: &'static str,
    pub need: usize,
    pub perk: Perk,
    pub amount: u32,
}

pub fn regions() -> impl Iterator<Item = Region> {
    REGIONS.iter().filter_map(|&(name, need, perk, amount)| {
        Some(Region {
            name,
            need: usize::from(need),
            perk: Perk::from_number(perk)?,
            amount: u32::from(amount),
        })
    })
}

/// The cards of region `region`, the ones completing it first, then the bosses'
pub fn cards_of(region: usize) -> impl Iterator<Item = AlbumCard> {
    CARDS
        .iter()
        .filter(move |c| usize::from(c.1) == region)
        .map(|&(id, region, mob, boss, map)| AlbumCard {
            id,
            region: usize::from(region),
            mob,
            boss,
            map,
        })
}

pub fn total() -> usize {
    CARDS.len()
}

/// The rewards at each count: count, kind, amount
pub fn milestones() -> impl Iterator<Item = (usize, Perk, u32)> {
    MILESTONES
        .iter()
        .filter_map(|&(count, perk, amount)| Some((usize::from(count), Perk::from_number(perk)?, u32::from(amount))))
}

/// The cards the account has had
#[derive(Debug, Default, Clone)]
pub struct CardAlbum {
    owned: HashSet<u32>,
}

impl CardAlbum {
    /// One of the server's lines: true when it was one (`[[alb]]=` starts over, `[[alb]]+<id>,<id>` adds)
    pub fn apply(&mut self, line: &str) -> bool {
        let Some(rest) = line.strip_prefix(PREFIX) else {
            return false;
        };
        if rest == "=" {
            self.owned.clear();
        } else if let Some(ids) = rest.strip_prefix('+') {
            self.owned
                .extend(ids.split(',').filter_map(|id| id.trim().parse::<u32>().ok()));
        }
        true
    }

    pub fn has(&self, card: u32) -> bool {
        self.owned.contains(&card)
    }

    /// Cards of the album the account has had
    pub fn count(&self) -> usize {
        CARDS.iter().filter(|c| self.owned.contains(&c.0)).count()
    }

    /// The region's cards in, of those completing it (bosses' aside)
    pub fn region_count(&self, region: usize) -> usize {
        cards_of(region)
            .filter(|c| !c.boss && self.owned.contains(&c.id))
            .count()
    }

    pub fn region_done(&self, region: &Region, index: usize) -> bool {
        self.region_count(index) >= region.need
    }

    /// Everything earned, each kind's amount, as the server adds it up
    pub fn earned(&self) -> Vec<(Perk, u32)> {
        let count = self.count();
        let mut sum = [0u32; 7];
        for (at, perk, amount) in milestones() {
            if count >= at {
                sum[perk as usize] += amount;
            }
        }
        for (i, region) in regions().enumerate() {
            if self.region_done(&region, i) {
                sum[region.perk as usize] += region.amount;
            }
        }
        Perk::ALL
            .iter()
            .map(|&p| (p, sum[p as usize].min(p.cap())))
            .filter(|&(_, amount)| amount > 0)
            .collect()
    }

    /// The next reward by count: how many cards, and what it gives
    pub fn next_milestone(&self) -> Option<(usize, Perk, u32)> {
        let count = self.count();
        milestones().find(|&(at, _, _)| at > count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_lines_fill_the_album_and_start_it_over() {
        let mut album = CardAlbum::default();
        let first = CARDS[0].0;
        let second = CARDS[1].0;
        assert!(album.apply(&format!("[[alb]]+{first},{second}")));
        assert!(album.has(first) && album.has(second));
        assert_eq!(album.count(), 2);
        assert!(album.apply("[[alb]]="));
        assert_eq!(album.count(), 0);
        assert!(!album.apply("Card Album: Poring Card (1/673)"));
    }

    #[test]
    fn a_region_complete_and_the_counts_reached_earn_their_rewards() {
        let mut album = CardAlbum::default();
        assert!(album.earned().is_empty());
        let (index, region) = regions().enumerate().last().unwrap();
        for card in cards_of(index).filter(|c| !c.boss) {
            album.apply(&format!("[[alb]]+{}", card.id));
        }
        assert!(album.region_done(&region, index));
        let earned = album.earned();
        assert!(earned.contains(&(region.perk, region.amount)) || earned.iter().any(|&(p, _)| p == region.perk));
        let (at, _, _) = album.next_milestone().unwrap();
        assert!(at > album.count());
    }

    #[test]
    fn every_card_has_a_region_and_every_region_can_be_completed() {
        let regions: Vec<_> = regions().collect();
        assert_eq!(regions.len(), REGIONS.len(), "every reward kind is known");
        for (i, region) in regions.iter().enumerate() {
            assert_eq!(cards_of(i).filter(|c| !c.boss).count(), region.need, "{}", region.name);
        }
        assert!(CARDS.iter().all(|c| usize::from(c.1) < regions.len()));
    }
}
