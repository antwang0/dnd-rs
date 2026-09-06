use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::TINY_ANIMATED_OBJECT_SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Tiny Animated Object — the swarm minion summoned by the level-5
/// Animate Objects transmutation. RAW stat block: AC 18, HP 20,
/// Slam +8 1d4+4 force. We approximate the +8 to-hit via STR 18
/// (+4) + the standard +2 proficiency, landing at +6 — close enough
/// that the swarm still meaningfully chips a CR-mid frontline. Tiny
/// Construct so the standard construct envelope (poison / psychic
/// immunity, charm / sleep / paralysis blocks) applies, matching the
/// 5e MM defaults for inanimate objects given life.
///
/// Pairs with the Animate Objects spell (`spells::ANIMATE_OBJECTS`)
/// via the standard summon helpers: `spawn_adjacent_summons` plants
/// up to ten of these around the caster, and the
/// `Conjured`-on-concentration-drop cleanup despawns them when the
/// caster's concentration ends.
pub static TINY_ANIMATED_OBJECT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TINY_ANIMATED_OBJECT_SLAM);
    CreatureTemplate {
        name: "Tiny Animated Object",
        // Lowercase 'o' for the smallest construct tier — keeps the
        // animated armor 'I' glyph distinct.
        glyph: 'o',
        ac: 18,
        hitpoints: "4d4+10".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 3,
        dexterity: 18,
        wisdom: 3,
        constitution: 10,
        charisma: 1,
        senses: std::collections::HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 1.0,
        size: Size::Tiny,
        creature_type: CreatureType::Construct,
        actions,
        // Standard construct damage profile — poison / psychic immunity.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // Standard construct condition immunities (mind-affecting +
        // poison + paralysis + blinded via blindsight).
        condition_immunities: std::collections::HashSet::from([
            // SRD 5.2 "Immunities Poison, Psychic; Charmed, Deafened,
            // Exhaustion, Frightened, Paralyzed, Petrified, Poisoned".
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Blinded,
            Condition::Asleep,
        ]),
        ..CreatureTemplate::defaults()
    }
});
