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

/// Shared "eager-debit + install prime condition" body used by every
/// vanilla metamagic prime (Empowered / Heightened / Careful / Distant /
/// Extended — the five primes whose entire side-effect is "spend N SP
/// and tag the caster with the prime condition until their next turn").
/// Quickened (no condition install — grants an extra Action instead)
/// and Twinned (no eager SP debit — paid at consume time) don't fit
/// this shape and stay bespoke.
fn install_prime(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    sp_cost: u32,
    condition: Condition,
    log_verb: &str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    spend_and_log(encounter, caster_id, sp_cost, log_verb);
    vec![Box::new(ApplyCondition {
        actor_id: caster_id,
        condition,
        timer: ConditionTimer::UntilStartOfNextTurn,
    })]
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
        // Routes through the shared `install_prime` helper: spend 1 SP
        // eagerly (so a duplicate queued use can't slip past the
        // validator) and install the EmpoweredSpelling tag on the
        // caster. The SP debit lives on the actor directly since we
        // don't have a dedicated `SorceryPoint` Resource variant
        // (avoiding a third resource enum entry for a single-feature
        // consumer).
        install_prime(
            encounter,
            caster_id,
            1,
            Condition::EmpoweredSpelling,
            "weaves the next spell with empowered metamagic",
        )
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
        install_prime(
            encounter,
            caster_id,
            3,
            Condition::HeightenedSpelling,
            "heightens the next spell with metamagic",
        )
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
        install_prime(
            encounter,
            caster_id,
            1,
            Condition::CarefulSpelling,
            "weaves the next AoE to spare allies",
        )
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
        install_prime(
            encounter,
            caster_id,
            1,
            Condition::DistantSpelling,
            "stretches the next spell's range with metamagic",
        )
    }
}

pub static DISTANT_SPELL: LazyLock<DistantSpell> = LazyLock::new(|| DistantSpell {});

/// 5e Sorcerer **Twinned Spell** metamagic. Bonus action — prime the next
/// single-target spell to re-fire against a second creature in range.
///
/// RAW: cost is `max(1, spell_level)` sorcery points, paid at the moment
/// the twinned spell fires (not at the prime). Modeled as a self-target
/// prime that installs the `TwinnedSpelling` condition; the SP debit
/// lives at the consume site (`EncounterInstance::consume_twinned_spell`)
/// in `Action::execute`, which sniffs the spell's level off its cost and
/// pays the SP at fire time. Cantrips cost 1 SP — the default when no
/// `SpellSlot` cost is present.
///
/// The prime itself doesn't debit SP up front (RAW timing) but the
/// validator does check the pool is non-empty so the prime never opens
/// with no path to a payoff. If the pool runs short when the spell
/// fires, the twin quietly fails and the original single-target cast
/// lands as normal — no SP is wasted on a no-op twin.
///
/// Modeled as a self-target prime mirroring the other metamagics; the
/// consume site lives in `EncounterInstance` so every single-target
/// action benefits automatically without per-spell wiring.
///
/// **Limitation — concentration spells.** Twinning a single-target
/// concentration spell (Hold Person, Witch Bolt, Hex, Hunter's Mark,
/// etc.) lands the damage and the immediate condition on both targets,
/// but only the *original* target keeps the concentration-bound
/// condition long-term. The twin target's persistent condition gets
/// scrubbed when the engine resolves the second `StartConcentration`
/// side-effect, which drops the prior concentration data per RAW
/// "you can only concentrate on one spell at a time." A faithful merge
/// would require side-effect downcasting and is out of scope; the
/// damaging cantrips that dominate Twinned Spell's canonical use
/// (Fire Bolt, Ray of Frost, Chill Touch, Chromatic Orb, Inflict
/// Wounds, etc.) are unaffected by this gap.
pub struct TwinnedSpell {}

impl Action for TwinnedSpell {
    fn name(&self) -> &str {
        "twinned spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["twin", "ts", "tspell"]
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
        // 1 SP floor cost — the prime gate just checks the pool is non-empty
        // so the sorcerer can't open a Twinned prime with no SP to spend
        // when the spell fires. Higher-level spells pay the extra SP at
        // consume time. No-stack on the prime condition: re-priming would
        // be a no-op and would risk a double-debit at the next consume.
        validate_metamagic_prime(encounter, caster_id, 1, Some(Condition::TwinnedSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Don't spend SP up front for Twinned — RAW pays at the consume
        // site, where the spell's level is known. The prime install is
        // cheap (just a condition tag) so a wasted prime only costs a
        // bonus action, not SP. This deviates from Empowered / Heightened
        // / Careful / Distant (all of which eagerly debit) but matches
        // RAW's "pay at cast" timing for Twinned specifically.
        encounter.log(format!(
            "{} primes the next spell with twinned metamagic.",
            encounter
                .actors
                .get(&caster_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default(),
        ));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::TwinnedSpelling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static TWINNED_SPELL: LazyLock<TwinnedSpell> = LazyLock::new(|| TwinnedSpell {});

/// 5e Sorcerer **Extended Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next spell with a duration of 1 minute or
/// longer; its duration doubles (RAW caps at 24 hours).
///
/// Modeled as a self-target prime mirroring Empowered / Heightened /
/// Careful / Distant. The action installs the `ExtendedSpelling`
/// condition on the caster (UntilStartOfNextTurn timer) and decrements
/// `sorcery_points` by 1. The side-effect-assembly chokepoint in
/// `Action::execute` calls `EncounterInstance::consume_extended_spell`,
/// which walks the action's side_effects and doubles any
/// `ApplyCondition` whose timer is `Rounds(n)` with n >= 10 (= 1 minute
/// in our 6-second rounds). The prime is consumed the first time at
/// least one eligible timer is doubled; short-duration spells
/// (`UntilStartOfNextTurn`) and pure-damage spells don't burn it, so a
/// sorcerer who primes Extended Spell and then casts Magic Missile
/// keeps the prime up for the next buff / debuff cast.
pub struct ExtendedSpell {}

impl Action for ExtendedSpell {
    fn name(&self) -> &str {
        "extended spell"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["extend", "es", "espell"]
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
        validate_metamagic_prime(encounter, caster_id, 1, Some(Condition::ExtendedSpelling))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        install_prime(
            encounter,
            caster_id,
            1,
            Condition::ExtendedSpelling,
            "stretches the next spell's duration with metamagic",
        )
    }
}

pub static EXTENDED_SPELL: LazyLock<ExtendedSpell> = LazyLock::new(|| ExtendedSpell {});

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
