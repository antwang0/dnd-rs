use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        Action, TargetingSchema, bonus_action_and_slot, bonus_action_only, first_target_id,
        first_target_location, free_cost,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, GiveResource, Heal,
            Resource,
        },
        types::{Coordinate, DamageType},
    },
};

/// Class-feature tags that refresh on a 5e short rest. Read by
/// `ActorInstance::short_rest` to repopulate `features_remaining` for any
/// matching tag the actor has on their template. The remaining tags in
/// this module are long-rest features and only restore via `long_rest`.
///
/// Keep this list in sync with new short-rest features as they're added —
/// the test `short_rest_features_listed_here_match_class_features` (in
/// class_features tests) covers the obvious additions.
pub const SHORT_REST_FEATURES: &[&str] = &[
    SECOND_WIND_TAG,
    ACTION_SURGE_TAG,
    ARCANE_RECOVERY_TAG,
    PRESERVE_LIFE_TAG,
    CUTTING_WORDS_TAG,
    BREATH_WEAPON_TAG,
];

/// Battle Master maneuver tags. RAW: maneuvers cost superiority dice
/// which refresh on a short or long rest. We collapse the dice pool to
/// per-tag once-per-rest charges so the gating stays uniform with the
/// other class features; the tags are listed both here and in the
/// `SHORT_REST_FEATURES` registry below so a short rest refreshes them.
pub const BATTLE_MASTER_MANEUVERS: &[&str] = &[
    TRIP_ATTACK_TAG,
    MENACING_ATTACK_TAG,
    DISARMING_ATTACK_TAG,
    PUSHING_ATTACK_TAG,
    GOADING_ATTACK_TAG,
    PRECISION_ATTACK_TAG,
    SWEEPING_ATTACK_TAG,
    FEINTING_ATTACK_TAG,
    LUNGING_ATTACK_TAG,
    RALLY_TAG,
    COMMANDERS_STRIKE_TAG,
    DISTRACTING_ATTACK_TAG,
];

/// Tags used by `ActorInstance::feature_available` / `spend_feature` to
/// gate once-per-long-rest class features. Stored as `&'static str` so
/// actor state stays a flat HashSet instead of carrying an enum import.
pub const SECOND_WIND_TAG: &str = "fighter.second_wind";
pub const ACTION_SURGE_TAG: &str = "fighter.action_surge";

/// Tag for the Half-Orc Relentless Endurance racial trait. Passive
/// no-action feature: once per long rest, when damage would reduce
/// the holder to 0 HP, they drop to 1 HP instead. We weave the trigger
/// into `ActorInstance::take_damage` next to the Death Ward hook —
/// mirrors the "intercept the 0-HP transition and roll back to 1"
/// shape, but with the once-per-rest feature flag in place of the
/// short-duration condition.
pub const RELENTLESS_ENDURANCE_TAG: &str = "half_orc.relentless_endurance";

/// Tag for the Sahuagin Blood Frenzy racial trait. Passive always-on
/// feature: the holder rolls melee attacks with advantage against any
/// target that doesn't have all its hit points. We weave the gate into
/// `compute_attack_mode` next to Pack Tactics — when the attacker has the
/// tag, the swing is melee, and the target's `current_hp() <
/// max_hitpoints()`, the swing gets advantage. The tag lives in the
/// passive-feature pool so a long-rest never "spends" it (it's not a
/// per-rest pool — it always fires on a wounded target).
pub const BLOOD_FRENZY_TAG: &str = "sahuagin.blood_frenzy";

/// Common gating shape for once-per-rest class features: the caster must
/// exist, be combat-active, and have an unspent charge of `tag`. Returns
/// true when all three hold. Centralizes the `is_some_and(|a|
/// a.is_combat_active() && a.feature_available(tag))` boilerplate that
/// every `custom_validate_input` previously open-coded — keeps the gate
/// to a one-line call and gives a single chokepoint for future cross-
/// cutting checks (e.g. "feature suppressed while Silenced").
pub fn feature_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
) -> bool {
    encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.is_combat_active() && a.feature_available(tag))
}

/// Gating shape for "while-raging" passive features whose action half
/// fires only inside an active Rage condition: the holder must exist, be
/// combat-active, carry the `tag` passive feature flag, AND have the
/// `Raging` condition. The classic Berserker Frenzy / Totem Warrior
/// secondary action shape — Frenzy and Eagle Dive (and any future
/// rage-gated bonus action — Reckless Throw, etc.) both route through
/// this one helper instead of re-inlining the three-clause chain.
pub fn raging_feature_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
) -> bool {
    encounter.actors.get(&caster_id).is_some_and(|a| {
        a.is_combat_active() && a.has_passive_feature(tag) && a.has_condition(Condition::Raging)
    })
}

/// Fighter Second Wind — bonus action; restore 1d10 + level HP. Once per
/// long rest. Self-targeted; only valid while combat-active (no reviving
/// yourself out of dying via this).
pub struct SecondWind {}

impl Action for SecondWind {
    fn name(&self) -> &str {
        "second wind"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sw", "wind"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, SECOND_WIND_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(1, 10));
        let level = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.level())
            .unwrap_or(1);
        let amount = raw + level;
        // Spend the feature now so a duplicate queued use can't slip
        // through — keeps inventory-style consistency with potions.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(SECOND_WIND_TAG);
        }
        encounter.log(format!(
            "  second wind: 1d10({})+{} = {} HP",
            raw, level, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static SECOND_WIND: LazyLock<SecondWind> = LazyLock::new(|| SecondWind {});

/// Fighter Action Surge — free; gain an extra Action this turn. Once per
/// long rest. Doesn't grant a Bonus Action or Movement (RAW: Action only).
pub struct ActionSurge {}

impl Action for ActionSurge {
    fn name(&self) -> &str {
        "action surge"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["as", "surge"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // RAW: Action Surge is "no action required" — no cost.
        free_cost()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, ACTION_SURGE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(ACTION_SURGE_TAG);
        }
        encounter.log("  action surge: extra Action gained.".to_string());
        grant_extra_action(caster_id)
    }
}

pub static ACTION_SURGE: LazyLock<ActionSurge> = LazyLock::new(|| ActionSurge {});

/// Tag for Cunning Action — at-will class feature, not consumable, so
/// it never appears in `features_remaining`. Kept as a const for
/// symmetry with the once-per-rest tags above and so creature templates
/// can declare it explicitly.
pub const CUNNING_ACTION_TAG: &str = "rogue.cunning_action";

/// 5e Rogue Assassin **Assassinate** (level 3 subclass) feature tag.
/// Passive once-only rider: the assassin rolls with advantage on any
/// attack against a creature that hasn't taken a turn yet in this
/// combat. RAW also crits on a hit against a *surprised* target, but
/// we don't model surprise as a discrete state — the engine starts
/// every encounter in initiative order with all actors eligible.
///
/// Read at `EncounterInstance::compute_attack_mode` next to the Pack
/// Tactics / Wolf Totem advantage clauses; the target's
/// `has_taken_turn_in_combat` latch is set on the first turn-start
/// (see `start_turn_for`). Stored as a `has_passive_feature` flag so
/// it never consumes a per-rest charge — the gate is purely the
/// target-side latch.
pub const ASSASSINATE_TAG: &str = "rogue.assassinate";

/// Rogue Cunning Action — bonus-action Dash. 5e gives the rogue a choice
/// of Dash, Disengage, or Hide as a bonus action; we expose Dash here
/// (the most universally useful) and leave a follow-up CunningDisengage
/// / CunningHide pair that mirror the same gating. This is the
/// signature once-a-turn rogue mobility tool.
pub struct CunningDash {}

impl Action for CunningDash {
    fn name(&self) -> &str {
        "cunning dash"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cdash", "ca-dash"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
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

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  cunning dash: extra movement gained.".to_string());
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static CUNNING_DASH: LazyLock<CunningDash> = LazyLock::new(|| CunningDash {});

/// Rogue Cunning Disengage — bonus-action Disengage. Same effect as the
/// regular Disengage action (your movement this turn doesn't provoke
/// OAs), at the cheaper bonus-action cost.
pub struct CunningDisengage {}

impl Action for CunningDisengage {
    fn name(&self) -> &str {
        "cunning disengage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cdis", "ca-dis"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
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

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDisengaging {
            actor_id: caster_id,
            disengaging: true,
        })]
    }
}

pub static CUNNING_DISENGAGE: LazyLock<CunningDisengage> = LazyLock::new(|| CunningDisengage {});

/// Rogue Cunning Hide — bonus-action Hide. Same condition as the regular
/// Hide action (Hidden flag for one-shot attack-advantage), at the
/// cheaper bonus-action cost. Keeps the rogue's signature cunning-action
/// trio symmetric (Dash / Disengage / Hide).
pub struct CunningHide {}

impl Action for CunningHide {
    fn name(&self) -> &str {
        "cunning hide"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["chide", "ca-hide"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
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

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Hidden lasts until the rogue's next attack — same one-shot
        // attack-advantage rider as the Hide Action. Tracked via the
        // existing `Hidden` condition.
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Hidden,
            timer: ConditionTimer::Permanent,
        })]
    }
}

pub static CUNNING_HIDE: LazyLock<CunningHide> = LazyLock::new(|| CunningHide {});

/// 5e Tasha's Rogue **Steady Aim** (level 3 alternate Cunning Action).
/// Bonus action: grant the rogue advantage on their next attack roll this
/// turn at the cost of their speed dropping to 0 until end of turn.
/// Pairs naturally with Sneak Attack (advantage qualifies for the rider)
/// and the rogue's ranged finesse options — the speed-zero cost is
/// minor for a sniping rogue and the advantage gate replaces the
/// ally-adjacency / disadvantage-free condition that Sneak Attack
/// normally checks.
///
/// RAW gate: "only if you haven't moved during this turn". Enforced via
/// `ActorInstance::has_moved_this_turn`, which compares the current
/// movement budget against `speed()`. After use, `zero_movement()`
/// drains the budget so the rest of the turn is locked in place. The
/// advantage rides on the existing `Helped` one-shot condition (cleared
/// on the next swing in `clear_attack_advantage_riders`).
pub struct SteadyAim {}

