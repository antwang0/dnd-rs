use crate::{
    actions::action_template::TargetingSchema,
    engine::{side_effects::GiveResource, types::Coordinate},
};
use std::{collections::HashSet, sync::LazyLock};

use crate::{
    actions::action_template::Action,
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{MoveActor, Resource, SkipTurn},
    },
};

pub struct Move {}

impl Action for Move {
    fn name(&self) -> &str {
        "move"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mv"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        let Some(dest) = target_locations.and_then(|t| t.first().copied()) else {
            return Vec::new();
        };
        let Some(dist) = encounter.path_cost_to(caster_id, dest) else {
            return Vec::new();
        };
        vec![Resource::Movement(dist)]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(tl) = target_locations else {
            return false;
        };
        let Some(&coord) = tl.first() else {
            return false;
        };
        // path_cost_to verifies destination footprint AND walkable path
        // within remaining movement budget.
        if encounter.path_cost_to(caster_id, coord).is_none() {
            return false;
        }
        // Frightened: can't willingly move closer to any enemy. We don't
        // track per-source fear yet, so any enemy is a "source." A move
        // that strictly decreases the gap to any enemy is forbidden.
        use crate::conditions::Condition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return true;
        };
        if !actor.has_condition(Condition::Frightened) {
            return true;
        }
        let my_size = get_tiles_from_size(actor.size());
        let my_loc = actor.location();
        let my_team = actor.team();
        for (other_id, other) in encounter.actors.iter() {
            if *other_id == caster_id || other.team() == my_team || !other.is_combat_active() {
                continue;
            }
            let o_loc = other.location();
            let o_size = get_tiles_from_size(other.size());
            let cur = footprint_chebyshev(my_loc, my_size, o_loc, o_size);
            let after = footprint_chebyshev(coord, my_size, o_loc, o_size);
            if after < cur {
                return false;
            }
        }
        true
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_location) = target_locations.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        // Use the same Dijkstra path that `cost` charged for, so OAs fire on
        // every threatened-square exit along the way. Falls back to a
        // single-tile teleport if pathing fails (validate should already
        // have caught this, but defense in depth).
        let path = encounter
            .path_to(caster_id, target_location)
            .unwrap_or_else(|| vec![target_location]);
        vec![Box::new(MoveActor {
            actor_id: caster_id,
            path,
        })]
    }
}

pub static MOVE: LazyLock<Move> = LazyLock::new(|| Move {});

pub struct Skip {}

impl Action for Skip {
    fn name(&self) -> &str {
        "skip"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["s"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        Vec::new()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(SkipTurn {})]
    }
}

pub static SKIP: LazyLock<Skip> = LazyLock::new(|| Skip {});

pub struct Dash {}

impl Action for Dash {
    fn name(&self) -> &str {
        "dash"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dsh"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(actor) = encounter.get_actor(caster_id) else {
            return Vec::new();
        };
        let speed = actor.speed();
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static DASH: LazyLock<Dash> = LazyLock::new(|| Dash {});

/// Stand up from being prone. 5e: standing up costs half your speed in
/// movement. Only valid while the caster has the Prone condition.
pub struct StandUp {}

impl Action for StandUp {
    fn name(&self) -> &str {
        "stand"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["standup", "su"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        match encounter.actors.get(&caster_id) {
            Some(a) => vec![Resource::Movement(a.speed() / 2.0)],
            None => Vec::new(),
        }
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
            .is_some_and(|a| a.has_condition(crate::conditions::Condition::Prone))
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::RemoveCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Prone,
        })]
    }
}

pub static STAND_UP: LazyLock<StandUp> = LazyLock::new(|| StandUp {});

/// Dodge action — attacks against you have disadvantage and you have
/// advantage on DEX saves until the start of your next turn (5e). Costs
/// an Action; harmless flag-flip side effect.
pub struct Dodge {}

impl Action for Dodge {
    fn name(&self) -> &str {
        "dodge"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dg"]
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
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDodging {
            actor_id: caster_id,
            dodging: true,
        })]
    }
}

pub static DODGE: LazyLock<Dodge> = LazyLock::new(|| Dodge {});

/// Disengage action — your movement this turn doesn't provoke
/// opportunity attacks (5e). Costs an Action.
pub struct Disengage {}

impl Action for Disengage {
    fn name(&self) -> &str {
        "disengage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["de", "dis"]
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
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDisengaging {
            actor_id: caster_id,
            disengaging: true,
        })]
    }
}

pub static DISENGAGE: LazyLock<Disengage> = LazyLock::new(|| Disengage {});

/// 5e Help action: target one ally; their next attack against a
/// designated foe before the start of *your* next turn has advantage.
/// We collapse the timing slightly: we record `Helped` against *any*
/// future attack, the helped actor consumes it on their next attack.
pub struct Help {}

