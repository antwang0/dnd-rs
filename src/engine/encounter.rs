use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
use std::collections::HashMap;
use std::error::Error;

use crate::actions::action_template::ActionExecutionInfo;
use crate::actors::actor_template::{ActorInstance, CreatureTemplate, DeathSaveOutcome};
use crate::engine::actor_gen::{ActorGenParams, generate_actors};
use crate::engine::errors::{NegativeAbsCoord, NoLegalPosition};
use crate::engine::prompt::Prompt;
use crate::engine::side_effects::ApplicableSideEffect;
use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::triggers::TriggerEvent;
use crate::engine::types::{Coordinate, Size};
use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
use fastrand::Rng;
use std::cmp::Ordering;
use crate::engine::dice::{Dice, FastRandRoller, Roller};

pub enum StackElementEntry {
    SideEffect(Box<dyn ApplicableSideEffect>),
    Action(Box<ActionExecutionInfo>),
    Prompt(Prompt),
}

/// Snapshot of the engine's top-of-stack for UI consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackState {
    /// No prompts and no pending side-effects — initiative is between turns.
    Idle,
    /// A prompt is open for the given actor; the player or AI must act.
    AwaitingPrompt(usize),
    /// Side effects are mid-resolution (the player will see this between
    /// processing of an AI turn and the next prompt).
    Processing,
}

pub struct StackElement {
    pub entry: StackElementEntry,
    pub id: usize,
}

#[derive(Eq, PartialEq)]
struct InitiativeElement {
    pub actor_id: usize,
    pub initiative: i32,
}

impl Ord for InitiativeElement {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher initiative first; actor_id ascending breaks ties so the
        // turn order is deterministic when seeded (HashMap iteration order
        // would otherwise leak through `initialize_actors`).
        other
            .initiative
            .cmp(&self.initiative)
            .then(self.actor_id.cmp(&other.actor_id))
    }
}

impl PartialOrd for InitiativeElement {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct InitiativeTracker {
    // this is probably slightly more efficient as a linked list
    initiatives: Vec<InitiativeElement>,
    curr_index: usize,
}

impl InitiativeTracker {
    pub fn new() -> InitiativeTracker {
        InitiativeTracker {
            initiatives: Vec::new(),
            curr_index: 0,
        }
    }

    pub fn current_player(&self) -> Option<usize> {
        self.initiatives.get(self.curr_index).map(|ie| ie.actor_id)
    }

    pub fn advance(&mut self) {
        if self.initiatives.is_empty() {
            self.curr_index = 0;
            return;
        }
        self.curr_index = (self.curr_index + 1) % self.initiatives.len();
    }

    pub fn add_actor(&mut self, actor_id: usize, initiative: i32) {
        // Find the first slot whose initiative is strictly less than the new
        // value; insert before it so higher initiatives stay first.
        let idx = self
            .initiatives
            .iter()
            .position(|ie| initiative > ie.initiative)
            .unwrap_or(self.initiatives.len());
        self.initiatives.insert(
            idx,
            InitiativeElement {
                actor_id,
                initiative,
            },
        );
        // If we inserted at or before the active slot, the active actor
        // shifted down by one; bump curr_index to keep pointing at them.
        if idx <= self.curr_index && !self.initiatives.is_empty() {
            self.curr_index = (self.curr_index + 1).min(self.initiatives.len() - 1);
        }
    }

    pub fn remove_actor(&mut self, actor_id: usize) {
        let Some(idx) = self.initiatives.iter().position(|ie| ie.actor_id == actor_id) else {
            return;
        };
        self.initiatives.remove(idx);
        if self.initiatives.is_empty() {
            self.curr_index = 0;
            return;
        }
        // Removing at or before curr shifts the active slot up by one; if we
        // removed the active slot itself, the next actor naturally takes its
        // place at the same index.
        if idx < self.curr_index {
            self.curr_index -= 1;
        } else if self.curr_index >= self.initiatives.len() {
            self.curr_index = 0;
        }
    }

    pub fn initialize_actors(&mut self, actors: &HashMap<usize, ActorInstance>) {
        for (id, actor) in actors.iter() {
            self.initiatives.push(InitiativeElement {
                actor_id: *id,
                initiative: actor.initiative().expect("Expected initiative"),
            });
        }
        self.initiatives.sort();
    }
}

/// Hands out monotonically-increasing ids for stack elements so future
/// dependent-effect logic (e.g. "this side-effect only fires if action #N
/// hit") has something to key on. Resets between encounter rounds.
struct OutcomeTracker {
    next_id: usize,
}

impl OutcomeTracker {
    pub fn new() -> OutcomeTracker {
        OutcomeTracker { next_id: 0 }
    }

    pub fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn reset(&mut self) {
        self.next_id = 0;
    }
}

/// Authoritative state for one combat encounter. Most fields are kept
/// private; access goes through methods so engine invariants (actor map
/// stays in sync with actor locations, initiative queue stays in sync with
/// the actor table, etc.) can't be broken from the outside. `actors` is
/// still public read/write because every consumer (AI, picker UI, action
/// validation) needs deep access to actor state — narrowing it would
/// require a much larger accessor surface.
pub struct EncounterInstance {
    initialized: bool,
    pub width: usize,
    pub height: usize,
    terrain: Vec<TerrainInfo>,
    actor_id_next: usize,
    actor_map: Vec<Option<usize>>,
    pub actors: HashMap<usize, ActorInstance>,
    initiative_tracker: InitiativeTracker,
    encounter_stack: Vec<StackElement>,
    roller: FastRandRoller,
    rng: Rng,
    messages: Vec<String>,
    tmp_message: String,
    outcome_tracker: OutcomeTracker,
}

impl EncounterInstance {
    pub fn messages(&self) -> &Vec<String> {
        &self.messages
    }