impl Action for SteadyAim {
    fn name(&self) -> &str {
        "steady aim"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["aim", "sa-rogue"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && !a.has_moved_this_turn()
                // No-stack: re-priming would just refresh the timer.
                && !a.has_condition(Condition::Helped)
        })
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.zero_movement();
            format!(
                "{} takes steady aim — speed drops to 0 for the rest of the turn.",
                actor.name()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Helped,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static STEADY_AIM: LazyLock<SteadyAim> = LazyLock::new(|| SteadyAim {});

/// 5e 2024 Rogue **Cunning Strike** flavors. Each variant is a bonus-action
/// prime: the rogue declares which trick they'll layer onto their next
/// Sneak Attack, trading one (or two) sneak-attack dice for a tactical
/// effect. The shortsword's sneak-attack rider walks the active prime
/// table, deducts the dice cost, and applies the effect — see the
/// `consume_cunning_strike_*` chokepoints in `RogueShortsword::side_effects`.
///
/// We model the four 2024 base variants:
/// - **Poison** (1d6 cost): CON save vs rogue's DEX-based DC; on fail the
///   target is `Poisoned` for 10 rounds (1 minute RAW).
/// - **Trip** (1d6 cost): DEX save vs the same DC; on fail the target is
///   knocked Prone (Large or smaller — gated at the consume site).
/// - **Withdraw** (1d6 cost): no save; immediately after the sneak the
///   rogue moves up to half speed without provoking OAs.
/// - **Daze** (2d6 cost): CON save; on fail the target's next turn loses
///   their Action and Reaction (modeled via the existing `MindWhipped`
///   action-clip plus `NoReaction`).
///
/// Each variant is validated mutually-exclusive (no double-priming) and
/// only valid when sneak attack hasn't been used this turn — otherwise
/// the bonus-action would be wasted on a swing that can't carry the
/// rider. Self-clearing via `UntilStartOfNextTurn` if the rogue never
/// connects with a sneak-eligible swing.
///
/// Cunning Strike: Poison variant. 1d6 sneak-attack die cost; on a
/// successful sneak-attack hit, the target makes a CON save vs the
/// rogue's DEX-based DC (8 + prof + DEX). On fail, Poisoned for 10
/// rounds (1 minute RAW). Bonus-action prime.
pub struct CunningStrikePoison {}

impl Action for CunningStrikePoison {
    fn name(&self) -> &str {
        "cunning strike (poison)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-poison", "cspoison", "cunningpoison"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikePoison,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_POISON: LazyLock<CunningStrikePoison> =
    LazyLock::new(|| CunningStrikePoison {});

/// Cunning Strike: Trip variant. 1d6 sneak-attack die cost; target makes
/// a DEX save (Large or smaller). On fail, knocked Prone. Bonus-action
/// prime.
pub struct CunningStrikeTrip {}

impl Action for CunningStrikeTrip {
    fn name(&self) -> &str {
        "cunning strike (trip)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-trip", "cstrip", "cunningtrip"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeTrip,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_TRIP: LazyLock<CunningStrikeTrip> =
    LazyLock::new(|| CunningStrikeTrip {});

/// Cunning Strike: Withdraw variant. 1d6 sneak-attack die cost; on a
/// successful sneak-attack hit, the rogue immediately moves up to half
/// their speed without provoking opportunity attacks. Bonus-action prime.
pub struct CunningStrikeWithdraw {}

impl Action for CunningStrikeWithdraw {
    fn name(&self) -> &str {
        "cunning strike (withdraw)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-withdraw", "cswith", "cunningwithdraw"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeWithdraw,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_WITHDRAW: LazyLock<CunningStrikeWithdraw> =
    LazyLock::new(|| CunningStrikeWithdraw {});

/// Cunning Strike: Daze variant. 2d6 sneak-attack die cost (RAW: 2
/// dice — strongest variant); target makes a CON save on a successful
/// sneak hit, and on fail their next turn loses its Action and Reaction
/// (we layer the action clip via `MindWhipped` plus a `NoReaction`).
/// Bonus-action prime.
pub struct CunningStrikeDaze {}

impl Action for CunningStrikeDaze {
    fn name(&self) -> &str {
        "cunning strike (daze)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-daze", "csdaze", "cunningdaze"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Daze costs 2d6 — only worth priming when the rogue's sneak
        // attack pool can spare 2 dice (level 3+ = 2d6, level 5+ = 3d6).
        if !cunning_strike_prime_ok(encounter, caster_id) {
            return false;
        }
        encounter.actors.get(&caster_id).is_some_and(|a| {
            crate::actions::class_attacks::sneak_attack_dice_for_level(a.level()) >= 2
        })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeDaze,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_DAZE: LazyLock<CunningStrikeDaze> =
    LazyLock::new(|| CunningStrikeDaze {});

/// Shared validation for the Cunning Strike bonus-action primes. Returns
/// true when:
/// - the rogue is combat-active,
/// - no other Cunning Strike prime is already up (mutually exclusive — the
///   first prime survives the bonus-action burn, the duplicate is the
///   wasted resource so we reject),
/// - the rogue hasn't already used Sneak Attack this turn (priming after
///   the once-per-turn fuse is spent is just a bonus-action waste).
fn cunning_strike_prime_ok(encounter: &EncounterInstance, caster_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&caster_id) else {
        return false;
    };
    if !actor.is_combat_active() {
        return false;
    }
    if actor.sneak_attack_used() {
        return false;
    }
    !actor.has_any_cunning_strike_prime()
}

/// Tag for Fighter's Indomitable — once per long rest.
pub const INDOMITABLE_TAG: &str = "fighter.indomitable";

/// Fighter Indomitable — free no-cost self-flag. Sets a one-shot "reroll
/// the next failed save" marker on the actor via `mark_indomitable_pending`.
/// The reroll lives at the save site (`EncounterInstance::roll_save`):
/// if the marker is set and the save fails, the engine re-rolls once and
/// keeps the better result, then clears the marker. Once per long rest
/// (consumed eagerly here so a duplicate queued use can't double-dip).
pub struct Indomitable {}

impl Action for Indomitable {
    fn name(&self) -> &str {
        "indomitable"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["indom", "indo"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
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
        // No action / bonus action cost — RAW: "no action required",
        // just spend the feature.
        free_cost()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, INDOMITABLE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(INDOMITABLE_TAG);
            actor.mark_indomitable_pending();
        }
        encounter.log("  indomitable: next failed save will be re-rolled.".to_string());
        Vec::new()
    }
}

pub static INDOMITABLE: LazyLock<Indomitable> = LazyLock::new(|| Indomitable {});

/// Class-feature tag for Barbarian's Rage — once per long rest.
pub const RAGE_TAG: &str = "barbarian.rage";

/// Rage — barbarian feature, bonus action. Applies the `Raging` condition
/// to the caster: resistance to bludgeoning / piercing / slashing damage
/// (folded into `effective_damage`), and advantage on STR checks / saves
/// (read by `compute_save_mode`). Lasts 10 rounds (approximation of the
/// 5e 1-minute duration). Once-per-long-rest gated on the `RAGE_TAG`
/// feature flag.
pub struct Rage {}

impl Action for Rage {
    fn name(&self) -> &str {
        "rage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["rg", "anger"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, RAGE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            RAGE_TAG,
            Condition::Raging,
            ConditionTimer::Rounds(10),
            "  rage: barbarian enters a battle frenzy.",
        )
    }
}

pub static RAGE: LazyLock<Rage> = LazyLock::new(|| Rage {});

/// 5e Barbarian Path of the Berserker — **Frenzy** (level 3). While raging,
/// the barbarian can use a bonus action on each of their turns to make
/// one additional melee weapon attack. Passive subclass tag (no per-rest
/// charge — the rate limit is the once-per-turn bonus action lane plus the
/// Rage duration ceiling). Held in `features_max` so the `Frenzy` action's
/// `custom_validate_input` can gate on `has_passive_feature(FRENZY_TAG)`
/// without re-checking the per-rest pool.
///
/// RAW: after the rage ends, the barbarian suffers one level of exhaustion.
/// We skip the exhaustion rider for now — modeling the post-rage hook
/// requires a timer-expiry callback that doesn't exist yet. The 10-round
/// Rage cap (and the once-per-long-rest gate on Rage itself) keeps the
/// Frenzy uses bounded per encounter regardless.
pub const FRENZY_TAG: &str = "barbarian.frenzy";

/// Berserker Frenzy bonus action: spend a bonus action to gain a fresh
/// Action this turn, used for one extra melee weapon swing. Mirrors the
/// `FlurryOfBlows` shape (BA → +Action token) so the barbarian's existing
/// Greataxe / weapon action consumes the granted Action. Gated on:
///   - holder has the `FRENZY_TAG` passive subclass feature
///   - holder currently has the `Raging` condition (Frenzy is rage-only)
///   - combat-active (no firing during a downed state)
pub struct Frenzy {}

impl Action for Frenzy {
    fn name(&self) -> &str {
        "frenzy"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fr", "frenzied"]
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
        raging_feature_ready(encounter, caster_id, FRENZY_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  frenzy: barbarian gains an extra Action for a follow-up strike.".to_string());
        grant_extra_action(caster_id)
    }
}

pub static FRENZY: LazyLock<Frenzy> = LazyLock::new(|| Frenzy {});

/// 5e Barbarian Path of the Totem Warrior — **Bear Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the holder has resistance to all
/// damage except psychic. Replaces the default Rage's "BPS resistance only"
/// envelope for the lucky few Bear Totem barbarians.
///
/// Stored as a `has_passive_feature` flag rather than a fresh condition so
/// it composes naturally with the existing `Raging` condition gate (only
/// fires *while* Raging). Read at the damage-pipeline chokepoint
/// `has_condition_resistance` next to the `TYPED_RESISTANCE_CONDITIONS`
/// table so the standard 5e "one halving per damage instance" rule still
/// holds — Bear Totem doesn't stack with a separate template resistance
/// (e.g. a dwarven barbarian still only gets the single /2 on poison).
pub const BEAR_TOTEM_TAG: &str = "barbarian.bear_totem";

/// 5e Barbarian Path of the Totem Warrior — **Wolf Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the holder's allies have advantage
/// on melee attack rolls against any creature footprint-adjacent to the
/// barbarian. The pack-hunter flavor — the wolf totem barbarian becomes a
/// melee anchor whose presence sharpens every teammate's swing on the same
/// target. Stored as a `has_passive_feature` flag so it composes with the
/// existing `Raging` gate and only fires while the rage is active.
///
/// Read at `EncounterInstance::compute_attack_mode` next to the Pack Tactics
/// branch (same ally-side advantage shape, just gated on a passive-feature +
/// raging cohort rather than a template trait). Mirrors Pack Tactics' "one
/// halving per damage instance" parallel — multiple wolf totem allies don't
/// double-stack advantage, since Advantage already collapses to the
/// `RollMode::Advantage` lattice point.
pub const WOLF_TOTEM_TAG: &str = "barbarian.wolf_totem";

/// 5e Barbarian Path of the Totem Warrior — **Eagle Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the eagle barbarian can Dash as a
/// bonus action (the kiter/skirmisher flavor — the eagle barbarian closes or
/// re-positions twice in one turn while the wolf totem anchors and the bear
/// totem tanks). Stored as a `has_passive_feature` flag so it composes
/// naturally with the `Raging` condition gate without consuming a per-rest
/// charge.
///
/// The Dash-as-bonus-action half is exposed via the `EAGLE_DIVE` action,
/// which mirrors `CunningDash`'s shape (`GiveResource::Movement(speed)`)
/// gated on raging + Eagle Totem rather than the rogue's Cunning Action.
pub const EAGLE_TOTEM_TAG: &str = "barbarian.eagle_totem";

/// Eagle Totem Spirit's Dash-as-bonus-action: spend a bonus action to gain
/// a fresh chunk of movement equal to the holder's speed. Mirrors the
/// rogue Cunning Dash shape, but gated on the Eagle Totem barbarian's
/// `EAGLE_TOTEM_TAG` passive feature AND the `Raging` condition — outside
/// of rage the eagle barbarian has no extra mobility.
pub struct EagleDive {}

impl Action for EagleDive {
    fn name(&self) -> &str {
        "eagle dive"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ed", "dive"]
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
        raging_feature_ready(encounter, caster_id, EAGLE_TOTEM_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  eagle dive: barbarian surges forward on totem wings.".to_string());
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static EAGLE_DIVE: LazyLock<EagleDive> = LazyLock::new(|| EagleDive {});

/// 5e Barbarian Path of the Totem Warrior — **Tiger Totem Spirit** (level 3,
/// 2024 PHB Path of the Wild Heart flavor). Passive subclass feature: while
/// raging, the holder's walking speed increases by 10 ft. Fourth sibling of
/// Bear (damage envelope), Wolf (ally-aura), Eagle (bonus-action Dash) —
/// the Tiger plays the pure-mobility skirmisher who doesn't need to burn
/// their bonus action on the Dash, since the +10 ft speed is always on
/// while the rage holds. Stored as a `has_passive_feature` flag so it
/// composes with the existing `Raging` condition gate without a per-rest
/// charge.
///
/// Read at the `condition_speed_bonus` chokepoint next to the Longstrider /
/// Expeditious Retreat speed buffs, but gated on `has_condition(Raging) &&
/// has_passive_feature(TIGER_TOTEM_TAG)` rather than a condition flag —
/// outside of rage the tiger barbarian has no extra speed.
pub const TIGER_TOTEM_TAG: &str = "barbarian.tiger_totem";

/// 5e Fighter Champion — **Survivor** (level 18) feature tag. Passive
/// at-start-of-turn regen: while combat-active and above 0 HP but at or
/// below half max HP, the holder regains `5 + CON modifier` HP at the
/// start of each of their turns. The capstone "I will not die" envelope —
/// composes with Second Wind (bonus-action big chunk) and Indomitable
/// (failed-save reroll) so a level-18 Champion stabilizes themselves
/// passively round-over-round without burning either per-rest charge.
///
/// Read at `ActorInstance::reset_for_new_round` next to the once-per-turn
/// flag resets so the regen lands before any condition-based start-of-turn
/// damage (Ongoing burn from Hellish Rebuke / Cloudkill) is rolled. Floor
/// at 1 HP recovered when CON is negative — a -2 CON Champion still ticks
/// up 3 HP (5 - 2). The gate routes through `is_combat_active` so a
/// downed Champion doesn't auto-resurrect — Survivor is a stabilization
/// tool, not a revival one.
pub const SURVIVOR_TAG: &str = "fighter.survivor";

/// 5e Barbarian **Relentless Rage** (level 11) feature tag. Passive
/// rest-charged save-intercept: when a killing blow would otherwise
/// drop the barbarian to 0 HP *while raging*, they make a Constitution
/// save vs DC 10 (climbs by 5 each subsequent successful use, resets to
/// 10 on short / long rest). On a pass, HP pins at 1 instead. Distinct
/// from Half-Orc Relentless Endurance (RELENTLESS_ENDURANCE_TAG) in
/// three ways:
///   1. **Gated on Raging** — non-raging hits fall through to the
///      normal Downed transition.
///   2. **Save-based, not auto-pass** — a failed save burns nothing
///      (the feature has no charge to spend; it just doesn't fire).
///   3. **DC climbs** — each successful save makes the next attempt
///      harder, so a barbarian can't lean on this indefinitely.
///
/// Tied to the `relentless_rage_dc` field on `ActorInstance` for the
/// per-rest DC progression; the save is rolled in the encounter-side
/// intercept `EncounterInstance::try_relentless_rage`, which the
/// `DealDamage::apply` path calls before the Downed transition lands.
pub const RELENTLESS_RAGE_TAG: &str = "barbarian.relentless_rage";

/// 5e **Colossus Slayer** — Hunter Ranger subclass feature (level 3).
/// Passive once-per-turn rider: on a weapon hit, if the target's
/// current HP is less than its maximum, the attack deals +1d8 of the
/// weapon's damage type. Stored as a `has_passive_feature` flag (no
/// per-rest charge — it's always-on but rate-limited to one trigger
/// per turn) and read at the attack-resolution chokepoint in
/// `engine::attack::resolve_attack_outcome` right after the
/// Improved Divine Smite block. The "once per turn" gate is the
/// `colossus_slayer_used` flag on the actor, cleared at turn-start
/// by `reset_for_new_round` (mirrors the rogue Sneak Attack guard).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider
/// fires on melee AND ranged weapon hits (RAW: "When you hit a
/// creature with a weapon attack") — no melee-only gate.
pub const COLOSSUS_SLAYER_TAG: &str = "ranger.colossus_slayer";

/// 5e Paladin **Improved Divine Smite** (level 11). Passive feature: every
/// melee weapon hit lays +1d8 radiant damage on the target — the paladin's
/// signature mid-tier damage spike, independent of the Divine Smite slot
/// burn. Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on). Read at the attack-resolution chokepoint in `engine::attack`
/// right after the `ON_HIT_RIDERS` loop so the rider stacks cleanly with
/// any active Smite prime (Divine Smite / Searing / Wrathful etc.) on the
/// same swing — the 1d8 fires whether or not a Smite is up.
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. Melee-only — RAW
/// Improved Divine Smite gates on "melee weapon attack" so a ranged shot
/// from a paladin without a thrown weapon doesn't pick up the rider.
pub const IMPROVED_DIVINE_SMITE_TAG: &str = "paladin.improved_divine_smite";

/// Class-feature tag for Paladin's Lay on Hands — once per long rest.
/// We collapse 5e's "pool of HP equal to 5 × level" healing well into a
/// single chunky use per rest so the once-per-rest gating pattern stays
/// uniform with the rest of the codebase (Second Wind, Action Surge,
/// Indomitable, Rage). The flat heal value is bigger than Cure Wounds
/// to compensate for the loss of pool flexibility.
pub const LAY_ON_HANDS_TAG: &str = "paladin.lay_on_hands";

/// Lay on Hands — paladin feature, touch range. Spend the once-per-rest
/// feature to heal an ally (or self) for `5 × level + CHA` HP. RAW's
/// pool mechanic lets the paladin split the heal across many casts; we
/// collapse to a single big chunk per rest so the feature follows the
/// same once-per-rest pattern as Second Wind. Plenty of healing for a
/// melee class that doesn't have spammable Cure Wounds slots.
pub struct LayOnHands {}

impl Action for LayOnHands {
    fn name(&self) -> &str {
        "lay on hands"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["loh", "hands"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — same tile as the target, footprint-adjacent.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, LAY_ON_HANDS_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        use crate::engine::types::AbilityScoreType;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let level = actor.level();
        let cha_mod = actor.ability_modifier(AbilityScoreType::Charisma);
        // 5 HP per paladin level + CHA modifier. At level 3 with CHA 16
        // (+3), that's 18 HP — beats Cure Wounds at 1d8+3 (avg 7) and
        // makes the once-per-rest gate worth the slot.
        let amount = (5 * level as i32 + cha_mod).max(1) as u32;
        if let Some(paladin) = encounter.actors.get_mut(&caster_id) {
            paladin.spend_feature(LAY_ON_HANDS_TAG);
        }
        encounter.log(format!(
            "  lay on hands: 5*{}{:+} = {} HP",
            level, cha_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static LAY_ON_HANDS: LazyLock<LayOnHands> = LazyLock::new(|| LayOnHands {});

/// Class-feature tag for the Paladin's Cleansing Touch (Oath capstone,
/// once per long rest in our model — RAW: CHA-mod uses per long rest;
/// we collapse to a single charge so the gating stays uniform with
/// Lay on Hands / Second Wind / Action Surge).
pub const CLEANSING_TOUCH_TAG: &str = "paladin.cleansing_touch";

/// Cleansing Touch — Paladin action, touch range. Spend the once-per-rest
/// feature to end one spell affecting a willing creature (or self). The
/// load-bearing late-game paladin tool: removes a heavyweight debuff
/// (Hold Person / Charm / Fear / Confusion) from an ally without burning
/// a level-5 Greater Restoration slot, or drops a concentrating
/// enemy's spell entirely without paying the level-3 Dispel Magic tax.
///
/// Three-tier dispel logic lives inside `CleansingTouchOn`:
///   1. Drop target's concentration.
///   2. Strip one canonical spell-installed debuff.
///   3. Fallback: strip one beneficial buff (Dispel Magic shape).
///
/// Custom-validates that the feature is available *and* there's something
/// on the target worth cleansing so a misclick doesn't burn the once-per-
/// rest charge on a clean target.
pub struct CleansingTouch {}

impl Action for CleansingTouch {
    fn name(&self) -> &str {
        "cleansing touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ct", "cleanse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        // Targeting an ally is the headline use case; the AI's support
        // pipeline reads this lane to pick the action for debuffed
        // teammates rather than queueing it as offense.
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !feature_ready(encounter, caster_id, CLEANSING_TOUCH_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        if !target.is_combat_active() {
            return false;
        }
        // Mirror Lesser Restoration's gate: there must be *something*
        // on the target for the cleanse to grab. Walking the three
        // fallback lanes (concentration, debuff, buff) up front spares
        // the once-per-rest charge from a no-op apply.
        target.is_concentrating()
            || crate::engine::side_effects::CLEANSING_TOUCH_DEBUFFS
                .iter()
                .any(|c| target.has_condition(*c))
            || target
                .conditions()
                .keys()
                .any(|c| c.is_dispellable_buff())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(paladin) = encounter.actors.get_mut(&caster_id) {
            paladin.spend_feature(CLEANSING_TOUCH_TAG);
        }
        encounter.log("  cleansing touch: paladin channels divine cleansing.".to_string());
        vec![Box::new(crate::engine::side_effects::CleansingTouchOn {
            target_id,
        })]
    }
}

pub static CLEANSING_TOUCH: LazyLock<CleansingTouch> = LazyLock::new(|| CleansingTouch {});

/// Tag for the Aasimar Healing Hands racial trait. Once per long rest,
/// the aasimar touches a creature (or themselves) and heals them for a
/// number of HP equal to their level (RAW: 1 minute action, no other
/// resource cost). We collapse the duration to an Action cost — single
/// chunky heal per rest, mirroring Lay on Hands but smaller and racial
/// rather than class-locked.
pub const HEALING_HANDS_TAG: &str = "aasimar.healing_hands";

/// Healing Hands — Aasimar racial, touch range. Spend the once-per-rest
/// feature to heal an ally (or self) for `level` HP. The smaller heal
/// (compared to Lay on Hands) reflects that this is a racial trait
/// available to any class chassis, not a paladin-only kit feature.
/// Action cost (not bonus action) so the aasimar can't combo it with a
/// big swing — RAW: "Action" per the SRD.
pub struct HealingHands {}

impl Action for HealingHands {
    fn name(&self) -> &str {
        "healing hands"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hh", "heal-hands"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(HEALING_HANDS_TAG) {
            return false;
        }
        // Target must be an ally (or self) and combat-active. Mirrors
        // Lay on Hands' gating — no wasted heal on a corpse.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let amount = actor.level().max(1);
        if let Some(aasimar) = encounter.actors.get_mut(&caster_id) {
            aasimar.spend_feature(HEALING_HANDS_TAG);
        }
        encounter.log(format!(
            "  healing hands: aasimar channels celestial light, healing {} HP.",
            amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static HEALING_HANDS: LazyLock<HealingHands> = LazyLock::new(|| HealingHands {});

/// Divine Smite — paladin feature, bonus action. Spends a level-1 spell
/// slot to prime the next successful melee weapon hit with +2d8 radiant
/// damage (consumed at the hit site in `resolve_attack`). RAW lets the
/// paladin spend higher-level slots for more radiant dice; we collapse
/// to the flat 2d8 lane to keep the resource model clean and avoid an
/// override-style level picker. The Smiting condition acts as the
/// primed flag — short timer (2 rounds) so a swing-less smite expires
/// rather than dangling indefinitely.
pub struct DivineSmite {}

impl Action for DivineSmite {
    fn name(&self) -> &str {
        "divine smite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ds", "smite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirectly: the rider damage lands on the next hit, not on
        // this action's resolution. Returning false keeps the AI's
        // focus-fire pipeline from picking it as a damage option.
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
        // Bonus action + level-1 spell slot. Burning the slot is the
        // load-bearing resource cost; the bonus action just prevents the
        // paladin from chaining smites with other bonus actions.
        bonus_action_and_slot(1)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't double-prime: re-casting Divine Smite while already
        // primed is a waste of a slot. The AI's pipeline doesn't deeply
        // model this; the gate is here for symmetry with other
        // self-buff actions (Mage Armor / Rage / Sacred Weapon).
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::Smiting))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Smiting,
            // 2-round window so a primed paladin who can't connect on
            // their own turn still has one more attack to land it on the
            // following round (e.g. a reaction-attack-of-opportunity).
            timer: ConditionTimer::Rounds(2),
        })]
    }
}

pub static DIVINE_SMITE: LazyLock<DivineSmite> = LazyLock::new(|| DivineSmite {});

/// Class-feature tag for Paladin's Channel Divinity: Sacred Weapon —
/// once per long rest. The Channel Divinity *resource* is shared between
/// multiple paladin sub-feature variants in RAW (Oath of Devotion's
/// Sacred Weapon + Turn the Unholy etc.); we only model Sacred Weapon so
/// the tag is sub-feature-specific.
pub const SACRED_WEAPON_TAG: &str = "paladin.sacred_weapon";

/// Channel Divinity: Sacred Weapon — paladin action. The paladin's
/// weapon glows with divine light: attack rolls gain a flat +CHA bonus
/// (read by `condition_attack_bonus`) for up to 10 rounds (1 minute
/// RAW). Once per long rest. We use a regular condition timer rather
/// than concentration so it stacks with the paladin's own spell
/// concentration (e.g. Compelled Duel + Sacred Weapon).
pub struct SacredWeapon {}

impl Action for SacredWeapon {
    fn name(&self) -> &str {
        "sacred weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sw-pal", "cd-sacred", "consecrate"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, SACRED_WEAPON_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            SACRED_WEAPON_TAG,
            Condition::Sacred,
            ConditionTimer::Rounds(10),
            "  sacred weapon: paladin's blade glows with divine light.",
        )
    }
}

pub static SACRED_WEAPON: LazyLock<SacredWeapon> = LazyLock::new(|| SacredWeapon {});

/// Class-feature tag for the Monk's Stunning Strike (once per long
/// rest, in our model — RAW is one per ki point, but we collapse the
/// ki pool into a single big-burst prime to keep the once-per-rest
/// gating pattern uniform). The actual stun save fires on the next
/// melee hit via the StunningStrike condition rider in
/// `EncounterInstance::resolve_attack`.
pub const STUNNING_STRIKE_TAG: &str = "monk.stunning_strike";

/// Monk Stunning Strike — bonus action. Primes the monk's next melee
/// hit: when the swing lands, the target makes a CON save vs the monk's
/// WIS-based DC (8 + prof + WIS). On fail, the target is Stunned until
/// the end of the monk's next turn. We model the prime as a caster-side
/// condition (StunningStrike) that the on-hit hook in `resolve_attack`
/// consumes — mirrors the Smiting pattern.
pub struct StunningStrike {}

impl Action for StunningStrike {
    fn name(&self) -> &str {
        "stunning strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss-monk", "stun"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| {
                a.is_combat_active()
                    && a.feature_available(STUNNING_STRIKE_TAG)
                    && !a.has_condition(Condition::StunningStrike)
            })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 2-round prime window so a primed monk who misses the
        // first swing still has the rest of this turn + next to
        // connect (same envelope as Divine Smite).
        prime_self_condition(
            encounter,
            caster_id,
            STUNNING_STRIKE_TAG,
            Condition::StunningStrike,
            ConditionTimer::Rounds(2),
            "  stunning strike: monk's next hit primes a stun save.",
        )
    }
}

pub static STUNNING_STRIKE: LazyLock<StunningStrike> = LazyLock::new(|| StunningStrike {});

/// Class-feature tag for the Monk's Patient Defense — bonus-action
/// Dodge. At-will (RAW: 1 ki point per use; we drop the ki pool to keep
/// the bonus-action mobility tools uniform with Cunning Action).
pub const PATIENT_DEFENSE_TAG: &str = "monk.patient_defense";

/// Patient Defense — Monk bonus action. Take the Dodge action as a
/// bonus action: attacks vs the monk have disadvantage and DEX saves
/// gain advantage until the start of the monk's next turn. Mirrors
/// `CunningDisengage` / `CunningHide` — same one-shot bonus-action
/// pattern, just a different resulting flag.
pub struct PatientDefense {}

impl Action for PatientDefense {
    fn name(&self) -> &str {
        "patient defense"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pd", "patient"]
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
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDodging {
            actor_id: caster_id,
            dodging: true,
        })]
    }
}

pub static PATIENT_DEFENSE: LazyLock<PatientDefense> = LazyLock::new(|| PatientDefense {});

/// Step of the Wind — 5e Monk level-2 bonus action. Spends ki to take the
/// Dash AND Disengage actions for free as a bonus action (we collapse
/// the ki cost into the bonus-action lane since the engine doesn't track
/// a ki pool — same simplification as Patient Defense / Flurry of Blows).
/// Mechanically a fusion of the rogue's `CunningDash` (extra movement
/// equal to speed) and `CunningDisengage` (movement this turn doesn't
/// provoke OAs) — fired in one bonus action instead of two separate
/// activations, matching the monk's signature "blow past the front line"
/// flavor. RAW also doubles jump distance for the turn; we don't model
/// vertical movement so that clause is a no-op.
pub struct StepOfTheWind {}

impl Action for StepOfTheWind {
    fn name(&self) -> &str {
        "step of the wind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sotw", "step", "wind"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  step of the wind: monk surges past the front line.".to_string());
        vec![
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::Movement(speed),
            }),
            Box::new(crate::engine::side_effects::SetDisengaging {
                actor_id: caster_id,
                disengaging: true,
            }),
        ]
    }
}

pub static STEP_OF_THE_WIND: LazyLock<StepOfTheWind> = LazyLock::new(|| StepOfTheWind {});

/// Stillness of Mind — Monk action (5e level 7). At-will: spend an Action
/// to end one Charmed or Frightened condition currently affecting the
/// monk. RAW gates on "you can use your action" so it's never resource-
/// gated — a long-rest cap or feature tag would be over-restrictive. We
/// model exactly as RAW: removes both conditions in a single action when
/// either is up, and silently no-ops when neither is up so a misguided
/// click doesn't burn the Action lane. Self-targeted; needs no spell slot
/// or other resource beyond the standard Action.
///
/// Sits adjacent to Patient Defense (the other in-combat survival action
/// on the monk's sheet). The "auto-clear both" simplification matches our
/// Charmed/Frightened coverage — both are handled at the same chokepoints
/// (compute_attack_mode for the disadvantage on attacks, charmed_by for
/// the can't-target-charmer gate), so stripping both at once stays
/// consistent with how the conditions are read.
pub struct StillnessOfMind {}

impl Action for StillnessOfMind {
    fn name(&self) -> &str {
        "stillness of mind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["som", "still"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Only fire when there's actually something to cleanse; otherwise
        // a stray click would burn the monk's Action for nothing.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && (a.has_condition(Condition::Charmed)
                    || a.has_condition(Condition::Frightened))
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        encounter.log("  stillness of mind: monk centers their mind.".to_string());
        // Drop both Charmed and Frightened in one swing — the
        // RemoveCondition side-effect is a no-op for missing conditions
        // so an actor with only one of the two gets the clean cleanse.
        vec![
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Charmed,
            }),
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Frightened,
            }),
        ]
    }
}

pub static STILLNESS_OF_MIND: LazyLock<StillnessOfMind> = LazyLock::new(|| StillnessOfMind {});

/// Class-feature tag for the Bard's Bardic Inspiration (RAW: a pool of
/// CHA-mod uses per long rest — we collapse to a single big use to keep
/// the once-per-rest pattern uniform).
pub const BARDIC_INSPIRATION_TAG: &str = "bard.bardic_inspiration";

/// Bardic Inspiration — Bard bonus action, single ally. Grants the
/// Inspired condition on a willing ally within 60ft (24 tiles), letting
/// them add a flat +3 (the d6-average) to their next attack roll, save,
/// or ability check. We tag both the attack-roll bonus (via
/// `condition_attack_bonus`) and the save bonus (`condition_save_bonus`)
/// so the inspiration die is useful regardless of which roll comes up
/// next. The condition has a 10-round timer (1 minute RAW); the next
/// attack / save consumes it implicitly when the on-hit / save site
/// strips the condition (see `clear_inspired_on_attack`).
pub struct BardicInspiration {}

impl Action for BardicInspiration {
    fn name(&self) -> &str {
        "bardic inspiration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bi", "inspire"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.feature_available(BARDIC_INSPIRATION_TAG) {
            return false;
        }
        // Target must be an ally (same team), combat-active, and not
        // already Inspired — re-inspiration would just refresh the
        // timer without giving the AI a meaningful new effect.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team()
            && target.is_combat_active()
            && !target.has_condition(Condition::Inspired)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(BARDIC_INSPIRATION_TAG);
        }
        encounter.log("  bardic inspiration: ally rallies, gaining a die.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Inspired,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static BARDIC_INSPIRATION: LazyLock<BardicInspiration> = LazyLock::new(|| BardicInspiration {});

/// Class-feature tag for Cleric Channel Divinity: Turn Undead.
pub const TURN_UNDEAD_TAG: &str = "cleric.turn_undead";

/// Turn Undead — Cleric Channel Divinity, action. Every creature of the
/// Undead type within 30ft (12 tiles) makes a WIS save vs the cleric's
/// WIS-based DC. On fail, they're Frightened for 10 rounds (1 minute
/// RAW). Uses the `CreatureType::Undead` tag for accurate type checking.
/// Once per long rest.
pub struct TurnUndead {}

impl Action for TurnUndead {
    fn name(&self) -> &str {
        "turn undead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["turn", "cd-turn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, TURN_UNDEAD_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(TURN_UNDEAD_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let caster_loc = caster.location();
        let caster_team = caster.team();
        let caster_size = get_tiles_from_size(caster.size());
        encounter.log(format!(
            "  turn undead: every undead within 30ft saves (DC {}).",
            dc
        ));

        // Snapshot candidates so we don't mutate during iteration.
        let candidates: Vec<usize> = encounter
            .sorted_actor_ids()
            .into_iter()
            .filter(|id| {
                let Some(a) = encounter.actors.get(id) else {
                    return false;
                };
                if *id == caster_id || a.team() == caster_team || !a.is_combat_active() {
                    return false;
                }
                // Only undead are affected by Turn Undead.
                if !a.creature_type().is_undead() {
                    return false;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    caster_loc,
                    caster_size,
                );
                dist <= 12
            })
            .collect();

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in candidates {
            let save = encounter.roll_save(id, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static TURN_UNDEAD: LazyLock<TurnUndead> = LazyLock::new(|| TurnUndead {});

/// Flurry of Blows — Monk bonus action. After the monk takes the Attack
/// action, they may spend a ki point (modeled as a bonus action — we don't
/// track ki) to make two unarmed strikes against a target. We collapse to
/// a single side-effect: the monk gets one extra Action (which they can
/// then use on a martial-arts strike, double-dipping their swing cap for
/// the turn). The "must have already attacked" gate from RAW is dropped
/// for simplicity — the bonus action is gated on the monk having a Martial
/// Arts attack available, which proxies the same intent.
///
/// At-will (RAW: 1 ki point per use; we drop the ki pool for symmetry
/// with Patient Defense, the other monk bonus action). The economic
/// payoff is real: spending a bonus action to gain a second main-action
/// swing puts the monk's per-turn damage well ahead of any other PC.
pub struct FlurryOfBlows {}

impl Action for FlurryOfBlows {
    fn name(&self) -> &str {
        "flurry of blows"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fob", "flurry"]
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
        // Gate on the monk having a Martial Arts action — keeps Flurry
        // out of the dispatcher for non-monk actors that somehow got the
        // template.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.find_action("martial arts").is_some())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  flurry of blows: monk gains an extra Action for a follow-up strike.".to_string());
        grant_extra_action(caster_id)
    }
}

pub static FLURRY_OF_BLOWS: LazyLock<FlurryOfBlows> = LazyLock::new(|| FlurryOfBlows {});

/// Class-feature tag for the Cleric's Divine Strike (5e level-8 RAW;
/// once per long rest in our model). RAW exposes Divine Strike as a
/// passive "+1d8 typed damage on weapon hits" at level 8, but we model
/// it as an explicit bonus-action prime (mirrors the Smiting pattern)
/// so the cleric has a flavorful spike-damage button paired with the
/// Channel Divinity: Turn Undead lane.
pub const DIVINE_STRIKE_TAG: &str = "cleric.divine_strike";

/// Divine Strike — Cleric feature, bonus action. Spends the once-per-rest
/// feature to prime the cleric's next melee hit with +1d8 radiant
/// damage (consumed at the hit site in `resolve_attack` via the
/// OnHitRider table — see the `DivineStriking` rider entry). Tick-down
/// timer caps the prime to 2 rounds so an idle cleric doesn't carry
/// the prime across rests.
pub struct DivineStrike {}

impl Action for DivineStrike {
    fn name(&self) -> &str {
        "divine strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dstrike", "cd-strike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the rider lands on the next hit, not on cast.
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(DIVINE_STRIKE_TAG)
                && !a.has_condition(Condition::DivineStriking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            DIVINE_STRIKE_TAG,
            Condition::DivineStriking,
            ConditionTimer::Rounds(2),
            "  divine strike: cleric's next melee hit will land with radiant fury.",
        )
    }
}

pub static DIVINE_STRIKE: LazyLock<DivineStrike> = LazyLock::new(|| DivineStrike {});

/// Class-feature tag for the Fighter's Trip Attack Battle Master
/// maneuver (once per long rest). RAW exposes maneuvers as a pool of
/// superiority dice; we collapse to a single charge per rest so the
/// once-per-rest gating pattern stays uniform with Second Wind / Action
/// Surge / Indomitable.
pub const TRIP_ATTACK_TAG: &str = "fighter.trip_attack";

/// Trip Attack — Fighter Battle Master maneuver. Bonus action; primes
/// the next melee weapon hit: on connect, the target makes a STR save
/// vs the fighter's maneuver DC (8 + prof + STR); on fail, they're
/// knocked Prone. RAW's superiority-die damage rider is skipped; the
/// prone-on-fail half is the load-bearing tactical effect. One-shot
/// — the OnHitRider table strips the prime the moment a melee swing
/// lands. Tick-down timer (2 rounds) caps the prime if the fighter
/// can't connect.
pub struct TripAttack {}

impl Action for TripAttack {
    fn name(&self) -> &str {
        "trip attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["trip", "ta"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(TRIP_ATTACK_TAG)
                && !a.has_condition(Condition::TripAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            TRIP_ATTACK_TAG,
            Condition::TripAttacking,
            ConditionTimer::Rounds(2),
            "  trip attack: fighter's next hit forces a STR save vs prone.",
        )
    }
}

pub static TRIP_ATTACK: LazyLock<TripAttack> = LazyLock::new(|| TripAttack {});

/// Build the "+1 Action token" side-effect vector for bonus-action
/// economy-trade actions: `Action Surge`, `Flurry of Blows`, `Frenzy`,
/// and `Quickened Spell` all spend a bonus action (or feature charge,
/// or SP pool) and hand the caster a fresh Action to spend on a
/// follow-up attack / spell this turn. The literal
///
/// ```ignore
/// vec![Box::new(GiveResource {
///     actor_id: caster_id,
///     resource: Resource::Action,
/// })]
/// ```
///
/// fires from four different action sites; folding it behind one
/// helper keeps the per-action `side_effects` block shorter and gives
/// us a single chokepoint if the action-token grant ever needs an
/// engine-side hook (e.g. a future "extra Action provokes opportunity
/// attacks" rule). Public so the `metamagic` module can share the same
/// helper from outside this file.
pub fn grant_extra_action(caster_id: usize) -> Vec<Box<dyn ApplicableSideEffect>> {
    vec![Box::new(GiveResource {
        actor_id: caster_id,
        resource: Resource::Action,
    })]
}

/// Spend a once-per-rest feature charge and install a self-applied prime
/// condition on the caster. The classic "bonus-action prime" shape:
/// `spend_feature(tag)` then `ApplyCondition` for `prime`. Centralizes
/// the pattern repeated by every Battle Master maneuver, every Smite
/// prime, Sacred Weapon, Divine Strike, Stunning Strike, etc. Logs the
/// flavor line so the caller doesn't have to repeat the format string.
///
/// Returns the side-effect vector the action site should hand back to
/// the engine — typically just the one `ApplyCondition`. Note: this is
/// for *self-only* primes; targeted effects (Cutting Words, Bardic
/// Inspiration) still inline their own logic since they apply to an
/// ally / enemy id, not the caster.
fn prime_self_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    prime: Condition,
    timer: ConditionTimer,
    log_line: &str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    encounter.log(log_line.to_string());
    vec![Box::new(ApplyCondition {
        actor_id: caster_id,
        condition: prime,
        timer,
    })]
}

/// Class-feature tag for the Fighter's Menacing Attack Battle Master
/// maneuver (once per long rest in our model). RAW: a pool of superiority
/// dice; we collapse to a single charge per rest so the gating stays
/// uniform with Trip Attack / Second Wind / Indomitable.
pub const MENACING_ATTACK_TAG: &str = "fighter.menacing_attack";

/// Menacing Attack — Fighter Battle Master maneuver. Bonus action; primes
/// the next melee weapon hit: on connect, the target makes a WIS save
/// vs the fighter's STR-based maneuver DC; on fail, they're Frightened
/// until the end of the fighter's next turn. RAW's +1d8 superiority-die
/// damage is skipped (same caveat as Trip Attack); the frighten-on-fail
/// IS the load-bearing tactical effect. Mirrors Trip Attack's shape.
pub struct MenacingAttack {}

impl Action for MenacingAttack {
    fn name(&self) -> &str {
        "menacing attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["menace", "ma"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(MENACING_ATTACK_TAG)
                && !a.has_condition(Condition::MenacingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            MENACING_ATTACK_TAG,
            Condition::MenacingAttacking,
            ConditionTimer::Rounds(2),
            "  menacing attack: fighter's next hit forces a WIS save vs frighten.",
        )
    }
}

pub static MENACING_ATTACK: LazyLock<MenacingAttack> = LazyLock::new(|| MenacingAttack {});

/// Class-feature tag for the Fighter's Disarming Attack Battle Master
/// maneuver (once per long rest in our model).
pub const DISARMING_ATTACK_TAG: &str = "fighter.disarming_attack";

/// Disarming Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// STR save vs the fighter's STR-based maneuver DC; on fail, they're
/// Disarmed — attack rolls have disadvantage until the start of their
/// next turn. RAW: target drops their weapon; we collapse the
/// pickup-takes-a-move clause into the `UntilStartOfNextTurn` timer
/// since the engine doesn't track held items.
pub struct DisarmingAttack {}

impl Action for DisarmingAttack {
    fn name(&self) -> &str {
        "disarming attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["disarm", "da"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(DISARMING_ATTACK_TAG)
                && !a.has_condition(Condition::DisarmingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            DISARMING_ATTACK_TAG,
            Condition::DisarmingAttacking,
            ConditionTimer::Rounds(2),
            "  disarming attack: fighter's next hit forces a STR save vs disarm.",
        )
    }
}

pub static DISARMING_ATTACK: LazyLock<DisarmingAttack> = LazyLock::new(|| DisarmingAttack {});

/// Class-feature tag for the Fighter's Pushing Attack Battle Master
/// maneuver (once per long rest in our model).
pub const PUSHING_ATTACK_TAG: &str = "fighter.pushing_attack";

/// Pushing Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// STR save vs the fighter's STR-based maneuver DC; on fail, they're
/// shoved 15 ft (4 tiles in our 2.5ft grid) away from the fighter via
/// the standard `PushActor` helper. RAW's +1d8 superiority-die damage
/// is skipped (same caveat as the other maneuvers); the displacement
/// IS the load-bearing tactical effect. First maneuver to use the
/// `Push` variant of `FollowUpEffect`.
pub struct PushingAttack {}

impl Action for PushingAttack {
    fn name(&self) -> &str {
        "pushing attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pa", "shovea"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(PUSHING_ATTACK_TAG)
                && !a.has_condition(Condition::PushingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            PUSHING_ATTACK_TAG,
            Condition::PushingAttacking,
            ConditionTimer::Rounds(2),
            "  pushing attack: fighter's next hit forces a STR save vs shove.",
        )
    }
}

pub static PUSHING_ATTACK: LazyLock<PushingAttack> = LazyLock::new(|| PushingAttack {});

/// Class-feature tag for the Fighter's Goading Attack Battle Master
/// maneuver (once per long rest in our model). Refreshes on a short
/// rest via the `BATTLE_MASTER_MANEUVERS` registry above.
pub const GOADING_ATTACK_TAG: &str = "fighter.goading_attack";

/// Goading Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// WIS save vs the fighter's STR-based maneuver DC; on fail, they're
/// Goaded — attack rolls against anyone other than the fighter are at
/// disadvantage until the start of their next turn. RAW's +1d8
/// superiority-die damage is skipped (same caveat as the other
/// maneuvers); the goad debuff IS the load-bearing tactical effect.
/// Mirrors Compelled Duel's tank-anchor envelope but is per-rest
/// rather than concentration-bound.
pub struct GoadingAttack {}

impl Action for GoadingAttack {
    fn name(&self) -> &str {
        "goading attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["goad", "ga"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(GOADING_ATTACK_TAG)
                && !a.has_condition(Condition::GoadingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            GOADING_ATTACK_TAG,
            Condition::GoadingAttacking,
            ConditionTimer::Rounds(2),
            "  goading attack: fighter's next hit forces a WIS save vs goad.",
        )
    }
}

pub static GOADING_ATTACK: LazyLock<GoadingAttack> = LazyLock::new(|| GoadingAttack {});

/// Class-feature tag for the Fighter's Distracting Strike Battle Master
/// maneuver (once per long rest in our model). Refreshes on a short rest
/// via the `BATTLE_MASTER_MANEUVERS` registry.
pub const DISTRACTING_ATTACK_TAG: &str = "fighter.distracting_attack";

/// Distracting Strike — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target takes
/// +1d6 bonus damage (the superiority die) and is tagged Distracted —
/// the next attack roll against them by an attacker *other* than the
/// fighter has advantage until the end of the fighter's next turn.
/// RAW's superiority-die damage IS the load-bearing damage rider
/// (no save needed — the target-side advantage rider lands
/// unconditionally on hit). Mirrors Goading Attack's prime + target-
/// link pairing, but flipped to a target-side advantage rather than
/// an attacker-side disadvantage. One-shot — the OnHitRider table
/// strips this flag the moment a melee swing lands.
pub struct DistractingAttack {}

impl Action for DistractingAttack {
    fn name(&self) -> &str {
        "distracting strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["distract", "dsa"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(DISTRACTING_ATTACK_TAG)
                && !a.has_condition(Condition::DistractingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            DISTRACTING_ATTACK_TAG,
            Condition::DistractingAttacking,
            ConditionTimer::UntilStartOfNextTurn,
            "  distracting strike: fighter's next melee hit will rattle the target's guard.",
        )
    }
}

pub static DISTRACTING_ATTACK: LazyLock<DistractingAttack> =
    LazyLock::new(|| DistractingAttack {});

/// Class-feature tag for the Wizard's Arcane Recovery — once per long
/// rest, refreshes on long rest. RAW: once per day during a short rest,
/// recover spell slots whose combined levels equal half the wizard's
/// level (rounded up), with no slot above 5th. We collapse that pool
/// into a fixed-shape recovery (one level-1 + one level-2 slot for any
/// level-3+ wizard, only one level-1 slot below that) so the gate stays
/// a single feature-flag check rather than a slot picker. Listed in the
/// `SHORT_REST_FEATURES` registry so a short rest can re-enable the
/// feature; the long rest already enables it via the default refresh.
pub const ARCANE_RECOVERY_TAG: &str = "wizard.arcane_recovery";

/// Arcane Recovery — Wizard feature, action. Spend the once-per-rest
/// feature to restore one level-1 spell slot (plus a level-2 slot if
/// the wizard is at least level 3 and has a level-2 slot to restore).
/// Centralizes the RAW "half-level pool, no slot above 5th" math into a
/// flat per-tier shape; lets the wizard keep firing low-tier control
/// spells (Magic Missile / Shield / Web) across encounters without a
/// full long rest.
pub struct ArcaneRecovery {}

impl Action for ArcaneRecovery {
    fn name(&self) -> &str {
        "arcane recovery"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ar", "recover"]
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
        // No resource cost — RAW the recovery itself is free during a
        // short rest. The once-per-rest gate lives on the feature flag.
        free_cost()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Gate on the feature flag, combat-active state, AND at least one
        // missing spell slot in the level-1 or level-2 tier — recovering
        // a slot you didn't spend is a no-op, and gating here keeps the
        // AI from burning the feature on empty.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            if !a.is_combat_active() || !a.feature_available(ARCANE_RECOVERY_TAG) {
                return false;
            }
            let l1 = a.spell_slot_manager.spell_slots(1);
            let l2 = a.spell_slot_manager.spell_slots(2);
            l1.spell_slots < l1.max_spell_slots || l2.spell_slots < l2.max_spell_slots
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let (level, want_l1, want_l2) = {
            let Some(a) = encounter.actors.get(&caster_id) else {
                return Vec::new();
            };
            let l1 = a.spell_slot_manager.spell_slots(1);
            let l2 = a.spell_slot_manager.spell_slots(2);
            (
                a.level(),
                l1.spell_slots < l1.max_spell_slots,
                l2.spell_slots < l2.max_spell_slots,
            )
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(ARCANE_RECOVERY_TAG);
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if want_l1 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(1),
            }));
        }
        // RAW: pool of slot-levels equal to ceil(level/2), no slot above
        // 5th. We hand out the level-2 slot only at level 3+ (where a
        // ceil(3/2)=2 pool can afford it) and only if a slot was spent.
        if want_l2 && level >= 3 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(2),
            }));
        }
        encounter.log(format!(
            "  arcane recovery: {} restores {} spell slot{}.",
            encounter.actor_name(caster_id),
            effects.len(),
            if effects.len() == 1 { "" } else { "s" },
        ));
        effects
    }
}

pub static ARCANE_RECOVERY: LazyLock<ArcaneRecovery> = LazyLock::new(|| ArcaneRecovery {});

/// Class-feature tag for Cleric Channel Divinity: Preserve Life — once
/// per short or long rest. Shares the "Channel Divinity" RAW lane with
/// Turn Undead at the cleric level, but they're distinct features in our
/// model (each tag is a separate once-per-rest charge) so we don't have
/// to plumb a shared resource pool. Listed in `SHORT_REST_FEATURES`.
pub const PRESERVE_LIFE_TAG: &str = "cleric.preserve_life";

/// Channel Divinity: Preserve Life — Cleric action. Distribute a pool of
/// 5 * cleric level HP across wounded allies within 30ft (12 tiles),
/// healing each up to half their maximum HP. We collapse the RAW per-
/// target allocation choice into a deterministic algorithm: sort wounded
/// allies (including self) by HP fraction ascending so the most-hurt
/// allies get healed first, and bring each up to 50% max HP (or as close
/// as the remaining pool allows). Once per rest (short or long).
pub struct PreserveLife {}

impl Action for PreserveLife {
    fn name(&self) -> &str {
        "preserve life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pl", "cd-life", "preserve"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, PRESERVE_LIFE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(PRESERVE_LIFE_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let level = caster.level();
        let mut pool = 5 * level;
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());

        // Sort by HP fraction ascending so the most-wounded ally heals
        // first. Sorted-by-id within the same fraction keeps the choice
        // deterministic across runs with the same RNG seed (HashMap
        // iteration would otherwise shuffle ties).
        let mut candidates: Vec<(u32, u32, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team || !a.is_combat_active() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    caster_loc,
                    caster_size,
                );
                if dist > 12 {
                    return None;
                }
                let cap = a.max_hitpoints();
                if a.hitpoints() >= cap / 2 + (cap % 2) {
                    // Already at >= 50% max HP — RAW: feature can't heal
                    // a creature whose HP is at half or more.
                    return None;
                }
                // HP fraction scaled to a sortable u32 — multiply by max
                // so a wholly arbitrary `cap` size doesn't dominate.
                let frac = (a.hitpoints().saturating_mul(1_000_000)) / cap.max(1);
                Some((frac, *id as u32, *id))
            })
            .collect();
        candidates.sort_unstable();

        encounter.log(format!(
            "  preserve life: cleric distributes {} HP among wounded allies.",
            pool
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for (_frac, _id_sort, target_id) in candidates {
            if pool == 0 {
                break;
            }
            let Some(t) = encounter.actors.get(&target_id) else {
                continue;
            };
            let cap = t.max_hitpoints();
            let half = cap / 2 + (cap % 2);
            let needed = half.saturating_sub(t.hitpoints());
            if needed == 0 {
                continue;
            }
            let amount = needed.min(pool);
            pool -= amount;
            effects.push(Box::new(Heal {
                actor_id: target_id,
                amount,
            }));
        }
        effects
    }
}

pub static PRESERVE_LIFE: LazyLock<PreserveLife> = LazyLock::new(|| PreserveLife {});

/// Class-feature tag for the Bard's Cutting Words — once per short rest
/// (RAW: spends one Bardic Inspiration use). We collapse the
/// inspiration-die pool to a single per-rest charge to keep the gating
/// uniform with the other once-per-rest features; the recovery happens
/// via `SHORT_REST_FEATURES`.
pub const CUTTING_WORDS_TAG: &str = "bard.cutting_words";

/// Cutting Words — Bard bonus action (RAW reaction; collapsed to bonus
/// action because the engine doesn't yet have a reactive cast-on-attack
/// hook). Targets one enemy within 60ft and applies the `Mocked`
/// condition: their next attack roll has disadvantage. We approximate
/// the RAW "subtract a Bardic Inspiration die from an attack roll,
/// ability check, or damage roll" with the disadvantage clause, which
/// captures the load-bearing tactical effect (the bard caging an enemy
/// strike before it lands).
pub struct CuttingWords {}

impl Action for CuttingWords {
    fn name(&self) -> &str {
        "cutting words"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cw-bard", "cut"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles. Matches Bardic Inspiration's range.
        Some(24)
    }
    fn is_harmful(&self) -> bool {
        true
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(CUTTING_WORDS_TAG) {
            return false;
        }
        // Target must be an enemy, combat-active, not already Mocked
        // (re-applying with a one-shot timer would just refresh — wasted
        // bonus action if the target hasn't swung yet).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() != actor.team()
            && target.is_combat_active()
            && !target.has_condition(Condition::Mocked)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(CUTTING_WORDS_TAG);
        }
        encounter.log("  cutting words: bard quips, fouling the target's strike.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Mocked,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUTTING_WORDS: LazyLock<CuttingWords> = LazyLock::new(|| CuttingWords {});

/// Class-feature tag for the Fighter's Precision Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge alongside
/// every other maneuver tag.
pub const PRECISION_ATTACK_TAG: &str = "fighter.precision_attack";

/// Precision Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next attack roll with a flat +4 (modeling the +1d8
/// superiority die, d8 avg rounded down). RAW lets the fighter spend
/// the die *after* seeing the d20 result; we apply the bonus
/// proactively so the AI can use it as a "the next swing better land"
/// accuracy buff. The +4 is read at every attack-roll site via
/// `condition_attack_bonus`; the prime is consumed by
/// `clear_attack_advantage_riders` the moment the swing resolves,
/// mirroring Bardic Inspiration's single-shot lane.
///
/// Unlike Trip / Menacing / Disarming / Pushing / Goading, this
/// maneuver has no melee-only or save gate — it lands on any attack
/// roll the fighter makes that turn (RAW: weapon attack roll, melee or
/// ranged). One-shot via the clear-on-attack consume site.
pub struct PrecisionAttack {}

impl Action for PrecisionAttack {
    fn name(&self) -> &str {
        "precision attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["precision", "pra"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(PRECISION_ATTACK_TAG)
                && !a.has_condition(Condition::PrecisionAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            PRECISION_ATTACK_TAG,
            Condition::PrecisionAttacking,
            // 2-round window so the prime survives until the fighter's
            // next swing even if their turn ends on a movement-only
            // sequence (mirrors Trip / Menacing / Smite primes).
            ConditionTimer::Rounds(2),
            "  precision attack: fighter's next attack roll gains +4.",
        )
    }
}

pub static PRECISION_ATTACK: LazyLock<PrecisionAttack> = LazyLock::new(|| PrecisionAttack {});

/// Class-feature tag for the Fighter's Sweeping Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const SWEEPING_ATTACK_TAG: &str = "fighter.sweeping_attack";

/// Sweeping Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit to splash 1d8 slashing damage onto
/// one footprint-adjacent enemy of the primary target (via the
/// `SweepingAttacking` condition + `FollowUpEffect::Splash` rider).
/// RAW: damage matches the original weapon's type; we collapse to a
/// flat 1d8 slashing since the on-hit rider table doesn't carry
/// per-weapon typing into the splash. The splash auto-applies on hit
/// (no save) — RAW: the original attack roll is reused.
///
/// One-shot — the rider table strips the `SweepingAttacking` flag the
/// moment a melee swing lands.
pub struct SweepingAttack {}

impl Action for SweepingAttack {
    fn name(&self) -> &str {
        "sweeping attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sweep", "swa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the splash damage lands on the consuming swing, not
        // on cast. Mirrors the other Battle Master primes / Smite
        // bonus-actions.
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(SWEEPING_ATTACK_TAG)
                && !a.has_condition(Condition::SweepingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            SWEEPING_ATTACK_TAG,
            Condition::SweepingAttacking,
            ConditionTimer::Rounds(2),
            "  sweeping attack: fighter's next melee hit splashes to an adjacent foe.",
        )
    }
}

pub static SWEEPING_ATTACK: LazyLock<SweepingAttack> = LazyLock::new(|| SweepingAttack {});

/// Class-feature tag for the Fighter's Feinting Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const FEINTING_ATTACK_TAG: &str = "fighter.feinting_attack";

/// Feinting Attack — Fighter Battle Master maneuver. Bonus action
/// targeting one enemy within melee reach. The fighter gains advantage
/// on the next attack roll against the chosen creature this turn.
///
/// Modeled by installing a self-help-grant via `set_help_grant`: the
/// helper is the fighter themselves, the designated target is the
/// feinted enemy. `attack_mode_with_riders` reads the grant on the
/// next swing against that target, folding in advantage and consuming
/// the grant. Mirrors how Help's lane works but the fighter targets
/// themselves rather than a separate ally.
///
/// Unlike the prime-style maneuvers (Trip / Menacing / Sweeping), this
/// doesn't go through the OnHitRider table — the entire effect is the
/// pre-roll advantage, which lands cleanly through the help-grant
/// machinery already wired into `compute_attack_mode`.
pub struct FeintingAttack {}

impl Action for FeintingAttack {
    fn name(&self) -> &str {
        "feinting attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["feint", "fa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — same envelope as Help / a melee weapon swing.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        // The action targets an enemy but doesn't itself deal damage or
        // apply a condition — it's a setup buff for the *next* swing.
        // Marking it harmful would route it through the AI's hostile
        // pipeline (good) but also block it via Sanctuary / Charmed-by
        // gating (we want Feint to behave like a Help / Hide setup —
        // unblocked by Sanctuary on the feinter's side). Leaving false
        // mirrors how Help's lane handles the targeting.
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(FEINTING_ATTACK_TAG) {
            return false;
        }
        // Target must be a live enemy (RAW: any creature, but feinting a
        // friendly is a wasted bonus action — the AI's hostile pipeline
        // is the consumer).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() != actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(FEINTING_ATTACK_TAG);
            // Self-help-grant: the fighter helps themselves get
            // advantage on the next attack vs the feinted target. Read
            // by `attack_mode_with_riders` which folds in
            // `consume_help_for` on the next swing.
            actor.set_help_grant(Some(crate::actors::actor_template::HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        encounter.log(
            "  feinting attack: fighter feints; advantage on the next attack vs the target."
                .to_string(),
        );
        Vec::new()
    }
}

pub static FEINTING_ATTACK: LazyLock<FeintingAttack> = LazyLock::new(|| FeintingAttack {});

/// Class-feature tag for the Fighter's Lunging Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const LUNGING_ATTACK_TAG: &str = "fighter.lunging_attack";

/// Lunging Attack — Fighter Battle Master maneuver. Bonus action prime;
/// the next melee weapon attack gains +5 ft of reach (one tile in this
/// engine's 2.5ft grid). Drives the `LungingAttacking` condition, which
/// `Action::validate_input` consults via `ActorInstance::extra_melee_reach`
/// to extend the action's reach check. The prime is consumed by
/// `CONSUMED_ON_ATTACK` on the next attack the fighter makes.
///
/// One-shot — mirrors the Trip / Menacing / Sweeping shape but with the
/// reach extension as the load-bearing effect instead of a save-or-debuff.
pub struct LungingAttack {}

impl Action for LungingAttack {
    fn name(&self) -> &str {
        "lunging attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lunge", "la"]
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(LUNGING_ATTACK_TAG)
                && !a.has_condition(Condition::LungingAttacking)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            LUNGING_ATTACK_TAG,
            Condition::LungingAttacking,
            // Short timer caps the prime so an idle fighter doesn't carry
            // the reach extension across rests. CONSUMED_ON_ATTACK clears
            // it on the next swing; the timer is just a safety net.
            ConditionTimer::Rounds(2),
            "  lunging attack: fighter's next melee swing gains +5 ft of reach.",
        )
    }
}

pub static LUNGING_ATTACK: LazyLock<LungingAttack> = LazyLock::new(|| LungingAttack {});

/// Class-feature tag for the Fighter's Rally Battle Master maneuver
/// (once per long rest in our model). Listed in `SHORT_REST_FEATURES`
/// so a short rest refreshes the charge.
pub const RALLY_TAG: &str = "fighter.rally";

/// Rally — Fighter Battle Master maneuver. Bonus action targeting one
/// ally within 30 ft (12 tiles). The chosen ally gains
/// `1d10 + CHA modifier` temp HP. RAW: superiority die scaling
/// (d8 → d12 by level); we use a flat 1d10 since the engine's per-class
/// scaling pool doesn't carry into this lane. Minimum +0 on the CHA
/// floor — a CHA-dump fighter still hands out d10 worth of buffer.
///
/// Non-priming maneuver — the effect is the immediate temp HP grant
/// rather than an OnHitRider prime. Distinct from Second Wind / Lay on
/// Hands because the target is an *ally* and the buffer is temp HP
/// rather than healing.
pub struct Rally {}

impl Action for Rally {
    fn name(&self) -> &str {
        "rally"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles. Matches Bardic Inspiration / Healing Word's
        // medium-range buff envelope.
        Some(12)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // Temp HP is the rally's buffer — slot it into the AI's support
        // pipeline alongside other healing taps so the fighter rallies
        // a low-HP ally rather than handing the buff to a full-HP one.
        true
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(RALLY_TAG) {
            return false;
        }
        // Ally-only — RAW: "you can choose a friendly creature who can
        // see or hear you". The AI's support pipeline picks live allies
        // already; we just enforce the team-match gate here so a
        // misqueued enemy-target invocation doesn't slip through.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(1, 10));
        let cha_mod = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Charisma))
            .unwrap_or(0);
        // RAW: temp HP can't drop below the roll itself, so floor the
        // CHA contribution at 0 (a -1 CHA fighter still hands out d10).
        let amount = (raw as i32 + cha_mod).max(raw as i32) as u32;
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RALLY_TAG);
        }
        encounter.log(format!(
            "  rally: 1d10({}){:+} = {} temp HP",
            raw, cha_mod, amount
        ));
        vec![Box::new(GainTempHp {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static RALLY: LazyLock<Rally> = LazyLock::new(|| Rally {});

/// Class-feature tag for the Fighter's Commander's Strike Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const COMMANDERS_STRIKE_TAG: &str = "fighter.commanders_strike";

/// Commander's Strike — Fighter Battle Master maneuver. Bonus action
/// targeting one ally within 60 ft (24 tiles). The fighter directs the
/// ally to make one weapon attack with advantage as a reaction. We model
/// this by granting the ally a self-help-grant against the fighter's
/// nearest visible enemy — `attack_mode_with_riders` folds in advantage
/// on their next attack roll vs that target. RAW also gives the ally a
/// fresh Reaction; we mirror via a `GiveResource(Reaction)` so the ally
/// can immediately spend it on an opportunity-style attack out of turn.
///
/// Conceptually closer to Feinting Attack (advantage on next swing
/// against a specific target) than the prime-style maneuvers — the
/// effect is the pre-roll advantage + the bonus reaction, both of which
/// land through existing engine lanes without a new OnHitRider entry.
pub struct CommandersStrike {}

impl Action for CommandersStrike {
    fn name(&self) -> &str {
        "commander's strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["command", "cs"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles. The fighter shouts orders across the
        // battlefield; LOS isn't required by RAW so we skip the LOS
        // check (`requires_los` defaults to false).
        Some(24)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // The ally's swing deals damage, but the command itself doesn't.
        // Mirrors how Feinting / Help register in the support pipeline.
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(COMMANDERS_STRIKE_TAG) {
            return false;
        }
        // Ally-only — the target must be on the fighter's team and live.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(ally) = encounter.actors.get(&target_id) else {
            return false;
        };
        if ally.team() != actor.team() || !ally.is_combat_active() || target_id == caster_id {
            return false;
        }
        // Also need a hostile enemy on the map for the ally to attack —
        // otherwise the order has no recipient. We find one inside the
        // side_effects body so the validation here just checks the ally
        // is in a position to act.
        true
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::HelpGrant;
        let Some(ally_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Pick the closest visible enemy of the ally as the strike target.
        // The fighter doesn't get to pick — RAW: "the creature uses its
        // reaction to make one weapon attack" against a target the
        // commander chooses, but the engine has no per-action target
        // picker for ally-driven reactions, so we auto-target the nearest
        // enemy to the ally.
        let ally_team = match encounter.actors.get(&ally_id) {
            Some(a) => a.team(),
            None => return Vec::new(),
        };
        let strike_target = encounter
            .actors
            .iter()
            .filter(|(_, a)| a.team() != ally_team && a.is_combat_active())
            .min_by_key(|(id, _)| {
                encounter
                    .footprint_distance(ally_id, **id)
                    .unwrap_or(isize::MAX)
            })
            .map(|(id, _)| *id);
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(COMMANDERS_STRIKE_TAG);
        }
        encounter.log(format!(
            "  commander's strike: fighter directs ally for a reaction attack{}.",
            if strike_target.is_some() {
                ""
            } else {
                " (no visible enemy; reaction granted but advantage idle)"
            }
        ));
        // Install the help-grant on the ally — advantage on next swing
        // against the chosen enemy (if any).
        if let Some(target_id) = strike_target
            && let Some(ally) = encounter.actors.get_mut(&ally_id)
        {
            ally.set_help_grant(Some(HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        // Give the ally a fresh Reaction so they can spend it on an
        // attack of opportunity / reaction-attack lane immediately.
        vec![Box::new(GiveResource {
            actor_id: ally_id,
            resource: Resource::Reaction,
        })]
    }
}

pub static COMMANDERS_STRIKE: LazyLock<CommandersStrike> = LazyLock::new(|| CommandersStrike {});

/// Tag for the Tiefling Infernal Legacy: Hellish Rebuke racial trait.
/// Once per long rest the tiefling fires a CHA-based Hellish Rebuke
/// without spending a spell slot — RAW: "Starting at 3rd level, you
/// can cast hellish rebuke as a 2nd-level spell once with this trait
/// and regain the ability to do so when you finish a long rest." We
/// gate on a feature flag and refresh on long rest like Lay on Hands /
/// Relentless Endurance.
pub const INFERNAL_LEGACY_REBUKE_TAG: &str = "tiefling.infernal_legacy_rebuke";

/// Tiefling Infernal Legacy — Hellish Rebuke (racial flavor). One swing
/// per long rest of a CHA-based DEX-save burst that deals 3d10 fire
/// (RAW: "cast hellish rebuke as a 2nd-level spell"). No slot cost —
/// the once-per-rest feature gate is the load-bearing resource.
///
/// Action cost rather than the RAW reaction cost — the engine doesn't
/// have a clean "reactive on being damaged" hook for player-driven
/// actions, and the action cost keeps the racial useful even when the
/// tiefling hasn't been hit yet.
pub struct InfernalLegacyRebuke {}

impl Action for InfernalLegacyRebuke {
    fn name(&self) -> &str {
        "infernal rebuke"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["infernal", "ireb", "tiefling-rebuke"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, INFERNAL_LEGACY_REBUKE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        if let Some(c) = encounter.actors.get_mut(&caster_id) {
            c.spend_feature(INFERNAL_LEGACY_REBUKE_TAG);
        }
        // 3d10 fire — matches Hellish Rebuke cast at level 2 (RAW).
        let (dmg, _) = crate::actions::spells::save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 10),
            DamageType::Fire,
            "infernal rebuke",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Fire,
        })]
    }
}

pub static INFERNAL_LEGACY_REBUKE: LazyLock<InfernalLegacyRebuke> =
    LazyLock::new(|| InfernalLegacyRebuke {});

/// Tag for the Dragonborn Breath Weapon racial trait. Once per short
/// rest (RAW: "Once you use your breath weapon, you can't use it again
/// until you complete a short or long rest"). Listed in the
/// `SHORT_REST_FEATURES` registry so a short rest refreshes the charge
/// alongside the other once-per-rest features.
pub const BREATH_WEAPON_TAG: &str = "dragonborn.breath_weapon";

/// Dragonborn Breath Weapon — racial action. 2-tile burst from the
/// dragonborn's footprint, DEX save vs the dragonborn's CON-based DC
/// (8 + prof + CON), 2d6 of the draconic ancestor's damage type, half
/// on save. Once per short rest. Scales by character level (3d6 at
/// L6, 4d6 at L11, 5d6 at L16).
///
/// The damage type is sourced from the actor's `draconic_ancestry()` —
/// `None` means the actor has no ancestor and the action falls back to
/// fire (defensive default; the action is gated to dragonborn templates
/// in `custom_validate_input` so the fallback should never fire in
/// gameplay).
pub struct BreathWeapon {}

impl Action for BreathWeapon {
    fn name(&self) -> &str {
        "breath weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["breath", "bw"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5e RAW: 15-ft cone (or 5x30-ft line). We collapse to a
        // 2-tile burst — same envelope as Burning Hands — so the
        // AI's `try_attack_aoe` lane picks it up alongside the
        // sorcerer / wizard cone spells.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15-ft cone RAW — we approximate as a 2-tile burst targeted
        // anywhere within ~6 tiles (the cone's reach).
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Surfaced for the UI / AI heuristics. The actual type is
        // resolved at cast time from `draconic_ancestry`; we report a
        // representative list rather than a single guess, since
        // dragonborn variants pick different ancestries.
        vec![
            DamageType::Fire,
            DamageType::Cold,
            DamageType::Lightning,
            DamageType::Acid,
            DamageType::Poison,
        ]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.feature_available(BREATH_WEAPON_TAG)
                && a.draconic_ancestry().is_some()
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // RAW: Save DC = 8 + proficiency + CON modifier. Matches the
        // standard spell-save-DC formula with CON as the casting
        // ability — `spell_save_dc` reuses the same arithmetic.
        let dc = caster.spell_save_dc(AbilityScoreType::Constitution);
        let damage_type = caster.draconic_ancestry().unwrap_or(DamageType::Fire);
        // RAW scaling: 2d6 at L1, 3d6 at L6, 4d6 at L11, 5d6 at L16.
        // We use 1 + level/5 (clamped at 1) which yields 2d6 / 3d6 / 4d6
        // / 5d6 at the listed breakpoints.
        let dice_count = (1 + caster.level() / 5).max(1);
        if let Some(c) = encounter.actors.get_mut(&caster_id) {
            c.spend_feature(BREATH_WEAPON_TAG);
        }
        let raw = encounter.roll(&Dice::new(dice_count, 6));
        encounter.log(format!(
            "  breath weapon: {}d6({}) = {} {} cone",
            dice_count, raw, raw, damage_type
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            damage_type,
        )
    }
}

pub static BREATH_WEAPON: LazyLock<BreathWeapon> = LazyLock::new(|| BreathWeapon {});

/// Warlock Eldritch Invocation — **Agonizing Blast**. Passive feature:
/// when the holder casts Eldritch Blast, they add their Charisma
/// modifier to the damage of each beam (RAW: "When you cast Eldritch
/// Blast, add your Charisma modifier to the damage it deals on a hit").
/// Read at the EldritchBlast cast site via `feature_available` (the
/// invocation never gets spent — it's permanent, but the same gate is
/// the cleanest hook). Add this tag to a warlock template's `features`
/// set to install it.
pub const AGONIZING_BLAST_TAG: &str = "warlock.agonizing_blast";

/// Warlock Eldritch Invocation — **Repelling Blast**. Passive feature:
/// when the holder hits a Large or smaller creature with Eldritch Blast,
/// they can push the creature up to 10 feet (4 tiles in our 2.5ft grid)
/// away in a straight line (RAW). We approximate the size gate as
/// "Large or smaller" by skipping the push on Huge / Gargantuan
/// targets — those creatures are too massive for the cantrip's force.
/// Read at the EldritchBlast cast site via `feature_available`. Add this
/// tag to a warlock template's `features` set to install it.
pub const REPELLING_BLAST_TAG: &str = "warlock.repelling_blast";

/// Warlock Eldritch Invocation — **Eldritch Mind**. Passive feature: the
/// holder rolls with advantage on Constitution saving throws to maintain
/// concentration. Read at the concentration save chokepoint
/// (`EncounterInstance::roll_concentration_save`) which the DealDamage
/// pipeline drives whenever a concentrating actor takes damage and lives.
/// Permanent passive — never consumed. Add this tag to a warlock
/// template's `features` set to install it.
pub const ELDRITCH_MIND_TAG: &str = "warlock.eldritch_mind";

/// 5e Wild Magic Sorcerer **Tides of Chaos** feature tag. Once per long
/// rest charge — the sorcerer leans into the chaos of their bloodline to
/// gain advantage on their next attack roll, ability check, or saving
/// throw. We honor the attack-roll lane (the highest-leverage in our
/// combat model) via the `TidesOfChaos` condition, which grants
/// `grants_self_attack_advantage` and lives in `CONSUMED_ON_ATTACK`.
/// Tag is checked at the Tides of Chaos action's validate, decremented
/// on use, refilled on long rest.
pub const TIDES_OF_CHAOS_TAG: &str = "sorcerer.tides_of_chaos";

/// 5e Sorcerer **Sorcerous Restoration** feature tag (lv20 capstone).
/// Passive: the sorcerer regains 4 expended sorcery points the first
/// time they finish a short rest after using a metamagic option. We
/// collapse the trigger to "always restore 4 SP on short rest" — our
/// short-rest cadence is rare enough that the once-per-rest cap and
/// the metamagic-trigger gate would barely fire. Read by
/// `ActorInstance::short_rest` via a templated branch (no consume
/// because the feature is passive — the SP-give is the entire effect).
pub const SORCEROUS_RESTORATION_TAG: &str = "sorcerer.sorcerous_restoration";

/// 5e Wild Magic Sorcerer **Bend Luck** feature tag (level 6). Passive
/// reaction: when a creature you can see makes an attack roll against
/// you, you may spend 2 sorcery points + your reaction to subtract a
/// rolled 1d4 from the attack's total. The bend can drop a hit to a
/// miss but never undoes a natural-20 crit (the d20 face is decided
/// before the subtraction, mirroring RAW).
///
/// RAW also covers ability checks and saving throws; we model only the
/// attack-roll lane (the highest-leverage in our combat model) — the
/// save / check lanes have no central save chokepoint that reads
/// passive reaction features yet. The trigger fires automatically when
/// the resource gates pass (mirrors Uncanny Dodge / Halfling Lucky's
/// "no player prompt" shape — the engine commits the SP + reaction the
/// moment the attack would land). See
/// `EncounterInstance::apply_bend_luck_penalty` for the wire site.
pub const BEND_LUCK_TAG: &str = "sorcerer.bend_luck";

/// 5e Wild Magic Sorcerer **Wild Magic Surge** feature tag (level 1).
/// Passive: whenever the sorcerer casts a sorcerer spell of 1st level
/// or higher, the DM can have them roll a d20; on a 1, a Wild Magic
/// Surge fires from the surge table.
///
/// We honor the RAW trigger: every level-1+ cast rolls. The check lives
/// in `EncounterInstance::trigger_wild_magic_surge`, which is called
/// from the cross-cutting `Action::execute` site (the same chokepoint
/// the other "consume on cast" metamagic primes use). The function
/// reads this tag from `features_max` (it's passive, not consumed) and
/// short-circuits when the spell-slot level sniffed off the action's
/// cost is 0 (cantrips / non-spell actions never surge).
///
/// Tag is checked via `ActorInstance::has_passive_feature`. Tag is
/// added to a creature template's `features` set to install it.
pub const WILD_MAGIC_SURGE_TAG: &str = "sorcerer.wild_magic_surge";

/// 5e Wild Magic Sorcerer **Tides of Chaos**. Bonus action; once per
/// long rest, install the `TidesOfChaos` prime → advantage on the next
/// attack roll. RAW also grants advantage on the next ability check or
/// saving throw, but the attack-roll lane is the load-bearing one in
/// our combat model; the prime is consumed at the first swing via the
/// `CONSUMED_ON_ATTACK` cohort. Self-target, no SP cost — the once-per-
/// long-rest gate is the entire resource cost (mirrors Second Wind /
/// Indomitable's rest-charge shape). Pairs naturally with a metamagic
/// prime: the Tides advantage stacks on the same swing the metamagic
/// prime modifies, so a Tides + Empowered + Fire Bolt combo gives both
/// the advantage on the attack roll and the damage reroll.
pub struct TidesOfChaos {}

impl Action for TidesOfChaos {
    fn name(&self) -> &str {
        "tides of chaos"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tides", "toc", "chaos"]
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(TIDES_OF_CHAOS_TAG) {
            return false;
        }
        // No-stack on the prime condition: re-priming would just refresh
        // the timer and waste the once-per-long-rest charge.
        !actor.has_condition(Condition::TidesOfChaos)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.spend_feature(TIDES_OF_CHAOS_TAG);
            format!("{} embraces the tides of chaos.", actor.name())
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::TidesOfChaos,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static TIDES_OF_CHAOS: LazyLock<TidesOfChaos> = LazyLock::new(|| TidesOfChaos {});

/// 5e Sorcerer **Font of Magic** — Create Spell Slot. Bonus action; spend
/// `sp_cost` sorcery points to recreate one expended spell slot of the
/// matching level. RAW cost table caps at 5th-level slot:
///
/// |   slot   | SP cost |
/// |  level   |         |
/// |    1     |    2    |
/// |    2     |    3    |
/// |    3     |    5    |
/// |    4     |    6    |
/// |    5     |    7    |
///
/// We expose one static per slot level (rather than a single action with
/// an override parameter) to match how the other Action statics are
/// shaped — each entry is a focused, named option the picker UI surfaces.
/// The action validates the caster has the SP, has at least one expended
/// slot at the requested level, and is combat-active. The side-effect
/// debits SP up front (mirroring metamagic's eager-debit pattern) and
/// restores the slot via the engine's `restore_spell_slot` lane.
pub struct CreateSpellSlot {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub slot_level: u32,
    pub sp_cost: u32,
}

impl Action for CreateSpellSlot {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() {
            return false;
        }
        if actor.sorcery_points() < self.sp_cost {
            return false;
        }
        // Only valid if at least one slot at this level is currently
        // spent — recreating an already-full level is a no-op that would
        // waste the SP.
        let ssi = actor.spell_slot_manager.spell_slots(self.slot_level);
        ssi.spell_slots < ssi.max_spell_slots
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Direct mutation pattern mirrors the metamagic primes:
        // eager-debit + log + restore-slot, all inside `side_effects`.
        // No dedicated `SorceryPoint` Resource lane (avoids a third
        // resource enum entry for a single feature).
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.spend_sorcery_points(self.sp_cost);
            actor.spell_slot_manager.restore_spell_slot(self.slot_level, 1);
            format!(
                "{} converts {} SP into a level-{} slot ({} SP left).",
                actor.name(),
                self.sp_cost,
                self.slot_level,
                actor.sorcery_points()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        Vec::new()
    }
}

pub static CREATE_SPELL_SLOT_1: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-1 slot",
    aliases: &["cs1", "fontslot1"],
    slot_level: 1,
    sp_cost: 2,
};

pub static CREATE_SPELL_SLOT_2: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-2 slot",
    aliases: &["cs2", "fontslot2"],
    slot_level: 2,
    sp_cost: 3,
};

pub static CREATE_SPELL_SLOT_3: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-3 slot",
    aliases: &["cs3", "fontslot3"],
    slot_level: 3,
    sp_cost: 5,
};

/// 5e Sorcerer **Font of Magic** — Convert Spell Slot. Bonus action; burn
/// one expended-able spell slot of `slot_level` to gain `slot_level`
/// sorcery points (RAW: "you can transform one of your spell slots into
/// sorcery points; the slot value is added to your sorcery points pool,
/// up to your maximum"). Hard-capped at slot_level <= 5 — RAW lets you
/// convert any slot but the highest practical use is refilling SP for
/// metamagic, and a level-6+ slot is more valuable held.
///
/// We expose one static per slot level (rather than a single action with
/// an override parameter) to match how the other Action statics are
/// shaped. The action validates an open slot is available and the SP
/// pool has room (we cap at `sorcery_points_max` per RAW — the surplus
/// is wasted). The side-effect debits the slot up front and grants SP
/// via the actor's pool directly (no dedicated `SorceryPoint` Resource).
pub struct ConvertSpellSlot {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub slot_level: u32,
}

impl Action for ConvertSpellSlot {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // The spell slot cost lives in the resource lane so the engine's
        // existing slot debit/log pipeline handles it cleanly (vs an
        // inline `consume_spell_slot` in `side_effects`).
        bonus_action_and_slot(self.slot_level)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() {
            return false;
        }
        // Only valid if SP isn't already at the long-rest cap — RAW says
        // SP gained "up to your maximum"; converting beyond the cap is
        // strictly a slot waste, so we just block it.
        actor.sorcery_points() < actor.sorcery_points_max()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            // RAW "up to your maximum": cap the gain at the long-rest pool
            // size so we don't grow `sorcery_points` past `sorcery_points_max`.
            let cap = actor.sorcery_points_max();
            let gained = self.slot_level.min(cap.saturating_sub(actor.sorcery_points()));
            actor.give_sorcery_points(gained);
            format!(
                "{} converts a level-{} slot into {} SP ({} SP total).",
                actor.name(),
                self.slot_level,
                gained,
                actor.sorcery_points()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        Vec::new()
    }
}

pub static CONVERT_SPELL_SLOT_1: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-1 slot",
    aliases: &["convertslot1", "fontsp1"],
    slot_level: 1,
};

pub static CONVERT_SPELL_SLOT_2: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-2 slot",
    aliases: &["convertslot2", "fontsp2"],
    slot_level: 2,
};

pub static CONVERT_SPELL_SLOT_3: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-3 slot",
    aliases: &["convertslot3", "fontsp3"],
    slot_level: 3,
};
