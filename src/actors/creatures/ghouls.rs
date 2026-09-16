use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GHOUL_CLAWS, GHOUL_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ghoul — CR 1 undead, and a creature with a decision to make every
/// turn:
///
///   - **Multiattack** — RAW's routine, two Bites, each `1d6` piercing
///     plus `1d6` necrotic. About eighteen average points.
///   - **Claws** — one swing, `1d4`, and a CON save DC 10 or Paralyzed
///     for two rounds. Paralysis turns every subsequent melee hit into
///     an auto-crit, so a lone ghoul that lands it can lock a PC out of
///     two full turns and have its friends collect.
///
/// Kill or lock: that trade is the whole monster, and for a long time
/// the engine offered only half of it. The bite and the multiattack
/// were missing entirely, which left a CR 1 undead making one 2d4 swing
/// a turn.
///
/// Immune to Poisoned / Charmed (standard undead suite).
pub static GHOUL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // RAW's Multiattack is two Bites; the claw is the option, not
    // the routine. Both are on the sheet — see `GHOUL_BITE`.
    actions.push(&*GHOUL_MULTI);
    actions.push(&*GHOUL_CLAWS);
    CreatureTemplate {
        name: "Ghoul",
        // 'U' for undead — 'G' is taken by goblin.
        glyph: 'U',
        ac: 12,
        hitpoints: "5d8".parse().unwrap(),
        strength: 13,
        dexterity: 15,
        constitution: 10,
        intelligence: 7,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison; Charmed, Exhaustion, Poisoned".
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::defaults()
    }
});
