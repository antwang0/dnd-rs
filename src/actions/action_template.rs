use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride,
    encounter::EncounterInstance,
    saves::SaveDamagePolicy,
    side_effects::{ApplicableSideEffect, ConsumeResource, DealDamage, Resource},
    types::{AbilityScoreType, Coordinate, DamageType},
};

/// **The** per-target save-and-damage loop. Every burst in the engine
/// resolves through this function — the two neutral / enemy wrappers
/// below it, and `spells::burst_save_damage`, which layers a shared
/// caster-aware damage roll and a log line on top of it.
///
/// Walks `target_ids` in the given order, rolls a caster-aware save (so
/// Sorcerer Heightened Spell forces disadvantage on the *first* save in
/// the burst per RAW), applies the caster-side and target-side damage
/// modifiers at the shared chokepoint (Potent Cantrip, Rogue / Monk /
/// Ranger Evasion), and emits a `DealDamage` side-effect for every
/// non-zero hit. Ids in `shielded` (Sorcerer Careful Spell / Evocation
/// Wizard Sculpt Spells — see `auto_pass_shielded_allies`) auto-pass
/// with 0 damage and skip the roll entirely.
///
/// Returns the effects *and* `(target_id, passed)` for every actor that
/// took the save, in resolution order. The saves vector is what lets a
/// caller hang a per-target failure rider — Tidal Wave's Prone, Mental
/// Prison's Restrained — off the burst without re-walking it and
/// re-deriving who failed. Shielded allies are recorded as having
/// passed, because RAW for both shielding features is "automatically
/// succeed on their saving throws"; a rider that skips them is
/// therefore right rather than approximate.
///
/// This used to exist twice. `spells.rs` carried a near-identical copy
/// with the policy and the saves vector, and this one hardcoded
/// `HalfOnSave` and threw the saves away — so the two drifted on which
/// rules they applied and in what order, and a fix to one silently
/// missed roughly half the bursts in the game. There is one loop now,
/// and the differences that were real (where the target list comes
/// from, whether the damage is rolled here or handed in) live in the
/// thin wrappers around it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_burst_targets(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: &[usize],
    save_ability: AbilityScoreType,
    dc: i32,
    damage: u32,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
    shielded: &HashSet<usize>,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    let mut saves: Vec<(usize, bool)> = Vec::new();
    for &target_id in target_ids {
        if shielded.contains(&target_id) {
            saves.push((target_id, true));
            continue;
        }
        // Route through the caster-aware save helper so the 5e Sorcerer
        // Heightened Spell metamagic forces disadvantage on the *first*
        // save in the burst (RAW). Subsequent targets in the same cast
        // fall through to the normal save path — `roll_save_against_caster`
        // consumes the prime on its first call.
        let save = encounter.roll_save_against_caster(target_id, save_ability, dc, caster_id);
        let passed = save.passed();
        saves.push((target_id, passed));
        // Post-save damage (Potent Cantrip on the caster side, Evasion
        // on the target side, plus `policy`) resolves at the shared
        // engine chokepoint — see `resolve_post_save_damage`.
        let dmg = encounter.resolve_post_save_damage(
            caster_id,
            target_id,
            save_ability,
            policy,
            damage,
            passed,
        );
        if dmg == 0 {
            continue;
        }
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type,
        }));
    }
    (effects, saves)
}

/// Resolve a damage-burst AoE: every combat-active actor whose footprint is
/// within `radius` of `center` (excluding the caster) makes a save against
/// `dc` using `save_ability`. Pass = half damage (rounded down), fail = full.
/// `damage` is rolled once and shared, matching 5e shared-roll semantics
/// for area effects. Returns DealDamage side-effects (empty for actors who
/// take 0). Caller controls the actual roll + log message.
///
/// Centralizes the pattern shared by Sacred Burst, Burning Hands, and the
/// Fireball scroll — keeps save sequencing deterministic (sorted ids) and
/// the caster-exempt + combat-active filters consistent.
#[allow(clippy::too_many_arguments)]
pub fn resolve_burst_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    center: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    damage: u32,
    damage_type: DamageType,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // Shielded allies in the burst auto-pass the save AND take 0 damage
    // (Sorcerer Careful Spell, Evocation Wizard Sculpt Spells). Resolved
    // up-front so the shared per-target loop can skip them cleanly; the
    // Careful Spell prime is consumed inside the helper.
    let shielded = encounter.auto_pass_shielded_allies(caster_id, center, radius);
    // `neutral_burst_targets` shares the "caster-excluded, combat-active,
    // footprint in radius" filter with the rest of the engine — folding it
    // here keeps the caster-exclusion / footprint-Chebyshev / sorted-ids
    // invariant in one place instead of re-inlining the loop.
    let target_ids = encounter.neutral_burst_targets(caster_id, center, radius);
    resolve_burst_targets(
        encounter,
        caster_id,
        &target_ids,
        save_ability,
        dc,
        damage,
        damage_type,
        SaveDamagePolicy::HalfOnSave,
        &shielded,
    )
    .0
}

