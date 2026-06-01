use std::collections::HashSet;

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
/// vanilla install-prime metamagic. The `InstallPrimeMetamagic` struct
/// below threads its (sp_cost, condition, log_verb) into this helper —
/// kept as a standalone function so a bespoke metamagic (or a test)
/// can still build the same effect shape inline.
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

/// Data-driven install-prime metamagic — Empowered / Heightened / Careful
/// / Distant / Extended / Seeking all share the exact same Action-trait
/// shape: NoArgs targeting, bonus-action-only cost, no-stack validator on
/// the prime condition, side-effect that debits `sp_cost` sorcery points
/// and installs `condition` on the caster (`UntilStartOfNextTurn` timer).
/// One struct + one static per metamagic keeps the file ~80% shorter than
/// the previous six-struct shape without losing any behavior — each
/// per-spell consumer (`roll_empowered`, `roll_save_against_caster`,
/// `careful_spell_shielded`, `extra_spell_reach`, `consume_extended_spell`,
/// `reroll_seeking_spell`) reads the same condition tag it always did.
///
/// **Quickened** (grants an Action instead of installing a condition) and
/// **Twinned** (no eager SP debit — paid at consume time) don't fit this
/// shape and stay bespoke below.
pub struct InstallPrimeMetamagic {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub sp_cost: u32,
    pub condition: Condition,
    pub log_verb: &'static str,
}

impl Action for InstallPrimeMetamagic {
    fn name(&self) -> &str {
        self.display_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        validate_metamagic_prime(encounter, caster_id, self.sp_cost, Some(self.condition))
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
            self.sp_cost,
            self.condition,
            self.log_verb,
        )
    }
}

/// 5e Sorcerer **Empowered Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next spell-damage roll: dice that come up
/// at 1 or 2 are rerolled (up to CHA-mod of them, RAW). Routed through
/// `EncounterInstance::roll_empowered` so the reroll lane is opt-in per
/// damage call site (Fireball, Magic Missile, Lightning Bolt, ...).
pub static EMPOWERED_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "empowered spell",
    aliases: &["empower", "emp"],
    sp_cost: 1,
    condition: Condition::EmpoweredSpelling,
    log_verb: "weaves the next spell with empowered metamagic",
};

/// 5e Sorcerer **Heightened Spell** metamagic. Bonus action — spend three
/// sorcery points to prime the next save-or-suck cast: the first creature
/// that makes a saving throw against that spell rolls at disadvantage
/// (RAW). Consumed at `EncounterInstance::roll_save_against_caster` —
/// every single-target save spell and the shared burst-save helper read
/// the prime, so each save path benefits without per-spell wiring.
pub static HEIGHTENED_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "heightened spell",
    aliases: &["heighten", "hs", "hspell"],
    sp_cost: 3,
    condition: Condition::HeightenedSpelling,
    log_verb: "heightens the next spell with metamagic",
};

/// 5e Sorcerer **Careful Spell** metamagic. Bonus action — spend one
/// sorcery point so up to CHA-mod allies caught in the next AoE
/// auto-pass their save AND take no damage (RAW). Engine reads the prime
/// via `EncounterInstance::careful_spell_shielded`, which the burst-save
/// chokepoints consult to find protected ids.
pub static CAREFUL_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "careful spell",
    aliases: &["careful", "cs", "cspell"],
    sp_cost: 1,
    condition: Condition::CarefulSpelling,
    log_verb: "weaves the next AoE to spare allies",
};

/// 5e Sorcerer **Distant Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next ranged spell: its reach doubles
/// (RAW: "When you cast a spell that has a range of 5 feet or greater,
/// you can spend 1 sorcery point to double the range of the spell").
/// Engine reads the prime via `ActorInstance::extra_spell_reach()`
/// which `Action::validate_input` folds into the effective range;
/// the prime is consumed inside `Action::execute` on the first ranged
/// action that fires (reach > 2 gate, so a melee swing can't burn it).
pub static DISTANT_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "distant spell",
    aliases: &["distant", "ds", "dspell"],
    sp_cost: 1,
    condition: Condition::DistantSpelling,
    log_verb: "stretches the next spell's range with metamagic",
};

/// 5e Sorcerer **Extended Spell** metamagic. Bonus action — spend one
/// sorcery point to prime the next spell that installs a long-duration
/// condition (RAW: 1 minute or longer; we gate on `Rounds(n)` with
/// n >= 10) so its timer doubles. The side-effect-assembly chokepoint
/// in `Action::execute` calls `EncounterInstance::consume_extended_spell`,
/// which walks side-effects and doubles eligible timers in place —
/// short-duration buffs and pure-damage spells leave the prime up for
/// the next eligible cast.
pub static EXTENDED_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "extended spell",
    aliases: &["extend", "es", "espell"],
    sp_cost: 1,
    condition: Condition::ExtendedSpelling,
    log_verb: "stretches the next spell's duration with metamagic",
};

/// 5e Tasha's Sorcerer **Seeking Spell** metamagic. Bonus action — spend
/// two sorcery points to prime the next spell-attack roll: if it misses,
/// it's rerolled and the new face is used (RAW: "you must use the new
/// roll"). Hooked into `spell_attack_outcome` (spells.rs) via
/// `EncounterInstance::reroll_seeking_spell`; the prime is consumed by
/// the reroll regardless of whether the new face hits. Spells that don't
/// make attack rolls (save-for-half bursts, auto-hit Magic Missile) don't
/// burn the prime, so a sorcerer can prime ahead of a Fire Bolt / Ray of
/// Frost / Chromatic Orb cast without wasting SP on a burst.
pub static SEEKING_SPELL: InstallPrimeMetamagic = InstallPrimeMetamagic {
    display_name: "seeking spell",
    aliases: &["seek", "seekspell"],
    sp_cost: 2,
    condition: Condition::SeekingSpelling,
    log_verb: "primes the next spell attack to seek its target",
};

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
        // mirrors the install-prime gating pattern. No condition gate:
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
        // the validator. Mirrors the install-prime eager-debit pattern.
        spend_and_log(encounter, caster_id, 2, "quickens the next spell with metamagic");
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Action,
        })]
    }
}

pub static QUICKENED_SPELL: QuickenedSpell = QuickenedSpell {};

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
/// Doesn't fit the `InstallPrimeMetamagic` data-driven shape because of
/// the deferred-debit timing: the consume site needs the cast level (a
/// per-cast value) to size the SP cost, so the eager-debit field on
/// `InstallPrimeMetamagic` can't be reused.
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
        // bonus action, not SP. This deviates from the install-prime
        // family (all of which eagerly debit) but matches RAW's "pay at
        // cast" timing for Twinned specifically.
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

pub static TWINNED_SPELL: TwinnedSpell = TwinnedSpell {};

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
