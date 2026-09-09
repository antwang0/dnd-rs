use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride,
    areas::AreaShape,
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
/// Walks `target_ids` in the given order, credits each target with 5e
/// **cover** measured from `origin` when the save is a Dexterity one,
/// rolls a caster-aware save (so Sorcerer Heightened Spell forces
/// disadvantage on the *first* save in the burst per RAW), applies the
/// caster-side and target-side damage modifiers at the shared
/// chokepoint (Potent Cantrip, Rogue / Monk / Ranger Evasion), and
/// emits a `DealDamage` side-effect for every non-zero hit.
///
/// `origin` is the area's point of origin, which is where RAW measures
/// its cover from — not the caster's tile, and not the target's. A
/// blast that goes off on the near side of a low wall gives the
/// creature behind it half cover; one centred past the wall gives it
/// nothing, because the wall is no longer between them. Ids in `shielded` (Sorcerer Careful Spell / Evocation
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
    origin: Coordinate,
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
        // 5e Cover, the half of the rule the engine only had one of.
        // "A target with half cover has a +2 bonus to AC **and
        // Dexterity saving throws**"; three-quarters cover is +5 to
        // both. The AC half has been on every attack roll since
        // `cover_ac_bonus` existed and the save half was on nothing, so
        // an archer behind a low wall was harder to shoot and exactly
        // as easy to Fireball as one standing in the open.
        //
        // Measured from the burst's own origin, which is where RAW
        // measures an area's cover from, and spent as a DC reduction
        // rather than as a bonus to the roll — the same arithmetic, and
        // it keeps the whole save-mode / rider stack in
        // `roll_save_against_caster` untouched.
        //
        // Dexterity only. RAW's sentence names that one ability, and
        // it is the right one: a wall you can duck behind does nothing
        // about a Constitution save against poison gas that has already
        // filled the room.
        let cover = if matches!(save_ability, AbilityScoreType::Dexterity) {
            encounter.cover_bonus_from_point(origin, caster_id, target_id)
        } else {
            0
        };
        if cover > 0 {
            let name = encounter.actor_name(target_id);
            let note = EncounterInstance::cover_log_suffix(cover);
            encounter.log(format!("  {} saves at DC {}{}", name, dc - cover, note));
        }
        // Route through the caster-aware save helper so the 5e Sorcerer
        // Heightened Spell metamagic forces disadvantage on the *first*
        // save in the burst (RAW). Subsequent targets in the same cast
        // fall through to the normal save path — `roll_save_against_caster`
        // consumes the prime on its first call.
        let save =
            encounter.roll_save_against_caster(target_id, save_ability, dc - cover, caster_id);
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
    resolve_area_save_damage(
        encounter,
        caster_id,
        AreaShape::Burst { radius },
        center,
        save_ability,
        dc,
        damage,
        damage_type,
    )
}

/// `resolve_burst_save_damage` for an area of any shape — the friend-or-
/// foe lane a cone or a line resolves through.
///
/// Every clause the burst version had is a clause about *areas* rather
/// than about spheres: allies shielded out of the blast, the caster
/// excluded, cover measured from where the effect came from. So the body
/// moved here whole and the burst entry point above is the one-line
/// wrapper, which is the right way round — a Cone of Cold that skipped
/// Careful Spell would be a bug nobody would think to look for.
#[allow(clippy::too_many_arguments)]
pub fn resolve_area_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    shape: AreaShape,
    aim: Coordinate,
    save_ability: AbilityScoreType,
    dc: i32,
    damage: u32,
    damage_type: DamageType,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // Shielded allies in the area auto-pass the save AND take 0 damage
    // (Sorcerer Careful Spell, Evocation Wizard Sculpt Spells). Resolved
    // up-front so the shared per-target loop can skip them cleanly; the
    // Careful Spell prime is consumed inside the helper.
    let shielded = encounter.auto_pass_shielded_allies_in(caster_id, shape, aim);
    // `neutral_area_targets` shares the "caster-excluded, combat-active,
    // footprint inside the shape" filter with the rest of the engine —
    // folding it here keeps the caster-exclusion / geometry / sorted-ids
    // invariant in one place instead of re-inlining the loop.
    let target_ids = encounter.neutral_area_targets(caster_id, shape, aim);
    resolve_burst_targets(
        encounter,
        caster_id,
        encounter.area_origin(caster_id, shape, aim),
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
    resolve_enemy_area_save_damage(
        encounter,
        caster_id,
        AreaShape::Burst { radius },
        center,
        save_ability,
        dc,
        damage,
        damage_type,
        policy,
    )
}