/// Enemy-only sibling of `resolve_burst_save_damage`. Every combat-active
/// enemy inside `radius` of `center` makes a save vs `dc` against a
/// pre-rolled `damage`, resolved under `policy`. Same shape as the
/// neutral variant — the ally-shield sweep is a no-op here since both
/// features on it only protect allies (enemy bursts already exclude
/// them). Same Evasion handling for DEX saves.
///
/// Used by class features whose RAW target set is "hostile creatures
/// within the burst" — Radiance of the Dawn, which is save-for-half, and
/// the Sun Soul Monk's Searing Sunburst, which is save-for-nothing —
/// distinct from friend-or-foe bursts (Sacred Burst / Burning Hands /
/// Fireball) that route through the neutral variant.
///
/// This is the variant that carries the `policy` parameter, and the
/// neutral one doesn't, because the enemy lane is where both policies
/// have shown up. Threading it through the neutral wrapper too would put
/// an explicit `SaveDamagePolicy::HalfOnSave` on twenty-six call sites
/// in service of none of them; the day a neutral burst zeroes on a save,
/// it is the same one-parameter change made here.
#[allow(clippy::too_many_arguments)]
pub fn resolve_enemy_burst_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    center: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    damage: u32,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // Enemy bursts skip allies at the target-list step, so the
    // ally-shield sweep has nothing left to spare — pass an empty set
    // through instead of re-running the lookup.
    let shielded: HashSet<usize> = HashSet::new();
    let target_ids = encounter.enemy_burst_targets(caster_id, center, radius);
    resolve_burst_targets(
        encounter,
        caster_id,
        &target_ids,
        save_ability,
        dc,
        damage,
        damage_type,
        policy,
        &shielded,
    )
    .0
}

/// Reach for melee/touch actions, expressed as a footprint-Chebyshev gap cap.
/// 5e melee weapons are 5ft = 1-tile gap in this 2.5ft grid. Polearms /
/// reach weapons would be 2. Ranged actions return their max range here.
pub const MELEE_REACH: isize = 1;

/// Sister helper to `resolve_burst_save_damage` for the "save-or-pick-up-
/// a-condition" burst shape: every enemy in `radius` of `center` rolls
/// `save_ability` vs `dc`; failed-save targets pick up `condition` for
/// `timer`. Damage-free — the load-bearing effect is the condition install.
///
/// Used by monster save-or-condition AoEs (Gorgon's Petrifying Breath
/// burst, future gaze / shout / aura abilities) where the spell-shape
/// `concentration_burst_condition_only` doesn't fit (no concentration to
/// anchor — monster abilities just fire-and-forget). Keeps the recurring
/// "iterate enemy_burst_targets → roll save → install on fail" loop in
/// one place so future tweaks (e.g. a caster-aware save helper for the
/// Heightened-Spell metamagic prime on the first save in the burst) land
/// once.
#[allow(clippy::too_many_arguments)]
pub fn resolve_burst_save_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    center: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let target_ids = encounter.enemy_burst_targets(caster_id, center, radius);
    install_condition_on_failed_saves(encounter, &target_ids, save_ability, dc, condition, timer)
}

/// The save-or-condition half of every condition burst: roll `save_ability`
/// vs `dc` for each id in `target_ids`, in the order given, and queue an
/// `ApplyCondition` for each one that fails.
///
/// Sibling to `resolve_burst_targets` on the damage lane, and split out
/// for the same reason: the two condition bursts above differ only in
/// how they pick their targets, and a loop written twice is a rule that
/// can be fixed in one place and stay broken in the other.
fn install_condition_on_failed_saves(
    encounter: &mut EncounterInstance,
    target_ids: &[usize],
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    use crate::engine::side_effects::ApplyCondition;
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    for &tid in target_ids {
        if encounter.roll_save(tid, save_ability, dc).passed() {
            continue;
        }
        effects.push(Box::new(ApplyCondition {
            actor_id: tid,
            condition,
            timer,
        }));
    }
    effects
}

/// LOS-gated burst-save-condition variant for gaze / glare / aura abilities
/// where bending a stare around a wall would be silly. Extends
/// `resolve_burst_save_condition` with two extra filters:
/// 1. Drops any target without line-of-sight to the caster (a gaze can't
///    bend around a corner — see Mummy / Mummy Lord Dreadful Glare).
/// 2. Optionally drops targets immune to a damage type — the canonical
///    proxy for "the undead are unaffected by this fear glare" (necrotic
///    immunity ≈ undead in this engine; see the Mummy / Mummy Lord glare
///    RAW gate).
///
/// Centralizes the recurring "enemy_burst_targets → LOS gate → immunity
/// gate → roll save → install on fail" loop that the Mummy / Mummy Lord
/// glares (and any future bodak-style stare ability) reimplement. The
/// `radius` is measured from the caster's footprint via
/// `enemy_burst_targets` — same chokepoint as the spell-burst lane, so
/// the LOS gate folds in cleanly without needing a separate point arg.
#[allow(clippy::too_many_arguments)]
pub fn resolve_los_glare_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
    skip_immune_to_damage: Option<DamageType>,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
        return Vec::new();
    };
    // Both filters run before any save is rolled, so a target the glare
    // can't reach never touches the dice — which keeps the roll sequence
    // (and therefore the seeded log) free of phantom saves for creatures
    // that were never in the glare's path.
    let target_ids: Vec<usize> = encounter
        .enemy_burst_targets(caster_id, caster_loc, radius)
        .into_iter()
        .filter(|&tid| {
            let immune = skip_immune_to_damage
                .is_some_and(|dt| encounter.actors.get(&tid).is_some_and(|a| a.is_immune_to(dt)));
            !immune && encounter.actor_has_line_of_sight(caster_id, tid)
        })
        .collect();
    install_condition_on_failed_saves(encounter, &target_ids, save_ability, dc, condition, timer)
}

