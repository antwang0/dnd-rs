use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema, bonus_action_only},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, GiveResource, Resource},
        types::Coordinate,
    },
};

/// Common debit + log shared by every metamagic prime: burn `sp_cost`
/// sorcery points off the caster and emit a `"<name> <verb>… (N SP left)"`
/// line. Returns silently if the caster vanished between validation and
/// resolution. Eager-debit pattern keeps a duplicate queued use from
/// slipping past the validator — mirrors how Empowered / Quickened /
/// Heightened all needed to spend up front before the new side effect
/// could install.
fn spend_and_log(encounter: &mut EncounterInstance, caster_id: usize, sp_cost: u32, verb: &str) {
    let line = encounter
        .actors
        .get_mut(&caster_id)
        .map(|actor| {
            actor.spend_sorcery_points(sp_cost);
            format!(
                "{} {} ({} SP left).",
                actor.name(),
                verb,
                actor.sorcery_points()
            )
        });
    if let Some(line) = line {
        encounter.log(line);
    }
}

/// 5e Sorcerer **Empowered Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next spell-damage roll: dice that come up
/// at 1 or 2 are rerolled (up to CHA-mod of them, RAW). Routed through
/// `EncounterInstance::roll_empowered` so the reroll lane is opt-in per
/// damage call site (Fireball, Magic Missile, Lightning Bolt, ...).
///
/// Modeled as a self-target prime: the action installs the
/// `EmpoweredSpelling` condition on the caster (UntilStartOfNextTurn
/// timer) and decrements `sorcery_points`. The condition is consumed by
/// the engine's `roll_empowered` helper on the next spell-damage roll —
/// uncast primes tick off at the start of the caster's next turn, so a
/// sorcerer can't bank Empowered Spell rounds across rests.
pub struct EmpoweredSpell {}

