//! **Species traits** — the fourth axis a stat block is built on, and
//! the one this engine had no home for.
//!
//! A class gives a character a chassis, a subclass gives it a
//! direction, a feat is the thing none of those decided; a species is
//! the body all three are hung on. SRD 5.2 prints nine of them —
//! Dragonborn, Dwarf, Elf, Gnome, Goliath, Halfling, Human, Orc,
//! Tiefling — and the engine carries traits for six, every one of them
//! declared in `actions::class_features` beside the class features it
//! is named for.
//!
//! That was fine while a species trait was a boolean (`has_lucky`,
//! `has_dwarven_resilience`) or a single action (the Dragonborn's
//! breath, the Tiefling's Hellish Rebuke). The Goliath is not: SRD
//! 5.2's Giant Ancestry is six mutually exclusive benefits, five of
//! which touch a different engine chokepoint, plus two traits that
//! apply whichever of the six was chosen. That is a species-shaped
//! feature rather than a class-shaped one, and it is what this module
//! is for.
//!
//! **The six older species traits stay where they are.** Every one of
//! them reaches the engine as a `&'static str` tag on a cohort row or
//! as a template boolean, so which module the constant is declared in
//! is a naming question and not a wiring one — moving them would be a
//! large mechanical diff across the bestiary that changed no behaviour.
//! New species work lands here.
//!
//! ## The Goliath
//!
//! > *Creature Type: Humanoid. Size: Medium (about 7–8 feet tall).
//! > Speed: 35 feet.*
//!
//! | trait | category | engine surface |
//! |-------|----------|----------------|
//! | Powerful Build | always on | advantage on the check to end a grapple |
//! | Large Form | bonus action, 1/rest | `RESIZING_CONDITIONS` + speed + STR checks |
//! | Cloud's Jaunt | bonus action, N/rest | `TeleportActor`, 30 ft |
//! | Fire's Burn | on hit, N/rest | once-per-turn weapon die rider, +1d10 fire |
//! | Frost's Chill | on hit, N/rest | the same rider, +1d6 cold and a −10 ft slow |
//! | Hill's Tumble | on hit, N/rest | on-hit condition mark, Prone |
//! | Stone's Endurance | reaction, N/rest | `REACTIVE_DAMAGE_CLAMPS`, 1d12 + CON |
//! | Storm's Thunder | reaction, N/rest | any-attack reflect, 1d8 thunder |
//!
//! Five of the six ancestries are one row on a cohort that already
//! existed, which is the best argument there is that the cohorts were
//! cut along the right joints. The sixth — Cloud's Jaunt — is Misty
//! Step with the slot swapped for a charge, and rides the same
//! `TeleportActor` side effect and the same AI escape lane.
//!
//! ## What "a number of times equal to your Proficiency Bonus" means here
//!
//! RAW sizes every Giant Ancestry pool at the goliath's proficiency
//! bonus, which is +2 on the CR-2 chassis these templates are built to.
//! `GIANT_ANCESTRY_USES` is that number, and it is declared once here
//! rather than six times in `class_features::FEATURE_CHARGES` so the
//! six ancestries cannot drift apart — they are one trait with six
//! faces, and a pool that differed between them would be a typo rather
//! than a decision.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema, bonus_action_only, first_target_location},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, Resource, TeleportActor},
        types::Coordinate,
    },
};

/// How many uses each Giant Ancestry benefit gets per long rest.
///
/// RAW is the goliath's proficiency bonus, which is +2 at the level
/// these chassis are built to — so this is RAW to the number rather
/// than the "collapse a scaling pool to something spendable" compromise
/// most of `FEATURE_CHARGES` is making.
pub const GIANT_ANCESTRY_USES: u32 = 2;

/// **Powerful Build** — *"You have Advantage on any ability check you
/// make to end the Grappled condition."*
///
/// Read at `EncounterInstance::escape_check_mode`, which is the roll
/// `default_actions::GrappleEscape` makes on both of its lanes: the
/// contest against a named grappler and the flat-DC check against a
/// hold nobody is on the other end of. Both get the advantage, because
/// RAW's clause is about the condition rather than about who applied
/// it — a goliath in a Web strains out of it as readily as one in an
/// ogre's fist.
///
/// RAW's second half — *"You also count as one size larger when
/// determining your carrying capacity"* — is not modeled, and could not
/// be: the engine tracks no encumbrance. It is the half that never
/// comes up in a fight.
///
/// Deliberately *not* a row on `Size::can_grapple`'s ladder. RAW makes
/// Powerful Build a bonus to escaping, not a promotion of the goliath's
/// size category, and the three other places the engine measures size
/// (what can be grappled, what can be shoved prone, what fits through a
/// gap) all read the real one.
pub const POWERFUL_BUILD_TAG: &str = "goliath.powerful_build";

