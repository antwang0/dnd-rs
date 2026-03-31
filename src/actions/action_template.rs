use crate::actors::actor_template::ActorInstance;
pub struct Action<'a> {
    name: &'a str,
    lambda: &'a action_closure_type!(),
}

impl Action<'_> {
    pub const fn new<'a>(name: &'a str, lambda: &'a action_closure_type!()) -> Action<'a> {
        Action {
            name: name,
            lambda: lambda,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn lambda(&self) -> &action_closure_type!() {
        self.lambda
    }
}