impl Action for EmpoweredSpell {
    fn name(&self) -> &str {
        "empowered spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["empower", "emp"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Gate on combat-active + non-zero sorcery pool. The condition
        // itself doesn't stack (re-priming would just re-install with
        // the same timer), but blocking re-cast saves the SP.
        validate_metamagic_prime(encounter, caster_id, 1, Some(Condition::EmpoweredSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the sorcery point up front (via the shared helper) so
        // the side-effect queue can't double-install on re-validation.
        // The condition apply is the side-effect proper; the SP debit
        // lives on the actor directly since we don't have a dedicated
        // `SorceryPoint` Resource variant (avoiding a third resource
        // enum entry for a single-feature consumer).
        spend_and_log(
            encounter,
            caster_id,
            1,
            "weaves the next spell with empowered metamagic",
        );
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::EmpoweredSpelling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static EMPOWERED_SPELL: LazyLock<EmpoweredSpell> = LazyLock::new(|| EmpoweredSpell {});

/// 5e Sorcerer **Quickened Spell** metamagic. Bonus action — spend two
/// sorcery points to gain an extra Action slot this turn (RAW: "you can
/// spend 2 sorcery points to change the casting time of [a 1-action]
/// spell to 1 bonus action for this casting").
///
/// We model the spell-cost transformation as a simple action-economy
/// trade rather than a per-spell rewrite: the sorcerer spends their
/// bonus action + 2 SP and gains a fresh Action. The net effect — two
/// Action-cost casts in one turn — matches RAW's intent without
/// requiring every leveled spell to grow a "quickened?" branch in its
/// `cost()`. Cleanest hook for the existing engine.
///
/// Approximates RAW's "no other spell except a cantrip can be cast in
/// the same turn" rider — we don't enforce that gate, but the 2 SP
/// cost rate-limits the trick. Cleanest hook into the existing resource
/// model.
pub struct QuickenedSpell {}

impl Action for QuickenedSpell {
    fn name(&self) -> &str {
        "quickened spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["quicken", "qs", "qspell"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // 2 SP cost for Quickened per RAW. We check the SP pool here
        // because Resource doesn't have a `SorceryPoint` variant —
        // mirrors the Empowered Spell gating pattern. No condition gate:
        // a sorcerer can re-cast Quickened freely each turn (the 2 SP
        // cost is the rate limit).
        validate_metamagic_prime(encounter, caster_id, 2, None)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Burn 2 SP up front so a duplicate queued use can't slip past
        // the validator. Mirrors Empowered Spell's eager-debit pattern.
        spend_and_log(encounter, caster_id, 2, "quickens the next spell with metamagic");
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Action,
        })]
    }
}

pub static QUICKENED_SPELL: LazyLock<QuickenedSpell> = LazyLock::new(|| QuickenedSpell {});

/// 5e Sorcerer **Heightened Spell** metamagic. Bonus action — spend
/// three sorcery points to prime the next save-or-suck cast: the first
/// creature that makes a saving throw against that spell rolls at
/// disadvantage (RAW).
///
/// Modeled as a self-target prime mirroring `EmpoweredSpell` /
/// `QuickenedSpell`. The action installs the `HeightenedSpelling`
/// condition on the caster (UntilStartOfNextTurn timer) and decrements
/// `sorcery_points` by 3. The save chokepoint
/// `EncounterInstance::roll_save_against_caster` reads the prime and,
/// when present, combines disadvantage with the target's normal save
/// mode before consuming the prime. Single-target lockdown spells
/// (Hold Person / Hold Monster / Polymorph / Banishment / Dominate
/// Person / Dominate Monster) route their save through that helper,
/// as does the shared `resolve_burst_save_damage` AoE helper — so
/// every burst save spell benefits too, with the prime consumed by
/// the first target's save.
pub struct HeightenedSpell {}

impl Action for HeightenedSpell {
    fn name(&self) -> &str {
        "heightened spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["heighten", "hs", "hspell"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // 3 SP cost per RAW; gated on combat-active + non-stacking prime.
        // Mirrors the Empowered / Quickened pattern of eager validation.
        validate_metamagic_prime(encounter, caster_id, 3, Some(Condition::HeightenedSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend SP eagerly so a duplicate queued use can't slip past
        // the validator — same pattern as Empowered / Quickened.
        spend_and_log(encounter, caster_id, 3, "heightens the next spell with metamagic");
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::HeightenedSpelling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static HEIGHTENED_SPELL: LazyLock<HeightenedSpell> = LazyLock::new(|| HeightenedSpell {});

/// 5e Sorcerer **Careful Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next AoE: up to CHA-mod allies caught in
/// the blast auto-pass their save AND take no damage (RAW). Engine reads
/// the prime via `EncounterInstance::careful_spell_shielded`, which the
/// burst-save chokepoints (`resolve_burst_save_damage` and
/// `burst_save_damage`) consult to find the ids to skip; the prime is
/// consumed the first time the shield list is non-empty (RAW: "you
/// spend 1 sorcery point and choose a number of those creatures up to
/// your Charisma modifier" — so the prime fires the moment allies are
/// shielded). Self-target prime mirroring Empowered / Heightened in
/// every other respect.
pub struct CarefulSpell {}

impl Action for CarefulSpell {
    fn name(&self) -> &str {
        "careful spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["careful", "cs", "cspell"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        validate_metamagic_prime(encounter, caster_id, 1, Some(Condition::CarefulSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_and_log(
            encounter,
            caster_id,
            1,
            "weaves the next AoE to spare allies",
        );
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CarefulSpelling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CAREFUL_SPELL: LazyLock<CarefulSpell> = LazyLock::new(|| CarefulSpell {});

/// 5e Sorcerer **Distant Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next ranged spell: its reach doubles
/// (RAW: "When you cast a spell that has a range of 5 feet or greater,
/// you can spend 1 sorcery point to double the range of the spell").
/// Engine reads the prime via `ActorInstance::extra_spell_reach()`
/// which the action_template's reach check folds into the effective
/// range; the prime is consumed inside `Action::execute` on the first
/// ranged action that fires (reach > 2 gate, so a melee swing can't
/// burn the prime).
pub struct DistantSpell {}

impl Action for DistantSpell {
    fn name(&self) -> &str {
        "distant spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["distant", "ds", "dspell"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        validate_metamagic_prime(encounter, caster_id, 1, Some(Condition::DistantSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_and_log(
            encounter,
            caster_id,
            1,
            "stretches the next spell's range with metamagic",
        );
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::DistantSpelling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static DISTANT_SPELL: LazyLock<DistantSpell> = LazyLock::new(|| DistantSpell {});

/// Shared validator for sorcerer metamagic primes: every prime gates on
/// combat-active + sufficient sorcery points + (optionally) the prime
/// condition not already being installed (to avoid re-priming + double-
/// spending SP). The condition gate is `None` for primes that can be
/// re-cast freely each turn (e.g. Quickened, where re-paying 2 SP for
/// another extra Action is RAW-valid).
fn validate_metamagic_prime(
    encounter: &EncounterInstance,
    caster_id: usize,
    sp_cost: u32,
    no_stack_condition: Option<Condition>,
) -> bool {
    encounter.actors.get(&caster_id).is_some_and(|a| {
        a.is_combat_active()
            && a.sorcery_points() >= sp_cost
            && !no_stack_condition.is_some_and(|c| a.has_condition(c))
    })
}
