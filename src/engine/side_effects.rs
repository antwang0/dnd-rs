use crate::actors::actor_template::{ConcentrationData, DamageOutcome, HealOutcome};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::triggers::TriggerEvent;
use crate::engine::types::{Coordinate, DamageType};

pub trait ApplicableSideEffect {
    fn apply(&self, ei: &mut EncounterInstance);

    /// 5e Sorcerer Extended Spell metamagic hook. If this side-effect
    /// installs a long-duration condition (RAW: 1 minute or longer; we
    /// gate on `Rounds(n)` with `n >= EXTENDED_SPELL_MIN_ROUNDS`),
    /// double the timer in place (saturating at
    /// `EXTENDED_SPELL_MAX_ROUNDS` to keep timers from running away on
    /// re-extension) and return true. Default: no-op (returns false).
    /// Called once per side-effect in `Action::execute` when the caster
    /// has the `ExtendedSpelling` prime up; the first `true` return
    /// consumes the prime.
    fn extend_duration(&mut self) -> bool {
        false
    }

    /// 5e Tasha's Sorcerer Transmuted Spell metamagic hook. If this
    /// side-effect deals damage of one of the six elemental types
    /// (acid, cold, fire, lightning, poison, thunder), remap the damage
    /// type to `new_type` and return true. Default: no-op (returns
    /// false). Called once per side-effect in `Action::execute` when the
    /// caster has the `TransmutedSpelling` prime up; the first `true`
    /// return consumes the prime. Side-effects that don't deal elemental
    /// damage (force / radiant / necrotic / psychic / physical) leave
    /// the prime up for the next eligible cast.
    fn remap_damage_type(&mut self, _new_type: DamageType) -> bool {
        false
    }

    /// If this side-effect carries a target actor + elemental damage
    /// type, return the pair. Used by Transmuted Spell to pick the best
    /// replacement element based on the primary target's resistance
    /// profile. Default: `None` (side-effect either isn't damage or
    /// isn't aimed at a single actor — caller falls back to a
    /// "nearest enemy" heuristic).
    fn elemental_damage_target(&self) -> Option<(usize, DamageType)> {
        None
    }

    /// If this side-effect starts a concentration, hand back the caster
    /// it belongs to together with the per-target payload it will clean
    /// up on drop. Default `None` — every side-effect that isn't a
    /// concentration install.
    ///
    /// Read by `fold_doubled_concentration` so a doubling feature can
    /// tell whether a re-fired cast produced a second, competing
    /// install. See that function for why the competition is a bug.
    fn concentration_payload(&self) -> Option<ConcentrationPayload<'_>> {
        None
    }

    /// Fold another install's per-target payload into this one. Returns
    /// true when absorbed. Default: no-op.
    fn absorb_concentration_payload(&mut self, _extra: &ConcentrationPayload<'_>) -> bool {
        false
    }
}

/// The per-target bookkeeping a `StartConcentration` will roll back
/// when the concentration drops. Borrowed out of the side-effect so
/// `fold_doubled_concentration` can compare and merge two installs
/// without cloning the whole `ConcentrationData`.
pub struct ConcentrationPayload<'a> {
    pub caster_id: usize,
    pub conditions: &'a [(usize, Condition)],
    pub attack_buffs: &'a [(usize, i32)],
    pub save_buffs: &'a [(usize, i32)],
    pub damage_buffs: &'a [(usize, i32)],
}

