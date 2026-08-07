use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CHUUL_PINCER, CHUUL_TENTACLES};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Chuul — CR 4 aberration. Lobster-like predator with paralyzing
/// tentacles. Two actions: pincer (2d6+4 bludgeoning + grapple on hit)
/// and tentacles (1d6+4 poison + CON save DC 13 or Paralyzed 1 round,
/// only on grappled targets). AC 16, ~93 HP (11d10+33). Immune to
/// poison damage and the Poisoned condition. Amphibious with darkvision
/// 60ft and tremorsense 60ft.
pub static CHUUL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CHUUL_PINCER);
    actions.push(&*CHUUL_TENTACLES);
    CreatureTemplate {
        name: "Chuul",
        glyph: 'ç',
        ac: 16,
        hitpoints: "11d10+33".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 5,
        dexterity: 10,
        wisdom: 11,
        constitution: 16,
        charisma: 5,
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});
