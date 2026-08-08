use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WEREBEAR_BITE, WEREBEAR_CLAWS, WEREBEAR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Werebear — CR 5 large lycanthrope (humanoid by type, hybrid by
/// silhouette). The biggest of the lycanthrope family — bear-bodied
/// fury with the iconic non-silver / non-magical BPS-resistance
/// envelope. Slots between Werewolf (CR 3) and the CR-6 mid-tier
/// monstrosities on the lycanthrope ladder — same defensive shape as
/// the werewolf but with a chunkier per-Action multi (1d10 bite +
/// 2d8 claws ≈ 18 average per Action vs the werewolf's 1d8+2d4 ≈ 12)
/// and double the HP pool.
///
/// Action lanes:
/// - **werebear multiattack** — 1 bite + 1 claws per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + slashing) —
///   the bite carries the DC-14 CON-save lycanthropy rider (Poisoned
///   3 rounds as a proxy for RAW's "curse of werebear lycanthropy"),
///   the claws are the steady damage lane.
/// - **werebear bite** (standalone) — STR-based 1d10+STR piercing
///   melee with the DC-14 CON save Poisoned rider. The "is werebear
///   scary?" tax in a single rider line — the multi-day curse RAW
///   collapses to a 3-round Poisoned debuff so the engine doesn't have
///   to model long-form lycanthropy transformations.
/// - **werebear claws** (standalone) — STR-based 2d8+STR slashing
///   melee. Vanilla `SimpleWeapon`; the chunky damage lane that pairs
///   with the curse-flavored bite.
///
/// Defensive identity: AC 11 (natural armor — the bear-hide), 135 HP
/// (18d8+54). Non-magical BPS resistance via the shared
/// `damage_modifiers_from` helper — RAW: "Damage Immunities
/// Bludgeoning, Piercing, and Slashing from Nonmagical Attacks that
/// aren't Silvered". The qualifier is real on both halves — a magic
/// weapon and a silvered one each get through; the *immunity* is
/// approximated as resistance, the same call the werewolf makes at
/// CR 3 and for the same reason. Keen Smell (RAW advantage on Perception
/// using smell) omitted — the engine doesn't tag Perception by sense
/// channel.
///
/// Stat shape: AC 11, ~135 HP (18d8+54), STR 19, DEX 10, CON 17, INT
/// 11, WIS 12, CHA 12. Speed 40 (hybrid-form burst speed, matching the
/// werewolf's tuned-up 40-ft for the same hybrid-form flavor).
/// Languages: Common (the humanoid-half retains speech). Size Large.
/// CR 5.
///
/// RAW also gives the werebear **Shapechanger** (bear / hybrid / human
/// form swaps as an Action) — omitted because the engine doesn't model
/// in-combat form swaps; the load-bearing slice is the hybrid-form
/// stat block (the form the werebear fights in), which is the one we
/// pin here.
pub static WEREBEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WEREBEAR_MULTI);
    actions.push(&WEREBEAR_BITE);
    actions.push(&WEREBEAR_CLAWS);
    CreatureTemplate {
        name: "Werebear",
        // 'B' (uppercase) — distinct from 'b' (Bandit / Bullette /
        // Berserker / Banshee all share lowercase 'b'). 'B' for
        // "werebear" reads as the larger-statured bear-bodied
        // silhouette; reserves the capital glyph slot for the more
        // imposing of the bear-themed creatures (distinguished from
        // 'B' the Brown Bear by the lycanthrope envelope below).
        glyph: 'B',
        ac: 11,
        // 18d8+54 ≈ 135 average per MM (CR 5).
        hitpoints: "18d8+54".parse().unwrap(),
        speed: 40.,
        strength: 19,
        intelligence: 11,
        dexterity: 10,
        wisdom: 12,
        constitution: 17,
        charisma: 12,
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Humanoid,
        actions,
        // Lycanthrope resistance to the physical trio, qualified to
        // nonmagical attacks that aren't silvered. Same call as the
        // werewolf template, and shared with it through the
        // constructor rather than restated here.
        ..CreatureTemplate::resistant_to_nonmagical_nonsilvered_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn werebear_template_shape() {
        let a = ActorInstance::from_creature_template(
            &WEREBEAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        // The werebear's three action lanes — multi (bite + claws)
        // primary, with each part also exposed as a standalone fallback
        // for the AI's per-resource picking.
        assert!(a.find_action("werebear multiattack").is_some());
        assert!(a.find_action("werebear bite").is_some());
        assert!(a.find_action("werebear claws").is_some());
    }

    #[test]
    fn werebear_has_lycanthrope_bps_resistance() {
        let a = ActorInstance::from_creature_template(
            &WEREBEAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Lycanthrope-canonical BPS resistance — same as the werewolf
        // but on a thicker HP frame at the higher CR.
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // No magic resistance — the werebear's defense is HP pool +
        // BPS half-damage, not the caster-disruption envelope.
        assert!(!a.has_magic_resistance());
    }
}