/// `resolve_enemy_burst_save_damage` for an area of any shape. See
/// `resolve_area_save_damage` for why the shape-general version is the
/// body and the burst one is the wrapper.
#[allow(clippy::too_many_arguments)]
pub fn resolve_enemy_area_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    shape: AreaShape,
    aim: Coordinate,
    save_ability: AbilityScoreType,
    dc: i32,
    damage: u32,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // Enemy areas skip allies at the target-list step, so the
    // ally-shield sweep has nothing left to spare — pass an empty set
    // through instead of re-running the lookup.
    let shielded: HashSet<usize> = HashSet::new();
    let target_ids = encounter.enemy_area_targets(caster_id, shape, aim);
    resolve_burst_targets(
        encounter,
        caster_id,
        encounter.area_origin(caster_id, shape, aim),
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

/// `resolve_enemy_burst_save_damage`, with the dice rolled and the
/// effects applied on the spot.
///
/// Every other caller of the `resolve_*` family is an `Action`, which
/// hands its effects back so the turn's side-effect stack can run them
/// in order. The layers that fire *outside* anybody's turn —
/// `engine::lair_actions`, `engine::legendary_actions`, the reaction
/// dispatcher — have no stack to queue onto and no turn to be part of,
/// so each of them wrapped this call in the same three lines: roll the
/// dice once, resolve, drain the vec.
///
/// Written down here rather than twice more out there for the reason
/// `install_condition_on_failed_saves` gives one screen up: a loop
/// written twice is a rule that can be fixed in one place and stay
/// broken in the other. The single roll is the part worth centralizing
/// — a burst rolls its dice once and bills every target from the same
/// number, and a caller that rolled inside the loop would be a
/// different rule that looked like this one.
#[allow(clippy::too_many_arguments)]
pub fn apply_enemy_burst_save_damage(
    encounter: &mut EncounterInstance,
    owner_id: usize,
    center: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: crate::engine::dice::Dice,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
) {
    let damage = encounter.roll(&dice);
    let effects = resolve_enemy_burst_save_damage(
        encounter,
        owner_id,
        center,
        radius,
        save_ability,
        dc,
        damage,
        damage_type,
        policy,
    );
    for effect in effects {
        effect.apply(encounter);
    }
}

/// `resolve_burst_save_condition`, applied on the spot. The condition
/// half of `apply_enemy_burst_save_damage`, and there for the same
/// callers and the same reason — see its docstring.
#[allow(clippy::too_many_arguments)]
pub fn apply_burst_save_condition(
    encounter: &mut EncounterInstance,
    owner_id: usize,
    center: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
) {
    let effects = resolve_burst_save_condition(
        encounter,
        owner_id,
        center,
        radius,
        save_ability,
        dc,
        condition,
        timer,
    );
    for effect in effects {
        effect.apply(encounter);
    }
}

/// Reach for melee/touch actions, expressed as a footprint-Chebyshev gap cap.
/// 5e melee weapons are 5ft = 1-tile gap in this 2.5ft grid. Polearms /
/// reach weapons would be 2. Ranged actions return their max range here.
pub const MELEE_REACH: isize = 1;

/// The widest gap a swing can still be a *melee* swing across, for the
/// actions that have to be guessed at.
///
/// `MELEE_REACH` is the reach of an ordinary weapon; this is a band
/// wide enough for the reach weapons — the ogre's greatclub and the
/// hill giant's at 2, the wyvern's stinger and the storm giant's
/// greatsword at 3, the Tarrasque's bite and tail sweep at 4.
///
/// It is *not* the reach of the longest arm in the bestiary, and the
/// docstring used to say it was. The kraken's tentacle reaches six, so
/// for as long as both existed the band called the engine's heaviest
/// melee thresher a shooter — see `SimpleWeapon::is_melee_attack` for
/// what that cost. A band can only ever be a guess about where swinging
/// stops, and the guess was already stale when it was written down as a
/// fact.
///
/// Which is why the band is no longer the primary answer.
/// `SimpleWeapon` — every shared weapon static in the bestiary —
/// declares `is_melee_attack` from the same `is_melee` field the die is
/// handed, so no reach can put it wrong. What is left here is the
/// fallback for bespoke `impl Action` blocks, which have nothing else
/// to go on. A future one that swings from further out than four tiles
/// should override the method rather than widen this number.
///
/// The two exist separately because a great deal of code wants to ask
/// "is this creature in melee with that one" and had only `MELEE_REACH`
/// to ask it with, which quietly answered *no* for every reach weapon
/// on the roster. That produced two visible bugs — an ogre whose
/// opportunity attack was a shove because its greatclub did not count
/// as a melee weapon, and a caster who did not register a giant
/// standing over it as a melee threat because the giant was two tiles
/// away rather than one.
///
/// Callers pair this with `Action::requires_los`, which is the engine's
/// existing melee/ranged marker — a melee swing needs contact rather
/// than sight, so it declares `false`, and every ranged attack declares
/// `true`. The pair is deliberately conservative from both directions:
/// the flag alone would admit a touch-range spell with a strange reach,
/// and the band alone would admit a short-range shot.
pub const MELEE_BAND_REACH: isize = 4;

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
    resolve_area_save_condition(
        encounter,
        caster_id,
        AreaShape::Burst { radius },
        center,
        save_ability,
        dc,
        condition,
        timer,
    )
}

/// `resolve_burst_save_condition` for an area of any shape — the lane a
/// gaze cone resolves through. See `resolve_area_save_damage` for why
/// the shape-general function is the body and the burst one is the
/// wrapper.
#[allow(clippy::too_many_arguments)]
pub fn resolve_area_save_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    shape: AreaShape,
    aim: Coordinate,
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let target_ids = encounter.enemy_area_targets(caster_id, shape, aim);
    install_condition_on_failed_saves(
        encounter, caster_id, &target_ids, save_ability, dc, condition, timer,
    )
}

/// The save-or-condition half of every condition burst: roll `save_ability`
/// vs `dc` for each id in `target_ids`, in the order given, and install
/// `condition` on each one that fails.
///
/// Sibling to `resolve_burst_targets` on the damage lane, and split out
/// for the same reason: the three condition bursts that call it differ
/// only in how they pick their targets, and a loop written three times
/// is a rule that can be fixed in one place and stay broken in the
/// other two.
///
/// Routed through `install_condition_with_link` rather than raising a
/// bare `ApplyCondition`, which is why it takes `caster_id` at all: a
/// condition that carries a back-link needs to record who applied it,
/// and every caller of this helper already had the answer and was
/// dropping it at this line. A condition with no link is unaffected —
/// the installer hands back a one-element vec — so the parameter costs
/// the non-linked callers nothing but the argument.
pub(crate) fn install_condition_on_failed_saves(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: &[usize],
    save_ability: AbilityScoreType,
    dc: i32,
    condition: crate::conditions::Condition,
    timer: crate::conditions::ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    for &tid in target_ids {
        if encounter
            .roll_save_vs_condition(tid, save_ability, dc, condition)
            .passed()
        {
            continue;
        }
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            condition, tid, caster_id, timer,
        ));
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
    install_condition_on_failed_saves(
        encounter, caster_id, &target_ids, save_ability, dc, condition, timer,
    )
}

