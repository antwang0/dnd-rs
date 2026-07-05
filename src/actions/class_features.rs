use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        Action, TargetingSchema, action_only, bonus_action_and_slot, bonus_action_only,
        first_target_id, first_target_location, free_cost, resolve_enemy_burst_save_damage,
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
        types::{AbilityScoreType, Coordinate, DamageType},
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
    NATURAL_RECOVERY_TAG,
    PRESERVE_LIFE_TAG,
    CUTTING_WORDS_TAG,
    BREATH_WEAPON_TAG,
    // 5e War Domain Cleric features — both refresh on a short rest.
    // WAR_PRIEST is the once-per-rest bonus-action extra swing; GUIDED
    // STRIKE is the Channel Divinity +10 accuracy prime.
    WAR_PRIEST_TAG,
    GUIDED_STRIKE_TAG,
    // 5e Light Domain Cleric Channel Divinity — Radiance of the Dawn:
    // once-per-short-rest 30ft radiant burst. Shares the RAW "Channel
    // Divinity" resource lane with Turn Undead / Preserve Life / Guided
    // Strike, but each tag is a distinct per-rest charge in our model.
    RADIANCE_OF_THE_DAWN_TAG,
    // 5e Light Domain Cleric level-1 feature — Warding Flare: passive
    // reaction that imposes disadvantage on an incoming attack roll.
    // RAW: uses per long rest equal to WIS mod (min 1), refreshed on a
    // long rest. We collapse to a once-per-short-rest charge so the
    // Light Cleric doesn't lose the flare between engagements — same
    // gating shape as the other Cleric Channel Divinity charges.
    WARDING_FLARE_TAG,
    // 5e Oathbreaker Paladin level-15 subclass feature — Fanatical
    // Focus. Auto-fire "reroll one failed save per short rest" gate.
    // RAW: refreshes on a short or long rest, matching the Cleric
    // Channel Divinity cadence — registered here so the short rest
    // refresh path picks it up alongside Guided Strike / Radiance of
    // the Dawn / Warding Flare.
    FANATICAL_FOCUS_TAG,
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

/// Tag for the Paladin **Undying Sentinel** feature (Oath of the
/// Ancients level 15 subclass feature). Passive no-action feature:
/// once per long rest, when damage would reduce the holder to 0 HP
/// (and they're not killed outright by Massive Damage), they drop to
/// 1 HP instead. Mechanically identical to Half-Orc Relentless
/// Endurance — both route through the shared
/// `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort in
/// `take_typed_damage` so a hypothetical multiclass (a half-orc
/// Ancients paladin) spends the tags in order rather than double-
/// dipping on the same damage instance.
///
/// Ships on the ANCIENTS_PALADIN_TEMPLATE feature set (Oath of the
/// Ancients level-15 capstone) above its strict RAW level gate for
/// the same reason Nature's Ward (lv15) does — class templates target
/// a balanced playable level, not lockstep PHB progression. Distinct
/// from Nature's Ward (self-immunity to Charmed / Frightened, always
/// on) — Undying Sentinel is a one-shot "cheat death" resource that
/// refreshes on long rest. RAW-flavored as "the paladin refuses to
/// fall while there's still a chance to strike back", it's the
/// signature Ancients tell in low-HP combat rounds.
///
/// Long-rest refresh: the tag lives on `features_max` for holders,
/// so `long_rest` restores the charge (features_remaining =
/// features_max). Not in `SHORT_REST_FEATURES` — RAW gates on the
/// long rest per PHB text.
pub const UNDYING_SENTINEL_TAG: &str = "paladin.undying_sentinel";

/// Ordered cohort of feature tags that intercept a lethal HP-to-zero
/// damage instance and convert it to a "drop to 1 HP instead" outcome.
/// Read in order by `ActorInstance::take_typed_damage` at the 0-HP
/// transition site: the *first* tag on the actor's
/// `features_remaining` is spent, the HP is clamped to 1, and the
/// damage outcome downgrades to `Reduced`. Subsequent tags in the
/// list stay unspent — RAW: each feature says "instead of 0 HP" so
/// only one fires per instance.
///
/// Ordering is significant: earlier entries are consumed first, so
/// a hypothetical Half-Orc / Ancients Paladin multiclass would burn
/// Relentless Endurance before Undying Sentinel on a single lethal
/// hit. Both refresh on long rest (both live on `features_max`);
/// neither refreshes on short rest (neither is in
/// `SHORT_REST_FEATURES`).
///
/// Death Ward is checked separately, *before* this cohort — RAW: the
/// spell is an active resource the caster chose to maintain, so
/// burning the racial / class feature before Death Ward would waste
/// the slot. Massive Damage (overflow ≥ max HP) also short-circuits
/// before this cohort — RAW: "if remaining damage after hitting 0 HP
/// equals or exceeds the creature's max HP, it dies instantly."
pub const LETHAL_DAMAGE_ABSORBER_FEATURES: &[&str] = &[
    RELENTLESS_ENDURANCE_TAG,
    UNDYING_SENTINEL_TAG,
];

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

