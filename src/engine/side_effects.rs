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
            Resource::Movement(amt) => format!("not enough movement ({:.1}ft needed)", amt),
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
/// A single-tile walk is just `path: vec![dest]`.
///
/// For a teleport (Misty Step, Dimension Door, etc.), use `TeleportActor`
/// instead — it bypasses per-step OAs because the actor doesn't traverse
/// intervening tiles.
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
            if let Err(e) = ei.place_actor_at(self.actor_id, dest) {
                ei.log(format!("MoveActor failed: {}", e));
                return;
            }
            // Walk-over auto-pickup: any items at the destination tile
            // get added to the actor's inventory. Logged inside.
            ei.pickup_items_at(self.actor_id, dest);
            // 5e Spike Growth: a Spiked actor takes 2d4 piercing per 5ft
            // (one tile in our grid) of movement. The damage rolls through
            // the standard pipeline so resistance / immunity is honored.
            // We resolve mid-loop so a creature with low HP can be downed
            // by spike damage and stop the walk via the is_combat_active
            // check at the top of the next iteration.
            let spiked = ei
                .actors
                .get(&self.actor_id)
                .is_some_and(|a| a.has_condition(Condition::Spiked));
            if spiked {
                let dmg = ei.roll(&crate::engine::dice::Dice::new(2, 4));
                ei.log(format!(
                    "  spike growth: 2d4({}) piercing as they step through",
                    dmg
                ));
                DealDamage {
                    actor_id: self.actor_id,
                    amount: dmg,
                    damage_type: DamageType::Piercing,
                }
                .apply(ei);
            }
            // 5e Booming Blade: the mark fires the *first* time the marked
            // creature moves voluntarily, dealing the rider damage and
            // burning off the mark (single-shot). We trip on any walked
            // step — bursts from forced movement (Telekinesis pull,
            // Thorn Whip) route through `TeleportActor` / `PullActor`
            // which skip this hook by RAW.
            let booming = ei
                .actors
                .get(&self.actor_id)
                .is_some_and(|a| a.has_condition(Condition::BoomingBladeMarked));
            if booming {
                let dmg = ei.roll(&crate::engine::dice::Dice::new(1, 8));
                ei.log(format!(
                    "  booming blade: 1d8({}) thunder as they step away",
                    dmg
                ));
                if let Some(a) = ei.actors.get_mut(&self.actor_id) {
                    a.remove_condition(Condition::BoomingBladeMarked);
                }
                DealDamage {
                    actor_id: self.actor_id,
                    amount: dmg,
                    damage_type: DamageType::Thunder,
                }
                .apply(ei);
            }
        }
    }
}

/// Move an actor to `dest` without firing per-step opportunity attacks.
/// 5e teleports (Misty Step, Dimension Door, fey step abilities) bypass
/// the normal "movement leaving threatened squares" trigger because the
/// mover never crosses the intervening tiles. We still pickup any items
/// on the destination tile so loot pickup is symmetric with `MoveActor`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct TeleportActor {
    pub actor_id: usize,
    pub dest: Coordinate,
}

