use crate::actors::actor_template::{ConcentrationData, DamageOutcome, HealOutcome};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::triggers::TriggerEvent;
use crate::engine::types::{Coordinate, DamageType};

pub trait ApplicableSideEffect {
    fn apply(&self, ei: &mut EncounterInstance);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resource {
    Movement(f32),
    SpellSlot(u32),
    Action,
    BonusAction,
    Reaction,
    LegendaryAction,
}

impl Resource {
    /// Player-facing reason an actor cannot afford this resource right now.
    /// Used by the picker UI to explain why an action is greyed out instead
    /// of the misleading "no targets in reach".
    pub fn lack_description(&self) -> String {
        match self {
            Resource::Action => "out of actions".to_string(),
            Resource::BonusAction => "out of bonus actions".to_string(),
            Resource::Reaction => "no reaction available".to_string(),
            Resource::LegendaryAction => "out of legendary actions".to_string(),
            Resource::Movement(_) => "out of movement".to_string(),
            Resource::SpellSlot(lvl) => format!("no level-{} spell slot", lvl),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConsumeResource {
    pub actor_id: usize,
    pub resource: Resource,
}

impl ApplicableSideEffect for ConsumeResource {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.consume_resource(self.resource);
        } else {
            ei.log(format!(
                "ConsumeResource: actor {} missing, ignoring",
                self.actor_id
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiveResource {
    pub actor_id: usize,
    pub resource: Resource,
}

impl ApplicableSideEffect for GiveResource {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.give_resource(self.resource);
        } else {
            ei.log(format!(
                "GiveResource: actor {} missing, ignoring",
                self.actor_id
            ));
        }
    }
}

/// Walks an actor through a sequence of tiles, one step at a time, firing
/// opportunity attacks on every step that exits a threatened square. `path`
/// excludes the actor's starting tile and includes the final destination.
/// A single-tile teleport is just `path: vec![dest]`.
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct MoveActor {
    pub actor_id: usize,
    pub path: Vec<Coordinate>,
}

impl ApplicableSideEffect for MoveActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        for &dest in &self.path {
            let from = match ei.actors.get(&self.actor_id) {
                Some(a) => a.location(),
                None => return,
            };
            if from == dest {
                continue;
            }
            // Fire OAs against the mover's pre-step tile. If the OA drops
            // them, abandon the rest of the path (they fall mid-move).
            ei.dispatch_reaction(TriggerEvent::ActorLeaving {
                actor_id: self.actor_id,
                from,
                to: dest,
            });
            if !ei
                .actors
                .get(&self.actor_id)
                .is_some_and(|a| a.is_combat_active())
            {
                return;
            }
            if let Err(e) = ei.set_actor_map(self.actor_id, dest) {
                ei.log(format!("MoveActor failed: {}", e));
                return;
            }
            if let Some(actor) = ei.get_actor(self.actor_id) {
                actor.set_location(dest);
            }
            // Walk-over auto-pickup: any items at the destination tile
            // get added to the actor's inventory. Logged inside.
            ei.pickup_items_at(self.actor_id, dest);
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct DealDamage {
    pub actor_id: usize,
    pub amount: u32,
    pub damage_type: DamageType,
}

impl ApplicableSideEffect for DealDamage {
    fn apply(&self, ei: &mut EncounterInstance) {
        use crate::actors::actor_template::DamageModifier;
        let Some(actor) = ei.get_actor(self.actor_id) else {
            ei.log(format!(
                "DealDamage: actor {} missing, ignoring",
                self.actor_id
            ));
            return;
        };
        // Apply per-damage-type resistance / vulnerability / immunity
        // before the HP delta. Resistance halves (rounding down — 5e
        // RAW), vulnerability doubles, immunity zeroes.
        let modifier = actor.damage_modifier(self.damage_type);
        let adjusted = match modifier {
            Some(DamageModifier::Resistance) => self.amount / 2,
            Some(DamageModifier::Vulnerability) => self.amount.saturating_mul(2),
            Some(DamageModifier::Immunity) => 0,
            None => self.amount,
        };
        let name = actor.name().to_string();
        if let Some(m) = modifier {
            let tag = match m {
                DamageModifier::Resistance => "resists",
                DamageModifier::Vulnerability => "is vulnerable to",
                DamageModifier::Immunity => "is immune to",
            };
            ei.log(format!(
                "  {} {} {:?} \u{2014} {} \u{2192} {}",
                name, tag, self.damage_type, self.amount, adjusted
            ));
        }
        // Re-borrow after the log() above (which immutably borrowed ei).
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let outcome = actor.take_damage(adjusted);
        let was_concentrating = actor.is_concentrating();
        // actor borrow ends here.
        // Use the post-modifier amount for downstream concentration-DC math.
        let dmg_for_conc = adjusted;

        match outcome {
            DamageOutcome::Downed => {
                ei.log(format!("{} falls unconscious.", name));
                // 5e: going to 0 HP auto-drops concentration.
                ei.drop_concentration(self.actor_id);
            }
            DamageOutcome::Killed => {
                // No log here — cleanup_dead_actors logs "X dies." when
                // it removes the actor on the next pass. We just need to
                // drop concentration before the actor is gone.
                ei.drop_concentration(self.actor_id);
            }
            DamageOutcome::Reduced if was_concentrating => {
                // 5e: take damage while concentrating → CON save vs DC max(10, dmg/2).
                let dc = ((dmg_for_conc / 2) as i32).max(10);
                let save = ei.roll_save(
                    self.actor_id,
                    crate::engine::types::AbilityScoreType::Constitution,
                    dc,
                );
                if !save.passed() {
                    ei.drop_concentration(self.actor_id);
                }
            }
            DamageOutcome::Reduced | DamageOutcome::DyingFailure => {}
        }
    }
}

/// Restore HP to an actor. Logs a "comes back to consciousness" line when
/// the heal pulls them out of Dying / Stable.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct Heal {
    pub actor_id: usize,
    pub amount: u32,
}

impl ApplicableSideEffect for Heal {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        let outcome = actor.heal(self.amount);
        match outcome {
            HealOutcome::Revived => ei.log(format!(
                "{} regains consciousness ({} HP).",
                name, self.amount
            )),
            HealOutcome::Healed => ei.log(format!("{} heals {} HP.", name, self.amount)),
            HealOutcome::AlreadyFull | HealOutcome::NoOp => {}
        }
    }
}

/// Install concentration on an actor. If they were already concentrating
/// on something else, the prior concentration is dropped first (its
/// applied conditions cleared). Use this from concentration spells'
/// `side_effects` AFTER queueing the conditions the spell applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartConcentration {
    pub caster_id: usize,
    pub data: ConcentrationData,
}

impl ApplicableSideEffect for StartConcentration {
    fn apply(&self, ei: &mut EncounterInstance) {
        // Drop any prior concentration first — cleanup its effects.
        ei.drop_concentration(self.caster_id);
        if let Some(actor) = ei.get_actor(self.caster_id) {
            actor.start_concentration(self.data.clone());
            let name = actor.name().to_string();
            ei.log(format!(
                "{} begins concentrating on {}.",
                name, self.data.spell_name
            ));
        }
    }
}

/// Add a status condition to an actor with a given timer. No-op if the
/// actor is missing; if the condition was already present its timer is
/// replaced (no stacking semantics yet — revisit when needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyCondition {
    pub actor_id: usize,
    pub condition: Condition,
    pub timer: ConditionTimer,
}

impl ApplicableSideEffect for ApplyCondition {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.add_condition(self.condition, self.timer) {
            let suffix = match self.timer {
                ConditionTimer::Permanent => String::new(),
                ConditionTimer::Rounds(n) => format!(" ({} round{})", n, if n == 1 { "" } else { "s" }),
            };
            ei.log(format!("{} is now {}{}.", name, self.condition.name(), suffix));
        }
    }
}

/// Remove a status condition from an actor. No-op if the actor is missing
/// or doesn't have the condition.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct RemoveCondition {
    pub actor_id: usize,
    pub condition: Condition,
}

