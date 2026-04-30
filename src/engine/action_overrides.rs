/// Caller-supplied overrides that bend an action's normal targeting /
/// resource rules. Threaded through the `Action` trait but not yet
/// consumed by any action — left in place because the picker UI will
/// need it once we wire user-facing modifiers (e.g. Twinned Spell,
/// Eldritch Spear). Keep the variant docs current as new overrides land.
#[derive(Clone, PartialEq, Hash, Eq)]
pub enum ActionOverride {
    /// Bumps the action's target cap by `n`. Reserved for sorcerer
    /// metamagic (Twinned). Today no action checks for this.
    IncreaseTargets(usize),
}
