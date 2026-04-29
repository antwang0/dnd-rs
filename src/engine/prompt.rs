use std::collections::VecDeque;

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
            .ok_or_else(|| ParseError::new(format!("missing actor {}", self.actor_id)))?;

        let mut tokens: VecDeque<&str> = input.split_whitespace().collect();
        let Some(action_name) = tokens.pop_front() else {
            return Err(ParseError::with_input("empty input", input));
        };

        let action: &(dyn Action + Send + Sync) = *self
            .actions
            .iter()
            .find(|e| action_name == e.name() || e.aliases().contains(&action_name))
            .ok_or_else(|| {
                ParseError::with_input(
                    format!("unknown or unavailable action {:?}", action_name),
                    input,
                )
            })?;

        // Text prompts can only specify locations today; actor-id targeting
        // is done by the UI's picker, which constructs the AEI directly.
        let mut target_locations: Vec<Coordinate> = Vec::new();

        while let Some(tok) = tokens.pop_front() {
            let token_trimmed = tok.trim();
            match parse_coord(token_trimmed, actor.location()) {
                Some(coord) => target_locations.push(coord),
                None => {
                    return Err(ParseError::with_input(
                        format!("could not parse argument {:?}", token_trimmed),
                        input,
                    ));
                }
            }
        }

        let aei = ActionExecutionInfo::new(
            action,
            self.actor_id,
            None,
            if target_locations.is_empty() {
                None
            } else {
                Some(target_locations)
            },
            None,
        );

        if !aei.validate(encounter_instance) {
            return Err(ParseError::with_input(
                "invalid arguments or insufficient resources",
                input,
            ));
        }
        Ok(aei)
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;

    fn ei() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let ap = ActorGenParams {
            cr_target: 0.25,
            n_teams: 2,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(99)).unwrap();
        e.process_stack();
        e
    }

    #[test]
    fn parse_empty_input_returns_error_with_context() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        assert!(err.to_string().to_lowercase().contains("empty"));
    }

    #[test]
    fn parse_unknown_action_returns_named_error() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("frobnicate", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        let msg = err.to_string();
        assert!(msg.contains("frobnicate"), "msg = {}", msg);
    }

    #[test]
    fn parse_skip_succeeds() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let aei = match prompt.process_input("skip", &e) {
            Ok(a) => a,
            Err(err) => panic!("unexpected error: {}", err),
        };
        assert!(aei.validate(&e));
    }
}