/// Sweep targets in a `radius` burst centered on `point` and return their
/// ids in ascending current-HP order — a target whose current HP exceeds
/// the running pool stops the sweep (5e Sleep / Color Spray semantics).
/// `skip_immune_to` filters out actors immune to that condition (the
/// condition itself doesn't stack, so re-entry would be a no-op anyway —
/// this is just an early prune so the pool isn't burnt on no-ops).
///
/// Returns the ids in the order they should be touched; the caller is
/// responsible for queueing whatever side-effect (ApplyCondition, etc.).
pub fn pool_sweep_targets(
    encounter: &EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    pool: u32,
    skip_immune_to: crate::conditions::Condition,
) -> Vec<usize> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let mut candidates: Vec<(u32, usize)> = encounter
        .actors
        .iter()
        .filter_map(|(id, a)| {
            if *id == caster_id || !a.is_combat_active() {
                return None;
            }
            // Honor dynamic immunities too — a Fey Ancestry actor
            // dodges Sleep's pool sweep, a Halfling Brave dodges any
            // future fear-pool sweep. The condition can't install on
            // them anyway, so burning the pool's HP budget on a no-op
            // re-entry would be wasted.
            if a.effectively_immune_to_condition(skip_immune_to) {
                return None;
            }
            let dist = footprint_chebyshev(
                a.location(),
                get_tiles_from_size(a.size()),
                point,
                1,
            );
            if dist > radius {
                return None;
            }
            Some((a.hitpoints(), *id))
        })
        .collect();
    candidates.sort_unstable();

    let mut remaining = pool;
    let mut hit: Vec<usize> = Vec::new();
    for (hp, id) in candidates {
        if hp == 0 || hp > remaining {
            break;
        }
        remaining -= hp;
        hit.push(id);
    }
    hit
}

/// Convenience for `SingleActor` schemas: extract the first id from the
/// optional id list, returning `None` on empty / missing. Side-effect
/// builders use this so they can early-return cleanly when the engine
/// has no live target after validation.
pub fn first_target_id(ids: Option<&Vec<usize>>) -> Option<usize> {
    ids.and_then(|v| v.first().copied())
}

/// Symmetric helper for `SinglePoint` / `Burst` schemas: extract the first
/// `Coordinate` from the optional location list, returning `None` on
/// empty / missing. Mirrors `first_target_id` — point-target side-effect
/// builders (Fireball, Burning Hands, Sacred Burst, AoE-burst spells)
/// all collapse to a single `let Some(point) = first_target_location(tl)`
/// early-return instead of re-inlining `tl.and_then(|t| t.first().copied())`
/// at every call site. Same chokepoint benefit: a future change to how
/// SinglePoint args are surfaced lands in one place.
pub fn first_target_location(locations: Option<&Vec<Coordinate>>) -> Option<Coordinate> {
    locations.and_then(|v| v.first().copied())
}

/// Extract the first id from `target_ids` AND verify that actor is on the
/// caster's team. Returns `Some(id)` only when both legs pass — the
/// target exists in the arg vec and `actors_allied` returns true for the
/// `(caster, target)` pair. Returns `None` on either failure so the
/// caller's `side_effects` builder collapses to a single
/// `let Some(target_id) = first_ally_target_id(...) else { return Vec::new(); };`
/// guard instead of two stacked early-returns.
///
/// Used by ally-target buff spells (Aid, Longstrider, Enhance Ability)
/// where the `is_harmful = false` flag is the picker-UI hint and this
/// helper is the side-effect-time enforcement. Centralizing the gate
/// keeps the "buff cast on a hostile target should fizzle quietly"
/// invariant in one place, so future tweaks (e.g. a charmed-by gate)
/// land here once.
pub fn first_ally_target_id(
    encounter: &EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
) -> Option<usize> {
    let target_id = first_target_id(target_ids)?;
    if encounter.actors_allied(caster_id, target_id) {
        Some(target_id)
    } else {
        None
    }
}

/// Buff "don't double-cast" gate. Returns `true` when `actor_id` exists
/// in the encounter AND does NOT currently hold `condition`. Used by
/// self-buff spells (Spirit Shroud, Investiture of Flame / Ice / Stone /
/// Wind, Otherworldly Guise, Shadow of Moil, Ashardalon's Stride, Flame
/// Arrows) inside their `custom_validate_input` to avoid burning the
/// slot on a no-op refresh — `add_condition` keeps the longer of the two
/// timers, so re-casting while the buff is up just spends the slot for
/// nothing.
///
/// Also used by ally-target buff spells (Guidance → Inspired, Barkskin)
/// to short-circuit when the picked ally already has the buff up. The
/// helper takes any actor id so the same chokepoint serves both lanes —
/// self-buff (`actor_id == caster_id`) and ally-buff (`actor_id ==
/// target_id`).
///
/// Routes through the standard "actor missing → fail" shape so a
/// vanished caster / target fails the validate (matching every other
/// actor-touch gate in this file). Centralizes the recurring three-line
/// pattern:
///
/// ```ignore
/// encounter
///     .actors
///     .get(&caster_id)
///     .is_some_and(|a| !a.has_condition(Condition::SpiritShrouded))
/// ```
///
/// Single source of truth: future changes to the "don't refresh while
/// active" semantics (e.g. allowing refresh when the timer has < N
/// rounds left) land here once instead of being scattered across the
/// ~13 buff impls.
pub fn actor_lacks_condition(
    encounter: &EncounterInstance,
    actor_id: usize,
    condition: crate::conditions::Condition,
) -> bool {
    encounter
        .actors
        .get(&actor_id)
        .is_some_and(|a| !a.has_condition(condition))
}

/// Recharge-pool availability gate. Returns `true` when `actor_id` exists
/// in the encounter AND has `recharge_key` currently available (i.e. the
/// last d6 roll at start-of-turn ticked it back on). Symmetric to
/// `actor_lacks_condition` — same "actor missing → fail" shape — but for
/// the recharge-pool resource lane.
///
/// Centralizes the recurring three-line pattern that every recharge-gated
/// action (`AndrosphinxRoar`, `BreathWeapon`, `Whelm`, `EttercapWeb`,
/// `BlinkDogTeleport`, `WaterJet`, `StoneSnare`, `MammothTramplingCharge`,
/// `HorrorNimbus`, `DretchFetidCloud`, `UnicornHealingTouch`):
///
/// ```ignore
/// encounter
///     .actors
///     .get(&caster_id)
///     .is_some_and(|a| a.is_recharge_available("breath_weapon"))
/// ```
///
/// Single source of truth: future tweaks to recharge semantics (e.g.
/// "Magic Resistance also halves recharge ticks") land here once instead
/// of being scattered across the ~13 `custom_validate_input` impls that
/// gate on recharge availability today.
pub fn actor_has_recharge(
    encounter: &EncounterInstance,
    actor_id: usize,
    recharge_key: &str,
) -> bool {
    encounter
        .actors
        .get(&actor_id)
        .is_some_and(|a| a.is_recharge_available(recharge_key))
}

