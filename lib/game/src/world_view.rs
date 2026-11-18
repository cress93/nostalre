//! kRO's world maps (the 2020 world map window): every world (Midgard, Midgard North, Far-Star, Dimension,
//! Isgard...), its picture, where each of its fields lies on it with the level kRO suggests, and its dungeons'
//! markers with their level ranges and floors. From `worldviewdata_list` (the worlds) and `worldviewdata_table`
//! (each world's `worldtable_*` fields and `*_Dun` markers), compiled Lua or, with the English text, Lua source.

use std::collections::HashMap;

use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_source::{self, LuaValue};
use nostalre_formats::lub::{self, LuaState, Value};

const DIR: &str = "data/luafiles514/lua files/worldviewdata/";

/// A field (or town) on a world: its map, its rectangle on the picture, the level kRO suggests for it
#[derive(Debug, Clone, PartialEq)]
pub struct WorldField {
    pub map: String,
    pub rect: (u16, u16, u16, u16),
    pub level: String,
}

/// A dungeon's marker on a world: its rectangle, the level range kRO gives it ("110~130"), and its floors with
/// theirs
#[derive(Debug, Clone, PartialEq)]
pub struct WorldDungeon {
    pub rect: (u16, u16, u16, u16),
    pub level: String,
    pub floors: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub name: String,
    /// The picture's texture path
    pub image: String,
    pub fields: Vec<WorldField>,
    pub dungeons: Vec<WorldDungeon>,
}

impl World {
    /// The field under a pixel of the picture (the smallest, where they overlap)
    pub fn field_at(&self, x: u16, y: u16) -> Option<&WorldField> {
        self.fields
            .iter()
            .filter(|f| inside(f.rect, x, y))
            .min_by_key(|f| u32::from(f.rect.2 - f.rect.0) * u32::from(f.rect.3 - f.rect.1))
    }

    pub fn dungeon_at(&self, x: u16, y: u16) -> Option<usize> {
        self.dungeons.iter().position(|d| inside(d.rect, x, y))
    }

    /// The world a map lies on (a field, or a dungeon's floor)
    pub fn has_map(&self, map: &str) -> bool {
        let key = crate::map_key(map);
        self.fields.iter().any(|f| f.map == key)
            || self
                .dungeons
                .iter()
                .any(|d| d.floors.iter().any(|(m, _)| *m == key))
    }
}

fn inside((l, t, r, b): (u16, u16, u16, u16), x: u16, y: u16) -> bool {
    x >= l && x <= r && y >= t && y <= b
}

/// A value read from either kind of file
enum Cell {
    Num(f64),
    Str(String),
}

/// A table's rows (positional, from 1), each a row of cells (positional, from 1)
type Rows = Vec<Vec<Option<Cell>>>;

/// The rows of global table `name`, from a compiled chunk's state
fn compiled_rows(state: &LuaState, name: &str) -> Option<Rows> {
    let table = state.global_table(name)?;
    let mut rows = Vec::new();
    for i in 1.. {
        let Some(row) = table
            .iter()
            .find(|(k, _)| k.as_number() == Some(i as f64))
            .and_then(|(_, v)| state.table(v))
        else {
            break;
        };
        let mut cells = Vec::new();
        for j in 1..=8 {
            let cell = row
                .iter()
                .find(|(k, _)| k.as_number() == Some(j as f64))
                .and_then(|(_, v)| match v {
                    Value::Number(n) => Some(Cell::Num(*n)),
                    Value::Str(s) => Some(Cell::Str(String::from_utf8_lossy(s).into_owned())),
                    _ => None,
                });
            cells.push(cell);
        }
        rows.push(cells);
    }
    Some(rows)
}

