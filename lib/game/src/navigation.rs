//! An NPC's navigation guide (0x08e2, `clif_navigateTo`, the `navigateto` script command): where it points, as
//! rAthena fills it and kRO's handler (0x7ff390) reads it before its navigation window (0x685be0) takes over

/// What the guide points at: rAthena's three kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTarget {
    /// A cell of a map (0)
    Cell { x: i16, y: i16 },
    /// A map, no cell (1; rAthena: "will fail if you are already on the map")
    Map,
    /// A monster on a map (3)
    Monster { mob_id: u16 },
}

impl NavTarget {
    /// rAthena's kinds, with what each carries; any other is none it sends
    pub fn from_packet(kind: u8, x: i16, y: i16, mob_id: u16) -> Option<Self> {
        match kind {
            0 => Some(Self::Cell { x, y }),
            1 => Some(Self::Map),
            3 => Some(Self::Monster { mob_id }),
            _ => None,
        }
    }
}

/// The ways of travel the guide may use, from `flag`'s decimal digits as kRO's handler splits them (0x7ff3b8): the
/// hundreds a Kafra, the tens a scroll, the ones an airship (rAthena's `NAV_KAFRA_ONLY` 100, `NAV_SCROLL_ONLY` 10,
/// `NAV_AIRSHIP_ONLY` 1; a digit above 1 counts as 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavServices {
    pub kafra: bool,
    pub scroll: bool,
    pub airship: bool,
}

impl NavServices {
    pub fn from_flag(flag: u8) -> Self {
        let kafra = flag >= 100;
        let rest = if kafra { flag - 100 } else { flag };
        let scroll = rest >= 10;
        let rest = if scroll { rest - 10 } else { rest };
        Self {
            kafra,
            scroll,
            airship: rest >= 1,
        }
    }
}

/// The guide the server last gave
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigation {
    pub map: String,
    pub target: NavTarget,
    pub services: NavServices,
    /// rAthena's `hideWindow`: kRO opens no navigation window (`navigateto`'s default)
    pub hide_window: bool,
}

/// How near the player must stand to a guide's cell for it to be done (ours: kRO's own end is its window's)
const ARRIVED_CELLS: u16 = 3;

impl Navigation {
    /// The guide's cell on `map`, when it points at one there
    pub fn cell_on(&self, map: &str) -> Option<(i16, i16)> {
        match self.target {
            NavTarget::Cell { x, y } if crate::same_map(&self.map, map) => Some((x, y)),
            _ => None,
        }
    }

    /// Whether the player, standing on `map` at `cell`, has reached the guide's cell
    pub fn reached(&self, map: &str, cell: (u16, u16)) -> bool {
        self.cell_on(map).is_some_and(|(x, y)| {
            cell.0
                .abs_diff(x.max(0) as u16)
                .max(cell.1.abs_diff(y.max(0) as u16))
                <= ARRIVED_CELLS
        })
    }
}


/// Where a route goes: a map, and on it a cell when there is one to reach
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub map: String,
    pub cell: Option<(u16, u16)>,
    /// What the HUD calls it ("Payon Field 08", "Talk to Emily")
    pub label: String,
}

/// One way out of a map on a route: walk onto the portal at `x, y`, or talk to `npc` standing there
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub map: &'static str,
    pub x: u16,
    pub y: u16,
    pub to: &'static str,
    pub npc: &'static str,
}

/// What the player is to head for on the map they stand on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// A portal or an NPC that leads on
    Way(Step),
    /// The destination's own cell
    Cell(u16, u16),
    /// Somewhere on the destination map: arrived
    Here,
}

/// The cells a portal costs over walking, so a route takes no needless map; and an NPC's, more than any walk: an NPC
/// may want zeny or a quest done first, so one is talked to only where no walk leads (a ship to an island)
const PORTAL_COST: u32 = 5;
const NPC_COST: u32 = 5000;
/// How far an arrival at a random cell (a warp to 0, 0) counts as from anywhere on that map
const RANDOM_ARRIVAL: u32 = 80;

fn cells_apart(a: (u16, u16), b: (u16, u16)) -> u32 {
    u32::from(a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)))
}

/// The links out of `map` (crate::nav_link_tables, by map)
fn links_from(map: &str) -> std::ops::Range<usize> {
    use crate::nav_link_tables::LINKS;
    let start = LINKS.partition_point(|l| l.0 < map);
    let end = LINKS.partition_point(|l| l.0 <= map);
    start..end
}

