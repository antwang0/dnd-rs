use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    IRON_GOLEM_BREATH, IRON_GOLEM_MULTI, IRON_GOLEM_SLAM, IRON_GOLEM_SWORD,
};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::conditions::Condition;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Iron Golem — CR 16 boss-tier construct. The pinnacle of the golem
/// ladder: heavier hit points, harder swings, longer reach, and a poison
/// breath weapon on top of the standard "anti-caster" construct envelope.
/// Slots above Stone Golem (CR 10) and below Tarrasque (CR 30) on the
/// boss-tier ladder; the canonical "magic doesn't work on me" sentinel
/// for a wizard tower / vault encounter.
///
/// Action lanes:
/// - **Iron Golem Multiattack** — 1 sword + 1 slam per Action. Mixed-limb
///   `CompoundAttack` (sword first to inherit its reach-2 envelope so
///   the multi can land on a target a full tile beyond MELEE_REACH).
/// - **Iron Sword** (standalone) — STR-based 3d10+STR slashing, reach 2
///   (10 ft RAW). The longer reach gives the golem a real threat envelope
///   around its 2×2 Large footprint.
/// - **Iron Slam** (standalone) — STR-based 3d8+STR bludgeoning melee
///   (reach 1). Use when the sword's reach isn't needed.
/// - **Iron Poison Breath** — burst-3 / range-4 cone, recharge 6.
///   10d8 poison, DC-19 CON, half on save. Heavier per-die count than
///   the dragon breath cone but at a smaller burst radius — matches the
///   RAW "15 ft cone, 10d8 poison" stat block.
///
/// Damage envelope: nonmagical B/P/S resistance plus immunity to fire,
/// poison, and psychic damage. The RAW **Fire Absorption** clause (fire
/// damage heals the golem instead of harming it) collapses to flat fire
/// immunity here — the engine doesn't yet model damage-type-to-heal
/// conversion. The lossy approximation is a small overshoot in the
/// golem's favor (RAW: the golem benefits from fire; here: it just shrugs
/// it off). Documented at the top so a future Fire Absorption hook lands
/// cleanly without surprising callers.
///
/// **Immutable Form** RAW makes the golem immune to any spell or effect
/// that would alter its form (Polymorph, Petrification, Flesh to Stone,
/// True Polymorph). We capture the load-bearing slice via the standard
/// Petrified condition immunity — the engine's surface for "your form
/// is being altered" — plus the inherited construct immunity envelope.
/// Polymorph immunity isn't modeled separately; the engine's Polymorph
/// install would still take, but the golem's massive HP pool absorbs
/// any beast-form HP swap cleanly.
///
/// **Magic Resistance** plus **Legendary Resistance (3/Day)** — the
/// signature anti-caster combo. Three failed saves per long rest auto-
/// promote to passes, on top of the blanket advantage Magic Resistance
/// grants on every save vs spells. Combined with the magic-immunity
/// envelope, the golem demands magical weapons to whittle down — spells
/// bounce off.
///
/// Stat shape: AC 20, ~210 average HP (20d10+100), STR 24, INT 3,
/// darkvision 120 ft. No languages — golems understand commands from
/// their creator but don't speak. CR 16.
pub static IRON_GOLEM_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*IRON_GOLEM_MULTI);
    actions.push(&IRON_GOLEM_SWORD);
    actions.push(&IRON_GOLEM_SLAM);
    actions.push(&IRON_GOLEM_BREATH);
    CreatureTemplate {
        name: "Iron Golem",
        // 'I' (uppercase) — distinct from 'G' (stone golem) and 'i'
        // (imp), kept short for the ASCII map readability.
        glyph: 'I',
        ac: 20,
        // 20d10+100 ≈ 210 average per MM (CR 16).
        hitpoints: "20d10+100".parse().unwrap(),
        speed: 30.,
        strength: 24,
        intelligence: 3,
        dexterity: 9,
        wisdom: 11,
        constitution: 20,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        cr: 16.0,
        size: Size::Large,
        creature_type: CreatureType::Construct,
        actions,
        // Construct damage envelope: nonmagical B/P/S resistance plus
        // immunity to fire, poison, and psychic. Fire immunity is the
        // approximation of RAW's Fire Absorption (heals from fire) —
        // the engine doesn't yet model damage-to-heal conversion.
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            // Standard construct envelope: no mind to charm / frighten,
            // no metabolism to poison or exhaust, no joints to freeze or
            // petrify (the latter captures the RAW Immutable Form clause).
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
        ]),
        // 5e Legendary Resistance (3/Day) — the boss-construct anti-
        // caster signature, three failed saves auto-promote to passes.
        legendary_resistances: 3,
        // 5e Magic Resistance — advantage on every save vs spells /
        // magical effects, read by `compute_save_mode`.
        has_magic_resistance: true,
        // 5e Recharge 6 on the poison breath — RAW: "Recharge 6" only
        // refreshes on a d6 of exactly 6 at start of turn, distinct from
        // the more common "Recharge 5-6" the dragons share. The lower
        // refresh probability keeps the breath as a once-per-encounter-
        // ish nuke rather than a turn-2 re-tap.
        recharge_abilities: vec![("iron poison breath", 6)],
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn iron_golem_has_full_construct_envelope() {
        let a = ActorInstance::from_creature_template(
            &IRON_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fire / poison / psychic immunity plus physical resistance.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Psychic),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn iron_golem_has_magic_resistance_and_legendary_resistance() {
        let a = ActorInstance::from_creature_template(
            &IRON_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
        assert_eq!(a.legendary_resistance_remaining(), 3);
    }

    #[test]
    fn iron_golem_has_construct_condition_immunities() {
        let a = ActorInstance::from_creature_template(
            &IRON_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Charmed));
        assert!(a.is_immune_to_condition(Condition::Frightened));
        assert!(a.is_immune_to_condition(Condition::Paralyzed));
        assert!(a.is_immune_to_condition(Condition::Petrified));
        assert!(a.is_immune_to_condition(Condition::Poisoned));
        assert!(a.is_immune_to_condition(Condition::Exhausted));
    }

    #[test]
    fn iron_golem_has_multi_sword_slam_and_breath() {
        let a = ActorInstance::from_creature_template(
            &IRON_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("iron sword + slam").is_some());
        assert!(a.find_action("iron sword").is_some());
        assert!(a.find_action("iron slam").is_some());
        assert!(a.find_action("iron poison breath").is_some());
    }
}
