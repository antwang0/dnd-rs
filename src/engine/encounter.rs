use std::collections::{HashMap, LinkedList};
use std::error::Error;

use crate::actions::action_template::ActionExecutionInfo;
use crate::actors::actor_template::{ActorInstance, CreatureTemplate};
use crate::engine::actor_gen::{ActorGenParams, generate_actors};
use crate::engine::errors::NoLegalPosition;
use crate::engine::prompt::Prompt;
use crate::engine::side_effects::ApplicableSideEffect;
use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::triggers::TriggerEventType;
use crate::engine::types::Size;
use crate::engine::util::{get_colored_span, get_tiles_from_size};
use rand::seq::SliceRandom;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{style::Color, text::Span};
use std::cmp::Ordering;
use tyche::dice::roller::FastRand as FastRandRoller;

pub enum StackElement {
    SideEffect(Box<dyn ApplicableSideEffect>),
    Action(Box<ActionExecutionInfo>),
    Prompt(Prompt),
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
        if let Some(ie) = self.initiatives.get(self.curr_index) {
            Some(ie.actor_id)
        } else {
            None
        }
    }

    pub fn advance(&mut self) {
        if self.curr_index >= self.initiatives.len() - 1 {
            self.curr_index = 0;
        } else {
            self.curr_index += 1;
        }
    }

    pub fn add_actor(&mut self, actor_id: usize, initiative: i32) {
        let mut idx: usize = 0;
        for (i, ie) in self.initiatives.iter().enumerate() {
            idx = i;
            if initiative > ie.initiative {
                break;
            }
        }
        self.initiatives.insert(
            idx,
            InitiativeElement {
                actor_id: actor_id,
                initiative: initiative,
            },
        );
        if idx <= self.curr_index {
            self.curr_index += 1;
        }
    }

    pub fn initialize_actors(&mut self, actors: &HashMap<usize, Box<ActorInstance>>) {
        for (id, actor) in actors.iter() {
            self.initiatives.push(InitiativeElement {
                actor_id: *id,
                initiative: actor.initiative().expect("Expected initiative"),
            });
        }
        self.initiatives.sort();
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
    pub actors: HashMap<usize, Box<ActorInstance>>,
    initiative_tracker: InitiativeTracker,
    pub encounter_stack: Vec<StackElement>,
    pub temp_encounter_queue: LinkedList<StackElement>, // for handling multiple reactions
    pub roller: FastRandRoller,
}

impl EncounterInstance {
    pub fn next_actor_id(&mut self) -> usize {
        let next_actor_id = self.actor_id_next;
        self.actor_id_next += 1;
        next_actor_id
    }

    pub fn idx(&self, x: usize, y: usize) -> usize {
        x + y * self.width
    }

    pub fn is_spawnable(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        if let Some(_) = self.actor_id_at(x, y) {
            return false;
        }

        let ti = self.terrain_at(x, y);
        match ti.terrain_type {
            TerrainType::Floor => true,
            _ => false,
        }
    }

    fn get_random_coord_list(&self) -> Vec<(usize, usize)> {
        // TODO: probably try random order of one axis first
        let mut all_coords: Vec<(usize, usize)> = Vec::new();
        for x in 0..self.width {
            for y in 0..self.height {
                all_coords.push((x, y));
            }
        }
        all_coords.shuffle(&mut rand::rng());
        all_coords
    }