impl ApplicableSideEffect for RemoveCondition {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.remove_condition(self.condition) {
            ei.log(format!("{} is no longer {}.", name, self.condition.name()));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SkipTurn {}

impl ApplicableSideEffect for SkipTurn {
    fn apply(&self, ei: &mut EncounterInstance) {
        ei.skip_turn();
    }
}

/// Toggle the actor's per-turn dodge flag. Set true by the Dodge
/// action; cleared automatically in `reset_for_new_round`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetDodging {
    pub actor_id: usize,
    pub dodging: bool,
}

impl ApplicableSideEffect for SetDodging {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        actor.set_dodging(self.dodging);
        if self.dodging {
            ei.log(format!("{} takes the Dodge action.", name));
        }
    }
}

/// Toggle the actor's per-turn disengage flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetDisengaging {
    pub actor_id: usize,
    pub disengaging: bool,
}

impl ApplicableSideEffect for SetDisengaging {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        actor.set_disengaging(self.disengaging);
        if self.disengaging {
            ei.log(format!("{} disengages — no OAs this turn.", name));
        }
    }
}

/// Grant a Help bonus on `recipient_id`'s next attack against
/// `against_id`. Stored on the recipient via `set_help_grant`. Consumed
/// by `weapon_attack` when the recipient swings at `against_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantHelp {
    pub helper_id: usize,
    pub recipient_id: usize,
    pub against_id: usize,
}

impl ApplicableSideEffect for GrantHelp {
    fn apply(&self, ei: &mut EncounterInstance) {
        use crate::actors::actor_template::HelpGrant;
        let Some(recipient) = ei.get_actor(self.recipient_id) else {
            return;
        };
        recipient.set_help_grant(Some(HelpGrant {
            helper_id: self.helper_id,
            against: self.against_id,
        }));
        let recipient_name = recipient.name().to_string();
        let helper_name = ei
            .actors
            .get(&self.helper_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        let against_name = ei
            .actors
            .get(&self.against_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        ei.log(format!(
            "{} helps {} — advantage on next attack vs {}.",
            helper_name, recipient_name, against_name
        ));
    }
}

/// Apply a Bless buff to a recipient: they roll attacks and saves with
/// advantage for `rounds` rounds. Today we collapse the +1d4 mechanic
/// into full advantage — coarser but moves the dial in the right
/// direction without modeling the per-die add. Concentration is
/// installed by the spell's side_effects, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyBless {
    pub actor_id: usize,
    pub rounds: u32,
}

impl ApplicableSideEffect for ApplyBless {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        actor.apply_bless(self.rounds);
        ei.log(format!("{} is blessed.", name));
    }
}

/// Apply Shield of Faith to a recipient: +2 AC for `rounds` rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyShieldOfFaith {
    pub actor_id: usize,
    pub rounds: u32,
}

impl ApplicableSideEffect for ApplyShieldOfFaith {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        actor.apply_shield_of_faith(self.rounds);
        ei.log(format!(
            "{} is shielded by faith (+2 AC).",
            name
        ));
    }
}
