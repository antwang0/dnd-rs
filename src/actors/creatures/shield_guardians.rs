use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SHIELD_GUARDIAN_FIST, SHIELD_GUARDIAN_MULTI};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Shield Guardian — CR 7 large construct. A bound golem that exists to
/// be hit instead of its master, and that gets back up while you are
/// still hitting it.
///
/// Action lanes:
/// - **shield guardian multiattack** — 2 fists, 2d6+STR bludgeoning
///   each. Plain, heavy, and entirely beside the point.
/// - **shield guardian fist** (standalone) — the same swing, once.
///
/// **Regeneration** is the stat block. RAW: "the shield guardian regains
/// 10 hit points at the start of its turn if it has at least 1 hit
/// point." Ten a round against a CR 7 party is not a wall, it is a
/// clock: a party that cannot spend more than ten hit points of damage
/// per round on the guardian will never finish it, and a party that can
/// is spending its whole turn on the bodyguard rather than the wizard
/// behind it. Carried on `regen_per_round`, the same field the troll
/// reads — but with an empty `regen_suppressors` set, which is the
/// difference between the two creatures: a troll's regeneration stops
/// for fire and acid, and a construct's stops for nothing.
///
/// **Bound** and **Spell Storing** — the master link that redirects half
/// the master's damage onto the guardian, and the stored spell it
/// releases on command — are not carried. Both need a second creature
/// the guardian is attached to, and the engine has one such link
/// (mounts) built for a different shape. What survives is what a party
/// actually meets: a large construct that does not tire, does not fear,
/// does not bleed out, and heals faster than a low-tier party can chip.
///
/// Stat shape per the SRD: AC 17 (natural armor), 142 HP (15d10+60), STR
/// 18 / DEX 8 / CON 18 / INT 7 / WIS 10 / CHA 3. Speed 30. Blindsight
/// 10, Darkvision 60. Poison immune, condition envelope of a thing with
/// no blood in it. CR 7.
pub static SHIELD_GUARDIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SHIELD_GUARDIAN_MULTI);
    actions.push(&SHIELD_GUARDIAN_FIST);
    CreatureTemplate {
        name: "Shield Guardian",
        // 'G' (uppercase) — the golem band, shared with the Stone and
        // Iron Golems it stands beside on the construct ladder.
        glyph: 'G',
        ac: 17,
        // 15d10+60 ≈ 142 average per the SRD (CR 7).
        hitpoints: "15d10+60".parse().unwrap(),
        speed: 30.,
        strength: 18,
        dexterity: 8,
        constitution: 18,
        intelligence: 7,
        wisdom: 10,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(10), SpecialSense::Darkvision(60)]),
        cr: 7.0,
        size: Size::Large,
        creature_type: CreatureType::Construct,
        actions,
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
        ]),
        // RAW's flat 10 a round. Deliberately unsuppressed — see the
        // type docs for why a construct's regeneration is a different
        // rule from a troll's.
        regen_per_round: 10,
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
            &SHIELD_GUARDIAN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn shield_guardian_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 7.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Construct);
        assert!(a.find_action("shield guardian multiattack").is_some());
        assert!(a.find_action("shield guardian fist").is_some());
    }

    /// A construct's regeneration answers to nothing, which is the one
    /// line that separates this stat block from the troll's. Asserting
    /// the empty suppressor set is the only way to state a negative
    /// that a reader of the literal would otherwise have to infer from
    /// a field that isn't there.
    #[test]
    fn nothing_stops_a_constructs_regeneration() {
        let a = make();
        let mut a = a;
        assert_eq!(a.regen_per_round(), 10);
        for t in DamageType::ALL {
            a.note_regen_damage(t);
            assert!(
                !a.regen_suppressed(),
                "{:?} should not switch off a construct's repair",
                t
            );
        }
    }
}
