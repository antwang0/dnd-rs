use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, GiveResource, Heal, Resource},
        types::Coordinate,
    },
};

/// Tags used by `ActorInstance::feature_available` / `spend_feature` to
/// gate once-per-long-rest class features. Stored as `&'static str` so
/// actor state stays a flat HashSet instead of carrying an enum import.
pub const SECOND_WIND_TAG: &str = "fighter.second_wind";
pub const ACTION_SURGE_TAG: &str = "fighter.action_surge";

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
        vec![Resource::BonusAction]
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
        Vec::new()
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
        vec![Resource::BonusAction]
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
        vec![Resource::BonusAction]
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
        vec![Resource::BonusAction]
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
        Vec::new()
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
        vec![Resource::BonusAction]
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
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RAGE_TAG);
        }
        encounter.log("  rage: barbarian enters a battle frenzy.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Raging,
            timer: ConditionTimer::Rounds(10),
        })]
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
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
        use crate::engine::util::modifier_from_score;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let level = actor.level();
        let cha_mod = modifier_from_score(actor.ability_score(AbilityScoreType::Charisma));
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
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
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(SACRED_WEAPON_TAG);
        }
        encounter.log("  sacred weapon: paladin's blade glows with divine light.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Sacred,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static SACRED_WEAPON: LazyLock<SacredWeapon> = LazyLock::new(|| SacredWeapon {});