/// Gating shape for once-per-rest bonus-action primes that install a
/// self-targeted "next swing rider" condition (Trip Attack, Menacing
/// Attack, Disarming Attack, Pushing Attack, Goading Attack, Distracting
/// Attack, Precision Attack, Sweeping Attack, Lunging Attack, Stunning
/// Strike, Divine Strike). Returns true when:
///   - the caster is combat-active,
///   - the once-per-rest `tag` charge is available,
///   - the caster does NOT already carry `prime` (re-priming would
///     just refresh the timer and waste the charge).
///
/// Centralizes the three-clause chain that every one of the ~10
/// bonus-action primes above previously open-coded, and gives a single
/// chokepoint if the prime-install gate ever needs a cross-cutting
/// check (e.g. "no prime while Silenced" for the future Silence-shuts-
/// down-verbal-effects half). Sibling to `feature_ready` (which lacks
/// the prime-condition no-stack clause) — reach for this one whenever
/// the action installs a caster-side condition and reach for
/// `feature_ready` when the action's effect is one-shot damage / heal /
/// resource grant without a lingering flag.
pub fn feature_prime_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
    prime: Condition,
) -> bool {
    encounter.actors.get(&caster_id).is_some_and(|a| {
        a.is_combat_active() && a.feature_available(tag) && !a.has_condition(prime)
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
        spend_feature_and_grant_extra_action(
            encounter,
            caster_id,
            ACTION_SURGE_TAG,
            "  action surge: extra Action gained.",
        )
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

/// 5e Barbarian Path of the Berserker — **Mindless Rage** (level 6) feature
/// tag. Passive subclass feature: while raging, the barbarian is immune to
/// installs of Charmed and Frightened, AND any existing Charmed or
/// Frightened is suppressed for the duration (RAW: "you can't be charmed
/// or frightened while raging"; installs mid-rage bounce, and any
/// pre-existing installs are held at bay). The immunity flips off the
/// moment the `Raging` condition drops.
///
/// Stored as a `has_passive_feature` flag so it composes naturally with
/// the existing `Raging` condition gate. Read by the flag-driven
/// immunity table in `actor_template.rs` next to Nature's Ward
/// (Charmed / Frightened) and Halfling Brave (Frightened) — the row's
/// closure predicate checks both the passive flag AND the Raging
/// condition, so a non-raging Berserker gets no benefit. RAW also
/// suspends any pre-existing Charmed / Frightened while raging and
/// resumes them when rage ends; our engine's `dynamic_immunity_to`
/// short-circuits the "has this condition installed" check for immune
/// creatures at every read site (compute_attack_mode, etc.), so a
/// mid-rage install that later drops-and-resumes matches this
/// simplified read-only suppression.
///
/// Pairs naturally with the Berserker's Frenzy — the Berserker
/// barbarian is meant to bull-rush the enemy caster's opener, and the
/// classic anti-melee-brute answer is a fear / charm lockout. Mindless
/// Rage gives the Berserker the "no, I don't care" no-op that frees
/// them to keep swinging while every teammate eats the same effect.
pub const MINDLESS_RAGE_TAG: &str = "barbarian.mindless_rage";

/// 5e Barbarian Path of the Zealot — **Divine Fury** (level 3) feature
/// tag. Passive subclass feature: while raging, the first creature the
/// zealot hits with a weapon attack on each of their turns takes extra
/// `1d6 + half barbarian level` (min +1) radiant damage. RAW gives the
/// zealot a choice of radiant or necrotic per RAW; we lock the type to
/// radiant to keep the tell "holy warrior" flavor unambiguous.
///
/// Stored as a `has_passive_feature` flag with no per-rest charge — the
/// rate limit is the per-turn `divine_fury_used` ledger on
/// `ActorInstance`, sibling to `sneak_attack_used` / `colossus_slayer_used`
/// / `foe_slayer_used`. Cleared at turn-start by `reset_for_new_round`
/// so the opening swing of every turn re-arms the rider.
///
/// Three gates on the swing site (`engine::attack::resolve_attack_outcome`):
///   1. Caster has the `DIVINE_FURY_TAG` passive feature flag.
///   2. Caster carries the `Raging` condition (RAW: "while you're
///      raging"). The rider vanishes the moment rage drops.
///   3. Caster hasn't already fired Divine Fury this turn.
///
/// Composes cleanly with the barbarian's other on-hit riders: Brutal
/// Critical (extra weapon die on a crit), Rage's flat +2 melee bump
/// through `MELEE_CASTER_BUMPS`, and any smite-lane rider a hypothetical
/// paladin / barbarian multiclass might carry — all fire on the same
/// swing without stepping on each other. Weapon-only (RAW: "with a
/// weapon attack") so a hypothetical spell attack won't consume the
/// prime.
pub const DIVINE_FURY_TAG: &str = "barbarian.divine_fury";

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

/// 5e Barbarian **Fast Movement** (level 5) feature tag. Passive: while
/// not wearing heavy armor, the barbarian's walking speed increases by
/// 10 ft. Our engine doesn't model armor tiers so we collapse the
/// "not-wearing-heavy-armor" gate to "always on for a barbarian holding
/// the tag" — the same simplification we use for Monk Unarmored Movement
/// (the monk's flat AC 15 folds in the Wisdom / Dex scaling instead of
/// gating on armor). Read at the `condition_speed_bonus` chokepoint next
/// to the Longstrider / Tiger Totem speed buffs.
///
/// Distinct from Tiger Totem Spirit (also +10 ft speed) in two ways:
///   1. **Always-on** — Fast Movement fires the moment the barbarian is
///      alive; Tiger Totem gates on the `Raging` condition being active.
///   2. **Non-subclass** — Fast Movement is a class feature every
///      barbarian picks up at level 5, so it stacks additively on Bear /
///      Wolf / Eagle / Berserker barbarians. Tiger + Fast Movement while
///      raging is +20 ft in total.
pub const FAST_MOVEMENT_TAG: &str = "barbarian.fast_movement";

/// 5e Monk **Purity of Body** (level 10) feature tag. Passive: the monk
/// gains immunity to disease and poison (RAW: "your mastery of the ki
/// flowing through you makes you immune to disease and poison"). Two
/// mechanically visible effects in our engine:
///   1. Immunity to the `Poisoned` condition — installs bounce at the
///      `dynamic_immunity_to(Poisoned)` chokepoint next to the Purified
///      / Petrified / Fey Ancestry checks.
///   2. Immunity to poison damage — `effective_damage` short-circuits
///      to 0 at the same "typed condition immunity" lane the Petrified
///      poison-immunity rider already uses; here the gate is a passive
///      feature flag rather than a condition. Disease has no mechanical
///      surface in the combat engine, so the disease half of RAW is a
///      no-op we don't need to wire up.
///
/// Sibling to Sahuagin Blood Frenzy (passive combat advantage) and
/// Bear Totem Spirit (raging damage envelope) on the passive-feature-
/// flag lane — no per-rest charge, no condition gate, just an
/// always-on effect while the monk is alive.
pub const PURITY_OF_BODY_TAG: &str = "monk.purity_of_body";

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

/// 5e Rogue **Sneak Attack** feature tag. Passive once-per-turn +Nd6
/// damage rider on any weapon attack that qualifies (advantage or
/// ally-adjacent target, no disadvantage; RAW: PHB p. 96). Used as
/// the ledger key for the shared `ONCE_PER_TURN_RIDER_TAGS` cohort on
/// `ActorInstance` — the once-per-turn gate reads / writes through
/// `actor.once_per_turn_used(SNEAK_ATTACK_TAG)` /
/// `mark_once_per_turn_used(SNEAK_ATTACK_TAG)`. Cleared at turn-start
/// by `reset_for_new_round` alongside the sibling rider tags.
///
/// The tag exists solely for the ledger — Sneak Attack is exposed as
/// an attack-time property of the rogue's weapon path, NOT as a
/// `has_passive_feature` check on the actor (the rogue's chassis
/// implicitly qualifies). The `class_attacks::sneak_attack_dice_for_level`
/// helper reads the caster's level to size the die pool.
pub const SNEAK_ATTACK_TAG: &str = "rogue.sneak_attack";

/// Ordered cohort of feature tags whose "once-per-turn used" ledger
/// lives on `ActorInstance::once_per_turn_marks`. Adding a future
/// once-per-turn attack rider (a new Battle Master maneuver's
/// per-turn window, a subclass equivalent to Colossus Slayer) lands
/// as a fresh const + one entry here rather than a fresh `bool`
/// field + accessor pair + reset call. The shared ledger is a
/// `HashSet<&'static str>` cleared once at `reset_for_new_round`.
///
/// Entries are listed for docs / self-check purposes; the ledger
/// itself doesn't consult this array at runtime — any tag can be
/// marked / read through the shared API without pre-registration.
/// Keeps the cohort discoverable in one place, mirroring the shape
/// of `SHORT_REST_FEATURES` / `LETHAL_DAMAGE_ABSORBER_FEATURES`.
pub const ONCE_PER_TURN_RIDER_TAGS: &[&str] = &[
    SNEAK_ATTACK_TAG,
    COLOSSUS_SLAYER_TAG,
    FOE_SLAYER_TAG,
    DIVINE_FURY_TAG,
];

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

/// 5e **Multiattack Defense** — Hunter Ranger subclass feature (level
/// 7 Defensive Tactics, "Multiattack Defense" option). Passive: when a
/// creature hits the holder with an attack, the holder gains a +4
/// bonus to AC against all subsequent attacks made by that same
/// creature for the rest of the turn.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited by "already hit me this turn" state on
/// the attacker). Read at both attack chokepoints
/// (`resolve_attack_outcome` for weapon swings and `spell_attack_outcome`
/// for spell attacks) right after the initial AC read: if the
/// attacker's `has_hit_target_this_turn(target_id)` returns true AND
/// the target holds this tag, +4 is added to the target's effective
/// AC. The hit-mark is written on the first connecting swing so the
/// second swing of a multi-attack sequence (Extra Attack, Action
/// Surge, Scorching Ray beam 2 / 3) is the earliest one to eat the
/// penalty — exactly RAW.
///
/// Composes cleanly with cover (+2 / +5 from intervening creatures)
/// and the target's own template AC — all three lanes sum into a
/// single effective AC read once per attack.
pub const MULTIATTACK_DEFENSE_TAG: &str = "ranger.multiattack_defense";

/// 5e Ranger **Foe Slayer** (level 20 capstone). Passive once-per-turn
/// weapon-hit rider: on any connecting weapon attack, the ranger adds
/// their Wisdom modifier as flat bonus damage of the weapon's damage
/// type. RAW: "Once on each of your turns, you can add your Wisdom
/// modifier to the attack roll or the damage roll of an attack you
/// make." — we collapse to the damage-roll lane (the load-bearing
/// pick; the attack-roll lane is already covered by Colossus Slayer's
/// die-based rider plus advantage sources).
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited by the once-per-turn `foe_slayer_used`
/// ledger on the actor, sibling to `sneak_attack_used` and
/// `colossus_slayer_used`). Read at the attack-resolution chokepoint
/// in `engine::attack::resolve_attack_outcome` right after the Colossus
/// Slayer block so both riders can fire on the same hit (RAW: Foe
/// Slayer is a separate feature, not a Hunter subclass rider — a
/// Hunter ranger with both Colossus Slayer AND Foe Slayer stacks the
/// two once-per-turn +damage lanes on the opening shot).
///
/// Ships on the CR-1 baseline RANGER_TEMPLATE and the CR-1 Hunter
/// Ranger subclass template above their strict RAW level gate for the
/// same reason Colossus Slayer / Multiattack Defense / Superior
/// Hunter's Defense do — class templates target a balanced playable
/// level, not lockstep PHB progression. Fires on melee AND ranged
/// weapon hits (RAW: "an attack you make" — no melee-only gate);
/// crits don't double the flat mod (RAW: the crit-doubling rule
/// applies to damage dice, not flat modifiers).
pub const FOE_SLAYER_TAG: &str = "ranger.foe_slayer";

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

/// Class-feature tag for the Vengeance Paladin's Channel Divinity: Vow
/// of Enmity (RAW: lv3 subclass feature, once per short rest in RAW; we
/// collapse to once per long rest so the gating stays uniform with the
/// rest of the Channel Divinity envelope — Sacred Weapon / Turn Undead).
/// The actual advantage rider fires in `compute_attack_mode` via the
/// `matched_link_mode` helper reading the `Sworn` condition + `sworn_by`
/// link on the target.
pub const VOW_OF_ENMITY_TAG: &str = "paladin.vow_of_enmity";

/// Vow of Enmity — Vengeance Paladin Channel Divinity, bonus action.
/// Mark one creature within 10 ft (4 tiles) as the paladin's quarry;
/// the paladin (and only the paladin) gets advantage on attack rolls
/// against the marked target for up to 10 rounds (1 minute RAW). Once
/// per long rest.
///
/// Engine wiring:
///   - Installs `Condition::Sworn` on the target with a 10-round timer.
///   - Sets the target's `sworn_by` link to the paladin's id via
///     `SetSwornBy` (mirrors Compelled Duel's Dueled + dueled_by /
///     Goading Attack's Goaded + goaded_by chain — same flag-plus-link
///     install shape, distinct field).
///   - `compute_attack_mode` reads the (Sworn, sworn_by == attacker)
///     pair via `matched_link_mode` and combines advantage when the
///     paladin attacks the sworn target.
///
/// Range gate (4 tiles = 10 ft RAW) matches the Compelled Duel envelope
/// but is shorter than Hunter's Mark (90 ft); the vow is meant for a
/// committed melee paladin marking the foe they're already closing on.
pub struct VowOfEnmity {}

impl Action for VowOfEnmity {
    fn name(&self) -> &str {
        "vow of enmity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["voe", "vow", "enmity"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10ft RAW = 4 tiles.
        Some(4)
    }
    fn is_harmful(&self) -> bool {
        // Vow of Enmity is *cast on* an enemy but has no harmful effect
        // by itself (no damage, no save). Marking the AI's targeting
        // pipeline as harmful keeps the helpful-action lane from
        // accidentally picking the vow against an ally; the
        // ally-vs-enemy gate in `custom_validate_input` enforces the
        // hostile-target requirement either way.
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
        if !feature_ready(encounter, caster_id, VOW_OF_ENMITY_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // Hostile-only target — vowing enmity against a teammate is
        // nonsense and the AI shouldn't pick it. Also skip if the
        // target is already sworn-by-us: re-applying the vow just
        // refreshes the timer without granting a new mechanical
        // benefit, and burns a once-per-rest charge for nothing.
        target.team() != caster.team()
            && target.is_combat_active()
            && !(target.has_condition(Condition::Sworn)
                && target.sworn_by() == Some(caster_id))
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
            paladin.spend_feature(VOW_OF_ENMITY_TAG);
        }
        encounter.log("  vow of enmity: paladin swears wrath against the foe.".to_string());
        let mut out: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Sworn,
                // 10 rounds = 1 minute RAW. Same envelope as Sacred
                // Weapon — the vow's accuracy buff and the weapon
                // glow both ride the same per-fight window.
                timer: ConditionTimer::Rounds(10),
            }),
        ];
        // Pull the SetSwornBy install from the central
        // `condition_link_side_effect` dispatch — same source of truth
        // the weapon on-hit rider chain and Compelled Duel use, so a
        // single match arm there serves every flag-plus-link install.
        if let Some(link) = crate::engine::side_effects::condition_link_side_effect(
            Condition::Sworn,
            target_id,
            caster_id,
        ) {
            out.push(link);
        }
        out
    }
}

