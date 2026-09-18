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
    /// SRD 5.2 **Knocking Out a Creature**: *"When you would reduce a
    /// creature to 0 Hit Points with a melee attack, you can instead
    /// reduce the creature to 1 Hit Point. The creature then has the
    /// Unconscious condition."*
    ///
    /// The one override that is about an *attack* rather than a spell,
    /// and it is here for the same reason `CastLevel` is: RAW gives the
    /// swinger a choice, the choice has to travel from wherever the
    /// swing was chosen down to where the damage resolves, and the
    /// override set is the channel that already runs the whole length
    /// of that. A player types `sword goblin ko`; the AI never passes
    /// it, which is correct — a monster is not taking prisoners.
    ///
    /// Read by `SimpleWeapon::side_effects`, which stamps it onto
    /// `AttackParams::nonlethal`; RAW's *"with a melee attack"* is
    /// enforced there rather than here.
    Nonlethal,
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

/// True when the swinger asked for a pulled punch — SRD 5.2's
/// *Knocking Out a Creature*.
///
/// The `cast_level` sibling on the other override, and a free function
/// for the same reason: the question is asked at the weapon chassis
/// rather than at the enum, and a `matches!` open-coded at each site is
/// a `matches!` that can drift.
pub fn pulls_the_punch(overrides: Option<&std::collections::HashSet<ActionOverride>>) -> bool {
    overrides.is_some_and(|o| o.contains(&ActionOverride::Nonlethal))
}
