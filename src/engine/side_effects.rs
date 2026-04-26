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

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct MoveActor {
    pub actor_id: usize,
    pub target: Coordinate,
}

impl ApplicableSideEffect for MoveActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        let from = match ei.actors.get(&self.actor_id) {
            Some(a) => a.location(),
            None => return,
        };
        // Fire opportunity attacks before the position changes so reactors
        // can target the mover at their pre-move tile. If the mover is
        // downed by an OA the move is abandoned (they fall in their from-tile).
        if from != self.target {
            ei.dispatch_reaction(TriggerEvent::ActorLeaving {
                actor_id: self.actor_id,
                from,
                to: self.target,
            });
            if !ei
                .actors
                .get(&self.actor_id)
                .is_some_and(|a| a.is_combat_active())
            {
                return;
            }
        }

        if let Err(e) = ei.set_actor_map(self.actor_id, self.target) {
            ei.log(format!("MoveActor failed: {}", e));
            return;
        }
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.set_location(self.target);
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
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.take_damage(self.amount);
        } else {
            ei.log(format!(
                "DealDamage: actor {} missing, ignoring",
                self.actor_id
            ));
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