pub static VOW_OF_ENMITY: LazyLock<VowOfEnmity> = LazyLock::new(|| VowOfEnmity {});

/// Class-feature tag for the Open Hand Monk's **Wholeness of Body** (lv6
/// subclass feature, once per long rest in our model — RAW: once per
/// long rest at lv6 already). Action; self-heal for `3 × level` HP.
/// Distinct from Second Wind (bonus action, fighter-only, 1d10 + level)
/// and Lay on Hands (action, paladin-only, `5 × level + CHA`) on the
/// once-per-rest self-heal lane — same gating envelope, distinct numbers
/// and class lock.
pub const WHOLENESS_OF_BODY_TAG: &str = "monk.wholeness_of_body";

/// Wholeness of Body — Open Hand Monk action. Spend the once-per-rest
/// feature to heal self for `3 × level` HP (no scaling ability mod —
/// pure level scaling, mirroring the RAW). At level 6 (the strict RAW
/// gate) this is 18 HP; on higher-level Open Hand templates the heal
/// climbs linearly. Action cost (not bonus action) so the monk can't
/// stack it with a Flurry of Blows — the heal is a tempo trade, not
/// a free burst.
pub struct WholenessOfBody {}

impl Action for WholenessOfBody {
    fn name(&self) -> &str {
        "wholeness of body"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wob", "wholeness"]
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
        feature_ready(encounter, caster_id, WHOLENESS_OF_BODY_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let level = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.level())
            .unwrap_or(1);
        // 3 HP per monk level — at level 6 that's 18, comparable to a
        // mid-tier Lay on Hands but action-cost not bonus.
        let amount = (3 * level).max(1);
        if let Some(monk) = encounter.actors.get_mut(&caster_id) {
            monk.spend_feature(WHOLENESS_OF_BODY_TAG);
        }
        encounter.log(format!(
            "  wholeness of body: monk channels ki for {} HP.",
            amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static WHOLENESS_OF_BODY: LazyLock<WholenessOfBody> = LazyLock::new(|| WholenessOfBody {});

/// Class-feature tag for the Monk's **Empty Body** (RAW: level 18 monk
/// capstone-adjacent, once per long rest). Action; the monk spends 4 ki
/// points to project their body as a semi-corporeal echo: **Invisible**
/// for 10 rounds (1 minute RAW), plus **DamageResistant** for the
/// duration (RAW: resistance to all damage except force — we collapse
/// the "except force" carve-out into the general resistance since the
/// engine's DamageResistant lane halves every type; the delta from RAW
/// only shows on the rare Magic Missile / Disintegrate hit against a
/// monk holding this buff up). Distinct from `WholenessOfBody`
/// (self-heal): Empty Body is a defensive-invisibility burst, no HP
/// restore.
///
/// Composes cleanly with Patient Defense (bonus-action Dodge) and Step
/// of the Wind (bonus-action Dash+Disengage) on the same turn — the monk
/// enters Empty Body via the Action lane, then dashes clear via a bonus
/// action, leaving them a 30ft-away invisible + damage-halved threat
/// that no attacker can plausibly close for the first round.
pub const EMPTY_BODY_TAG: &str = "monk.empty_body";

/// Empty Body — Monk action, once per long rest. Spends the feature
/// charge to install both `Invisible` and `DamageResistant` on the monk
/// for 10 rounds (1 minute RAW). Action cost (not bonus action) so the
/// monk can't stack it with a Flurry — burning the Action lane is the
/// tempo trade for the defensive envelope. The DamageResistant install
/// covers every damage type in this engine (RAW carves out force damage
/// — a minor delta since the only in-engine force damage sources are
/// Magic Missile / Disintegrate / Bigby's Hand, all of which are rare
/// against a lv18 monk anyway).
///
/// Both installs use `Rounds(10)` timers so a stray Dispel Magic or a
/// long fight burn-off both drop naturally — no bespoke concentration
/// wire-up needed.
pub struct EmptyBody {}

impl Action for EmptyBody {
    fn name(&self) -> &str {
        "empty body"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "empty"]
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
        if !feature_ready(encounter, caster_id, EMPTY_BODY_TAG) {
            return false;
        }
        // No-op if the monk is already invisible AND damage-resistant —
        // re-priming would just refresh timers without granting a new
        // mechanical benefit, and burns the once-per-rest charge for
        // nothing.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            !(a.has_condition(Condition::Invisible)
                && a.has_condition(Condition::DamageResistant))
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(monk) = encounter.actors.get_mut(&caster_id) {
            monk.spend_feature(EMPTY_BODY_TAG);
        }
        encounter.log(
            "  empty body: monk projects a semi-corporeal echo, becoming invisible and resistant to damage.".to_string(),
        );
        // Two installs, both `Rounds(10)` — 1 minute RAW. Same timer
        // envelope as Sacred Weapon / Rage / Vow of Enmity.
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::DamageResistant,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static EMPTY_BODY: LazyLock<EmptyBody> = LazyLock::new(|| EmptyBody {});

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
        feature_prime_ready(encounter, caster_id, STUNNING_STRIKE_TAG, Condition::StunningStrike)
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
        feature_prime_ready(encounter, caster_id, DIVINE_STRIKE_TAG, Condition::DivineStriking)
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

/// Shared "bonus-action prime → next melee hit rides a save-vs-condition
/// (or accuracy / splash / reach) rider" shape for Battle Master
/// maneuvers and any other class feature whose only surface is a
/// self-installed priming condition. Every field ships as a `&'static`
/// so the type can be a `pub const` (matching how `CreateSpellSlot` /
/// `ConvertSpellSlot` are declared elsewhere in this file).
///
/// Collapses the ~60-line `impl Action for XxxAttack {}` block that
/// nine Battle Master primes previously open-coded — every one wrote
/// the same trait body (NoArgs / bonus-action-only / feature-prime-
/// gated / self-condition-install) with only the tag / condition /
/// timer / log line changing. Sibling to the `SpellSlotRecovery` shape
/// (which collapses Arcane Recovery + Natural Recovery through the
/// same channel) — one row per maneuver instead of one impl block per
/// maneuver.
///
/// The shared `feature_prime_ready` + `prime_self_condition` helpers
/// already carried the gate + install logic — this struct just gives
/// them a shape that the `Action` trait can hang off. Adding a future
/// Battle Master pickup (Riposte, Parry, Bait and Switch, etc.) that
/// installs a caster-side prime lands as one `pub const` row here
/// instead of a fresh 60-line trait impl.
pub struct ManeuverPrime {
    /// Display name — surfaces in the action list + log line prefix and
    /// as the prompt parser's canonical entry (`Action::name`).
    pub name: &'static str,
    /// Alias set for the prompt parser (`Action::aliases`). Kept as a
    /// `&'static [&'static str]` so the struct stays plain data at
    /// LazyLock init.
    pub aliases: &'static [&'static str],
    /// Feature tag whose once-per-rest charge gates the prime. Read via
    /// `feature_available` in `custom_validate_input`; spent via
    /// `spend_feature` when the prime installs.
    pub tag: &'static str,
    /// The self-condition this prime installs on the caster. Read on
    /// both the validate side (`feature_prime_ready`'s no-stack clause
    /// bounces a duplicate cast while the prime is still up) and the
    /// install side (`prime_self_condition` runs an `ApplyCondition`).
    pub prime_condition: Condition,
    /// Duration of the installed prime. RAW's per-maneuver "until end of
    /// your next turn" varies (Rounds(2) covers the typical prime; the
    /// Distracting Strike case uses `UntilStartOfNextTurn` to match
    /// RAW's until-end-of-target's-next-turn window on the ally-side
    /// advantage rider).
    pub timer: ConditionTimer,
    /// Log flavor line emitted by `prime_self_condition` when the prime
    /// installs — surfaces in the combat log so the player can see
    /// exactly which prime is up.
    pub log_line: &'static str,
}

impl Action for ManeuverPrime {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        // The prime targets the caster, not an enemy — mirrors how
        // every other self-installed prime (Sacred Weapon, Divine Smite,
        // Stunning Strike) declares itself non-harmful. The eventual
        // damage / debuff lands on the *consuming* swing, which routes
        // through the weapon's `is_harmful` gate.
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
        feature_prime_ready(encounter, caster_id, self.tag, self.prime_condition)
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
            self.tag,
            self.prime_condition,
            self.timer,
            self.log_line,
        )
    }
}

/// Trip Attack — Fighter Battle Master maneuver. Bonus action; primes
/// the next melee weapon hit: on connect, the target makes a STR save
/// vs the fighter's maneuver DC (8 + prof + STR); on fail, they're
/// knocked Prone. RAW's superiority-die damage rider is skipped; the
/// prone-on-fail half is the load-bearing tactical effect. One-shot
/// — the OnHitRider table strips the prime the moment a melee swing
/// lands. Tick-down timer (2 rounds) caps the prime if the fighter
/// can't connect. Backed by the shared `ManeuverPrime` shape.
pub static TRIP_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "trip attack",
    aliases: &["trip", "ta"],
    tag: TRIP_ATTACK_TAG,
    prime_condition: Condition::TripAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  trip attack: fighter's next hit forces a STR save vs prone.",
});

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

