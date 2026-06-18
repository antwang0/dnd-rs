use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bandit — generic mook with both melee and ranged options. Scimitar
/// for in-melee swings, heavy crossbow for ranged pressure. Decent AC
/// from leather armor, modest HP. CR 1/8.
pub static BANDIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*HEAVY_CROSSBOW);
    CreatureTemplate {
        name: "Bandit",
        glyph: 'B',
        ac: 12,
        hitpoints: "2d8".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 10,
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
