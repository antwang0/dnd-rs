use crate::{
    actions::action_template::{Action, ActionExecutionInfo},
    engine::errors::ParseError,
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

    pub fn process_input(&self, input: &str) -> Result<ActionExecutionInfo, ParseError> {
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(ParseError::new("EMPTY"));
        }
        let action_name: &str = tokens.first().unwrap();

        if let Some(&action) = self
            .actions
            .iter()
            .find(|e| action_name == e.name() || e.aliases().contains(&action_name))
        {
            return Ok(ActionExecutionInfo::new(
                action,
                self.actor_id,
                None,
                None,
                None,
            ));
        }
        Err(ParseError::new(&format!(
            "could not find action {:?}",
            action_name
        )))
        // TODO: parse args
    }
}