/// Combined "spend a once-per-rest feature charge, log a flavor line,
/// and grant an extra Action token" side-effect builder. Action Surge
/// (Fighter) and War Priest (War Domain Cleric) share the same shape:
/// burn the per-rest charge, log the flavor line, and hand back the
/// single-entry `GiveResource(Action)` vector.
///
/// Sibling to `grant_extra_action` (the raw grant) — this wrapper adds
/// the spend + log layer for per-rest gated variants. Distinct from
/// Flurry of Blows / Frenzy (at-will / Rage-gated grants that don't
/// spend a feature charge) — those still call `grant_extra_action`
/// directly. Adding a future per-rest extra-Action feature drops in as
/// a one-liner instead of the three-line `if let Some(actor) →
/// spend_feature → encounter.log → grant_extra_action` boilerplate.
fn spend_feature_and_grant_extra_action(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    tag: &'static str,
    log_line: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(tag);
    }
    encounter.log(log_line.to_string());
    grant_extra_action(caster_id)
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
/// IS the load-bearing tactical effect. Backed by the shared
/// `ManeuverPrime` shape.
pub static MENACING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "menacing attack",
    aliases: &["menace", "ma"],
    tag: MENACING_ATTACK_TAG,
    prime_condition: Condition::MenacingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  menacing attack: fighter's next hit forces a WIS save vs frighten.",
});

