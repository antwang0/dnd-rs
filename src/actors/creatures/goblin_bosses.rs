use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GOBLIN_BOSS_MULTI, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Goblin Boss — 5e MM CR 1, the tougher cousin of the standard goblin.
/// Multiattack: two scimitar swings per Action, a real upgrade from the
/// regular goblin's single swing. Higher AC (chain shirt + shield) and
/// more HP make this the closest thing the codebase has to a "miniboss"
/// — hard enough to break a low-CR encounter open without a full party.
pub static GOBLIN_BOSS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GOBLIN_BOSS_MULTI);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Goblin Boss",
        glyph: 'B',
        ac: 17, // chain shirt + shield
        hitpoints: "6d6+6".parse().unwrap(),
        strength: 10,
        dexterity: 14,
        constitution: 10,
        intelligence: 10,
        wisdom: 8,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 1.0,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
