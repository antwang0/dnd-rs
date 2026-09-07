//! The **Phantom Watchdog** — the body SRD 5.2's *Faithful Hound* puts
//! on the board and then walks away from.
//!
//! RAW is four sentences of stat block spread through a spell
//! description rather than a block of its own:
//!
//! > You conjure a phantom watchdog in an unoccupied space that you can
//! > see within range. […] No one but you can see the hound, and it is
//! > intangible and invulnerable. […] The hound has Truesight with a
//! > range of 30 feet. At the start of each of your turns, the hound
//! > attempts to bite one enemy within 5 feet of it. That enemy must
//! > succeed on a Dexterity saving throw or take 4d8 Force damage.
//!
//! Three of those four turn into rows on a `CreatureTemplate` without
//! argument — the invisibility is `innate_conditions`, the truesight is
//! a `SpecialSense`, the bite is a `SingleTargetSaveDamage` whose save
//! negates. The fourth is the interesting one, and it is the reason
//! this file carries a module doc.
//!
//! ## What the hound gives up to be a creature
//!
//! **It is not invulnerable.** The engine has no unattackable lane —
//! every occupant of a tile is a target — and the Shepherd druid's
//! totems already made and documented that trade. The hound gets the
//! same deal they got: a small hit-point pool, a high AC, and the
//! intangible envelope (immune to poison, resistant to the physical
//! trio) standing in for a sentence the type system cannot say. The
//! departure cuts the right way for the same reason it does there — an
//! enemy spending a turn punching a phantom is an enemy not punching
//! the wizard — and unlike the totems, killing this one is genuinely
//! hard: AC 18 with resistance to everything a sword does means most of
//! the bestiary needs two good rolls to make any progress at all.
//!
//! **It bites on its own initiative, not at the start of the caster's
//! turn.** RAW spends the caster's turn structure to fire the bite; the
//! engine has no channel for one actor to act inside another's turn
//! outside the reaction lane, and a reaction is not what RAW describes.
//! The Eldritch Cannon made this trade first and for the same reason.
//! The frequency is identical — one bite per round either way — and
//! what moves is only *when* in the round it lands.
//!
//! **It does not move.** RAW's "on your later turns, you can take a
//! Magic action to move the hound up to 30 feet" is the same
//! missing channel again, so `speed: 0.` is the whole implementation of
//! "it stays where you put it". That is what makes the spell's real
//! decision *where* rather than *whether*: a hound conjured in the back
//! rank is 27 hit points of scenery, and one conjured in the doorway the
//! ogres are coming through bites every round for the rest of the fight.
//!
//! Not modeled at all: the barking. RAW's "the hound starts barking
//! loudly" when an unfamiliar creature comes within 30 feet is an
//! alarm, and an encounter that has already rolled initiative has
//! nothing left to be alarmed about.

use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SingleTargetSaveDamage;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::eldritch_cannons::CONSTRUCT_CONDITION_IMMUNITIES;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The DC the hound's bite is rolled against.
///
/// RAW names no number: the save is the spell's, and the spell's is the
/// caster's. The engine's summons carry their own — a
/// `SingleTargetSaveDamage` has no channel back to whoever called it,
/// which is the same bargain `summoned_spirits::SPIRIT_SAVE_DC` strikes
/// one rung down. 15 is the spell save DC of a caster with a +4
/// spellcasting modifier and a +3 proficiency bonus, which is what a
/// character holding fourth-level slots looks like, so the substitution
/// costs the spell nothing at the level it is cast.
const HOUND_SAVE_DC: i32 = 15;

/// **Bite** — RAW's whole offensive line: *"That enemy must succeed on
/// a Dexterity saving throw or take 4d8 Force damage."*
///
/// `save_negates` is doing real work here. Nearly every save-for-damage
/// printing in the bestiary halves on a success; this one says nothing
/// at all about a success, so a target that dodges takes nothing. That
/// is a third of the ability's expected damage, and it is what makes
/// the hound a threat a nimble creature can walk past and a heavy one
/// cannot.
pub static HOUND_BITE: SingleTargetSaveDamage = SingleTargetSaveDamage::new(
    "phantom bite",
    &["bite", "hound bite"],
    crate::actions::action_template::MELEE_REACH,
    AbilityScoreType::Dexterity,
    HOUND_SAVE_DC,
    Dice::new(4, 8),
    DamageType::Force,
)
.save_negates();