/// The hearing-gated sibling of `resolve_los_glare_condition`, for the
/// effects RAW words as sound rather than sight: every combat-active
/// enemy within `radius` that can hear rolls `save_ability` vs `dc`,
/// and the failures pick up `condition` for `timer`.
///
/// The two helpers differ in exactly one filter and that difference is
/// the point. A gaze cannot bend around a corner, so the glare variant
/// drops anything without line of sight; a moan travels perfectly well
/// through a wall, so this one does not ask. What it asks instead is
/// whether the target can hear at all — the clause every one of these
/// effects carries in its RAW quote and none of them carried in code,
/// because until `ActorInstance::can_hear` existed there was nothing to
/// ask. See that method for the five effects this closes.
///
/// `skip_immune_to_damage` is the same creature-cohort proxy the glare
/// variant takes: necrotic immunity stands in for undead on the
/// banshee's wail, psychic immunity for aberrations on the cloaker's
/// moan. It is a proxy and not the rule, and it is stated as one at
/// each call site.
#[allow(clippy::too_many_arguments)]
pub fn resolve_audible_burst_condition(
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
    // Both filters run before any save is rolled, for the same reason
    // the glare variant's do: a target the sound never reaches must not
    // touch the dice, or the seeded log fills with saves nobody made.
    let target_ids: Vec<usize> = encounter
        .enemy_burst_targets(caster_id, caster_loc, radius)
        .into_iter()
        .filter(|&tid| {
            encounter.actors.get(&tid).is_some_and(|a| {
                a.can_hear() && !skip_immune_to_damage.is_some_and(|dt| a.is_immune_to(dt))
            })
        })
        .collect();
    install_condition_on_failed_saves(
        encounter, caster_id, &target_ids, save_ability, dc, condition, timer,
    )
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
    pool_sweep_area_targets(
        encounter,
        caster_id,
        AreaShape::Burst { radius },
        point,
        pool,
        skip_immune_to,
    )
}

