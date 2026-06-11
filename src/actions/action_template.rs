use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride,
    encounter::EncounterInstance,
    side_effects::{ApplicableSideEffect, ConsumeResource, DealDamage, Resource},
    types::{AbilityScoreType, Coordinate, DamageType},
};

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
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    // 5e Sorcerer Careful Spell metamagic: protected allies in the burst
    // auto-pass the save AND take 0 damage. Resolved up-front so the loop
    // below can skip them cleanly; the prime is consumed inside the helper.
    let shielded = encounter.careful_spell_shielded(caster_id, center, radius);
    // `neutral_burst_targets` shares the "caster-excluded, combat-active,
    // footprint in radius" filter with the rest of the engine — folding
    // it here keeps the caster-exclusion / footprint-Chebyshev / sorted-
    // ids invariant in one place instead of re-inlining the loop.
    for target_id in encounter.neutral_burst_targets(caster_id, center, radius) {
        if shielded.contains(&target_id) {
            continue;
        }
        // Route through the caster-aware save helper so the 5e Sorcerer
        // Heightened Spell metamagic forces disadvantage on the *first*
        // save in the burst (RAW). Subsequent targets in the same cast
        // fall through to the normal save path — `roll_save_against_caster`
        // consumes the prime on its first call.
        let save = encounter.roll_save_against_caster(target_id, save_ability, dc, caster_id);
        let has_evasion = save_ability == AbilityScoreType::Dexterity
            && encounter
                .actors
                .get(&target_id)
                .is_some_and(|a| a.has_evasion());
        let dmg = match (save.passed(), has_evasion) {
            (true, true) => 0,
            (true, false) => damage / 2,
            (false, true) => damage / 2,
            (false, false) => damage,
        };
        if dmg == 0 {
            if has_evasion && save.passed() {
                encounter.log(format!(
                    "  evasion: {} takes no damage",
                    encounter.actor_name(target_id)
                ));
            }
            continue;
        }
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type,
        }));
    }
    effects
}

/// Reach for melee/touch actions, expressed as a footprint-Chebyshev gap cap.
/// 5e melee weapons are 5ft = 1-tile gap in this 2.5ft grid. Polearms /
/// reach weapons would be 2. Ranged actions return their max range here.
pub const MELEE_REACH: isize = 1;

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

    /// Damage types this action can deal (for actor-side resistance /
    /// immunity hints in the prompt UI). Empty for non-damaging actions
    /// or those whose typing depends on runtime data.
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        vec![Resource::Action]
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
                    .map(|a| a.extra_melee_reach(reach) + a.extra_spell_reach(reach))
                    .unwrap_or(0);
                if dist > reach + bonus {
                    return false;
                }
            }
            if self.requires_los() && !encounter.actor_has_line_of_sight(caster_id, target_id) {
                return false;
            }
            // 5e Charmed: the target of a Charm spell cannot make any
            // hostile action against their charmer. Block harmful actions
            // whose declared target is the charmer. We require both the
            // Charmed condition AND the `charmed_by` link so an actor
            // who is charm-immune (and thus never received the condition)
            // is unaffected even if a SetCharmedBy ran in isolation.
            if self.is_harmful()
                && let Some(caster) = encounter.actors.get(&caster_id)
                && caster.has_condition(crate::conditions::Condition::Charmed)
                && caster.charmed_by() == Some(target_id)
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
            if let Some(twin_id) = encounter.consume_twinned_spell(
                caster_id,
                self.name(),
                self.is_harmful(),
                self.reach_tiles(),
                self.requires_los(),
                sp_cost,
                original_target_id,
            ) {
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
                side_effects.append(&mut twin_effects);
            }
        }
        let costs = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        // 5e Wild Magic Sorcerer **Wild Magic Surge**: after a sorcerer
        // spell of 1st level or higher resolves, the engine rolls a d20;
        // on a 1, a random effect from the surge table fires. Sniff the
        // spell-slot level off the resolved cost — cantrips and non-spell
        // actions have no `SpellSlot` cost entry, which the trigger
        // function short-circuits on `spell_level == 0`. Surge side-
        // effects sit between the spell's effects and the cost-consume
        // effects so the surge resolves "after the spell" per RAW.
        let spell_level = crate::engine::side_effects::spell_slot_level(&costs).unwrap_or(0);
        let mut surge_effects = encounter.trigger_wild_magic_surge(caster_id, spell_level);
        side_effects.append(&mut surge_effects);
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