impl ApplicableSideEffect for TeleportActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        let name = ei
            .actors
            .get(&self.actor_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        match ei.place_actor_at(self.actor_id, self.dest) {
            Ok(()) => {
                if !name.is_empty() {
                    ei.log(format!("{} teleports to {}.", name, self.dest));
                }
                ei.pickup_items_at(self.actor_id, self.dest);
            }
            Err(e) => ei.log(format!("TeleportActor failed: {}", e)),
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
        let name = actor.name().to_string();

        // Apply per-creature damage modifier (resistance / immunity /
        // vulnerability) before HP is touched. Logging the adjustment
        // makes it obvious why a hit did half / no damage.
        let modifier = actor.damage_modifier(self.damage_type);
        let scaled = actor.effective_damage(self.amount, self.damage_type);
        let was_concentrating = actor.is_concentrating();
        let temp_before = actor.temp_hp();
        if let Some(m) = modifier {
            let label = match m {
                DamageModifier::Resistance => "resists",
                DamageModifier::Immunity => "is immune to",
                DamageModifier::Vulnerability => "is vulnerable to",
            };
            ei.log(format!(
                "  {} {} {:?} ({} \u{2192} {})",
                name, label, self.damage_type, self.amount, scaled
            ));
        }
        if scaled == 0 {
            // Immunity (or zeroed scaling): no further effects — no HP
            // delta, no concentration save, no transition to dying.
            return;
        }

        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let (outcome, landed) = actor.take_typed_damage(self.amount, self.damage_type);
        // Regenerator suppression: flag the actor if this damage type is
        // on their suppressor list (troll vs acid/fire). The flag is
        // cleared at round_end after the heal is skipped.
        if landed > 0 {
            actor.note_regen_damage(self.damage_type);
        }
        // 5e Sleep: any damage wakes the target. Strip the Asleep
        // condition silently — the engine logs the damage line right
        // below, so we don't need a separate wake-up log.
        if landed > 0 {
            actor.remove_condition(crate::conditions::Condition::Asleep);
        }
        let temp_absorbed = temp_before.saturating_sub(actor.temp_hp());
        if temp_absorbed > 0 {
            ei.log(format!(
                "  {} absorbs {} damage (temp HP)",
                name, temp_absorbed
            ));
        }
        ei.log(format!(
            "  {} takes {} {:?} damage",
            name, landed, self.damage_type
        ));

        match outcome {
            DamageOutcome::Downed => {
                ei.log(format!("{} falls unconscious.", name));
                ei.drop_concentration(self.actor_id);
            }
            DamageOutcome::Killed => {
                // cleanup_dead_actors logs "X dies." when it removes
                // the actor; we just drop concentration here.
                ei.drop_concentration(self.actor_id);
            }
            DamageOutcome::Reduced if was_concentrating && landed > 0 => {
                // 5e: take damage while concentrating → CON save vs
                // DC max(10, dmg/2). Use the post-mitigation amount so a
                // resisted hit makes a smaller DC.
                let dc = ((landed / 2) as i32).max(10);
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
            ConditionTimer::UntilStartOfNextTurn => " (until next turn)".to_string(),
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

/// Forced movement toward a fixed point, up to `max_tiles` steps, without
/// firing opportunity attacks (5e treats forced movement as not a willing
/// move). The actor stops as soon as it can't legally advance further —
/// blocked by a wall, another actor's footprint, or hitting the target.
/// Used by Thorn Whip's pull, future Repelling Blast push, etc.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct PullActor {
    pub actor_id: usize,
    pub toward: Coordinate,
    pub max_tiles: u32,
}

impl ApplicableSideEffect for PullActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(actor) = ei.actors.get(&self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        let my_size = get_tiles_from_size(actor.size());
        let mut from = actor.location();
        // 1×1 anchor target for the gap math — that's how 5e treats the
        // tile we're being pulled toward.
        let mut remaining = self.max_tiles;
        let mut last_good = from;
        while remaining > 0 {
            let gap = footprint_chebyshev(from, my_size, self.toward, 1);
            if gap == 0 {
                break;
            }
            let step_x = (self.toward.x - from.x).signum();
            let step_y = (self.toward.y - from.y).signum();
            let next = Coordinate::new(from.x + step_x, from.y + step_y);
            if next == from {
                break;
            }
            if !ei.can_move_to(self.actor_id, next) {
                break;
            }
            from = next;
            last_good = next;
            remaining -= 1;
        }
        if last_good == actor.location() {
            return;
        }
        let dest = last_good;
        if let Err(e) = ei.place_actor_at(self.actor_id, dest) {
            ei.log(format!("PullActor failed: {}", e));
            return;
        }
        ei.log(format!("{} is pulled to {}.", name, dest));
        ei.pickup_items_at(self.actor_id, dest);
    }
}

/// Remove the *first* of the listed conditions that the actor has. Used by
/// targeted-cleanse spells (Lesser Restoration: caster picks which
/// condition to lift, but we just pop the first applicable). Logs only on
/// a successful clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveOneOfConditions {
    pub actor_id: usize,
    pub candidates: Vec<Condition>,
}

impl ApplicableSideEffect for RemoveOneOfConditions {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        for c in &self.candidates {
            if actor.remove_condition(*c) {
                ei.log(format!("{} is no longer {}.", name, c.name()));
                return;
            }
        }
    }
}

/// Promote a Dying actor to Stable without restoring any HP — the 5e
/// Spare the Dying outcome. No-op for non-Dying actors. Logs only on a
/// successful stabilization.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct StabilizeActor {
    pub actor_id: usize,
}

impl ApplicableSideEffect for StabilizeActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.stabilize() {
            ei.log(format!("{} is stabilized.", name));
        }
    }
}

/// Record which actor Charmed the target. Paired with ApplyCondition
/// (Charmed): the condition flag is read by `compute_attack_mode` for
/// future debuffs, and the `charmed_by` link is read by
/// `Action::validate_input` to block hostile actions against the
/// charmer. Pass `charmer = None` to clear (e.g. on save success); the
/// engine also clears it automatically when the Charmed condition is
/// removed via `remove_condition`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetCharmedBy {
    pub target_id: usize,
    pub charmer: Option<usize>,
}

impl ApplicableSideEffect for SetCharmedBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_charmed_by(self.charmer);
        }
    }
}

