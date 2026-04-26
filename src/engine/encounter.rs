use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
use std::collections::{HashMap, LinkedList};
use std::error::Error;

use crate::actions::action_template::ActionExecutionInfo;
use crate::actors::actor_template::{ActorInstance, CreatureTemplate};
use crate::engine::actor_gen::{ActorGenParams, generate_actors};
use crate::engine::errors::{NegativeAbsCoord, NoLegalPosition};
use crate::engine::prompt::Prompt;
use crate::engine::side_effects::ApplicableSideEffect;
use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::triggers::TriggerEventType;
use crate::engine::types::{Coordinate, Size};
use crate::engine::util::{get_colored_span, get_tiles_from_size};
use fastrand::Rng;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};
use std::cmp::Ordering;
use tyche::dice::roller::FastRand as FastRandRoller;

/// True/false toggle that flips every ~500ms based on wall-clock time.
/// Used to manually blink UI elements; ANSI SLOW_BLINK is unreliable on
/// many terminals (notably Windows Terminal).
fn blink_on() -> bool {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    (millis / 500).is_multiple_of(2)
}

pub enum StackElementEntry {
    SideEffect(Box<dyn ApplicableSideEffect>),
    Action(Box<ActionExecutionInfo>),
    Prompt(Prompt),
}

pub struct StackElement {
    pub entry: StackElementEntry,
    pub id: usize,
    pub success_dependencies: Option<Vec<usize>>,
}

#[derive(Eq, PartialEq)]
struct InitiativeElement {
    pub actor_id: usize,
    pub initiative: i32,
}

