use std::collections::{HashMap, LinkedList};
use std::error::Error;

use crate::actors::actor_template::{ActorInstance, CreatureTemplate};
use crate::engine::actor_gen::{ActorGenParams, generate_actors};
use crate::engine::side_effects::ApplicableSideEffect;
use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::types::{NoLegalPosition, Size};
use crate::engine::util::{get_colored_span, get_tiles_from_size};
use rand::seq::SliceRandom;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{
    style::Color,
    text::Span,
};
use tyche::dice::roller::FastRand as FastRandRoller;

// TODO: having all pub is not very good
pub struct EncounterInstance {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<TerrainInfo>,
    pub actor_id_next: usize,
    pub actor_map: Vec<Option<usize>>,
    pub actors: HashMap<usize, Box<ActorInstance>>,
    pub side_effect_stack: LinkedList<Box<dyn ApplicableSideEffect>>,
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

    pub fn render(&self, frame: &mut Frame, area: Rect) {
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

    pub fn from_params(
        terrain_params: &TerrainGenParams,
        actor_params: &ActorGenParams,
    ) -> EncounterInstance {
        let mut ei = EncounterInstance {
            width: terrain_params.width,
            height: terrain_params.height,
            terrain: generate_terrain(terrain_params),
            actor_id_next: 0,
            actor_map: vec![None; terrain_params.width * terrain_params.height],
            actors: HashMap::new(),
            side_effect_stack: LinkedList::new(),
            roller: FastRandRoller::default(), // TODO: seed https://docs.rs/tyche/latest/tyche/#rolling-dice
        };
        match generate_actors(&mut ei, actor_params) {
            Ok(()) => {}
            Err(_) => panic!("failed to generate actors"),
        }
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
        creature_template: &CreatureTemplate,
        location: (usize, usize),
        team_id: usize,
    ) -> Result<usize, Box<dyn Error>> {
        let actor_id = self.next_actor_id();

        self.actors.insert(
            actor_id,
            Box::new(ActorInstance::from_creature_template(
                &creature_template,
                location,
                team_id,
                &mut self.roller,
            )?),
        );

        self.set_actor_map(actor_id, location, None)?;

        Ok(actor_id)
    }
}