/// Merge a doubled cast's second `StartConcentration` into the first,
/// dropping the redundant install from `doubled`.
///
/// **The bug this fixes.** Two features re-run a single-target spell's
/// `side_effects` against a second creature and append the result: the
/// Sorcerer's Twinned Spell metamagic and the Enchantment Wizard's
/// Split Enchantment. When the spell concentrates, that produces *two*
/// `StartConcentration` effects for the same caster — and
/// `StartConcentration::apply` opens by calling `drop_concentration`,
/// which tears down whatever the caster was holding. The second install
/// therefore ripped the condition straight back off the first target,
/// so a twinned Hold Person / Heroism / Bestow Curse / Crown of Madness
/// landed on the *second* target only. The feature silently did nothing
/// for the slot it charged, on exactly the spells worth twinning.
///
/// RAW is unambiguous: one cast is one spell and one concentration, and
/// it holds both targets. `ConcentrationData` already models that — its
/// rollback lists are per-target — so the fix is to merge rather than
/// to stack: the surviving install cleans up both targets when it ends,
/// and one broken concentration check still drops the whole spell.
///
/// Returns true when a merge happened. Cheap and total when it doesn't:
/// a non-concentration spell has no payload on either side and the
/// walk falls straight through.
pub fn fold_doubled_concentration(
    primary: &mut [Box<dyn ApplicableSideEffect>],
    doubled: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> bool {
    // Find the doubled install (there is at most one per cast) and pull
    // its payload out into owned form — the borrow can't outlive the
    // walk over `primary` below.
    let Some(idx) = doubled
        .iter()
        .position(|se| se.concentration_payload().is_some())
    else {
        return false;
    };
    let owned = {
        let payload = doubled[idx].concentration_payload().unwrap();
        (
            payload.caster_id,
            payload.conditions.to_vec(),
            payload.attack_buffs.to_vec(),
            payload.save_buffs.to_vec(),
            payload.damage_buffs.to_vec(),
        )
    };
    let extra = ConcentrationPayload {
        caster_id: owned.0,
        conditions: &owned.1,
        attack_buffs: &owned.2,
        save_buffs: &owned.3,
        damage_buffs: &owned.4,
    };
    let merged = primary.iter_mut().any(|se| {
        se.concentration_payload()
            .is_some_and(|p| p.caster_id == extra.caster_id)
            && se.absorb_concentration_payload(&extra)
    });
    if merged {
        // The doubled install is now redundant — leaving it in would
        // re-introduce the very `drop_concentration` that caused the bug.
        doubled.remove(idx);
    }
    merged
}

/// 5e Sorcerer Extended Spell: minimum `Rounds(n)` timer that qualifies
/// for the doubling. RAW gates on "1 minute or longer" — we use 1 round
/// ≈ 6 seconds, so 10 rounds ≈ 1 minute is the natural threshold.
pub const EXTENDED_SPELL_MIN_ROUNDS: u32 = 10;

/// 5e Sorcerer Extended Spell: maximum `Rounds(n)` timer after doubling.
/// RAW caps the extension at 24 hours (~14,400 combat rounds, which
/// dwarfs any encounter span). We pick 200 here because the longest
/// canonical install in this engine is `Rounds(100)` (Mage Armor / Mind
/// Blank's "effectively permanent for the encounter" sentinel), so 200
/// is the smallest cap that lets every base timer double cleanly without
/// risking a runaway value on a future install that nudges past 100.
pub const EXTENDED_SPELL_MAX_ROUNDS: u32 = 200;

/// Damage types considered "spell-typical" for the purpose of the
/// Ancients Paladin's **Aura of Warding** (RAW: "damage from spells").
/// We approximate the RAW clause by pattern-matching on the closed set
/// of magical damage types that show up on every canonical damage-
/// dealing spell in the SRD. Physical types (bludgeoning / piercing /
/// slashing) are excluded — they belong to weapon attacks and the
/// nearby ally shouldn't get their greatsword swing accidentally
/// halved. Poison is also excluded — RAW spells that deal poison
/// damage (Poison Spray, Cloudkill, etc.) do exist, but the tag also
/// covers monster poison bites where the aura shouldn't fire. We err
/// toward the conservative "close but not RAW-exact" pick over an
/// invasive `is_spell` plumbing pass through every `DealDamage` call
/// site — the load-bearing coverage (fireball, lightning bolt, cone
/// of cold, thunderwave, disintegrate, magic missile, sunburst, and
/// every future evocation blast) all fall inside the set.
pub const SPELL_TYPICAL_DAMAGE_TYPES: &[DamageType] = &[
    DamageType::Acid,
    DamageType::Cold,
    DamageType::Fire,
    DamageType::Force,
    DamageType::Lightning,
    DamageType::Necrotic,
    DamageType::Psychic,
    DamageType::Radiant,
    DamageType::Thunder,
];

/// Walk `side_effects` and call `extend_duration` on each entry. Returns
/// true if at least one entry doubled its timer (i.e. the cast carried
/// an eligible long-duration install). Used by both the original
/// Extended Spell consume path and the Twinned Spell re-issue path so a
/// Twinned + Extended cast sees the doubled timer applied to both the
/// primary and twin targets RAW.
pub fn extend_side_effect_timers(
    side_effects: &mut [Box<dyn ApplicableSideEffect>],
) -> bool {
    let mut extended = false;
    for se in side_effects.iter_mut() {
        if se.extend_duration() {
            extended = true;
        }
    }
    extended
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

/// Sniff the spell-slot level off a resolved cost vec. Returns
/// `Some(lvl)` if any `Resource::SpellSlot(lvl)` entry is present,
/// `None` otherwise. Used by the cross-cutting `Action::execute` hooks
/// (Twinned Spell SP sizing, Wild Magic Surge trigger gate) to detect
/// "this action is a leveled spell cast" without re-walking the cost
/// vec by hand at every call site.
pub fn spell_slot_level(costs: &[Resource]) -> Option<u32> {
    costs.iter().find_map(|c| match c {
        Resource::SpellSlot(lvl) => Some(*lvl),
        _ => None,
    })
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

/// Apply a "step through hostile terrain" damage rider to `mover_id` if
/// they currently carry `condition`. Rolls `dice`, logs
/// `  {label}: NdM(roll) {damage_type} {trailer}`, then deals the rolled
/// damage to the mover through the standard `DealDamage` pipeline
/// (resistance / immunity / death-save transitions all honored). If
/// `consume` is true the condition is stripped after the rider fires
/// — Booming Blade is single-shot; Spike Growth fires on every step
/// for the rest of the move.
///
/// Centralizes the recurring "if mover has cond X, roll Y dice, log,
/// damage self" shape used by the in-loop step riders in `MoveActor`.
/// Adding a new per-step self-damage rider becomes a single
/// `apply_per_step_self_rider(...)` call instead of another three-line
/// `has_condition → roll → log → DealDamage` block in the loop body.
#[allow(clippy::too_many_arguments)]
fn apply_per_step_self_rider(
    ei: &mut EncounterInstance,
    mover_id: usize,
    condition: Condition,
    dice: crate::engine::dice::Dice,
    damage_type: DamageType,
    label: &str,
    trailer: &str,
    consume: bool,
) {
    let active = ei
        .actors
        .get(&mover_id)
        .is_some_and(|a| a.has_condition(condition));
    if !active {
        return;
    }
    let dmg = ei.roll(&dice);
    ei.log(format!(
        "  {}: {}({}) {:?} {}",
        label, dice, dmg, damage_type, trailer
    ));
    if consume
        && let Some(a) = ei.actors.get_mut(&mover_id)
    {
        a.remove_condition(condition);
    }
    DealDamage {
        actor_id: mover_id,
        amount: dmg,
        damage_type,
    }
    .apply(ei);
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
            apply_per_step_self_rider(
                ei,
                self.actor_id,
                Condition::Spiked,
                crate::engine::dice::Dice::new(2, 4),
                DamageType::Piercing,
                "spike growth",
                "as they step through",
                false,
            );
            // 5e Booming Blade: the mark fires the *first* time the marked
            // creature moves voluntarily, dealing the rider damage and
            // burning off the mark (single-shot). We trip on any walked
            // step — bursts from forced movement (Telekinesis pull,
            // Thorn Whip) route through `TeleportActor` / `PullActor`
            // which skip this hook by RAW.
            apply_per_step_self_rider(
                ei,
                self.actor_id,
                Condition::BoomingBladeMarked,
                crate::engine::dice::Dice::new(1, 8),
                DamageType::Thunder,
                "booming blade",
                "as they step away",
                true,
            );
            // 5e Ashardalon's Stride (TCE level-3 transmutation,
            // concentration). The caster's blazing wake scorches every
            // footprint-adjacent enemy as they pass: each tracked enemy
            // takes 1d6 fire per step. Symmetric to Spike Growth's "step
            // and burn" envelope, but the rider lives on the *mover*
            // and damages everyone else nearby instead of the mover
            // themselves — so the routine fires once per step, scanning
            // for adjacent enemies on the new tile and rolling a fresh
            // 1d6 per victim. Damage rolls through the standard
            // pipeline so resistance / immunity is honored. The
            // `combat_active_enemy_ids_adjacent` chokepoint keeps the
            // caster-team / footprint-zero filter in one place.
            let striding = ei
                .actors
                .get(&self.actor_id)
                .is_some_and(|a| a.has_condition(Condition::AshardalonStriding));
            if striding {
                let adjacent = ei.combat_active_enemy_ids_adjacent(self.actor_id);
                for tid in adjacent {
                    let dmg = ei.roll(&crate::engine::dice::Dice::new(1, 6));
                    ei.log(format!(
                        "  ashardalon's stride: 1d6({}) fire as wake scorches {}",
                        dmg,
                        ei.actor_name(tid)
                    ));
                    DealDamage {
                        actor_id: tid,
                        amount: dmg,
                        damage_type: DamageType::Fire,
                    }
                    .apply(ei);
                }
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
        let name = ei.actor_name(self.actor_id);
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

        // Snapshot the actor's name and self-reduction state before any
        // encounter-wide lookups — the aura check below re-borrows `ei`
        // immutably and can't coexist with a live `&mut actor`.
        let (name, has_own_reduction) = {
            let Some(actor) = ei.get_actor(self.actor_id) else {
                ei.log(format!(
                    "DealDamage: actor {} missing, ignoring",
                    self.actor_id
                ));
                return;
            };
            (
                actor.name().to_string(),
                actor.has_own_typed_reduction(self.damage_type),
            )
        };

        // 5e Ancients Paladin **Aura of Warding** (lv7): allies inside
        // a paladin's 10-ft aura resist damage from spells. We
        // approximate "damage from spells" with the closed set of
        // spell-typical damage types (`SPELL_TYPICAL_DAMAGE_TYPES`);
        // weapon-only physical types + poison are excluded so a
        // greatsword swing through the bubble doesn't get accidentally
        // halved. The halving runs BEFORE `effective_damage` so the
        // standard "one halving per damage instance" rule still holds —
        // if the target already halves / zeros / doubles the type via
        // their own template / condition / item lanes, the aura no-ops
        // and the existing resistance pipeline fires unchanged.
        let aura_halves = SPELL_TYPICAL_DAMAGE_TYPES.contains(&self.damage_type)
            && !has_own_reduction
            && ei.is_in_aura_of_warding(self.actor_id);
        let raw_amount = if aura_halves {
            let halved = self.amount / 2;
            ei.log(format!(
                "  {} shrugs off spell magic (aura of warding: {} \u{2192} {} {:?})",
                name, self.amount, halved, self.damage_type
            ));
            halved
        } else {
            self.amount
        };
        let Some(actor) = ei.get_actor(self.actor_id) else {
            return;
        };

        // Apply per-creature damage modifier (resistance / immunity /
        // vulnerability) before HP is touched. Logging the adjustment
        // makes it obvious why a hit did half / no damage.
        let modifier = actor.damage_modifier(self.damage_type);
        let scaled = actor.effective_damage(raw_amount, self.damage_type);
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
                name, label, self.damage_type, raw_amount, scaled
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
        // Snapshot the Warding Bond partner *before* HP changes —
        // if this damage drops the actor unconscious, `remove_condition`
        // (via downstream cleanup) would clear the link before we read
        // it. Mirror damage to the partner uses the post-resistance
        // amount (`scaled`), matching RAW: "you take the same amount of
        // damage" applies to whatever the bonded ally actually absorbs.
        let warding_partner = if actor.has_condition(crate::conditions::Condition::WardingBonded) {
            actor.warding_partner()
        } else {
            None
        };
        let ward_before = actor.arcane_ward();
        let (outcome, landed) = actor.take_typed_damage(raw_amount, self.damage_type);
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
        // 5e Displacer Beast: displacement flickers off the moment the
        // creature takes any damage. It restores at the start of its
        // next turn (handled by start_turn_for).
        let displacement_dropped = landed > 0
            && actor.has_condition(crate::conditions::Condition::Displaced);
        if displacement_dropped {
            actor.remove_condition(crate::conditions::Condition::Displaced);
        }
        let temp_after = actor.temp_hp();
        let temp_absorbed = temp_before.saturating_sub(temp_after);
        // 5e Armor of Agathys: the ice shield IS the temp HP. The
        // moment temp HP is exhausted, the spell's retaliation rider
        // dies with it — strip the condition so subsequent melee hits
        // don't get free cold damage off a depleted shield.
        let agathys_shatters = temp_absorbed > 0
            && temp_after == 0
            && actor.has_condition(crate::conditions::Condition::AgathysShielded);
        if agathys_shatters {
            actor.remove_condition(crate::conditions::Condition::AgathysShielded);
        }
        // Log the two absorption pools in the order they drained, so a
        // reader can see why a hit that "should" have landed didn't.
        let ward_absorbed = ward_before.saturating_sub(actor.arcane_ward());
        if ward_absorbed > 0 {
            ei.log(format!(
                "  {}'s arcane ward absorbs {} damage",
                name, ward_absorbed
            ));
        }
        if temp_absorbed > 0 {
            ei.log(format!(
                "  {} absorbs {} damage (temp HP)",
                name, temp_absorbed
            ));
        }
        if agathys_shatters {
            ei.log(format!("{}'s armor of agathys shatters.", name));
        }
        if displacement_dropped {
            ei.log(format!("  {}'s displacement flickers off.", name));
        }
        ei.log(format!(
            "  {} takes {} {:?} damage",
            name, landed, self.damage_type
        ));

        match outcome {
            DamageOutcome::Downed => {
                // 5e Barbarian Relentless Rage intercept: a raging
                // barbarian with the feature gets a CON save to pin HP
                // at 1 instead of falling. The helper rolls the save,
                // logs the outcome, and reverts the Dying transition
                // on a pass. Skipped silently for non-barbarians.
                if !ei.try_relentless_rage(self.actor_id) {
                    ei.log(format!("{} falls unconscious.", name));
                    ei.drop_concentration(self.actor_id);
                    // Shared kill-triggered temp HP cohort: reducing a
                    // hostile to 0 HP grants the swinger temp HP if
                    // they hold any row's tag (Fiend Warlock **Dark
                    // One's Blessing** — CHA mod + level; Long Death
                    // Monk **Touch of Death** — 1 + CON mod + level).
                    // Fires after Relentless Rage since a raging
                    // barbarian who pins at 1 HP wasn't actually
                    // reduced to 0 per RAW.
                    ei.trigger_kill_triggered_temp_hp(self.actor_id);
                }
            }
            DamageOutcome::Killed => {
                // cleanup_dead_actors logs "X dies." when it removes
                // the actor; we just drop concentration here.
                ei.drop_concentration(self.actor_id);
                // Shared kill-triggered temp HP cohort — same trigger
                // as the Downed branch: the outright kill path
                // (monster HP → 0) is functionally a "reduced to 0
                // HP" event per RAW. Attributed to whoever's turn is
                // currently active via `current_turn_actor_id`.
                ei.trigger_kill_triggered_temp_hp(self.actor_id);
            }
            DamageOutcome::Reduced if was_concentrating && landed > 0 => {
                // 5e: take damage while concentrating → CON save vs
                // DC max(10, dmg/2). Use the post-mitigation amount so a
                // resisted hit makes a smaller DC. The save site is
                // `roll_concentration_save` so it can layer the Warlock
                // Eldritch Mind invocation's advantage on top of the
                // actor's normal save mode.
                let dc = ((landed / 2) as i32).max(10);
                let save = ei.roll_concentration_save(self.actor_id, dc);
                if !save.passed() {
                    ei.drop_concentration(self.actor_id);
                }
            }
            DamageOutcome::Reduced | DamageOutcome::DyingFailure => {}
        }
        // 5e Warding Bond reflect: mirror the post-resistance damage onto
        // the bonding partner. Skip when the partner is the actor itself
        // (a self-bond is a no-op), the partner is missing, the partner
        // is also bonded back to us (rare cross-bond — bail to avoid
        // infinite recursion), or `scaled` is zero (immunity / nothing
        // landed). The partner takes the damage through `DealDamage`
        // again so their own resistance / concentration save / death
        // pipeline applies uniformly. The partner's hit shouldn't
        // re-mirror back through our actor — guarded by the cross-bond
        // check above.
        if let Some(partner_id) = warding_partner
            && partner_id != self.actor_id
            && scaled > 0
        {
            let partner_loops = ei.actors.get(&partner_id).is_some_and(|p| {
                p.has_condition(crate::conditions::Condition::WardingBonded)
                    && p.warding_partner() == Some(self.actor_id)
            });
            if !partner_loops {
                let partner_name = ei
                    .actors
                    .get(&partner_id)
                    .map(|p| p.name().to_string())
                    .unwrap_or_default();
                if !partner_name.is_empty() {
                    ei.log(format!(
                        "  warding bond mirrors {} {:?} damage onto {}",
                        scaled, self.damage_type, partner_name
                    ));
                }
                DealDamage {
                    actor_id: partner_id,
                    amount: scaled,
                    damage_type: self.damage_type,
                }
                .apply(ei);
            }
        }
    }

    /// 5e Tasha's Sorcerer Transmuted Spell metamagic hook. If the current
    /// `damage_type` is one of the six elemental types (acid, cold, fire,
    /// lightning, poison, thunder), swap to `new_type` in place and return
    /// true. Non-elemental damage (force / radiant / necrotic / psychic /
    /// physical) is untouched and returns false so the prime stays up for
    /// the next eligible cast.
    fn remap_damage_type(&mut self, new_type: DamageType) -> bool {
        if !is_transmutable_element(self.damage_type) {
            return false;
        }
        self.damage_type = new_type;
        true
    }

    fn elemental_damage_target(&self) -> Option<(usize, DamageType)> {
        if !is_transmutable_element(self.damage_type) {
            return None;
        }
        Some((self.actor_id, self.damage_type))
    }
}

/// 5e Tasha's Transmuted Spell metamagic — the six damage types eligible
/// for both the "source" side (must already be one of these for the prime
/// to engage) and the "target" side (the remap can pick any of these as
/// the new type). RAW: acid, cold, fire, lightning, poison, thunder.
pub const TRANSMUTABLE_DAMAGE_TYPES: [DamageType; 6] = [
    DamageType::Acid,
    DamageType::Cold,
    DamageType::Fire,
    DamageType::Lightning,
    DamageType::Poison,
    DamageType::Thunder,
];

/// True if `dt` is one of the six damage types Transmuted Spell can remap
/// between. Centralizes the cohort so any future "elemental damage type?"
/// gate reads from the same source.
pub fn is_transmutable_element(dt: DamageType) -> bool {
    TRANSMUTABLE_DAMAGE_TYPES.contains(&dt)
}

/// Walk `side_effects` and call `remap_damage_type(new_type)` on each
/// entry. Returns true if at least one entry was remapped (i.e. the cast
/// carried an eligible elemental damage source). Mirrors
/// `extend_side_effect_timers` so the Twinned + Transmuted re-issue path
/// can propagate the same remap to the twin's separately-built
/// side_effects vec.
pub fn remap_side_effect_damage_types(
    side_effects: &mut [Box<dyn ApplicableSideEffect>],
    new_type: DamageType,
) -> bool {
    let mut remapped = false;
    for se in side_effects.iter_mut() {
        if se.remap_damage_type(new_type) {
            remapped = true;
        }
    }
    remapped
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
    fn concentration_payload(&self) -> Option<ConcentrationPayload<'_>> {
        Some(ConcentrationPayload {
            caster_id: self.caster_id,
            conditions: &self.data.conditions,
            attack_buffs: &self.data.attack_buffs,
            save_buffs: &self.data.save_buffs,
            damage_buffs: &self.data.damage_buffs,
        })
    }

    fn absorb_concentration_payload(&mut self, extra: &ConcentrationPayload<'_>) -> bool {
        if extra.caster_id != self.caster_id {
            return false;
        }
        // Union rather than append: the doubled cast re-runs the same
        // builder, so a payload entry aimed at the caster themselves
        // (a self-buff rider) comes back identical and must not be
        // rolled back twice on drop.
        for entry in extra.conditions {
            if !self.data.conditions.contains(entry) {
                self.data.conditions.push(*entry);
            }
        }
        for entry in extra.attack_buffs {
            if !self.data.attack_buffs.contains(entry) {
                self.data.attack_buffs.push(*entry);
            }
        }
        for entry in extra.save_buffs {
            if !self.data.save_buffs.contains(entry) {
                self.data.save_buffs.push(*entry);
            }
        }
        for entry in extra.damage_buffs {
            if !self.data.damage_buffs.contains(entry) {
                self.data.damage_buffs.push(*entry);
            }
        }
        true
    }

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

/// Entry in the paladin-aura suppression table read by
/// `ApplyCondition::apply`. Pairs a condition install with an
/// encounter-aware aura predicate — if the predicate returns true for
/// the install's target, the install bounces and `log_verb` fires
/// instead of the standard "is now X" line. Adding a new aura-suppressed
/// condition (e.g. a future Aura of Freedom → Paralyzed) is a one-line
/// tuple entry.
struct AuraConditionSuppressor {
    condition: Condition,
    aura_check: fn(&EncounterInstance, usize) -> bool,
    log_verb: &'static str,
}

/// Central table of paladin auras that suppress conditions on allies
/// inside them. Read by `ApplyCondition::apply`. Ordering doesn't matter
/// — the first entry whose condition matches AND whose aura fires wins,
/// but at most one aura can suppress a given install. RAW-shape auras
/// all bounce the same way (silent no-op + a log line), so a table-
/// driven pass is exactly the same shape as the old inlined `if / if`
/// chain but scales linearly with new auras.
const APPLY_CONDITION_AURA_SUPPRESSORS: &[AuraConditionSuppressor] = &[
    // Aura of Courage (Paladin lv10+): allies inside the 10ft aura are
    // immune to Frightened.
    AuraConditionSuppressor {
        condition: Condition::Frightened,
        aura_check: EncounterInstance::is_in_aura_of_courage,
        log_verb: "resists fear (aura of courage)",
    },
    // Aura of Devotion (Devotion Paladin lv7+): allies inside the 10ft
    // aura are immune to Charmed.
    AuraConditionSuppressor {
        condition: Condition::Charmed,
        aura_check: EncounterInstance::is_in_aura_of_devotion,
        log_verb: "resists charm (aura of devotion)",
    },
];

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
    fn extend_duration(&mut self) -> bool {
        match self.timer {
            ConditionTimer::Rounds(n) if n >= EXTENDED_SPELL_MIN_ROUNDS => {
                self.timer = ConditionTimer::Rounds(
                    n.saturating_mul(2).min(EXTENDED_SPELL_MAX_ROUNDS),
                );
                true
            }
            _ => false,
        }
    }

    fn apply(&self, ei: &mut EncounterInstance) {
        // 5e Paladin aura suppression table — the encounter-aware immunity
        // lane. Each entry pairs a condition install with an aura
        // predicate: if the install's target is inside the aura, the
        // install bounces and a log line fires instead. Lives here rather
        // than in `ActorInstance::add_condition` because the aura needs
        // encounter geometry (the location of every aura-bearer). Adding
        // a new aura-suppressed condition (a future Charm of the Fae Aura
        // pushing Beguiled, etc.) lands as a one-line tuple entry —
        // mirrors how `TYPED_IMMUNITY_CONDITIONS` handles the
        // actor-local damage-type immunity lane.
        for entry in APPLY_CONDITION_AURA_SUPPRESSORS {
            if self.condition == entry.condition
                && (entry.aura_check)(ei, self.actor_id)
            {
                if let Some(actor) = ei.get_actor(self.actor_id) {
                    let name = actor.name().to_string();
                    ei.log(format!("{} {}.", name, entry.log_verb));
                }
                return;
            }
        }
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

/// Same shape as `AdjustAttackBuff` but for the damage-roll buff lane.
/// Used by Magic Weapon / Elemental Weapon-style spells that install a
/// flat `+N damage` buff alongside their attack buff. Pair with
/// concentration registration so the buff drops cleanly when the spell
/// ends; negative deltas remove the buff on cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AdjustDamageBuff {
    pub actor_id: usize,
    pub delta: i32,
}

impl ApplicableSideEffect for AdjustDamageBuff {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.actor_id) {
            actor.add_damage_bonus_buff(self.delta);
        }
    }
}

/// Forced-movement direction relative to an anchor point. `Toward` pulls
/// the actor closer (Thorn Whip, Telekinesis pull); `Away` pushes them
/// outward (Thunderwave, Repelling Blast). Both stop early when the actor
/// can't legally advance further (wall, occupied tile, or — for Toward —
/// reaches the anchor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForcedMoveDirection {
    Toward,
    Away,
}

