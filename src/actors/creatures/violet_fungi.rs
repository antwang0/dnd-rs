use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{VIOLET_FUNGUS_MULTI, VIOLET_FUNGUS_ROTTING_TOUCH};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Violet Fungus — CR ¼ medium plant. A stand of purple mushrooms that
/// is not a stand of purple mushrooms, waving four stalks of rotting
/// touch at anything that walks past.
///
/// Action lanes:
/// - **violet fungus multiattack** — 3 rotting touches at reach 10 ft.
///   Three 1d8 necrotic swings a round is a real number at CR ¼, and
///   necrotic is the point: almost nothing in the low-CR party resists
///   it, and the fungus is what teaches a party that a corridor of
///   mushrooms is worth walking around.
/// - **violet fungus rotting touch** (standalone) — one stalk.
///
/// **False Appearance** — RAW: "while the fungus remains motionless, it
/// is indistinguishable from an ordinary fungus" — is not carried. The
/// engine's Hidden condition is a stealth state a creature enters and
/// leaves, and a mimic-style ambush needs the party to *not know a
/// creature is there*, which is a fact about the player rather than
/// about the board. What is kept is the blindness the plant genuinely
/// has: it fights by touch and cannot be blinded because it never saw
/// anything to begin with.
///
/// Stat shape per the SRD: AC 5 — the lowest in the bestiary, and the
/// reason the fungus is a damage-race rather than a fight — 18 HP
/// (4d8), STR 3 / DEX 1 / CON 10 / INT 1 / WIS 3 / CHA 1. Speed 5.
/// Blindsight 30 (blind beyond). CR ¼.
pub static VIOLET_FUNGUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VIOLET_FUNGUS_MULTI);
    actions.push(&VIOLET_FUNGUS_ROTTING_TOUCH);
    CreatureTemplate {
        name: "Violet Fungus",
        // 'F' (uppercase) — the fungal / plant band. 'v' and 'V' are
        // the Vampire / Veteran / Vrock pool.
        glyph: 'F',
        ac: 5,
        // 4d8 ≈ 18 average per the SRD (CR ¼).
        hitpoints: "4d8".parse().unwrap(),
        speed: 5.,
        strength: 3,
        dexterity: 1,
        constitution: 10,
        intelligence: 1,
        wisdom: 3,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Plant,
        actions,
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Immunity)]),
        // The plant envelope: no eyes to blind, no mind to charm or
        // frighten, no metabolism to poison or tire.
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &VIOLET_FUNGUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn violet_fungus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.find_action("violet fungus multiattack").is_some());
        assert!(a.find_action("violet fungus rotting touch").is_some());
    }

    /// The stalk reaches ten feet and adds nothing for the plant's
    /// Strength. Both are deliberate and both are invisible in the
    /// literal: a STR-scaled touch on STR 3 would subtract four from
    /// every hit, and a reach-5 stalk would let the fungus be fought
    /// from a tile it cannot answer.
    #[test]
    fn the_stalk_reaches_past_its_own_tile_and_does_not_subtract_its_strength() {
        assert_eq!(VIOLET_FUNGUS_ROTTING_TOUCH.reach, 2);
        assert!(VIOLET_FUNGUS_ROTTING_TOUCH.damage_ability.is_none());
        assert_eq!(
            VIOLET_FUNGUS_ROTTING_TOUCH.damage_type,
            DamageType::Necrotic
        );
    }
}
