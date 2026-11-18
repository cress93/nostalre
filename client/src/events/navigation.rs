//! The navigation (`nostalre_game::navigation::Route`): a route to a map picked on the world map, a quest's place
//! or an NPC's guide, planned over the server's portals and followed map by map: the HUD says what to head for, the
//! minimap marks it, and the player may let it walk there.
use std::time::{Duration, Instant};

use crate::App;
use nostalre_game::entity::EntityState;
use nostalre_game::navigation::{Destination, Goal, Route, walk_path};
use nostalre_network::build_request_move_packet;

/// How far along its path auto-walk sends each walk (the client's own search reaches some 14 cells)
const WALK_AHEAD: usize = 12;
/// How often auto-walk may ask again while the last walk is not done
const WALK_EVERY: Duration = Duration::from_millis(400);
/// How near an NPC to stop: one may talk to it from there
const NPC_NEAR: u16 = 2;

/// Auto-walk's path across the map it is on, and when it last asked to walk
#[derive(Default)]
pub struct RouteWalk {
    /// The map and cell the path leads to, and the path
    path: Option<(String, (u16, u16), Vec<(u16, u16)>)>,
    last: Option<Instant>,
}

impl App {
    fn player_cell(&self) -> Option<(u16, u16)> {
        self.game
            .world
            .entities
            .player()
            .map(|e| e.movement.cell_position())
    }

    pub(crate) fn map_display_name(&self, map: &str) -> String {
        self.game
            .data_table
            .map_name
            .as_ref()
            .and_then(|t| t.display_name(map))
            .map(|n| n.split(" - ").next().unwrap_or(n).trim().to_string())
            .unwrap_or_else(|| map.to_string())
    }

    /// A route to `dest` from where the player stands, told in chat; none when the server has no way there
    pub(super) fn start_route(&mut self, dest: Destination) {
        let (Some(map), Some(cell)) = (self.game.session.current_map.clone(), self.player_cell()) else {
            return;
        };
        let label = dest.label.clone();
        match Route::new(dest, &map, cell) {
            Some(route) => {
                let line = match route.steps.len() {
                    0 => format!("Navigation: {label}, on this map"),
                    1 => format!("Navigation: {label}, the next map"),
                    n => format!("Navigation: {label}, {n} maps away"),
                };
                self.windows.chat_window.add_system(line);
                self.game.route = Some(route);
                self.game.route_walk = RouteWalk::default();
            }
            None => {
                self.windows
                    .chat_window
                    .add_system(format!("Navigation: no known way to {label} from here"));
            }
        }
    }

    pub(super) fn stop_route(&mut self) {
        self.game.route = None;
        self.game.route_walk = RouteWalk::default();
    }

    /// The player came to `map`: the route planned on from there
    pub(super) fn route_on_map(&mut self, map: &str, x: i16, y: i16) {
        let Some(route) = &mut self.game.route else {
            return;
        };
        self.game.route_walk.path = None;
        if !route.replan(map, (x.max(0) as u16, y.max(0) as u16)) {
            let label = route.dest.label.clone();
            self.stop_route();
            self.windows
                .chat_window
                .add_system(format!("Navigation: no known way to {label} from here, stopped"));
        }
    }

    /// Each frame: done once there, and auto-walk's next walk
    pub(crate) fn update_route(&mut self) {
        let (Some(map), Some(cell)) = (self.game.session.current_map.clone(), self.player_cell()) else {
            return;
        };
        let Some(route) = &self.game.route else {
            return;
        };
        if route.arrived(&map, cell) {
            let line = format!("Navigation: arrived at {}", route.dest.label);
            self.stop_route();
            self.windows.chat_window.add_system(line);
            return;
        }
        if route.auto_walk {
            self.auto_walk(&map, cell);
        }
    }

    pub(super) fn set_route_auto_walk(&mut self, on: bool) {
        if let Some(route) = &mut self.game.route {
            route.auto_walk = on;
            self.game.route_walk = RouteWalk::default();
        }
    }

    /// Manual walking takes over from auto-walk
    pub(crate) fn route_walk_interrupted(&mut self) {
        if let Some(route) = &mut self.game.route {
            route.auto_walk = false;
        }
    }

    fn auto_walk(&mut self, map: &str, cell: (u16, u16)) {
        let Some(route) = &self.game.route else {
            return;
        };
        let (target, near, npc) = match route.goal_on(map) {
            Some(Goal::Way(step)) if step.npc.is_empty() => ((step.x, step.y), 0, None),
            Some(Goal::Way(step)) => ((step.x, step.y), NPC_NEAR, Some(step.npc)),
            Some(Goal::Cell(x, y)) => ((x, y), 1, None),
            Some(Goal::Here) | None => return,
        };
        // by the NPC: the player talks to it
        if let Some(npc) = npc
            && cell.0.abs_diff(target.0).max(cell.1.abs_diff(target.1)) <= NPC_NEAR
        {
            self.set_route_auto_walk(false);
            self.windows
                .chat_window
                .add_system(format!("Navigation: talk to {npc}"));
            return;
        }
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        if player.state() == EntityState::Dead
            || self.windows.npc_dialog.dialog.is_open()
            || self.windows.npc_shop.shop.is_open()
        {
            return;
        }
        let moving = player.movement.is_moving();
        let now = Instant::now();
        if moving
            && self
                .game
                .route_walk
                .last
                .is_some_and(|at| now.duration_since(at) < WALK_EVERY)
        {
            return;
        }
        // a walk ahead is still under way and far from done: let it go on
        if moving
            && player
                .movement
                .destination()
                .is_some_and(|d| d.0.abs_diff(cell.0).max(d.1.abs_diff(cell.1)) > 4)
        {
            return;
        }
        let Some(gat) = &self.game.session.gat else {
            return;
        };
        let fresh = self
            .game
            .route_walk
            .path
            .as_ref()
            .is_some_and(|(m, t, path)| m == map && *t == target && path.contains(&cell));
        if !fresh {
            let path = walk_path(
                gat.width,
                gat.height,
                |x, y| gat.is_walkable(x, y),
                cell,
                target,
                near,
            );
            if path.is_empty() {
                self.set_route_auto_walk(false);
                self.windows
                    .chat_window
                    .add_system("Navigation: no walk leads there from here, auto-walk stopped".into());
                return;
            }
            self.game.route_walk.path = Some((map.to_string(), target, path));
        }
        let Some((_, _, path)) = &self.game.route_walk.path else {
            return;
        };
        let from = path.iter().position(|&c| c == cell).map_or(0, |i| i + 1);
        for ahead in [WALK_AHEAD, 8, 4, 1] {
            let Some(&(x, y)) = path.get((from + ahead).min(path.len()) - 1) else {
                continue;
            };
            if let Some(step) =
                nostalre_game::path::try_move_to(gat, cell.0, cell.1, i32::from(x), i32::from(y))
            {
                self.channel.send_packet(build_request_move_packet(
                    step.dest_x,
                    step.dest_y,
                    self.active_packetver,
                ));
                self.game.route_walk.last = Some(now);
                return;
            }
        }
    }
}
