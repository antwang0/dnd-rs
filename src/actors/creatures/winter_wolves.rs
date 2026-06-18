use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WINTER_WOLF_BITE, WINTER_WOLF_BREATH};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Winter Wolf — CR 3 large monstrosity. The frost-touched cousin of
/// the vanilla Wolf: same Pack Tactics signature for advantage when an
/// ally is adjacent, but the bite adds a 1d8 cold rider and the
/// signature Cold Breath weapon turns it into a CR-3 area threat. RAW
/// also has Keen Hearing and Smell + the Snow Camouflage racial — the
/// engine doesn't model terrain-typed Stealth, so we keep those
/// flavor-only and lean on the cold immunity + breath as the
/// distinguishing kit.
pub static WINTER_WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WINTER_WOLF_BITE);
    actions.push(&WINTER_WOLF_BREATH);
    CreatureTemplate {
        name: "Winter Wolf",
        // 'f' (lowercase) — free in the large monstrosity slot. 'W' is
        // taken by Wolf (the CR-¼ vanilla); 'f' for "frost wolf" reads
        // cleanly as the upgraded variant.
        glyph: 'f',
        ac: 13,
        // 9d10+18 ≈ 75 average per MM (CR 3).
        hitpoints: "9d10+18".parse().unwrap(),
        speed: 50.,
        strength: 18,
        intelligence: 7,
        dexterity: 13,
        wisdom: 12,
        constitution: 14,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant, Language::Common]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Cold immunity is the marquee defense; the engine halves cold
        // breath damage to 0 against this template just like a polar
        // creature should.
        damage_modifiers: HashMap::from([(DamageType::Cold, DamageModifier::Immunity)]),
        // Winter wolves don't fear the cold or fall asleep in blizzards;
        // we limit the immunity list to Charmed (their pack instincts
        // resist mind-magic) rather than over-tuning toward dragon-tier
        // condition coverage at CR 3.
        condition_immunities: HashSet::from([Condition::Charmed]),
        has_pack_tactics: true,
        recharge_abilities: vec![("breath_weapon", 5)],
        ..CreatureTemplate::defaults()
    }
});