/// Class-feature tag for the Fighter's Disarming Attack Battle Master
/// maneuver (once per long rest in our model).
pub const DISARMING_ATTACK_TAG: &str = "fighter.disarming_attack";

/// Disarming Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// STR save vs the fighter's STR-based maneuver DC; on fail, they're
/// Disarmed — attack rolls have disadvantage until the start of their
/// next turn. RAW: target drops their weapon; we collapse the
/// pickup-takes-a-move clause into the `UntilStartOfNextTurn` timer
/// since the engine doesn't track held items. Backed by the shared
/// `ManeuverPrime` shape.
pub static DISARMING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "disarming attack",
    aliases: &["disarm", "da"],
    tag: DISARMING_ATTACK_TAG,
    prime_condition: Condition::DisarmingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  disarming attack: fighter's next hit forces a STR save vs disarm.",
});

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
/// `Push` variant of `FollowUpEffect`. Backed by the shared
/// `ManeuverPrime` shape.
pub static PUSHING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "pushing attack",
    aliases: &["pa", "shovea"],
    tag: PUSHING_ATTACK_TAG,
    prime_condition: Condition::PushingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  pushing attack: fighter's next hit forces a STR save vs shove.",
});

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
/// rather than concentration-bound. Backed by the shared
/// `ManeuverPrime` shape.
pub static GOADING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "goading attack",
    aliases: &["goad", "ga"],
    tag: GOADING_ATTACK_TAG,
    prime_condition: Condition::GoadingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  goading attack: fighter's next hit forces a WIS save vs goad.",
});

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
/// strips this flag the moment a melee swing lands. Backed by the
/// shared `ManeuverPrime` shape; uses `UntilStartOfNextTurn` for the
/// timer to match RAW's until-end-of-target's-next-turn envelope on
/// the ally-side advantage rider.
pub static DISTRACTING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "distracting strike",
    aliases: &["distract", "dsa"],
    tag: DISTRACTING_ATTACK_TAG,
    prime_condition: Condition::DistractingAttacking,
    timer: ConditionTimer::UntilStartOfNextTurn,
    log_line: "  distracting strike: fighter's next melee hit will rattle the target's guard.",
});

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

/// Shared once-per-rest spell-slot recovery shape. Both the Wizard's
/// **Arcane Recovery** and the Druid Circle of the Land's **Natural
/// Recovery** collapse RAW's "recover slots totaling ceil(level/2), no
/// slot above 5th" pool into the same flat per-tier envelope: one
/// level-1 slot at any caster level, plus one level-2 slot at level 3+.
/// The two features are mechanically identical — only the tag, name,
/// aliases, and log flavor differ — so both fold through this shared
/// struct rather than duplicating the ~100-line validate + effects
/// bodies. Adding a future "recover a level-N slot on short rest"
/// feature (Sorcerer's Font of Magic recovery variants, etc.) lands as
/// a fresh `SpellSlotRecovery` const with a new tag and no touching
/// the Action impl body.
pub struct SpellSlotRecovery {
    /// Display name — surfaces in the action list, log lines, and the
    /// prompt parser (`Action::name`).
    pub name: &'static str,
    /// Alias set for the prompt parser (`Action::aliases`). Kept as a
    /// `&'static [&'static str]` so the struct stays a plain-data
    /// literal at LazyLock init.
    pub aliases: &'static [&'static str],
    /// Feature tag whose once-per-rest charge gates this recovery.
    /// Reads via `feature_available` in `custom_validate_input`;
    /// spent via `spend_feature` when the recovery fires.
    pub tag: &'static str,
}

