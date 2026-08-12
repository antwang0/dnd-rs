use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GRAY_OOZE_PSEUDOPOD;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Gray Ooze — CR ½ medium ooze. The bottom of a ladder whose other
/// rungs are the Ochre Jelly (CR 2), the Gelatinous Cube (CR 2) and the
/// Black Pudding (CR 4).
///
/// Two things make it worth fielding below all of them:
///   - **Ten average acid damage on a CR-½ stat block**, which is more
///     than anything else at that tier does in one swing, off an AC of
///     9 that makes it trivially hittable. A gray ooze trades hit
///     points for damage at a rate nothing else at its tier offers.
///   - **Stealth proficiency and blindsight 60** on a creature that
///     looks like wet stone. It is the ambush half of the ooze family.
///
/// Action lane:
/// - **gray ooze pseudopod** — STR-based 2d8+STR acid.
///
/// **Corrosive Form** and the pseudopod's armour clause are not
/// modeled; see `GRAY_OOZE_PSEUDOPOD` for why equipment degradation is
/// a system rather than a rider, and the rust monster's corrosive
/// sludge for the precedent.
///
/// **Amorphous** ("can move through a space as narrow as 1 inch")
/// collapses the same way the octopus's Compression does — there is no
/// gap on this board narrower than a tile for the clause to open.
///
/// Defensive identity: AC 9 and 22 hit points is nothing, and the
/// resistance and immunity envelope is everything. Acid, cold and fire
/// resistance covers three of the four elemental answers a low-level
/// party has, and the condition immunities — Blinded, Charmed,
/// Deafened, Exhaustion, Frightened, Grappled, Prone, Restrained — mean
/// the ooze cannot be controlled, only killed.
///
/// Stat shape: AC 9, ~22 HP (3d8+9), STR 12, DEX 6, CON 16, INT 1,
/// WIS 6, CHA 2. Speed 10. Skills Stealth. Senses Blindsight 60. Size
/// Medium. CR ½. XP 100 per RAW.
pub static GRAY_OOZE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GRAY_OOZE_PSEUDOPOD);
    CreatureTemplate {
        name: "Gray Ooze",
        // 'J' is the hippo and 'j' the jelly cohort's; 'z' reads as the
        // formless puddle and is free on the lowercase shelf.
        glyph: 'z',
        ac: 9,
        hitpoints: "3d8+9".parse().unwrap(),
        speed: 10.,
        strength: 12,
        intelligence: 1,
        dexterity: 6,
        wisdom: 6,
        constitution: 16,
        charisma: 2,
        skills: HashSet::from([Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Ooze,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
        ]),
        // An ooze has no anatomy to grab, no eyes to blind and no mind
        // to frighten. The list is RAW's verbatim, and it is what makes
        // the creature unanswerable by anything but damage.
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Prone,
            Condition::Restrained,
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
            &GRAY_OOZE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn gray_ooze_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Ooze);
        assert!(a.find_action("gray ooze pseudopod").is_some());
    }

    /// The envelope is the creature. AC 9 and 22 hit points would be
    /// scenery without it.
    #[test]
    fn the_ooze_cannot_be_controlled_only_killed() {
        let a = make();
        for c in [
            Condition::Blinded,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Prone,
            Condition::Restrained,
        ] {
            assert!(a.is_immune_to_condition(c), "{:?} should not stick", c);
        }
        for dt in [DamageType::Acid, DamageType::Cold, DamageType::Fire] {
            assert_eq!(a.damage_modifier(dt), Some(DamageModifier::Resistance));
        }
    }
}