/// Shared forced-movement step loop used by `PullActor` and `PushActor`.
/// Walks the actor one tile per iteration along the line between their
/// footprint and `anchor`, in the direction dictated by `dir`. 5e treats
/// forced movement as not a willing move, so opportunity attacks don't
/// fire here. Returns silently if the actor never moved.
fn forced_move(
    ei: &mut EncounterInstance,
    actor_id: usize,
    anchor: Coordinate,
    max_tiles: u32,
    dir: ForcedMoveDirection,
    verb: &str,
) {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let Some(actor) = ei.actors.get(&actor_id) else {
        return;
    };
    let name = actor.name().to_string();
    let my_size = get_tiles_from_size(actor.size());
    let start = actor.location();
    let mut from = start;
    let mut last_good = from;
    let mut remaining = max_tiles;
    while remaining > 0 {
        // For `Toward`, stop once the actor's footprint touches the
        // anchor tile. For `Away`, no such stop — we keep walking outward
        // until we run out of budget or hit an obstacle.
        let (dx, dy) = match dir {
            ForcedMoveDirection::Toward => {
                if footprint_chebyshev(from, my_size, anchor, 1) == 0 {
                    break;
                }
                (anchor.x - from.x, anchor.y - from.y)
            }
            ForcedMoveDirection::Away => {
                // Standing exactly on the anchor — no outward direction
                // to take; bail rather than pick an arbitrary axis.
                if from == anchor {
                    break;
                }
                (from.x - anchor.x, from.y - anchor.y)
            }
        };
        let next = Coordinate::new(from.x + dx.signum(), from.y + dy.signum());
        if next == from {
            break;
        }
        if !ei.can_move_to(actor_id, next) {
            break;
        }
        from = next;
        last_good = next;
        remaining -= 1;
    }
    if last_good == start {
        return;
    }
    let dest = last_good;
    if let Err(e) = ei.place_actor_at(actor_id, dest) {
        ei.log(format!("forced move ({}) failed: {}", verb, e));
        return;
    }
    ei.log(format!("{} is {} to {}.", name, verb, dest));
    ei.pickup_items_at(actor_id, dest);
}