impl Action for SpellSlotRecovery {
    fn name(&self) -> &str {
        self.name
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
        // Gate on the feature flag, combat-active state, AND at least
        // one missing spell slot in the level-1 or level-2 tier —
        // recovering a slot you didn't spend is a no-op, and gating
        // here keeps the AI from burning the feature on empty.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            if !a.is_combat_active() || !a.feature_available(self.tag) {
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
            actor.spend_feature(self.tag);
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if want_l1 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(1),
            }));
        }
        // RAW: pool of slot-levels equal to ceil(level/2), no slot
        // above 5th. We hand out the level-2 slot only at level 3+
        // (where a ceil(3/2)=2 pool can afford it) and only if a slot
        // was spent.
        if want_l2 && level >= 3 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(2),
            }));
        }
        encounter.log(format!(
            "  {}: {} restores {} spell slot{}.",
            self.name,
            encounter.actor_name(caster_id),
            effects.len(),
            if effects.len() == 1 { "" } else { "s" },
        ));
        effects
    }
}

/// Arcane Recovery — Wizard feature. Spend the once-per-rest charge
/// to restore one level-1 spell slot (plus a level-2 slot if the
/// wizard is at least level 3 and has a level-2 slot to restore).
/// Centralizes the RAW "half-level pool, no slot above 5th" math into
/// a flat per-tier shape; lets the wizard keep firing low-tier control
/// spells (Magic Missile / Shield / Web) across encounters without a
/// full long rest. Backed by the shared `SpellSlotRecovery` shape.
pub static ARCANE_RECOVERY: LazyLock<SpellSlotRecovery> = LazyLock::new(|| SpellSlotRecovery {
    name: "arcane recovery",
    aliases: &["ar", "recover"],
    tag: ARCANE_RECOVERY_TAG,
});

/// Class-feature tag for the Druid Circle of the Land's **Natural
/// Recovery** (level 2). RAW: once per day during a short rest, recover
/// spell slots whose combined levels equal half the druid's level
/// (rounded up), with no slot above 5th. We collapse that pool into a
/// fixed-shape recovery — one level-1 slot at any level, plus one
/// level-2 slot at level 3+ — mirroring the Arcane Recovery shape so
/// the gate stays a single feature-flag check rather than a slot
/// picker. Listed in `SHORT_REST_FEATURES` so a short rest re-enables
/// the once-per-rest charge alongside Arcane Recovery.
///
/// Distinct from Arcane Recovery only in flavor and template placement:
/// the recovery math is identical (RAW pool: ceil(level/2) slot-levels,
/// no slot above 5th). Ships on a new LAND_DRUID_TEMPLATE — the
/// baseline DRUID_TEMPLATE stays feature-free so the Circle of the
/// Land subclass reads unambiguously.
pub const NATURAL_RECOVERY_TAG: &str = "druid.natural_recovery";

/// Natural Recovery — Druid Circle of the Land feature. Free-cost
/// (no spell slot / no action-lane burn beyond the free-action tick),
/// gated on the once-per-rest feature flag. Restores one level-1
/// spell slot plus a level-2 slot at level 3+, matching the Arcane
/// Recovery envelope for engine uniformity — both features fold
/// through the shared `SpellSlotRecovery` shape. Fires the recovery
/// even when the druid isn't in combat — the RAW gate is "during a
/// short rest," which our engine collapses to the free-cost action
/// lane (mirrors Arcane Recovery's out-of-combat usage pattern).
pub static NATURAL_RECOVERY: LazyLock<SpellSlotRecovery> = LazyLock::new(|| SpellSlotRecovery {
    name: "natural recovery",
    aliases: &["nr", "natural"],
    tag: NATURAL_RECOVERY_TAG,
});

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
/// ranged). One-shot via the clear-on-attack consume site. Backed by
/// the shared `ManeuverPrime` shape.
pub static PRECISION_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "precision attack",
    aliases: &["precision", "pra"],
    tag: PRECISION_ATTACK_TAG,
    prime_condition: Condition::PrecisionAttacking,
    // 2-round window so the prime survives until the fighter's next
    // swing even if their turn ends on a movement-only sequence
    // (mirrors Trip / Menacing / Smite primes).
    timer: ConditionTimer::Rounds(2),
    log_line: "  precision attack: fighter's next attack roll gains +4.",
});

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
/// moment a melee swing lands. Backed by the shared `ManeuverPrime`
/// shape.
pub static SWEEPING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "sweeping attack",
    aliases: &["sweep", "swa"],
    tag: SWEEPING_ATTACK_TAG,
    prime_condition: Condition::SweepingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  sweeping attack: fighter's next melee hit splashes to an adjacent foe.",
});

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
/// Backed by the shared `ManeuverPrime` shape.
pub static LUNGING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "lunging attack",
    aliases: &["lunge", "la"],
    tag: LUNGING_ATTACK_TAG,
    prime_condition: Condition::LungingAttacking,
    // Short timer caps the prime so an idle fighter doesn't carry the
    // reach extension across rests. CONSUMED_ON_ATTACK clears it on
    // the next swing; the timer is just a safety net.
    timer: ConditionTimer::Rounds(2),
    log_line: "  lunging attack: fighter's next melee swing gains +5 ft of reach.",
});

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

/// 5e Warlock — Otherworldly Patron **The Fiend**, level-1 feature
/// **Dark One's Blessing**. Passive: whenever the warlock reduces a
/// hostile creature to 0 HP, they gain temporary hit points equal to
/// their Charisma modifier + warlock level (min 1). RAW: "When you
/// reduce a hostile creature to 0 hit points, you gain temporary hit
/// points equal to your Charisma modifier + your warlock level (a
/// minimum of 1)."
///
/// Read at the `DealDamage::apply` chokepoint on the `Downed` /
/// `Killed` outcome branches: the current turn actor is looked up
/// via `EncounterInstance::current_turn_actor_id`, and if they hold
/// this tag AND the dropped target is on a different team (RAW
/// "hostile"), the temp HP is granted through the standard
/// `GainTempHp` side effect so the max-of-current-and-new stack rule
/// still holds. Attributing the "reduced-to-0 hit" to the current
/// turn actor sidesteps threading an attacker id through every damage
/// path (reactive burns, ongoing DoT, condition drips) — RAW's plain
/// reading is that the warlock is the one landing the killing hit,
/// and out-of-turn triggers (Hellish Rebuke fired on someone else's
/// turn) don't reward the wrong warlock.
///
/// Passive with no per-rest charge — fires every time the trigger
/// condition holds. Add this tag to a warlock template's `features`
/// set to install it. Not shipped on the baseline
/// `WARLOCK_TEMPLATE` (Patron is a subclass pick); rides on the
/// dedicated `FIEND_WARLOCK_TEMPLATE`.
pub const DARK_ONES_BLESSING_TAG: &str = "warlock.dark_ones_blessing";

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

