use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::OCHRE_JELLY_PSEUDOPOD;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ochre Jelly — CR 2 large ooze. A sheet of digestive acid that flows
/// under doors, and the bestiary's clearest lesson in reading a damage
/// type before choosing a spell.
///
/// Action lane: **ochre jelly pseudopod**, 2d6+STR acid. One attack, no
/// rider, no save. The jelly's threat is that it does not stop.
///
/// The defensive envelope is the stat block:
///   - **Acid immunity** — it is made of the stuff.
///   - **Lightning immunity** — RAW, and the clause that makes the
///     jelly memorable: lightning is what *splits* an ochre jelly, and
///     the split is why RAW gives it immunity rather than resistance.
///   - **Slashing resistance** — you cannot cut a liquid in half in any
///     way it minds.
///   - **Amorphous** and the condition immunities that go with being a
///     puddle: Blinded, Charmed, Deafened, Exhausted, Frightened,
///     Prone. An ooze with no eyes cannot be blinded and an ooze with
///     no legs cannot be knocked down.
///
/// **Split** — RAW: "when a jelly that is Large or larger is subjected
/// to lightning or slashing damage, it splits into two new jellies" —
/// is the clause not carried. It needs a mid-combat instantiation lane
/// keyed on a damage type, which the engine has for summons and not for
/// damage triggers, and half of it is already expressed: the lightning
/// immunity and the slashing resistance are RAW's own admission that
/// neither type is how you kill this thing.
///
/// Stat shape per the SRD: AC 8, 52 HP (7d10+14), STR 15 / DEX 6 / CON
/// 14 / INT 2 / WIS 6 / CHA 1. Speed 20 — among the slowest things on the
/// board, and the reason an ochre jelly is a corridor problem rather
/// than an open-field one. Blindsight 60 (blind beyond). CR 2.
pub static OCHRE_JELLY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&OCHRE_JELLY_PSEUDOPOD);
    CreatureTemplate {
        name: "Ochre Jelly",
        // 'J' (uppercase) — the ooze band's spare glyph. 'O' is the
        // Ogre / Owlbear / Otyugh pool and lowercase 'o' the Orc's.
        glyph: 'J',
        ac: 8,
        // 7d10+14 ≈ 52 average per the SRD (CR 2).
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 20.,
        strength: 15,
        dexterity: 6,
        constitution: 14,
        intelligence: 2,
        wisdom: 6,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Ooze,
        actions,
        // SRD 5.2 "Resistances Acid" and "Immunities Lightning,
        // Slashing" — and the two rows had been swapped round the wrong
        // way here. Both immunities exist for the same reason and it is
        // not toughness: lightning and a sword edge are the two things
        // that *split* a jelly rather than hurt it, and the jelly that
        // comes out of it is two jellies. Its own acid is the thing it
        // merely shrugs off.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Immunity),
            (DamageType::Slashing, DamageModifier::Immunity),
        ]),
        // SRD 5.2 also prints Grappled and Restrained here, left off
        // for the reason the whole ooze cohort leaves them off: see
        // `GELATINOUS_CUBE_TEMPLATE`.
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Prone,
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
            &OCHRE_JELLY_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn ochre_jelly_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Ooze);
        assert!(a.find_action("ochre jelly pseudopod").is_some());
    }

    /// The two damage types RAW writes the jelly's Split clause about
    /// are the two it is *immune* to, which is the whole reason the
    /// immunity list looks strange — and the acid it is made of is the
    /// one it merely resists. The three had been ordered wrong here:
    /// slashing was a resistance and acid an immunity, which is the
    /// intuitive reading and the opposite of SRD 5.2's "Resistances
    /// Acid / Immunities Lightning, Slashing".
    #[test]
    fn the_jelly_shrugs_off_exactly_what_would_have_split_it() {
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Acid),
            Some(DamageModifier::Resistance)
        );
    }

    /// A puddle has nothing to blind, nothing to frighten and no feet
    /// to sweep. The ooze condition envelope is the difference between
    /// a slow creature and an unstoppable one.
    #[test]
    fn a_puddle_cannot_be_blinded_charmed_or_knocked_down() {
        let a = make();
        for c in [
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Prone,
        ] {
            assert!(
                a.effectively_immune_to_condition(c),
                "an ooze should be immune to {:?}",
                c
            );
        }
    }
}
