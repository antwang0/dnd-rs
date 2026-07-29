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
    ///
    /// Mechanically equivalent to invoking `apply` against a policy
    /// shifted one notch better: HalfOnSave → NoneOnSave-on-pass,
    /// HalfOnSave-on-fail → half (fresh half lane). NoneOnSave already
    /// zeros on pass and full-on-fail, so evasion is a no-op — that
    /// half of the table just delegates to `apply` directly.
    pub fn apply_with_evasion(self, raw: u32, passed: bool) -> u32 {
        match self {
            // Evasion shifts the HalfOnSave table one notch better:
            // pass → 0 (was raw/2), fail → raw/2 (was raw).
            SaveDamagePolicy::HalfOnSave => {
                if passed {
                    0
                } else {
                    raw / 2
                }
            }
            // NoneOnSave: pass already zeros, fail already full —
            // evasion has nothing to improve. Delegate to `apply`.
            SaveDamagePolicy::NoneOnSave => self.apply(raw, passed),
        }
    }

    /// Caster-side mirror of `apply_with_evasion`: the 5e Evocation
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
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_with_evasion(10, true), 0);
        assert_eq!(SaveDamagePolicy::HalfOnSave.apply_with_evasion(10, false), 5);
    }

    #[test]
    fn evasion_is_a_noop_for_none_on_save() {
        // Cantrip-style: evasion can't improve on already-zero pass /
        // already-full fail.
        assert_eq!(SaveDamagePolicy::NoneOnSave.apply_with_evasion(10, true), 0);
        assert_eq!(SaveDamagePolicy::NoneOnSave.apply_with_evasion(10, false), 10);
    }
}