    pub fn get_random_spawn(&mut self, size: Size) -> Result<(usize, usize), NoLegalPosition> {
        let actor_width: usize = get_tiles_from_size(size);
        let coords = self.get_random_coord_list();

        'coord_loop: for (x, y) in coords.iter() {
            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    if !self.is_spawnable(x + x_off, y + y_off) {
                        continue 'coord_loop;
                    }
                }
            }
            return Ok((*x, *y));
        }
        Err(NoLegalPosition)
    }

    pub fn actor_id_at(&self, x: usize, y: usize) -> Option<usize> {
        if let Some(r) = self.actor_map.get(self.idx(x, y)) {
            *r
        } else {
            None
        }
    }

    pub fn set_actor_id_at(&mut self, actor_id: Option<usize>, x: usize, y: usize) {
        let idx = self.idx(x, y);
        self.actor_map[idx] = actor_id;
    }

    pub fn terrain_at(&self, x: usize, y: usize) -> &TerrainInfo {
        &self.terrain[self.idx(x, y)]
    }

    pub fn render_map(&self, frame: &mut Frame, area: Rect) {
        let mut text: Vec<Line> = Vec::new();

        for y in 0..self.height {
            let mut row: Vec<Span> = Vec::new();
            for x in 0..self.width {
                if let Some(actor_id) = self.actor_id_at(x, y) {
                    match self.actors.get(&actor_id) {
                        Some(actor) => {
                            let (s, c, bg): (String, Color, Color) =
                                get_colored_span(actor_id, actor.team());
                            row.push(Span::styled(s, Style::default().fg(c).bg(bg)));
                        }
                        None => {
                            panic!("Actor not found");
                        }
                    }
                } else {
                    let s = Span::from(
                        match self.terrain_at(x, y).terrain_type {
                            TerrainType::Empty => ' ',
                            TerrainType::Floor => '░',
                            TerrainType::Wall => '█',
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

    pub fn render_sideinfo(&self, frame: &mut Frame, area: Rect) {
        let curr_actor_id = self
            .initiative_tracker
            .current_player()
            .expect("tried to render sideinfo for uninit initiative");
        let curr_actor = self.actors.get(&curr_actor_id).expect("missing actor");
        let (s, c, bg): (String, Color, Color) = get_colored_span(curr_actor_id, curr_actor.team());

        let mut initiative_bar: Vec<Span> = Vec::new();
        initiative_bar.push(Span::from(format!("Current actor: {} ", curr_actor.name())));
        initiative_bar.push(Span::styled(s, Style::default().fg(c).bg(bg)));
        let mut txt: Vec<Line> = Vec::new();
        txt.push(Line::from(initiative_bar));

        frame.render_widget(
            Paragraph::new(txt).block(Block::default().borders(Borders::ALL).title("Initiative")),
            area,
        );
    }

    pub fn from_params(
        terrain_params: &TerrainGenParams,
        actor_params: &ActorGenParams,
    ) -> EncounterInstance {
        let mut ei = EncounterInstance {
            initialized: false,
            width: terrain_params.width,
            height: terrain_params.height,
            terrain: generate_terrain(terrain_params),
            actor_id_next: 0,
            actor_map: vec![None; terrain_params.width * terrain_params.height],
            actors: HashMap::new(),
            initiative_tracker: InitiativeTracker::new(),
            encounter_stack: Vec::new(),
            temp_encounter_queue: LinkedList::new(),
            roller: FastRandRoller::default(), // TODO: seed https://docs.rs/tyche/latest/tyche/#rolling-dice
        };
        match generate_actors(&mut ei, actor_params) {
            Ok(()) => {}
            Err(_) => panic!("failed to generate actors"),
        }
        ei.initialize();
        ei
    }

    pub fn set_actor_map(
        &mut self,
        actor_id: usize,
        (x, y): (usize, usize),
        old_location_opt: Option<(usize, usize)>,
    ) -> Result<(), Box<dyn Error>> {
        // assumes unit is square
        if let Some((old_x, old_y)) = old_location_opt {
            for i in old_x..self.width {
                if self.actor_id_at(i, old_y) == None {
                    break;
                }
                for j in old_y..self.height {
                    if self.actor_id_at(i, j) == None {
                        break;
                    }
                    self.set_actor_id_at(None, i, j);
                }
            }
        }
        if let Some(actor) = self.actors.get(&actor_id) {
            let actor_width = get_tiles_from_size(actor.size());

            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    self.set_actor_id_at(Some(actor_id), x + x_off, y + y_off);
                }
            }
            return Ok(());
        }
        Err("Actor not found".into())
    }

    pub fn instantiate_creature(
        &mut self,
        creature_template: &mut CreatureTemplate,
        location: (usize, usize),
        team_id: usize,
    ) -> Result<usize, Box<dyn Error>> {
        let actor_id = self.next_actor_id();

        let mut ai_box = Box::new(ActorInstance::from_creature_template(
            creature_template,
            location,
            team_id,
            &mut self.roller,
        )?);

        if self.initialized {
            ai_box.roll_initiative(&mut self.roller);
            self.initiative_tracker
                .add_actor(actor_id, ai_box.initiative().unwrap());
        }

        self.actors.insert(actor_id, ai_box);

        self.set_actor_map(actor_id, location, None)?;

        Ok(actor_id)
    }

    pub fn initialize(&mut self) {
        if self.initialized {
            panic!("attempted to initialize already initialized encounter")
        }
        for (_, actor) in self.actors.iter_mut() {
            actor.roll_initiative(&mut self.roller);
        }
        self.initiative_tracker.initialize_actors(&self.actors);
        self.initialized = true;
    }

    pub fn check_triggers(&mut self, event: &StackElement, event_type: TriggerEventType) {
        match event {
            StackElement::Prompt(_) => return,
            StackElement::Action(a) => {
                // TODO
            }
            StackElement::SideEffect(se) => {
                // TODO
            }
        }
    }

    pub fn enqueue_event(&mut self, se: StackElement) {
        self.check_triggers(&se, TriggerEventType::Enqueue);
        self.encounter_stack.push(se);
    }

    pub fn process_stack(&mut self) {
        if !self.initialized {
            panic!("attempted to run uninitialized encounter");
        }
        // if we ever encounter something that prompts a user/AI input, we
        // should stop processing the stack

        // check if we are done processing the current batch of possible reactions
        if let Some(StackElement::Prompt(_)) = self.encounter_stack.last() {
            // exit on prompt
            return;
        }
        // transfer the temp queue to the stack
        while !self.temp_encounter_queue.is_empty() {
            self.encounter_stack.push(
                self.temp_encounter_queue
                    .pop_front()
                    .expect("temp queue should not be empty"),
            );
        }

        while !self.encounter_stack.is_empty() {
            let se = self.encounter_stack.pop().expect("unexpected empty stack");
            self.check_triggers(&se, TriggerEventType::Execute);
            match se {
                StackElement::Prompt(_) => return,
                StackElement::Action(a) => {
                    let mut side_effects = a.execute(self);
                    for sen in side_effects.drain(..) {
                        self.enqueue_event(StackElement::SideEffect(sen));
                    }
                }
                StackElement::SideEffect(s) => {
                    s.apply(self);
                }
            };
        }

        // TODO: get the next prompt if necessary
        // the stack should contain a prompt at the top always
        if let Some(StackElement::Prompt(_)) = self.encounter_stack.last() {
            // exit on prompt
            return;
        }
    }

    pub fn skip_turn(&mut self) {
        self.initiative_tracker.advance();
    }
}
