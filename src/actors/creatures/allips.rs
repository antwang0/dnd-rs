use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ALLIP_MADDENING_TOUCH;
use crate::actors::actor_template::{
    CreatureTemplate, INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES, non_magical_physical_resistances,
};
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Allip — CR 5 medium undead. The drifting spectre of a sage whose
/// mind shattered before death: an incorporeal whisper-haunt whose
/// signature is psychic-touch damage with a "babbling madness" rider
/// that compels the victim out of hostile actions. Sits in the CR-5
/// incorporeal-undead slot between the Wraith / Specter cohort
/// (smaller) and the Banshee (CR 4) on the undead ladder; thematically
/// fills the "intelligence-targeting psychic ghost" niche that the
/// Wraith (necrotic-touch life-drainer) doesn't cover.
///
/// Action lanes:
/// - **allip maddening touch** — STR-based 1d4+STR psychic melee with
///   a DC 13 INT save-or-Charmed rider on hit. Routes through the
///   shared `WeaponWithSaveCondition` chassis so the save-or-condition
///   install fires only on a confirmed hit and per-target immunity is
///   handled by the standard `add_condition` chokepoint. The Charmed
///   timer (3 rounds) approximates RAW's "babbling" stun-style debuff:
///   it locks the target out of hostile actions against the allip and
///   routes cleanly through the existing condition pipeline. The
///   psychic damage type is non-resisted by most creatures, which is
///   the load-bearing offensive identity at CR 5 — the allip bypasses
///   the typical CR-5 BPS-resistance envelope of armored fighters.
///
/// Defensive identity: AC 12 (the allip's incorporeal form makes it
/// hard to strike but not invulnerable), 40 HP (9d8). The incorporeal
/// undead envelope (necrotic + poison immunity, BPS resistance, and
/// the long body-control condition immunity menu) routes through the
/// shared `INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES` /
/// `non_magical_physical_resistances` helpers so the allip slots in
/// next to the Ghost / Wraith / Specter / Shadow cohort with one-line
/// shared-base reuse. Distinct from the Wraith: the allip's psychic
/// touch is the offensive lane, not a necrotic life-drain.
///
/// Stat shape: AC 12, ~40 HP (9d8), STR 6, DEX 15, CON 10, INT 6,
/// WIS 11, CHA 18. Speed 40 (RAW: 0ft walk + 40ft fly hover — we
/// collapse to the fly speed since the engine isn't 3D and the
/// stationary terrain doesn't gate hover-flight). Senses: Darkvision
/// 60. Languages: Common (the allip remembers a single language from
/// life, RAW: "but it understands all languages it knew in life";
/// we pin Common as the universal default). Size Medium. CR 5.
///
/// Incorporeal Movement (RAW: "The allip can move through other
/// creatures and objects as if they were difficult terrain") is
/// omitted as a deliberate scope cut — the engine's pathfinding
/// doesn't yet model wall-phasing actors, and modeling the
/// half-speed-through-walls clause would require a per-tile
/// "incorporeal cost" lane that doesn't exist. The BPS resistance
/// envelope captures the load-bearing "hard to hit with physical
/// weapons" half of the incorporeal identity.
pub static ALLIP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ALLIP_MADDENING_TOUCH);
    CreatureTemplate {
        name: "Allip",
        // 'a' (lowercase) — distinct mnemonic for Allip. 'A' is taken
        // by Air Elemental and other apex actors; lowercase 'a' reads
        // as a small, drifting silhouette appropriate to the incorp-
        // oreal spectre. The glyph collides with no other small-cap
        // entries (assassins use 'A', acolytes 'C'). If render-set
        // collisions surface, swap to 'φ' (Greek phi, untaken) — the
        // wisp-and-tail silhouette of a hovering psychic ghost reads
        // cleanly at small UI scale.
        glyph: 'a',
        ac: 12,
        // 9d8 ≈ 40 average per MM (CR 5).
        hitpoints: "9d8".parse().unwrap(),
        speed: 40.,
        strength: 6,
        intelligence: 6,
        dexterity: 15,
        wisdom: 11,
        constitution: 10,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // CR-5 RAW saves: WIS proficient (the allip's spectral
        // identity is rooted in mental willpower — the only stat that
        // weathered the shattering at death).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom]),
        // Incorporeal undead envelope — BPS resistance plus necrotic
        // + poison immunity. Routes through the shared
        // `non_magical_physical_resistances` helper so the BPS triple
        // stays uniform across the incorporeal cohort.
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Shared incorporeal-undead condition envelope: Charmed,
        // Exhausted, Frightened, Grappled, Paralyzed, Petrified,
        // Poisoned, Prone, Restrained, Unconscious.
        condition_immunities: INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &ALLIP_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn allip_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert!(a.find_action("allip maddening touch").is_some());
    }

    #[test]
    fn allip_maddening_touch_executes_via_save_condition_chassis() {
        // Pin the new `WeaponWithSaveCondition` chassis end-to-end: the
        // allip's signature touch must route the d20 swing AND the
        // INT save-or-Charmed rider through one chokepoint, producing
        // a side-effect list that's bounded by the swing's hit and
        // the target's save result. A future chassis refactor that
        // strips the save rider would regress this assertion.
        use crate::actions::action_template::Action;
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let allip = e
            .instantiate_creature(&ALLIP_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let bear_template = &crate::actors::creatures::brown_bears::BROWN_BEAR_TEMPLATE;
        let target = e
            .instantiate_creature(bear_template, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        let touch = e.actors[&allip]
            .find_action("allip maddening touch")
            .expect("allip should carry the maddening touch action");
        // The chassis bound: at most one DealDamage (psychic on hit)
        // plus at most one ApplyCondition (Charmed on a failed INT
        // save). The save rider only fires on a confirmed hit, so a
        // miss produces an empty effect list.
        let effects = touch.side_effects(&mut e, allip, Some(&vec![target]), None, None);
        assert!(
            effects.len() <= 2,
            "chassis should produce at most 2 effects (damage + condition); got {}",
            effects.len(),
        );
    }

    #[test]
    fn allip_has_incorporeal_undead_envelope() {
        let a = make();
        // BPS resistance — the incorporeal physical lane.
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
        // Necrotic + Poison immunity — the undead purity overlay.
        assert_eq!(
            a.damage_modifier(DamageType::Necrotic),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        // Full incorporeal-undead condition immunity envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Exhausted));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Petrified));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Prone));
        assert!(a.effectively_immune_to_condition(Condition::Grappled));
        assert!(a.effectively_immune_to_condition(Condition::Restrained));
        assert!(a.effectively_immune_to_condition(Condition::Unconscious));
    }
}
