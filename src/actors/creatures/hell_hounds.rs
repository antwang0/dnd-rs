use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HELL_HOUND_BITE, HELL_HOUND_FIRE_BREATH};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Hell Hound — CR 3 fiend. Mastiff-sized infernal hunter with two
/// signature lanes: a STR-based bite that smolders with a 1d6 fire
/// rider, and a 15-ft cone of fire breath (DC 12 DEX save). Fire-
/// immune (the breath belongs to them) and Charmed-immune (their
/// minds belong to their devil masters, not seducers). Speaks Infernal
/// per RAW; we tag it for completeness even though languages don't
/// fully drive gameplay yet.
pub static HELL_HOUND_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HELL_HOUND_BITE);
    actions.push(&*HELL_HOUND_FIRE_BREATH);
    CreatureTemplate {
        name: "Hell Hound",
        // 'h' (lowercase): Hippogriff uses uppercase 'H'; lowercase 'h'
        // is free. Visual reads as a small fiery dog on the map.
        glyph: 'h',
        ac: 15,
        // 9d8+18 = ~58 average per MM (CR 3).
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 50.,
        strength: 17,
        dexterity: 12,
        constitution: 14,
        intelligence: 6,
        wisdom: 13,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        // Fiends are immune to being charmed by mortals — keeps the
        // hound's pack discipline from being broken by Charm Person.
        condition_immunities: HashSet::from([Condition::Charmed]),
        ..CreatureTemplate::defaults()
    }
});