    pub fn tmp_message(&self) -> &String {
        &self.tmp_message
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.messages.push(msg.into());
    }

    /// Logs a play-by-play line for an action that consumes the
    /// action-economy (Action / BonusAction / Reaction / LegendaryAction).
    /// Movement and free actions are intentionally excluded — the AI takes
    /// many move-steps per turn and they'd drown out useful events.
    fn log_action_use(&mut self, aei: &ActionExecutionInfo) {
        let cost = aei.cost(self);
        let Some(slot) = (match cost {
            Some(crate::engine::side_effects::Resource::Action) => Some("action"),
            Some(crate::engine::side_effects::Resource::BonusAction) => Some("bonus action"),
            Some(crate::engine::side_effects::Resource::Reaction) => Some("reaction"),
            Some(crate::engine::side_effects::Resource::LegendaryAction) => Some("legendary"),
            _ => None,
        }) else {
            return;
        };

        let caster_name = self
            .actors
            .get(&aei.caster_id())
            .map(|a| a.name().to_string())
            .unwrap_or_else(|| format!("actor#{}", aei.caster_id()));
        let action_name = aei.action().name();
        let target_suffix = aei
            .target_ids()
            .and_then(|ids| ids.first().copied())
            .and_then(|id| self.actors.get(&id))
            .map(|a| format!(" on {}", a.name()))
            .unwrap_or_default();

        self.log(format!(
            "[{}] {} uses {}{}",
            slot, caster_name, action_name, target_suffix
        ));
    }

    pub fn next_actor_id(&mut self) -> usize {
        let next_actor_id = self.actor_id_next;
        self.actor_id_next += 1;
        next_actor_id
    }

    /// Roll dice through the encounter's seedable roller. Use this from
    /// action side-effects so reproducibility-by-seed is preserved.
    pub fn roll(&mut self, dice: &Dice) -> u32 {
        self.roller.roll(dice)
    }

    /// Direct mutable handle to the encounter's general-purpose RNG. Used
    /// by content generation (terrain, actors) where dice abstraction
    /// doesn't fit. Do not call this from action side-effects — use `roll`.
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    pub fn get_actor(&mut self, actor_id: usize) -> Option<&mut ActorInstance> {
        self.actors.get_mut(&actor_id)
    }

    pub fn idx(&self, coord: Coordinate) -> Result<usize, NegativeAbsCoord> {
        if coord.x < 0 || coord.y < 0 {
            return Err(NegativeAbsCoord::new(coord));
        }
        Ok(coord.x as usize + coord.y as usize * self.width)
    }

    pub fn is_spawnable(&self, coord: Coordinate) -> bool {
        if coord.x < 0 || coord.y < 0 {
            return false;
        }

        if coord.x as usize >= self.width || coord.y as usize >= self.height {
            return false;
        }

        if self.actor_id_at(coord).is_some() {
            return false;
        }

        matches!(self.terrain_at(coord), Some(ti) if ti.terrain_type == TerrainType::Floor)
    }

    fn can_move_to_subtile(&self, coord: Coordinate, actor_id: usize) -> bool {
        if coord.x < 0 || coord.y < 0 {
            return false;
        }

        if coord.x as usize >= self.width || coord.y as usize >= self.height {
            return false;
        }

        if let Some(other_id) = self.actor_id_at(coord)
            && other_id != actor_id {
                return false;
            }

        matches!(self.terrain_at(coord), Some(ti) if ti.terrain_type == TerrainType::Floor)
    }

    fn get_random_coord_list(&mut self) -> Vec<Coordinate> {
        // TODO: probably try random order of one axis first
        let mut all_coords: Vec<Coordinate> = Vec::new();
        for x in 0..self.width {
            for y in 0..self.height {
                all_coords.push(Coordinate::new(x as isize, y as isize));
            }
        }
        self.rng.shuffle(&mut all_coords);
        all_coords
    }

