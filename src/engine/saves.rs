/// Outcome of a single saving throw against a DC. We don't yet
/// distinguish nat-1 / nat-20 because no current effect cares (5e treats
/// crits and fumbles specially only for death saves and attack rolls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    Pass,
    Fail,
}

impl SaveOutcome {
    pub fn passed(&self) -> bool {
        matches!(self, SaveOutcome::Pass)
    }
}
