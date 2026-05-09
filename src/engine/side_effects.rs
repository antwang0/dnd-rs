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
        use crate::engine::types::DamageModifier;

        let Some(actor) = ei.get_actor(self.actor_id) else {
            ei.log(format!(
                "DealDamage: actor {} missing, ignoring",
                self.actor_id
            ));
            return;
        };
        // 5e: resistance halves, vulnerability doubles, immunity zeroes.
        // Applied before taking damage so the dying-actor branch and
        // concentration-DC math see the post-mitigation number.
        let adjusted = actor.adjusted_damage(self.amount, self.damage_type);
        let name = actor.name().to_string();
        // Apply per-creature damage modifier (resistance / immunity /
        // vulnerability) before HP is touched. Logging the adjustment
        // makes it obvious why a fireball did half / no damage.
        let modifier = actor.damage_modifier(self.damage_type);
        let final_amount = match modifier {
            Some(m) => m.apply(self.amount),
            None => self.amount,
        };
        if let Some(m) = modifier {
            let label = match m {
                DamageModifier::Resistance => "resists",
                DamageModifier::Immunity => "is immune to",
                DamageModifier::Vulnerability => "is vulnerable to",
            };
            ei.log(format!(
                "  {} {} {:?} ({} -> {})",
                name, label, self.damage_type, self.amount, final_amount
            ));
        }
        if final_amount == 0 && matches!(modifier, Some(DamageModifier::Immunity)) {
            // No further effects — immunity short-circuits everything
            // (no concentration save, no transition to dying).
            return;
        }

        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let outcome = actor.take_damage(final_amount);
        let was_concentrating = actor.is_concentrating();
        if scaled != self.amount {
            let suffix = if scaled == 0 {
                "immune"
            } else if scaled < self.amount {
                "resistant"
            } else {
                "vulnerable"
            };
            ei.log(format!(
                "  {} is {} to {:?}: {} -> {} damage",
                name, suffix, self.damage_type, self.amount, scaled
            ));
        }
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let outcome = actor.take_damage(scaled);
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
            DamageOutcome::Reduced if was_concentrating && scaled > 0 => {
                // 5e: take damage while concentrating → CON save vs DC max(10, dmg/2).
                // Use the post-modifier amount: a resisted hit is half
                // damage and the save DC follows the actually-felt damage.
                let dc = ((final_amount / 2) as i32).max(10);
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

/// Grant temp HP to an actor. 5e: temp HP doesn't stack — the higher
/// pool replaces the lower. Logs only when the new pool actually grows.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct GrantTempHp {
    pub actor_id: usize,
    pub amount: u32,
}

impl ApplicableSideEffect for GrantTempHp {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.grant_temp_hp(self.amount) {
            ei.log(format!("{} gains {} temporary HP.", name, self.amount));
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

/// Grant `amount` temporary HP. 5e: doesn't stack — the bigger of the
/// existing pool and the new grant wins. No-op if `amount` is 0 or the
/// actor is missing.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct GainTempHp {
    pub actor_id: usize,
    pub amount: u32,
}

impl ApplicableSideEffect for GainTempHp {
    fn apply(&self, ei: &mut EncounterInstance) {
        if self.amount == 0 {
            return;
        }
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        let before = actor.temp_hp();
        let after = actor.gain_temp_hp(self.amount);
        if after > before {
            ei.log(format!("{} gains {} temp HP.", name, after));
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
        let newly_added = actor.add_condition(self.condition, self.timer);
        let suffix = match self.timer {
            ConditionTimer::Permanent => String::new(),
            ConditionTimer::Rounds(n) => {
                format!(" ({} round{})", n, if n == 1 { "" } else { "s" })
            }
        };
        if newly_added {
            ei.log(format!("{} is now {}{}.", name, self.condition.name(), suffix));
        } else {
            // Re-application — log the refresh so the player sees that
            // the timer changed (e.g. a re-cast Bless extending duration).
            ei.log(format!(
                "{}'s {} refreshes{}.",
                name,
                self.condition.name(),
                suffix
            ));
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

/// Toggle the actor's Dodge flag. The flag is consumed at the start of
/// their next turn by `reset_for_new_round`. While set, attacks against
/// the actor have disadvantage and they have advantage on DEX saves.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
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

/// Toggle the actor's Disengage flag. While set, this actor's movement
/// doesn't provoke opportunity attacks. Cleared at start-of-next-turn.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
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
            ei.log(format!("{} disengages.", name));
        }
    }
}

/// Grant temporary HP. Doesn't stack — replaces the existing pool only
/// if larger (see `ActorInstance::grant_temp_hp`).
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct GrantTempHp {
    pub actor_id: usize,
    pub amount: u32,
}

impl ApplicableSideEffect for GrantTempHp {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.grant_temp_hp(self.amount) {
            ei.log(format!("{} gains {} temp HP.", name, self.amount));
        }
    }
}

/// Modify an actor's flat attack-roll buff (used by Bless). Pair with
/// concentration installation so the buff drops cleanly when the spell
/// ends. Negative deltas remove the buff on cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AdjustAttackBuff {
    pub actor_id: usize,
    pub delta: i32,
}

impl ApplicableSideEffect for AdjustAttackBuff {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.add_attack_bonus_buff(self.delta);
        }
    }
}

/// Same as AdjustAttackBuff but for the save-roll buff lane.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AdjustSaveBuff {
    pub actor_id: usize,
    pub delta: i32,
}

impl ApplicableSideEffect for AdjustSaveBuff {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.add_save_bonus_buff(self.delta);
        }
    }
}
