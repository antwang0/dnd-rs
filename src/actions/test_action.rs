use crate::units::unit_template::ActorInstance;
use crate::actions::action_template::Action;

action_fn!(apply_test_action, {
    true
});

pub const TEST_ACTION: Action = Action::new(
    "test action",
    &apply_test_action
);