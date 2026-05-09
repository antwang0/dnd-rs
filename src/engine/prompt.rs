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

    /// Parse one argument token into either a target actor id or a
    /// target tile. Supported forms:
    /// - `id:N` (or `#N`) — actor id N. Lookup is exact; rejects unknown ids.
    /// - `<x>,<y>` / `r3u2` / `l4d1` etc. — coordinate (see `parse_coord`).
    /// - `<name>` — exact actor name match (e.g. "Zombie 0" must be quoted
    ///   or use `id:N`; we don't do fuzzy matching).
    fn parse_arg(
        token: &str,
        encounter: &EncounterInstance,
        base: Coordinate,
    ) -> Option<TargetArg> {
        // Explicit actor-id form. Accepts "id:7", "#7".
        let id_str = token
            .strip_prefix("id:")
            .or_else(|| token.strip_prefix('#'));
        if let Some(s) = id_str
            && let Ok(id) = s.parse::<usize>()
            && encounter.actors.contains_key(&id)
        {
            return Some(TargetArg::Actor(id));
        }
        // Coordinate forms.
        if let Some(coord) = parse_coord(token, base) {
            return Some(TargetArg::Point(coord));
        }
        // Exact actor name match (no spaces — names have one). We compare
        // ignoring case to keep typing quick.
        encounter
            .actors
            .iter()
            .find(|(_, a)| a.name().eq_ignore_ascii_case(token))
            .map(|(id, _)| TargetArg::Actor(*id))
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

        let mut target_ids: Vec<usize> = Vec::new();
        let mut target_locations: Vec<Coordinate> = Vec::new();

        while let Some(tok) = tokens.pop_front() {
            let token_trimmed = tok.trim();
            match Self::parse_arg(token_trimmed, encounter_instance, actor.location()) {
                Some(TargetArg::Actor(id)) => target_ids.push(id),
                Some(TargetArg::Point(coord)) => target_locations.push(coord),
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

enum TargetArg {
    Actor(usize),
    Point(Coordinate),
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

    /// Player can target an enemy actor by `id:N` syntax for SingleActor
    /// actions like Slam. Before this, the prompt only ever populated
    /// target_locations and SingleActor actions could never validate from
    /// a typed command.
    #[test]
    fn parse_id_form_targets_actor() {
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        // Use bare encounter without process_stack so we can place actors
        // ourselves and craft a deterministic prompt.
        let tp = TerrainGenParams {
            width: 20,
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let attacker = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        // Build a prompt manually for the attacker.
        use super::Prompt;
        let prompt = Prompt::new(attacker, e.actors[&attacker].available_actions());
        let aei = prompt
            .process_input(&format!("trip id:{}", target), &e)
            .expect("id:N should resolve to actor target");
        assert_eq!(aei.target_ids().and_then(|ids| ids.first().copied()), Some(target));
    }
}