/// Forced movement toward a fixed point, up to `max_tiles` steps, without
/// firing opportunity attacks (5e treats forced movement as not a willing
/// move). The actor stops as soon as it can't legally advance further —
/// blocked by a wall, another actor's footprint, or hitting the target.
/// Used by Thorn Whip's pull, Telekinesis, Lightning Lure's catch.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct PullActor {
    pub actor_id: usize,
    pub toward: Coordinate,
    pub max_tiles: u32,
}

impl ApplicableSideEffect for PullActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        forced_move(
            ei,
            self.actor_id,
            self.toward,
            self.max_tiles,
            ForcedMoveDirection::Toward,
            "pulled",
        );
    }
}

/// Forced movement *away from* a fixed point — the symmetric counterpart
/// to `PullActor`. Same no-opportunity-attack semantics; stops when the
/// actor can't legally advance further (wall, occupied tile). Used by
/// Thunderwave's push and any future shove / repelling effects.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct PushActor {
    pub actor_id: usize,
    /// The anchor the actor is pushed *away from*. Typically the caster's
    /// location or the burst center.
    pub from: Coordinate,
    pub max_tiles: u32,
}

impl ApplicableSideEffect for PushActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        forced_move(
            ei,
            self.actor_id,
            self.from,
            self.max_tiles,
            ForcedMoveDirection::Away,
            "pushed",
        );
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