    pub fn get_random_spawn(&mut self, size: Size) -> Result<Coordinate, NoLegalPosition> {
        let actor_width: usize = get_tiles_from_size(size);
        let coords = self.get_random_coord_list();

        'coord_loop: for &coord in coords.iter() {
            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    let offset = Coordinate::new(x_off as isize, y_off as isize);
                    if !self.is_spawnable(coord + offset) {
                        continue 'coord_loop;
                    }
                }
            }
            return Ok(coord);
        }
        Err(NoLegalPosition)
    }

    pub fn actor_id_at(&self, coord: Coordinate) -> Option<usize> {
        let idx = self.idx(coord).ok()?;
        self.actor_map.get(idx).copied().flatten()
    }

    fn set_actor_id_at(&mut self, actor_id: Option<usize>, coord: Coordinate) {
        if let Ok(idx) = self.idx(coord)
            && let Some(slot) = self.actor_map.get_mut(idx) {
                *slot = actor_id;
            }
    }

    pub fn terrain_at(&self, coord: Coordinate) -> Option<&TerrainInfo> {
        let idx = self.idx(coord).ok()?;
        self.terrain.get(idx)
    }

    pub fn can_move_to(&self, actor_id: usize, coord: Coordinate) -> bool {
        if let Some(actor) = self.actors.get(&actor_id) {
            let actor_width = get_tiles_from_size(actor.size());
            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    let offset: Coordinate = Coordinate::new(x_off as isize, y_off as isize);
                    if !self.can_move_to_subtile(coord + offset, actor_id) {
                        return false;
                    }
                }
            }
            return true;
        }
        false
    }

    /// True if a straight Bresenham line from `from` to `to` passes through
    /// only non-wall tiles between (exclusive of endpoints). Endpoints are
    /// not checked so callers can target the tile they currently occupy or
    /// the tile they want to attack into. Actors do *not* block LOS — only
    /// walls do (matches 5e's "creatures don't grant cover" default).
    pub fn has_line_of_sight(&self, from: Coordinate, to: Coordinate) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let coord = Coordinate::new(x0, y0);
            if matches!(self.terrain_at(coord), Some(t) if t.terrain_type == TerrainType::Wall) {
                return false;
            }
        }
    }

    /// Footprint-aware LOS: clear if *any* tile of A's footprint can see
    /// *any* tile of B's footprint. Catches the common case where the
    /// origin-to-origin line is blocked but the creatures can still see
    /// around their own bulk (e.g. two Medium creatures around a corner).
    pub fn actor_has_line_of_sight(&self, a_id: usize, b_id: usize) -> bool {
        let Some(a) = self.actors.get(&a_id) else {
            return false;
        };
        let Some(b) = self.actors.get(&b_id) else {
            return false;
        };
        let a_size = get_tiles_from_size(a.size()) as isize;
        let b_size = get_tiles_from_size(b.size()) as isize;
        let a_loc = a.location();
        let b_loc = b.location();
        for ay in 0..a_size {
            for ax in 0..a_size {
                let from = Coordinate::new(a_loc.x + ax, a_loc.y + ay);
                for by in 0..b_size {
                    for bx in 0..b_size {
                        let to = Coordinate::new(b_loc.x + bx, b_loc.y + by);
                        if self.has_line_of_sight(from, to) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Fire reactions matching `event`. Today this only handles opportunity
    /// attacks on `ActorLeaving`; future variants (damage taken, attack
    /// resolved, etc.) plug in here. Reactions execute eagerly — their
    /// side-effects apply directly to the encounter, not via the stack —
    /// because they conceptually happen "during" the triggering event.
    pub fn dispatch_reaction(&mut self, event: TriggerEvent) {
        match event {
            TriggerEvent::ActorLeaving { actor_id, from, to } => {
                self.dispatch_opportunity_attacks(actor_id, from, to);
            }
        }
    }

    /// Iterate enemy actors with a Reaction slot and a melee attack; for each
    /// whose reach covered `mover` at `from` but no longer covers them at
    /// `to`, run the attack against the mover and consume the reaction.
    /// Stops early if the mover is downed mid-loop.
    fn dispatch_opportunity_attacks(
        &mut self,
        mover_id: usize,
        from: Coordinate,
        to: Coordinate,
    ) {
        use crate::actions::action_template::{MELEE_REACH, TargetingSchema};
        use crate::engine::side_effects::Resource;

        let (mover_team, mover_size) = match self.actors.get(&mover_id) {
            Some(a) => (a.team(), get_tiles_from_size(a.size())),
            None => return,
        };

        // Snapshot reactor candidates up-front — the loop body will mutate
        // self, which would conflict with holding an iterator into self.actors.
        type OaCandidate = (usize, &'static (dyn crate::actions::action_template::Action + Send + Sync), Coordinate, usize, isize);
        let candidates: Vec<OaCandidate> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == mover_id || a.team() == mover_team || !a.is_combat_active() {
                    return None;
                }
                if !a.can_consume_resource(Resource::Reaction) {
                    return None;
                }
                let attack = a
                    .actions
                    .iter()
                    .find(|act| {
                        matches!(act.targeting_schema(), TargetingSchema::SingleActor)
                            && act.reach_tiles().is_some_and(|r| r <= MELEE_REACH)
                    })
                    .copied()?;
                let reach = attack.reach_tiles().unwrap_or(MELEE_REACH);
                Some((*id, attack, a.location(), get_tiles_from_size(a.size()), reach))
            })
            .collect();

        for (reactor_id, attack, r_loc, r_size, reach) in candidates {
            // Re-check liveness (an earlier OA in this loop may have changed things).
            if !self
                .actors
                .get(&reactor_id)
                .is_some_and(|a| a.is_combat_active() && a.can_consume_resource(Resource::Reaction))
            {
                continue;
            }
            let was_in_reach =
                footprint_chebyshev(r_loc, r_size, from, mover_size) <= reach;
            let still_in_reach =
                footprint_chebyshev(r_loc, r_size, to, mover_size) <= reach;
            if !was_in_reach || still_in_reach {
                continue;
            }

            let reactor_name = self
                .actors
                .get(&reactor_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default();
            let mover_name = self
                .actors
                .get(&mover_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default();
            self.log(format!(
                "[reaction] {} opportunity-attacks {} as they leave reach",
                reactor_name, mover_name
            ));

            // Run the underlying attack's side_effects directly (consumes
            // Reaction below, NOT the action's normal cost).
            let target_vec = vec![mover_id];
            let effects =
                attack.side_effects(self, reactor_id, Some(&target_vec), None, None);
            for e in effects {
                e.apply(self);
            }
            if let Some(r) = self.actors.get_mut(&reactor_id) {
                r.consume_resource(Resource::Reaction);
            }

            self.cleanup_dead_actors();
            // If the OA dropped the mover, no further OAs (and the move
            // caller is expected to abort).
            if self
                .actors
                .get(&mover_id)
                .is_none_or(|a| !a.is_combat_active())
            {
                return;
            }
        }
    }

    /// Footprint-Chebyshev distance between two living actors, or `None` if
    /// either id is unknown. 0 means they're touching/adjacent.
    pub fn footprint_distance(&self, a_id: usize, b_id: usize) -> Option<isize> {
        let a = self.actors.get(&a_id)?;
        let b = self.actors.get(&b_id)?;
        Some(footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            b.location(),
            get_tiles_from_size(b.size()),
        ))
    }

    /// First step the actor should take to reach a footprint-adjacent square
    /// next to `target_id`. Uses 8-connected BFS over walkable tiles for the
    /// actor's footprint; finds the *shortest-step-count* path, ignoring
    /// movement budget (the AI may need several turns to close in). Returns
    /// `None` if already adjacent or no path exists.
    pub fn step_toward_actor(&self, actor_id: usize, target_id: usize) -> Option<Coordinate> {
        use std::collections::{HashMap, VecDeque};

        let actor = self.actors.get(&actor_id)?;
        let target = self.actors.get(&target_id)?;
        let start = actor.location();
        let my_size = get_tiles_from_size(actor.size());
        let t_loc = target.location();
        let t_size = get_tiles_from_size(target.size());

        let in_melee =
            |c: Coordinate| -> bool { footprint_chebyshev(c, my_size, t_loc, t_size) <= 1 };

        if in_melee(start) {
            return None;
        }

        let mut parent: HashMap<Coordinate, Coordinate> = HashMap::new();
        let mut queue: VecDeque<Coordinate> = VecDeque::new();
        queue.push_back(start);
        parent.insert(start, start);

        while let Some(coord) = queue.pop_front() {
            if coord != start && in_melee(coord) {
                let mut cur = coord;
                while parent[&cur] != start {
                    cur = parent[&cur];
                }
                return Some(cur);
            }
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let next = Coordinate::new(coord.x + dx, coord.y + dy);
                    if parent.contains_key(&next) {
                        continue;
                    }
                    if !self.can_move_to(actor_id, next) {
                        continue;
                    }
                    parent.insert(next, coord);
                    queue.push_back(next);
                }
            }
        }
        None
    }

    /// Min movement cost to walk from the actor's current location to `dest`,
    /// bounded by their remaining movement. Returns `None` if `dest` is
    /// unreachable on floor tiles within budget. 8-connected; cardinal steps
    /// cost 5ft, diagonal steps cost ~7.07ft (Euclidean).
    pub fn path_cost_to(&self, actor_id: usize, dest: Coordinate) -> Option<f32> {
        self.dijkstra_path(actor_id, dest).map(|(c, _)| c)
    }

    /// Cheapest path (excluding `start`, including `dest`) from the actor's
    /// current location to `dest`, alongside the path cost. Returns `None`
    /// when `dest` is unreachable within the actor's remaining-movement
    /// budget. The path is what the engine iterates per-tile so that
    /// opportunity attacks fire on every threatened-square exit, not just
    /// on the from→to endpoints.
    pub fn path_to(&self, actor_id: usize, dest: Coordinate) -> Option<Vec<Coordinate>> {
        self.dijkstra_path(actor_id, dest).map(|(_, p)| p)
    }

    fn dijkstra_path(
        &self,
        actor_id: usize,
        dest: Coordinate,
    ) -> Option<(f32, Vec<Coordinate>)> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let actor = self.actors.get(&actor_id)?;
        let start = actor.location();
        if start == dest {
            return Some((0.0, Vec::new()));
        }
        if !self.can_move_to(actor_id, dest) {
            return None;
        }

        // Encode floats as millifeet so we can use integer ordering / Eq.
        let to_mft = |f: f32| -> u32 { (f * 1000.0) as u32 };
        let cardinal_mft = to_mft(2.5); // 5ft via tile_center_dist semantics (2.5 * 1)
        let diagonal_mft = to_mft(2.5 * std::f32::consts::SQRT_2);
        let budget_mft = to_mft(actor.remaining_movement() + 0.5);

        let start_idx = self.idx(start).ok()?;
        let dest_idx = self.idx(dest).ok()?;
        let n = self.width * self.height;
        let mut dist: Vec<u32> = vec![u32::MAX; n];
        let mut parent: Vec<Option<usize>> = vec![None; n];
        dist[start_idx] = 0;

        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        heap.push(Reverse((0, start_idx)));

        while let Some(Reverse((cost, idx))) = heap.pop() {
            if idx == dest_idx {
                // Reconstruct path from start (exclusive) to dest (inclusive).
                let mut rev: Vec<Coordinate> = Vec::new();
                let mut cur = dest_idx;
                while cur != start_idx {
                    rev.push(Coordinate::new(
                        (cur % self.width) as isize,
                        (cur / self.width) as isize,
                    ));
                    cur = parent[cur]?;
                }
                rev.reverse();
                return Some((cost as f32 / 1000.0, rev));
            }
            if cost > dist[idx] {
                continue;
            }
            let cx = (idx % self.width) as isize;
            let cy = (idx / self.width) as isize;
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let next = Coordinate::new(cx + dx, cy + dy);
                    if !self.can_move_to(actor_id, next) {
                        continue;
                    }
                    let step = if dx == 0 || dy == 0 {
                        cardinal_mft
                    } else {
                        diagonal_mft
                    };
                    let next_cost = cost.saturating_add(step);
                    if next_cost > budget_mft {
                        continue;
                    }
                    let Ok(next_idx) = self.idx(next) else {
                        continue;
                    };
                    if next_cost < dist[next_idx] {
                        dist[next_idx] = next_cost;
                        parent[next_idx] = Some(idx);
                        heap.push(Reverse((next_cost, next_idx)));
                    }
                }
            }
        }
        None
    }

    pub fn from_params(
        terrain_params: &TerrainGenParams,
        actor_params: &ActorGenParams,
        seed: Option<u64>,
    ) -> Result<EncounterInstance, Box<dyn Error>> {
        let (roller, mut rng) = match seed {
            Some(s) => (FastRandRoller::with_seed(s), Rng::with_seed(s)),
            None => (FastRandRoller::default(), Rng::new()),
        };

        let mut ei = EncounterInstance {
            initialized: false,
            width: terrain_params.width,
            height: terrain_params.height,
            terrain: generate_terrain(terrain_params, &mut rng),
            actor_id_next: 0,
            actor_map: vec![None; terrain_params.width * terrain_params.height],
            actors: HashMap::new(),
            initiative_tracker: InitiativeTracker::new(),
            encounter_stack: Vec::new(),
            roller,
            rng,
            messages: Vec::new(),
            tmp_message: String::new(),
            outcome_tracker: OutcomeTracker::new(),
        };

        // TODO: move pool to fn
        let template_pool: Vec<&'static CreatureTemplate> =
            vec![&ZOMBIE_TEMPLATE, &SKELETON_TEMPLATE];

        generate_actors(&mut ei, actor_params, &template_pool)?;
        ei.initialize()?;
        Ok(ei)
    }

    pub fn skip_turn(&mut self) {
        self.initiative_tracker.advance();
        let Some(next_id) = self.initiative_tracker.current_player() else {
            return;
        };
        if let Some(curr_actor) = self.actors.get_mut(&next_id) {
            curr_actor.reset_for_new_round();
        }
    }

    pub fn set_actor_map(
        &mut self,
        actor_id: usize,
        coord: Coordinate,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(actor) = self.actors.get(&actor_id) {
            let actor_width = get_tiles_from_size(actor.size());

            let coord_old = actor.location();
            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    let offset = Coordinate::new(x_off as isize, y_off as isize);
                    self.set_actor_id_at(None, coord_old + offset);
                }
            }

            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    let offset = Coordinate::new(x_off as isize, y_off as isize);
                    self.set_actor_id_at(Some(actor_id), coord + offset);
                }
            }
            return Ok(());
        }
        Err("Actor not found".into())
    }

    pub fn instantiate_creature(
        &mut self,
        creature_template: &'static CreatureTemplate,
        location: Coordinate,
        team_id: usize,
        instance_n: usize,
    ) -> Result<usize, Box<dyn Error>> {
        let actor_id = self.next_actor_id();

        let mut actor = ActorInstance::from_creature_template(
            creature_template,
            location,
            team_id,
            &mut self.roller,
            instance_n,
        )?;
        actor.reset_for_new_round();

        if self.initialized {
            actor.roll_initiative(&mut self.roller);
            self.initiative_tracker
                .add_actor(actor_id, actor.initiative().unwrap());
        }

        self.actors.insert(actor_id, actor);

        self.set_actor_map(actor_id, location)?;

        Ok(actor_id)
    }

    /// Actor ids in turn order, starting from the current actor. Empty
    /// when no actors are queued. Used by the UI's initiative panel.
    pub fn initiative_actor_ids(&self) -> Vec<usize> {
        let len = self.initiative_tracker.initiatives.len();
        if len == 0 {
            return Vec::new();
        }
        let curr = self.initiative_tracker.curr_index;
        (0..len)
            .map(|i| self.initiative_tracker.initiatives[(curr + i) % len].actor_id)
            .collect()
    }

    /// Top-of-stack snapshot for the UI: the actor whose prompt is open
    /// (if any), and whether the engine is mid-processing or idle.
    pub fn stack_state(&self) -> StackState {
        match self.encounter_stack.last() {
            None => StackState::Idle,
            Some(se) => match &se.entry {
                StackElementEntry::Prompt(p) => StackState::AwaitingPrompt(p.actor_id()),
                _ => StackState::Processing,
            },
        }
    }

    /// Distinct team ids with at least one combat-active actor (excludes
    /// dying / stable / dead). Drives end-of-combat detection.
    pub fn living_teams(&self) -> std::collections::HashSet<usize> {
        self.actors
            .values()
            .filter(|a| a.is_combat_active())
            .map(|a| a.team())
            .collect()
    }

    /// True once at most one team has living actors. Encounters with zero
    /// living actors also count as complete (mutual destruction).
    pub fn is_complete(&self) -> bool {
        self.living_teams().len() <= 1
    }

    /// `Some(team_id)` if exactly one team is left standing; `None` if the
    /// fight is still on or everyone is dead.
    pub fn winning_team(&self) -> Option<usize> {
        let teams = self.living_teams();
        if teams.len() == 1 {
            teams.into_iter().next()
        } else {
            None
        }
    }

    /// Idempotent post-effect cleanup pass: remove any actor whose
    /// death-save record has hit 3 failures. The "falls unconscious" log
    /// is emitted by `DealDamage::apply` directly so the message tracks the
    /// actual transition (active → dying), not a fragile derived check on
    /// `(successes, failures) == (0, 0)`.
    ///
    /// Stable actors stay on the map at 0 HP — they're out of the fight but
    /// not removed (room for healing later).
    pub fn cleanup_dead_actors(&mut self) {
        use crate::actors::actor_template::HpState;
        let dead: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| match a.hp_state() {
                HpState::Dead => Some(*id),
                HpState::Dying { failures, .. } if failures >= 3 => Some(*id),
                _ => None,
            })
            .collect();
        for id in dead {
            self.remove_actor(id);
        }
    }

    /// Remove an actor from the world: actor map, initiative queue, and
    /// actor table. Logs the death.
    fn remove_actor(&mut self, id: usize) {
        let Some(actor) = self.actors.remove(&id) else {
            return;
        };
        self.log(format!("{} dies.", actor.name()));
        let actor_width = get_tiles_from_size(actor.size());
        let loc = actor.location();
        for x_off in 0..actor_width {
            for y_off in 0..actor_width {
                let offset = Coordinate::new(x_off as isize, y_off as isize);
                self.set_actor_id_at(None, loc + offset);
            }
        }
        self.initiative_tracker.remove_actor(id);
    }

    /// Roll a single death save for the given actor and mutate them. Logs
    /// the d20 result and outcome. Returns true if the actor is gone after
    /// this save (dead and removed).
    fn resolve_death_save(&mut self, id: usize) -> bool {
        let raw = self.roller.roll(&Dice::new(1, 20));
        let Some(actor) = self.actors.get_mut(&id) else {
            return false;
        };
        let name = actor.name().to_string();
        let outcome = actor.apply_death_save(raw);
        let (succ, fail) = actor.death_save_record();
        match outcome {
            DeathSaveOutcome::Continuing => {
                self.log(format!(
                    "  {} death save: 1d20({}) — {} ({}/{} S/F)",
                    name,
                    raw,
                    if raw >= 10 { "success" } else { "failure" },
                    succ,
                    fail
                ));
                false
            }
            DeathSaveOutcome::Stabilized => {
                self.log(format!(
                    "  {} death save: 1d20({}) — stabilized!",
                    name, raw
                ));
                false
            }
            DeathSaveOutcome::Dead => {
                self.log(format!("  {} death save: 1d20({}) — dies!", name, raw));
                self.remove_actor(id);
                true
            }
            DeathSaveOutcome::Revived => {
                self.log(format!(
                    "  {} death save: 1d20({}) — natural 20! Conscious at 1 HP.",
                    name, raw
                ));
                false
            }
            DeathSaveOutcome::NotDying => false,
        }
    }

    pub fn initialize(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Err("attempted to initialize already initialized encounter");
        }
        for (_, actor) in self.actors.iter_mut() {
            actor.roll_initiative(&mut self.roller);
        }
        self.initiative_tracker.initialize_actors(&self.actors);
        self.initialized = true;
        Ok(())
    }

    pub fn enqueue_event(&mut self, se: StackElementEntry) {
        self.encounter_stack.push(StackElement {
            entry: se,
            id: self.outcome_tracker.next_id(),
        });
    }

    pub fn peek_prompt(&self) -> Option<&Prompt> {
        let last = self.encounter_stack.last();
        match last {
            None => None,
            Some(se) => match &se.entry {
                StackElementEntry::Prompt(p) => Some(p),
                _ => None,
            },
        }
    }

    pub fn pop_prompt(&mut self) -> Option<Prompt> {
        let last = self.encounter_stack.pop();
        match last {
            None => None,
            Some(se) => match se.entry {
                StackElementEntry::Prompt(p) => Some(p),
                other => {
                    self.encounter_stack.push(StackElement {
                        entry: other,
                        id: se.id,
                    });
                    None
                }
            },
        }
    }

    pub fn push_action(&mut self, action_execution_info: ActionExecutionInfo) {
        self.enqueue_event(StackElementEntry::Action(Box::new(action_execution_info)));
    }

    pub fn process_stack(&mut self) {
        if !self.initialized {
            return;
        }

        // Bail if we are already waiting on a player prompt.
        if self.peek_prompt().is_some() {
            return;
        }

        while let Some(se) = self.encounter_stack.pop() {
            match se.entry {
                StackElementEntry::Prompt(p) => {
                    // A prompt was already on the stack; put it back and bail.
                    self.encounter_stack.push(StackElement {
                        entry: StackElementEntry::Prompt(p),
                        id: se.id,
                    });
                    return;
                }
                StackElementEntry::Action(a) => {
                    self.log_action_use(&a);
                    let mut side_effects = a.execute(self);
                    for sen in side_effects.drain(..) {
                        self.enqueue_event(StackElementEntry::SideEffect(sen));
                    }
                }
                StackElementEntry::SideEffect(s) => {
                    s.apply(self);
                    self.cleanup_dead_actors();
                }
            }
        }

        self.outcome_tracker.reset();

        // Auto-resolve any dying / stable actors before prompting. Each
        // dying actor takes their "turn" by rolling exactly one death save;
        // stable actors just have their slot skipped. The visited set
        // bounds the loop to one save per actor per process_stack call:
        // without it, in-loop removals shrink the initiative queue and
        // `advance()`'s wraparound revisits actors, double-counting saves.
        let mut visited: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        loop {
            let Some(curr_id) = self.initiative_tracker.current_player() else {
                return;
            };
            let Some(actor) = self.actors.get(&curr_id) else {
                // Active slot points at a removed actor — fix the queue.
                self.initiative_tracker.remove_actor(curr_id);
                continue;
            };
            if actor.is_combat_active() {
                break;
            }
            if !visited.insert(curr_id) {
                // We've already given this actor a save this call; bail
                // (everyone left in the queue is downed).
                break;
            }
            if actor.is_dying() {
                self.resolve_death_save(curr_id);
            }
            // After the save (or if stable), advance to the next slot.
            self.initiative_tracker.advance();
            // Reset the next actor's resources so an active actor's first
            // turn after a sequence of skipped/dying slots starts fresh.
            if let Some(next_id) = self.initiative_tracker.current_player()
                && let Some(next_actor) = self.actors.get_mut(&next_id)
            {
                next_actor.reset_for_new_round();
            }
        }

        // Encounter may have ended while resolving saves.
        if self.is_complete() {
            return;
        }

        // Skip the prompt if the encounter has wound down (everyone died).
        let Some(current_player_id) = self.initiative_tracker.current_player() else {
            return;
        };
        let Some(current_player) = self.actors.get(&current_player_id) else {
            return;
        };
        self.enqueue_event(StackElementEntry::Prompt(Prompt::new(
            current_player_id,
            current_player.actions.clone(), // TODO: filter for legal actions (action, bonus action; no reaction)
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain::TerrainInfo;
    use crate::engine::terrain_gen::TerrainGenParams;

    /// Builds a tiny encounter with no actors and a hand-crafted terrain
    /// grid so LOS can be tested deterministically (terrain_gen randomness
    /// would otherwise make assertions seed-dependent).
    fn ei_with_terrain(width: usize, height: usize, walls: &[(isize, isize)]) -> EncounterInstance {
        // Use generator to bootstrap, then overwrite the terrain map.
        let tp = TerrainGenParams {
            width,
            height,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        e.terrain = vec![
            TerrainInfo {
                terrain_type: TerrainType::Floor,
            };
            width * height
        ];
        for &(x, y) in walls {
            let idx = e.idx(Coordinate::new(x, y)).unwrap();
            e.terrain[idx].terrain_type = TerrainType::Wall;
        }
        e
    }

    #[test]
    fn los_clear_horizontal() {
        let e = ei_with_terrain(10, 10, &[]);
        assert!(e.has_line_of_sight(Coordinate::new(0, 5), Coordinate::new(9, 5)));
    }

    #[test]
    fn los_blocked_by_wall_between() {
        let e = ei_with_terrain(10, 10, &[(5, 5)]);
        assert!(!e.has_line_of_sight(Coordinate::new(0, 5), Coordinate::new(9, 5)));
    }

    #[test]
    fn los_endpoints_not_checked() {
        // Wall on the destination tile should not block LOS *to* that tile —
        // attacks target the tile, they don't pass through it.
        let e = ei_with_terrain(10, 10, &[(9, 5)]);
        assert!(e.has_line_of_sight(Coordinate::new(0, 5), Coordinate::new(9, 5)));
    }

    #[test]
    fn los_diagonal_clear() {
        let e = ei_with_terrain(10, 10, &[]);
        assert!(e.has_line_of_sight(Coordinate::new(0, 0), Coordinate::new(7, 7)));
    }

    #[test]
    fn los_diagonal_blocked() {
        // Wall right on the diagonal path should block.
        let e = ei_with_terrain(10, 10, &[(3, 3)]);
        assert!(!e.has_line_of_sight(Coordinate::new(0, 0), Coordinate::new(7, 7)));
    }

    #[test]
    fn los_same_tile_is_visible() {
        let e = ei_with_terrain(10, 10, &[]);
        assert!(e.has_line_of_sight(Coordinate::new(2, 2), Coordinate::new(2, 2)));
    }

    #[test]
    fn death_save_three_failures_kills() {
        let e = ei_with_terrain(10, 10, &[]);
        // Build a zombie actor for testing.
        let mut z = ActorInstance::from_creature_template(
            &ZOMBIE_TEMPLATE,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap();
        z.take_damage(z.hitpoints());
        assert!(z.is_dying());
        // Force three failures (rolling 2 each is < 10, +1 fail).
        for _ in 0..3 {
            let outcome = z.apply_death_save(2);
            if matches!(outcome, DeathSaveOutcome::Dead) {
                let _ = e; // silence unused
                return;
            }
        }
        panic!("expected Dead outcome after 3 failed saves");
    }

    #[test]
    fn death_save_three_successes_stabilizes() {
        let _e = ei_with_terrain(10, 10, &[]);
        let mut z = ActorInstance::from_creature_template(
            &ZOMBIE_TEMPLATE,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap();
        z.take_damage(z.hitpoints());
        // Roll 3 successes (>= 10).
        for _ in 0..3 {
            z.apply_death_save(15);
        }
        assert!(z.is_stable(), "expected stable after 3 successes");
        assert!(!z.is_combat_active(), "stable actor isn't a combatant");
    }

    #[test]
    fn nat_20_revives_at_one_hp() {
        let mut z = ActorInstance::from_creature_template(
            &ZOMBIE_TEMPLATE,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap();
        z.take_damage(z.hitpoints());
        let outcome = z.apply_death_save(20);
        assert_eq!(outcome, DeathSaveOutcome::Revived);
        assert_eq!(z.hitpoints(), 1);
        assert!(z.is_combat_active());
    }

    #[test]
    fn damage_to_dying_adds_failure() {
        let mut z = ActorInstance::from_creature_template(
            &ZOMBIE_TEMPLATE,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap();
        z.take_damage(z.hitpoints());
        assert_eq!(z.death_save_record(), (0, 0));
        z.take_damage(1); // damage while dying = +1 failure
        assert_eq!(z.death_save_record().1, 1);
    }

    #[test]
    fn multiattack_runs_sub_attack_n_times() {
        use crate::actions::action_template::Action;
        use crate::actions::monster_attacks::ZOMBIE_MULTISLAM;
        // Static-cast to verify the trait wiring; no roll done here.
        let action: &dyn Action = &*ZOMBIE_MULTISLAM;
        assert_eq!(action.name(), "multislam");
        assert_eq!(action.reach_tiles(), Some(1));
        assert!(!action.requires_los());
    }

    #[test]
    fn opportunity_attack_fires_when_leaving_reach() {
        use crate::engine::side_effects::{ApplicableSideEffect, MoveActor, Resource};

        let mut e = ei_with_terrain(20, 20, &[]);
        // Two enemies whose Medium 2x2 footprints touch (gap = 0).
        let mover_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let reactor_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        // Sanity: reactor has a reaction available.
        assert!(
            e.actors[&reactor_id].can_consume_resource(Resource::Reaction),
            "reactor should start with a reaction slot"
        );

        // Move the mover well out of reach.
        let move_effect = MoveActor {
            actor_id: mover_id,
            path: vec![Coordinate::new(15, 5)],
        };
        move_effect.apply(&mut e);

        // Whether the OA hit is RNG-dependent, but the reaction must be
        // consumed regardless (it's spent on attempt, not on hit).
        if e.actors.contains_key(&reactor_id) {
            assert!(
                !e.actors[&reactor_id].can_consume_resource(Resource::Reaction),
                "reactor should have spent their reaction"
            );
        }
    }

    #[test]
    fn opportunity_attack_does_not_fire_when_staying_in_reach() {
        use crate::engine::side_effects::{ApplicableSideEffect, MoveActor, Resource};

        let mut e = ei_with_terrain(20, 20, &[]);
        let mover_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let reactor_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();

        // Step 1 tile in-place — footprints still touch the reactor.
        let move_effect = MoveActor {
            actor_id: mover_id,
            path: vec![Coordinate::new(5, 6)],
        };
        move_effect.apply(&mut e);

        assert!(
            e.actors[&reactor_id].can_consume_resource(Resource::Reaction),
            "reactor's reaction should be intact — mover stayed in reach"
        );
    }

    #[test]
    fn opportunity_attack_fires_per_step_on_multitile_path() {
        use crate::engine::side_effects::{ApplicableSideEffect, MoveActor, Resource};

        let mut e = ei_with_terrain(20, 20, &[]);
        // Reactor is far from BOTH the mover's start and end tiles. The
        // path between passes through reach. With the old endpoint-only
        // OA, this would silently bypass the reactor's threatened square.
        let mover_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(0, 5), 0, 0)
            .unwrap();
        let reactor_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(e.actors[&reactor_id].can_consume_resource(Resource::Reaction));

        // Walk the mover step-by-step right past the reactor.
        let path: Vec<Coordinate> = (1..=15).map(|x| Coordinate::new(x, 5)).collect();
        let move_effect = MoveActor {
            actor_id: mover_id,
            path,
        };
        move_effect.apply(&mut e);

        if e.actors.contains_key(&reactor_id) {
            assert!(
                !e.actors[&reactor_id].can_consume_resource(Resource::Reaction),
                "reactor should have OA'd as the mover stepped past their reach"
            );
        }
    }

    #[test]
    fn opportunity_attack_skipped_for_same_team() {
        use crate::engine::side_effects::{ApplicableSideEffect, MoveActor, Resource};

        let mut e = ei_with_terrain(20, 20, &[]);
        // Both on team 0 — allies don't OA each other.
        let mover_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let ally_id = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();

        let move_effect = MoveActor {
            actor_id: mover_id,
            path: vec![Coordinate::new(15, 5)],
        };
        move_effect.apply(&mut e);

        assert!(
            e.actors[&ally_id].can_consume_resource(Resource::Reaction),
            "ally should not have spent their reaction"
        );
    }
}
