use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DRETCH_BITE, DRETCH_CLAWS, DRETCH_FETID_CLOUD, DRETCH_MULTI};
use crate::conditions::Condition;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Dretch — CR ¼ small fiend (chaotic-evil demon, manes-tier).
/// The lowest-tier demon — a cowardly grunt swarmer of the Abyss. Slots
/// at the bottom of the fiend ladder alongside Sprite / Pixie (CR ¼),
/// distinguished by demon resistances and the Fetid Cloud condition
/// install. The Quasit (CR 1) is the next demon up; the Imp (CR 1) is
/// the parallel devilish CR-1 mirror.
///
/// Action lanes:
/// - **dretch multiattack** — 1 bite + 1 claws per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + slashing) —
///   both swings are flat (no STR mod) per RAW, low expected damage but
///   the Fetid Cloud is the load-bearing tactical clause.
/// - **dretch bite** (standalone) — STR-based 1d6 piercing melee, no
///   STR mod. Damage-only fallback.
/// - **dretch claws** (standalone) — STR-based 2d4 slashing melee, no
///   STR mod. The heavier per-swing roll.
/// - **fetid cloud** — Recharge 6 burst, 10-ft radius around the dretch,
///   DC 11 CON save or Poisoned until start of dretch's next turn (1
///   round). Promoted from RAW 1/Day to Recharge 6 so the burst can
///   occasionally re-fire across a long combat without hand-tracking
///   long-rest cadence. The cloud's range is unaffected by the dretch's
///   own creature type — fellow demons (poison-immune) shrug it off via
///   the standard condition-immunity chokepoint.
///
/// Defensive identity: resistant to cold / fire / lightning (the
/// demonic damage envelope), immune to poison. The "non-magical
/// weapons" RAW clause is omitted — the engine doesn't tag attacks as
/// magical/mundane, and the dretch's 18-HP statline reads as a
/// throwaway grunt anyway. No condition immunities beyond the implicit
/// Poisoned (immune via poison-damage immunity at the apply chokepoint).
/// No magic resistance — the dretch is the lowest demon and RAW lacks
/// the trait; the Quasit (CR 1, MR-flagged) is the next demon up.
///
/// Stat shape: AC 11 (natural armor — the leathery hide), 18 HP (4d6+4),
/// STR 11, DEX 11, CON 12, INT 5, WIS 8, CHA 3 (a dretch has no
/// personality to speak of). Speed 20 (the manes-shuffle). Senses:
/// Darkvision 60. Languages: Abyssal (the demon tongue), with
/// understanding-but-not-speaking of Common — we drop the listener-only
/// flag and surface only Abyssal as a speaker-language. Size Small.
/// CR ¼.
pub static DRETCH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRETCH_MULTI);
    actions.push(&DRETCH_BITE);
    actions.push(&DRETCH_CLAWS);
    actions.push(&*DRETCH_FETID_CLOUD);
    CreatureTemplate {
        name: "Dretch",
        // 'd' (lowercase) — distinct from 'D' (Death Dog / Dire Wolf,
        // uppercase). The descender hints at the dretch's hunched
        // silhouette; reads as a small grunt.
        glyph: 'd',
        ac: 11,
        // 4d6+4 ≈ 18 average per MM (CR ¼).
        hitpoints: "4d6+4".parse().unwrap(),
        speed: 20.,
        strength: 11,
        intelligence: 5,
        dexterity: 11,
        wisdom: 8,
        constitution: 12,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Fiend,
        // SRD 5.2 "Immunities Poison; Poisoned" — the poison
        // immunity on the damage row and the condition on this one are
        // two halves of the same sentence, and only the first half was
        // here.
        condition_immunities: HashSet::from([Condition::Poisoned]),
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Fetid Cloud is gated on the shared recharge chassis (start-of-
        // turn d6 roll, available again on 6). Keeps the dretch from
        // spamming the cloud every turn — RAW is 1/Day, we promote to
        // Recharge 6 so the burst occasionally fires more than once.
        recharge_abilities: vec![("dretch_fetid_cloud", 6)],
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
    fn dretch_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DRETCH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        // All four action lanes — multi, bite + claws standalone, plus
        // the fetid cloud burst.
        assert!(a.find_action("dretch multiattack").is_some());
        assert!(a.find_action("dretch bite").is_some());
        assert!(a.find_action("dretch claws").is_some());
        assert!(a.find_action("fetid cloud").is_some());
    }

    #[test]
    fn dretch_has_demon_damage_envelope() {
        let a = ActorInstance::from_creature_template(
            &DRETCH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Demon damage envelope: poison immune, fire / cold / lightning
        // resistant. No magic resistance (RAW: not on a dretch).
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
        assert!(!a.has_magic_resistance());
    }

    /// Fetid Cloud is Recharge 6. Verify the validator gates the
    /// action on the recharge resource — fresh dretch can fire it,
    /// post-use it's locked out until the start-of-turn d6 lands on 6.
    #[test]
    fn fetid_cloud_recharge_gating() {
        use crate::actions::action_template::Action;
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        let tp = TerrainGenParams {
            width: 30,
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(13)).unwrap();
        let attacker = e
            .instantiate_creature(&DRETCH_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        // Fresh dretch: cloud is available.
        assert!(e.actors[&attacker].is_recharge_available("dretch_fetid_cloud"));
        assert!(DRETCH_FETID_CLOUD.validate_input(&e, attacker, None, None, None));
        // Spend the cloud — recharge flips off.
        for ef in DRETCH_FETID_CLOUD.side_effects(&mut e, attacker, None, None, None) {
            ef.apply(&mut e);
        }
        assert!(!e.actors[&attacker].is_recharge_available("dretch_fetid_cloud"));
        assert!(!DRETCH_FETID_CLOUD.validate_input(&e, attacker, None, None, None));
    }
}