/// Record which fighter has tagged the target with Goading Attack.
/// Pairs with ApplyCondition (Goaded): `compute_attack_mode` reads this
/// to apply disadvantage on attacks against anyone *other* than the
/// goader. `set_goaded_by(None)` clears the link explicitly; the
/// engine also clears it automatically when the Goaded condition is
/// removed via `remove_condition`. Mirrors `SetDueledBy` — same shape,
/// distinct field on `ActorInstance`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetGoadedBy {
    pub target_id: usize,
    pub goader: Option<usize>,
}

impl ApplicableSideEffect for SetGoadedBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_goaded_by(self.goader);
        }
    }
}

/// Record which fighter has tagged the target with Distracting Strike.
/// Pairs with ApplyCondition (Distracted): `compute_attack_mode` reads
/// this to grant advantage on attack rolls against the target by any
/// attacker *other* than the distractor. `set_distracted_by(None)`
/// clears the link explicitly; the engine also clears it automatically
/// when the Distracted condition is removed via `remove_condition`.
/// Mirrors `SetGoadedBy` in shape, distinct field on `ActorInstance`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetDistractedBy {
    pub target_id: usize,
    pub distracter: Option<usize>,
}

impl ApplicableSideEffect for SetDistractedBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_distracted_by(self.distracter);
        }
    }
}