fn walk_cost(from: Option<(u16, u16)>, to: (u16, u16)) -> u32 {
    from.map_or(RANDOM_ARRIVAL, |f| cells_apart(f, to))
}

/// The ways from `from_map` (standing at `from`) to `dest`'s map, the shortest by cells walked; `Some(vec![])` when
/// already there, `None` when the server has no way there
pub fn plan(from_map: &str, from: (u16, u16), dest: &Destination) -> Option<Vec<Step>> {
    use crate::nav_link_tables::LINKS;
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let from_map = crate::map_key(from_map);
    let dest_map = crate::map_key(&dest.map);
    if from_map == dest_map {
        return Some(Vec::new());
    }
    let link_cost = |i: usize| if LINKS[i].6.is_empty() { PORTAL_COST } else { NPC_COST };
    let arrival = |i: usize| {
        let l = &LINKS[i];
        (l.4 != 0 || l.5 != 0).then_some((l.4, l.5))
    };
    // the links are the nodes, a link's cost what reaching and taking it took; usize::MAX the destination
    let mut best = vec![u32::MAX; LINKS.len()];
    let mut came_from = vec![usize::MAX; LINKS.len()];
    let mut heap = BinaryHeap::new();
    for i in links_from(&from_map) {
        let cost = cells_apart(from, (LINKS[i].1, LINKS[i].2)) + link_cost(i);
        if cost < best[i] {
            best[i] = cost;
            heap.push(Reverse((cost, i)));
        }
    }
    let mut done: Option<(u32, usize)> = None;
    while let Some(Reverse((cost, i))) = heap.pop() {
        if done.is_some_and(|(c, _)| c <= cost) {
            break;
        }
        if cost > best[i] {
            continue;
        }
        if LINKS[i].3 == dest_map {
            let total = cost + dest.cell.map_or(0, |c| walk_cost(arrival(i), c));
            if done.is_none_or(|(c, _)| total < c) {
                done = Some((total, i));
            }
            continue;
        }
        for j in links_from(LINKS[i].3) {
            let next = cost + walk_cost(arrival(i), (LINKS[j].1, LINKS[j].2)) + link_cost(j);
            if next < best[j] {
                best[j] = next;
                came_from[j] = i;
                heap.push(Reverse((next, j)));
            }
        }
    }
    let (_, mut i) = done?;
    let mut steps = Vec::new();
    loop {
        let l = &LINKS[i];
        steps.push(Step {
            map: l.0,
            x: l.1,
            y: l.2,
            to: l.3,
            npc: l.6,
        });
        if came_from[i] == usize::MAX {
            break;
        }
        i = came_from[i];
    }
    steps.reverse();
    Some(steps)
}

/// A route being followed: where to, and the ways there from the map it was last planned on
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub dest: Destination,
    pub steps: Vec<Step>,
    /// The map the steps start from
    pub planned_on: String,
    /// Walking there by itself
    pub auto_walk: bool,
}

impl Route {
    /// A route to `dest` from where the player stands; `None` when the server has no way there
    pub fn new(dest: Destination, map: &str, cell: (u16, u16)) -> Option<Self> {
        let steps = plan(map, cell, &dest)?;
        Some(Self {
            dest,
            steps,
            planned_on: crate::map_key(map),
            auto_walk: false,
        })
    }

    /// Planned again from a map the player came to (on the route or off it); false when there is no way on
    pub fn replan(&mut self, map: &str, cell: (u16, u16)) -> bool {
        if crate::same_map(map, &self.planned_on) {
            return true;
        }
        match plan(map, cell, &self.dest) {
            Some(steps) => {
                self.steps = steps;
                self.planned_on = crate::map_key(map);
                true
            }
            None => false,
        }
    }

    /// What to head for on `map`
    pub fn goal_on(&self, map: &str) -> Option<Goal> {
        if crate::same_map(map, &self.dest.map) {
            return Some(self.dest.cell.map_or(Goal::Here, |(x, y)| Goal::Cell(x, y)));
        }
        self.steps
            .iter()
            .find(|s| crate::same_map(s.map, map))
            .map(|s| Goal::Way(*s))
    }