/// Phantom Watchdog — the Medium construct **Faithful Hound** conjures.
///
/// Glyph 'd' for **d**og.
pub static PHANTOM_WATCHDOG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HOUND_BITE);
    CreatureTemplate {
        name: "Phantom Watchdog",
        glyph: 'd',
        // The armour half of RAW's invulnerability. Nothing about a
        // phantom is hard to hit; what this number says is that hitting
        // it is not how the encounter gets solved.
        ac: 18,
        hitpoints: "5d8+5".parse().unwrap(),
        // RAW roots it where it was conjured — see the module doc.
        speed: 0.,
        // A watchdog's body, borrowed from the Mastiff, because
        // something has to fill these rows and the bite reads none of
        // them: its damage is flat dice and its DC is `HOUND_SAVE_DC`.
        strength: 13,
        dexterity: 14,
        constitution: 12,
        intelligence: 3,
        wisdom: 12,
        charisma: 7,
        // RAW, exactly: "The hound has Truesight with a range of 30
        // feet." The one line of the stat block that transfers without
        // any interpretation at all, and the one that makes the hound
        // worth putting *in front of* the party rather than beside it —
        // an invisible assassin creeping up the corridor is something
        // this creature can see and the wizard behind it cannot.
        senses: HashSet::from([SpecialSense::Truesight(30)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Medium,
        // A conjured phantom with no mind, no biology and no will of
        // its own. Construct also buys the condition envelope below
        // wholesale, which is the closest the engine gets to RAW's
        // "intangible".
        creature_type: CreatureType::Construct,
        actions,
        // Poison immunity plus resistance to the three things a weapon
        // does: what "intangible" means to an engine that has to let
        // the ogre swing at something.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        condition_immunities: CONSTRUCT_CONDITION_IMMUNITIES.clone(),
        // RAW: "No one but you can see the hound." The engine has one
        // notion of unseen and this is it — permanent, because nothing
        // about the hound's own bite is supposed to give it away.
        innate_conditions: vec![(Condition::Invisible, ConditionTimer::Permanent)],
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn instance() -> ActorInstance {
        ActorInstance::from_creature_template(
            &PHANTOM_WATCHDOG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .expect("the watchdog instantiates")
    }

    #[test]
    fn the_hound_stands_where_it_was_put() {
        let hound = instance();
        assert_eq!(hound.speed(), 0.);
    }

    #[test]
    fn the_hound_sees_the_unseen() {
        let hound = instance();
        assert!(hound.senses().contains(&SpecialSense::Truesight(30)));
    }

    /// RAW: *"No one but you can see the hound."*
    ///
    /// Routed through `instantiate_creature` rather than through the
    /// constructor, because that is where the `innate_conditions` lane
    /// is actually applied — a test that built the actor directly would
    /// read an empty condition set off a template whose list is
    /// perfectly correct, and a test that read the template's own list
    /// would pass even if nothing ever applied it.
    #[test]
    fn the_hound_arrives_already_unseen() {
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        let mut e = EncounterInstance::from_params(
            &TerrainGenParams {
                width: 20,
                height: 20,
                branch_depth: 0,
                branch_prob: 0.0,
            },
            &ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            },
            Some(3),
        )
        .unwrap();
        let id = e
            .instantiate_creature(&PHANTOM_WATCHDOG_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert!(e.actors[&id].has_condition(Condition::Invisible));
    }

    #[test]
    fn the_bite_leaves_a_successful_save_untouched() {
        // RAW prints no "Success: Half damage only" clause on this one,
        // which is the whole reason `save_negates` exists.
        assert!(!HOUND_BITE.half_on_save);
        assert_eq!(HOUND_BITE.damage_dice, Dice::new(4, 8));
        assert_eq!(HOUND_BITE.damage_type, DamageType::Force);
    }

    #[test]
    fn the_hound_carries_its_bite() {
        let hound = instance();
        assert!(hound.find_action("phantom bite").is_some());
    }
}