/// Record which paladin has sworn Vow of Enmity against the target (5e
/// Vengeance Paladin Channel Divinity, lv3 subclass). Pairs with
/// ApplyCondition (Sworn): `compute_attack_mode` reads this to grant
/// advantage on the swearing paladin's attack rolls against this target
/// (positive-polarity sibling of `SetDistractedBy` — only the swearer
/// gets the buff, not other allies). `set_sworn_by(None)` clears the
/// link explicitly; the engine also clears it automatically when the
/// Sworn condition is removed via `remove_condition`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetSwornBy {
    pub target_id: usize,
    pub swearer: Option<usize>,
}

impl ApplicableSideEffect for SetSwornBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_sworn_by(self.swearer);
        }
    }
}

/// Record which Eldritch Knight's weapon hit marked the target (5e
/// Eldritch Knight Fighter **Eldritch Strike**, subclass lv10). Pairs
/// with ApplyCondition (EldritchStruck): the `CASTER_SAVE_MODE_RIDERS`
/// cohort in `roll_save_against_caster` reads this to bend the target's
/// next save against *this* knight's spell to disadvantage. Same
/// positive-polarity flag-plus-link shape as `SetSwornBy` — only the
/// marker benefits — but on the save-roll axis rather than the
/// attack-roll one. `set_eldritch_struck_by(None)` clears the link
/// explicitly; the engine also clears it automatically when the
/// EldritchStruck condition is removed via `remove_condition`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetEldritchStruckBy {
    pub target_id: usize,
    pub striker: Option<usize>,
}

