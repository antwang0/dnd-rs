/// Caller-supplied overrides that bend an action's normal targeting /
/// resource rules. Threaded through the `Action` trait; the picker UI
/// and AI use these to modify spell behavior at cast time (e.g. upcasting
/// a Fireball at level 5 to add extra dice).
#[derive(Clone, PartialEq, Hash, Eq)]
pub enum ActionOverride {
    /// Bumps the action's target cap by `n`. Reserved for sorcerer
    /// metamagic (Twinned) and other future "extra-target" hooks. Today
    /// no action checks for this directly — Twinned Spell is wired
    /// through `EncounterInstance::consume_twinned_spell` instead so the
    /// metamagic works on every SingleActor action without per-spell
    /// opt-in. This variant stays reserved for spells that want explicit
    /// per-cast target-count control.
    IncreaseTargets(usize),
    /// Cast a leveled spell at a higher slot level than its base. The
    /// spell's `cost()` returns `SpellSlot(cast_level)` and its
    /// `side_effects()` scales damage / healing / targets accordingly.
    /// The AI passes this when the base-level slot is exhausted but a
    /// higher one is available, or when the extra power is worth the
    /// slot. Spells that don't support upcasting simply ignore it.
    CastLevel(u32),
}

/// Extract the cast level from an override set, defaulting to `base`.
/// Shared by every spell that supports upcasting — called in both
/// `cost()` (to return the right `SpellSlot`) and `side_effects()`
/// (to scale dice / healing / targets).
pub fn cast_level(overrides: Option<&std::collections::HashSet<ActionOverride>>, base: u32) -> u32 {
    overrides
        .and_then(|o| {
            o.iter().find_map(|ov| match ov {
                ActionOverride::CastLevel(lvl) => Some(*lvl),
                _ => None,
            })
        })
        .unwrap_or(base)
        .max(base)
}