/// 5e War Domain Cleric **War Priest** feature tag (level 1 subclass).
/// RAW: WIS-mod uses per long rest of a bonus-action extra weapon attack
/// after taking the Attack action. We collapse the WIS-scaled charge
/// pool to a single per-short-rest charge so the gating stays uniform
/// with the other class-feature tags (Second Wind / Action Surge /
/// Sacred Weapon) — the once-per-short-rest cadence sits between the
/// once-per-long-rest floor and the WIS-mod / long-rest ceiling.
/// Registered in `SHORT_REST_FEATURES` so short rests refresh it.
pub const WAR_PRIEST_TAG: &str = "cleric.war_priest";

/// War Priest — War Domain Cleric bonus action. Grants the cleric an
/// extra Action token for a follow-up weapon swing this turn. Same
/// action-economy trade as Flurry of Blows / Frenzy / Action Surge —
/// spend a bonus action (plus a short-rest charge) to buy a fresh main
/// Action. The AI's normal attack picker handles the weapon / target
/// selection on the granted swing.
///
/// Once per short rest. Gated on the cleric holding the `WAR_PRIEST_TAG`
/// feature flag AND being combat-active. Pairs naturally with the
/// cleric's Divine Strike prime — bonus-action Divine Strike into
/// bonus-action War Priest wouldn't work RAW (only one bonus action per
/// turn), but Divine Strike prime one round → War Priest next round
/// with the prime still up gets the follow-up swing riding the +1d8
/// radiant rider.
pub struct WarPriest {}

impl Action for WarPriest {
    fn name(&self) -> &str {
        "war priest"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wp", "priest"]
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
        feature_ready(encounter, caster_id, WAR_PRIEST_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_feature_and_grant_extra_action(
            encounter,
            caster_id,
            WAR_PRIEST_TAG,
            "  war priest: cleric gains an extra Action for a follow-up strike.",
        )
    }
}

pub static WAR_PRIEST: LazyLock<WarPriest> = LazyLock::new(|| WarPriest {});

/// 5e War Domain Cleric **Guided Strike** feature tag (Channel Divinity,
/// level 2 subclass). Once per short rest, prime the cleric's next
/// attack roll with a flat +10 accuracy buff — the largest single-swing
/// accuracy buff in the game, meant to turn a marginal near-miss into a
/// guaranteed connect. Registered in `SHORT_REST_FEATURES` so short
/// rests refresh it.
///
/// Distinct from Sacred Weapon (Devotion Paladin Channel Divinity):
/// Sacred Weapon lasts 10 rounds and adds +CHA (typically +2-4);
/// Guided Strike is a one-shot flat +10, higher per-swing buff at the
/// cost of the one-and-done lifecycle.
pub const GUIDED_STRIKE_TAG: &str = "cleric.guided_strike";

/// Channel Divinity: Guided Strike — War Domain Cleric bonus action.
/// Installs the `GuidedStriking` prime on the cleric for their next
/// attack roll: +10 flat bonus, consumed on the first swing that fires.
/// Once per short rest.
///
/// Sibling to Sacred Weapon (Devotion Paladin Channel Divinity) on the
/// attack-roll buff lane — Sacred Weapon spreads +CHA across a 10-round
/// concentration window, Guided Strike concentrates a fatter +10 on a
/// single-shot prime. Backed by the same `feature_prime_ready` gate
/// and `prime_self_condition` install helper the other bonus-action
/// primes route through.
pub struct GuidedStrike {}

impl Action for GuidedStrike {
    fn name(&self) -> &str {
        "guided strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gs", "cd-guided", "guided"]
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
        feature_prime_ready(encounter, caster_id, GUIDED_STRIKE_TAG, Condition::GuidedStriking)
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
            GUIDED_STRIKE_TAG,
            Condition::GuidedStriking,
            // Tick-down timer caps the prime to a single round so an
            // idle cleric doesn't carry it across encounters. Consumed
            // by `CONSUMED_ON_ATTACK` on the first swing that fires.
            ConditionTimer::UntilStartOfNextTurn,
            "  guided strike: cleric's next attack rides a +10 divine accuracy buff.",
        )
    }
}

pub static GUIDED_STRIKE: LazyLock<GuidedStrike> = LazyLock::new(|| GuidedStrike {});

/// 5e Light Domain Cleric **Radiance of the Dawn** Channel Divinity tag
/// (level 2 subclass). Once per short rest, action-cost 30ft self-centered
/// radiant burst — every enemy in range makes a CON save vs the cleric's
/// spell save DC. On fail: 2d10 + cleric level radiant. On save: half.
/// RAW clause "any magical darkness within 30 ft is dispelled" is a
/// no-op in our engine (magical darkness isn't a modeled hazard).
///
/// Registered in `SHORT_REST_FEATURES` so short rests refresh the charge
/// alongside Turn Undead / Preserve Life / Guided Strike / War Priest.
pub const RADIANCE_OF_THE_DAWN_TAG: &str = "cleric.radiance_of_the_dawn";

/// Channel Divinity: Radiance of the Dawn — Light Domain Cleric action.
/// Self-centered 30ft (12-tile) enemy burst. Each enemy rolls a CON save
/// vs the cleric's spell save DC (WIS-based); pass = half, fail = full
/// `2d10 + cleric level` radiant. Once per short rest.
///
/// The Light Domain's signature crowd-control-and-damage lane — sibling
/// to Turn Undead (Frightened install, undead-only) and Preserve Life
/// (mass heal). Where Turn Undead targets a narrow creature type and
/// Preserve Life heals allies, Radiance of the Dawn is undiscriminating
/// damage against every enemy in the burst. RAW gates on the Light
/// Domain subclass; we surface it through the `LIGHT_CLERIC_TEMPLATE`
/// subclass template so a baseline cleric can't fire it.
///
/// Pairs naturally with Guiding Bolt (single-target 4d6 radiant) and
/// Sacred Flame (single-target 1d8 radiant) for a radiant-damage
/// combo — the Light cleric plays the "burst of light" damage lane
/// where Turn Undead / Preserve Life play the crowd-control / heal
/// lanes.
pub struct RadianceOfTheDawn {}

impl Action for RadianceOfTheDawn {
    fn name(&self) -> &str {
        "radiance of the dawn"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rod", "cd-radiance", "dawn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, RADIANCE_OF_THE_DAWN_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the charge up-front so a mid-resolution actor lookup can't
        // double-fire (same shape as Preserve Life / Turn Undead).
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RADIANCE_OF_THE_DAWN_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let level = caster.level();
        // 2d10 + cleric level radiant — rolled once and shared across
        // the burst (5e AoE damage rolls are shared). Log the breakdown
        // for the same reason Flame Strike / Destructive Wave do:
        // players can trace back exactly how much each target ate.
        let raw = encounter.roll(&Dice::new(2, 10));
        let damage = raw + level;
        encounter.log(format!(
            "  radiance of the dawn: 2d10({})+{} radiant, DC {} CON save for half.",
            raw, level, dc
        ));
        resolve_enemy_burst_save_damage(
            encounter,
            caster_id,
            center,
            12,
            AbilityScoreType::Constitution,
            dc,
            damage,
            DamageType::Radiant,
        )
    }
}