/// `SingleActor` `custom_validate_input` gate for "this swing only lands
/// against a target who already has condition X." Returns `true` when the
/// first id in `target_ids` resolves to a live actor holding `condition`;
/// fails closed on missing target / missing actor (same convention as
/// `actor_has_recharge`).
///
/// Centralizes the recurring three-line pattern that every "I can only
/// hit you if you're already down" combat clause reimplements — Mammoth
/// Stomp (Prone), future Coup-de-Grace / pinned-only stinger / sleeper-
/// throat-slit variants. Single source of truth so a future tweak
/// ("Mind-Blanked targets are immune to this prone-gate exception",
/// etc.) lands once instead of across N inlined gates.
pub fn target_has_condition(
    encounter: &EncounterInstance,
    target_ids: Option<&Vec<usize>>,
    condition: crate::conditions::Condition,
) -> bool {
    let Some(target_id) = first_target_id(target_ids) else {
        return false;
    };
    encounter
        .actors
        .get(&target_id)
        .is_some_and(|a| a.has_condition(condition))
}

/// Standard leveled-spell cost shape: one Action plus a level-`lvl` slot.
/// Used by ~60 leveled-spell impls; the helper keeps the cost block to
/// one line at the call site and gives us a single chokepoint for any
/// future cross-cutting change (e.g. a "verbal-component blocked while
/// Silenced" gate would slot in here).
pub fn action_and_slot(lvl: u32) -> Vec<Resource> {
    vec![Resource::Action, Resource::SpellSlot(lvl)]
}

/// Bonus-action variant of `action_and_slot` for quickened-style spells
/// (Healing Word, Mass Healing Word, Healing Spirit, Sanctuary, etc.).
/// Same chokepoint benefit as the Action variant.
pub fn bonus_action_and_slot(lvl: u32) -> Vec<Resource> {
    vec![Resource::BonusAction, Resource::SpellSlot(lvl)]
}

/// Bonus-action-only cost — no spell slot, no other resource. Used by
/// class features that fire as a bonus action without a slot (Rage,
/// Cunning Dash / Disengage / Hide, Bardic Inspiration, Divine Smite's
/// bonus action portion of the cost, etc.). Centralizes the
/// `vec![Resource::BonusAction]` literal so a future cross-cutting
/// change (e.g. "all bonus actions provoke an opportunity attack")
/// can land in one place.
pub fn bonus_action_only() -> Vec<Resource> {
    vec![Resource::BonusAction]
}

/// Action-only cost — one Action, no spell slot. Symmetric counterpart to
/// `bonus_action_only` for slot-less Action-cost consumables and class
/// features (e.g. the `BurstSaveDamageItem` scrolls, `Drink Potion of X`
/// at Action cost). The matching chokepoint half of the toggle below.
pub fn action_only() -> Vec<Resource> {
    vec![Resource::Action]
}

/// Boolean-gated picker for the two main-slot action costs: bonus action
/// when `true`, full Action when `false`. Used by the `SelfHealItem` /
/// `SelfConditionItem` / `SingleTargetHealItem` factor structs so the
/// per-impl `cost()` block collapses to a one-liner. Centralizes the
/// "bonus action OR action, no spell slot" toggle every consumable-with-
/// configurable-action-economy item rides.
pub fn action_or_bonus_only(bonus_action: bool) -> Vec<Resource> {
    if bonus_action {
        bonus_action_only()
    } else {
        action_only()
    }
}

/// Free action — no resource cost at all. Used by Action Surge,
/// Indomitable, etc. — features that don't consume action economy
/// directly. The empty vec lives behind a name so call sites read
/// `free_cost()` rather than `Vec::new()` and the intent is obvious.
pub fn free_cost() -> Vec<Resource> {
    Vec::new()
}

/// Reaction-only cost — one Reaction, nothing else. The third member of
/// the `action_only` / `bonus_action_only` family, and the one that had
/// no callers until the Circle of Spores Druid's Halo of Spores arrived.
///
/// Everything reactive in the engine before it was *dispatched* — an
/// opportunity attack, Uncanny Dodge, Parry, Bend Luck — fired from
/// inside a trigger site that spent the slot itself. Halo of Spores is
/// the first reaction a player *declares*: RAW lets the druid spend it
/// on their own turn or anyone else's, and the engine's turn model only
/// hands the player a decision point on their own turn, so what makes it
/// work is that the reaction slot is a resource distinct from the Action
/// and Bonus Action ones. A druid who spends it on their turn has
/// genuinely given up their Uncanny-Dodge-equivalent for the round,
/// which is the real cost RAW charges.
pub fn reaction_only() -> Vec<Resource> {
    vec![Resource::Reaction]
}

pub enum TargetingSchema {
    NoArgs,
    SinglePoint,
    SingleActor,
    /// Target a single tile; the action's effect applies to every actor
    /// whose footprint lies within `radius` tile-gap of the point. Used
    /// by AoE spells (Fireball, Burning Hands, Sacred Burst). The radius
    /// is in tile-gap units consistent with `footprint_chebyshev` — 0 is
    /// the point itself, 1 includes the 8 surrounding tiles, etc.
    Burst {
        radius: isize,
    },
    Custom,
}

