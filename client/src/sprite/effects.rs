use crate::App;
use nostalre_formats::grf::GrfArchive;
use nostalre_game::sprite_loader;

/// A skin's sprite by its path without the extension
fn load_skin_sprite(
    grf: &GrfArchive,
    path: &str,
) -> Option<nostalre_game::sprite_loader::SpriteData> {
    sprite_loader::load_sprite_data(grf, &format!("{path}.spr"), &format!("{path}.act"))
}

impl App {
    pub(crate) fn load_emotion_sprite(&mut self, grf: &GrfArchive) {
        if let Some(sprite_data) = sprite_loader::load_emotion_sprite(grf)
            && let Some(textures) = self.upload_sprite(&sprite_data)
        {
            self.game.assets.emotion_textures = Some(textures);
            self.game.assets.emotion_act = Some(sprite_data.act);
        }
    }

    pub(crate) fn load_status_overlay_sprites(&mut self, grf: &GrfArchive) {
        for overlay in nostalre_game::ailment::AilmentOverlay::ALL {
            if let Some(sprite_data) = sprite_loader::load_status_overlay_sprite(grf, overlay)
                && let Some(textures) = self.upload_sprite(&sprite_data)
            {
                self.game
                    .assets
                    .status_overlay_sprites
                    .insert(overlay, (textures, sprite_data.act));
            }
        }
    }

    /// The chosen damage skin's number and message sprites (kRO's default, `숫자` and `msg`, without the skin tables)
    pub(crate) fn load_damage_skin_sprites(&mut self, grf: &GrfArchive) {
        let skin = self
            .game
            .data_table
            .damage_skins
            .get(self.config.damage_skin as usize);
        let (number, msg) = match skin {
            Some(skin) => (
                load_skin_sprite(grf, &skin.number),
                load_skin_sprite(grf, &skin.msg),
            ),
            None => (
                sprite_loader::load_damage_number_sprite(grf),
                sprite_loader::load_damage_miss_msg_sprite(grf),
            ),
        };
        if let Some(sprite_data) = number
            && let Some(textures) = self.upload_glyph_sprite(&sprite_data)
        {
            self.game.assets.damage_number_textures = Some(textures);
            self.game.assets.damage_number_act = Some(sprite_data.act);
        }
        if let Some(sprite_data) = msg
            && let Some(textures) = self.upload_glyph_sprite(&sprite_data)
        {
            self.game.assets.damage_msg_textures = Some(textures);
            self.game.assets.damage_msg_act = Some(sprite_data.act);
        }
    }

    pub(crate) fn load_damage_sprites(&mut self, grf: &GrfArchive) {
        self.load_damage_skin_sprites(grf);
        if let Some(sprite_data) = sprite_loader::load_rank_font_sprite(grf)
            && let Some(textures) = self.upload_glyph_sprite(&sprite_data)
        {
            self.game.assets.rank_font_textures = Some(textures);
            self.game.assets.rank_font_act = Some(sprite_data.act);
        }
        if let Some(sprite_data) = sprite_loader::load_time_font_sprite(grf)
            && let Some(textures) = self.upload_glyph_sprite(&sprite_data)
        {
            self.game.assets.time_font_textures = Some(textures);
            self.game.assets.time_font_act = Some(sprite_data.act);
        }
    }

    pub(crate) fn preload_item_icons(&mut self, icon_paths: Vec<String>) {
        // a monster's portrait is no file: it is built from its sprite
        let (portraits, icon_paths): (Vec<String>, Vec<String>) = icon_paths
            .into_iter()
            .partition(|p| nostalre_game::capture::portrait_mob(p).is_some());
        for key in portraits {
            self.build_mob_portrait(&key);
        }
        if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
            let icon_refs: Vec<&str> = icon_paths.iter().map(|s| s.as_str()).collect();
            renderer.preload_textures(&icon_refs, grf);
        }
    }

    /// A monster's portrait for the menu (`nostalre_game::capture::portrait_key`): the first frame of its standing
    /// pose, as its sprite has it
    fn build_mob_portrait(&mut self, key: &str) {
        let Some(mob_id) = nostalre_game::capture::portrait_mob(key) else {
            return;
        };
        let (Some(grf), Some(renderer), Some(names)) =
            (&self.grf, &mut self.renderer, &self.game.data_table.name)
        else {
            return;
        };
        let Some(sprite) = sprite_loader::load_entity_sprite_data(grf, names, mob_id) else {
            tracing::warn!("No sprite for the portrait of monster {mob_id}");
            return;
        };
        let body = &sprite.body;
        let index = body
            .act
            .actions
            .first()
            .and_then(|action| action.motions.first())
            .and_then(|motion| motion.clips.iter().find(|clip| clip.sprite_index >= 0))
            .map_or(0, |clip| {
                let rgba = usize::from(clip.sprite_type == 1);
                clip.sprite_index as usize + rgba * body.indexed_count
            });
        let Some(image) = body.images.get(index).or_else(|| body.images.first()) else {
            return;
        };
        let (w, h) = (image.width, image.height);
        renderer.register_rgba_texture(key, &image.data, w, h);
        self.windows.menu.portrait_built(key, w, h);
    }
}