impl Ord for InitiativeElement {
    fn cmp(&self, other: &Self) -> Ordering {
        other.initiative.cmp(&self.initiative)
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

struct OutcomeTracker {
    next_id: usize,
    successes: HashMap<usize, bool>,
}

impl OutcomeTracker {
    pub fn new() -> OutcomeTracker {
        OutcomeTracker {
            next_id: 0,
            successes: HashMap::new(),
        }
    }

    pub fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn reset(&mut self) {
        self.next_id = 0;
        self.successes.clear();
    }
}

// TODO: having all pub is not very good
pub struct EncounterInstance {
    initialized: bool,
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<TerrainInfo>,
    pub actor_id_next: usize,
    pub actor_map: Vec<Option<usize>>,
    pub actors: HashMap<usize, ActorInstance>,
    initiative_tracker: InitiativeTracker,
    pub encounter_stack: Vec<StackElement>,
    pub temp_encounter_queue: LinkedList<StackElement>, // for handling multiple reactions
    pub roller: FastRandRoller,
    pub rng: Rng,
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

    pub fn next_actor_id(&mut self) -> usize {
        let next_actor_id = self.actor_id_next;
        self.actor_id_next += 1;
        next_actor_id
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

    /// Min movement cost to walk from the actor's current location to `dest`,
    /// bounded by their remaining movement. Returns `None` if `dest` is
    /// unreachable on floor tiles within budget. 8-connected; cardinal steps
    /// cost 5ft, diagonal steps cost ~7.07ft (Euclidean).
    pub fn path_cost_to(&self, actor_id: usize, dest: Coordinate) -> Option<f32> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let actor = self.actors.get(&actor_id)?;
        let start = actor.location();
        if start == dest {
            return Some(0.0);
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
        let mut dist: Vec<u32> = vec![u32::MAX; self.width * self.height];
        dist[start_idx] = 0;

        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        heap.push(Reverse((0, start_idx)));

        while let Some(Reverse((cost, idx))) = heap.pop() {
            if idx == dest_idx {
                return Some(cost as f32 / 1000.0);
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
                        heap.push(Reverse((next_cost, next_idx)));
                    }
                }
            }
        }
        None
    }

    pub fn render_map(&self, frame: &mut Frame, area: Rect) {
        let mut text: Vec<Line> = Vec::new();

        let active_actor_id: Option<usize> = self.encounter_stack.last().and_then(|se| {
            if let StackElementEntry::Prompt(p) = &se.entry {
                Some(p.actor_id())
            } else {
                None
            }
        });

        for y in (0..self.height).rev() {
            let mut row: Vec<Span> = Vec::new();
            for x in 0..self.width {
                let coord = Coordinate::new(x as isize, y as isize);
                if let Some(actor_id) = self.actor_id_at(coord)
                    && let Some(actor) = self.actors.get(&actor_id)
                {
                    // Stale id (cleanup race between damage tick and frame draw)
                    // would otherwise crash the renderer; skip to the terrain branch.
                    let (s, c, bg): (String, Color, Color) =
                        get_colored_span(actor_id, actor.team());
                    let mut style = Style::default().fg(c).bg(bg);
                    if Some(actor_id) == active_actor_id && !blink_on() {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                    row.push(Span::styled(s, style));
                } else {
                    let s = Span::from(
                        match self.terrain_at(coord).map(|t| &t.terrain_type) {
                            Some(TerrainType::Floor) => '░',
                            Some(TerrainType::Wall) => '█',
                            _ => ' ',
                        }
                        .to_string(),
                    );
                    row.push(s);
                }
            }
            text.push(Line::from(row));
        }
        frame.render_widget(
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Map")),
            area,
        );
    }

    pub fn render_sideinfo(&mut self, frame: &mut Frame, area: Rect, selected_action_idx: usize) {
        fn hp_bar_spans(current: u32, max: u32, width: usize) -> Vec<Span<'static>> {
            if max == 0 {
                return vec![];
            }
            let ratio = current as f64 / max as f64;
            let filled = ((ratio * width as f64).round() as usize).min(width);
            let empty = width - filled;
            let color = if ratio > 0.5 {
                Color::Green
            } else if ratio > 0.25 {
                Color::Yellow
            } else {
                Color::Red
            };
            let mut spans: Vec<Span<'static>> = Vec::new();
            if filled > 0 {
                spans.push(Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(color),
                ));
            }
            if empty > 0 {
                spans.push(Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            spans
        }

        let area_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
            .split(area);

        // Extract prompt data early to avoid holding a borrow across field accesses.
        let prompt_info: Option<(usize, Vec<String>)> =
            self.encounter_stack.last().and_then(|se| {
                if let StackElementEntry::Prompt(p) = &se.entry {
                    Some((
                        p.actor_id(),
                        p.actions().iter().map(|a| a.name().to_string()).collect(),
                    ))
                } else {
                    None
                }
            });
        let stack_status = match self.encounter_stack.last() {
            None => "no_prompt",
            Some(se) if matches!(se.entry, StackElementEntry::Prompt(_)) => "prompt",
            Some(_) => "processing",
        };

        // Initiative queue: show all actors in turn order, starting from the
        // current actor; highlight + blink-glyph the active one.
        let init_len = self.initiative_tracker.initiatives.len();
        let curr_idx = self.initiative_tracker.curr_index;
        let mut initiative_lines: Vec<Line<'static>> = Vec::new();
        for i in 0..init_len {
            let slot = (curr_idx + i) % init_len;
            let actor_id = self.initiative_tracker.initiatives[slot].actor_id;
            let is_current = prompt_info.as_ref().is_some_and(|(id, _)| *id == actor_id);
            if let Some(actor) = self.actors.get(&actor_id) {
                let (s, c, bg) = get_colored_span(actor_id, actor.team());
                let prefix = if is_current { "> " } else { "  " };
                let mut glyph_style = Style::default().fg(c).bg(bg);
                let name_style = if is_current {
                    if !blink_on() {
                        glyph_style = glyph_style.add_modifier(Modifier::REVERSED);
                    }
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let mut spans: Vec<Span<'static>> = vec![
                    Span::raw(prefix),
                    Span::styled(s, glyph_style),
                    Span::raw(" "),
                    Span::styled(actor.name().to_string(), name_style),
                    Span::raw(" "),
                ];
                spans.extend(hp_bar_spans(actor.hitpoints(), actor.max_hitpoints(), 8));
                initiative_lines.push(Line::from(spans));
            }
        }
        frame.render_widget(
            Paragraph::new(initiative_lines)
                .block(Block::default().borders(Borders::ALL).title("Initiative")),
            area_split[0],
        );

        // Resources / Actions panels need a valid prompt with a known actor.
        let Some((curr_actor_id, action_names)) = prompt_info else {
            let msg = match stack_status {
                "processing" => "(processing...)",
                _ => "(no prompt)",
            };
            frame.render_widget(
                Paragraph::new(msg)
                    .block(Block::default().borders(Borders::ALL).title("Resources")),
                area_split[1],
            );
            frame.render_widget(
                Paragraph::new("").block(Block::default().borders(Borders::ALL).title("Actions")),
                area_split[2],
            );
            return;
        };

        let Some(curr_actor) = self.actors.get(&curr_actor_id) else {
            frame.render_widget(
                Paragraph::new("(missing actor)")
                    .block(Block::default().borders(Borders::ALL).title("Resources")),
                area_split[1],
            );
            frame.render_widget(
                Paragraph::new("").block(Block::default().borders(Borders::ALL).title("Actions")),
                area_split[2],
            );
            return;
        };

        let hp = curr_actor.hitpoints();
        let max_hp = curr_actor.max_hitpoints();
        let ac = curr_actor.armor_class();
        let movement = curr_actor.remaining_movement();
        let action_slots = curr_actor.action_slots();
        let bonus_slots = curr_actor.bonus_action_slots();

        let mut hp_spans: Vec<Span<'static>> = vec![Span::raw("HP: ")];
        hp_spans.extend(hp_bar_spans(hp, max_hp, 10));
        hp_spans.push(Span::raw(format!(" {}/{}", hp, max_hp)));

        let stats_lines: Vec<Line<'static>> = vec![
            Line::from(hp_spans),
            Line::from(Span::raw(format!("AC: {}", ac))),
            Line::from(Span::raw(format!("Movement: {:.0}", movement))),
            Line::from(Span::raw(format!(
                "Actions: {}  Bonus: {}",
                action_slots, bonus_slots
            ))),
        ];
        frame.render_widget(
            Paragraph::new(stats_lines)
                .block(Block::default().borders(Borders::ALL).title("Resources")),
            area_split[1],
        );

        let n_actions = action_names.len();
        let highlight_idx = if n_actions > 0 {
            selected_action_idx % n_actions
        } else {
            0
        };
        let action_lines: Vec<Line<'static>> = action_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let is_selected = i == highlight_idx && n_actions > 0;
                let prefix = if is_selected { "> " } else { "  " };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(vec![Span::raw(prefix), Span::styled(name.clone(), style)])
            })
            .collect();
        frame.render_widget(
            Paragraph::new(action_lines)
                .block(Block::default().borders(Borders::ALL).title("Actions")),
            area_split[2],
        );
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
            temp_encounter_queue: LinkedList::new(),
            roller,
            rng,
            messages: Vec::new(),
            tmp_message: String::new(),
            outcome_tracker: OutcomeTracker::new(),
        };

        // TODO: move pool to fn
        let template_pool: Vec<&'static CreatureTemplate> = vec![&ZOMBIE_TEMPLATE];

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

    /// Removes any actor with 0 HP from the initiative queue, the actor map,
    /// and the actors table. Logs each death. Idempotent.
    pub fn cleanup_dead_actors(&mut self) {
        let dead: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| if a.hitpoints() == 0 { Some(*id) } else { None })
            .collect();
        for id in dead {
            let Some(actor) = self.actors.remove(&id) else {
                continue;
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

    pub fn check_triggers(&mut self, event: &StackElementEntry, _event_type: TriggerEventType) {
        match event {
            StackElementEntry::Prompt(_) => (),
            StackElementEntry::Action(_a) => {
                // TODO
            }
            StackElementEntry::SideEffect(_se) => {
                // TODO
            }
        }
    }

    pub fn enqueue_event(
        &mut self,
        se: StackElementEntry,
        success_dependencies: Option<Vec<usize>>,
    ) {
        self.check_triggers(&se, TriggerEventType::Enqueue);
        self.encounter_stack.push(StackElement {
            entry: se,
            id: self.outcome_tracker.next_id(),
            success_dependencies,
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
                        success_dependencies: se.success_dependencies,
                    });
                    None
                }
            },
        }
    }

    pub fn push_action(&mut self, action_execution_info: ActionExecutionInfo) {
        // TODO: temp stack for reactions
        self.enqueue_event(
            StackElementEntry::Action(Box::new(action_execution_info)),
            None,
        );
    }

    pub fn process_stack(&mut self) {
        if !self.initialized {
            return;
        }

        // Bail if we are already waiting on a player prompt.
        if self.peek_prompt().is_some() {
            return;
        }

        // Transfer the reaction queue onto the main stack before draining.
        while let Some(se) = self.temp_encounter_queue.pop_front() {
            self.encounter_stack.push(se);
        }

        while let Some(se) = self.encounter_stack.pop() {
            self.check_triggers(&se.entry, TriggerEventType::Execute);
            match se.entry {
                StackElementEntry::Prompt(p) => {
                    // A prompt was already on the stack; put it back and bail.
                    self.encounter_stack.push(StackElement {
                        entry: StackElementEntry::Prompt(p),
                        id: se.id,
                        success_dependencies: se.success_dependencies,
                    });
                    return;
                }
                StackElementEntry::Action(a) => {
                    let mut side_effects = a.execute(self);
                    for sen in side_effects.drain(..) {
                        self.enqueue_event(StackElementEntry::SideEffect(sen), None);
                    }
                }
                StackElementEntry::SideEffect(s) => {
                    s.apply(self);
                    self.cleanup_dead_actors();
                }
            }
        }

        self.outcome_tracker.reset();

        // Skip the prompt if the encounter has wound down (everyone died).
        let Some(current_player_id) = self.initiative_tracker.current_player() else {
            return;
        };
        let Some(current_player) = self.actors.get(&current_player_id) else {
            return;
        };
        self.enqueue_event(
            StackElementEntry::Prompt(Prompt::new(
                current_player_id,
                current_player.actions.clone(), // TODO: filter for legal actions (action, bonus action; no reaction)
            )),
            None,
        );
    }
}
