#[macro_export]
macro_rules! action_fn {
    ($name:ident, $body:block) => {
        fn $name(caster: &mut ActorInstance, targets: Option<Vec<&mut ActorInstance>>, target_location: Option<(usize, usize)>, dry_run: bool) -> bool $body
    };
}

macro_rules! action_closure_type {
    () => {
        dyn Fn(&mut ActorInstance, Option<Vec<&mut ActorInstance>>, Option<(usize, usize)>, bool) -> bool
    }
}