pub static RADIANCE_OF_THE_DAWN: LazyLock<RadianceOfTheDawn> =
    LazyLock::new(|| RadianceOfTheDawn {});

/// 5e Light Domain Cleric level-1 subclass feature — **Warding Flare**.
/// Passive reaction: when a creature within 30 ft that the cleric can
/// see makes an attack roll against them, the cleric can use their
/// reaction to impose disadvantage on the attack roll. RAW uses per
/// long rest equal to WIS mod (min 1); we collapse to a once-per-
/// short-rest charge (registered in `SHORT_REST_FEATURES`) so the
/// Light cleric doesn't lose the flare between engagements — same
/// gating shape as the Cleric's other Channel Divinity charges.
///
/// No action surface — the trigger fires automatically at the attack
/// chokepoint (`resolve_attack` for weapon swings and
/// `spell_attack_outcome` for spell attacks) via
/// `EncounterInstance::apply_warding_flare_disadvantage`. Ships as a
/// pure passive template tag on `LIGHT_CLERIC_TEMPLATE` — no per-
/// action `Action` impl, no bonus-action lane, mirroring how
/// Blood Frenzy / Fast Movement / Mindless Rage layer as passive
/// tags without a bonus-action shape.
///
/// RAW blindness clause ("can see"): we approximate with `!Blinded`.
/// The 30ft range collapses to a footprint-Chebyshev cap of 12 tiles
/// (2.5ft grid). The engine reads the trigger in
/// `apply_warding_flare_disadvantage`, spending the reaction + charge
/// on fire and combining `Disadvantage` into the attack mode.
pub const WARDING_FLARE_TAG: &str = "cleric.warding_flare";

/// 5e Oathbreaker Paladin (DMG) level-15 subclass feature —
/// **Fanatical Focus**. Auto-fire once-per-short-rest reroll of a
/// failed saving throw. RAW: "If you fail a saving throw while your
/// aura is active, you can reroll it. Once you use this ability, you
/// can't use it again until you finish a short or long rest."
///
/// Distinct from Fighter Indomitable (`INDOMITABLE_TAG`) in three
/// ways:
///   1. **Auto-fire vs pre-primed** — Indomitable requires an Action
///      call to set `mark_indomitable_pending` *before* the save; if
///      the fighter didn't pre-prime, a failed save doesn't trigger.
///      Fanatical Focus fires automatically on the first failed save
///      after a short rest — no advance planning needed.
///   2. **Short rest vs long rest** — Indomitable is once per long
///      rest (not in `SHORT_REST_FEATURES`). Fanatical Focus is once
///      per short rest — the Oathbreaker paladin gets one on every
///      engagement rather than one per adventuring day.
///   3. **Uses a feature charge, not a pending latch** — Indomitable
///      spends the tag at Action time and stashes a pending latch on
///      the actor; Fanatical Focus spends the tag *at the save site*
///      the first time the paladin fails a save with an unspent
///      charge. One less state field to carry per actor, and the
///      "sees an unspent charge → fire" gate reads uniformly with
///      Legendary Resistance's shape.
///
/// RAW gates on "while your aura is active" (Aura of Protection, lv6+).
/// Since Aura of Protection isn't a togglable resource in the engine —
/// it's always on for any level-6+ paladin holding the
/// `has_aura_of_protection` flag — the "while active" clause collapses
/// to a no-op on the Oathbreaker paladin chassis. A hypothetical build
/// that lost the aura (e.g. Silenced-cohort suppression down the line)
/// would still fire this: the RAW clause exists as flavor rather than
/// a mechanical gate, so we don't tie the reroll to a re-check of the
/// aura flag.
///
/// Wired in `EncounterInstance::roll_save_with_extra_mode`: on a
/// failed save (post-Indomitable, pre-Legendary-Resistance), if the
/// actor holds `FANATICAL_FOCUS_TAG` on `features_remaining`, the tag
/// is spent and the save re-rolled once with the same modifier /
/// mode. Legendary Resistance stays checked below the Fanatical
/// Focus gate — RAW: LR is an active DM/boss resource, so the
/// once-per-short-rest passive fires first.
pub const FANATICAL_FOCUS_TAG: &str = "paladin.fanatical_focus";

/// 5e Oathbreaker Paladin (DMG) level-7 subclass feature — **Aura of
/// Hate**. Passive template flag: the paladin and any fiends / undead
/// within 10 ft gain a bonus to melee weapon damage rolls equal to the
/// paladin's Charisma modifier (minimum +1). We collapse the RAW aura
/// shape to a self-only bonus at the caster-side melee bumps table in
/// `engine::attack::resolve_attack_outcome` — the paladin themselves
/// picks up the +CHA mod on every melee swing while the flag is set.
///
/// The "any fiends and undead within 10 ft" clause is dropped in the
/// current model since we don't tag allied fiends / undead as an
/// aura-eligible cohort at the template level. Adding a broader
/// "adjacent-fiend-or-undead ally gets +CHA mod melee damage" scan
/// would need a fresh footprint-Chebyshev pass at the attack site,
/// which is an aura-shape mismatch with the other paladin auras (Aura
/// of Protection, Aura of Courage, Aura of Devotion) that read the
/// EMITTER's flag on the ally-side rather than the ATTACKER's own
/// flag on the caster-side. Self-only keeps the read local and the
/// bump-table plumbing uniform.
///
/// Reads through the existing melee bumps table next to Rage (+2),
/// Dueling (+2), Two-Weapon Fighting (+STR mod) — one new tuple, no
/// re-shape of the attack-outcome shape. Minimum +1 clause folds in
/// via `max(1)` on the CHA modifier lookup, matching RAW.
///
/// Distinguished from Vow of Enmity (Vengeance paladin, once-per-
/// long-rest Advantage prime): Aura of Hate is passive, always-on
/// (once the flag ships on the Oathbreaker template), and a damage
/// bump rather than an attack-mode bump. Distinguished from
/// Improved Divine Smite (+1d8 radiant on every melee hit, no gate):
/// Aura of Hate is a flat mod rather than a die, and its damage rides
/// as part of the base weapon damage roll rather than a separate
/// `push_die_rider` payload — so a resistance-halving on the weapon
/// type also halves the Aura of Hate bonus, mirroring how Rage /
/// Dueling / Two-Weapon Fighting fold into the base swing.
pub const AURA_OF_HATE_TAG: &str = "paladin.aura_of_hate";
