use std::collections::LinkedList;

use crate::{
    actions::action_template::{Action, ActionExecutionInfo},
    engine::{
        encounter::EncounterInstance, errors::ParseError, types::Coordinate, util::parse_coord,
    },
};

pub struct Prompt {
    actor_id: usize,
    actions: Vec<&'static (dyn Action + Send + Sync)>,
}

impl Prompt {
    pub fn new(actor_id: usize, actions: Vec<&'static (dyn Action + Send + Sync)>) -> Self {
        Self { actor_id, actions }
    }

    pub fn actor_id(&self) -> usize {
        self.actor_id
    }

    pub fn actions(&self) -> &Vec<&'static (dyn Action + Send + Sync)> {
        &self.actions
    }

    pub fn process_input(
        &self,
        input: &str,
        encounter_instance: &EncounterInstance,
    ) -> Result<ActionExecutionInfo, ParseError> {
        let actor = encounter_instance
            .actors
            .get(&self.actor_id)
            .expect("missing actor");
        let mut tokens: LinkedList<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(ParseError::new("EMPTY"));
        }
        let action_name: &str = tokens.pop_front().unwrap();

        let action: &(dyn Action + Send + Sync) = *self
            .actions
            .iter()
            .find(|e| action_name == e.name() || e.aliases().contains(&action_name))
            .ok_or_else(|| ParseError::new(&format!("could not find action {}", action_name)))?;

        let target_ids: Vec<usize> = Vec::new();
        let mut target_locations: Vec<Coordinate> = Vec::new();

        while !tokens.is_empty() {
            let token_trimmed = tokens.pop_front().unwrap().trim();
            if let Some(coord) = parse_coord(token_trimmed, actor.location()) {
                target_locations.push(coord);
            }
        }

        let aei: ActionExecutionInfo = ActionExecutionInfo::new(
            action,
            self.actor_id,
            if target_ids.len() > 0 {
                Some(target_ids)
            } else {
                None
            },
            if target_locations.len() > 0 {
                Some(target_locations)
            } else {
                None
            },
            None, // TODO: overrides
        );

        if !aei.validate(&encounter_instance) {
            // TODO: better error for insufficient resources etc
            return Err(ParseError::new(&format!(
                "argument validation failed for {}",
                input
            )));
        }
        return Ok(aei);
    }
}
