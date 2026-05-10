use crate::actions::action_template::{Action, ActionExecutionInfo, MELEE_REACH, TargetingSchema};
use crate::ai::{Controller, ControllerDecision};
use crate::conditions::Condition;
use crate::engine::dice::RollMode;
use crate::engine::encounter::EncounterInstance;
use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

/// Tactical heuristic AI. The decision pipeline runs in priority order:
/// 1. **Kite**: if I have a ranged attack and an enemy is in melee reach
///    of me, step away (one tile) before attacking. Repeated calls per
///    turn means the actor will kite then shoot in the same round.
/// 2. **Focus fire**: among enemies I can hit *this instant*, attack the
///    one with the lowest current HP — finishing wounded targets is
///    higher leverage than spreading damage.
/// 3. **Approach lowest HP**: if no one's in reach, BFS-step toward the
///    weakest visible enemy (not the nearest).
/// 4. **Skip**: nothing useful to do — end the turn.
///
/// Stateless across turns. New behaviors land as new helpers + a new
/// pipeline entry; existing helpers (`try_attack`, `try_step_toward`)
/// stay narrow so adding tactics doesn't tangle them.
pub struct SimpleAi;

impl Controller for SimpleAi {
    fn decide(&self, encounter: &EncounterInstance, actor_id: usize) -> ControllerDecision {
        if !encounter.actors.contains_key(&actor_id) {
            return skip_or_await(encounter, actor_id);
        }

        // 1. Stand up if prone — disadvantage on attacks and 0 movement
        //    otherwise. Costs half-speed; the rest of the turn still has
        //    resources to act.
        if let Some(aei) = try_stand_up(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 2. Kite if we're a ranged attacker under melee threat.
        if has_ranged_attack(encounter, actor_id)
            && under_melee_threat(encounter, actor_id)
            && let Some(aei) = try_step_away_from_threats(encounter, actor_id)
        {
            return ControllerDecision::Act(aei);
        }

        // 3. Heal a dying / wounded ally.
        if let Some(aei) = try_support_heal(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 4. Hold Person — lock down toughest enemy if we have it and
        //    aren't already concentrating on something. Comes before
        //    buffs because hard CC is higher leverage than +1d4.
        if let Some(aei) = try_hold_person(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5. Buff an unbuffed ally — Bless, Shield of Faith. Skipped
        //    while concentrating to avoid spending slots on a spell
        //    that drops the previous one.
        if let Some(aei) = try_buff_ally(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5. AoE — point that catches 2+ enemies, no friendly fire.
        if let Some(aei) = try_attack_aoe(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 6. Focus-fire: pick targets with advantage > normal > disadv;
        //    tie-break by lower HP (finish wounded).
        if let Some(aei) = try_attack_focus_fire(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 7. No one in reach — close on the lowest-HP enemy.
        if let Some(aei) = try_step_toward_lowest_hp(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 8. Nothing useful. End the turn.
        skip_or_await(encounter, actor_id)
    }
}

/// If the actor is Prone, return the StandUp action invocation. The action
/// itself custom-validates `has_condition(Prone)` and pays half-speed in
/// movement.
fn try_stand_up(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::Prone) {
        return None;
    }
    let stand = actor.actions.iter().find(|a| a.name() == "stand").copied()?;
    let aei = ActionExecutionInfo::new(stand, actor_id, None, None, None);
    if aei.validate(encounter) {
        Some(aei)
    } else {
        None
    }
}

/// Cast Hold Person on the toughest in-range enemy if we have it and
/// aren't already concentrating. "Toughest" = highest current HP among
/// not-already-stunned enemies (no point double-locking).
fn try_hold_person(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let hold = actor
        .actions
        .iter()
        .find(|a| a.name() == "hold person")
        .copied()?;
    let my_team = actor.team();

    let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
    ids.sort_unstable();

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for target_id in ids {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        if target.has_condition(Condition::Stunned) {
            continue;
        }
        let aei = ActionExecutionInfo::new(hold, actor_id, Some(vec![target_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = target.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Sort key for advantage-aware target selection — lower wins.
fn mode_priority(mode: RollMode) -> u8 {
    match mode {
        RollMode::Advantage => 0,
        RollMode::Normal => 1,
        RollMode::Disadvantage => 2,
    }
}

/// Heuristic: an action's name contains "heal" or "cure" if it actually
/// restores HP. Used to keep buffing spells (Bless, Shield of Faith) out
/// of the heal pipeline. A future `Action::is_healing()` method would be
/// crisper, but a name check keeps the trait surface small.
fn is_healing_action(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("heal") || lower.contains("cure")
}

/// Heuristic: an action's name suggests a non-damaging buff
/// (Bless, Shield of Faith, ...). Used by `try_buff_ally` so the AI
/// knows what to consider.
fn is_buff_action(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "bless" | "shield of faith"
    )
}

/// True if the actor has any single-actor attack with reach beyond melee.
/// Doesn't require a current valid target — the kite tactic only cares
/// whether we *could* shoot once we have space.
fn has_ranged_attack(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    actor.actions.iter().any(|a| {
        matches!(a.targeting_schema(), TargetingSchema::SingleActor)
            && a.reach_tiles().is_some_and(|r| r > MELEE_REACH)
    })
}

/// True if any combat-active enemy has a melee attack whose reach covers
/// our current footprint distance to them. "Melee" = reach ≤ MELEE_REACH.
fn under_melee_threat(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(me) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let my_team = me.team();
    let my_loc = me.location();
    let my_size = get_tiles_from_size(me.size());

    encounter.actors.iter().any(|(other_id, other)| {
        if *other_id == actor_id || other.team() == my_team || !other.is_combat_active() {
            return false;
        }
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            other.location(),
            get_tiles_from_size(other.size()),
        );
        other.actions.iter().any(|a| {
            matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && a.reach_tiles()
                    .is_some_and(|r| r <= MELEE_REACH && dist <= r)
        })
    })
}

/// Step one tile in the direction that maximizes the minimum footprint
/// distance to any combat-active enemy, validating against Move.
fn try_step_away_from_threats(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let move_action = actor
        .actions
        .iter()
        .find(|a| a.name() == "move")
        .copied()?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    // Snapshot enemy footprints once — we'll project candidate destinations
    // against them. Self-team and downed actors don't count as threats.
    let enemies: Vec<(Coordinate, usize)> = encounter
        .actors
        .iter()
        .filter(|(id, a)| {
            **id != actor_id && a.team() != my_team && a.is_combat_active()
        })
        .map(|(_, a)| (a.location(), get_tiles_from_size(a.size())))
        .collect();
    if enemies.is_empty() {
        return None;
    }

    let current_min = enemies
        .iter()
        .map(|(loc, sz)| footprint_chebyshev(my_loc, my_size, *loc, *sz))
        .min()
        .unwrap_or(0);

    let mut best: Option<(isize, Coordinate)> = None;
    for dy in -1..=1isize {
        for dx in -1..=1isize {
            if dx == 0 && dy == 0 {
                continue;
            }
            let cand = Coordinate::new(my_loc.x + dx, my_loc.y + dy);
            let min_dist = enemies
                .iter()
                .map(|(loc, sz)| footprint_chebyshev(cand, my_size, *loc, *sz))
                .min()
                .unwrap_or(0);
            // Strictly increase distance to closest threat — pacing in
            // place or moving sideways is no better than just shooting.
            if min_dist <= current_min {
                continue;
            }
            if best.is_some_and(|(d, _)| d >= min_dist) {
                continue;
            }
            let aei =
                ActionExecutionInfo::new(move_action, actor_id, None, Some(vec![cand]), None);
            if aei.validate(encounter) {
                best = Some((min_dist, cand));
            }
        }
    }
    let (_, dest) = best?;
    Some(ActionExecutionInfo::new(
        move_action,
        actor_id,
        None,
        Some(vec![dest]),
        None,
    ))
}

/// Cast a healing action (Healing Word, Cure Wounds, ...) on an ally who
/// needs it. Priority: dying allies first (revival prevents death-save
/// failure), then wounded combat-active allies below 50% HP. Stable and
/// full-HP allies are ignored. Self-targeting is excluded — the actor
/// should make hostile turns, not heal themselves preemptively.
///
/// Only actions whose name suggests actual HP restoration ("heal", "cure")
/// qualify — buffs like Bless / Shield of Faith are non-harmful
/// SingleActor too but don't help a dying ally. Buffs go through their
/// own pipeline entry.
fn try_support_heal(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    let heal_actions: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            !a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && is_healing_action(a.name())
        })
        .copied()
        .collect();
    if heal_actions.is_empty() {
        return None;
    }

    // Sort actor ids for deterministic tiebreak.
    let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
    ids.sort_unstable();

    // (priority, hp, aei): lower priority value = more urgent.
    // 0 = dying, 1 = wounded combat-active.
    let mut best: Option<(u8, u32, ActionExecutionInfo)> = None;
    for ally_id in ids {
        if ally_id == actor_id {
            continue;
        }
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team {
            continue;
        }

        let priority = if ally.is_dying() {
            0u8
        } else if ally.is_combat_active()
            && (ally.hitpoints() as f32) / (ally.max_hitpoints().max(1) as f32) < 0.5
        {
            1u8
        } else {
            continue; // healthy or stable — skip
        };

        for &heal in &heal_actions {
            let aei =
                ActionExecutionInfo::new(heal, actor_id, Some(vec![ally_id]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let hp = ally.hitpoints();
            let pick = match &best {
                None => true,
                Some((best_pri, best_hp, _)) => {
                    priority < *best_pri || (priority == *best_pri && hp < *best_hp)
                }
            };
            if pick {
                best = Some((priority, hp, aei));
            }
        }
    }

    best.map(|(_, _, aei)| aei)
}

/// Cast a buff (Bless, Shield of Faith) on a combat-active ally who
/// doesn't already have it. Skipped if we're concentrating — buffs are
/// concentration spells and a fresh cast would drop the prior one.
/// Picks the lowest-id valid (target, action) pair for determinism;
/// allies that already have the relevant condition are skipped.
fn try_buff_ally(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let buff_actions: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            !a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && is_buff_action(a.name())
        })
        .copied()
        .collect();
    if buff_actions.is_empty() {
        return None;
    }
    let my_team = actor.team();

    let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
    ids.sort_unstable();

    for ally_id in ids {
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team || !ally.is_combat_active() {
            continue;
        }
        for &buff in &buff_actions {
            // Skip buffs whose effect already sits on the ally — Bless
            // when blessed, Shield of Faith when shielded.
            let already = match buff.name() {
                "bless" => ally.has_condition(Condition::Blessed),
                "shield of faith" => ally.has_condition(Condition::Shielded),
                _ => false,
            };
            if already {
                continue;
            }
            let aei = ActionExecutionInfo::new(buff, actor_id, Some(vec![ally_id]), None, None);
            if aei.validate(encounter) {
                return Some(aei);
            }
        }
    }
    None
}

/// Try to fire a Burst-schema action centered on a tile that hits as many
/// enemies as possible without catching any allies. Candidate tiles are
/// every combat-active enemy's location (we don't sweep the full map —
/// the optimum is always near an enemy footprint). Picks the tile with
/// the highest enemy-hit count, ties broken by lower target-id of the
/// "anchor" enemy for determinism. Returns None if no Burst action exists,
/// or no point hits 2+ enemies cleanly.
fn try_attack_aoe(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Find Burst actions we own. Most actors have none — bail early.
    let burst_actions: Vec<(&'static (dyn Action + Send + Sync), isize)> = actor
        .actions
        .iter()
        .filter_map(|a| {
            if !a.is_harmful() {
                return None;
            }
            match a.targeting_schema() {
                TargetingSchema::Burst { radius } => Some((*a, radius)),
                _ => None,
            }
        })
        .collect();
    if burst_actions.is_empty() {
        return None;
    }

    // Iterate enemies in id order for deterministic tie-break.
    let mut anchor_ids: Vec<usize> = encounter.actors.keys().copied().collect();
    anchor_ids.sort_unstable();

    let mut best: Option<(usize, usize, ActionExecutionInfo)> = None; // (enemy_hits, anchor_id, aei)
    for anchor_id in &anchor_ids {
        let Some(anchor) = encounter.actors.get(anchor_id) else {
            continue;
        };
        if anchor.team() == my_team || !anchor.is_combat_active() {
            continue;
        }
        let point = anchor.location();

        for (action, radius) in &burst_actions {
            // Validate caster→point reach + LOS + cost via the action's
            // own validation (avoids reimplementing).
            let aei =
                ActionExecutionInfo::new(*action, actor_id, None, Some(vec![point]), None);
            if !aei.validate(encounter) {
                continue;
            }

            // Count combat-active actors in the radius. Friendly fire
            // disqualifies the candidate entirely — we don't damage our
            // own side. Self also counts as an ally.
            let mut enemy_hits = 0usize;
            let mut friendly_fire = false;
            for (id, a) in encounter.actors.iter() {
                if !a.is_combat_active() {
                    continue;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist > *radius {
                    continue;
                }
                if *id == actor_id || a.team() == my_team {
                    friendly_fire = true;
                    break;
                }
                enemy_hits += 1;
            }
            if friendly_fire || enemy_hits < 2 {
                continue;
            }
            let pick = match &best {
                None => true,
                Some((best_hits, best_anchor, _)) => {
                    enemy_hits > *best_hits
                        || (enemy_hits == *best_hits && *anchor_id < *best_anchor)
                }
            };
            if pick {
                best = Some((enemy_hits, *anchor_id, aei));
            }
        }
    }
    best.map(|(_, _, aei)| aei)
}

/// Find the (target, action) pair where the target has the lowest current
/// HP among combat-active enemies AND we can validly hit them right now.
/// Ties on HP break by attack reach (prefer longer-reach action) so we use
/// our better tools when offered the choice.
fn try_attack_focus_fire(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Iterate actors by sorted id for determinism — HashMap iteration
    // order changes between process runs and would make the AI's
    // tiebreakers nondeterministic given the same seed.
    let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
    ids.sort_unstable();

    // Sort key: (mode_pri, target_hp, -reach). Lower wins:
    //   - mode_pri (advantage=0, normal=1, disadvantage=2): fish for
    //     advantage opportunities first.
    //   - HP ascending: focus-fire wounded.
    //   - Reach descending: prefer the longest-reach action when tied
    //     (so a longbow gets used over a one-tile melee on a far target,
    //     etc.).
    let mut best: Option<(u8, u32, isize, ActionExecutionInfo)> = None;
    for target_id in ids {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        let Some((reach, action)) = best_attack_against(actor, encounter, target_id) else {
            continue;
        };
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let is_melee = reach <= MELEE_REACH;
        let mode = encounter.compute_attack_mode(actor_id, target_id, is_melee);
        let mode_pri = mode_priority(mode);
        let hp = target.hitpoints();
        let pick = match &best {
            None => true,
            Some((bm, bh, br, _)) => {
                (mode_pri, hp, std::cmp::Reverse(reach))
                    < (*bm, *bh, std::cmp::Reverse(*br))
            }
        };
        if pick {
            best = Some((mode_pri, hp, reach, aei));
        }
    }
    best.map(|(_, _, _, aei)| aei)
}

/// Among the actor's SingleActor actions, the longest-reach one whose
/// reach covers the current footprint distance to `target_id`. Doesn't
/// validate cost / LOS; the caller wraps it in `ActionExecutionInfo` and
/// validates.
fn best_attack_against(
    actor: &crate::actors::actor_template::ActorInstance,
    encounter: &EncounterInstance,
    target_id: usize,
) -> Option<(isize, &'static (dyn Action + Send + Sync))> {
    let target = encounter.actors.get(&target_id)?;
    let dist = footprint_chebyshev(
        actor.location(),
        get_tiles_from_size(actor.size()),
        target.location(),
        get_tiles_from_size(target.size()),
    );
    let mut best: Option<(isize, &(dyn Action + Send + Sync))> = None;
    for &action in &actor.actions {
        if !matches!(action.targeting_schema(), TargetingSchema::SingleActor) {
            continue;
        }
        // Skip helpful actions (heals, buffs) — focus-fire only considers
        // attacks. Otherwise the AI would happily Healing-Word an enemy.
        if !action.is_harmful() {
            continue;
        }
        let Some(reach) = action.reach_tiles() else {
            continue;
        };
        if dist > reach {
            continue;
        }
        if best.is_some_and(|(r, _)| r >= reach) {
            continue;
        }
        best = Some((reach, action));
    }
    best
}

/// BFS-step toward the lowest-HP visible enemy. Falls back to step toward
/// any enemy if HP-based selection fails for some reason.
fn try_step_toward_lowest_hp(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let move_action = actor
        .actions
        .iter()
        .find(|a| a.name() == "move")
        .copied()?;
    // Sort by id to break HP ties deterministically (HashMap iteration is
    // non-deterministic across processes).
    let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
    ids.sort_unstable();
    let target_id = ids
        .into_iter()
        .filter_map(|id| {
            let t = encounter.actors.get(&id)?;
            if id == actor_id || t.team() == my_team || !t.is_combat_active() {
                None
            } else {
                Some((id, t.hitpoints()))
            }
        })
        .min_by_key(|(_, hp)| *hp)?
        .0;
    let dest = encounter.step_toward_actor(actor_id, target_id)?;
    let aei = ActionExecutionInfo::new(move_action, actor_id, None, Some(vec![dest]), None);
    if aei.validate(encounter) {
        Some(aei)
    } else {
        None
    }
}

/// Last-resort: invoke the actor's Skip action so the turn advances. If
/// the actor somehow has no Skip in their action list, fall back to
/// AwaitInput to avoid an infinite engine loop.
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

use crate::engine::types::Coordinate;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
    use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
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
            pc_template: None,
            start_team: 0,
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

    /// Build a no-actors encounter we can hand-place creatures into.
    fn empty_arena() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap()
    }

    #[test]
    fn focus_fire_picks_wounded_target() {
        let mut e = empty_arena();
        // Attacker on team 0; both enemies on team 1 in melee reach.
        let attacker = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let healthy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let wounded = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 7), 1, 1)
            .unwrap();
        // Knock the second target down to 1 HP.
        let max = e.actors[&wounded].max_hitpoints();
        e.actors.get_mut(&wounded).unwrap().take_damage(max - 1);

        let ai = SimpleAi;
        let decision = ai.decide(&e, attacker);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(wounded), "AI should focus the wounded target");
        let _ = healthy;
    }

    #[test]
    fn ai_heals_wounded_ally_over_attacking() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + wounded cleric ally; enemy zombie far enough that no
        // melee threat (so kite doesn't pre-empt) but in Sacred Flame range
        // (so attack would validate). Heal-tactic should win the priority.
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let wounded = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(7, 5), 0, 1)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 14), 1, 0)
            .unwrap();
        // Drop the ally below 50% HP.
        let max = e.actors[&wounded].max_hitpoints();
        e.actors.get_mut(&wounded).unwrap().take_damage(max - 1);

        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "healing word");
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(wounded));
    }

    #[test]
    fn ai_revives_dying_ally_first() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;

        let mut e = empty_arena();
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Use Fighter for the dying ally — only PCs enter the dying state;
        // a downed Cleric would just die.
        let dying_ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        let just_wounded = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 7), 0, 2)
            .unwrap();
        // dying_ally drops to 0 HP (dying); wounded only loses half.
        let dying_max = e.actors[&dying_ally].max_hitpoints();
        e.actors.get_mut(&dying_ally).unwrap().take_damage(dying_max);
        let wounded_max = e.actors[&just_wounded].max_hitpoints();
        e.actors
            .get_mut(&just_wounded)
            .unwrap()
            .take_damage(wounded_max / 2 + 1);

        assert!(e.actors[&dying_ally].is_dying());
        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected a heal");
        };
        assert_eq!(aei.action().name(), "healing word");
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(dying_ally), "dying ally should win priority");
    }

    #[test]
    fn ai_doesnt_heal_full_hp_ally() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + healthy ally + nearby enemy. AI should attack, not heal.
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _ally = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(7, 5), 0, 1)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "healing word",
            "no ally needs healing"
        );
    }

    #[test]
    fn ai_picks_aoe_when_two_enemies_clustered() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric on team 0; two enemies tightly clustered on team 1, no
        // allies near them. Pre-set the cleric's concentration so Hold
        // Person (higher priority than AoE) is gated out — this test is
        // specifically about the AoE-vs-single-target choice.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 10), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData {
                spell_name: "Placeholder".to_string(),
                conditions: vec![],
            });

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(
            aei.action().name(),
            "sacred burst",
            "two-enemy cluster should pull AoE over single-target"
        );
    }

    #[test]
    fn ai_avoids_aoe_with_friendly_fire() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + ally clustered with two enemies — any radius-3 burst
        // catches the ally too. AI should fall back to single-target.
        // Pre-set concentration to gate out Hold Person.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _ally = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(10, 10), 0, 1)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 10), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData {
                spell_name: "Placeholder".to_string(),
                conditions: vec![],
            });

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "sacred burst",
            "any burst would clip the ally — AI should pick single-target"
        );
    }

    #[test]
    fn ai_stands_up_when_prone() {
        use crate::conditions::ConditionTimer;

        let mut e = empty_arena();
        let actor = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        e.actors
            .get_mut(&actor)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);

        let ai = SimpleAi;
        let decision = ai.decide(&e, actor);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "stand");
    }

    #[test]
    fn ai_casts_hold_person_when_available() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Two enemies: a low-HP zombie (would be focus-fire pick) and a
        // high-HP zombie (Hold Person target). Hold should beat single-
        // target attack in priority since it's a bigger lockdown.
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 5), 1, 1)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(
            aei.action().name(),
            "hold person",
            "cleric with Hold Person should cast it on a tough target"
        );
    }

    #[test]
    fn ai_does_not_recast_concentration() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 5), 1, 1)
            .unwrap();
        // Pretend the cleric is already concentrating — Hold should be
        // skipped and the AI should fall through to attack tactics.
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData {
                spell_name: "Bless".to_string(),
                conditions: vec![],
            });

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "hold person",
            "AI should not replace existing concentration"
        );
    }

    #[test]
    fn ai_focus_fire_prefers_advantage_target() {
        use crate::conditions::ConditionTimer;

        let mut e = empty_arena();
        // Attacker on team 0; two enemies in melee reach. One is healthy,
        // one is healthy AND prone. The prone one gives melee advantage.
        // Focus-fire should pick the prone target despite equal HP.
        let attacker = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let upright = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let prone = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 7), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&prone)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);

        let ai = SimpleAi;
        let decision = ai.decide(&e, attacker);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(
            target,
            Some(prone),
            "AI should fish for advantage when HP ties"
        );
        let _ = upright;
    }

    #[test]
    fn ai_does_not_heal_enemy() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric (has Healing Word + Sacred Flame) on team 0; lone enemy
        // skeleton on team 1 within Sacred Flame's range. The AI must
        // pick Sacred Flame, not Healing Word, against the enemy.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&SKELETON_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        assert_ne!(
            aei.action().name(),
            "healing word",
            "AI should not target an enemy with a heal"
        );
    }

    #[test]
    fn ai_buffs_unbuffed_ally_when_no_enemy_to_lock() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        // Place an enemy that's already stunned so Hold Person is skipped
        // (it filters out already-stunned). No other lockdown candidates.
        let enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        e.actors.get_mut(&enemy).unwrap().add_condition(
            crate::conditions::Condition::Stunned,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        // Pre-burn the cleric's level-2 slots so Hold Person is unaffordable.
        let mgr = &mut e.actors.get_mut(&cleric).unwrap().spell_slot_manager;
        assert!(mgr.consume_spell_slot(2));
        assert!(mgr.consume_spell_slot(2));

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected a buff action");
        };
        assert!(
            aei.action().name() == "bless" || aei.action().name() == "shield of faith",
            "cleric with idle resources should buff an ally; got {}",
            aei.action().name()
        );
        let _ = ConcentrationData {
            spell_name: String::new(),
            conditions: vec![],
        };
    }

    #[test]
    fn ai_does_not_rebuff_already_buffed_ally() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        // The only ally already has Bless and Shielded — try_buff_ally
        // should return None and the AI should fall through to attacks.
        e.actors
            .get_mut(&ally)
            .unwrap()
            .add_condition(Condition::Blessed, ConditionTimer::Rounds(10));
        e.actors
            .get_mut(&ally)
            .unwrap()
            .add_condition(Condition::Shielded, ConditionTimer::Rounds(10));
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(aei.action().name(), "bless");
        assert_ne!(aei.action().name(), "shield of faith");
    }

    #[test]
    fn ranged_attacker_kites_when_threatened() {
        let mut e = empty_arena();
        // Skeleton (longbow only) on team 0, zombie (melee multislam) in
        // melee reach on team 1. Skeleton should step away first.
        let skeleton = e
            .instantiate_creature(&SKELETON_TEMPLATE, Coordinate::new(10, 5), 0, 0)
            .unwrap();
        let _zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, skeleton);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected a kite step or attack");
        };
        // Confirm it's a Move, not the longbow.
        assert_eq!(aei.action().name(), "move", "skeleton should kite first");
    }
}
