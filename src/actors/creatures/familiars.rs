//! The **spirit familiar** — the body SRD 5.2's *Find Familiar* puts on
//! the board, and the one creature in this bestiary that is defined by
//! what it may not do.
//!
//! RAW hands the caster an option table and one hard clause:
//!
//! > You gain the service of a familiar, a spirit that takes an animal
//! > form you choose: Bat, Cat, Frog, Hawk, Lizard, Octopus, Owl, Rat,
//! > Raven, Spider, Weasel, or another Beast that has a Challenge
//! > Rating of 0. […] the familiar has the statistics of the chosen
//! > form […] though it is a Celestial, Fey, or Fiend (your choice)
//! > instead of a Beast.
//! >
//! > **Combat.** The familiar is an ally to you and your allies. It
//! > rolls its own Initiative and acts on its own turn. **A familiar
//! > can't attack**, but it can take other actions as normal.
//!
//! ## Why an owl
//!
//! The option table collapses to one branch, which is this engine's
//! standing treatment of a RAW option table — see
//! `spells::SummonSpell::template` for why a cast-time choice has
//! nowhere to live when the template is a `&'static` shared by every
//! cast on the board. Eleven of the twelve named forms differ only in
//! ability scores nobody rolls and a movement lane the board is flat
//! for; the owl differs in two things that are worth a rung on the
//! ladder:
//!
//!   - **Flyby.** A familiar's entire combat contribution here is
//!     standing next to something and Helping, and Help costs the whole
//!     Action. Flyby is what makes that survivable: the owl flies in,
//!     grants advantage, and leaves without provoking. Any other form
//!     spends its next turn being killed by whatever it walked up to,
//!     which with one hit point is not a long conversation.
//!   - **Darkvision 120.** The longest on the CR-0 shelf by a factor of
//!     two, which is the whole reason to have one at all under
//!     `AmbientLight::Dark`.
//!
//! ## Why it is a template rather than the owl with a flag
//!
//! `OWL_TEMPLATE` is the beast SRD 5.2 prints, and this is a spirit
//! wearing its shape. The two differ on three rows that all have to
//! move together:
//!
//!   - **The talons are gone.** RAW's "a familiar can't attack" is
//!     enforced by `CANNOT_ATTACK_TAG` rather than by the empty action
//!     row, because Shove and Grapple ride `DEFAULT_ACTIONS` and are
//!     attacks in exactly the sense RAW means. An empty attack row
//!     would have said nothing about either — see
//!     `ActorInstance::blocked_from_attacking`.
//!   - **It is a Celestial**, not a Beast. Of RAW's three offered types
//!     the celestial is the one with a mechanical surface in this
//!     engine: creature type is read by Hunter's Mark cohorts, by the
//!     paladin's smite riders and by half a dozen "against a fiend or
//!     undead" clauses, and a familiar that reads as a beast would be
//!     one a Ranger's Favored Enemy could pick out. A spirit called up
//!     out of an incense burner should not be somebody's quarry.
//!   - **It does not answer the bestiary.** The owl is rolled by the
//!     encounter generator; the familiar arrives one way only, and the
//!     absence of a `cr` worth rolling for is part of the point.
//!
//! ## What is deliberately not modeled
//!
//! Everything RAW hangs off the *telepathic bond*: seeing through the
//! familiar's eyes, the hundred-foot leash, the touch-spell delivery,
//! the pocket dimension it can be dismissed to. All four are channels
//! between two creatures' turns, and the engine has one of those — the
//! reaction — which is not what any of them describes. The
//! touch-delivery clause is the one worth naming, because it is the
//! only one with real combat consequence: it would let a wizard cast
//! Vampiric Touch from thirty feet away, and it needs a "spend another
//! actor's reaction as part of my own action" lane that nothing else in
//! the engine wants yet.

use crate::actions::class_features::{CANNOT_ATTACK_TAG, FLYBY_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Spirit Familiar — the owl form of SRD 5.2's *Find Familiar*, with
/// the owl's stat block and none of its talons.
///
/// Stat shape is `OWL_TEMPLATE`'s exactly: AC 11, ~1 HP (1d4−1, floored
/// at 1), STR 3, DEX 13, CON 8, INT 2, WIS 12, CHA 7, speed 5 walking /
/// 60 flying, Perception and Stealth, Darkvision 120, Tiny, CR 0. What
/// changes is the creature type, the missing attack, and the tag that
/// makes the missing attack a rule rather than an omission.
pub static FAMILIAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Nothing but the default set — no talons. The familiar Dashes,
    // Dodges, Disengages, Hides and Helps, which is RAW's "it can take
    // other actions as normal", and the hostile half of that list is
    // closed by `CANNOT_ATTACK_TAG` rather than by what is missing here.
    let actions = DEFAULT_ACTIONS.clone();
    CreatureTemplate {
        name: "Familiar",
        // 'w', the same glyph the owl carries, because it is an owl.
        // Glyphs are only ever required to be unique within a PC family;
        // across the bestiary they are a silhouette, and two things that
        // look the same on the map should read the same.
        glyph: 'w',
        ac: 11,
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 5.,
        fly_speed: 60.,
        strength: 3,
        intelligence: 2,
        dexterity: 13,
        wisdom: 12,
        constitution: 8,
        charisma: 7,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        cr: 0.0,
        size: Size::Tiny,
        // RAW offers Celestial, Fey or Fiend. See the module docs for
        // why the celestial is the branch that ships.
        creature_type: CreatureType::Celestial,
        actions,
        features: HashSet::from([FLYBY_TAG, CANNOT_ATTACK_TAG]),
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
            &FAMILIAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn familiar_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(
            a.creature_type(),
            CreatureType::Celestial,
            "a spirit called up out of an incense burner is not somebody's Favored Enemy"
        );
        assert!(a.is_airborne(), "the owl form flies");
    }

    /// RAW's one hard clause, and the reason it is a tag rather than an
    /// empty action row: the familiar carries Shove and Grapple like
    /// every other creature on the board, and may take neither.
    #[test]
    fn a_familiar_may_not_swing_shove_or_grapple() {
        let a = make();
        assert!(a.blocked_from_attacking());
        assert!(
            a.find_action("owl talons").is_none(),
            "the familiar wears the owl's shape, not its talons"
        );
        for hostile in ["shove", "grapple"] {
            assert!(
                a.find_action(hostile).is_some(),
                "{hostile} is on the default list and the familiar carries it"
            );
        }
    }

    /// The half of the clause that is *not* a block. "It can take other
    /// actions as normal" is the whole of the familiar's combat
    /// contribution, and Help is what that contribution is.
    #[test]
    fn a_familiar_may_still_help() {
        use crate::actions::action_template::Action;

        let a = make();
        assert!(a.find_action("help").is_some());
        assert!(
            !crate::actions::default_actions::HELP.is_harmful(),
            "Help has to stay on the harmless side of the gate that stops the talons"
        );
    }
}