/// The shared body of `Action::expected_damage` for a weapon swing:
/// average damage on one swing, times the number of swings the action
/// actually makes.
///
/// Every weapon chassis in the engine — `SimpleWeapon`, `RogueWeapon`,
/// `BeastNaturalWeapon` — resolves its swing the same way and chains
/// Extra Attack through the same helper, so they estimate through one
/// function rather than three copies that can drift on which swings
/// they counted.
///
/// `extra_swings` is for the forms that chain beyond Extra Attack (the
/// Beast Barbarian's claws add one), and `cost_resource` is what gates
/// the Extra Attack multiplier: RAW hangs it off the Attack action, so a
/// bonus-action bow shot or a reaction strike swings once however many
/// attacks the actor's Action buys.
pub fn weapon_expected_damage(
    encounter: &EncounterInstance,
    caster_id: usize,
    dice: crate::engine::dice::Dice,
    damage_ability: Option<AbilityScoreType>,
    cost_resource: Resource,
    extra_swings: u32,
) -> Option<f32> {
    let caster = encounter.actors.get(&caster_id)?;
    let per_swing = dice.average_roll()
        + damage_ability
            .map(|a| caster.ability_modifier(a) as f32)
            .unwrap_or(0.0);
    let extra_attack =
        u32::from(cost_resource == Resource::Action && caster.has_extra_attack());
    let swings = 1 + extra_attack + extra_swings;
    // A damage modifier deep enough to zero a swing (a Strength penalty
    // on a small die) shouldn't make the weapon read as *negative* and
    // sort below an unannotated action; the floor is what keeps the hint
    // monotone in the die size.
    Some((per_swing * swings as f32).max(0.0))
}

pub trait Action {
    fn name(&self) -> &str;

    fn aliases(&self) -> Vec<&str>;

    fn targeting_schema(&self) -> TargetingSchema;

    /// Maximum footprint-Chebyshev gap from caster to target for this action
    /// to be valid. `None` disables the spatial check (non-targeted actions
    /// or actions that do their own range logic). Used both by the engine
    /// (validation) and by the UI (target picker filters by reach).
    fn reach_tiles(&self) -> Option<isize> {
        None
    }

    /// Whether this action requires unobstructed line-of-sight from caster
    /// to target. True for ranged attacks and most spells; false for melee
    /// (you have to be touching). Walls block, actors don't.
    fn requires_los(&self) -> bool {
        false
    }

    /// True for hostile actions targeting enemies (default). Buffing or
    /// healing actions override to false so the AI's support pipeline
    /// excludes them from heal-target consideration.
    fn is_harmful(&self) -> bool {
        true
    }

    /// True if this action's primary effect is HP loss on the target
    /// (default). Hostile control actions like Shove return false so
    /// the AI's focus-fire pipeline doesn't pick them over attacks that
    /// actually whittle down enemy HP.
    fn deals_damage(&self) -> bool {
        true
    }

    /// True if this action restores HP / temp HP on its target.
    /// Used by the AI's support pipeline (heal-the-lowest target).
    fn is_heal(&self) -> bool {
        false
    }

    /// True if this action's load-bearing effect is putting new
    /// friendly bodies on the board — Conjure Animals, Conjure
    /// Elemental, Animate Dead, Animate Objects, the Ranger's
    /// Companion.
    ///
    /// Exists because summons fit none of the AI's existing lanes and
    /// were therefore invisible to it. They are `is_harmful` (they end
    /// fights faster) but hurt nobody directly; they `deals_damage`
    /// only through a creature that doesn't exist yet; they declare no
    /// `damage_types`; and their targeting schema is `NoArgs`, so the
    /// burst picker's "harmful NoArgs with a damage type or an explicit
    /// no-damage flag" filter excluded every one of them. The result
    /// was that no AI-driven caster ever summoned anything, on any
    /// template, at any point — a whole category of spell that existed
    /// only for a human player to type.
    ///
    /// A trait method rather than a name list in the AI for the reason
    /// the metamagic gates already give: the next summon spell should
    /// be picked up by declaring what it is, not by someone remembering
    /// to add a string somewhere else.
    fn summons_allies(&self) -> bool {
        false
    }

