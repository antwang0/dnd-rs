use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{INVISIBLE_STALKER_MULTI, INVISIBLE_STALKER_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Invisible Stalker — CR 6 large air elemental. Conjured by Conjure
/// Elemental (or as a faithful tracker by Find Familiar's bigger cousin),
/// the stalker is an air-elemental given a single hunting purpose. It
/// stays permanently invisible, glides on Darkvision 60 ft, and slams
/// its quarry to death with the same per-swing dice as a vanilla air
/// elemental but with the attack-mode advantage that flows from being
/// unseeable.
///
/// Action lanes:
/// - **invisible stalker multiattack** — 2 slams per Action via the
///   shared homogeneous `Multiattack` chassis. The stalker's full per-
///   turn output. Each swing routes through `SimpleWeapon::side_effects`
///   so the standard attacker-side advantage from `Condition::Invisible`
///   is applied — RAW: "Attack rolls against the stalker have
///   disadvantage, and the stalker's attack rolls have advantage."
/// - **invisible stalker slam** (standalone) — STR-based 2d8 + STR
///   bludgeoning melee, reach 1 (5 ft). Standalone so the AI can fall
///   back to a single swing when bonus-action-tagged or moving in.
///
/// Defensive identity: AC 14 (the air variant's chassis), 104 HP
/// (16d10+16). The headline trait is the **always-invisible** envelope
/// installed at instantiation via the template's `innate_conditions`
/// lane — `(Invisible, Permanent)` — so the stalker's first swing of
/// the fight already benefits from the attacker-side advantage AND
/// the target-side disadvantage on attacks targeting the stalker
/// (read by `compute_attack_mode` via `grants_self_attack_advantage`
/// and `imposes_disadvantage_to_attackers` respectively). The
/// invisibility doesn't drop when the stalker attacks — RAW: "the
/// stalker remains invisible until it attacks or until its
/// concentration ends"; the engine's `clear_attack_advantage_riders`
/// chokepoint does NOT consume `Invisible` (only `Hidden` lives in
/// `CONSUMED_ON_ATTACK`), so the permanent install holds steady. A
/// `TrueSighted` enemy still pierces the disadvantage clause via the
/// `countered_by_truesight` cohort.
///
/// Damage envelope: standard elemental baseline (poison immune + BPS
/// resistance). No signature damage-type overlay — the stalker is a
/// pure tracker, not an elemental tyrant; RAW gives no extra fire /
/// cold / lightning resistance beyond the elemental quartet's BPS.
///
/// Condition envelope: full elemental immunity cohort (Charmed /
/// Frightened / Paralyzed / Petrified / Poisoned / Asleep / Prone /
/// Grappled / Restrained) via the shared `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 14, ~104 HP (16d10+16), STR 16, DEX 19, CON 14,
/// INT 10, WIS 15, CHA 11. Speed 50 (RAW 30 ft walk + 50 ft fly hover
/// — we collapse to the faster of the two since the engine isn't 3D).
/// Senses: Darkvision 60 ft. Languages: Auran collapsed to Primordial
/// in this engine (matching the elemental quartet's rollup). Size
/// Large. CR 6.
pub static INVISIBLE_STALKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&INVISIBLE_STALKER_SLAM);
    actions.push(&*INVISIBLE_STALKER_MULTI);
    CreatureTemplate {
        name: "Invisible Stalker",
        // 'Ξ' (Greek capital xi) — three stacked horizontal strokes
        // read as wisps of layered wind. Distinct from every other
        // glyph in the elemental / air pool: 'A' = Air Elemental,
        // 'D' = Djinni, 'U' = Water Elemental, 'E' = Fire Elemental,
        // 'Π' = Dao. The Greek-letter family follows the same
        // convention as Δ (Air Sphinx-style boss), Ψ, Ω used for
        // celestials and ascended elementals.
        glyph: 'Ξ',
        ac: 14,
        // 16d10+16 ≈ 104 average per MM (CR 6).
        hitpoints: "16d10+16".parse().unwrap(),
        speed: 50.,
        strength: 16,
        intelligence: 10,
        dexterity: 19,
        wisdom: 15,
        constitution: 14,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Standard elemental baseline: poison immune + BPS resistance.
        // No signature damage-type overlay — the stalker has no
        // elemental affinity beyond the air-family default. Distinct
        // from the Air Elemental (lightning + thunder resistance):
        // RAW gives the stalker no thematic energy resistance beyond
        // the BPS triplet, since its identity is "hidden hunter" not
        // "storm avatar."
        damage_modifiers: elemental_damage_modifiers([]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // The headline trait: the stalker is born invisible. Routes
        // through the new `innate_conditions` template lane — applied
        // once at instantiation via `add_condition`. RAW: the
        // invisibility is *not* broken by attacking, so the
        // `Permanent` timer is correct (the engine's
        // `clear_attack_advantage_riders` site only consumes the
        // Hidden / Helped / Inspired one-shot cohort, never plain
        // Invisible).
        innate_conditions: vec![(Condition::Invisible, ConditionTimer::Permanent)],
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    fn empty_encounter() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap()
    }

    #[test]
    fn invisible_stalker_template_shape() {
        let a = ActorInstance::from_creature_template(
            &INVISIBLE_STALKER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 6.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Two action lanes: the multi for the AI's primary attack and
        // the standalone slam for single-swing fallbacks.
        assert!(a.find_action("invisible stalker multiattack").is_some());
        assert!(a.find_action("invisible stalker slam").is_some());
    }

    #[test]
    fn invisible_stalker_has_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &INVISIBLE_STALKER_TEMPLATE,
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
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // No thunder / lightning bias — distinct from the Air Elemental,
        // which RAW gets resistance to both. The stalker is a tracker,
        // not a storm avatar.
        assert_eq!(a.damage_modifier(DamageType::Thunder), None);
        assert_eq!(a.damage_modifier(DamageType::Lightning), None);
        // Full elemental condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Prone));
        assert!(a.effectively_immune_to_condition(Condition::Grappled));
        assert!(a.effectively_immune_to_condition(Condition::Restrained));
    }

    #[test]
    fn invisible_stalker_is_invisible_on_spawn() {
        // The headline trait: born invisible. This pins the new
        // `innate_conditions` template lane — instantiation through
        // `EncounterInstance::instantiate_creature` must install the
        // Invisible condition before the actor takes its first turn,
        // so the stalker's first slam already benefits from
        // attacker-side advantage AND the target-side disadvantage on
        // attacks targeting it.
        let mut e = empty_encounter();
        let id = e
            .instantiate_creature(
                &INVISIBLE_STALKER_TEMPLATE,
                Coordinate::new(2, 2),
                0,
                0,
            )
            .unwrap();
        assert!(
            e.actors[&id].has_condition(Condition::Invisible),
            "the stalker should be invisible from the moment it spawns"
        );
    }
}