impl ApplicableSideEffect for SetEldritchStruckBy {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_eldritch_struck_by(self.striker);
        }
    }
}

/// Single source of truth for "which conditions carry a back-link to
/// the actor that applied them, and what `Set*By` side-effect installs
/// that link." Returns `Some(boxed_side_effect)` for the five flag-plus-
/// link conditions (Dueled / Goaded / Distracted / Sworn /
/// EldritchStruck); returns `None` for conditions that stand alone with
/// just `ApplyCondition`.
///
/// Used by:
///   - `engine::attack::attacker_link_side_effect` (weapon on-hit rider
///     chain for Goaded / Distracted),
///   - `engine::attack::push_on_hit_condition_marks` (the passive
///     weapon-hit mark cohort — Eldritch Strike installs
///     EldritchStruck, Unwavering Mark installs Dueled),
///   - direct-cast actions that install a flag-plus-link condition
///     without going through the rider chain (Compelled Duel installs
///     Dueled; Vow of Enmity installs Sworn).
///
/// Adding a future linked condition lands as one match arm here —
/// both the rider chain AND the direct-cast action pipeline pick up
/// the new link install for free, with no duplicate dispatch tables.
pub fn condition_link_side_effect(
    condition: crate::conditions::Condition,
    target_id: usize,
    caster_id: usize,
) -> Option<Box<dyn ApplicableSideEffect>> {
    use crate::conditions::Condition;
    match condition {
        Condition::Dueled => Some(Box::new(SetDueledBy {
            target_id,
            duelist: Some(caster_id),
        })),
        Condition::Goaded => Some(Box::new(SetGoadedBy {
            target_id,
            goader: Some(caster_id),
        })),
        Condition::Distracted => Some(Box::new(SetDistractedBy {
            target_id,
            distracter: Some(caster_id),
        })),
        Condition::Sworn => Some(Box::new(SetSwornBy {
            target_id,
            swearer: Some(caster_id),
        })),
        Condition::EldritchStruck => Some(Box::new(SetEldritchStruckBy {
            target_id,
            striker: Some(caster_id),
        })),
        _ => None,
    }
}

