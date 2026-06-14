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

/// What a passing save does to the damage value: halve it (leveled
/// save-for-half spells / scrolls — Fireball / Cone of Cold / Finger of
/// Death) or zero it (cantrip-style save-or-nothing — Thunderclap /
/// Sword Burst / Disintegrate). The distinction is the 5e RAW split:
/// cantrips don't half-on-save, and a handful of leveled spells
/// (Disintegrate) zero on save instead of halving.
///
/// `apply` and `apply_with_evasion` collapse the post-save damage
/// computation into one chokepoint, so the AoE-burst helper, the
/// single-target damage-scroll factor, and any future save-for-half
/// site all route through the same logic. Stored here in
/// `engine::saves` alongside the pass/fail `SaveOutcome` so every
/// save-shaped decision lives in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDamagePolicy {
    /// Successful save → half damage; failed save → full damage.
    /// 5e default for leveled save-or-half spells (Fireball, Cone of
    /// Cold, Finger of Death, Vitriolic Sphere, Cloudkill, ...).
    HalfOnSave,
    /// Successful save → 0 damage; failed save → full damage. Used by
    /// cantrip-style save-or-nothing effects (Thunderclap, Sword Burst,
    /// Sacred Flame, Mind Sliver) and a small set of leveled spells /
    /// scrolls that zero on save (Disintegrate).
    NoneOnSave,
}

impl SaveDamagePolicy {
    /// Resolve the damage `raw` would deal to a target whose save
    /// outcome is `passed`. Mirrors the standard 5e save-for-half /
    /// save-or-nothing split on the policy lane.
    pub fn apply(self, raw: u32, passed: bool) -> u32 {
        match (self, passed) {
            (_, false) => raw,
            (SaveDamagePolicy::HalfOnSave, true) => raw / 2,
            (SaveDamagePolicy::NoneOnSave, true) => 0,
        }
    }

    /// Evasion-aware variant: on DEX saves against effects that allow
    /// half damage on a successful save, the Rogue Evasion class
    /// feature (and equivalents) turns pass → 0 and fail → half. On
    /// save-or-nothing effects (cantrips, Disintegrate) evasion has
    /// nothing to "evade up to" — pass still zeros, fail still hits
    /// for full — so the table reduces to the non-evasion shape.
    /// Caller is responsible for confirming evasion is in play (DEX
    /// save AND the target has the feature).
    pub fn apply_with_evasion(self, raw: u32, passed: bool) -> u32 {
        match (self, passed) {
            (SaveDamagePolicy::HalfOnSave, true) => 0,
            (SaveDamagePolicy::HalfOnSave, false) => raw / 2,
            (SaveDamagePolicy::NoneOnSave, true) => 0,
            (SaveDamagePolicy::NoneOnSave, false) => raw,
        }
    }
}