/// `pool_sweep_targets` for an area of any shape — Color Spray's cone
/// and Sleep's sphere are the same rule over different ground.
pub fn pool_sweep_area_targets(
    encounter: &EncounterInstance,
    caster_id: usize,
    shape: AreaShape,
    aim: Coordinate,
    pool: u32,
    skip_immune_to: crate::conditions::Condition,
) -> Vec<usize> {
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
            if !encounter.area_catches(caster_id, shape, aim, *id) {
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

/// `PartialEq` and `Debug` because a schema is a *fact about an action*
/// worth asserting on directly: nine spells declare a cone or a line as
/// one line each, and a test that can only ask "is it an area" cannot
/// tell a 60-foot cone from a 15-foot one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// 5e's **Cone** — a wedge thrown from the caster's own body toward
    /// a tile it names. Takes one point argument exactly as `Burst`
    /// does, and the point supplies a *direction* rather than a centre:
    /// see `crate::engine::areas`.
    ///
    /// `length` is in tiles, so a 60-foot breath is 24.
    Cone {
        length: isize,
    },
    /// 5e's **Line** — "a 90-foot-long, 5-foot-wide Line" — thrown from
    /// the caster's body toward the named tile. Same argument shape as
    /// `Cone`; `half_width` is in tiles, so RAW's 5-foot line is 1.
    Line {
        length: isize,
        half_width: isize,
    },
    Custom,
}

impl TargetingSchema {
    /// This schema's area of effect, or `None` for the schemas that are
    /// not areas at all.
    ///
    /// The one place the three area variants are unified. Every caller
    /// that used to ask "is this a `Burst`, and how big" — the AI's two
    /// placement searches, its Careful Spell gate, the target prompt —
    /// asks this instead, and picks up cones and lines without knowing
    /// they exist. Adding a fourth shape is a variant here and an arm
    /// in `AreaShape`, not a sweep over the callers.
    pub fn area_shape(&self) -> Option<AreaShape> {
        match *self {
            TargetingSchema::Burst { radius } => Some(AreaShape::Burst { radius }),
            TargetingSchema::Cone { length } => Some(AreaShape::Cone { length }),
            TargetingSchema::Line { length, half_width } => {
                Some(AreaShape::Line { length, half_width })
            }
            _ => None,
        }
    }

    /// The schema that asks for `shape` — the inverse of
    /// `area_shape`, for the action chassis that carry an `AreaShape`
    /// on the struct and derive their schema from it.
    ///
    /// `BreathWeapon` is the first: a breath is a cone or a line
    /// depending on the dragon, that fact belongs on the stat-block
    /// literal beside the dice and the DC, and the schema is a
    /// restatement of it. Declaring both would be two places to change
    /// a black dragon's line into a cone by mistake.
    pub fn from_area(shape: AreaShape) -> Self {
        match shape {
            AreaShape::Burst { radius } => TargetingSchema::Burst { radius },
            AreaShape::Cone { length } => TargetingSchema::Cone { length },
            AreaShape::Line { length, half_width } => {
                TargetingSchema::Line { length, half_width }
            }
        }
    }

    /// True if this schema is invoked with exactly one target tile and
    /// no target actors — `SinglePoint` and all three areas.
    ///
    /// The argument-shape half of `validate_input`, lifted out because
    /// four schemas answered it identically and the fourth was added by
    /// remembering to.
    pub fn takes_one_point(&self) -> bool {
        matches!(
            self,
            TargetingSchema::SinglePoint
                | TargetingSchema::Burst { .. }
                | TargetingSchema::Cone { .. }
                | TargetingSchema::Line { .. }
        )
    }
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
    weapon_expected_damage_named(
        encounter,
        caster_id,
        "",
        dice,
        damage_ability,
        cost_resource,
        extra_swings,
    )
}

/// `weapon_expected_damage` for a swing that knows its own name, so the
/// estimate can include the creature's charge clause when one is riding
/// this particular attack and the run behind it already qualifies.
///
/// The name is what a charge is keyed on — a minotaur's clause names its
/// gore and says nothing about the greataxe in its other hand — and the
/// picker is exactly where knowing costs something: gore and greataxe
/// tie on matchup, roll mode and reach, so the damage estimate is what
/// decides between them, and a minotaur that has just thundered ten feet
/// at somebody should be putting its head down rather than swinging.
///
/// The run is read live rather than predicted, which is the right order:
/// the AI moves and then picks its attack, so by the time this is asked
/// the ground is already covered. A creature that hasn't run gets the
/// plain estimate and the greataxe wins, which is also correct.
pub fn weapon_expected_damage_named(
    encounter: &EncounterInstance,
    caster_id: usize,
    weapon_name: &str,
    dice: crate::engine::dice::Dice,
    damage_ability: Option<AbilityScoreType>,
    cost_resource: Resource,
    extra_swings: u32,
) -> Option<f32> {
    let caster = encounter.actors.get(&caster_id)?;
    let charge_bonus = caster
        .charge()
        .filter(|c| c.weapon.is_none_or(|w| w == weapon_name))
        .filter(|c| caster.straight_run_tiles().is_some_and(|n| n >= c.run_tiles))
        // A once-per-turn clause already cashed is worth nothing to the
        // swing being weighed — the same gate the resolution path reads,
        // so the estimate and the die agree about the second swing of an
        // Extra Attack.
        .filter(|c| {
            c.once_per_turn_tag
                .is_none_or(|tag| !caster.once_per_turn_used(tag))
        })
        .map(|c| c.dice.average_roll())
        .unwrap_or(0.0);
    let per_swing = dice.average_roll()
        + charge_bonus
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

/// `expected_damage` for a **melee-touch attack cantrip** — one whose
/// die pool scales with `cantrip_dice_count` and whose damage roll adds
/// no ability modifier.
///
/// The narrow cohort this exists for is the one the AI's attack picker
/// actually has to arbitrate: a cantrip that costs an Action, reaches
/// `MELEE_REACH`, and therefore stands in exactly the same lane as the
/// weapon in the caster's hand. Shocking Grasp, Booming Blade and
/// Green-Flame Blade are all of it today.
///
/// Ranged attack cantrips are deliberately left unannotated. The
/// picker's damage key only ever decides a tie between two candidates
/// that already agree on matchup, roll mode and reach — and a ranged
/// cantrip never ties with a melee weapon on reach, so an estimate for
/// it would sort nothing that is not already sorted. Annotating the
/// cohort that can tie, and only that cohort, is what keeps the hint
/// from quietly becoming a second, worse ranking of everything.
pub fn melee_cantrip_expected_damage(
    encounter: &EncounterInstance,
    caster_id: usize,
    faces: u32,
) -> Option<f32> {
    let caster = encounter.actors.get(&caster_id)?;
    let n = crate::engine::util::cantrip_dice_count(caster.level());
    Some(crate::engine::dice::Dice::new(n, faces).average_roll())
}

pub trait Action {
    fn name(&self) -> &str;

    fn aliases(&self) -> Vec<&str>;

    fn targeting_schema(&self) -> TargetingSchema;

    /// Maximum footprint-Chebyshev gap from caster to target for this action
    /// to be valid. `None` disables the spatial check (non-targeted actions
    /// or actions that do their own range logic). Used both by the engine
    /// (validation) and by the UI (target picker filters by reach).
    ///
    /// **Defaults to the area's own length for a cone or a line**, and to
    /// `None` for everything else, including a burst — see
    /// `AreaShape::aim_reach` for why those two answers differ. A
    /// projected area is aimed by naming a tile inside it, so its reach
    /// and its length are the same number and declaring them separately
    /// is two places to say one thing.
    ///
    /// The default is load-bearing rather than a convenience. An action
    /// with no reach at all is one nothing measures: it validates
    /// against a tile on the far side of the map, and — the reason this
    /// was found — `EncounterInstance::is_ranged_engagement_option`
    /// reads `reach_tiles` *first*, so a wizard whose only ranged
    /// option was Lightning Bolt was marked stalemate-locked behind a
    /// wall it could have shot straight over.
    fn reach_tiles(&self) -> Option<isize> {
        self.targeting_schema().area_shape().and_then(|s| s.aim_reach())
    }

    /// The recharge pool this action is gated on, or `None` for the
    /// overwhelming majority that are gated on nothing.
    ///
    /// Declared rather than inferred, because the AI used to infer it
    /// from the *name*: the breath rung selected any Burst action whose
    /// name contained the substring `"breath"`, which worked for as
    /// long as every recharge burst in the engine was a dragon's. The
    /// Sphinx of Lore's Mind-Rending Roar is the same shape, the same
    /// pool and the same tactical decision, and it is not called a
    /// breath, so the rung could not see it — and nothing would have
    /// reported that, because an ability nobody selects looks exactly
    /// like an ability nobody needed.
    fn recharge_key(&self) -> Option<&'static str> {
        None
    }

    /// True when this action's area deliberately skips the caster's own
    /// side — RAW's "each **enemy** in a 20-foot Emanation" rather than
    /// "each creature in the area".
    ///
    /// The AI's two area rungs both veto a placement that catches an
    /// ally, which is right for a fireball and wrong for a planetar's
    /// Holy Burst: an angel standing in the middle of its own party
    /// could never fire the one ability it has for exactly that
    /// situation. The resolvers have known the difference since the
    /// enemy-scoped burst arrived; this is how the picker finds out.
    fn spares_allies(&self) -> bool {
        false
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

    /// True when this action swings rather than shoots — a melee weapon
    /// attack or a touch spell, as opposed to a bow, a thrown rock or a
    /// Fire Bolt.
    ///
    /// Derived rather than declared, from the two things every action
    /// already says. `requires_los` is the engine's existing
    /// melee/ranged marker: a swing needs contact and declares `false`,
    /// a shot needs sight and declares `true`. `reach_tiles` bounds it
    /// at `MELEE_BAND_REACH`, so a strange long-reach action that
    /// happens not to require sight cannot claim to be a swing.
    ///
    /// It exists because three call sites were each deriving this from
    /// scratch and two of them got it wrong in the same way — by
    /// comparing against `MELEE_REACH`, which is the reach of an
    /// *ordinary* weapon and excludes every reach weapon in the
    /// bestiary. See `MELEE_BAND_REACH` for the two bugs that produced.
    /// The three consumers are `first_melee_weapon_action` (which swing
    /// answers an opportunity attack or a riposte), the AI's attack
    /// picker (which mode a candidate rolls in), and the picker's own
    /// melee/ranged bookkeeping.
    ///
    /// Note this asks what *kind* of attack it is, not whether it is an
    /// attack at all — Shove is a melee action by this reading, and
    /// callers that care pair it with `deals_damage`.
    fn is_melee_attack(&self) -> bool {
        !self.requires_los() && self.reach_tiles().is_some_and(|r| r <= MELEE_BAND_REACH)
    }

    /// The footprint gap *below* which this action's swing rolls at
    /// disadvantage, or `None` for everything that doesn't care how
    /// close its target is.
    ///
    /// 5e's **lance** is the only entry today: "you have disadvantage
    /// when you use a lance to attack a target within 5 feet of you."
    /// The mirror of the long-range penalty every bow already carries,
    /// and declared on the trait for the same reason `reach_tiles` is —
    /// two consumers need the number and neither of them is the weapon.
    /// `engine::attack::resolve_attack` applies it to the die, through
    /// `AttackParams::min_range`; the AI's attack picker reads it here,
    /// so a knight with a longsword on their belt doesn't jab with the
    /// wrong end of a lance at point-blank range.
    fn min_effective_reach(&self) -> Option<isize> {
        None
    }

    /// The footprint gap *above* which this action's shot rolls at
    /// disadvantage — 5e's "normal range", as distinct from the
    /// `reach_tiles` long range it can still reach at all. `None` for
    /// melee weapons and for every spell, neither of which has one.
    ///
    /// The exact mirror of `min_effective_reach`, and declared on the
    /// trait for the same two consumers. `engine::attack::resolve_attack`
    /// has always applied it to the die through
    /// `AttackParams::long_range`; what it could not do until now was
    /// answer 5e's Underwater Combat clause — "a ranged weapon attack
    /// automatically misses a target beyond the weapon's normal range" —
    /// on the AI's side of the fence, because the number lived on the
    /// weapon struct and the picker only ever sees a `&dyn Action`.
    ///
    /// So an archer standing in a lake used to line up a longbow shot
    /// across it, spend its Action, and miss on every roll for the rest
    /// of the fight, with nothing on the sheet the picker could have
    /// read to know better.
    fn normal_range(&self) -> Option<isize> {
        None
    }

    /// True if this action resolves as a 5e **weapon attack** — a swing
    /// or a shot with something the creature is holding — rather than
    /// as a spell attack, a save-or-suck, or a class feature.
    ///
    /// The distinction 5e's Underwater Combat rules are written on:
    /// every one of their clauses says "melee **weapon** attack" or
    /// "ranged **weapon** attack", and a Fire Bolt cast in a lake is
    /// unaffected by all of them. `engine::attack` has always known the
    /// answer — it is `!AttackParams::is_spell` — but the AI's attack
    /// picker holds a `&dyn Action` and cannot see an `AttackParams`,
    /// so the answer has to be askable from the sheet.
    ///
    /// **Defaults to `false`, which is the conservative direction and
    /// deliberately so.** A missing `true` costs the picker a ranking
    /// hint; a wrong `true` would have it apply a penalty RAW doesn't
    /// impose. `SimpleWeapon` overrides it, which covers every shared
    /// weapon static in the bestiary — and, not coincidentally, every
    /// weapon named on either of the two underwater cohorts, since
    /// those cohorts are RAW's list of ordinary armoury weapons. A
    /// bespoke natural weapon (a claw, a bite, a tentacle) leaves it
    /// `false` and loses nothing by it: nothing of that shape is on
    /// either cohort, so every one of a creature's bespoke swings is
    /// penalised identically underwater and the ranking between them
    /// is unchanged either way.
    fn is_weapon_attack(&self) -> bool {
        false
    }

    /// The name 5e's Underwater Combat rules should be asked about when
    /// they look this attack up by weapon — defaulting to the action's
    /// own name, which is right for everything that swings once.
    ///
    /// RAW's melee clause is a five-weapon allowlist (dagger, javelin,
    /// shortsword, spear, trident) and its ranged clause a similar one,
    /// so `UnderwaterVerdict::for_attack` matches on a string. That
    /// works until an action's name is not a weapon's name, which is
    /// exactly what a multiattack's is: the marid's three-trident
    /// Action is called "marid multiattack", and a trident is on the
    /// list where a multiattack is not. The die never noticed, because
    /// down there each swing resolves under its own name; the AI's
    /// picker holds only the wrapper, so it would have predicted
    /// disadvantage on three swings the water leaves alone.
    ///
    /// Overridden by the multiattack chassis to forward its
    /// sub-attack's, which is the answer the die will use.
    fn underwater_weapon_name(&self) -> &str {
        self.name()
    }

    /// True if this action is a swing with a 5e **light** melee weapon —
    /// the property that opens RAW's two-weapon fighting option:
    /// "when you take the Attack action and attack with a light melee
    /// weapon that you're holding in one hand, you can use a bonus
    /// action to attack with a different light melee weapon".
    ///
    /// Read once per resolved action, at the stack's execution
    /// chokepoint in `EncounterInstance::process_stack`, which stamps
    /// the swinger's per-turn light-weapon ledger. `OffHandAttack`
    /// reads that ledger back as its gate, so the bonus swing is legal
    /// exactly when a light main-hand swing has actually happened this
    /// turn — not merely when the Action slot is gone.
    ///
    /// **Defaults to `false`, the conservative direction.** A missing
    /// `true` costs a dual-wielder their bonus swing; a wrong `true`
    /// would hand one out for a greataxe. `SimpleWeapon` overrides it
    /// from its `is_light` field, which covers every shared armoury
    /// static; a bespoke natural weapon leaves it `false`, which is
    /// correct — RAW's light property is a property of *weapons*, and
    /// a bear's claw is not one.
    fn is_light_melee_weapon(&self) -> bool {
        false
    }

    /// True if this action is the two-weapon-fighting **off-hand
    /// swing** — `actions::two_weapon::OffHandAttack` and nothing else.
    ///
    /// Read once per resolved action at the stack's execution
    /// chokepoint, which stamps the swinger's once-per-turn off-hand
    /// ledger; the **Nick** weapon mastery reads that ledger back to
    /// decide whether this turn's off-hand swing is free. Kept as a
    /// declared property rather than inferred from the cost, because
    /// Nick *changes* the cost — inferring "is this the off-hand swing"
    /// from "does it cost a bonus action" would stop being true for
    /// exactly the swing the ledger exists to count.
    fn is_offhand_swing(&self) -> bool {
        false
    }

    /// This weapon's 5e (2024 / SRD 5.2) **mastery property**, or `None`
    /// for an action that isn't an armoury weapon.
    ///
    /// Purely descriptive at this level: the property is *applied* by
    /// the weapon's own swing, which hands `engine::mastery::MasteryRider`
    /// to the shared attack pipeline. What reads it here is everything
    /// that wants to talk *about* the weapon without swinging it — the
    /// UI's action list, which annotates a mastered weapon with the
    /// clause it carries, and the AI's attack picker, which prefers a
    /// weapon whose property is worth something against the target in
    /// front of it.
    ///
    /// Defaults to `None`, which is right for every natural weapon and
    /// every spell: RAW's mastery table lists objects out of the weapon
    /// shop, and a bite is not one.
    fn weapon_mastery(&self) -> Option<crate::engine::mastery::WeaponMastery> {
        None
    }

    /// True when resolving this action lands more than one attack — the
    /// `Multiattack` and `CompoundAttack` wrappers, which sit on the
    /// same action list as the swings they contain.
    ///
    /// Exists for the reaction lanes. RAW's opportunity attack and the
    /// Battle Master's Riposte each grant "one melee attack", and both
    /// dispatchers resolve the chosen action's `side_effects` directly
    /// without charging a cost — so a wrapper reaching either of them
    /// hands out a monster's whole Attack routine for a reaction, once
    /// per creature that walks past. A tarrasque answering a step with
    /// bite-claw-claw-tail is not the rule; it is the picker having no
    /// way to tell a swing from a turn.
    ///
    /// Declared rather than sniffed, for the reason `holds_concentration`
    /// is: the wrappers are the only things that know how many swings
    /// they contain, and a consumer guessing from the name or the damage
    /// estimate would be wrong the first time a bespoke routine landed.
    fn chains_multiple_attacks(&self) -> bool {
        false
    }

    /// Whether **Haste**'s extra Action may be spent on this.
    ///
    /// SRD 5.2: *"it gains an additional action on each of its turns.
    /// That action can be used to take only the Attack (one attack
    /// only), Dash, Disengage, Hide, or Utilize action."* That sentence
    /// is the whole of the spell's cost control — without it a hasted
    /// wizard casts two Fireballs a round — and it is the reason the
    /// extra action is a *restricted* slot rather than a second copy of
    /// the ordinary one. See `ActorInstance::restricted_action_slots`.
    ///
    /// The default reads RAW's first and hardest clause off two flags
    /// the trait already carries: an attack, and only one of them. A
    /// multiattack is excluded by `chains_multiple_attacks`, which is
    /// exactly RAW's parenthesis — a hasted marid gets a fourth trident
    /// thrust, not a second set of three.
    ///
    /// Dash, Disengage and Hide override to `true`; they are the rest of
    /// RAW's list and they are three actions rather than a category.
    /// Utilize has no engine equivalent — item use here is a family of
    /// bespoke actions rather than one verb — so it is left out, which
    /// is the conservative direction: an action that says nothing can
    /// never be paid for with the haste slot, and the spell is never
    /// stronger than RAW.
    ///
    /// Bespoke natural weapons that leave `is_weapon_attack` false (the
    /// trait doc says they may) inherit `false` here too. A hasted
    /// ghoul gets a slot it cannot spend, which is a rendering of the
    /// spell that is too weak rather than too strong.
    fn hasted_action_eligible(&self) -> bool {
        self.is_weapon_attack() && !self.chains_multiple_attacks()
    }

    /// True if this action's primary effect is HP loss on the target.
    /// Hostile control actions like Shove return false so the AI's
    /// focus-fire pipeline doesn't pick them over attacks that actually
    /// whittle down enemy HP.
    ///
    /// **Defaults to `is_harmful()`**, which is the only default that
    /// can't be wrong by accident: an action that isn't aimed at an
    /// enemy at all is not whittling anybody's hit points, and an action
    /// that is aimed at one usually is. A flat `true` here — which is
    /// what this was — quietly gave twenty-one buffs, heals and
    /// movement actions the claim that they deal damage. Dodge, Dash,
    /// Help, Hide, Second Wind, Action Surge, Bless, Aid, Cure Wounds,
    /// Healing Word, Shield and Shield of Faith all said so, and the
    /// only reason nothing broke is that every consumer happens to
    /// check `is_harmful()` first. That is a coincidence rather than a
    /// design, and the next consumer to read this alone would have
    /// inherited twelve wrong answers.
    ///
    /// The two clauses stay separate because a real distinction lives
    /// between them: **harmful but not damaging** is a whole category —
    /// Shove, Grapple, Hold Person, Banishment, Sanctuary — and those
    /// override to `false`. The reverse (not harmful yet damaging) has
    /// exactly one member, Spirit Guardians, whose aura hurts enemies
    /// while the *cast* targets nobody; it already says `false` here for
    /// the same reason the AI shouldn't treat it as an attack.
    fn deals_damage(&self) -> bool {
        self.is_harmful()
    }

    /// True if this action restores HP / temp HP on its target.
    /// Used by the AI's support pipeline (heal-the-lowest target).
    fn is_heal(&self) -> bool {
        false
    }

    /// The conditions this action lifts from the creature that takes it,
    /// or an empty slice — which is the answer for all but a handful.
    ///
    /// The cure-shaped sibling of `is_heal`, and it returns the *list*
    /// rather than a boolean for the reason a heal does not have to: a
    /// heal is worth taking whenever the taker is wounded, and a cure is
    /// worth taking only when the taker has the specific thing it cures.
    /// A Potion of Vitality ends exhaustion and poison and does nothing
    /// whatever for a paralyzed drinker, so an AI rung that could only
    /// ask "is this a cure" would drink it on the wrong turn.
    ///
    /// Read by `ai::simple::try_self_cleanse`. Scoped to the taker,
    /// which is why it needs no target parameter — every implementor is
    /// a `TargetingSchema::NoArgs` self-consumable. An ally-targeted
    /// cure (Lesser Restoration, Greater Restoration) is a different
    /// question with a different rung, and answers `&[]` here rather
    /// than pretending its list applies to the caster.
    fn cures_conditions(&self) -> &'static [crate::conditions::Condition] {
        &[]
    }

    /// True if this action's whole effect is a buff spread over the
    /// allies standing near the actor — the Artillerist Protector
    /// cannon's temp-HP pulse, the Bard's Countercharm.
    ///
    /// Read by `ai::simple::try_ally_support_pulse`, which is the rung
    /// for actions like this and was gated on `is_heal` because for a
    /// while every member of the cohort was one. Countercharm is not:
    /// it restores nothing, and declaring it a heal to reach the rung
    /// would have made it one everywhere else too — including in
    /// `try_self_heal`, where a bard below half hit points would have
    /// "healed" by singing.
    ///
    /// So the rung's real question gets its own method. A declaration
    /// rather than a name list for the reason `summons_allies` gives
    /// one screen down: the next ally pulse should be picked up by
    /// saying what it is.
    ///
    /// Deliberately narrower than "a buff": the rung fires unconditionally
    /// whenever a teammate is in range, which is only safe for effects
    /// that are free, repeatable and self-limiting. An action that costs
    /// a slot or a charge is a decision and belongs somewhere that can
    /// make one.
    fn pulses_ally_buff(&self) -> bool {
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

    /// True if resolving this action puts the caster's concentration on
    /// the line — i.e. its side effects include a `StartConcentration`.
    ///
    /// A *declaration*, read by the AI's gates so they can ask an action
    /// what it costs instead of matching its name against a list. The
    /// two rungs that consult it are `try_summon_allies` and
    /// `try_area_control`, and both are answering the same question:
    /// would firing this trade a concentration effect that has already
    /// landed for one that hasn't?
    ///
    /// It replaced a hand-kept `bool` column beside a name list in the
    /// AI, which had the defect every name list has — the fact lived
    /// somewhere the spell couldn't see, so a spell whose concentration
    /// changed had two places to update and only one of them would fail
    /// a test.
    ///
    /// **Defaults to `false`, and that is a real default rather than an
    /// unknown**: the overwhelming majority of actions in the engine —
    /// every weapon swing, every monster attack, every item use, every
    /// non-concentration spell — genuinely do not concentrate. What the
    /// default cannot do is catch a concentration spell that forgets to
    /// override, so the two cohorts the AI actually reads are pinned by
    /// `the_ai_gated_cohorts_declare_their_concentration`: a new summon
    /// or area-control spell has to state its answer or fail the build's
    /// tests.
    ///
    /// Note this asks whether the action *starts* concentration, not
    /// whether the caster is holding any — that is
    /// `ActorInstance::is_concentrating`.
    fn holds_concentration(&self) -> bool {
        false
    }

    /// Damage types this action can deal (for actor-side resistance /
    /// immunity hints in the prompt UI). Empty for non-damaging actions
    /// or those whose typing depends on runtime data.
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }

    /// True when `damage_types()` is a **menu** the caster picks one
    /// entry from at resolution, rather than a bundle the action lands
    /// all of at once.
    ///
    /// The distinction the list could not carry on its own. A flaming
    /// longsword's `[Slashing, Fire]` is a bundle: every swing lands
    /// both, and a target that resists either resists part of every
    /// hit. Chromatic Orb's six, Sorcerous Burst's seven and Dragon's
    /// Breath's five are a menu — the resolver runs
    /// `pick_damage_type_against_target` and hands the target the one
    /// type it is least able to shrug off.
    ///
    /// Read by the AI's attack picker, which scores a menu on its best
    /// entry and a bundle on its worst. Without it, Sorcerous Burst was
    /// ranked as "resisted" against anything that resisted any one of
    /// its seven types — which is most of the bestiary — and lost the
    /// comparison to a Fire Bolt that the same creature was immune to.
    ///
    /// Defaults to `false`, the conservative direction: a bundle read
    /// as a menu would flatter an action that genuinely eats a
    /// resistance, and every weapon in the armoury is a bundle.
    fn chooses_damage_type(&self) -> bool {
        false
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
        let schema = self.targeting_schema();
        let schema_ok = match schema {
            TargetingSchema::NoArgs => {
                target_ids.is_none() && target_locations.is_none() && overrides.is_none()
            }
            TargetingSchema::SingleActor => {
                target_locations.is_none()
                    && target_ids.is_some_and(|ids| !ids.is_empty())
            }
            TargetingSchema::Custom => true,
            _ if schema.takes_one_point() => {
                target_ids.is_none()
                    && target_locations.is_some_and(|tl| tl.len() == 1)
            }
            _ => false,
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
        // The other half of the same sentence: 5e's flat "the target
        // can't attack", which Gaseous Form prints beside "or cast
        // spells". Gated on `is_harmful` rather than on a weapon
        // marker, because a creature that has turned into a cloud
        // cannot shove or grapple either, and both are attacks in the
        // sense RAW means. It leaves Dash, Dodge, Disengage and Hide
        // alone, which is what separates the clause from
        // `Incapacitated`.
        // Asked here as well as inside `hostility_blocked` below,
        // because this one has to reach an action with no targets at all
        // — a cloud may not drop a Fireball either, and the gate down
        // there is per-named-target.
        if self.is_harmful()
            && encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.blocked_from_attacking())
        {
            return false;
        }
        // 5e Swallow: "the toad can't use Bite while it has a swallowed
        // target." A named attack rather than a general block — a
        // tarrasque with six people inside it is not slowed down at all
        // — so the gate reads the action's own name against the one
        // `SwallowProfile::blocked_while_full` names. Beside the two
        // caster-state gates above because it is the same shape: a fact
        // about the swinger that no target can change.
        if encounter.swallow_blocks_action(caster_id, self.name()) {
            return false;
        }
        // 5e Antimagic Field: "spells and other magical effects … are
        // suppressed in the sphere and can't protrude into it." Same
        // lane as the `blocked_from_casting` gate above and for the
        // same reason — `school()` is the engine's "this is a spell"
        // marker, and cantrips never touch the `SpellSlot` resource
        // where a slot-side gate would sit.
        //
        // Where the two differ is that this one is about a *place*
        // rather than about the caster's own state, so it needs the
        // aimed-at end as well: a caster outside the sphere may not
        // reach into it, and one inside may not reach out.
        if self.school().is_some()
            && encounter.magic_suppressed_for_cast(caster_id, target_ids, target_locations)
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
            // 5e Banishment: the target is on another plane. Checked
            // across *every* declared target for the same reason the
            // Charmed clause below is — the reach and LOS clauses are
            // deliberately first-target-only, and "you cannot reach
            // somewhere you are not" binds on each name in the list
            // independently.
            //
            // This is the one off-board gate that cannot ride
            // `is_combat_active`. A dying creature is not combat-active
            // either and is still very much lying there to be finished
            // off, so the targeting lane has never asked that question
            // and must not start; what it needs is the narrower one.
            // See `crate::engine::banishment`.
            if targets
                .iter()
                .any(|tid| encounter.actors.get(tid).is_some_and(|t| t.is_off_board()))
            {
                return false;
            }
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
            // Every pair-scoped rule that forbids acting hostilely
            // toward a named target: 5e Charmed's "can't attack the
            // charmer", the attach clause's "can attack only the
            // target", a swallow's Total Cover. Checked across *every*
            // declared target, not just the first: the reach / LOS
            // clauses above are deliberately first-target-only (a
            // multi-target action measures its envelope off its
            // primary), but these bind on each name in the list
            // independently, so a multi-target harmful action naming
            // the charmer second would otherwise slip through. Shared
            // with the three reaction dispatchers, which reach
            // hostility without passing through validation at all — see
            // `EncounterInstance::hostility_blocked`.
            if self.is_harmful()
                && targets
                    .iter()
                    .any(|&tid| encounter.hostility_blocked(caster_id, tid))
            {
                return false;
            }
            // 5e's Swallow clause: a creature inside another one "has
            // Total Cover against attacks and other effects outside"
            // it, in both directions. Asked separately from the
            // hostility list above — which already contains it — because
            // it is the one rule there that is *not* about hostility:
            // Total Cover stops a Cure Wounds from reaching somebody
            // inside a kraken exactly as firmly as it stops an arrow, so
            // this arm runs whether the action is harmful or not.
            if targets
                .iter()
                .any(|&tid| encounter.swallow_blocks_targeting(caster_id, tid))
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
            // 5e **Haste**'s restricted slot. `can_consume_resource`
            // counts Action slots and cannot tell one kind from the
            // other — it takes a `Resource` and not an action — so the
            // one gate that needs to know which action is being paid
            // for lives here, at the only place that has both.
            //
            // Read as: an ineligible action needs an *ordinary* Action
            // left, which is one that is not the haste grant. See
            // `ActorInstance::restricted_action_slots`.
            if costs.contains(&Resource::Action)
                && !self.hasted_action_eligible()
                && actor.action_slots() <= actor.restricted_action_slots()
            {
                return false;
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
        encounter.enter_cast(
            self.school(),
            cast_level,
            crate::engine::types::DamageTypeSet::from_types(&self.damage_types()),
        );
        let mut side_effects = self.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        // `holds_concentration` is a declaration, and a declaration that
        // nothing checks is a comment. This is the check: an action that
        // queues a `StartConcentration` and says it does not hold
        // concentration is caught the first time it is cast under a
        // debug build, which the whole test suite is.
        //
        // It is worth a runtime assertion rather than a review habit
        // because the failure is silent and the default is the wrong
        // answer. Ninety-two concentration spells in this file declared
        // nothing — Haste, Polymorph, Banishment, Wall of Fire, every
        // wall, every aura — and the AI's summon and area-control rungs,
        // which ask an action what it costs before trading a landed
        // effect for an unlanded one, were told all ninety-two were free.
        //
        // The payload accessor is the ground truth because it is what
        // the concentration machinery itself reads; there is no way to
        // start concentration without going through it.
        debug_assert!(
            self.holds_concentration()
                || !side_effects
                    .iter()
                    .any(|e| e.concentration_payload().is_some()),
            "{} queues a StartConcentration but declares holds_concentration() == false",
            self.name()
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
            // An Action spend is billed through the typed path so
            // Haste's restricted slot is cashed by the actions RAW lets
            // it buy, rather than being left for last by a spender that
            // does not know what it is paying for. Everything else goes
            // out as a plain `ConsumeResource`.
            side_effects.push(if cost == Resource::Action {
                Box::new(crate::engine::side_effects::SpendActionSlot {
                    actor_id: caster_id,
                    hasted_eligible: self.hasted_action_eligible(),
                }) as Box<dyn ApplicableSideEffect>
            } else {
                Box::new(ConsumeResource {
                    actor_id: caster_id,
                    resource: cost,
                })
            });
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