/// Record the partner of a Warding Bond (5e level-2 abjuration). Paired
/// with ApplyCondition (WardingBonded) on the same target: the condition
/// flag carries the AC / save / resistance buff, while the `warding_partner`
/// link tells the damage-reflect site which actor to mirror the hit onto.
/// Pass `partner = None` to clear; the engine also clears it automatically
/// when the WardingBonded condition is removed via `remove_condition`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct SetWardingPartner {
    pub target_id: usize,
    pub partner: Option<usize>,
}

impl ApplicableSideEffect for SetWardingPartner {
    fn apply(&self, ei: &mut EncounterInstance) {
        if let Some(actor) = ei.get_actor(self.target_id) {
            actor.set_warding_partner(self.partner);
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

/// End one spell affecting `target_id` (5e Cleansing Touch). Walks three
/// fallbacks so the action is useful regardless of what's loaded on the
/// target:
///   1. Drop the target's concentration (ends a spell *they* are sustaining
///      — useful when the paladin themselves is concentrating on something
///      they want to swap out, or when the target is an enemy concentrator
///      that the paladin can touch).
///   2. Strip one of the canonical "spell-installed debuff" conditions
///      (Paralyzed / Stunned / Charmed / Frightened / Blinded / Poisoned
///      / Restrained / Hexed / Hunter's Marked / Confused / Mocked /
///      Baned / Slowed). This is the load-bearing use case — the paladin
///      cleanses a debuffed ally without burning a level-5 Greater
///      Restoration slot.
///   3. Fallback to the Dispel Magic "strip one beneficial buff" lane so
///      the action is never wasted on a clean concentration-free target.
///
/// Caller is responsible for spending the once-per-rest feature charge —
/// the side effect itself is pure dispel logic. Logged on success;
/// silent no-op when the target has nothing eligible to end.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct CleansingTouchOn {
    pub target_id: usize,
}

/// Spell-installed debuffs Cleansing Touch will lift (step 2 of the
/// fallback chain). Order matters — the first match pops, so the heavyweight
/// lockdown conditions (Paralyzed, Stunned) sit at the top so the cleanse
/// targets the most-impactful debuff first. Mirrors `GreaterRestoration::
/// CANDIDATES` in spirit but with a broader spell-source list since RAW
/// Cleansing Touch ends *any* spell, not just the curated Greater
/// Restoration set. Public so the action's validate gate can pre-flight
/// "is there anything to cleanse?" without copying the list.
pub const CLEANSING_TOUCH_DEBUFFS: &[Condition] = &[
    Condition::Paralyzed,
    Condition::Stunned,
    Condition::Petrified,
    Condition::Charmed,
    Condition::Dominated,
    Condition::Frightened,
    Condition::Confused,
    Condition::Restrained,
    Condition::Blinded,
    Condition::Poisoned,
    Condition::Deafened,
    Condition::Asleep,
    Condition::Hexed,
    Condition::HuntersMarked,
    Condition::Mocked,
    Condition::Baned,
    Condition::Slowed,
    // Power Word: Pain (XGtE lv7 necromancy). Cleansing Touch's "end
    // one spell effect on you or a creature you touch" clause covers
    // the pain rider cleanly — a paladin adjacent to a pained ally
    // (or an enemy caster reachable by Cleansing Touch) can lift the
    // condition without waiting on the target's per-turn CON save
    // via `ROUND_END_SAVES`. Sibling to `Slowed` above (Slow spell
    // debuff, ×½ speed compound) on the "spell-installed movement
    // debuff" corner — Cleansing Touch treats both as strippable.
    Condition::PowerWordPained,
    Condition::Outlined,
    Condition::Burning,
];

impl ApplicableSideEffect for CleansingTouchOn {
    fn apply(&self, ei: &mut EncounterInstance) {
        // Step 1: drop the target's own concentration if any. Highest-
        // leverage outcome — ends both the spell *and* every condition
        // it installed across the encounter cleanly through the existing
        // concentration-drop pipeline.
        if ei
            .actors
            .get(&self.target_id)
            .is_some_and(|a| a.is_concentrating())
        {
            ei.drop_concentration(self.target_id);
            return;
        }
        // Step 2: strip one canonical spell-installed debuff.
        let Some(actor) = ei.get_actor(self.target_id) else {
            return;
        };
        let name = actor.name().to_string();
        for &c in CLEANSING_TOUCH_DEBUFFS {
            if actor.remove_condition(c) {
                ei.log(format!(
                    "{} is no longer {} (cleansing touch).",
                    name,
                    c.name()
                ));
                return;
            }
        }
        // Step 3: fallback to Dispel Magic semantics — strip one
        // beneficial buff. Deterministic order via name sort, same
        // selection rule as `DispelMagicOn`.
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
        if actor.remove_condition(c) {
            ei.log(format!(
                "{} is no longer {} (cleansing touch).",
                name,
                c.name()
            ));
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
