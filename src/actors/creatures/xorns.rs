use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{XORN_BITE, XORN_CLAW, XORN_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Xorn — CR 5 medium elemental (true neutral). The three-legged earth
/// dweller is a bruiser elemental: heavy multiattack (3 claws + 1 bite),
/// solid HP, AC 19 from natural stone-hide armor, and the standard
/// elemental's "non-magical physical resistance" envelope. RAW: Earth
/// Glide (move through stone), Treasure Sense (smells gold), and
/// Tremorsense from the three-eyed silhouette. We surface only the
/// load-bearing combat slice: the Multi, the tremorsense, the resistance
/// envelope.
///
/// Damage envelope: resistant to non-magical bludgeoning / piercing /
/// slashing from nonmagical attacks (RAW qualifies the clause and the
/// engine now can too — a mundane pick bounces off a xorn and an
/// enchanted one does not), immune to poison (rock body), immune to the Poisoned / Paralyzed /
/// Petrified / Unconscious conditions (elemental physiology).
///
/// Stats track MM Xorn at CR 5 — high STR for the heavy claw + bite
/// combo, decent CON for the HP pool, low CHA. Senses include
/// Tremorsense for the burrowing flavor.
pub static XORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&XORN_CLAW);
    actions.push(&XORN_BITE);
    actions.push(&*XORN_MULTI);
    CreatureTemplate {
        name: "Xorn",
        // 'X' — uppercase letter directly evokes the creature's name and
        // the three-pawed / three-eyed symmetry. Not currently shared
        // with any other creature glyph in the pool.
        glyph: 'X',
        ac: 19,
        hitpoints: "8d8+48".parse().unwrap(),
        speed: 20.,
        strength: 17,
        intelligence: 11,
        dexterity: 10,
        wisdom: 10,
        constitution: 22,
        charisma: 11,
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Elemental,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            crate::conditions::Condition::Poisoned,
            crate::conditions::Condition::Paralyzed,
            crate::conditions::Condition::Petrified,
            crate::conditions::Condition::Unconscious,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn xorn_is_poison_immune() {
        let a = ActorInstance::from_creature_template(
            &XORN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(a.effectively_immune_to_condition(crate::conditions::Condition::Poisoned));
    }

    #[test]
    fn xorn_has_strong_hp() {
        let a = ActorInstance::from_creature_template(
            &XORN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // 8d8+48 averages ~84 HP; the bruiser elemental should be well
        // above the CR-5 baseline tank threshold.
        assert!(a.max_hitpoints() >= 40);
    }
}
