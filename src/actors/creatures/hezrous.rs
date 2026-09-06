use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEZROU_BITE, HEZROU_CLAW, HEZROU_MULTI};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hezrou — CR 8 large demon (Type II demon). A bipedal toad-shaped fiend
/// with bulging muscles and a stench so foul it sickens nearby creatures.
/// Slots above Vrock (CR 6) and below Glabrezu (CR 9) on the demon
/// hierarchy ladder. RAW: bite + 2 claw multi, Magic Resistance, and a
/// stench aura that radiates Poisoned within 10 ft (CON save DC 14).
///
/// Engine model: heavy bite (2d10 + STR) + 2 claws (2d6 + STR each) via
/// the standard `CompoundAttack` chassis (same shape as Pit Fiend's bite
/// + 2 claws).
///
/// Damage profile: resistant to cold / fire / lightning + the
/// non-magical BPS triplet, immune to Poison damage AND the Poisoned
/// condition (demon physiology). Magic Resistance gives advantage on
/// every save vs spells / magical effects.
///
/// The Stench aura RAW (10ft sickening burst at start of nearby creature's
/// turn) is omitted — the engine has no per-tick aura hook surface that
/// matches the "start of *other* creature's turn" trigger cleanly, and
/// the load-bearing combat clause is the heavy multi + resistance
/// envelope. Future polish bucket if a `start_of_other_turn_aura` hook
/// lands.
pub static HEZROU_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HEZROU_BITE);
    actions.push(&HEZROU_CLAW);
    actions.push(&*HEZROU_MULTI);
    CreatureTemplate {
        name: "Hezrou",
        // 'Z' — unused in the demon pool ('B' balor, 'V' vrock, 'G'
        // glabrezu via uppercase letters elsewhere). 'Z' for the
        // hezrou's hunched silhouette and toad-like sluggishness.
        glyph: 'Z',
        ac: 18,
        // 15d10+75 ≈ 157 average per MM (CR 8).
        hitpoints: "15d10+75".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 5,
        dexterity: 17,
        wisdom: 12,
        constitution: 20,
        charisma: 13,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 8.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        // Hezrou proficient saves: STR, CON, WIS, CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Demon damage envelope: non-magical BPS resistance + cold / fire
        // / lightning resistance + poison immunity. Mirrors the Vrock /
        // Glabrezu damage profile (same demon family) at the CR-8 tier.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Demons brush off poison-related conditions (RAW: poisoned
        // immunity from demon physiology).
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // 5e Magic Resistance — advantage on every save vs spells /
        // magical effects. Read by `compute_save_mode`.
        has_magic_resistance: true,
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
    fn hezrou_has_demon_resistance_envelope() {
        let a = ActorInstance::from_creature_template(
            &HEZROU_TEMPLATE,
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
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn hezrou_has_magic_resistance_and_poisoned_immunity() {
        let a = ActorInstance::from_creature_template(
            &HEZROU_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
        assert!(a.is_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn hezrou_has_compound_multiattack() {
        let a = ActorInstance::from_creature_template(
            &HEZROU_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("bite + claws").is_some());
        assert!(a.find_action("hezrou bite").is_some());
        assert!(a.find_action("hezrou claw").is_some());
    }
}