/// The rows of global table `name`, from Lua source
fn source_rows(globals: &HashMap<String, LuaValue>, name: &str) -> Option<Rows> {
    let LuaValue::Table(table) = globals.get(name)? else {
        return None;
    };
    let rows = table
        .entries
        .iter()
        .filter_map(|(_, v)| match v {
            LuaValue::Table(row) => Some(
                row.entries
                    .iter()
                    .map(|(_, c)| match c {
                        LuaValue::Number(n) => Some(Cell::Num(*n)),
                        LuaValue::Str(s) => Some(Cell::Str(s.clone())),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    Some(rows)
}

/// A file's tables, whichever kind it is
enum Source {
    Compiled(LuaState),
    Text(HashMap<String, LuaValue>),
}

impl Source {
    fn read(grf: &GrfArchive, file: &str) -> Option<Self> {
        Self::parse(&grf.read_file(&format!("{DIR}{file}")).ok()?)
    }

    fn parse(data: &[u8]) -> Option<Self> {
        if lub::is_compiled_chunk(data) {
            let mut state = LuaState::new();
            lub::load_chunk(data, &mut state).ok()?;
            Some(Self::Compiled(state))
        } else {
            Some(Self::Text(lua_source::globals(data)))
        }
    }

    fn rows(&self, name: &str) -> Option<Rows> {
        match self {
            Self::Compiled(state) => compiled_rows(state, name),
            Self::Text(globals) => source_rows(globals, name),
        }
    }
}

fn num(row: &[Option<Cell>], i: usize) -> Option<f64> {
    match row.get(i)? {
        Some(Cell::Num(n)) => Some(*n),
        _ => None,
    }
}

fn text(row: &[Option<Cell>], i: usize) -> String {
    match row.get(i) {
        Some(Some(Cell::Str(s))) => s.clone(),
        _ => String::new(),
    }
}

fn rect(row: &[Option<Cell>], from: usize) -> Option<(u16, u16, u16, u16)> {
    Some((
        num(row, from)? as u16,
        num(row, from + 1)? as u16,
        num(row, from + 2)? as u16,
        num(row, from + 3)? as u16,
    ))
}

/// The worlds, in kRO's order; empty when the game files have none of this
pub fn load(grf: &GrfArchive, ui_dir: &str) -> Vec<World> {
    let (Some(list), Some(table)) = (
        Source::read(grf, "worldviewdata_list.lub"),
        Source::read(grf, "worldviewdata_table.lub"),
    ) else {
        tracing::warn!("No world view data: the world map shows Midgard's old picture only");
        return Vec::new();
    };
    let worlds = worlds(&list, &table, ui_dir);
    tracing::info!("Loaded {} world maps", worlds.len());
    worlds
}

/// The worlds from the two files' bytes
pub fn parse(list: &[u8], table: &[u8], ui_dir: &str) -> Vec<World> {
    match (Source::parse(list), Source::parse(table)) {
        (Some(list), Some(table)) => worlds(&list, &table, ui_dir),
        _ => Vec::new(),
    }
}

fn worlds(list: &Source, table: &Source, ui_dir: &str) -> Vec<World> {
    let mut worlds = Vec::new();
    for entry in list.rows("World_List").unwrap_or_default() {
        let (name, fields_table, dungeons_table, image) = (
            text(&entry, 0),
            text(&entry, 1),
            text(&entry, 2),
            text(&entry, 3),
        );
        if name.is_empty() || image.is_empty() {
            continue;
        }
        // a group under 100 is a dungeon's floors (its marker has the same number), from 100 a region of fields
        let mut fields = Vec::new();
        let mut floors: HashMap<u32, Vec<(String, String)>> = HashMap::new();
        for row in table.rows(&fields_table).unwrap_or_default() {
            let (Some(group), Some(r)) = (num(&row, 0), rect(&row, 2)) else {
                continue;
            };
            let map = crate::map_key(&text(&row, 1));
            let level = text(&row, 7);
            if group < 100.0 {
                floors.entry(group as u32).or_default().push((map, level));
            } else {
                fields.push(WorldField {
                    map,
                    rect: r,
                    level,
                });
            }
        }
        let dungeons = table
            .rows(&dungeons_table)
            .unwrap_or_default()
            .iter()
            .filter_map(|row| {
                let id = num(row, 0)? as u32;
                Some(WorldDungeon {
                    rect: rect(row, 1)?,
                    level: text(row, 6),
                    floors: floors.remove(&id).unwrap_or_default(),
                })
            })
            .filter(|d| !d.floors.is_empty())
            .collect();
        worlds.push(World {
            name,
            image: format!("{ui_dir}{image}"),
            fields,
            dungeons,
        });
    }
    worlds
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's own files, when a copy is at hand (WORLDVIEW_DIR): every world has fields, Midgard dungeons
    #[test]
    fn the_game_files_read() {
        let Ok(dir) = std::env::var("WORLDVIEW_DIR") else {
            return;
        };
        let read = |f: &str| std::fs::read(format!("{dir}/{f}")).unwrap();
        let worlds = parse(&read("list.lub"), &read("table.lub"), "ui/");
        for w in &worlds {
            eprintln!(
                "{} {} fields {} dungeons {}",
                w.name,
                w.image,
                w.fields.len(),
                w.dungeons.len()
            );
        }
        assert_eq!(worlds.len(), 10);
        assert!(
            worlds[0]
                .dungeons
                .iter()
                .any(|d| d.floors.iter().any(|(m, _)| m == "tha_t01"))
        );
        assert!(worlds.iter().all(|w| !w.fields.is_empty()));
    }
}
