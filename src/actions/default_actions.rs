use crate::{
    actions::action_template::TargetingSchema,
    engine::{side_effects::GiveResource, types::Coordinate},
};
use std::{collections::HashSet, sync::LazyLock};

use crate::{
    actions::action_template::{Action, first_target_id, first_target_location},
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
        let Some(dest) = first_target_location(target_locations) else {
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
        let Some(target_location) = first_target_location(target_locations) else {
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
        let Some(target_id) = first_target_id(target_ids) else {
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(ally_id) = first_target_id(target_ids) else {
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

/// 5e Shove (special melee attack): contested Athletics check — attacker's
/// d20 + STR mod vs target's d20 + max(STR mod, DEX mod). On success the
/// target is knocked prone AND pushed 1 tile (5 ft) away from the attacker.
/// Target must be no more than one size category larger. Costs an Action.
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // 5e: target must be no more than one size larger.
        caster.size().can_grapple(target.size())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::dice::Dice;
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_name = caster.name().to_string();
        let caster_str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let caster_loc = caster.location();
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let target_name = target.name().to_string();
        let target_str_mod = target.ability_modifier(AbilityScoreType::Strength);
        let target_dex_mod = target.ability_modifier(AbilityScoreType::Dexterity);
        let target_best = target_str_mod.max(target_dex_mod);

        // Contested check: d20 + STR vs d20 + max(STR, DEX). Attacker wins ties.
        let d20 = Dice::new(1, 20);
        let atk_roll = encounter.roll(&d20) as i32 + caster_str_mod;
        let def_roll = encounter.roll(&d20) as i32 + target_best;
        encounter.log(format!(
            "  shove: {} rolls {} vs {} rolls {}",
            caster_name, atk_roll, target_name, def_roll
        ));
        if atk_roll < def_roll {
            encounter.log("  shove: target resists");
            return Vec::new();
        }
        encounter.log("  shove: target knocked prone and pushed");
        vec![
            Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: crate::conditions::Condition::Prone,
                timer: crate::conditions::ConditionTimer::Permanent,
            }) as Box<dyn crate::engine::side_effects::ApplicableSideEffect>,
            Box::new(crate::engine::side_effects::PushActor {
                actor_id: target_id,
                from: caster_loc,
                max_tiles: 1,
            }),
        ]
    }
}

pub static SHOVE: LazyLock<Shove> = LazyLock::new(|| Shove {});

/// 5e Grapple (special melee attack): contested Athletics check — attacker's
/// d20 + STR mod vs target's d20 + max(STR mod, DEX mod). On success the
/// target gains the Grappled condition (speed = 0). Target must be no more
/// than one size category larger. Costs an Action.
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // 5e: target must be no more than one size larger.
        caster.size().can_grapple(target.size())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::dice::Dice;
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_name = caster.name().to_string();
        let caster_str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let target_name = target.name().to_string();
        let target_str_mod = target.ability_modifier(AbilityScoreType::Strength);
        let target_dex_mod = target.ability_modifier(AbilityScoreType::Dexterity);
        let target_best = target_str_mod.max(target_dex_mod);

        // Contested check: d20 + STR vs d20 + max(STR, DEX). Attacker wins ties.
        let d20 = Dice::new(1, 20);
        let atk_roll = encounter.roll(&d20) as i32 + caster_str_mod;
        let def_roll = encounter.roll(&d20) as i32 + target_best;
        encounter.log(format!(
            "  grapple: {} rolls {} vs {} rolls {}",
            caster_name, atk_roll, target_name, def_roll
        ));
        if atk_roll < def_roll {
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

/// 5e Grapple Escape — a grappled creature uses its Action to attempt to
/// break free. The actor rolls d20 + max(STR mod, DEX mod) vs DC 13
/// (approximation of 8 + typical grappler STR mod + prof bonus). On
/// success the Grappled (or Adhered / EarthenGrasped) condition is removed.
pub struct GrappleEscape {}

impl Action for GrappleEscape {
    fn name(&self) -> &str {
        "escape grapple"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["escape", "break free"]
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
        use crate::conditions::Condition;
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| {
                a.is_combat_active()
                    && (a.has_condition(Condition::Grappled)
                        || a.has_condition(Condition::Adhered)
                        || a.has_condition(Condition::EarthenGrasped))
            })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::types::AbilityScoreType;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = actor.ability_modifier(AbilityScoreType::Strength);
        let dex_mod = actor.ability_modifier(AbilityScoreType::Dexterity);
        let best_mod = str_mod.max(dex_mod);
        // Snapshot which grapple-like conditions are active before we
        // mutably borrow `encounter` for the roll and log calls.
        let active_conditions: Vec<Condition> =
            [Condition::Grappled, Condition::Adhered, Condition::EarthenGrasped]
                .into_iter()
                .filter(|c| actor.has_condition(*c))
                .collect();
        // Drop the immutable borrow of `actor` before rolling.
        let roll = encounter.roll(&crate::engine::dice::Dice::new(1, 20)) as i32;
        let total = roll + best_mod;
        let dc = 13;
        encounter.log(format!(
            "  escape grapple: 1d20({}){:+} = {} vs DC {}",
            roll, best_mod, total, dc
        ));
        if total >= dc {
            encounter.log("  broke free!".to_string());
            let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
                Vec::new();
            // Remove whichever grapple-like condition was active
            for condition in active_conditions {
                effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                    actor_id: caster_id,
                    condition,
                }));
            }
            effects
        } else {
            encounter.log("  failed to break free.".to_string());
            Vec::new()
        }
    }
}

