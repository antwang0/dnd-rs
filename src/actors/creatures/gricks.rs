use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GRICK_TENTACLES_WEAPON, GRICK_BEAK};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Grick — CR 2 monstrosity. Worm-like ambush predator that lurks in
/// caverns. Tentacles deal 2d6+2 slashing, beak deals 1d6+2 piercing.
/// Resistant to bludgeoning/piercing/slashing from nonmagical attacks
/// (we model as resistance to all three physical types). AC 14, ~27 HP
/// (6d8). Darkvision 60ft.
pub static GRICK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GRICK_TENTACLES_WEAPON);
    actions.push(&*GRICK_BEAK);
    CreatureTemplate {
        name: "Grick",
        glyph: 'ğ',
        ac: 14,
        hitpoints: "6d8".parse().unwrap(),
        strength: 14,
        dexterity: 14,
        constitution: 11,
        intelligence: 3,
        wisdom: 14,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        damage_modifiers: non_magical_physical_resistances([]),
        ..CreatureTemplate::defaults()
    }
});