/// Record which paladin has tagged the target with Compelled Duel.
/// Pairs with ApplyCondition (Dueled): `compute_attack_mode` reads this
/// to apply disadvantage on attacks against anyone *other* than the
/// duelist. `set_dueled_by(None)` clears the link explicitly; the
/// engine also clears it automatically when the Dueled condition is
/// removed via `remove_condition`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetDueledBy {
    pub target_id: usize,
    pub duelist: Option<usize>,
}

impl ApplicableSideEffect for SetDueledBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_dueled_by(self.duelist);
        }
    }
}

/// Grant `count` Mirror Image decoys to the target. Re-application
/// overwrites the existing pool (5e: recasting refreshes the duplicates).
/// Pair with ApplyCondition (MirroredImages) so the engine knows the
/// holder has the buff active; removing the condition zeros the pool.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetMirrorImages {
    pub actor_id: usize,
    pub count: u32,
}

impl ApplicableSideEffect for SetMirrorImages {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        actor.set_mirror_images(self.count);
        if self.count > 0 {
            ei.log(format!(
                "{} is surrounded by {} duplicate{}.",
                name,
                self.count,
                if self.count == 1 { "" } else { "s" }
            ));
        }
    }
}

/// Drop the target's concentration (5e Dispel Magic). No-op if the actor
/// is missing or not concentrating. Logs the spell name that was dispelled
/// so the player sees which buff fell. If the target wasn't concentrating
/// at all, strips the first beneficial buff in `Condition::is_dispellable_buff`
/// instead — matches the spirit of Dispel Magic's "end one magical effect"
/// clause on non-concentration buffs.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct DispelMagicOn {
    pub target_id: usize,
}

impl ApplicableSideEffect for DispelMagicOn {
    fn apply(&self, ei: &mut EncounterInstance) {
        // Concentration first — that's the highest-value cleanse.
        let concentrating = ei
            .actors
            .get(&self.target_id)
            .map(|a| a.is_concentrating())
            .unwrap_or(false);
        if concentrating {
            ei.drop_concentration(self.target_id);
            return;
        }
        // Otherwise strip one beneficial condition (deterministic order
        // via the enum's natural variant ordering — we sort the snapshot
        // by the name string so the choice is stable across builds).
        let Some(actor) = ei.get_actor(self.target_id) else {
            return;
        };
        let mut buffs: Vec<Condition> = actor
            .conditions()
            .keys()
            .copied()
            .filter(|c| c.is_dispellable_buff())
            .collect();
        buffs.sort_unstable_by_key(|c| c.name());
        let Some(&c) = buffs.first() else {
            return;
        };
        let name = actor.name().to_string();
        if actor.remove_condition(c) {
            ei.log(format!("{} is no longer {} (dispelled).", name, c.name()));
        }
    }
}

/// Pull a Dying actor back to 1 HP, clearing the auxiliary Unconscious /
/// Prone conditions that come with the Dying state. No-op for Active /
/// Stable / Dead actors — 5e Revivify only works on creatures that died
/// in the last minute, but our model can't reach Dead-but-not-removed,
/// so we restrict to Dying (which covers PCs mid-death-save). Stable
/// actors are still alive at 0 HP and can be picked up by Heal.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct ReviveDying {
    pub actor_id: usize,
}

impl ApplicableSideEffect for ReviveDying {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        if !actor.is_dying() {
            return;
        }
        let name = actor.name().to_string();
        // Heal lifts the actor out of Dying via the Revived path; it also
        // clears Unconscious. We strip Prone separately — Heal doesn't
        // touch it but a revived actor is no longer flopped on the floor.
        let _ = actor.heal(1);
        actor.remove_condition(Condition::Prone);
        ei.log(format!("{} returns to life at 1 HP.", name));
    }
}

/// Permanently shift an actor's max HP by `delta`. Negative deltas model
/// Wraith Life Drain and exhaustion effects; positive deltas model Aid's
/// hp boost. Current HP rises with the cap on a positive delta (so the
/// buff is immediately useful) and is clamped down on a negative one.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AdjustMaxHp {
    pub actor_id: usize,
    pub delta: i32,
}

impl ApplicableSideEffect for AdjustMaxHp {
    fn apply(&self, ei: &mut EncounterInstance) {
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };
        let name = actor.name().to_string();
        let before = actor.max_hitpoints();
        actor.bump_max_hp(self.delta);
        let after = actor.max_hitpoints();
        if after == before {
            return;
        }
        if self.delta < 0 {
            ei.log(format!(
                "{}'s max HP drops {} \u{2192} {}.",
                name, before, after
            ));
        } else {
            ei.log(format!(
                "{}'s max HP rises {} \u{2192} {}.",
                name, before, after
            ));
        }
    }
}
