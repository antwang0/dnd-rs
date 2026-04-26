use crate::actions::action_template::{Action, ActionExecutionInfo, TargetingSchema};
use crate::actors::actor_template::ActorInstance;
use crate::ai::{Controller, ControllerDecision};
use crate::engine::encounter::EncounterInstance;
use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

/// Greedy melee AI: attack an adjacent enemy if possible, otherwise step
/// toward the nearest one, otherwise skip. Intentionally crude — exists to
/// unblock playtesting and serve as a reference impl for the trait.
pub struct SimpleAi;

impl Controller for SimpleAi {
    fn decide(&self, encounter: &EncounterInstance, actor_id: usize) -> ControllerDecision {
        let Some(actor) = encounter.actors.get(&actor_id) else {
            return skip_or_await(encounter, actor_id);
        };
        let my_team = actor.team();
        let my_loc = actor.location();
        let my_size = get_tiles_from_size(actor.size());

        let nearest = encounter
            .actors
            .iter()
            .filter(|(other_id, a)| {
                **other_id != actor_id && a.team() != my_team && a.is_combat_active()
            })
            .map(|(id, a)| (*id, a))
            .min_by_key(|(_, a)| {
                footprint_chebyshev(my_loc, my_size, a.location(), get_tiles_from_size(a.size()))
            });

        let Some((target_id, target)) = nearest else {
            return skip_or_await(encounter, actor_id);
        };
        let dist_to_target = footprint_chebyshev(
            my_loc,
            my_size,
            target.location(),
            get_tiles_from_size(target.size()),
        );

        // 1. Attack with the longest-reach action that can reach the target.
        if let Some(aei) = try_attack(encounter, actor_id, target_id, dist_to_target) {
            return ControllerDecision::Act(aei);
        }

        // 2. Otherwise step toward the target via BFS so we navigate around
        // walls instead of pacing in greedy circles.
        if let Some(aei) = try_step_toward(encounter, actor_id, target_id) {
            return ControllerDecision::Act(aei);
        }

        // 3. Nothing useful — end the turn.
        skip_or_await(encounter, actor_id)
    }
}

/// Pick the SingleActor-targeted action with the longest reach that can hit
/// the target. Skips Move/Skip/Dash because they have different schemas.
fn try_attack(
    encounter: &EncounterInstance,
    caster_id: usize,
    target_id: usize,
    dist_to_target: isize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&caster_id)?;
    let mut best: Option<(isize, &dyn Action)> = None;
    for &action in &actor.actions {
        if !matches!(action.targeting_schema(), TargetingSchema::SingleActor) {
            continue;
        }
        let Some(reach) = action.reach_tiles() else {
            continue;
        };
        if dist_to_target > reach {
            continue;
        }
        if best.is_some_and(|(r, _)| r >= reach) {
            continue;
        }
        best = Some((reach, action));
    }
    let (_, action) = best?;
    let aei = ActionExecutionInfo::new(action, caster_id, Some(vec![target_id]), None, None);
    if aei.validate(encounter) {
        Some(aei)
    } else {
        None
    }
}

/// Walk one BFS step toward `target_id`. Defers actual pathfinding to
/// `EncounterInstance::step_toward_actor` and only validates the resulting
/// Move (which may fail if movement is exhausted, in which case the turn
/// will fall through to skip).
fn try_step_toward(
    encounter: &EncounterInstance,
    caster_id: usize,
    target_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor: &ActorInstance = encounter.actors.get(&caster_id)?;
    let move_action = actor
        .actions
        .iter()
        .find(|a| a.name() == "move")
        .copied()?;
    let dest = encounter.step_toward_actor(caster_id, target_id)?;
    let aei = ActionExecutionInfo::new(move_action, caster_id, None, Some(vec![dest]), None);
    if aei.validate(encounter) {
        Some(aei)
    } else {
        None
    }
}

/// Last-resort: invoke the actor's Skip action so the turn advances. If the
/// actor somehow has no Skip in their action list, fall back to AwaitInput
/// to avoid an infinite engine loop.
fn skip_or_await(encounter: &EncounterInstance, caster_id: usize) -> ControllerDecision {
    let Some(actor) = encounter.actors.get(&caster_id) else {
        return ControllerDecision::AwaitInput;
    };
    let Some(skip) = actor.actions.iter().find(|a| a.name() == "skip").copied() else {
        return ControllerDecision::AwaitInput;
    };
    let aei = ActionExecutionInfo::new(skip, caster_id, None, None, None);
    if aei.validate(encounter) {
        ControllerDecision::Act(aei)
    } else {
        ControllerDecision::AwaitInput
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain_gen::TerrainGenParams;

    fn run_to_completion(seed: u64) -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let ap = ActorGenParams {
            cr_target: 0.5,
            n_teams: 2,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
        let ai = SimpleAi;

        // Hard cap so a runaway loop fails the test instead of hanging.
        for _ in 0..20_000 {
            e.process_stack();
            if e.is_complete() {
                return e;
            }
            let Some(prompt) = e.peek_prompt() else { break };
            let actor_id = prompt.actor_id();
            match ai.decide(&e, actor_id) {
                ControllerDecision::AwaitInput => {
                    panic!("SimpleAi returned AwaitInput — should always act");
                }
                ControllerDecision::Act(aei) => {
                    e.pop_prompt();
                    e.push_action(aei);
                }
            }
        }
        let snap: Vec<String> = e
            .actors
            .values()
            .map(|a| format!("{} t{} hp{} @{}", a.name(), a.team(), a.hitpoints(), a.location()))
            .collect();
        panic!("seed {} did not terminate; survivors: {:?}", seed, snap);
    }

    /// Drive several AI-vs-AI encounters to completion. Validates the
    /// controller dispatch loop and that SimpleAi terminates regardless of
    /// terrain layout.
    #[test]
    fn ai_vs_ai_terminates() {
        for seed in [1u64, 7, 42, 99, 12345] {
            let e = run_to_completion(seed);
            assert!(
                e.winning_team().is_some() || e.living_teams().is_empty(),
                "seed {}: ambiguous outcome",
                seed
            );
        }
    }
}
