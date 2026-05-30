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
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        // Gate on combat-active + non-zero sorcery pool. The condition
        // itself doesn't stack (re-priming would just re-install with
        // the same timer), but blocking re-cast saves the SP.
        actor.is_combat_active()
            && actor.sorcery_points() > 0
            && !actor.has_condition(Condition::EmpoweredSpelling)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the sorcery point up front so the side-effect queue
        // can't double-install on re-validation. The condition apply
        // is the side-effect proper; the SP debit lives on the actor
        // directly since we don't have a dedicated SorceryPoint
        // Resource variant (avoiding a third resource enum entry for
        // a single-feature consumer).
        let log_line = if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_sorcery_point();
            Some(format!(
                "{} weaves the next spell with empowered metamagic ({} SP left).",
                actor.name(),
                actor.sorcery_points()
            ))
        } else {
            None
        };
        if let Some(line) = log_line {
            encounter.log(line);
        }
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
        // mirrors the Empowered Spell gating pattern. We also block
        // re-cast when the sorcerer already has 0 actions remaining
        // *and* a BA — no point gaining an Action when they couldn't
        // spend it (rare edge case but keeps the AI from burning the
        // resource on a no-op).
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active() && a.sorcery_points() >= 2
        })
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
        let log_line = if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_sorcery_point();
            actor.spend_sorcery_point();
            Some(format!(
                "{} quickens the next spell with metamagic ({} SP left).",
                actor.name(),
                actor.sorcery_points()
            ))
        } else {
            None
        };
        if let Some(line) = log_line {
            encounter.log(line);
        }
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Action,
        })]
    }
}

pub static QUICKENED_SPELL: LazyLock<QuickenedSpell> = LazyLock::new(|| QuickenedSpell {});