/// **Large Form** — *"Starting at character level 5, you can change
/// your size to Large as a Bonus Action if you're in a big enough
/// space. This transformation lasts for 10 minutes or until you end it.
/// For that duration, you have Advantage on Strength checks, and your
/// Speed increases by 10 feet. Once you use this trait, you can't use
/// it again until you finish a Long Rest."*
///
/// One charge and one condition (`Condition::LargeForm`), which three
/// cohorts then read: `RESIZING_CONDITIONS` grows the footprint,
/// `CONDITION_SPEED_BONUSES` pays the +10, and
/// `STRENGTH_CHECK_MODE_CONDITIONS` hands over the advantage.
///
/// The interesting half is the footprint, for the reason the Rune
/// Knight's Giant's Might docstring gives: a footprint is what the
/// engine measures reach from, so a Large goliath threatens opportunity
/// attacks across a wider ring and can reach a caster standing one tile
/// further back. RAW's *"if you're in a big enough space"* needs no code
/// — `reconcile_footprints` is what actually stamps the larger body, and
/// it declines in a corridor and stamps the moment a neighbour steps
/// aside, exactly as Giant's Might already does.
///
/// It is *not* Giant's Might with a different name, and the differences
/// run in both directions: Large Form pays speed where Giant's Might
/// pays a damage die, and Giant's Might's advantage covers Strength
/// saves where this one is written for checks alone. They share a
/// resizing row and nothing else.
pub const LARGE_FORM_TAG: &str = "goliath.large_form";

/// **Cloud's Jaunt** (Cloud Giant ancestry) — *"As a Bonus Action, you
/// magically teleport up to 30 feet to an unoccupied space you can
/// see."*
///
/// Misty Step with the 2nd-level slot swapped for an ancestry charge,
/// and it is implemented as exactly that: the same `TeleportActor` side
/// effect, the same `can_move_to` landing check, the same
/// `SELF_TELEPORT_ESCAPES` rung on the AI's escape lane. What the swap
/// buys is who gets to make it — this is a blink on a martial chassis
/// that has no spell list at all.
pub const CLOUDS_JAUNT_TAG: &str = "goliath.clouds_jaunt";

/// **Fire's Burn** (Fire Giant ancestry) — *"When you hit a target with
/// an attack roll and deal damage to it, you can also deal 1d10 Fire
/// damage to that target."*
///
/// One row on `ONCE_PER_TURN_WEAPON_DIE_RIDERS`, and the first row on
/// that cohort to spend a per-rest charge as well as the once-a-turn
/// ledger. See the cohort's `charge_tag` field for why both gates ride
/// together: the charge is RAW's pool, and the ledger is the engine's
/// narrowing so a goliath with Extra Attack cannot spend the whole
/// day's ancestry inside one turn.
///
/// The largest single die on that cohort by some distance — 1d10 where
/// the class riders sit at 1d4 to 1d8 — which is the trade the pool is
/// buying: twice a day rather than every turn of every fight.
pub const FIRES_BURN_TAG: &str = "goliath.fires_burn";

/// **Frost's Chill** (Frost Giant ancestry) — *"When you hit a target
/// with an attack roll and deal damage to it, you can also deal 1d6
/// Cold damage to that target and reduce its Speed by 10 feet until the
/// start of your next turn."*
///
/// Fire's Burn's sibling row, trading four points of average damage for
/// the slow — and the slow is the half worth having. It installs
/// `Condition::Hobbled`, which is the −10 ft row the Slow weapon
/// mastery already put on `CONDITION_SPEED_BONUSES`; sharing it means a
/// goliath's chill and a club's mastery cannot stack into a −20, which
/// is what RAW says about two applications of the same reduction.
pub const FROSTS_CHILL_TAG: &str = "goliath.frosts_chill";

/// **Hill's Tumble** (Hill Giant ancestry) — *"When you hit a Large or
/// smaller creature with an attack roll and deal damage to it, you can
/// give that target the Prone condition."*
///
/// No die at all, which is why it sits on `ON_HIT_CONDITION_MARKS`
/// rather than beside its two siblings on the damage-rider cohort — and
/// why that cohort grew a size gate and a charge column to take it.
///
/// The strongest of the three on-hit ancestries and the one whose value
/// is hardest to read off the numbers: Prone costs the target half its
/// movement to stand up, hands every melee attacker advantage until it
/// does, and taxes its own attacks in the meantime. Twice a day, with
/// no save, off a swing the goliath was making anyway.
pub const HILLS_TUMBLE_TAG: &str = "goliath.hills_tumble";