    /// Whether the player, on `map` at `cell`, is there
    pub fn arrived(&self, map: &str, cell: (u16, u16)) -> bool {
        match self.goal_on(map) {
            Some(Goal::Here) => true,
            Some(Goal::Cell(x, y)) => cells_apart(cell, (x, y)) <= u32::from(ARRIVED_CELLS),
            _ => false,
        }
    }

    /// Maps still to cross, this one's way included
    pub fn maps_left(&self, map: &str) -> usize {
        self.steps
            .iter()
            .position(|s| crate::same_map(s.map, map))
            .map_or(0, |i| self.steps.len() - i)
    }
}

/// A compass word for the way from `from` to `to` on the map (north is up, +y)
pub fn compass(from: (u16, u16), to: (u16, u16)) -> &'static str {
    let dx = f32::from(to.0) - f32::from(from.0);
    let dy = f32::from(to.1) - f32::from(from.1);
    if dx.abs() < 1.0 && dy.abs() < 1.0 {
        return "here";
    }
    let octant = ((dy.atan2(dx).to_degrees() + 360.0 + 22.5) % 360.0 / 45.0) as usize;
    ["east", "north-east", "north", "north-west", "west", "south-west", "south", "south-east"][octant.min(7)]
}

/// The cells from `from` to a cell within `near` of `to`, 8-way as RO walks (a diagonal only past two open sides),
/// the start left out: the whole way across a map, which the client's own search (a short A*) does not go. Empty
/// when nothing leads there, or already there.
pub fn walk_path(
    width: i32,
    height: i32,
    walkable: impl Fn(i32, i32) -> bool,
    from: (u16, u16),
    to: (u16, u16),
    near: u16,
) -> Vec<(u16, u16)> {
    let (w, h) = (width.max(0) as usize, height.max(0) as usize);
    let at = |x: i32, y: i32| y as usize * w + x as usize;
    let done = |x: i32, y: i32| {
        (x - i32::from(to.0)).unsigned_abs().max((y - i32::from(to.1)).unsigned_abs()) <= u32::from(near)
    };
    let start = (i32::from(from.0), i32::from(from.1));
    if start.0 >= width || start.1 >= height || done(start.0, start.1) {
        return Vec::new();
    }
    let mut came = vec![u32::MAX; w * h];
    came[at(start.0, start.1)] = at(start.0, start.1) as u32;
    let mut queue = std::collections::VecDeque::from([start]);
    let open = |x: i32, y: i32| x >= 0 && y >= 0 && x < width && y < height && walkable(x, y);
    while let Some((x, y)) = queue.pop_front() {
        if done(x, y) {
            let mut path = Vec::new();
            let mut i = at(x, y);
            while i != at(start.0, start.1) {
                path.push(((i % w) as u16, (i / w) as u16));
                i = came[i] as usize;
            }
            path.reverse();
            return path;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if !open(nx, ny) || came[at(nx, ny)] != u32::MAX {
                continue;
            }
            if dx != 0 && dy != 0 && !(open(x + dx, y) && open(x, y + dy)) {
                continue;
            }
            came[at(nx, ny)] = at(x, y) as u32;
            queue.push_back((nx, ny));
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kinds_rathena_sends_and_no_other() {
        assert_eq!(
            NavTarget::from_packet(0, 252, 297, 0),
            Some(NavTarget::Cell { x: 252, y: 297 })
        );
        assert_eq!(NavTarget::from_packet(1, 0, 0, 0), Some(NavTarget::Map));
        assert_eq!(
            NavTarget::from_packet(3, 0, 0, 1002),
            Some(NavTarget::Monster { mob_id: 1002 })
        );
        assert_eq!(NavTarget::from_packet(2, 1, 1, 0), None);
    }

    #[test]
    fn the_flag_splits_by_its_digits() {
        let s = NavServices::from_flag;
        assert_eq!(s(0), NavServices::default());
        assert_eq!(
            s(101),
            NavServices {
                kafra: true,
                scroll: false,
                airship: true
            },
            "NAV_KAFRA_AND_AIRSHIP, navigateto's default"
        );
        assert_eq!(
            s(110),
            NavServices {
                kafra: true,
                scroll: true,
                airship: false
            }
        );
        assert_eq!(
            s(255),
            NavServices {
                kafra: true,
                scroll: true,
                airship: true
            },
            "111 to 255 are all"
        );
        assert_eq!(s(19), s(11), "a digit above 1 counts as 1");
    }

    #[test]
    fn same_map_keys_names_as_map_key_does_without_building_keys() {
        for (a, b) in [
            ("payon", "PAYON.gat"),
            ("payon.rsw", " payon "),
            ("prt_fild08", "prt_fild08.GAT"),
        ] {
            assert!(crate::same_map(a, b), "{a} {b}");
            assert_eq!(crate::map_key(a), crate::map_key(b));
        }
        assert!(!crate::same_map("payon", "payon2"));
        assert!(!crate::same_map(".gat", "x"));
        assert!(
            !crate::same_map("가.gat", "가가"),
            "no panic on a multi-byte name"
        );
    }

    #[test]
    fn a_guide_is_reached_within_three_cells_of_its_cell_on_its_map() {
        let nav = Navigation {
            map: "payon".into(),
            target: NavTarget::Cell { x: 252, y: 297 },
            services: NavServices::default(),
            hide_window: true,
        };
        assert_eq!(nav.cell_on("payon.gat"), Some((252, 297)));
        assert!(nav.reached("payon", (250, 299)));
        assert!(!nav.reached("payon", (248, 297)));
        assert!(!nav.reached("prontera", (252, 297)));
    }

    fn dest(map: &str, cell: Option<(u16, u16)>) -> Destination {
        Destination {
            map: map.into(),
            cell,
            label: map.into(),
        }
    }

    #[test]
    fn a_route_crosses_the_fields_between_two_towns_by_their_portals() {
        let steps = plan("prontera", (156, 191), &dest("payon", None)).expect("a way to Payon");
        assert_eq!(steps.first().map(|s| s.map), Some("prontera"));
        assert_eq!(steps.last().map(|s| s.to), Some("payon"));
        for pair in steps.windows(2) {
            assert_eq!(pair[0].to, pair[1].map, "each step leaves the map the last one led to");
        }
        assert!(steps.iter().all(|s| s.npc.is_empty()), "walked, no NPC: {steps:?}");
    }

    #[test]
    fn already_there_is_no_step_and_nowhere_is_none() {
        assert_eq!(plan("payon.gat", (1, 1), &dest("PAYON", None)), Some(vec![]));
        assert_eq!(plan("prontera", (156, 191), &dest("no_such_map", None)), None);
    }

    #[test]
    fn a_route_says_what_to_head_for_and_when_it_is_done() {
        let route = Route::new(dest("prt_fild08", Some((100, 100))), "prontera", (156, 191)).unwrap();
        assert!(matches!(route.goal_on("prontera"), Some(Goal::Way(s)) if s.map == "prontera"));
        assert_eq!(route.goal_on("prt_fild08"), Some(Goal::Cell(100, 100)));
        assert!(route.arrived("prt_fild08", (102, 98)));
        assert!(!route.arrived("prt_fild08", (110, 100)));
        assert_eq!(route.goal_on("geffen"), None, "off the route: replanned on arrival");
    }

    #[test]
    fn the_compass_points_with_north_up() {
        assert_eq!(compass((100, 100), (100, 150)), "north");
        assert_eq!(compass((100, 100), (150, 100)), "east");
        assert_eq!(compass((100, 100), (60, 60)), "south-west");
        assert_eq!(compass((100, 100), (100, 100)), "here");
    }

    #[test]
    fn a_walk_goes_round_a_wall_and_stops_by_a_cell_it_cannot_stand_on() {
        // a wall at x = 5 from y = 0 to 8, open above
        let open = |x: i32, y: i32| !(x == 5 && y <= 8) && !(x == 9 && y == 2);
        let path = walk_path(10, 10, open, (2, 2), (8, 2), 0);
        assert_eq!(path.last(), Some(&(8, 2)));
        assert!(path.iter().all(|&(x, y)| open(i32::from(x), i32::from(y))));
        assert!(path.iter().any(|&(_, y)| y == 9), "round the wall's end");
        let by = walk_path(10, 10, open, (8, 0), (9, 2), 1);
        assert_eq!(by.last().map(|&(x, y)| x.abs_diff(9).max(y.abs_diff(2))), Some(1));
        assert!(walk_path(10, 10, open, (2, 2), (2, 3), 1).is_empty(), "already there");
    }
}
