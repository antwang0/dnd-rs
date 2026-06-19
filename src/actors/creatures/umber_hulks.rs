use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::UMBER_CLAW;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Umber Hulk — CR 5 monstrosity. Heavily armoured burrowing predator
/// with massive claws and confusing gaze (gaze not yet modelled).
/// High AC (18) from its thick carapace, strong STR-based claw attacks.
/// Darkvision 120ft and Tremorsense 60ft make it a subterranean ambusher.
pub static UMBER_HULK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&UMBER_CLAW);
    CreatureTemplate {
        name: "Umber Hulk",
        glyph: 'U',
        ac: 18,
        hitpoints: "12d10+48".parse().unwrap(),
        strength: 20,
        dexterity: 13,
        constitution: 18,
        intelligence: 9,
        wisdom: 10,
        charisma: 10,
        senses: HashSet::from([
            SpecialSense::Darkvision(120),
            SpecialSense::Tremorsense(60),
        ]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