/// **Stone's Endurance** (Stone Giant ancestry) — *"When you take
/// damage, you can take a Reaction to roll 1d12. Add your Constitution
/// modifier to the number rolled and reduce the damage by that total."*
///
/// One row on `REACTIVE_DAMAGE_CLAMPS`, in the `RollMinus` shape Parry
/// and Deflect Missiles already use, with the largest die on the
/// cohort.
///
/// **RAW's trigger is wider than the cohort's.** "When you take damage"
/// covers a failed save against a fireball; the clamp cohort is walked
/// from the two attack chokepoints and so only answers landed attacks.
/// That is the same narrowing Uncanny Dodge lives with — RAW's *"when
/// an attacker hits you"* happens to match, so nobody had to write it
/// down before. Widening the lane means giving the save path a
/// reaction-priced clamp of its own, which is a subsystem rather than a
/// row, and the attack half is where most of a fight's damage arrives.
pub const STONES_ENDURANCE_TAG: &str = "goliath.stones_endurance";

/// **Storm's Thunder** (Storm Giant ancestry) — *"When you take damage
/// from a creature within 60 feet of you, you can take a Reaction to
/// deal 1d8 Thunder damage to that creature."*
///
/// One row on `ANY_ATTACK_REFLECT_FEATURES`, the lane the Conquest
/// Paladin's Scornful Rebuke opened — and the row that made the lane
/// grow a die and a price. Scornful Rebuke is free, flat and permanent;
/// this rolls, costs a reaction, and runs out.
///
/// Narrowed the same way Stone's Endurance is, and it is the same
/// narrowing: the reflect lane fires off a landed attack rather than
/// off damage in general. The 60-foot clause needs no code — an attack
/// that reached the goliath came from someone the goliath can answer.
pub const STORMS_THUNDER_TAG: &str = "goliath.storms_thunder";

/// The six Giant Ancestry benefits, in the order SRD 5.2 prints them.
///
/// One list, for the reason `feats::FEAT_TAGS` is one list: the
/// invariant worth checking across an axis this shape is that every
/// member is actually reachable, and a benefit nobody carries is
/// invisible by construction. See
/// `every_giant_ancestry_is_carried_by_a_goliath`.
///
/// It is also what makes "exactly one" checkable. RAW has the goliath
/// choose one benefit; six templates each carrying one is the engine's
/// spelling of that, and `no_goliath_carries_two_ancestries` is what
/// keeps a copy-pasted template from quietly carrying two.
pub const GIANT_ANCESTRY_TAGS: &[&str] = &[
    CLOUDS_JAUNT_TAG,
    FIRES_BURN_TAG,
    FROSTS_CHILL_TAG,
    HILLS_TUMBLE_TAG,
    STONES_ENDURANCE_TAG,
    STORMS_THUNDER_TAG,
];

/// **Large Form** — the goliath's bonus action to grow. See
/// `LARGE_FORM_TAG`.
///
/// Structurally the Rune Knight's `GiantsMightAction`: the same
/// `feature_prime_ready` gate (a charge in hand and the condition not
/// already up) and the same `prime_self_condition` install. What
/// differs is the timer — RAW gives Large Form ten minutes where
/// Giant's Might gets one — and the condition, which pays speed and
/// Strength checks instead of a damage die.
pub struct LargeFormAction {}

impl Action for LargeFormAction {
    fn name(&self) -> &str {
        "large form"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["largeform", "loom"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        crate::actions::class_features::feature_prime_ready(
            encounter,
            caster_id,
            LARGE_FORM_TAG,
            Condition::LargeForm,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        crate::actions::class_features::prime_self_condition(
            encounter,
            caster_id,
            LARGE_FORM_TAG,
            Condition::LargeForm,
            // RAW's ten minutes. A hundred rounds outlives any fight
            // this engine runs, which is the honest reading — the
            // duration is not a clock the goliath has to watch, it is a
            // once-a-day posture.
            ConditionTimer::Rounds(100),
            "  large form: the goliath draws itself up, and the room gets smaller.",
        )
    }
}

pub static LARGE_FORM: LazyLock<LargeFormAction> = LazyLock::new(|| LargeFormAction {});

/// **Cloud's Jaunt** — the Cloud Giant ancestry's bonus-action blink.
/// See `CLOUDS_JAUNT_TAG`.
pub struct CloudsJauntAction {}

impl Action for CloudsJauntAction {
    fn name(&self) -> &str {
        "cloud's jaunt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["clouds jaunt", "jaunt", "cj"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles on the 2.5 ft grid, the same envelope Misty
        // Step gets.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        // RAW: "to an unoccupied space you can see".
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(CLOUDS_JAUNT_TAG))
            // The destination has to hold the goliath's whole footprint
            // — which is a live question for this species in a way it
            // is not for a Misty Stepping wizard, because a goliath in
            // Large Form is jaunting a body twice the width.
            && encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(CLOUDS_JAUNT_TAG);
        }
        encounter.log("  cloud's jaunt: the goliath steps through a knot of cloud.".to_string());
        // A teleport, so no opportunity attacks from the tiles it did
        // not cross — the same reason Misty Step queues this effect
        // rather than a `MoveActor`.
        vec![Box::new(TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static CLOUDS_JAUNT: LazyLock<CloudsJauntAction> = LazyLock::new(|| CloudsJauntAction {});
