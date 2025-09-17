use crate::actions::action_template::Action;
use crate::actors::actor_template::ActorInstance;

action_fn!(apply_test_action, { true });

pub const TEST_ACTION: Action = Action::new("test action", &apply_test_action);
