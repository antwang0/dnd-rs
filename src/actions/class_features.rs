use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        Action, TargetingSchema, bonus_action_only, free_cost,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, GiveResource, Heal, Resource},
        types::Coordinate,
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(SECOND_WIND_TAG))
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(ACTION_SURGE_TAG))
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
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Action,
        })]
    }
}

pub static ACTION_SURGE: LazyLock<ActionSurge> = LazyLock::new(|| ActionSurge {});

/// Tag for Cunning Action — at-will class feature, not consumable, so
/// it never appears in `features_remaining`. Kept as a const for
/// symmetry with the once-per-rest tags above and so creature templates
/// can declare it explicitly.
pub const CUNNING_ACTION_TAG: &str = "rogue.cunning_action";

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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(INDOMITABLE_TAG))
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(RAGE_TAG))
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(LAY_ON_HANDS_TAG))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(SACRED_WEAPON_TAG))
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(TURN_UNDEAD_TAG))
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
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Action,
        })]
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
            encounter
                .actors
                .get(&caster_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default(),
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.feature_available(PRESERVE_LIFE_TAG))
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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
