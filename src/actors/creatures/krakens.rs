use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    KRAKEN_LIGHTNING_STORM, KRAKEN_MULTI, KRAKEN_TENTACLE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Kraken — CR 23 gargantuan titan-monstrosity, the apex aquatic boss.
/// One of the few non-Tarrasque CR-23 entries in the engine (paired with
/// Ancient Blue Dragon at the same tier). The marquee features are the
/// triple tentacle Multiattack at reach-6 (30 ft) — the engine's longest-
/// reach melee profile — and the Lightning Storm recharge that ravages
/// any cluster of enemies caught in line-of-sight.
///
/// Action lanes:
/// - **Kraken Multiattack** — 3 tentacle swings per Action. Vanilla
///   single-sub-attack Multiattack; the bursty melee lane that scales
///   with the kraken's STR-30 modifier (+10) plus prof (+7) = +17 to-hit.
/// - **Kraken Tentacle** (standalone) — STR-based 3d6+STR bludgeoning at
///   reach 6 (30 ft RAW). The single-tentacle lane for when one swing
///   suffices (e.g. mopping up a single low-HP target).
/// - **Lightning Storm** — Action: every enemy within radius 12 (60 ft)
///   of the kraken with line-of-sight makes a DC 23 DEX save; failures
///   take 4d10 lightning, passes take half. Recharge 5-6 via the shared
///   `"breath_weapon"` pool. See `KrakenLightningStorm` for the RAW-vs-
///   engine modeling notes.
///
/// Damage envelope: **Lightning immunity** (the kraken is a creature of
/// the storm — its own bolts can't hurt it). No physical resistance —
/// kraken flesh is mortal even at gargantuan scale, unlike the dragons
/// or fiends; the load-bearing defense is raw HP (472 average) plus the
/// triple Legendary Resistance.
///
/// Condition immunities: **Frightened + Paralyzed** per MM. The kraken
/// is too primal to fear and too vast to lock down. Charm immunity is
/// conspicuously absent RAW — a high-level enchanter's Charm Monster can
/// theoretically work, though the Magic Resistance lane gives the kraken
/// advantage on the save.
///
/// Stat shape: AC 18 (slick hide), 472 average HP (27d20+189), STR 30
/// (the engine's tied-highest after Tarrasque), CON 25. Truesight 120 ft
/// (the kraken sees through illusion and into the Ethereal Plane). No
/// Darkvision needed — Truesight subsumes it. Speed 20 ft (slow on
/// land) — RAW's 60 ft swim is the larger number and the engine models
/// one magnitude, so the land speed is the one on the sheet. The swim
/// *speed* itself ships as `SWIM_SPEED_TAG`, which is what keeps a pool
/// from charging the kraken double to cross. Being Gargantuan it is
/// almost never `is_immersed` — that wants all sixty-four of its
/// footprint tiles under water — so in practice the sea monster wades
/// through the map's ponds rather than swimming them, which is about
/// right for a creature twenty feet across.
///
/// Languages: understands Abyssal / Celestial / Infernal / Primordial but
/// can't speak. We surface only Primordial since the engine doesn't
/// distinguish "understands but can't speak" from "speaks fluently" —
/// the chosen language reads as the kraken's native cosmic tongue. CR 23.
///
/// Save profile: proficient on STR / DEX / CON / INT / WIS per MM. The
/// CHA save is conspicuously absent — the kraken's titanic intellect and
/// will dwarf its personality. Combined with **Magic Resistance** and 3
/// **Legendary Resistance** charges, the kraken shrugs off most party-
/// caster lockdown attempts.
///
/// Legendary actions: 3 per round per MM (Tentacle Attack / Lightning
/// Storm / Fling). We expose the 3-LA budget through
/// `legendary_actions_per_round`; the existing engine doesn't yet have
/// the Fling-throw lane (no thrown-object mechanic), so the LA budget
/// in practice translates to extra tentacle swings between PCs' turns.
pub static KRAKEN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*KRAKEN_MULTI);
    actions.push(&KRAKEN_TENTACLE);
    actions.push(&*KRAKEN_LIGHTNING_STORM);
    CreatureTemplate {
        name: "Kraken",
        // 'K' is taken by Rakshasa; 'k' (lowercase) reads as "kraken" and
        // is free in the gargantuan / titan pool. Distinct from 'T'
        // (Tarrasque) and 't' (Dragon Turtle).
        glyph: 'k',
        ac: 18,
        // 27d20+189 ≈ 472 average per MM (CR 23).
        hitpoints: "27d20+189".parse().unwrap(),
        speed: 20.,
        strength: 30,
        intelligence: 22,
        dexterity: 11,
        wisdom: 18,
        constitution: 25,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 23.0,
        size: Size::Gargantuan,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Kraken proficient saves: STR, DEX, CON, INT, WIS per MM. The
        // CHA save is conspicuously absent.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // Lightning immunity is the kraken's signature defense — its own
        // Lightning Storm can't blow back through self-centered casts.
        damage_modifiers: HashMap::from([(DamageType::Lightning, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            // Too primal to fear, too vast to paralyze.
            Condition::Frightened,
            Condition::Paralyzed,
        ]),
        // 5e Magic Resistance — advantage on every save vs spells /
        // magical effects. Read by `compute_save_mode`. Combined with the
        // 3 Legendary Resistances, the kraken is a hard nut for party
        // casters to crack mid-fight.
        has_magic_resistance: true,
        // 5e Legendary Resistance (3/Day): three failed saves per long
        // rest are auto-promoted to passes. Critical for surviving
        // Banishment / Hold Monster / Power Word Kill from party
        // casters mid-encounter.
        legendary_resistances: 3,
        // 5e Recharge 5-6 on the Lightning Storm via the shared
        // `"breath_weapon"` pool — same chassis the dragons / gorgons /
        // iron golem share.
        recharge_abilities: vec![("breath_weapon", 5)],
        // 5e Legendary Actions — 3 per round. The kraken spends them
        // between other actors' turns for extra tentacle swings.
        legendary_actions_per_round: 3,
        // 5e lair actions — the water around it is part of the fight.
        // See `engine::lair_actions`.
        lair_actions: crate::engine::lair_actions::KRAKEN_LAIR,
        has_extra_attack: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
    fn kraken_has_lightning_immunity_and_no_physical_resistance() {
        let a = ActorInstance::from_creature_template(
            &KRAKEN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Immunity)
        );
        // No physical-resistance envelope — kraken flesh is mortal at
        // gargantuan scale; the engine's HP pool carries the defense.
        assert_eq!(a.damage_modifier(DamageType::Slashing), None);
        assert_eq!(a.damage_modifier(DamageType::Bludgeoning), None);
        assert_eq!(a.damage_modifier(DamageType::Piercing), None);
        assert_eq!(a.damage_modifier(DamageType::Fire), None);
    }

    #[test]
    fn kraken_has_fear_and_paralysis_immunity_no_charm_immunity() {
        let a = ActorInstance::from_creature_template(
            &KRAKEN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Frightened));
        assert!(a.is_immune_to_condition(Condition::Paralyzed));
        // Charm immunity is conspicuously absent RAW — Magic Resistance
        // gives advantage on the save but doesn't auto-block.
        assert!(!a.is_immune_to_condition(Condition::Charmed));
    }

    #[test]
    fn kraken_has_legendary_envelope() {
        let a = ActorInstance::from_creature_template(
            &KRAKEN_TEMPLATE,
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
    fn kraken_has_tentacle_multi_and_lightning_storm() {
        let a = ActorInstance::from_creature_template(
            &KRAKEN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("kraken multiattack").is_some());
        assert!(a.find_action("kraken tentacle").is_some());
        assert!(a.find_action("lightning storm").is_some());
        assert_eq!(a.cr(), 23.0);
        assert_eq!(a.size(), Size::Gargantuan);
    }
}
