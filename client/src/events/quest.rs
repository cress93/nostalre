use crate::App;
use nostalre_game::quest::{MARKER_NONE, Quest, QuestHuntEntry, QuestMarker, marker_sprite_action};
use nostalre_ui_component::game::chat_window::ChatChannel;

impl App {
    pub(super) fn handle_quest_list_received(&mut self, quests: Vec<Quest>) {
        self.game.quest_log.replace(quests);
        // the server's list knows nothing of the quest board's
        self.board_into_quest_log();
    }

    /// The quest board's quests taken (nostalRE's server) as quest log entries, first in it, so the quest window and
    /// the HUD's list track them like any quest
    pub(crate) fn board_into_quest_log(&mut self) {
        use nostalre_game::questboard::{QUEST_ID, SLOTS};
        let data = &self.game.data_table;
        let entries = self
            .game
            .character
            .board
            .log_entries(|id| data.item_name_for(id, true));
        let board_ids = QUEST_ID..QUEST_ID + SLOTS as u32;
        self.game.quest_log.quests.retain(|q| !board_ids.contains(&q.id));
        for (i, (quest, title, summary, description)) in entries.into_iter().enumerate() {
            if let Some(display) = self.game.data_table.quest_display.as_mut() {
                display.insert(quest.id, title, summary, description);
            }
            self.game.quest_log.quests.insert(i, quest);
        }
    }

    pub(super) fn handle_quest_added(&mut self, quest: Quest) {
        let title = self
            .game
            .data_table
            .quest_display
            .as_ref()
            .map(|t| t.title(quest.id))
            .unwrap_or_else(|| "Unknown Quest".to_string());
        self.game.quest_log.add(quest);
        self.windows.chat_window.add_message(
            format!("Quest added : {title}"),
            [1.0, 1.0, 1.0, 1.0],
            ChatChannel::Public,
        );
    }

    pub(super) fn handle_quest_removed(&mut self, quest_id: u32) {
        self.game.quest_log.remove(quest_id);
    }

    pub(super) fn handle_quest_hunt_updated(&mut self, entries: Vec<QuestHuntEntry>) {
        for entry in entries {
            let before = self
                .game
                .quest_log
                .objective(entry.hunt_id)
                .map(|(quest_id, o)| (quest_id, o.current, o.name.clone()));
            self.game.quest_log.update_hunt(entry);
            if let Some((quest_id, old_current, name)) = before
                && entry.current > old_current
            {
                let title = self
                    .game
                    .data_table
                    .quest_display
                    .as_ref()
                    .map(|t| t.title(quest_id))
                    .unwrap_or_default();
                self.windows.chat_window.add_message(
                    format!(
                        "Mission [{title}], you killed {name}. ({}/{})",
                        entry.current, entry.required
                    ),
                    [1.0, 1.0, 1.0, 1.0],
                    ChatChannel::Public,
                );
            }
        }
    }

    pub(super) fn handle_quest_active_changed(&mut self, quest_id: u32, active: bool) {
        self.game.quest_log.set_active(quest_id, active);
    }

    pub(super) fn handle_quest_npc_marker(
        &mut self,
        npc_id: u32,
        x: u16,
        y: u16,
        effect: i16,
        color: u8,
    ) {
        if effect == MARKER_NONE {
            self.game.quest_markers.remove(&npc_id);
        } else {
            if marker_sprite_action(effect).is_none() {
                tracing::debug!(
                    npc_id,
                    effect,
                    "quest marker icon kRO draws with another effect, not drawn"
                );
            }
            self.game.quest_markers.insert(
                npc_id,
                QuestMarker {
                    x,
                    y,
                    effect,
                    color,
                },
            );
        }
    }
}