    /// Damage types this action can deal (for actor-side resistance /
    /// immunity hints in the prompt UI). Empty for non-damaging actions
    /// or those whose typing depends on runtime data.
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }

    /// Roughly how much damage one *use* of this action lands on a
    /// single target, if the action can say. `None` means "no estimate"
    /// and is the default.
    ///
    /// Read by the AI's attack picker (`best_attack_against`), which
    /// used to rank candidate swings by damage-type matchup and reach
    /// and then stop — so two melee weapons that were neutral against
    /// the target and had the same reach were separated by nothing but
    /// their order in the actor's action list. A Knight swung whichever
    /// of its two weapons happened to be pushed first. A Beast
    /// Barbarian's claws, which land three swings a turn to a
    /// greataxe's two, were never picked at all.
    ///
    /// "One use" means the whole Action, chained swings included, which
    /// is the only unit the picker can compare: Extra Attack multiplies
    /// most weapons by two and the Beast Barbarian's claws by three, and
    /// a per-swing number would hide exactly that difference. Riders
    /// that depend on the target (Sneak Attack's eligibility, a smite
    /// prime, Colossus Slayer's wounded-target gate) are deliberately
    /// left out: they apply to whichever weapon is chosen, so including
    /// them would move every estimate by the same amount and change no
    /// comparison.
    ///
    /// An estimate, and only ever an estimate. It does not model
    /// accuracy, crits, resistance (the matchup score already outranks
    /// it) or anything the die does after it is rolled.
    ///
    /// The picker only lets the estimate decide a tie when *both* sides
    /// return `Some`. Most attacks in the bestiary are bespoke `impl
    /// Action` blocks that roll their dice inline and have nothing to
    /// declare here; ranking a `None` as zero would sort every one of
    /// them below every annotated weapon, which is not a better ordering
    /// than the declaration order it replaced — just a different
    /// arbitrary one. So the hint is strictly additive: a pair where
    /// either side declines is ordered exactly as it was before this
    /// existed.
    ///
    /// The one place that makes it a *requirement* rather than a nicety
    /// is a wrapper action — `Multiattack`, `CompoundAttack` — that sits
    /// on the same action list as the swings it contains. Those must
    /// aggregate their parts, because a wrapper that returned `None`
    /// against a sub-attack that returns `Some` would be ranked by
    /// declaration order against something strictly worse than itself.
    fn expected_damage(&self, _encounter: &EncounterInstance, _caster_id: usize) -> Option<f32> {
        None
    }

    /// The 5e school of magic this action belongs to, or `None` for
    /// anything that isn't a spell (weapon / monster attacks, class
    /// features, item actions) — and for spell impls whose school no
    /// consumer reads yet.
    ///
    /// Read at the post-cast chokepoint in `execute` and threaded into
    /// `EncounterInstance::dispatch_post_cast_triggers`, so a
    /// school-keyed feature (Arcane Ward's "cast an abjuration spell of
    /// 1st level or higher" recharge, Empowered Evocation's damage
    /// bump, Sculpt Spells' ally shield) reads one value instead of
    /// pattern-matching spell names. Also read directly by the
    /// pre-damage hooks that need the school *before* side-effects are
    /// built (Sculpt Spells at burst-target time).
    ///
    /// Defaults to `None` so every non-spell action and every untagged
    /// spell fails school gates closed. Tagging a spell is a two-line
    /// override next to `damage_types`.
    fn school(&self) -> Option<crate::engine::types::SpellSchool> {
        None
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>>;

    /// All resources this action consumes when executed. Most actions
    /// have one cost (just an Action slot, just a Bonus Action, etc.);
    /// leveled spells return multiple (`[Action, SpellSlot(2)]` for a
    /// level-2 spell, `[BonusAction, SpellSlot(1)]` for a quickened
    /// healing word). Empty vec = free (e.g. Skip). All costs are
    /// validated together; the action only fires if the actor can
    /// afford every entry.
    ///
    /// Default: `[Action]` — the most common case (single-Action attacks
    /// and most cantrips). Override for free actions, bonus-action
    /// attacks, leveled spells, and movement-priced actions.
    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }

    fn validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // First gate: did the caller pass argument shapes consistent with
        // the action's declared schema? Each branch returns true on a
        // legal shape and false otherwise. Custom schemas opt out and
        // delegate everything to `custom_validate_input` below.
        let schema_ok = match self.targeting_schema() {
            TargetingSchema::NoArgs => {
                target_ids.is_none() && target_locations.is_none() && overrides.is_none()
            }
            TargetingSchema::SinglePoint => {
                target_ids.is_none()
                    && target_locations.is_some_and(|tl| tl.len() == 1)
            }
            TargetingSchema::SingleActor => {
                target_locations.is_none()
                    && target_ids.is_some_and(|ids| !ids.is_empty())
            }
            TargetingSchema::Burst { .. } => {
                target_ids.is_none()
                    && target_locations.is_some_and(|tl| tl.len() == 1)
            }
            TargetingSchema::Custom => true,
        };
        if !schema_ok {
            return false;
        }
        // Can the caster cast at all? `Silenced` and `WildShaped` both
        // say no in RAW without qualification, but the only gate that
        // used to enforce it lived on the `SpellSlot` resource lane —
        // which cantrips never touch, so a silenced caster could Fire
        // Bolt and a wild-shaped druid could Poison Spray. A `school()`
        // is the engine's marker for "this action is a spell" (same one
        // `CastContext::is_cantrip` reads, kept complete for cantrips by
        // `every_cantrip_declares_its_school`), so the check belongs
        // here at the action layer where cantrips are visible.
        if self.school().is_some()
            && encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.blocked_from_casting())
        {
            return false;
        }
        // Reach + LOS check. SingleActor measures from caster to target
        // actor; Burst / SinglePoint measure from caster to the target
        // tile. Either way we check both reach (if declared) and LOS
        // (if required).
        if let Some(targets) = target_ids
            && let Some(&target_id) = targets.first()
        {
            if let Some(reach) = self.reach_tiles() {
                let Some(dist) = encounter.footprint_distance(caster_id, target_id) else {
                    return false;
                };
                // Lunging Attack and similar reach-extending primes add
                // tiles via `extra_melee_reach`, which itself gates the
                // bonus to melee envelopes so ranged actions are
                // unaffected. Distant Spell (sorcerer metamagic) doubles
                // the reach of ranged-only actions via `extra_spell_reach`.
                let bonus = encounter
                    .actors
                    .get(&caster_id)
                    .map(|a| a.extra_reach(reach))
                    .unwrap_or(0);
                if dist > reach + bonus {
                    return false;
                }
            }
            if self.requires_los() && !encounter.actor_has_line_of_sight(caster_id, target_id) {
                return false;
            }
            // 5e Charmed: "a charmed creature can't attack the charmer
            // or target the charmer with harmful abilities or magic
            // effects." Checked across *every* declared target, not just
            // the first: the reach / LOS clauses above are deliberately
            // first-target-only (a multi-target action measures its
            // envelope off its primary), but "don't target the charmer"
            // binds on each name in the list independently, so a
            // multi-target harmful action naming the charmer second
            // would otherwise slip through. Shared gate with the
            // opportunity-attack and Riposte dispatchers, which reach
            // hostility without passing through validation at all.
            if self.is_harmful()
                && targets
                    .iter()
                    .any(|&tid| encounter.charm_blocks_hostility(caster_id, tid))
            {
                return false;
            }
        } else if let Some(locs) = target_locations
            && let Some(&point) = locs.first()
        {
            if let Some(reach) = self.reach_tiles() {
                let Some(dist) = encounter.footprint_distance_to_point(caster_id, point) else {
                    return false;
                };
                // Distant Spell prime extends the range for ranged-only
                // actions (point-target AoEs like Fireball, Sleet Storm).
                let bonus = encounter
                    .actors
                    .get(&caster_id)
                    .map(|a| a.extra_spell_reach(reach))
                    .unwrap_or(0);
                if dist > reach + bonus {
                    return false;
                }
            }
            if self.requires_los()
                && !encounter.actor_has_line_of_sight_to_point(caster_id, point)
            {
                return false;
            }
        }
        // Check every declared cost; the action only fires if the actor
        // can afford all of them.
        let costs = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        if !costs.is_empty() {
            let Some(actor) = encounter.actors.get(&caster_id) else {
                return false;
            };
            for c in &costs {
                if !actor.can_consume_resource(*c) {
                    return false;
                }
            }
        }
        self.custom_validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        )
    }

    fn custom_validate_input(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        true
    }

    fn execute(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !self.validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ) {
            // The action became invalid between enqueue and execute (e.g. target died,
            // resource was consumed elsewhere). Skip silently; the engine logs context.
            return Vec::new();
        }
        // 5e Sorcerer Distant Spell metamagic: a ranged action burns the
        // prime as it fires. Gated on reach > 2 so a melee swing or
        // polearm-reach attack can't consume the prime — matches the
        // `extra_spell_reach` gate. Consumed here (before side_effects)
        // so the prime can't double-fire on a multi-target spell or be
        // observed by the spell's own logic in any surprising way.
        if let Some(reach) = self.reach_tiles()
            && reach > 2
        {
            encounter.consume_distant_spell(caster_id);
        }
        // Open the cast frame before any of the action's own logic runs,
        // so every resolution site nested inside `side_effects` — burst
        // save loops, shared damage rolls, ally-shield sweeps — can read
        // the school and slot level off `encounter.current_cast()`
        // instead of taking two more parameters from every helper
        // between here and there. The slot level is sniffed off the
        // resolved cost the same way the post-cast trigger dispatch
        // below does it; `cost` takes `&EncounterInstance`, so resolving
        // it early is side-effect free.
        //
        // Closed by `exit_cast` after the Twinned Spell block, which
        // re-enters `side_effects` for the twin and must see the same
        // frame. Everything past that point (cost resolution, post-cast
        // triggers, the ConsumeResource tail) is "after the spell" per
        // RAW and deliberately sits outside the frame.
        let cast_level = crate::engine::side_effects::spell_slot_level(&self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ))
        .unwrap_or(0);
        encounter.enter_cast(self.school(), cast_level);
        let mut side_effects = self.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        // 5e Sorcerer Extended Spell metamagic: if the caster has the
        // prime up and the action installs at least one long-duration
        // condition (Rounds(n) with n >= 10), double the timer in place
        // on every eligible side-effect and burn the prime. Run before
        // Twinned Spell — the `extended_for_twin` flag we capture here
        // propagates into the twin's separately-built side_effects so a
        // Twinned + Extended cast lands the doubled duration on both
        // targets RAW (the prime affects the *spell*, not just the
        // primary target). Done here rather than per-spell to spare each
        // spell impl from carrying the metamagic branch through its
        // side_effects builder.
        let extended_for_twin = encounter.consume_extended_spell(caster_id, &mut side_effects);
        // 5e Tasha's Sorcerer Transmuted Spell metamagic: if the caster has
        // the prime up and the cast carries at least one elemental
        // (acid / cold / fire / lightning / poison / thunder) DealDamage,
        // remap every eligible damage type to the primary target's worst
        // weakness and burn the prime. Returned type propagates to the
        // twin's separately-built side_effects so a Twinned + Transmuted
        // cast lands the same remap on both targets RAW.
        let transmuted_for_twin =
            encounter.consume_transmuted_spell(caster_id, &mut side_effects);
        // 5e Sorcerer Twinned Spell metamagic: re-fire the action's
        // side_effects against a second target if the prime is up and the
        // caster can afford the SP cost (max(1, spell_level)). Gated to
        // SingleActor + exactly-one-target shape; the action's cost lane
        // is paid once total — the metamagic doesn't burn a second spell
        // slot, only sorcery points (debited inside the helper). We sniff
        // the spell level off the resolved cost (`SpellSlot(lvl)` entry;
        // cantrips have no SlotCost and default to 1 SP). Multi-target
        // calls (e.g. spells that pass a vec of ids) skip the gate
        // because "twinning" them is ill-defined.
        if matches!(self.targeting_schema(), TargetingSchema::SingleActor)
            && let Some(ids) = target_ids
            && ids.len() == 1
        {
            let original_target_id = ids[0];
            let costs = self.cost(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            );
            // Sniff the slot level off the resolved cost so the SP debit
            // scales with the cast (cantrips → 1 SP, lvl-3 spell → 3 SP,
            // etc.). RAW floor is 1 SP — cantrips have no `SpellSlot`
            // entry, which the helper returns as `None`.
            let sp_cost = crate::engine::side_effects::spell_slot_level(&costs)
                .unwrap_or(1)
                .max(1);
            // They all resolve through the same twin block below
            // because the *effect* is identical — re-run the action's
            // side-effects against one more id — and they differ only in
            // the gate.
            //
            // Three features double a single-target cast, and they are
            // offered paid-first: the Sorcerer's Twinned Spell (a
            // consumable prime charging sorcery points, any
            // single-target spell), then the Enchantment Wizard's Split
            // Enchantment (free, leveled enchantments only), then the
            // Death Domain Cleric's Reaper (free, necromancy cantrips
            // only, and the second target must stand beside the first).
            //
            // The ordering is what keeps a caster holding more than one
            // from doubling twice, and it errs the right way: a sorcerer
            // would rather spend nothing, but the prime is already up
            // and would otherwise sit unspent across a cast it was
            // declared for. The two free passives cannot collide with
            // each other — one is leveled-only and the other
            // cantrip-only.
            let second_target = encounter
                .consume_twinned_spell(
                    caster_id,
                    self.name(),
                    self.is_harmful(),
                    self.reach_tiles(),
                    self.requires_los(),
                    sp_cost,
                    original_target_id,
                )
                .or_else(|| {
                    encounter.consume_split_enchantment(
                        caster_id,
                        self.name(),
                        self.school(),
                        crate::engine::side_effects::spell_slot_level(&costs).unwrap_or(0),
                        self.is_harmful(),
                        self.reach_tiles(),
                        self.requires_los(),
                        original_target_id,
                    )
                })
                .or_else(|| {
                    encounter.consume_reaper(
                        caster_id,
                        self.name(),
                        self.school(),
                        crate::engine::side_effects::spell_slot_level(&costs).unwrap_or(0),
                        self.is_harmful(),
                        self.reach_tiles(),
                        self.requires_los(),
                        original_target_id,
                    )
                });
            if let Some(twin_id) = second_target {
                let twin_targets = vec![twin_id];
                let mut twin_effects = self.side_effects(
                    encounter,
                    caster_id,
                    Some(&twin_targets),
                    target_locations,
                    overrides,
                );
                // Propagate the Extended Spell doubling to the twin's
                // side_effects: the prime was consumed on the original
                // cast (so we don't re-check it here), but Extended
                // affects the *spell* — both twin targets get the
                // doubled duration RAW.
                if extended_for_twin {
                    crate::engine::side_effects::extend_side_effect_timers(&mut twin_effects);
                }
                // Propagate the Transmuted Spell remap to the twin's
                // side_effects. Same shape as Extended: the prime is
                // already consumed, but the spell-level remap still
                // applies to both twin targets RAW.
                if let Some(new_type) = transmuted_for_twin {
                    crate::engine::side_effects::remap_side_effect_damage_types(
                        &mut twin_effects,
                        new_type,
                    );
                }
                // One cast is one spell and one concentration. Without
                // this fold the twin's own `StartConcentration` would
                // land second and its `drop_concentration` prologue
                // would rip the effect straight back off the first
                // target — see `fold_doubled_concentration`.
                crate::engine::side_effects::fold_doubled_concentration(
                    &mut side_effects,
                    &mut twin_effects,
                );
                side_effects.append(&mut twin_effects);
            }
        }
        // Close the cast frame: the spell's own effects (original and
        // twin) are fully built. Everything below resolves "after the
        // spell" per RAW and must not read as part of it.
        encounter.exit_cast();
        let costs = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        // Post-cast trigger dispatch: Wild Magic Surge, Heart of the
        // Storm eruption, and any future post-cast hook all fire from
        // the encounter-side dispatcher against the resolved spell
        // context. Sniff the spell-slot level off the resolved cost —
        // cantrips and non-spell actions have no `SpellSlot` cost
        // entry (spell_level==0), and each hook's own gate
        // short-circuits on the non-caster / non-sorcerer / wrong-
        // damage-type paths. Trigger effects sit between the spell's
        // effects and the cost-consume effects so they resolve "after
        // the spell" per RAW.
        let spell_level = crate::engine::side_effects::spell_slot_level(&costs).unwrap_or(0);
        let damage_types = self.damage_types();
        let mut post_cast_effects = encounter.dispatch_post_cast_triggers(
            caster_id,
            spell_level,
            &damage_types,
            self.school(),
            target_ids,
        );
        side_effects.append(&mut post_cast_effects);
        for cost in costs {
            side_effects.push(Box::new(ConsumeResource {
                actor_id: caster_id,
                resource: cost,
            }));
        }
        side_effects
    }
}

