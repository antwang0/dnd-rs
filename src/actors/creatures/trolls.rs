use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BITE, SLAM, TROLL_LIMB_REND};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Troll — large regenerating brute (CR 5). High HP and two attacks
/// (claw + bite) per turn via the multiattack wrapper. Mechanically the
/// troll exercises the Large footprint and reach-2 adjacency the same
/// way the Ogre does, but with more staying power.
///
/// Regeneration: 3 HP at end-of-round while combat-active, suppressed
/// for one round whenever the troll takes acid or fire damage. The
/// engine reads `regen_per_round` / `regen_suppressors` from the
/// template, and `DealDamage` flips `regen_suppressed` whenever a
/// suppressor type lands.
pub static TROLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Claws (slam) + Bite — two attacks per action, matching 5e multiattack.
    actions.push(&SLAM);
    actions.push(&BITE);
    CreatureTemplate {
        name: "Troll",
        glyph: 'T',
        ac: 15,
        hitpoints: "9d10+45".parse().unwrap(),
        strength: 18,
        dexterity: 13,
        constitution: 20,
        intelligence: 7,
        wisdom: 9,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Giant,
        actions,
        regen_per_round: 3,
        regen_suppressors: HashSet::from([DamageType::Acid, DamageType::Fire]),
        ..CreatureTemplate::defaults()
    }
});

/// Troll Limb — CR ½ small giant. An arm that got cut off and kept
/// going.
///
/// SRD 5.2 prints it as its own stat block, and it is the best joke in
/// the book: the limb has the troll's Strength, the troll's
/// regeneration and the troll's darkvision, on fourteen hit points and
/// a twenty-foot crawl. It hits for nine, which is what the whole troll
/// hits for, because it is the same arm.
///
/// The tactical shape is entirely about the regeneration. Five hit
/// points a turn on a fourteen-hit-point creature means a party that is
/// not carrying acid or fire cannot finish it — every round they do
/// four and it takes five back — and RAW's death clause is explicit
/// about the trick: "The limb dies only if it starts its turn with 0
/// Hit Points and doesn't regenerate." A torch ends it in one round; a
/// sword never does.
///
/// **Troll Spawn** — "If the limb isn't destroyed within 24 hours, roll
/// 1d12. On a 12, the limb turns into a Troll" — is not modeled, and
/// could not be: the roll happens a day after the fight, and the
/// engine's timeline stops when the initiative order does. It is also
/// the reason the limb is a stat block at all rather than scenery, so
/// it is worth knowing at a table even though nothing here reads it.
pub static TROLL_LIMB_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TROLL_LIMB_REND);
    CreatureTemplate {
        name: "Troll Limb",
        // 't' beside the troll's 'T' — the same creature, one size and
        // one dismemberment down.
        glyph: 't',
        ac: 13,
        // 4d6 = 14 average per SRD 5.2 (CR ½).
        hitpoints: "4d6".parse().unwrap(),
        speed: 20.,
        strength: 18,
        dexterity: 12,
        constitution: 10,
        intelligence: 1,
        wisdom: 9,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Giant,
        actions,
        // RAW: 5 a turn, and acid or fire shuts it off for the next
        // one. The same two suppressors the whole troll reads, because
        // it is the same flesh.
        regen_per_round: 5,
        regen_suppressors: HashSet::from([DamageType::Acid, DamageType::Fire]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(t: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            t,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn troll_limb_template_shape() {
        let a = make(&TROLL_LIMB_TEMPLATE);
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Giant);
        assert!(a.find_action("limb rend").is_some());
    }

    /// The limb regenerates harder than the troll it fell off, and the
    /// two shut off to the same two things.
    ///
    /// Both halves matter and they pull in opposite directions, which
    /// is why they are pinned together. Five a turn on fourteen hit
    /// points is a creature a party without acid or fire genuinely
    /// cannot kill — and the moment the suppressor list drifts apart
    /// from the troll's, the torch that ends one stops ending the
    /// other, for no reason anybody wrote down.
    #[test]
    fn the_arm_heals_faster_than_the_troll_and_burns_the_same() {
        let limb = make(&TROLL_LIMB_TEMPLATE);
        let troll = make(&TROLL_TEMPLATE);
        assert!(limb.regen_per_round() > troll.regen_per_round());
        for dt in [DamageType::Acid, DamageType::Fire] {
            assert!(limb.regen_suppressed_by(dt), "{:?} should stop it", dt);
            assert!(troll.regen_suppressed_by(dt), "{:?} should stop it", dt);
        }
    }
}
