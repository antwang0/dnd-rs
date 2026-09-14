use crate::actions::class_features::CANNOT_ATTACK_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Fly — CR 0 Large beast, and the second creature in this
/// bestiary that cannot attack anything.
///
/// SRD 5.2 prints it in an unusual place: not in the monster chapter at
/// all, but inside the **Figurine of Wondrous Power** entry, as the
/// thing an Ebony Fly becomes. The whole stat block is a body —
///
/// > **Giant Fly.** *Large Beast, Unaligned.* AC 11, HP 19 (3d10 + 3),
/// > Speed 30 ft., Fly 60 ft. Str 14, Dex 13, Con 13, Int 2, Wis 10,
/// > Cha 3. Senses Darkvision 60 ft. CR 0.
///
/// — with no Actions block, no traits and no reaction. It is the only
/// stat block in the document whose entire purpose is to be sat on.
///
/// ## Why a creature with no attack is worth having
///
/// The shrieker fungus got here first and made the case: `CANNOT_ATTACK_TAG`
/// exists, `SummonItem::summons_combatants` already reads it off the
/// template, and the AI has been coping with a body that does nothing
/// since the fungus shipped. What the fly adds is the other half — it
/// is **`mountable`**, which is RAW (*"can be ridden as a mount"*), and
/// in this engine a Large flier with a saddle is a real payout rather
/// than a flavour line:
///
///   - `engine::mounts` gives the rider the mount's legs, so a Medium
///     character on this thing moves 60 ft a turn *through the air*;
///   - `engine::falling` prices what that costs when the fly is dropped
///     out from under them, which is the counterplay;
///   - and nothing else an item can put on the board carries a rider
///     upward. The Bronze Griffon flies and is Large, but it is a
///     predator with a stat block to spend — the fly is pure transport.
///
/// So the summon that produces it (the Ebony Fly figurine, see
/// `item_actions::SET_DOWN_EBONY_FLY`) is the engine's only item that
/// hands a party *altitude* instead of a second sword.
///
/// **Darkvision and nothing else.** The book gives it no Flyby, no
/// keen senses and no bite, and it gets none here. Adding a 1d4 nip to
/// "make it a monster" would be a monster this document does not print
/// — the same call `shrieker_fungi` documents one file over.
///
/// Stat shape per the SRD: AC 11, 19 HP (3d10+3), STR 14 / DEX 13 /
/// CON 13 / INT 2 / WIS 10 / CHA 3. Speed 30, fly 60. Darkvision 60.
/// CR 0.
pub static GIANT_FLY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Nothing but the default action set, and `CANNOT_ATTACK_TAG` to
    // close the two holes an empty attack row leaves open: Shove and
    // Grapple ride `DEFAULT_ACTIONS`, and both are attacks in the sense
    // RAW means. Without the tag a "creature with no attacks" would
    // still be shoving people off ledges.
    let actions = DEFAULT_ACTIONS.clone();
    CreatureTemplate {
        name: "Giant Fly",
        // 'y' (lowercase) — free in the beast band; 'f' is the fire
        // imp's and 'F' the fungal cohort's, and the fly is neither.
        glyph: 'y',
        ac: 11,
        // 3d10+3 = 19 average per the SRD.
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 30.,
        fly_speed: 60.,
        strength: 14,
        dexterity: 13,
        constitution: 13,
        intelligence: 2,
        wisdom: 10,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([CANNOT_ATTACK_TAG]),
        // RAW's Ebony Fly: "can be ridden as a mount". The whole point
        // of the stat block — see the docstring above.
        mountable: true,
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
            &GIANT_FLY_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(7),
            0,
        )
        .unwrap()
    }

    /// The two clauses that make this stat block what it is: it flies,
    /// and it carries somebody.
    #[test]
    fn a_giant_fly_is_a_large_flying_mount() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert!(a.is_mountable());
        assert!(a.base_fly_speed() > 0.0, "a giant fly that cannot fly");
        // One size larger than Medium, which is what `can_mount`'s
        // size gate asks for — a fly that were Medium would be a
        // figurine nobody could ride.
        assert_eq!(a.size(), crate::engine::types::Size::Large);
    }

    /// No attack, and that includes the two the default action list
    /// would otherwise have handed it.
    #[test]
    fn a_giant_fly_has_nothing_to_attack_with() {
        let a = make();
        assert!(
            a.has_passive_feature(CANNOT_ATTACK_TAG),
            "a giant fly with an opinion about violence"
        );
    }
}