pub struct ActionExecutionInfo {
    action: &'static dyn Action,
    caster_id: usize,
    target_ids: Option<Vec<usize>>,
    target_locations: Option<Vec<Coordinate>>,
    overrides: Option<HashSet<ActionOverride>>,
}

impl ActionExecutionInfo {
    pub fn new(
        action: &'static dyn Action,
        caster_id: usize,
        target_ids: Option<Vec<usize>>,
        target_locations: Option<Vec<Coordinate>>,
        overrides: Option<HashSet<ActionOverride>>,
    ) -> Self {
        Self {
            action,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        }
    }

    pub fn action(&self) -> &'static dyn Action {
        self.action
    }

    pub fn caster_id(&self) -> usize {
        self.caster_id
    }

    pub fn target_ids(&self) -> Option<&[usize]> {
        self.target_ids.as_deref()
    }

    pub fn target_locations(&self) -> Option<&[Coordinate]> {
        self.target_locations.as_deref()
    }

    /// Resolved costs of this specific invocation (uses the stored
    /// target/loc args, not None placeholders) — useful for the engine
    /// log filter and the UI's cost-label / affordability display.
    pub fn cost(&self, encounter: &EncounterInstance) -> Vec<Resource> {
        self.action.cost(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }

    pub fn validate(&self, encounter: &EncounterInstance) -> bool {
        self.action.validate_input(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }

    pub fn execute(&self, encounter: &mut EncounterInstance) -> Vec<Box<dyn ApplicableSideEffect>> {
        self.action.execute(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }
}