impl Action for Help {
    fn name(&self) -> &str {
        "help"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["h", "assist"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Help requires an *ally* target — never self, never an enemy.
        let Some(target_id) = target_ids.and_then(|v| v.first().copied()) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == caster.team() && target.is_combat_active()
    }
    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(ally_id) = target_ids.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        // Pick the closest hostile to the helped ally as the "designated
        // foe" for the grant. This mirrors RAW where the helper picks a
        // target; using closest enemy is a reasonable default. If there
        // is no enemy in view, we skip the grant and just leave the
        // Helped condition — the engine reads either lane to grant
        // advantage on the next swing.
        let designated = encounter.actors.get(&ally_id).and_then(|a| {
            let my_team = a.team();
            let my_loc = a.location();
            encounter
                .actors
                .iter()
                .filter(|(id, other)| {
                    **id != ally_id && other.team() != my_team && other.is_combat_active()
                })
                .min_by_key(|(_, other)| {
                    let dx = other.location().x - my_loc.x;
                    let dy = other.location().y - my_loc.y;
                    dx.unsigned_abs().max(dy.unsigned_abs())
                })
                .map(|(id, _)| *id)
        });
        if let Some(target) = encounter.actors.get_mut(&ally_id) {
            match designated {
                Some(against) => target.set_help_grant(Some(
                    crate::actors::actor_template::HelpGrant {
                        helper_id: caster_id,
                        against,
                    },
                )),
                None => target.set_help_grant(None),
            }
        }
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: ally_id,
            condition: crate::conditions::Condition::Helped,
            timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static HELP: LazyLock<Help> = LazyLock::new(|| Help {});

/// 5e Shove (special melee attack): contested STR (Athletics) vs target's
/// STR or DEX (best of). On success, knock prone OR push 5 ft. We model
/// the simpler "knock prone" variant — push-direction needs movement
/// modeling we don't have. Costs an Action.
pub struct Shove {}

impl Action for Shove {
    fn name(&self) -> &str {
        "shove"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh", "push"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
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
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = target_ids.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        // Contested STR check — caster's d20+STR vs target's d20+max(STR,DEX).
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = 10 + crate::engine::util::modifier_from_score(caster.ability_score(AbilityScoreType::Strength));
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, dc);
        if save.passed() {
            encounter.log("  shove: target stays upright");
            return Vec::new();
        }
        encounter.log("  shove: target knocked prone");
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: target_id,
            condition: crate::conditions::Condition::Prone,
            timer: crate::conditions::ConditionTimer::Permanent,
        })]
    }
}

pub static SHOVE: LazyLock<Shove> = LazyLock::new(|| Shove {});

/// 5e Grapple (special melee attack): contested STR (Athletics) vs the
/// target's STR or DEX (best). On success, target is Grappled — speed
/// drops to 0 until the grappler releases or is knocked Incapacitated.
/// Costs an Action. Symmetric implementation with Shove (knock-prone).
pub struct Grapple {}

impl Action for Grapple {
    fn name(&self) -> &str {
        "grapple"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gr", "grab"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
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
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = target_ids.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Athletics-flavored: DC 10 + grappler's STR mod (matches Shove).
        let dc = 10
            + crate::engine::util::modifier_from_score(
                caster.ability_score(AbilityScoreType::Strength),
            );
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, dc);
        if save.passed() {
            encounter.log("  grapple: target slips free");
            return Vec::new();
        }
        encounter.log("  grapple: target is grappled");
        // Grappled until the grappler releases or is incapacitated. We
        // don't yet model release as an action — for now we use a long
        // Rounds timer (10 rounds = 1 minute) so it has a definite
        // expiration. Concentration-style auto-release would be a follow-up.
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: target_id,
            condition: crate::conditions::Condition::Grappled,
            timer: crate::conditions::ConditionTimer::Rounds(10),
        })]
    }
}

pub static GRAPPLE: LazyLock<Grapple> = LazyLock::new(|| Grapple {});

/// 5e Hide action — Stealth check; on success the actor becomes Hidden
/// (attackers have disadvantage, you have advantage on your next attack).
/// We use a flat DC 10 since we don't model passive Perception today.
/// Cannot be used while any enemy is footprint-adjacent — you can't
/// realistically duck from sight while they're inside arm's reach.
pub struct Hide {}

impl Action for Hide {
    fn name(&self) -> &str {
        "hide"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hd"]
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
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(me) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let my_team = me.team();
        let my_loc = me.location();
        let my_size = get_tiles_from_size(me.size());
        // Hide is invalid while a hostile is footprint-adjacent — too
        // close to slip out of sight.
        let in_melee = encounter.actors.iter().any(|(id, other)| {
            *id != caster_id
                && other.team() != my_team
                && other.is_combat_active()
                && footprint_chebyshev(
                    other.location(),
                    get_tiles_from_size(other.size()),
                    my_loc,
                    my_size,
                ) == 0
        });
        !in_melee
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let save = encounter.roll_save(caster_id, AbilityScoreType::Dexterity, 10);
        if !save.passed() {
            encounter.log("  hide: stealth fails");
            return Vec::new();
        }
        encounter.log("  hide: succeeds");
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Hidden,
            timer: crate::conditions::ConditionTimer::Permanent,
        })]
    }
}

pub static HIDE: LazyLock<Hide> = LazyLock::new(|| Hide {});

pub static DEFAULT_ACTIONS: LazyLock<Vec<&'static (dyn Action + Send + Sync)>> = LazyLock::new(
    || {
        vec![
            &*MOVE,
            &*DASH,
            &*SKIP,
            &*STAND_UP,
            &*DODGE,
            &*DISENGAGE,
            &*HELP,
            &*SHOVE,
            &*GRAPPLE,
            &*HIDE,
        ]
    },
);
