use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WERETIGER_BITE, WERETIGER_CLAWS, WERETIGER_MULTI};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Weretiger — CR 4 large lycanthrope. The jungle-stalker of the
/// lycanthrope family — tiger-bodied in hybrid form, with the same
/// pounce-flavored predatory niche as the saber-toothed tiger but
/// wearing a humanoid mask. Slots between Werewolf (CR 3) and
/// Werebear (CR 5) on the lycanthrope ladder, sharing the CR-4 slot
/// with the wereboar — the agile/predatory variant to the wereboar's
/// brute-melee chassis.
///
/// Action lanes:
/// - **weretiger multiattack** — 1 bite + 1 claws per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + slashing) —
///   the bite carries the DC-13 CON-save lycanthropy rider (Poisoned
///   3 rounds as a proxy for RAW's curse), the claws are the steady
///   damage lane.
/// - **weretiger bite** (standalone) — STR-based 1d10+STR piercing
///   melee with the lycanthropy curse rider. Mid-DC of the wereXX
///   family (12/13/14 — werewolf/weretiger/werebear).
/// - **weretiger claws** (standalone) — STR-based 1d8+STR slashing
///   melee. Vanilla `SimpleWeapon`; the per-Action damage lane that
///   pairs with the curse-flavored bite.
///
/// Defensive identity: AC 12 (light armor + DEX). 120 HP (16d10+32).
/// Non-magical BPS resistance via the shared
/// `non_magical_physical_resistances` helper — the canonical
/// lycanthrope envelope. RAW gives the weretiger **Keen Hearing and
/// Smell** + **Pounce** (move 15+ ft toward a target then hit with
/// claws → DC 14 STR save or Prone, free bonus-action bite) — both
/// omitted: the engine doesn't tag Perception by sense channel and
/// doesn't track per-turn movement for triggering on-hit save riders.
/// The load-bearing kit is the bite + claws multi.
///
/// Stat shape: AC 12, ~120 HP (16d10+32), STR 17, DEX 15, CON 16, INT
/// 10, WIS 13, CHA 11. Speed 40 (hybrid-form burst speed matching the
/// other wereXX templates). Senses: Darkvision 60. Languages: Common
/// (humanoid form retains speech). Size Large. CR 4.
pub static WERETIGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WERETIGER_MULTI);
    actions.push(&WERETIGER_BITE);
    actions.push(&WERETIGER_CLAWS);
    CreatureTemplate {
        name: "Weretiger",
        // 't' (lowercase) — 'T' is taken by Tiger (the big-cat beast);
        // 'T' for Treant. 't' is free since the lycanthrope is a
        // distinct silhouette from the plain tiger.
        glyph: 't',
        ac: 12,
        // 16d10+32 ≈ 120 average per MM (CR 4).
        hitpoints: "16d10+32".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 10,
        dexterity: 15,
        wisdom: 13,
        constitution: 16,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Humanoid,
        actions,
        damage_modifiers: non_magical_physical_resistances([]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn weretiger_template_shape() {
        let a = ActorInstance::from_creature_template(
            &WERETIGER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("weretiger multiattack").is_some());
        assert!(a.find_action("weretiger bite").is_some());
        assert!(a.find_action("weretiger claws").is_some());
    }

    #[test]
    fn weretiger_has_lycanthrope_bps_resistance() {
        let a = ActorInstance::from_creature_template(
            &WERETIGER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        assert!(!a.has_magic_resistance());
    }
}
