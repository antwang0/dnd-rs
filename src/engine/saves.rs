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
/// `apply` and `apply_mitigated` collapse the post-save damage
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

/// A target-side effect that improves what a saving throw leaves
/// standing. Both variants zero a successful save that would otherwise
/// have halved; they differ on whether a *failed* save is softened too,
/// and that difference is the whole of RAW's difference between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMitigation {
    /// 5e **Evasion** (Rogue / Monk / Ranger): "you instead take no
    /// damage if you succeed on the saving throw, and only half damage
    /// if you fail." Two clauses, so the whole table shifts a notch.
    /// Scoped by its holder to Dexterity saves, whatever the source.
    Evasion,
    /// 5e **Circle of Power**: "when an affected creature succeeds on a
    /// saving throw … it takes no damage instead of half damage." One
    /// clause, about success only — a creature that fails its save
    /// takes everything. Scoped by its holder to spells, whatever the
    /// ability.
    NoneOnSuccess,
}

impl SaveMitigation {
    /// Log-friendly name of the effect, for the "takes no damage" line.
    pub fn label(self) -> &'static str {
        match self {
            SaveMitigation::Evasion => "evasion",
            SaveMitigation::NoneOnSuccess => "circle of power",
        }
    }
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

    /// Apply this policy with a target-side `mitigation` in play. See
    /// `SaveMitigation` for the two shapes and what distinguishes
    /// them; caller is responsible for confirming the mitigation
    /// applies at all.
    pub fn apply_mitigated(self, raw: u32, passed: bool, mitigation: SaveMitigation) -> u32 {
        match (self, mitigation) {
            // Nothing to improve on a save-or-nothing effect: a pass
            // already zeroes and a fail already lands in full. Both
            // mitigations reduce to the bare policy here.
            (SaveDamagePolicy::NoneOnSave, _) => self.apply(raw, passed),
            // Evasion shifts the HalfOnSave table one whole notch
            // better: pass → 0 (was raw/2), fail → raw/2 (was raw).
            (SaveDamagePolicy::HalfOnSave, SaveMitigation::Evasion) => {
                if passed { 0 } else { raw / 2 }
            }
            // The ward touches only the successful half of the table.
            (SaveDamagePolicy::HalfOnSave, SaveMitigation::NoneOnSuccess) => {
                if passed { 0 } else { raw }
            }
        }
    }

    /// Caster-side mirror of `apply_mitigated`: the 5e Evocation
    /// Wizard **Potent Cantrip** shifts a save-or-nothing cantrip one
    /// notch *worse for the target* — a successful save now leaves half
    /// damage standing instead of none.
    ///
    /// Expressed as a policy upgrade rather than a damage tweak, because
    /// that is exactly what it is: `NoneOnSave` becomes `HalfOnSave` and
    /// everything downstream (Evasion, the caller's zero-damage
    /// short-circuit) reads the upgraded policy and composes correctly
    /// on its own. A rogue with Evasion who makes their DEX save against
    /// a potent Sacred Flame still takes nothing — Potent Cantrip lifts
    /// the effect into the "half on a successful save" class, which is
    /// precisely the class Evasion zeroes.
    ///
    /// `HalfOnSave` is returned unchanged: it already leaves half
    /// standing, and the leveled spells that carry it aren't cantrips.
    /// Caller is responsible for confirming the feature is in play AND
    /// that the cast is a cantrip.
    pub fn with_potent_cantrip(self) -> Self {
        match self {
            SaveDamagePolicy::NoneOnSave => SaveDamagePolicy::HalfOnSave,
            SaveDamagePolicy::HalfOnSave => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_on_save_halves_pass_full_on_fail() {
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply(10, true), 5);
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply(10, false), 10);
    }

    #[test]
    fn none_on_save_zeroes_pass_full_on_fail() {
        assert_eq!(SaveDamagePolicy::NoneOnSave.apply(10, true), 0);
        assert_eq!(SaveDamagePolicy::NoneOnSave.apply(10, false), 10);
    }

    #[test]
    fn evasion_shifts_half_on_save_one_notch_better() {
        // Pass → 0 (was 5), fail → 5 (was 10).
        let m = SaveMitigation::Evasion;
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_mitigated(10, true, m), 0);
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_mitigated(10, false, m), 5);
    }

    #[test]
    fn a_success_only_ward_leaves_a_failed_save_in_full() {
        // The one line that separates the two mitigations: a creature
        // that fails its save inside a Circle of Power takes
        // everything, where one with Evasion would take half.
        let m = SaveMitigation::NoneOnSuccess;
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_mitigated(10, true, m), 0);
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_mitigated(10, false, m), 10);
    }

    #[test]
    fn neither_mitigation_moves_a_save_or_nothing_effect() {
        // Cantrip-style: there is nothing to improve on an
        // already-zero pass or an already-full fail.
        for m in [SaveMitigation::Evasion, SaveMitigation::NoneOnSuccess] {
            assert_eq!(SaveDamagePolicy::NoneOnSave.apply_mitigated(10, true, m), 0);
            assert_eq!(SaveDamagePolicy::NoneOnSave.apply_mitigated(10, false, m), 10);
        }
    }
}