pub static GRAPPLE_ESCAPE: LazyLock<GrappleEscape> = LazyLock::new(|| GrappleEscape {});

/// 5e Hide action — Stealth check; on success the actor becomes Hidden
/// (attackers have disadvantage, you have advantage on your next attack).
/// DC is the highest passive Perception (10 + WIS mod) among active
/// enemies, defaulting to 10 if none are present. Cannot be used while
/// any enemy is footprint-adjacent — you can't realistically duck from
/// sight while they're inside arm's reach.
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
        // Find the highest passive Perception among active enemies, via
        // `ActorInstance::passive_perception` (which folds in Perception
        // skill proficiency where applicable).
        let caster_team = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.team())
            .unwrap_or(0);
        let dc = encounter
            .actors
            .iter()
            .filter(|(id, a)| {
                **id != caster_id && a.team() != caster_team && a.is_combat_active()
            })
            .map(|(_, a)| a.passive_perception())
            .max()
            .unwrap_or(10);
        let save = encounter.roll_save(caster_id, AbilityScoreType::Dexterity, dc);
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

/// 5e Search action — Wisdom (Perception) check vs the Stealth DC of
/// hidden enemies. Costs an Action. On success, every enemy within the
/// searcher's normal sight range whose Stealth roll the Perception check
/// beats loses their Hidden / Invisible cover. We approximate the
/// "Stealth DC" with `12 + DEX modifier` (the standard passive-Stealth
/// shape) per target, computed at the call site. Range is bounded by
/// footprint-Chebyshev gap of 12 (60 ft) — a reasonable in-combat
/// "scan the room" envelope. The Perception check itself routes through
/// `roll_ability_check` so racial / passive bonuses (Keen Senses, etc.)
/// stack on top cleanly.
pub struct Search {}

impl Action for Search {
    fn name(&self) -> &str {
        "search"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "scan", "look"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::types::{AbilityScoreType, Skill};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        const SEARCH_RANGE: isize = 12;

        let Some(searcher) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let searcher_team = searcher.team();
        let searcher_loc = searcher.location();
        let searcher_size = get_tiles_from_size(searcher.size());
        // Snapshot blinded — auto-fails the sight-based search.
        let blinded = searcher.has_condition(Condition::Blinded);
        if blinded {
            encounter.log("  search: blinded, fails to spot anything.".to_string());
            return Vec::new();
        }

        // Roll Perception once — the same roll compares against every
        // hidden enemy's Stealth DC. Mirrors 5e Perception scan semantics.
        let perception = encounter.roll_ability_check(
            caster_id,
            AbilityScoreType::Wisdom,
            Some(Skill::Perception),
        );
        encounter.log(format!("  search: perception check = {}", perception));

        // Walk the actor table; for any hidden / invisible enemy in range
        // with LOS, compare the searcher's roll to the target's stealth
        // DC. On a beat, reveal them.
        let candidates: Vec<usize> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || a.team() == searcher_team || !a.is_combat_active() {
                    return None;
                }
                if !a.has_condition(Condition::Hidden) && !a.has_condition(Condition::Invisible) {
                    return None;
                }
                let dist = footprint_chebyshev(
                    searcher_loc,
                    searcher_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                );
                if dist > SEARCH_RANGE {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut revealed = 0usize;
        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
            Vec::new();
        for target_id in candidates {
            if !encounter.actor_has_line_of_sight(caster_id, target_id) {
                continue;
            }
            let target = match encounter.actors.get(&target_id) {
                Some(a) => a,
                None => continue,
            };
            let dex_mod = target.ability_modifier(AbilityScoreType::Dexterity);
            let mut dc = 12 + dex_mod;
            if target.has_skill(Skill::Stealth) {
                dc += target.proficiency_bonus();
            }
            if perception < dc {
                continue;
            }
            let target_name = target.name().to_string();
            if target.has_condition(Condition::Hidden) {
                encounter.log(format!(
                    "  search: spotted {} (DC {}) — hidden status broken.",
                    target_name, dc
                ));
                effects.push(Box::new(
                    crate::engine::side_effects::RemoveCondition {
                        actor_id: target_id,
                        condition: Condition::Hidden,
                    },
                ));
                revealed += 1;
            } else if target.has_condition(Condition::Invisible) {
                // 5e: Search reveals an invisible creature's *location*; we
                // model the location-reveal by tagging Outlined for a
                // round. The Invisible condition itself stays (since
                // becoming visible would dispel the spell), but Outlined
                // grants attack advantage to allies for the round.
                encounter.log(format!(
                    "  search: pinpoint {}'s invisible location (DC {}).",
                    target_name, dc
                ));
                effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Outlined,
                    timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
                }));
                revealed += 1;
            }
        }
        if revealed == 0 {
            encounter.log("  search: nothing new spotted.".to_string());
        }
        effects
    }
}

pub static SEARCH: LazyLock<Search> = LazyLock::new(|| Search {});

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
            &*GRAPPLE_ESCAPE,
            &*HIDE,
            &*SEARCH,
        ]
    },
);
