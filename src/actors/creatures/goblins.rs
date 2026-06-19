use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sneaky melee skirmisher that doubles up its turn with a bonus-action
/// shortbow shot. Action: scimitar (close in and slash). Bonus: shortbow
/// (extra ranged ping). The action-economy split is the headline — most
/// creatures don't have a bonus-action attack option.
pub static GOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Goblin",
        glyph: 'G',
        ac: 15,
        hitpoints: "2d6".parse().unwrap(),
        strength: 8,
        dexterity: 14,
        constitution: 10,
        intelligence: 10,
        wisdom: 8,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
