#[derive(Clone, PartialEq, Hash, Eq)]
pub enum ActionOverride {
    IncreaseTargets(usize),
}
