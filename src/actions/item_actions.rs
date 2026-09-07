use std::collections::HashSet;

use crate::{
    actions::action_template::{
        Action, TargetingSchema, action_only, bonus_action_only, first_ally_target_id,
        first_target_id, first_target_location,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        saves::SaveDamagePolicy,
        side_effects::{ApplicableSideEffect, ApplyCondition, DealDamage, Heal, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
    },
};

/// Action-economy price for reaching into your own pack, with the Thief
/// Rogue's **Fast Hands** folded in.
///
/// `always_bonus_action` is the item's own declared price — a handful of
/// items (the Pearls of Power) are bonus actions for everybody, and pass
/// `true`. Everything else is an Action by default and becomes a bonus
/// action in one hand only: a Thief's.
///
/// The Fast Hands half is resolved *dynamically*, against the holder's
/// remaining bonus action, and that is deliberate. RAW's offer is "you
/// may spend your Cunning Action bonus action on this instead" — an
/// alternative, not a replacement. The engine's `cost()` returns a list
/// of resources that must all be paid and has no way to spell "either
/// of these," so a static swap to `bonus_action_only` would *take away*
/// the Action price and leave a Thief who had already dashed unable to
/// drink at all. Pricing against what the holder actually has left picks
/// the same branch RAW's player would: the bonus action while it is
/// there, the Action once it isn't.
///
/// **Scope, and where it parts company with RAW.** This covers the
/// support-consumable lane — potions, pearls, worn trinkets, and the
/// healing / buffing scrolls that share their config structs — because
/// that lane is the one whose `cost()` routes here. The offensive item
/// lane (Scroll of Fireball, Wand of Lightning Bolts, and the rest of
/// the `action_only()` cohort) is untouched, so Fast Hands can never
/// turn a bonus action into a Fireball.
///
/// RAW would draw the line one notch differently: reading *any* scroll
/// is casting a spell, not using an object, so a Scroll of Cure Wounds
/// should be excluded here and isn't. Following RAW exactly would mean
/// teaching the engine which items are objects and which are spells in a
/// tube — and the item model has no such axis. `SelfConditionItem` is
/// the literal same struct behind `DRINK_POTION_OF_BLUR` and
/// `READ_BLINK_SCROLL`; separating them would mean a second taxonomy of
/// "what an item is," competing with the one the cost lane already
/// carries, invented for the benefit of a single feature. The lane split
/// the engine does have — support versus offense — is the one that
/// matters for balance, and it is the one enforced.
pub fn item_use_cost(
    encounter: &EncounterInstance,
    caster_id: usize,
    always_bonus_action: bool,
) -> Vec<Resource> {
    if always_bonus_action || encounter.handles_items_as_a_bonus_action(caster_id) {
        bonus_action_only()
    } else {
        action_only()
    }
}

/// Config struct for "burst damage with a save for half" consumable
/// items — the shared shape behind Scroll of Fireball / Cone of Cold /
/// Lightning Bolt and the Wand of Fireballs / Lightning Bolts. Each
/// static instance encodes a single item's per-cast configuration; the
/// `Action` impl below routes through `resolve_burst_save_damage` so
/// evasion / Careful Spell / Heightened Spell shielding all flow
/// through the same chokepoint as the spell-side equivalents AND the
/// caster is excluded from their own burst.
///
/// Adding a new burst-save scroll / wand is a one-static declaration —
/// no new `Action` impl needed. Drops the ~75 lines per item the
/// previous one-struct-per-scroll approach required.
pub struct BurstSaveDamageItem {
    /// Player-facing action name (e.g. "read fireball scroll"). Returned
    /// from `Action::name`.
    pub action_name: &'static str,
    /// Picker aliases for the action (e.g. ["fireball", "scroll"]).
    /// Returned from `Action::aliases`.
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on (e.g.
    /// "Scroll of Fireball"). Must match the item's `name` field.
    pub item_name: &'static str,
    /// Log line prefix (e.g. "scroll of fireball"). The log row reads
    /// `  {log_label}: {count}d{faces} = {n} damage`.
    pub log_label: &'static str,
    /// Damage dice (e.g. 6d6, 8d6, 8d8). Rolled once and shared across
    /// every target in the burst — matches the spell-side AoE pattern.
    pub dice: Dice,
    /// Damage type (e.g. Fire, Cold, Lightning). Folded into the
    /// per-target `DealDamage` side-effect emitted by
    /// `resolve_burst_save_damage`.
    pub damage_type: DamageType,
    /// Save ability for the burst (e.g. DEX for Fireball / Lightning
    /// Bolt; CON for Cone of Cold).
    pub save: AbilityScoreType,
    /// Save DC (typically 15 for SRD scrolls / wands).
    pub dc: i32,
    /// Burst radius in tiles (e.g. 4 for Fireball, 2 for Lightning Bolt,
    /// 6 for Cone of Cold).
    pub radius: isize,
    /// Maximum reach in tiles for the targeting picker (e.g. 60 for
    /// Fireball's 150 ft, 40 for Lightning Bolt's 100 ft).
    pub reach: isize,
}

impl Action for BurstSaveDamageItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst {
            radius: self.radius,
        }
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::action_template::resolve_burst_save_damage;

        let Some(center) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }

        let damage = encounter.roll(&self.dice);
        encounter.log(format!(
            "  {}: {}d{} = {} damage",
            self.log_label, self.dice.count, self.dice.faces, damage
        ));

        resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            self.radius,
            self.save,
            self.dc,
            damage,
            self.damage_type,
        )
    }
}

/// Config struct for "self-targeted healing potion" consumables — the
/// shared shape behind Potion of Healing / Potion of Greater Healing
/// (and any future single-target heal potion). Each static instance
/// encodes the dice / flat bonus / cost; the `Action` impl below pops
/// the item from inventory and emits a `Heal` side-effect.
///
/// Adding a new heal potion is a one-static declaration — no new
/// `Action` impl needed.
pub struct SelfHealItem {
    /// Player-facing action name (e.g. "drink healing potion").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["potion", "drink"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Log line prefix (e.g. "potion of healing"). The row reads
    /// `  {log_label}: {count}d{faces}({raw})+{flat_bonus} = {amount} HP`.
    pub log_label: &'static str,
    /// Healing dice (e.g. 2d4, 4d4).
    pub dice: Dice,
    /// Flat bonus added to the rolled dice (e.g. +2 for Healing,
    /// +4 for Greater Healing).
    pub flat_bonus: i32,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost. Greater
    /// Healing is a bonus action (the wounded martial can drink AND
    /// swing in one turn); Healing is a full Action.
    pub bonus_action: bool,
}

impl Action for SelfHealItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
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

    fn is_heal(&self) -> bool {
        true
    }

    fn cost(
        &self,
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&self.dice) as i32;
        let amount = (raw + self.flat_bonus).max(1) as u32;
        // Pop the potion *now* — validate confirmed it was carried; the
        // consume-first ordering keeps inventory consistent even if the
        // heal fails (e.g. caster died mid-stack).
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        encounter.log(format!(
            "  {}: {}d{}({}){:+} = {} HP",
            self.log_label, self.dice.count, self.dice.faces, raw, self.flat_bonus, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

/// Config struct for "self-installs a single condition" consumable items
/// — the shared shape behind Potion of Invisibility / Potion of Flying /
/// Potion of Climbing / Boots of Speed. Each static instance encodes the
/// target condition and timer; the `Action` impl below consumes the item
/// and queues an `ApplyCondition` side-effect.
///
/// Adding a new self-condition consumable is a one-static declaration —
/// no new `Action` impl needed.
pub struct SelfConditionItem {
    /// Player-facing action name (e.g. "drink potion of flying").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["fly", "flying"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Full log line emitted on use (e.g. "Fighter drinks a potion of
    /// flying."). The `{actor}` placeholder is substituted with the
    /// caster's name; no other formatting is performed.
    pub log_text: &'static str,
    /// Condition to install on the holder.
    pub condition: Condition,
    /// Timer for the install (typically `Rounds(10)` for combat-scale
    /// potion buffs).
    pub timer: ConditionTimer,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost.
    pub bonus_action: bool,
    /// If `true`, the validator rejects when the condition is already
    /// up — prevents the consumable from being wasted on a no-op timer
    /// refresh. Use `false` for installs where the player explicitly
    /// might want to refresh (rare).
    pub reject_when_active: bool,
    /// Optional flat temp HP grant the item installs alongside the
    /// condition. 5e RAW: Potion of Heroism grants 10 temp HP + immunity
    /// to Frightened; Aid-style buffs grant a HP cushion. `None` (the
    /// default for most condition-only potions) emits no `GainTempHp`
    /// side-effect. Temp HP doesn't stack — the bigger of the existing
    /// pool and the new grant wins (see `GainTempHp`).
    pub temp_hp: Option<u32>,
}

impl Action for SelfConditionItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
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
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, self.item_name) {
            return false;
        }
        if self.reject_when_active
            && encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.has_condition(self.condition))
        {
            return false;
        }
        true
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::GainTempHp;
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: self.condition,
            timer: self.timer,
        })];
        // Optional temp HP grant — Potion of Heroism (10) and any future
        // Aid-flavored self-buff fold into the same one-static declaration.
        // `None` (the common case) skips the alloc cleanly; `GainTempHp`
        // also no-ops on 0 internally as a defense in depth.
        if let Some(amount) = self.temp_hp {
            effects.push(Box::new(GainTempHp {
                actor_id: caster_id,
                amount,
            }));
        }
        effects
    }

    fn is_heal(&self) -> bool {
        // A condition-only buff doesn't route through the AI's heal-target
        // pipeline; a temp-HP grant does (the cushion reads as healing the
        // weakest ally for engine purposes). Mirrors `SingleTargetBuffItem`'s
        // `is_heal = true` for the ally-buff lane on the temp-HP branch.
        self.temp_hp.is_some()
    }
}

const POTION_OF_HEALING_NAME: &str = "Potion of Healing";
const POTION_OF_GREATER_HEALING_NAME: &str = "Potion of Greater Healing";
const ANTITOXIN_NAME: &str = "Antitoxin";
const SCROLL_OF_FIREBALL_NAME: &str = "Scroll of Fireball";
const SCROLL_OF_MAGIC_MISSILE_NAME: &str = "Scroll of Magic Missile";
const POTION_OF_SPEED_NAME: &str = "Potion of Speed";
const POTION_OF_HEROISM_NAME: &str = "Potion of Heroism";
const POTION_OF_INVISIBILITY_NAME: &str = "Potion of Invisibility";
const SCROLL_OF_CURE_WOUNDS_NAME: &str = "Scroll of Cure Wounds";

/// Shared validate-hook body for consumable item actions: true iff the
/// caster is still carrying at least one copy of `item_name`. Centralizes
/// the `encounter.actors.get(&caster_id).is_some_and(|a| a.has_item_named(...))`
/// chain so every item action's `custom_validate_input` collapses to a
/// one-liner. Returns false when the caster vanished between enqueue and
/// validate (e.g. died to a reaction) — same fail-safe shape as the
/// previous inline copies.
fn caster_holds(
    encounter: &EncounterInstance,
    caster_id: usize,
    item_name: &str,
) -> bool {
    encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.has_item_named(item_name))
}

/// Pop one copy of `item_name` from the caster's inventory and return
/// true on success. Used at the head of every consumable's
/// `side_effects` to consume the item before the spell-style effect
/// rolls fire. Returns false (and the caller short-circuits with
/// `Vec::new()`) when the caster vanished or the item was already
/// consumed elsewhere — guards against a duplicate queued use slipping
/// past the validator.
fn consume_caster_item(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    item_name: &str,
) -> bool {
    encounter
        .actors
        .get_mut(&caster_id)
        .is_some_and(|a| a.remove_item_by_name(item_name))
}

/// Potion of Healing — 2d4+2 self-heal, Action. Fires through the shared
/// `SelfHealItem` impl: validate confirms the potion is in inventory,
/// side-effect rolls dice, pops the potion, and emits a `Heal`.
pub static DRINK_HEALING_POTION: SelfHealItem = SelfHealItem {
    action_name: "drink healing potion",
    action_aliases: &["potion", "drink"],
    item_name: POTION_OF_HEALING_NAME,
    log_label: "potion of healing",
    dice: Dice::new(2, 4),
    flat_bonus: 2,
    bonus_action: false,
};

/// Potion of Greater Healing — 4d4+4 self-heal, Bonus Action. Same
/// shape as the regular healing potion but a bigger pool and cheaper
/// action-economy cost (a wounded martial can drink AND swing on the
/// same turn).
pub static DRINK_GREATER_HEALING_POTION: SelfHealItem = SelfHealItem {
    action_name: "drink greater healing potion",
    action_aliases: &["potion+", "drink+"],
    item_name: POTION_OF_GREATER_HEALING_NAME,
    log_label: "potion of greater healing",
    dice: Dice::new(4, 4),
    flat_bonus: 4,
    bonus_action: true,
};

/// Scroll of Fireball: 6d6 fire DEX-save burst centered on a target tile.
/// Fires through the shared `BurstSaveDamageItem` impl — see that struct
/// for the routing through `resolve_burst_save_damage`.
pub static READ_FIREBALL_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read fireball scroll",
    action_aliases: &["fireball", "scroll"],
    item_name: SCROLL_OF_FIREBALL_NAME,
    log_label: "scroll of fireball",
    dice: Dice::new(6, 6),
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    // 150 ft range — well past any current map.
    reach: 60,
};

/// Config struct for "Magic Missile auto-hit dart volley" consumables —
/// the shared shape behind Scroll of Magic Missile and Wand of Magic
/// Missiles. Each static instance encodes the dart count; the `Action`
/// impl below rolls 1d4+1 force damage per dart against the single
/// target. Auto-hit, no save, no attack roll — pure reliability.
///
/// Adding a new variant (e.g. Staff of Magic Missiles) is a one-static
/// declaration — no new `Action` impl needed. Drops the ~70-line
/// duplicate per-item that the per-struct approach required.
pub struct MagicMissileItem {
    /// Player-facing action name (e.g. "read magic missile scroll").
    pub action_name: &'static str,
    /// Picker aliases.
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Log line prefix (e.g. "scroll of magic missile"). The row reads
    /// `  {log_label}: N*(1d4+1) [r1, r2, ...] = {total} force`.
    pub log_label: &'static str,
    /// Number of darts to fire (3 for the scroll, 5 for the wand).
    pub darts: u32,
    /// Maximum reach in tiles for the targeting picker (30 = 150 ft RAW).
    pub reach: isize,
}

impl Action for MagicMissileItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let mut total = 0u32;
        let mut roll_strs: Vec<String> = Vec::with_capacity(self.darts as usize);
        for _ in 0..self.darts {
            let r = encounter.roll(&Dice::new(1, 4));
            roll_strs.push((r + 1).to_string());
            total = total.saturating_add(r + 1);
        }
        encounter.log(format!(
            "  {}: {}*(1d4+1) [{}] = {} force",
            self.log_label,
            self.darts,
            roll_strs.join(", "),
            total
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total,
            damage_type: DamageType::Force,
        })]
    }
}

/// Scroll of Magic Missile — 3 darts of 1d4+1 force each, auto-hit, no
/// save. Fires through the shared `MagicMissileItem` impl.
pub static READ_MAGIC_MISSILE_SCROLL: MagicMissileItem = MagicMissileItem {
    action_name: "read magic missile scroll",
    action_aliases: &["mm scroll", "missile scroll"],
    item_name: SCROLL_OF_MAGIC_MISSILE_NAME,
    log_label: "scroll of magic missile",
    darts: 3,
    reach: 30,
};

/// Config struct for "single-target save-or-take-damage" consumables —
/// the shared shape behind Scroll of Disintegrate (10d6+40 force, no
/// save-half) and Scroll of Finger of Death (7d8+30 necrotic, save
/// halves). Mirrors `BurstSaveDamageItem` for the single-target variant.
///
/// The save is rolled through the caster-aware path so the Sorcerer
/// Heightened Spell prime (forces the target's first save to
/// disadvantage) flows in if the item is fired from a sorcerer's
/// inventory. Evasion (DEX-save halve-to-zero) applies the same way it
/// does for the burst variant — RAW evasion fires on any DEX save vs
/// an effect that already grants half-on-save, including single-target
/// ones.
///
/// Adding a new variant is a one-static declaration — no new `Action`
/// impl needed.
pub struct SingleSaveDamageItem {
    /// Player-facing action name (e.g. "read disintegrate scroll").
    pub action_name: &'static str,
    /// Picker aliases.
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Log line prefix (e.g. "scroll of disintegrate"). The row reads
    /// `  {log_label}: {count}d{faces}({raw}){+flat} = {total} {type}`.
    pub log_label: &'static str,
    /// Damage dice (rolled once).
    pub dice: Dice,
    /// Flat bonus added on top of the rolled dice (RAW Disintegrate: +40;
    /// Finger of Death: +30; 0 for "pure dice" payloads).
    pub flat_bonus: u32,
    /// Damage type emitted on the side-effect.
    pub damage_type: DamageType,
    /// Save ability for the target.
    pub save: AbilityScoreType,
    /// Save DC (typically 15 for SRD scrolls / wands).
    pub dc: i32,
    /// Maximum reach in tiles for the targeting picker.
    pub reach: isize,
    /// `true` ⇒ a successful save halves damage (Finger of Death style);
    /// `false` ⇒ a successful save zeros it (Disintegrate style). Evasion
    /// promotes both branches by one tier — pass+evasion zeros either
    /// way, fail+evasion halves either way.
    pub save_for_half: bool,
}

impl Action for SingleSaveDamageItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let raw = encounter.roll(&self.dice);
        let total = raw.saturating_add(self.flat_bonus);
        encounter.log(format!(
            "  {}: {}d{}({})+{} = {} {}",
            self.log_label,
            self.dice.count,
            self.dice.faces,
            raw,
            self.flat_bonus,
            total,
            self.damage_type,
        ));
        let save =
            encounter.roll_save_against_caster(target_id, self.save, self.dc, caster_id);
        let passed = save.passed();
        // RAW Evasion: only fires on DEX saves against effects that
        // allow half-damage on a successful save. Pass + evasion → 0;
        // fail + evasion → half. No-save-half effects (Disintegrate)
        // never trigger evasion — evasion has nothing to "evade" up to.
        let has_evasion = self.save == AbilityScoreType::Dexterity
            && self.save_for_half
            && encounter
                .actors
                .get(&target_id)
                .is_some_and(|a| a.has_evasion());
        let policy = if self.save_for_half {
            SaveDamagePolicy::HalfOnSave
        } else {
            SaveDamagePolicy::NoneOnSave
        };
        let dmg = if has_evasion {
            policy.apply_mitigated(
                total,
                passed,
                crate::engine::saves::SaveMitigation::Evasion,
            )
        } else {
            policy.apply(total, passed)
        };
        if dmg == 0 {
            if has_evasion && passed {
                encounter.log(format!(
                    "  evasion: {} takes no damage",
                    encounter.actor_name(target_id)
                ));
            }
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: self.damage_type,
        })]
    }
}

/// Drink an Antitoxin: removes the Poisoned condition and grants a flat
/// +5 save bonus until the next long rest (5e abstracts this as
/// "advantage on poison saves for an hour"; we approximate with a flat
/// save buff). Single-use; consumes one Antitoxin from inventory.
pub struct DrinkAntitoxin {}

impl Action for DrinkAntitoxin {
    fn name(&self) -> &str {
        "drink antitoxin"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["antitoxin", "anti"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, ANTITOXIN_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, ANTITOXIN_NAME) {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.remove_condition(crate::conditions::Condition::Poisoned);
            // +5 save bonus represents advantage on poison saves; cleared
            // at long rest with the rest of buff state.
            actor.add_save_bonus_buff(5);
            let name = actor.name().to_string();
            encounter.log(format!("{} drinks an antitoxin.", name));
        }
        Vec::new()
    }
}

pub static DRINK_ANTITOXIN: DrinkAntitoxin = DrinkAntitoxin {};

/// Drink a Potion of Speed: bonus action; gain an extra Action this turn
/// AND a one-shot AC/save bonus from the haste-style buff. We model the
/// haste effect simply as: +1 to attack/save buff and an extra action
/// slot. Single-use; consumes one Potion of Speed from inventory.
pub struct DrinkPotionOfSpeed {}

impl Action for DrinkPotionOfSpeed {
    fn name(&self) -> &str {
        "drink potion of speed"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["speed", "haste"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, POTION_OF_SPEED_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, POTION_OF_SPEED_NAME) {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            let name = actor.name().to_string();
            // Refund an Action and dump the haste buffs. Haste in 5e is
            // a concentration spell, but a potion is fire-and-forget;
            // we skip concentration tracking and just install the flat
            // buffs which clear on long rest.
            actor.give_resource(Resource::Action);
            actor.add_attack_bonus_buff(1);
            actor.add_save_bonus_buff(1);
            encounter.log(format!(
                "{} drinks a potion of speed (extra action, +1 attack/save).",
                name
            ));
        }
        Vec::new()
    }
}

pub static DRINK_POTION_OF_SPEED: DrinkPotionOfSpeed = DrinkPotionOfSpeed {};

/// Potion of Heroism — bonus action; gain 10 temp HP and install the
/// `Heroic` condition for 10 rounds (immune to Frightened + a buff aura
/// that regens temp HP each round). 5e RAW: 1-hour duration; the engine
/// collapses to 10 rounds to match the combat-scale timer envelope used
/// by the rest of the potion family. Routes through the shared
/// `SelfConditionItem` impl via the `temp_hp` lane so the install +
/// temp-HP grant flow as a single declarative static (no bespoke Action
/// impl). Refresh is allowed so a wounded ally whose Heroic ticked low
/// can re-drink for the fresh 10-HP cushion.
pub static DRINK_POTION_OF_HEROISM: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of heroism",
    action_aliases: &["heroism", "hero"],
    item_name: POTION_OF_HEROISM_NAME,
    log_text: "{actor} drinks a potion of heroism.",
    condition: Condition::Heroic,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: false,
    temp_hp: Some(10),
};

/// Potion of Invisibility — Action; installs the Invisible condition for
/// 10 rounds. Fires through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_INVISIBILITY: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of invisibility",
    action_aliases: &["invisibility", "invis"],
    item_name: POTION_OF_INVISIBILITY_NAME,
    log_text: "{actor} drinks a potion of invisibility.",
    condition: Condition::Invisible,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: false,
    temp_hp: None,
};

const POTION_OF_SUPERIOR_HEALING_NAME: &str = "Potion of Superior Healing";
const POTION_OF_STONESKIN_NAME: &str = "Potion of Stoneskin";

/// Potion of Superior Healing — 8d4+8 self-heal, Action. Top of the
/// healing-potion tier in this engine (Healing 2d4+2, Greater 4d4+4,
/// Superior 8d4+8). 5e RAW has a 10d4+20 Supreme tier above this; we
/// stop at Superior for the loot pool. Bonus-action cost would
/// trivialize action-economy at this payload tier, so the Superior
/// variant stays at full Action.
pub static DRINK_SUPERIOR_HEALING_POTION: SelfHealItem = SelfHealItem {
    action_name: "drink superior healing potion",
    action_aliases: &["potion++", "drink++"],
    item_name: POTION_OF_SUPERIOR_HEALING_NAME,
    log_label: "potion of superior healing",
    dice: Dice::new(8, 4),
    flat_bonus: 8,
    bonus_action: false,
};

/// Potion of Stoneskin — Action; installs `DamageResistant` for 10
/// rounds (halve all incoming damage). 5e RAW: the Stoneskin spell is
/// resistant to bludgeoning / piercing / slashing only; we model via the
/// engine's blanket `DamageResistant` condition (the same one Stoneskin
/// the spell installs) so the potion delivers the spell's exact
/// envelope. Single-use; rejects re-drink when the buff is already up.
pub static DRINK_POTION_OF_STONESKIN: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of stoneskin",
    action_aliases: &["stoneskin", "stone"],
    item_name: POTION_OF_STONESKIN_NAME,
    log_text: "{actor} drinks a potion of stoneskin; their skin hardens.",
    condition: Condition::DamageResistant,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

const SCROLL_OF_LIGHTNING_BOLT_NAME: &str = "Scroll of Lightning Bolt";

/// Scroll of Lightning Bolt: 8d6 lightning DEX-save burst. Tighter
/// radius (2 tiles) than the Fireball scroll, longer reach. Fires
/// through the shared `BurstSaveDamageItem` impl.
pub static READ_LIGHTNING_BOLT_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read lightning bolt scroll",
    action_aliases: &["lb scroll", "lightning scroll"],
    item_name: SCROLL_OF_LIGHTNING_BOLT_NAME,
    log_label: "scroll of lightning bolt",
    dice: Dice::new(8, 6),
    damage_type: DamageType::Lightning,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 2,
    reach: 40,
};

/// Config struct for "single-target heal" consumable items — the shared
/// shape behind Scroll of Cure Wounds (touch, 2d8+2) and any future
/// ally-targetable healing scroll / wand at varying tiers. Each static
/// instance encodes the per-cast dice / flat bonus / reach / cost; the
/// `Action` impl below pops the item from inventory and emits a `Heal`
/// side-effect against the picked target. Mirrors `SelfHealItem` for the
/// ally-targetable lane (the self-only potion family sits on the other
/// side).
///
/// Adding a new variant (e.g. Wand of Cure Wounds at 3d8+3) is a one-
/// static declaration — no new `Action` impl needed.
pub struct SingleTargetHealItem {
    /// Player-facing action name (e.g. "read cure wounds scroll").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["cw scroll", "cure scroll"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Log line prefix (e.g. "scroll of cure wounds"). The row reads
    /// `  {log_label}: {count}d{faces}({raw}){:+flat} = {amount} HP`.
    pub log_label: &'static str,
    /// Healing dice (e.g. 2d8 for Cure Wounds scroll).
    pub dice: Dice,
    /// Flat bonus added to the rolled dice. Stand-in for the spell's
    /// caster-ability modifier — the scroll has no caster-ability tie,
    /// so a fixed value keeps the expected total comparable across
    /// readers (a +2 ≈ a low-level cleric's WIS-mod).
    pub flat_bonus: i32,
    /// Maximum reach in tiles for the targeting picker (1 for touch-
    /// range, 24 for "healing word range" 60 ft, etc.).
    pub reach: isize,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost.
    pub bonus_action: bool,
}

impl Action for SingleTargetHealItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        // Implicitly true at touch range, but kept on for longer-reach
        // variants too so the picker doesn't surface targets behind walls.
        true
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, self.item_name) {
            return false;
        }
        // Ally-target gate: a heal scroll picked at an enemy is a no-op
        // RAW. Folding it into validate keeps the picker UX honest — the
        // affordability check, the LOS check, and the ally check all
        // agree on whether the action can fire. Without this, a queued
        // heal on a hostile target would fall through to `side_effects`,
        // consume the scroll, and then drop the heal into the enemy's
        // HP pool — strictly bad. Mirrors the ally-check `first_ally_target_id`
        // does on the spell-side `Aid` / `Cure Wounds` impls.
        first_ally_target_id(encounter, caster_id, target_ids).is_some()
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let raw = encounter.roll(&self.dice) as i32;
        let amount = (raw + self.flat_bonus).max(1) as u32;
        encounter.log(format!(
            "  {}: {}d{}({}){:+} = {} HP",
            self.log_label, self.dice.count, self.dice.faces, raw, self.flat_bonus, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

/// Scroll of Cure Wounds — touch (1-tile) ally heal for 2d8+2. Fires
/// through the shared `SingleTargetHealItem` impl. The +2 stand-in for
/// the spell's caster WIS / CHA modifier keeps the expected total
/// comparable to a low-level cleric's Cure Wounds without binding the
/// scroll to a caster ability.
pub static READ_CURE_WOUNDS_SCROLL: SingleTargetHealItem = SingleTargetHealItem {
    action_name: "read cure wounds scroll",
    action_aliases: &["cw scroll", "cure scroll"],
    item_name: SCROLL_OF_CURE_WOUNDS_NAME,
    log_label: "scroll of cure wounds",
    dice: Dice::new(2, 8),
    flat_bonus: 2,
    reach: 1,
    bonus_action: false,
};

const PEARL_OF_POWER_NAME: &str = "Pearl of Power";
const GREATER_PEARL_OF_POWER_NAME: &str = "Greater Pearl of Power";
const SUPREME_PEARL_OF_POWER_NAME: &str = "Supreme Pearl of Power";
const BOOTS_OF_SPEED_NAME: &str = "Boots of Speed";

/// Config struct for "single-use spell-slot refund" consumables — the
/// shared shape behind Pearl of Power and its Greater / Supreme tiers.
/// Each static instance encodes which slot level it refunds; the `Action`
/// impl below validates that the holder has a spent slot at that level,
/// pops the pearl, and restores one slot.
///
/// Adding a new pearl tier (e.g. a level-4 Archmage's Pearl) is a one-
/// static declaration — no new `Action` impl needed.
pub struct PearlOfPowerItem {
    /// Player-facing action name (e.g. "use pearl of power").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["pearl", "pop"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on (e.g.
    /// "Pearl of Power").
    pub item_name: &'static str,
    /// Spell-slot level to refund (1, 2, 3, ...). The validate path
    /// rejects when the holder has no spent slot at this level; the
    /// side-effect builder restores exactly one slot at this level.
    pub slot_level: u32,
}

impl Action for PearlOfPowerItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, self.item_name) {
            return false;
        }
        // Reject the use when the actor has no spent slot at this level
        // to restore — pearling up to refund a slot they didn't spend
        // is a no-op and would just burn the consumable. The check folds
        // through the slot manager so a caster with no slots at this
        // tier (e.g. a Greater Pearl in a level-1-only build) is gated
        // the same way.
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let info = actor.spell_slot_manager.spell_slots(self.slot_level);
        info.max_spell_slots > 0 && info.spell_slots < info.max_spell_slots
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            let name = actor.name().to_string();
            let restored = actor
                .spell_slot_manager
                .restore_spell_slot(self.slot_level, 1);
            if restored {
                encounter.log(format!(
                    "{} crushes a {}; a level-{} slot returns.",
                    name,
                    self.item_name.to_lowercase(),
                    self.slot_level
                ));
            } else {
                encounter.log(format!(
                    "{} crushes a {} — but the slot was already full.",
                    name,
                    self.item_name.to_lowercase()
                ));
            }
        }
        Vec::new()
    }
}

/// Pearl of Power — bonus action; restore one expended level-1 spell
/// slot. Fires through the shared `PearlOfPowerItem` impl. 5e RAW
/// refunds a slot of level 3 or lower; the engine ladders the loot
/// table through three pearl tiers instead so the per-tier item carries
/// the slot-level it refunds explicitly.
pub static USE_PEARL_OF_POWER: PearlOfPowerItem = PearlOfPowerItem {
    action_name: "use pearl of power",
    action_aliases: &["pearl", "pop"],
    item_name: PEARL_OF_POWER_NAME,
    slot_level: 1,
};

/// Greater Pearl of Power — bonus action; restore one expended level-2
/// spell slot. Sits a tier above the regular Pearl in the loot pool.
/// Fires through the shared `PearlOfPowerItem` impl.
pub static USE_GREATER_PEARL_OF_POWER: PearlOfPowerItem = PearlOfPowerItem {
    action_name: "use greater pearl of power",
    action_aliases: &["pearl+", "pop+"],
    item_name: GREATER_PEARL_OF_POWER_NAME,
    slot_level: 2,
};

/// Supreme Pearl of Power — bonus action; restore one expended level-3
/// spell slot. Top of the pearl ladder; matches the RAW pearl's
/// "level 3 or lower" envelope. Fires through the shared
/// `PearlOfPowerItem` impl.
pub static USE_SUPREME_PEARL_OF_POWER: PearlOfPowerItem = PearlOfPowerItem {
    action_name: "use supreme pearl of power",
    action_aliases: &["pearl++", "pop++"],
    item_name: SUPREME_PEARL_OF_POWER_NAME,
    slot_level: 3,
};

/// Boots of Speed — Bonus Action; installs `Fleet` for 10 rounds, which
/// is RAW's whole first sentence: *"your walking speed is doubled."*
/// Single-use consumable, fired through the shared `SelfConditionItem`
/// impl.
///
/// It used to install `Hasted` and this docstring used to say so, on the
/// grounds that the condition was a convenient bundle of AC, Dexterity
/// saves and speed. RAW's boots grant none of the first two, and once
/// `Hasted` grew the spell's extra Action the borrowed bundle would have
/// handed a bonus-action item a third of a third-level spell. See
/// `Condition::Fleet`.
///
/// Not modeled: RAW's second sentence, "opportunity attacks against you
/// have disadvantage".
pub static WEAR_BOOTS_OF_SPEED: SelfConditionItem = SelfConditionItem {
    action_name: "wear boots of speed",
    action_aliases: &["boots", "speedboots"],
    item_name: BOOTS_OF_SPEED_NAME,
    log_text: "{actor} taps the heels of the boots of speed; everything blurs.",
    condition: Condition::Fleet,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

const SCROLL_OF_CONE_OF_COLD_NAME: &str = "Scroll of Cone of Cold";
const WAND_OF_MAGIC_MISSILES_NAME: &str = "Wand of Magic Missiles";

/// Scroll of Cone of Cold: 8d8 cold CON-save burst (mirrors the
/// `CONE_OF_COLD` spell's 6-tile burst approximation of the 60-ft cone).
/// Distinct from the Fireball / Lightning Bolt scrolls in its save
/// ability (CON, not DEX) and damage tier (d8 pool, not d6).
pub static READ_CONE_OF_COLD_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read cone of cold scroll",
    action_aliases: &["coc scroll", "cone scroll"],
    item_name: SCROLL_OF_CONE_OF_COLD_NAME,
    log_label: "scroll of cone of cold",
    dice: Dice::new(8, 8),
    damage_type: DamageType::Cold,
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 6,
    // Self-cone in RAW; we cap the picker at the cone's reach (60 ft
    // = 24 tiles) so the targeting reticle doesn't drop across the
    // whole map. Mirrors `CONE_OF_COLD::reach_tiles`.
    reach: 24,
};

/// Wand of Magic Missiles — 5 darts of 1d4+1 force each, auto-hit, no
/// save. Sits a tier above the 3-dart scroll. Fires through the shared
/// `MagicMissileItem` impl.
pub static USE_WAND_OF_MAGIC_MISSILES: MagicMissileItem = MagicMissileItem {
    action_name: "use wand of magic missiles",
    action_aliases: &["wand", "mm wand"],
    item_name: WAND_OF_MAGIC_MISSILES_NAME,
    log_label: "wand of magic missiles",
    darts: 5,
    reach: 30,
};

const POTION_OF_FLYING_NAME: &str = "Potion of Flying";
const POTION_OF_CLIMBING_NAME: &str = "Potion of Climbing";
const WAND_OF_FIREBALLS_NAME: &str = "Wand of Fireballs";

/// Potion of Flying — Action; installs `Flying` for 10 rounds. Re-uses
/// the spell-side Flying condition (AC / disadvantage-to-ranged-attackers
/// / speed bump). Rejects re-drink when already flying so the consumable
/// isn't burned on a no-op timer refresh.
pub static DRINK_POTION_OF_FLYING: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of flying",
    action_aliases: &["fly", "flying"],
    item_name: POTION_OF_FLYING_NAME,
    log_text: "{actor} drinks a potion of flying.",
    condition: Condition::Flying,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Climbing — Bonus Action; installs `SpiderClimbing` for 10
/// rounds. Cheaper / lesser mobility envelope than Potion of Flying.
/// Re-uses the Spider Climb spell condition.
pub static DRINK_POTION_OF_CLIMBING: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of climbing",
    action_aliases: &["climb", "climbing"],
    item_name: POTION_OF_CLIMBING_NAME,
    log_text: "{actor} drinks a potion of climbing.",
    condition: Condition::SpiderClimbing,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Wand of Fireballs: 8d6 fire DEX-save burst. Sits a tier above the
/// Fireball scroll (6d6) — same shape, bigger pool. 5e RAW: 7 charges
/// at level 3 (+1 per extra charge); we collapse to a single 8d6 cast
/// per the engine's charge-less loot model.
pub static USE_WAND_OF_FIREBALLS: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "use wand of fireballs",
    action_aliases: &["fireballs", "fireball wand"],
    item_name: WAND_OF_FIREBALLS_NAME,
    log_label: "wand of fireballs",
    dice: Dice::new(8, 6),
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    reach: 60,
};

const WAND_OF_LIGHTNING_BOLTS_NAME: &str = "Wand of Lightning Bolts";

/// Wand of Lightning Bolts: 10d6 lightning DEX-save burst. Sits a tier
/// above the Lightning Bolt scroll (8d6) — same shape, bigger pool.
/// Sibling to `USE_WAND_OF_FIREBALLS` for the lightning lane.
pub static USE_WAND_OF_LIGHTNING_BOLTS: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "use wand of lightning bolts",
    action_aliases: &["lightning wand", "lb wand"],
    item_name: WAND_OF_LIGHTNING_BOLTS_NAME,
    log_label: "wand of lightning bolts",
    dice: Dice::new(10, 6),
    damage_type: DamageType::Lightning,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 2,
    reach: 40,
};

const SCROLL_OF_SHATTER_NAME: &str = "Scroll of Shatter";

/// Scroll of Shatter: 3d8 thunder CON-save burst. Fills the "thunder
/// damage scroll" niche — alongside Fireball (fire), Lightning Bolt
/// (lightning), and Cone of Cold (cold). 5e Shatter is a level-2 spell;
/// the scroll fires at its baseline 3d8 RAW. Tight 2-tile radius (vs the
/// Fireball scroll's 4) keeps the thunder lane in the "small but loud"
/// envelope.
pub static READ_SHATTER_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read shatter scroll",
    action_aliases: &["shatter", "shatter scroll"],
    item_name: SCROLL_OF_SHATTER_NAME,
    log_label: "scroll of shatter",
    dice: Dice::new(3, 8),
    damage_type: DamageType::Thunder,
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 2,
    // 60 ft range = 24 tiles, matching the spell's reach.
    reach: 24,
};

const WAND_OF_CONE_OF_COLD_NAME: &str = "Wand of Cone of Cold";

/// Wand of Cone of Cold: 10d8 cold CON-save burst. Sits a tier above
/// the Cone of Cold scroll (8d8) — same shape, bigger pool. Top-of-pool
/// burst-wand entry alongside Wand of Fireballs (8d6 fire) and Wand of
/// Lightning Bolts (10d6 lightning). Mirrors `READ_CONE_OF_COLD_SCROLL`'s
/// CON-save / 6-tile burst footprint.
pub static USE_WAND_OF_CONE_OF_COLD: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "use wand of cone of cold",
    action_aliases: &["coc wand", "cone wand"],
    item_name: WAND_OF_CONE_OF_COLD_NAME,
    log_label: "wand of cone of cold",
    dice: Dice::new(10, 8),
    damage_type: DamageType::Cold,
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 6,
    reach: 24,
};

const SCROLL_OF_MASS_HEALING_WORD_NAME: &str = "Scroll of Mass Healing Word";

/// Config struct for "self-centered burst that heals up to N nearest
/// allies" consumable items — the shared shape behind Mass Healing Word
/// scroll (bonus action, long-reach 24-tile envelope) and Mass Cure
/// Wounds scroll (Action, tight 4-tile burst). Each static instance
/// encodes the dice / flat bonus / action economy / range / target cap;
/// the `Action` impl below filters allies, sorts them by distance, and
/// fires a `Heal` per pick.
///
/// Heal targets include dying allies (a dying ally at 0 HP gets bumped
/// back up by the heal), matching the spell-side `MASS_HEALING_WORD` /
/// `MASS_CURE_WOUNDS` impls.
///
/// Adding a new variant (e.g. a Wand of Mass Healing Word, or a level-9
/// Mass Heal scroll) is a one-static declaration.
pub struct MultiTargetHealItem {
    /// Player-facing action name.
    pub action_name: &'static str,
    /// Picker aliases.
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Log line prefix. The row reads
    /// `  {log_label}: {count}d{faces}({raw})+{flat_bonus} = {amount} HP each`.
    pub log_label: &'static str,
    /// Healing dice (e.g. 1d4 for Mass Healing Word, 3d8 for Mass Cure
    /// Wounds). Rolled once and shared across every target — matches the
    /// spell-side semantics.
    pub dice: Dice,
    /// Flat bonus added to the rolled dice. The scroll has no caster-
    /// ability tie so this stands in for the caster's spellcasting mod.
    pub flat_bonus: i32,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost. Mass Healing
    /// Word is bonus action; Mass Cure Wounds is full Action.
    pub bonus_action: bool,
    /// Maximum tile-Chebyshev distance from the caster's footprint to
    /// any ally that qualifies for the heal. Mass Healing Word: 24
    /// (60 ft RAW). Mass Cure Wounds: 4 (30 ft RAW = 12 tiles, but the
    /// engine uses a tighter 10-ft-burst-from-self envelope for the
    /// existing front-line heal niche).
    pub range_tiles: isize,
    /// Maximum number of allies to heal. 5e RAW caps mass heals at
    /// 6 for the level-3 / level-5 family.
    pub max_targets: usize,
}

impl Action for MultiTargetHealItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let raw = encounter.roll(&self.dice) as i32;
        let amount = (raw + self.flat_bonus).max(1) as u32;
        encounter.log(format!(
            "  {}: {}d{}({}){:+} = {} HP each",
            self.log_label, self.dice.count, self.dice.faces, raw, self.flat_bonus, amount
        ));
        // Layer a heal-specific priority sort on top of the shared
        // ally-candidates walker:
        //   0 = dying (stabilize / revive — most urgent)
        //   1 = wounded combat-active (heal HP that won't go to waste)
        //   2 = full-HP combat-active (last-resort filler if slots remain)
        // We invert hp_deficit so larger deficits sort earlier inside
        // priority 1, then break ties by distance and id for determinism.
        // Matches the "smart player picks the wounded ones" intent of
        // 5e's "you choose creatures" RAW wording — the previous
        // nearest-first heuristic could waste max_targets slots on
        // full-HP front-liners while dying allies sat unattended.
        let mut candidates: Vec<(u8, std::cmp::Reverse<u32>, isize, usize)> = encounter
            .ally_candidates_in_range(caster_id, self.range_tiles)
            .into_iter()
            .filter_map(|(id, dist)| {
                let a = encounter.actors.get(&id)?;
                let dying = a.is_dying();
                let hp = a.hitpoints();
                let max_hp = a.max_hitpoints();
                let deficit = max_hp.saturating_sub(hp);
                let priority = if dying {
                    0u8
                } else if deficit > 0 {
                    1u8
                } else {
                    2u8
                };
                Some((priority, std::cmp::Reverse(deficit), dist, id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(self.max_targets);
        candidates
            .into_iter()
            .map(|(_, _, _, id)| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

/// Scroll of Mass Healing Word — bonus action; heal up to 6 nearest
/// allies (combat-active or dying) within 60 ft (24 tiles) for 1d4+3
/// HP each. The +3 stand-in for the caster's WIS modifier sits between
/// a level-1 cleric (+2) and a high-level cleric (+5) for the typical
/// reader. Fires through the shared `MultiTargetHealItem` impl.
pub static READ_MASS_HEALING_WORD_SCROLL: MultiTargetHealItem = MultiTargetHealItem {
    action_name: "read mass healing word scroll",
    action_aliases: &["mhw scroll", "mass-heal scroll"],
    item_name: SCROLL_OF_MASS_HEALING_WORD_NAME,
    log_label: "scroll of mass healing word",
    dice: Dice::new(1, 4),
    flat_bonus: 3,
    bonus_action: true,
    range_tiles: 24,
    max_targets: 6,
};

const POTION_OF_SUPREME_HEALING_NAME: &str = "Potion of Supreme Healing";
const POTION_OF_MAGE_ARMOR_NAME: &str = "Potion of Mage Armor";
const POTION_OF_BLUR_NAME: &str = "Potion of Blur";
const GREATER_WAND_OF_MAGIC_MISSILES_NAME: &str = "Greater Wand of Magic Missiles";
const SCROLL_OF_BURNING_HANDS_NAME: &str = "Scroll of Burning Hands";
const SCROLL_OF_THUNDERWAVE_NAME: &str = "Scroll of Thunderwave";

/// Potion of Supreme Healing — 10d4+20 self-heal, Action. Top tier of the
/// healing-potion ladder, matching 5e RAW. Sits above Potion of Superior
/// Healing (8d4+8) for the rare drop slot. Fires through the shared
/// `SelfHealItem` impl.
pub static DRINK_SUPREME_HEALING_POTION: SelfHealItem = SelfHealItem {
    action_name: "drink supreme healing potion",
    action_aliases: &["potion+++", "drink+++"],
    item_name: POTION_OF_SUPREME_HEALING_NAME,
    log_label: "potion of supreme healing",
    dice: Dice::new(10, 4),
    flat_bonus: 20,
    bonus_action: false,
};

/// Potion of Mage Armor — Action; installs `MageArmored` for 10 rounds
/// (AC floor of 13 + DEX modifier). 5e RAW spell duration is 8 hours;
/// we collapse to the engine's combat-scale 10-round envelope. Single-
/// use consumable; rejects re-drink when the buff is already up so the
/// potion isn't burned on a no-op timer refresh. Routes through the
/// shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_MAGE_ARMOR: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of mage armor",
    action_aliases: &["ma potion", "mage armor potion"],
    item_name: POTION_OF_MAGE_ARMOR_NAME,
    log_text: "{actor} drinks a potion of mage armor; an arcane shell forms.",
    condition: Condition::MageArmored,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Blur — Action; installs `Blurred` for 10 rounds (attacks
/// against the holder have disadvantage). Single-use consumable. 5e RAW:
/// the Blur spell is concentration; the potion bypasses concentration so
/// the holder can stack it on top of an existing concentration buff.
/// Rejects re-drink when the buff is already up.
pub static DRINK_POTION_OF_BLUR: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of blur",
    action_aliases: &["blur potion", "blur"],
    item_name: POTION_OF_BLUR_NAME,
    log_text: "{actor} drinks a potion of blur; their outline wavers.",
    condition: Condition::Blurred,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Greater Wand of Magic Missiles — 7 darts of 1d4+1 force each, auto-hit,
/// no save. Top of the Magic Missile loot ladder: Scroll (3 darts) →
/// Wand (5 darts) → Greater Wand (7 darts). Matches the RAW upcast at
/// level 4. Fires through the shared `MagicMissileItem` impl.
pub static USE_GREATER_WAND_OF_MAGIC_MISSILES: MagicMissileItem = MagicMissileItem {
    action_name: "use greater wand of magic missiles",
    action_aliases: &["mm wand+", "wand+"],
    item_name: GREATER_WAND_OF_MAGIC_MISSILES_NAME,
    log_label: "greater wand of magic missiles",
    darts: 7,
    reach: 30,
};

/// Scroll of Burning Hands — 3d6 fire DEX-save burst, 2-tile radius.
/// Single-use consumable. Mirrors the `BURNING_HANDS` spell's envelope at
/// the level-1 baseline (3d6). Fills the entry-level fire-burst niche
/// between the cantrip Fire Bolt and the Fireball scroll (6d6) in the
/// loot pool. Fires through the shared `BurstSaveDamageItem` impl.
pub static READ_BURNING_HANDS_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read burning hands scroll",
    action_aliases: &["bh scroll", "hands scroll"],
    item_name: SCROLL_OF_BURNING_HANDS_NAME,
    log_label: "scroll of burning hands",
    dice: Dice::new(3, 6),
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 2,
    // 15 ft cone in RAW; capped to 6 tiles for the picker since the
    // burst origin is the cone's far edge.
    reach: 6,
};

/// Scroll of Thunderwave — 2d8 thunder CON-save burst, 2-tile radius.
/// Single-use consumable. Mirrors the `THUNDERWAVE` spell's damage roll
/// at the level-1 baseline; the scroll variant drops the RAW push rider
/// (the helper-shared `BurstSaveDamageItem` doesn't fork into a push
/// follow-up — that lives on the spell-side custom impl). Sits in the
/// loot pool as the cheap thunder-burst entry, distinct from the rare
/// Scroll of Shatter (3d8) and matching the thunder lane's "small but
/// loud" envelope.
pub static READ_THUNDERWAVE_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read thunderwave scroll",
    action_aliases: &["tw scroll", "thunderwave scroll"],
    item_name: SCROLL_OF_THUNDERWAVE_NAME,
    log_label: "scroll of thunderwave",
    dice: Dice::new(2, 8),
    damage_type: DamageType::Thunder,
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 2,
    // 15 ft cube self-centered in RAW; we cap at the picker reach for
    // safety (caster picks the cube's center). 6 tiles ≈ 15 ft.
    reach: 6,
};

/// Config struct for "burst save-or-condition" consumable items — the
/// shared shape behind Wand of Web (Restrained), Pipes of Haunting
/// (Frightened), and any future area-control consumable that hits a
/// tile with a save-or-suck install instead of damage. Each static
/// instance encodes a single item's per-cast configuration; the `Action`
/// impl below sweeps every combat-active enemy inside the burst,
/// routes each save through `roll_save_against_caster` (so Heightened
/// Spell metamagic still bites the first save) and queues an
/// `ApplyCondition` on every failed save.
///
/// Mirrors `BurstSaveDamageItem` for the CC half of the consumable
/// envelope. Adding a new variant (e.g. a Wand of Sleet that spams
/// Prone on a DEX save) is a one-static declaration — no new `Action`
/// impl needed. Enemy-only filtering matches the player-friendly
/// design of every other harmful item: the player's allies caught in
/// the burst zone never make a save, mirroring `enemy_burst_targets`'s
/// "spare the friendly side" envelope.
pub struct BurstSaveConditionItem {
    /// Player-facing action name (e.g. "use wand of web").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["web", "wand of web"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Full log line emitted on use. The `{actor}` placeholder is
    /// substituted with the caster's name; no other formatting is
    /// performed.
    pub log_text: &'static str,
    /// Save ability for the burst (e.g. DEX for Web, WIS for Haunting).
    pub save: AbilityScoreType,
    /// Save DC (typically 15 for SRD scrolls / wands).
    pub dc: i32,
    /// Burst radius in tiles.
    pub radius: isize,
    /// Maximum reach in tiles for the targeting picker.
    pub reach: isize,
    /// Condition to install on a failed save.
    pub condition: Condition,
    /// Timer for the install (typically `Rounds(10)` for combat-scale
    /// CC consumables — ~1 minute RAW).
    pub timer: ConditionTimer,
}

impl Action for BurstSaveConditionItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst {
            radius: self.radius,
        }
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        // Pure crowd-control — no HP loss. Keeps the AI's focus-fire
        // pipeline from picking these over actual damage attacks.
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(center) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Enemy-only burst — consistent with the "player-friendly
        // consumable" stance of every other harmful item in the pool.
        // The AI's `try_attack_aoe` heuristic already filters tiles that
        // catch allies; this lets the player aim through their own line
        // without burning the consumable on allies that pass / fail RAW.
        for tid in encounter.enemy_burst_targets(caster_id, center, self.radius) {
            // Skip targets immune to this condition — the install would
            // no-op at `add_condition` anyway. The bigger reason for the
            // skip is the Sorcerer Heightened Spell prime: it consumes
            // on the FIRST save in the burst, so wasting it on a target
            // whose install can't land would leak the metamagic onto a
            // no-op. The HYPNOTIC_PATTERN spell-side impl does the same
            // up-front filter for the same reason.
            if encounter.actor_immune_to_condition(tid, self.condition) {
                continue;
            }
            let save =
                encounter.roll_save_against_caster(tid, self.save, self.dc, caster_id);
            if !save.passed() {
                // Through the linked installer, so a condition that
                // carries a back-link records the wielder — an item's
                // Frightened has a source in the room exactly as a
                // spell's does.
                effects.extend(crate::engine::side_effects::install_condition_with_link(
                    self.condition,
                    tid,
                    caster_id,
                    self.timer,
                ));
            }
        }
        effects
    }
}

/// Config struct for "single-target save-or-condition" consumable items
/// — the shared shape behind Wand of Paralysis (Paralyzed) and any
/// future single-target wand whose effect is a save-or-suck install
/// instead of damage. Mirrors `MagicMissileItem` (single-target damage
/// volley) for the CC half of the wand-style consumable envelope.
///
/// Adding a new variant (e.g. a Wand of Sleep that installs `Asleep`
/// on a single target) is a one-static declaration — no new `Action`
/// impl needed.
pub struct SingleSaveConditionItem {
    /// Player-facing action name (e.g. "use wand of paralysis").
    pub action_name: &'static str,
    /// Picker aliases.
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Full log line emitted on use. The `{actor}` placeholder is
    /// substituted with the caster's name; no other formatting is
    /// performed.
    pub log_text: &'static str,
    /// Save ability (e.g. CON for Paralysis, WIS for Fear).
    pub save: AbilityScoreType,
    /// Save DC (typically 15 for SRD wands).
    pub dc: i32,
    /// Maximum reach in tiles for the targeting picker.
    pub reach: isize,
    /// Condition to install on a failed save.
    pub condition: Condition,
    /// Timer for the install.
    pub timer: ConditionTimer,
}

impl Action for SingleSaveConditionItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));
        // Skip the save roll against a target who's immune to the
        // installed condition — the install can't land anyway, and
        // skipping preserves the Sorcerer Heightened Spell prime from
        // leaking onto a no-op save. Matches the burst-variant's
        // up-front filter. The consumable still consumes (the player
        // chose to fire it; that's a UX decision).
        if encounter.actor_immune_to_condition(target_id, self.condition) {
            return Vec::new();
        }
        let save =
            encounter.roll_save_against_caster(target_id, self.save, self.dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        crate::engine::side_effects::install_condition_with_link(
            self.condition,
            target_id,
            caster_id,
            self.timer,
        )
    }
}

const WAND_OF_WEB_NAME: &str = "Wand of Web";
const PIPES_OF_HAUNTING_NAME: &str = "Pipes of Haunting";
const WAND_OF_PARALYSIS_NAME: &str = "Wand of Paralysis";
const WAND_OF_FEAR_NAME: &str = "Wand of Fear";

/// Wand of Web — Action; 4-tile burst, DEX save vs DC 15, fail =
/// Restrained for 10 rounds. Single-use consumable. Mirrors the
/// `WEB` spell's burst envelope at a fixed save DC (the wand has no
/// caster-ability tie). 5e RAW: 7 charges casting the Web spell; we
/// collapse to a single-use fire-and-forget cast — no concentration,
/// no charges tracked. Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static USE_WAND_OF_WEB: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "use wand of web",
    action_aliases: &["web", "web wand"],
    item_name: WAND_OF_WEB_NAME,
    log_text: "{actor} flicks the wand of web; sticky strands erupt.",
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    condition: Condition::Restrained,
    timer: ConditionTimer::Rounds(10),
};

/// Pipes of Haunting — Action; 4-tile burst, WIS save vs DC 13, fail =
/// Frightened for 10 rounds. Single-use consumable. 5e RAW: 30-ft cone
/// fear-burst, 3 charges; we collapse to a one-shot cast with the same
/// shape as the other burst-save consumables. Pairs with the Wand of
/// Fear single-target variant — Pipes covers the "soft area fear"
/// niche, the wand covers the "hard single-target fear" niche. Fires
/// through the shared `BurstSaveConditionItem` impl.
pub static PLAY_PIPES_OF_HAUNTING: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "play pipes of haunting",
    action_aliases: &["pipes", "haunt"],
    item_name: PIPES_OF_HAUNTING_NAME,
    log_text: "{actor} plays the pipes of haunting; a mournful dirge fills the air.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    radius: 4,
    // 30 ft cone RAW; 12 tiles in the 2.5ft grid.
    reach: 12,
    condition: Condition::Frightened,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Paralysis — Action; single-target line, CON save vs DC 15,
/// fail = Paralyzed for 10 rounds. Single-use consumable. 5e RAW: 7
/// charges firing a 60-ft line of paralysis at one creature; we
/// collapse to a single-use beam cast — no charges tracked. Paralyzed
/// is one of the engine's hardest CC envelopes (zero movement, action
/// economy blocked, auto-fail STR/DEX saves, melee crits land
/// automatically), so the consumable sits in the rare half of the
/// loot pool. Fires through the shared `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_PARALYSIS: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of paralysis",
    action_aliases: &["paralysis", "paralyze"],
    item_name: WAND_OF_PARALYSIS_NAME,
    log_text: "{actor} aims the wand of paralysis; a chill beam lances out.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    condition: Condition::Paralyzed,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Fear — Action; single-target, WIS save vs DC 15, fail =
/// Frightened for 10 rounds. Single-use consumable. 5e RAW: 7 charges
/// casting Fear (a 30-ft cone) at level 3; we collapse to a single-
/// target single-use cast — no charges, no cone. Distinct from the
/// Pipes of Haunting (also Frightened) by save ability (WIS) — actually
/// the same — but the wand is single-target / harder DC (15 vs 13) so
/// it lands cleanly on a tougher target where the pipes' wider burst
/// might miss several saves. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_FEAR: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of fear",
    action_aliases: &["fear", "fear wand"],
    item_name: WAND_OF_FEAR_NAME,
    log_text: "{actor} brandishes the wand of fear; shadows lengthen.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    condition: Condition::Frightened,
    timer: ConditionTimer::Rounds(10),
};

const SCROLL_OF_HOLD_PERSON_NAME: &str = "Scroll of Hold Person";
const SCROLL_OF_HOLD_MONSTER_NAME: &str = "Scroll of Hold Monster";
const WAND_OF_CONFUSION_NAME: &str = "Wand of Confusion";
const SCROLL_OF_HYPNOTIC_PATTERN_NAME: &str = "Scroll of Hypnotic Pattern";
const SCROLL_OF_VITRIOLIC_SPHERE_NAME: &str = "Scroll of Vitriolic Sphere";
const ARCHMAGE_PEARL_OF_POWER_NAME: &str = "Archmage Pearl of Power";
const POTION_OF_SANCTUARY_NAME: &str = "Potion of Sanctuary";
const WAND_OF_CURE_WOUNDS_NAME: &str = "Wand of Cure Wounds";
const SCROLL_OF_HEALING_WORD_NAME: &str = "Scroll of Healing Word";
const POTION_OF_GROWTH_NAME: &str = "Potion of Growth";
const WAND_OF_GREATER_HEALING_NAME: &str = "Wand of Greater Healing";

/// Scroll of Hold Person — Action; single-target, WIS save vs DC 13,
/// fail = Paralyzed for 10 rounds. 5e RAW: level-2 enchantment with
/// concentration and re-save each turn; the scroll variant drops the
/// concentration / re-save mechanics for the simpler "fixed 10-round
/// paralysis" envelope every other CC consumable rides. Lower DC (13
/// vs the Wand of Paralysis's 15) keeps the scroll at the entry-level
/// CC tier alongside Pipes of Haunting. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_HOLD_PERSON_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read hold person scroll",
    action_aliases: &["hp scroll", "hold scroll"],
    item_name: SCROLL_OF_HOLD_PERSON_NAME,
    log_text: "{actor} reads a scroll of hold person; arcane shackles seek their mark.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    condition: Condition::Paralyzed,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Hold Monster — Action; single-target, WIS save vs DC 15,
/// fail = Paralyzed for 10 rounds. 5e RAW: level-5 enchantment, same
/// shape as Hold Person but lifts the "humanoid only" restriction. The
/// engine doesn't model creature types beyond template, so the scroll's
/// niche over Hold Person is purely the harder DC and longer reach
/// (90 ft RAW). Fires through the shared `SingleSaveConditionItem`
/// impl.
pub static READ_HOLD_MONSTER_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read hold monster scroll",
    action_aliases: &["hm scroll", "monster scroll"],
    item_name: SCROLL_OF_HOLD_MONSTER_NAME,
    log_text: "{actor} reads a scroll of hold monster; otherworldly chains lash out.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 90 ft range RAW; 36 tiles in the 2.5ft grid.
    reach: 36,
    condition: Condition::Paralyzed,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Confusion — Action; 4-tile burst, WIS save vs DC 15, fail =
/// Confused for 10 rounds (disadvantage on attacks AND no reactions).
/// 5e RAW: level-4 enchantment, 90-ft range / 10-ft cube; we collapse
/// to the burst envelope every other AoE CC consumable rides. Top-of-
/// pool burst CC alongside Wand of Paralysis (single-target Paralyzed)
/// — the wand of confusion trades single-target lockdown for a wider
/// soft-CC blanket. Fires through the shared `BurstSaveConditionItem`
/// impl.
pub static USE_WAND_OF_CONFUSION: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "use wand of confusion",
    action_aliases: &["confusion", "confuse"],
    item_name: WAND_OF_CONFUSION_NAME,
    log_text: "{actor} flourishes the wand of confusion; minds unravel.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    radius: 4,
    // 90 ft range RAW; 36 tiles in the 2.5ft grid.
    reach: 36,
    condition: Condition::Confused,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Hypnotic Pattern — Action; 4-tile burst, WIS save vs DC 14,
/// fail = Incapacitated for 10 rounds. 5e RAW: level-3 illusion, 120-ft
/// range / 30-ft cube, concentration; the scroll variant drops the
/// concentration gate and uses the burst envelope every other AoE CC
/// consumable rides. Lower DC (14 vs the Wand of Confusion's 15)
/// reflects the "common burst CC" niche between Pipes of Haunting
/// (DC 13 Frightened) and Wand of Confusion (DC 15 Confused). Fires
/// through the shared `BurstSaveConditionItem` impl.
pub static READ_HYPNOTIC_PATTERN_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read hypnotic pattern scroll",
    action_aliases: &["hp pattern", "hypnotic scroll"],
    item_name: SCROLL_OF_HYPNOTIC_PATTERN_NAME,
    log_text: "{actor} reads a scroll of hypnotic pattern; swirling lights mesmerize.",
    save: AbilityScoreType::Wisdom,
    dc: 14,
    radius: 4,
    // 120 ft range RAW; 48 tiles in the 2.5ft grid.
    reach: 48,
    condition: Condition::Incapacitated,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Vitriolic Sphere — Action; 10d4 acid DEX-save burst,
/// 4-tile radius. 5e RAW: level-4 evocation, 150-ft range / 20-ft
/// radius, 10d4 acid (failed save) + 5d4 next-turn drip (passed save).
/// We collapse the next-turn drip clause onto the front-loaded 10d4
/// since the engine's burst helper doesn't fork into a follow-up tick.
/// Fills the acid lane in the burst-damage scroll family — alongside
/// Fireball (fire), Lightning Bolt (lightning), Cone of Cold (cold),
/// and Shatter (thunder). Fires through the shared
/// `BurstSaveDamageItem` impl.
pub static READ_VITRIOLIC_SPHERE_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read vitriolic sphere scroll",
    action_aliases: &["vs scroll", "acid scroll"],
    item_name: SCROLL_OF_VITRIOLIC_SPHERE_NAME,
    log_label: "scroll of vitriolic sphere",
    dice: Dice::new(10, 4),
    damage_type: DamageType::Acid,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    // 150 ft range RAW; well past any current map. Capped at 60 to
    // match the Fireball scroll's picker envelope.
    reach: 60,
};

/// Archmage Pearl of Power — bonus action; restore one expended level-4
/// spell slot. Top of the pearl ladder above Supreme Pearl of Power
/// (level-3 refund). 5e RAW pearls cap at level-3 slots; the engine
/// extends the ladder to cover the level-4 slot tier as the
/// rarest-tier caster consumable. Fires through the shared
/// `PearlOfPowerItem` impl.
pub static USE_ARCHMAGE_PEARL_OF_POWER: PearlOfPowerItem = PearlOfPowerItem {
    action_name: "use archmage pearl of power",
    action_aliases: &["pearl+++", "pop+++"],
    item_name: ARCHMAGE_PEARL_OF_POWER_NAME,
    slot_level: 4,
};

/// Potion of Sanctuary — Bonus Action; installs `Sanctuary` for 10
/// rounds. 5e RAW: the Sanctuary spell is a level-1 abjuration, bonus
/// action cost, requires a willing target; the potion bypasses the
/// targeting constraint and only protects the drinker. Hostile actions
/// against the drinker require a WIS save vs the source's DC or they
/// silently no-op (see the `Sanctuary` condition docs for the gate).
/// The buff drops the moment the drinker themselves attacks or casts a
/// damaging spell. Fires through the shared `SelfConditionItem` impl;
/// rejects re-drink when already Sanctified so the consumable isn't
/// burned on a no-op timer refresh.
pub static DRINK_POTION_OF_SANCTUARY: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of sanctuary",
    action_aliases: &["sanctuary", "sanc"],
    item_name: POTION_OF_SANCTUARY_NAME,
    log_text: "{actor} drinks a potion of sanctuary; an unseen ward settles over them.",
    condition: Condition::Sanctuary,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Wand of Cure Wounds — Action; touch (1-tile) ally heal for 3d8+3.
/// Sits a tier above the Scroll of Cure Wounds (2d8+2) — same shape,
/// bigger pool. 5e RAW: 7 charges casting Cure Wounds at level 1-3;
/// we collapse to a single-use cast at the level-3 upcast (3d8) for
/// the engine's charge-less loot model. Fires through the shared
/// `SingleTargetHealItem` impl.
pub static USE_WAND_OF_CURE_WOUNDS: SingleTargetHealItem = SingleTargetHealItem {
    action_name: "use wand of cure wounds",
    action_aliases: &["cw wand", "cure wand"],
    item_name: WAND_OF_CURE_WOUNDS_NAME,
    log_label: "wand of cure wounds",
    dice: Dice::new(3, 8),
    flat_bonus: 3,
    reach: 1,
    bonus_action: false,
};

/// Scroll of Healing Word — Bonus Action; 24-tile ranged ally heal for
/// 1d4+3. 5e RAW: level-1 evocation, bonus action, 60-ft range. The
/// scroll-as-slot envelope drops the spell-slot cost. Pairs with the
/// touch-range Scroll of Cure Wounds — the healing-word scroll trades
/// payload for reach (kite-heal an ally across the room) and action
/// economy (BA vs Action). Fires through the shared
/// `SingleTargetHealItem` impl.
pub static READ_HEALING_WORD_SCROLL: SingleTargetHealItem = SingleTargetHealItem {
    action_name: "read healing word scroll",
    action_aliases: &["hw scroll", "word scroll"],
    item_name: SCROLL_OF_HEALING_WORD_NAME,
    log_label: "scroll of healing word",
    dice: Dice::new(1, 4),
    flat_bonus: 3,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    bonus_action: true,
};

/// Potion of Growth — Action; installs `Enlarged` for 10 rounds
/// (+1d4 weapon damage rider, size bump). 5e RAW: 1d4-hour duration
/// matching the Enlarge spell's "use enlarge twin" envelope; we
/// collapse to the combat-scale 10-round timer every other buff
/// consumable rides. Single-use; rejects re-drink when already
/// Enlarged so the consumable isn't burned on a no-op timer refresh.
/// Fires through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_GROWTH: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of growth",
    action_aliases: &["growth", "enlarge"],
    item_name: POTION_OF_GROWTH_NAME,
    log_text: "{actor} drinks a potion of growth; their frame surges in size.",
    condition: Condition::Enlarged,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Longstrider — Bonus Action; installs `Longstriding` for
/// 100 rounds (≈ 10 minutes, matching the Longstrider spell's effective
/// duration in this engine). +10 ft speed for one ally / self.
/// Bonus-action cost — same envelope as Boots of Speed / Potion of
/// Flying — so the holder can drink AND move on the same turn.
/// Routes through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_LONGSTRIDER: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of longstrider",
    action_aliases: &["longstrider", "longstride", "ls-potion"],
    item_name: POTION_OF_LONGSTRIDER_NAME,
    log_text: "{actor} drinks a potion of longstrider; their stride lengthens.",
    condition: Condition::Longstriding,
    timer: ConditionTimer::Rounds(100),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

const POTION_OF_LONGSTRIDER_NAME: &str = "Potion of Longstrider";

/// Wand of Greater Healing — Action; touch (1-tile) ally heal for
/// 4d8+4. Sits a tier above the Wand of Cure Wounds (3d8+3) and the
/// Scroll of Cure Wounds (2d8+2). 5e RAW: 7 charges casting Cure
/// Wounds at level 4 (4d8); we collapse to a single 4d8+4 cast for
/// the engine's charge-less loot model. Fires through the shared
/// `SingleTargetHealItem` impl.
pub static USE_WAND_OF_GREATER_HEALING: SingleTargetHealItem = SingleTargetHealItem {
    action_name: "use wand of greater healing",
    action_aliases: &["cw wand+", "cure wand+"],
    item_name: WAND_OF_GREATER_HEALING_NAME,
    log_label: "wand of greater healing",
    dice: Dice::new(4, 8),
    flat_bonus: 4,
    reach: 1,
    bonus_action: false,
};

/// Config struct for "single-target ally buff" consumable items — the
/// shared shape behind Scroll of Bless / Scroll of Shield of Faith / any
/// future ally-targetable condition-install scroll. Mirrors
/// `SingleTargetHealItem` for the buff lane (no dice / flat_bonus — the
/// install is fixed by the scroll's spell). Each static instance encodes
/// the condition + timer + reach + cost; the `Action` impl below pops the
/// item from inventory and emits an `ApplyCondition` side-effect against
/// the picked target. The target gate matches the existing ally-targetable
/// pattern — picker / AI route through `is_harmful = false`.
///
/// Adding a new variant is a one-static declaration — no new `Action`
/// impl needed.
pub struct SingleTargetBuffItem {
    /// Player-facing action name (e.g. "read bless scroll").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["bless scroll", "bless"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Full log line emitted on use. The `{actor}` placeholder is
    /// substituted with the caster's name; no other formatting is
    /// performed.
    pub log_text: &'static str,
    /// Condition to install on the picked target.
    pub condition: Condition,
    /// Timer for the install.
    pub timer: ConditionTimer,
    /// Maximum reach in tiles for the targeting picker (1 for touch,
    /// 12 for "close ranged", 24 for "30 ft RAW").
    pub reach: isize,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost.
    pub bonus_action: bool,
    /// If `true`, the validator rejects when the picked target already
    /// has the buff up — prevents the consumable from being wasted on a
    /// no-op timer refresh (since `add_condition` keeps the longer of
    /// the two timers). Mirrors `SelfConditionItem.reject_when_active`
    /// for the ally lane.
    pub reject_when_active: bool,
}

impl Action for SingleTargetBuffItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        // The buff install reads as a support action so the AI's heal
        // / buff pipeline can pick the scroll up alongside genuine heals.
        true
    }

    fn cost(
        &self,
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, self.item_name) {
            return false;
        }
        // Ally-target gate: a buff scroll picked at an enemy is a no-op
        // RAW. Without this, a queued buff on a hostile target would
        // fall through to `side_effects`, consume the scroll, and stamp
        // the friendly condition on the enemy — strictly bad. Mirrors
        // the spell-side gate (`first_ally_target_id` on Bless / Shield
        // of Faith).
        let Some(tid) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return false;
        };
        if self.reject_when_active
            && encounter
                .actors
                .get(&tid)
                .is_some_and(|t| t.has_condition(self.condition))
        {
            return false;
        }
        true
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: self.condition,
            timer: self.timer,
        })]
    }
}

const SCROLL_OF_BLESS_NAME: &str = "Scroll of Bless";
const SCROLL_OF_SHIELD_OF_FAITH_NAME: &str = "Scroll of Shield of Faith";
const SCROLL_OF_BLINDNESS_NAME: &str = "Scroll of Blindness";
const SCROLL_OF_BANE_NAME: &str = "Scroll of Bane";
const SCROLL_OF_FAERIE_FIRE_NAME: &str = "Scroll of Faerie Fire";
const WAND_OF_POLYMORPH_NAME: &str = "Wand of Polymorph";
const POTION_OF_BARKSKIN_NAME: &str = "Potion of Barkskin";
const POTION_OF_FIRE_RESISTANCE_NAME: &str = "Potion of Fire Resistance";
const POTION_OF_COLD_RESISTANCE_NAME: &str = "Potion of Cold Resistance";
const POTION_OF_HILL_GIANT_STRENGTH_NAME: &str = "Potion of Hill Giant Strength";

/// Scroll of Bless — Action; install `Blessed` for 10 rounds on a single
/// ally (+1d4 to attack rolls and saving throws). 5e RAW: level-1
/// concentration enchantment hitting up to 3 creatures; the scroll
/// collapses to a single-target install with the standard fixed-duration
/// timer all consumable buffs ride. Touch range (the engine's "ally
/// support" niche). Fires through the shared `SingleTargetBuffItem`
/// impl.
pub static READ_BLESS_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read bless scroll",
    action_aliases: &["bless scroll", "bless"],
    item_name: SCROLL_OF_BLESS_NAME,
    log_text: "{actor} reads a scroll of bless; a soft golden light settles.",
    condition: Condition::Blessed,
    timer: ConditionTimer::Rounds(10),
    // 30 ft range RAW; 12 tiles in the 2.5ft grid.
    reach: 12,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Shield of Faith — Action; install `ShieldOfFaith` for 10
/// rounds (+2 AC) on a single ally. 5e RAW: level-1 abjuration,
/// concentration, bonus action; the scroll bypasses the concentration
/// gate and the bonus-action cost (drops to Action for the read +
/// targeting envelope). Fires through the shared `SingleTargetBuffItem`
/// impl.
pub static READ_SHIELD_OF_FAITH_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read shield of faith scroll",
    action_aliases: &["sof scroll", "shield faith scroll"],
    item_name: SCROLL_OF_SHIELD_OF_FAITH_NAME,
    log_text: "{actor} reads a scroll of shield of faith; faint motes of shimmering light orbit.",
    condition: Condition::ShieldOfFaith,
    timer: ConditionTimer::Rounds(10),
    // 60 ft range RAW; 24 tiles.
    reach: 24,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Blindness — Action; CON save vs DC 13, fail = Blinded for
/// 10 rounds on a single target. 5e RAW: level-2 necromancy
/// (Blindness/Deafness), CON save, no concentration. Fills the single-
/// target Blinded niche in the loot pool alongside Wand of Paralysis
/// (Paralyzed) / Wand of Fear (Frightened). Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_BLINDNESS_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read blindness scroll",
    action_aliases: &["blindness scroll", "blind"],
    item_name: SCROLL_OF_BLINDNESS_NAME,
    log_text: "{actor} reads a scroll of blindness; the words sear the air.",
    save: AbilityScoreType::Constitution,
    dc: 13,
    // 30 ft range RAW; 12 tiles.
    reach: 12,
    condition: Condition::Blinded,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Bane — Action; 4-tile burst, CHA save vs DC 13, fail =
/// Baned for 10 rounds (-1d4 to attack rolls and saving throws). 5e RAW:
/// level-1 enchantment, concentration, CHA save; the scroll collapses
/// to a burst envelope every other AoE CC consumable rides. Mirror of
/// `READ_BLESS_SCROLL` on the debuff lane — enemies caught in the
/// burst eat the penalty for 10 rounds. Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static READ_BANE_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read bane scroll",
    action_aliases: &["bane scroll", "bane"],
    item_name: SCROLL_OF_BANE_NAME,
    log_text: "{actor} reads a scroll of bane; a creeping shadow seeps over the foes.",
    save: AbilityScoreType::Charisma,
    dc: 13,
    radius: 4,
    // 30 ft range RAW; 12 tiles.
    reach: 12,
    condition: Condition::Baned,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Faerie Fire — Action; 4-tile burst, DEX save vs DC 13, fail
/// = Outlined for 10 rounds (attacks against them have advantage, can't
/// benefit from Hidden / Invisible). 5e RAW: level-1 evocation,
/// concentration, DEX save; the scroll drops the concentration gate.
/// Fills the "burst Outlined" niche alongside the existing single-target
/// outline sources. Fires through the shared `BurstSaveConditionItem`
/// impl.
pub static READ_FAERIE_FIRE_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read faerie fire scroll",
    action_aliases: &["faerie fire", "ff scroll"],
    item_name: SCROLL_OF_FAERIE_FIRE_NAME,
    log_text: "{actor} reads a scroll of faerie fire; motes of pale light tag the foes.",
    save: AbilityScoreType::Dexterity,
    dc: 13,
    radius: 4,
    // 60 ft range RAW; 24 tiles.
    reach: 24,
    condition: Condition::Outlined,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Polymorph — Action; single-target, WIS save vs DC 15, fail =
/// Polymorphed for 10 rounds AND a 30 temp HP buffer (matching the
/// Polymorph spell's beast-form HP pool). 5e RAW: level-4 transmutation,
/// concentration, WIS save; the wand collapses to the standard fixed-
/// duration consumable envelope and drops the concentration gate. Sits
/// in the rare half of the loot pool — Polymorphed locks a target out
/// of their spellcasting (the condition is in `is_dispellable_buff`),
/// and the temp HP buffer means a target who later succeeds on a
/// dispel returns to base HP with the buffer drained.
///
/// Custom impl rather than `SingleSaveConditionItem` because the temp
/// HP grant lives outside the single-condition envelope the shared
/// factor handles.
pub struct UseWandOfPolymorph {}

impl Action for UseWandOfPolymorph {
    fn name(&self) -> &str {
        "use wand of polymorph"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["polymorph", "poly wand"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, WAND_OF_POLYMORPH_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::GainTempHp;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, WAND_OF_POLYMORPH_NAME) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(format!(
            "{} aims the wand of polymorph; the target's form ripples.",
            name
        ));
        // Skip the save against a Polymorphed-immune target — same
        // pattern as `SingleSaveConditionItem`'s up-front filter so the
        // Sorcerer Heightened Spell prime doesn't leak onto a no-op.
        if encounter.actor_immune_to_condition(target_id, Condition::Polymorphed) {
            return Vec::new();
        }
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Wisdom,
            15,
            caster_id,
        );
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: 30,
            }),
        ]
    }
}

pub static USE_WAND_OF_POLYMORPH: UseWandOfPolymorph = UseWandOfPolymorph {};

/// Potion of Barkskin — Bonus Action; installs `Barkskinned` for 10
/// rounds (AC floor of 16). 5e RAW: level-2 transmutation, action cost,
/// concentration, 1-hour duration; the potion drops concentration and
/// uses the bonus-action drink envelope every other defensive
/// consumable rides. Single-use; rejects re-drink when already
/// barkskinned so the consumable isn't burned on a no-op timer refresh.
/// Fires through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_BARKSKIN: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of barkskin",
    action_aliases: &["barkskin", "bark"],
    item_name: POTION_OF_BARKSKIN_NAME,
    log_text: "{actor} drinks a potion of barkskin; their skin hardens to rough bark.",
    condition: Condition::Barkskinned,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Fire Resistance — Action; installs `DamageResistant` (halve
/// all incoming damage) for 10 rounds. 5e RAW grants resistance to a
/// single damage type; the engine's `DamageResistant` condition is type-
/// blanket so the potion delivers broader value than RAW. Single-use;
/// rejects re-drink when already resistant. Fires through the shared
/// `SelfConditionItem` impl. (We don't model per-type buffs as
/// conditions yet — this is the closest envelope.)
pub static DRINK_POTION_OF_FIRE_RESISTANCE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of fire resistance",
    action_aliases: &["fire potion", "fire res"],
    item_name: POTION_OF_FIRE_RESISTANCE_NAME,
    log_text: "{actor} drinks a potion of fire resistance; a cooling shimmer wraps them.",
    condition: Condition::DamageResistant,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Cold Resistance — Action; installs `DamageResistant` for 10
/// rounds. Sibling to Potion of Fire Resistance — same envelope, distinct
/// flavor. Both potions share the `DamageResistant` lane (blanket damage
/// halve) since the engine doesn't yet model per-type buffs as separate
/// conditions; the loot pool just gets two flavored entries instead of
/// one. Single-use; rejects re-drink when already resistant.
pub static DRINK_POTION_OF_COLD_RESISTANCE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of cold resistance",
    action_aliases: &["cold potion", "cold res"],
    item_name: POTION_OF_COLD_RESISTANCE_NAME,
    log_text: "{actor} drinks a potion of cold resistance; a warm glow sinks into their skin.",
    condition: Condition::DamageResistant,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Hill Giant Strength — Action; installs `Enlarged` for 10
/// rounds (+1d4 weapon damage rider, size bump). 5e RAW: STR becomes 21
/// for 1 hour; the engine doesn't overwrite ability scores, so we route
/// through the Enlarged envelope every other "size up" consumable
/// (Potion of Growth) uses — same flavor, slightly different in-fiction
/// trigger. Single-use; rejects re-drink when already enlarged. Fires
/// through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_HILL_GIANT_STRENGTH: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of hill giant strength",
    action_aliases: &["giant strength", "giant str"],
    item_name: POTION_OF_HILL_GIANT_STRENGTH_NAME,
    log_text: "{actor} drinks a potion of hill giant strength; their frame swells.",
    condition: Condition::Enlarged,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

const WAND_OF_SLEEP_NAME: &str = "Wand of Sleep";
const SCROLL_OF_SLOW_NAME: &str = "Scroll of Slow";
const SCROLL_OF_STINKING_CLOUD_NAME: &str = "Scroll of Stinking Cloud";
const SCROLL_OF_DEATH_WARD_NAME: &str = "Scroll of Death Ward";
const SCROLL_OF_AID_NAME: &str = "Scroll of Aid";

/// Wand of Sleep — Action; single-target, WIS save vs DC 13, fail =
/// `Asleep` for 10 rounds. Single-use consumable. 5e RAW: level-1
/// enchantment with an HP-pool gate (5d8 HP of creatures fall asleep
/// lowest-first); the wand variant collapses to a per-target save
/// envelope every other CC consumable rides. Pairs with Scroll of Hold
/// Person at the entry-level CC tier — Sleep is the "wake-on-damage"
/// counterpart to Hold Person's "duration-bound paralysis," letting a
/// martial follow-up burn the lock with a single swing. Fires through
/// the shared `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_SLEEP: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of sleep",
    action_aliases: &["sleep", "sleep wand"],
    item_name: WAND_OF_SLEEP_NAME,
    log_text: "{actor} waves the wand of sleep; soft motes of sand drift.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    // 90 ft RAW; 36 tiles. Keep at 24 (60 ft) for the engine's typical
    // wand reach cap — long enough for any plausible CC opener.
    reach: 24,
    condition: Condition::Asleep,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Slow — Action; 4-tile burst, WIS save vs DC 13, fail =
/// `Slowed` for 10 rounds (halved speed, -2 AC, -2 DEX saves). 5e RAW:
/// level-3 transmutation, WIS save, concentration, 40-ft cube; the
/// scroll collapses to the standard fixed-duration burst envelope and
/// drops the concentration gate. Mirrors Scroll of Bane / Faerie Fire on
/// the burst-debuff lane — Slowed is the AC/movement counterpart to
/// Bane's roll penalties. Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static READ_SLOW_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read slow scroll",
    action_aliases: &["slow", "slow scroll"],
    item_name: SCROLL_OF_SLOW_NAME,
    log_text: "{actor} reads a scroll of slow; the air around the foes thickens.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    radius: 4,
    // 120 ft RAW; 48 tiles. 24 (60 ft) matches the engine's typical
    // burst-CC scroll reach.
    reach: 24,
    condition: Condition::Slowed,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Stinking Cloud — Action; 4-tile burst, CON save vs DC 15,
/// fail = `Poisoned` for 10 rounds (disadvantage on attacks / ability
/// checks). 5e RAW: level-3 conjuration, CON save, concentration, 20-ft
/// radius; the scroll collapses to the standard fixed-duration burst
/// envelope and drops the concentration gate. Fills the burst-Poisoned
/// niche in the loot pool alongside Pipes of Haunting (burst Frightened)
/// and Wand of Web (burst Restrained). Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static READ_STINKING_CLOUD_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read stinking cloud scroll",
    action_aliases: &["stinking", "stink"],
    item_name: SCROLL_OF_STINKING_CLOUD_NAME,
    log_text: "{actor} reads a scroll of stinking cloud; a sickly yellow fog billows.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 4,
    // 90 ft RAW; 36 tiles. 24 (60 ft) matches the engine's typical
    // burst-CC scroll reach.
    reach: 24,
    condition: Condition::Poisoned,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Death Ward — Action; install `DeathWarded` on a single
/// ally for 100 rounds. 5e RAW: level-4 abjuration, action, touch,
/// 8-hour duration; the scroll collapses to the engine's standard fixed-
/// duration buff envelope and the touch reach (1 tile). The ward
/// intercepts the next lethal hit (damage that would drop the target to
/// 0 HP leaves them at 1) and then burns off — handled by the existing
/// `DeathWarded` lane in `take_damage`. Fires through the shared
/// `SingleTargetBuffItem` impl.
pub static READ_DEATH_WARD_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read death ward scroll",
    action_aliases: &["death ward", "dw scroll"],
    item_name: SCROLL_OF_DEATH_WARD_NAME,
    log_text: "{actor} reads a scroll of death ward; a faint silver aura settles.",
    condition: Condition::DeathWarded,
    // RAW: 8-hour duration. 100 rounds = ~10 minutes engine time —
    // plenty to span any encounter, short enough to drop between long
    // rests cleanly.
    timer: ConditionTimer::Rounds(100),
    reach: 1,
    bonus_action: false,
    reject_when_active: true,
};

/// Aid — multi-ally permanent +5 max-HP / current-HP buff. RAW affects
/// up to 3 creatures within 30 ft; the scroll uses a self-centered
/// `NoArgs` schema (mirroring `Scroll of Mass Healing Word`) and picks
/// the 3 lowest-HP-percent allies inside range automatically. Keeping
/// the targeting implicit lets the AI's `try_support_heal` heuristic
/// reach for it without a burst-aware picker — same shape as the
/// existing multi-ally heal scroll.
///
/// Distinct from `SingleTargetBuffItem` / `SingleTargetHealItem` because
/// the effect is a permanent base-HP bump (`bump_max_hp`) rather than a
/// condition install or a Heal side-effect — neither shared template
/// covers the lane.
pub struct ReadAidScroll {}

impl Action for ReadAidScroll {
    fn name(&self) -> &str {
        "read aid scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["aid scroll", "aid"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, SCROLL_OF_AID_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_AID_NAME) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(format!(
            "{} reads a scroll of aid; warm light pulses across allies.",
            name
        ));
        // RAW Aid: up to 3 ally targets within 30 ft. Pick the lowest-HP-
        // percent allies first so the buff lands where it matters (a near-
        // dead front-liner needs the +5 cushion more than the at-full
        // mage). 12 tiles = 30 ft on the 2.5 ft grid.
        const RANGE_TILES: isize = 12;
        const MAX_TARGETS: usize = 3;
        // Allow dying allies — RAW Aid: "the target's hit point maximum
        // and current hit points increase by 5." A dying ally at 0 HP
        // gets bumped to 5 HP and is back in the fight (the `bump_max_hp`
        // helper raises current by the same delta). Mirrors the spell-
        // side `Aid` impl which doesn't filter dying targets.
        // Allies already carrying Aid are skipped rather than counted —
        // PHB: "the effects of the same spell cast multiple times don't
        // combine" — so a scroll read into a party that is already
        // buffed spends itself on whoever is not, instead of stacking a
        // second five onto the three lowest.
        let mut candidates: Vec<(u64, usize)> = encounter
            .ally_candidates_in_range(caster_id, RANGE_TILES)
            .into_iter()
            .filter_map(|(id, _dist)| {
                let a = encounter.actors.get(&id)?;
                if a.has_condition(Condition::Aided) {
                    return None;
                }
                let max = a.max_hitpoints().max(1) as u64;
                let hp_pct = (a.hitpoints() as u64 * 1000) / max;
                Some((hp_pct, id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(MAX_TARGETS);
        for (_, tid) in &candidates {
            crate::actions::spells::apply_aid(encounter, *tid);
        }
        encounter.log(format!(
            "  aid: +5 max HP / +5 HP on {} ally{}",
            candidates.len(),
            if candidates.len() == 1 { "" } else { "(ies)" }
        ));
        Vec::new()
    }
}

pub static READ_AID_SCROLL: ReadAidScroll = ReadAidScroll {};

const WAND_OF_BINDING_NAME: &str = "Wand of Binding";
const SCROLL_OF_BANISHMENT_NAME: &str = "Scroll of Banishment";
const SCROLL_OF_FEAR_NAME: &str = "Scroll of Fear";

/// Wand of Binding — Action; single-target DEX save vs DC 15, fail =
/// `Restrained` for 10 rounds. Single-use consumable. Single-target
/// counterpart to the Wand of Web (burst Restrained); the wand trades
/// area coverage for a more targeted lock. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_BINDING: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of binding",
    action_aliases: &["binding", "bind"],
    item_name: WAND_OF_BINDING_NAME,
    log_text: "{actor} aims the wand of binding; iron-rune bands lash out.",
    save: AbilityScoreType::Dexterity,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Restrained,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Banishment — Action; single-target CHA save vs DC 15,
/// fail = `Mazed` for 10 rounds (banished-demiplane envelope; the
/// engine's `Mazed` condition is the load-bearing inert tag). 5e RAW:
/// level-4 abjuration, concentration; the scroll drops concentration
/// and uses a fixed 10-round timer. Top-of-pool single-target CC
/// consumable alongside Wand of Polymorph — both effectively remove the
/// target. Fires through the shared `SingleSaveConditionItem` impl.
pub static READ_BANISHMENT_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read banishment scroll",
    action_aliases: &["banishment", "banish"],
    item_name: SCROLL_OF_BANISHMENT_NAME,
    log_text: "{actor} reads a scroll of banishment; the target shimmers and fades.",
    save: AbilityScoreType::Charisma,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Mazed,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Fear — Action; 4-tile burst, WIS save vs DC 15, fail =
/// `Frightened` for 10 rounds. 5e RAW: level-3 illusion, concentration,
/// 30-ft cone; the scroll drops concentration and uses a burst envelope.
/// Harder DC counterpart to Pipes of Haunting (burst DC 13 Frightened);
/// sits alongside Wand of Fear (single-target DC 15 Frightened) so the
/// loot pool covers all three combinations of (burst/single, soft/hard
/// DC) on the Frightened lane. Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static READ_FEAR_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read fear scroll",
    action_aliases: &["fear scroll", "fear burst"],
    item_name: SCROLL_OF_FEAR_NAME,
    log_text: "{actor} reads a scroll of fear; shadows leap from the parchment.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    radius: 4,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Frightened,
    timer: ConditionTimer::Rounds(10),
};

const SCROLL_OF_CHARM_PERSON_NAME: &str = "Scroll of Charm Person";
const WAND_OF_CHARM_MONSTER_NAME: &str = "Wand of Charm Monster";
const SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER_NAME: &str = "Scroll of Tasha's Hideous Laughter";
const SCROLL_OF_HEAT_METAL_NAME: &str = "Scroll of Heat Metal";
const SCROLL_OF_ICE_STORM_NAME: &str = "Scroll of Ice Storm";
const SCROLL_OF_WEB_NAME: &str = "Scroll of Web";
const POTION_OF_RESISTANCE_NAME: &str = "Potion of Resistance";
const POTION_OF_VIGILANCE_NAME: &str = "Potion of Vigilance";

/// Scroll of Charm Person — Action; single-target WIS save vs DC 13, fail =
/// `Charmed` for 10 rounds. 5e RAW: level-1 enchantment, 30-ft range, 1-hour
/// duration; the scroll uses the engine's standard 10-round CC envelope.
/// Charm-immune families (undead / constructs / fiends) silently skip the
/// save through the up-front immunity filter in `SingleSaveConditionItem`.
/// Fires through the shared `SingleSaveConditionItem` impl.
pub static READ_CHARM_PERSON_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read charm person scroll",
    action_aliases: &["charm", "charm person"],
    item_name: SCROLL_OF_CHARM_PERSON_NAME,
    log_text: "{actor} reads a scroll of charm person; honeyed words weave through the air.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Charmed,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Charm Monster — Action; single-target WIS save vs DC 15, fail =
/// `Charmed` for 10 rounds. 5e RAW: level-4 enchantment, 60-ft range, 1-hour
/// duration. Top-tier enchantment consumable: harder DC and longer reach
/// than `READ_CHARM_PERSON_SCROLL`. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_CHARM_MONSTER: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of charm monster",
    action_aliases: &["charm monster", "charm+"],
    item_name: WAND_OF_CHARM_MONSTER_NAME,
    log_text: "{actor} aims the wand of charm monster; the air shimmers with persuasive light.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Charmed,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Tasha's Hideous Laughter — Action; single-target WIS save vs
/// DC 13, fail = `Incapacitated` for 10 rounds. 5e RAW: level-1 enchantment,
/// 30-ft range, concentration; the scroll drops concentration and uses the
/// engine's standard fixed-duration envelope. Entry-tier single-target
/// Incapacitated installer. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_TASHAS_HIDEOUS_LAUGHTER_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read tasha's hideous laughter scroll",
    action_aliases: &["laughter", "tasha"],
    item_name: SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER_NAME,
    log_text: "{actor} reads a scroll of hideous laughter; the target chokes on uncontrollable mirth.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Incapacitated,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Heat Metal — Action; single-target CON save vs DC 13, fail =
/// `HeatMetaled` for 10 rounds (attack-roll / ability-check disadvantage).
/// 5e RAW: level-2 transmutation, no save on cast plus 2d8 fire per round
/// while concentration holds; we collapse the spell's load-bearing combat
/// clause (attack disadvantage) to a single CON save and drop the
/// damage-per-round rider. The condition flows through
/// `compute_attack_mode`'s disadvantage clause. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_HEAT_METAL_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read heat metal scroll",
    action_aliases: &["heat metal", "heat"],
    item_name: SCROLL_OF_HEAT_METAL_NAME,
    log_text: "{actor} reads a scroll of heat metal; the target's gear glows red-hot.",
    save: AbilityScoreType::Constitution,
    dc: 13,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::HeatMetaled,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Ice Storm — Action; 4-tile burst, DEX save vs DC 15, fail =
/// 4d8 cold, pass = half. 5e RAW: level-4 evocation, 2d8 bludgeoning + 4d6
/// cold; the scroll collapses the dual-type damage to a single cold roll
/// (4d8). Fires through the shared `BurstSaveDamageItem` impl.
pub static READ_ICE_STORM_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read ice storm scroll",
    action_aliases: &["ice storm", "ice"],
    item_name: SCROLL_OF_ICE_STORM_NAME,
    log_label: "scroll of ice storm",
    dice: Dice::new(4, 8),
    damage_type: DamageType::Cold,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    // 300 ft RAW; we cap to a map-realistic 48 tiles (120 ft).
    reach: 48,
};

/// Scroll of Web — Action; 4-tile burst, DEX save vs DC 13, fail =
/// `Restrained` for 10 rounds. Cheap-tier counterpart to Wand of Web (DC 15
/// burst Restrained) — same shape, easier DC. Fires through the shared
/// `BurstSaveConditionItem` impl.
pub static READ_WEB_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read web scroll",
    action_aliases: &["web scroll", "web burst"],
    item_name: SCROLL_OF_WEB_NAME,
    log_text: "{actor} reads a scroll of web; sticky strands erupt across the ground.",
    save: AbilityScoreType::Dexterity,
    dc: 13,
    radius: 4,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Restrained,
    timer: ConditionTimer::Rounds(10),
};

/// Potion of Resistance — Action; installs `DamageResistant` for 10 rounds.
/// Un-flavored generic counterpart to Potion of Fire / Cold Resistance —
/// same envelope, no flavor tied to a specific damage type. Single-use;
/// rejects re-drink when already resistant. Fires through the shared
/// `SelfConditionItem` impl.
pub static DRINK_POTION_OF_RESISTANCE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of resistance",
    action_aliases: &["resistance", "resist"],
    item_name: POTION_OF_RESISTANCE_NAME,
    log_text: "{actor} drinks a potion of resistance; a translucent shimmer wraps them.",
    condition: Condition::DamageResistant,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Vigilance — Bonus Action; installs `DangerSense` for 10 rounds
/// (advantage on DEX saves while not Blinded / Incapacitated / Deafened).
/// Consumable counterpart to the passive `AMULET_OF_THE_VIGILANT` trinket.
/// Single-use; rejects re-drink when already active. Fires through the
/// shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_VIGILANCE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of vigilance",
    action_aliases: &["vigilance", "vigil"],
    item_name: POTION_OF_VIGILANCE_NAME,
    log_text: "{actor} drinks a potion of vigilance; their senses sharpen to a knife's edge.",
    condition: Condition::DangerSense,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

const NECKLACE_OF_FIREBALLS_NAME: &str = "Necklace of Fireballs";
const DUST_OF_DISAPPEARANCE_NAME: &str = "Dust of Disappearance";
const WAND_OF_SUGGESTION_NAME: &str = "Wand of Suggestion";
const SCROLL_OF_CALM_EMOTIONS_NAME: &str = "Scroll of Calm Emotions";
const WAND_OF_BLINDNESS_NAME: &str = "Wand of Blindness";
const SCROLL_OF_MASS_CURE_WOUNDS_NAME: &str = "Scroll of Mass Cure Wounds";
const RING_OF_SPELL_STORING_NAME: &str = "Ring of Spell Storing";

/// Necklace of Fireballs — 5d6 fire DEX-save burst (DC 15, 4 radius).
/// Single-bead consumable; the RAW multi-bead ladder collapses to a
/// single-use scroll-style envelope so the loot pool stays simple. Sits
/// between Scroll of Fireball (6d6) and Wand of Fireballs (8d6) on the
/// fire-burst payload ladder. Fires through the shared
/// `BurstSaveDamageItem` impl.
pub static USE_NECKLACE_OF_FIREBALLS: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "use necklace of fireballs",
    action_aliases: &["necklace", "bead"],
    item_name: NECKLACE_OF_FIREBALLS_NAME,
    log_label: "necklace of fireballs (bead)",
    dice: Dice::new(5, 6),
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 4,
    // 60 ft RAW; 24 tiles.
    reach: 24,
};

/// Dust of Disappearance — Bonus Action; installs `Invisible` on the
/// holder for 10 rounds. Quickened counterpart to Potion of Invisibility
/// (Action cost). Fires through the shared `SelfConditionItem` impl.
pub static USE_DUST_OF_DISAPPEARANCE: SelfConditionItem = SelfConditionItem {
    action_name: "use dust of disappearance",
    action_aliases: &["dust", "disappear"],
    item_name: DUST_OF_DISAPPEARANCE_NAME,
    log_text: "{actor} sprinkles dust of disappearance; their form fades.",
    condition: Condition::Invisible,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Wand of Suggestion — Action; single-target WIS save vs DC 15, fail =
/// `Charmed` for 10 rounds. 5e RAW: level-2 enchantment, concentration;
/// the wand drops the concentration gate and uses the standard
/// fixed-duration consumable envelope. Sibling to Wand of Charm Monster
/// on the Charmed lane — same DC, slightly shorter reach (24 tiles vs
/// 30 ft RAW), distinct flavor (suggestion vs raw charm). Fires through
/// the shared `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_SUGGESTION: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of suggestion",
    action_aliases: &["suggestion", "suggest"],
    item_name: WAND_OF_SUGGESTION_NAME,
    log_text: "{actor} aims the wand of suggestion; a honeyed whisper threads through the air.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Charmed,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Calm Emotions — Action; 4-tile burst, CHA save vs DC 13,
/// fail = `Charmed` for 10 rounds. 5e RAW: level-2 enchantment,
/// concentration, two-option toggle (suppress fear OR Charm). The
/// scroll collapses to the Charm-installer half (the engine-relevant
/// combat clause) and drops the concentration gate. Burst counterpart
/// to Scroll of Charm Person (single-target, same DC 13) on the
/// Charmed lane. Fires through the shared `BurstSaveConditionItem`
/// impl.
pub static READ_CALM_EMOTIONS_SCROLL: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "read calm emotions scroll",
    action_aliases: &["calm", "calm emotions"],
    item_name: SCROLL_OF_CALM_EMOTIONS_NAME,
    log_text: "{actor} reads a scroll of calm emotions; a soothing wave washes over the foes.",
    save: AbilityScoreType::Charisma,
    dc: 13,
    radius: 4,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Charmed,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Blindness — Action; single-target CON save vs DC 15, fail =
/// `Blinded` for 10 rounds. Top-tier counterpart to Scroll of Blindness
/// (CON save DC 13, 12 reach). Same condition, harder DC and longer
/// reach. Fires through the shared `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_BLINDNESS: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of blindness",
    action_aliases: &["blindness wand", "blind+"],
    item_name: WAND_OF_BLINDNESS_NAME,
    log_text: "{actor} aims the wand of blindness; harsh light sears the target's eyes.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Blinded,
    timer: ConditionTimer::Rounds(10),
};

/// Ring of Spell Storing — single-use Magic-Missile-style force-dart
/// volley (3 darts × 1d4+1 force, auto-hit, no save). RAW: the ring
/// stores up to 5 levels worth of spells the wearer can release. We
/// collapse the storage subsystem to a fixed force-dart payload — same
/// envelope as the Scroll of Magic Missile so the loot pool has a
/// trinket-flavored variant on the auto-hit lane. Fires through the
/// shared `MagicMissileItem` impl.
pub static USE_RING_OF_SPELL_STORING: MagicMissileItem = MagicMissileItem {
    action_name: "use ring of spell storing",
    action_aliases: &["spell storing", "rss"],
    item_name: RING_OF_SPELL_STORING_NAME,
    log_label: "ring of spell storing (stored magic missile)",
    darts: 3,
    // 30 tile reach matches the Scroll of Magic Missile (150 ft RAW).
    reach: 30,
};

/// Scroll of Mass Cure Wounds — Action; self-centered 4-tile burst that
/// heals up to 6 closest allies for 3d8+5 HP each. RAW: 3d8 + caster
/// mod per ally at level 5. The +5 stand-in matches a typical mid-level
/// cleric's WIS mod. Sits one tier above Mass Healing Word (bonus action,
/// 1d4+3) — the Action cost trades for bigger per-ally pool. Fires
/// through the shared `MultiTargetHealItem` impl.
pub static READ_MASS_CURE_WOUNDS_SCROLL: MultiTargetHealItem = MultiTargetHealItem {
    action_name: "read mass cure wounds scroll",
    action_aliases: &["mcw", "mass cure"],
    item_name: SCROLL_OF_MASS_CURE_WOUNDS_NAME,
    log_label: "scroll of mass cure wounds",
    dice: Dice::new(3, 8),
    flat_bonus: 5,
    bonus_action: false,
    // 30 ft RAW. Tighter envelope than Mass Healing Word (24 tiles)
    // because the Action cost reads as a more focused front-line heal.
    range_tiles: 4,
    max_targets: 6,
};

const SCROLL_OF_CLOUDKILL_NAME: &str = "Scroll of Cloudkill";
const SCROLL_OF_PRAYER_OF_HEALING_NAME: &str = "Scroll of Prayer of Healing";
const SCROLL_OF_GREATER_CURE_WOUNDS_NAME: &str = "Scroll of Greater Cure Wounds";
const POTION_OF_HASTE_NAME: &str = "Potion of Haste";
const SCROLL_OF_FLESH_TO_STONE_NAME: &str = "Scroll of Flesh to Stone";
const SCROLL_OF_SYNAPTIC_STATIC_NAME: &str = "Scroll of Synaptic Static";
const SCROLL_OF_CIRCLE_OF_DEATH_NAME: &str = "Scroll of Circle of Death";
const POTION_OF_MIND_BLANK_NAME: &str = "Potion of Mind Blank";

/// Scroll of Cloudkill — 5d8 poison-damage burst at DC 15 CON save.
/// The only poison-damage burst consumable in the loot pool; sits
/// alongside Stinking Cloud's burst-Poisoned condition variant.
/// Fires through the shared `BurstSaveDamageItem` impl.
pub static READ_CLOUDKILL_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read cloudkill scroll",
    action_aliases: &["cloudkill", "cloud"],
    item_name: SCROLL_OF_CLOUDKILL_NAME,
    log_label: "scroll of cloudkill",
    dice: Dice::new(5, 8),
    damage_type: DamageType::Poison,
    save: AbilityScoreType::Constitution,
    dc: 15,
    radius: 4,
    // 120 ft RAW; 48 tiles. Capped to map-realistic 48.
    reach: 48,
};

/// Scroll of Prayer of Healing — 2d8+3 per-ally heal, up to 6 closest
/// allies within 6 tiles. Mid-tier between Mass Healing Word (1d4+3
/// bonus action, 24 tiles) and Mass Cure Wounds (3d8+5 action, 4 tiles
/// from self) on the multi-target heal ladder. Fires through the shared
/// `MultiTargetHealItem` impl.
pub static READ_PRAYER_OF_HEALING_SCROLL: MultiTargetHealItem = MultiTargetHealItem {
    action_name: "read prayer of healing scroll",
    action_aliases: &["prayer", "poh"],
    item_name: SCROLL_OF_PRAYER_OF_HEALING_NAME,
    log_label: "scroll of prayer of healing",
    dice: Dice::new(2, 8),
    flat_bonus: 3,
    bonus_action: false,
    // 30 ft range RAW; tighter envelope than Mass Healing Word at 60 ft.
    range_tiles: 6,
    max_targets: 6,
};

/// Scroll of Greater Cure Wounds — 4d8+5 single-target touch heal.
/// Slots between Cure Wounds scroll (2d8+2) and Wand of Greater Healing
/// (4d8+4) on the ally-heal ladder. Fires through the shared
/// `SingleTargetHealItem` impl.
pub static READ_GREATER_CURE_WOUNDS_SCROLL: SingleTargetHealItem = SingleTargetHealItem {
    action_name: "read greater cure wounds scroll",
    action_aliases: &["gcw", "cure+"],
    item_name: SCROLL_OF_GREATER_CURE_WOUNDS_NAME,
    log_label: "scroll of greater cure wounds",
    dice: Dice::new(4, 8),
    flat_bonus: 5,
    bonus_action: false,
    // Touch range RAW — 1 tile gap.
    reach: 1,
};

/// Potion of Haste — Bonus Action; installs `Hasted` on the holder for
/// 10 rounds (+2 AC, advantage on DEX saves, doubled walking speed).
/// Distinct from Potion of Speed (extra-action burst) — Haste rides the
/// engine's existing `Hasted` condition for the AC/DEX/speed bundle.
/// Fires through the shared `SelfConditionItem` impl. Rejects re-drink
/// when already Hasted.
pub static DRINK_POTION_OF_HASTE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of haste",
    action_aliases: &["haste potion", "hasten"],
    item_name: POTION_OF_HASTE_NAME,
    log_text: "{actor} drinks a potion of haste; their movements blur to a streak.",
    condition: Condition::Hasted,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Scroll of Flesh to Stone — Action; single-target CON save vs DC 15,
/// fail = `Petrified` for 10 rounds. Top of the single-target lockdown
/// ladder alongside Wand of Polymorph. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_FLESH_TO_STONE_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read flesh to stone scroll",
    action_aliases: &["flesh to stone", "fts"],
    item_name: SCROLL_OF_FLESH_TO_STONE_NAME,
    log_text: "{actor} reads a scroll of flesh to stone; the target's skin pales to gray.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Petrified,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Synaptic Static — Action; 4-tile burst, INT save vs DC 15,
/// fail = 8d6 psychic damage; pass = half. 5e RAW: level-5 enchantment,
/// 20-ft radius (4 tiles), 120-ft range (48 tiles). Fills the psychic
/// burst-damage niche in the scroll family alongside Fire / Lightning /
/// Cold / Acid / Thunder / Poison. Routes through the shared
/// `BurstSaveDamageItem` impl — evasion / Careful Spell / Heightened
/// Spell all flow through the same chokepoint as every other AoE save.
pub static READ_SYNAPTIC_STATIC_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read synaptic static scroll",
    action_aliases: &["synaptic", "static"],
    item_name: SCROLL_OF_SYNAPTIC_STATIC_NAME,
    log_label: "scroll of synaptic static",
    dice: Dice::new(8, 6),
    damage_type: DamageType::Psychic,
    save: AbilityScoreType::Intelligence,
    dc: 15,
    radius: 4,
    // 120 ft RAW; 48 tiles.
    reach: 48,
};

/// Scroll of Circle of Death — Action; 6-tile burst, CON save vs DC 15,
/// fail = 8d6 necrotic damage; pass = half. 5e RAW: level-6 necromancy,
/// 60-ft radius (we tighten to 6 tiles so the burst stays on-grid for the
/// typical encounter map), 150-ft range. Fills the necrotic burst-damage
/// niche in the scroll family — the only necrotic-typed burst consumable
/// in the loot pool. Routes through the shared `BurstSaveDamageItem` impl.
pub static READ_CIRCLE_OF_DEATH_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read circle of death scroll",
    action_aliases: &["circle of death", "death circle"],
    item_name: SCROLL_OF_CIRCLE_OF_DEATH_NAME,
    log_label: "scroll of circle of death",
    dice: Dice::new(8, 6),
    damage_type: DamageType::Necrotic,
    save: AbilityScoreType::Constitution,
    dc: 15,
    // RAW 60 ft radius — collapsed to 6 tiles so the burst fits the
    // grid envelope every other scroll rides (4-6 tile radius range).
    radius: 6,
    // 150 ft RAW; 48 tiles (engine cap).
    reach: 48,
};

/// Potion of Mind Blank — Action; installs `MindBlanked` on the drinker
/// for 10 rounds (immunity to psychic damage and the Charmed condition).
/// 5e RAW: level-8 abjuration, 24-hour duration; the potion collapses
/// to a combat-scale fixed-duration buff. Top-tier mental-defense
/// consumable — pairs with Periapt of Proof against Poison (poison
/// immunity) on the typed-immunity consumable lane. Fires through the
/// shared `SelfConditionItem` impl. Rejects re-drink when already up.
pub static DRINK_POTION_OF_MIND_BLANK: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of mind blank",
    action_aliases: &["mind blank", "mb potion"],
    item_name: POTION_OF_MIND_BLANK_NAME,
    log_text: "{actor} drinks a potion of mind blank; their thoughts dim to a silent grey.",
    condition: Condition::MindBlanked,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

const SCROLL_OF_DISINTEGRATE_NAME: &str = "Scroll of Disintegrate";
const SCROLL_OF_FINGER_OF_DEATH_NAME: &str = "Scroll of Finger of Death";
const WAND_OF_HOLD_MONSTER_NAME: &str = "Wand of Hold Monster";
const POTION_OF_FORESIGHT_NAME: &str = "Potion of Foresight";
const NECKLACE_OF_PRAYER_BEADS_NAME: &str = "Necklace of Prayer Beads";
const SCROLL_OF_HEAL_NAME: &str = "Scroll of Heal";
const SCROLL_OF_PHANTASMAL_KILLER_NAME: &str = "Scroll of Phantasmal Killer";
const POTION_OF_MIRROR_IMAGE_NAME: &str = "Potion of Mirror Image";

/// Scroll of Disintegrate — Action; 24-tile reach, DEX save vs DC 15.
/// On fail, 10d6+40 force damage; on save, nothing (no half). Force
/// damage is rarely resisted in the engine's pool, so a landed hit
/// reads as full payload. Top of the single-target burst-damage scroll
/// ladder — sits alongside the burst Scrolls of Synaptic Static / Circle
/// of Death on the rare typed-burst lane. Fires through the shared
/// `SingleSaveDamageItem` impl.
pub static READ_DISINTEGRATE_SCROLL: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "read disintegrate scroll",
    action_aliases: &["disintegrate", "disint scroll"],
    item_name: SCROLL_OF_DISINTEGRATE_NAME,
    log_label: "scroll of disintegrate",
    dice: Dice::new(10, 6),
    flat_bonus: 40,
    damage_type: DamageType::Force,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    reach: 24,
    save_for_half: false,
};

/// Scroll of Finger of Death — Action; 24-tile reach, CON save vs DC 15.
/// On fail, 7d8+30 necrotic damage; on save, half. RAW: level-7
/// necromancy with a "raise-as-zombie" rider — the scroll drops the
/// raise clause and surfaces the damage half. Sibling to Scroll of
/// Disintegrate on the rare single-target damage-scroll lane (Force vs
/// Necrotic; no-save-half vs save-half). Fires through the shared
/// `SingleSaveDamageItem` impl.
pub static READ_FINGER_OF_DEATH_SCROLL: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "read finger of death scroll",
    action_aliases: &["finger of death", "fod scroll"],
    item_name: SCROLL_OF_FINGER_OF_DEATH_NAME,
    log_label: "scroll of finger of death",
    dice: Dice::new(7, 8),
    flat_bonus: 30,
    damage_type: DamageType::Necrotic,
    save: AbilityScoreType::Constitution,
    dc: 15,
    reach: 24,
    save_for_half: true,
};

/// Wand of Hold Monster — Action; single-target WIS save vs DC 17, fail =
/// `Paralyzed` for 10 rounds. Top tier of the Hold-Paralyzed ladder
/// (Scroll of Hold Person at DC 13, Wand of Paralysis at DC 15,
/// Scroll of Hold Monster at DC 15 / 36 reach, Wand of Hold Monster at
/// DC 17 / 36 reach). 5e RAW: level-5 enchantment, concentration; the
/// wand collapses to a single-use cast with the standard
/// fixed-duration consumable envelope. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static USE_WAND_OF_HOLD_MONSTER: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of hold monster",
    action_aliases: &["hold monster wand", "hm wand"],
    item_name: WAND_OF_HOLD_MONSTER_NAME,
    log_text: "{actor} aims the wand of hold monster; iron sigils freeze the target's limbs.",
    save: AbilityScoreType::Wisdom,
    dc: 17,
    // 90 ft RAW; 36 tiles.
    reach: 36,
    condition: Condition::Paralyzed,
    timer: ConditionTimer::Rounds(10),
};

/// Potion of Foresight — Action; installs `Foreseen` for 10 rounds.
/// 5e RAW: level-9 divination spell, 8-hour concentration; the potion
/// collapses to a combat-scale fixed-duration self-buff and bypasses
/// concentration. Foreseen grants advantage on every attack roll, save,
/// and ability check while attackers against the holder roll with
/// disadvantage — the mightiest single-target buff in the SRD,
/// collapsed to a one-shot consumable for the rare top-tier defensive
/// lane. Rejects re-drink while already up. Fires through the shared
/// `SelfConditionItem` impl.
pub static DRINK_POTION_OF_FORESIGHT: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of foresight",
    action_aliases: &["foresight", "foresight potion"],
    item_name: POTION_OF_FORESIGHT_NAME,
    log_text: "{actor} drinks a potion of foresight; threads of fate spool out before them.",
    condition: Condition::Foreseen,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Necklace of Prayer Beads — Bonus Action; installs `Blessed` on a
/// touching ally for 10 rounds. 5e RAW (DMG): a strand of 24 to 30
/// beads, each storing one cleric spell; we collapse to a single-bead
/// consumable that fires the Bless spell at the bearer's chosen ally.
/// Sibling to Scroll of Bless on the Blessed lane — the necklace is the
/// trinket-flavored bonus-action variant. Touch range (1 tile) matches
/// the strand-of-beads / lay-hands flavor. Fires through the shared
/// `SingleTargetBuffItem` impl.
pub static USE_NECKLACE_OF_PRAYER_BEADS: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "use necklace of prayer beads",
    action_aliases: &["prayer beads", "beads"],
    item_name: NECKLACE_OF_PRAYER_BEADS_NAME,
    log_text: "{actor} pulls a single bead from the necklace of prayer beads; a halo of warm light settles.",
    condition: Condition::Blessed,
    timer: ConditionTimer::Rounds(10),
    reach: 1,
    bonus_action: true,
    reject_when_active: true,
};

/// Scroll of Heal — Action; touch-range single-target heal for a flat
/// 70 HP. 5e RAW: level-6 evocation, 70 HP heal + cures Blinded /
/// Deafened / Diseased on the target. The scroll collapses to the
/// raw-HP-heal half (the cure-condition clauses are flavor-only at the
/// engine's current granularity). Sits at the top of the single-target
/// ally heal ladder above Wand of Greater Healing (4d8+4). Custom
/// `Action` impl rather than the shared `SingleTargetHealItem` because
/// the heal is a flat number (no dice) — feeding 0d1+70 through the
/// shared factor would print "0d1(0)+70 = 70 HP" which is ugly.
pub struct ReadHealScroll {}

impl Action for ReadHealScroll {
    fn name(&self) -> &str {
        "read heal scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["heal scroll", "scroll of heal"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Touch range = 1 tile.
        Some(1)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, SCROLL_OF_HEAL_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_HEAL_NAME) {
            return Vec::new();
        }
        encounter.log("  scroll of heal: 70 HP".to_string());
        vec![Box::new(Heal {
            actor_id: target_id,
            amount: 70,
        })]
    }
}

pub static READ_HEAL_SCROLL: ReadHealScroll = ReadHealScroll {};

/// Scroll of Phantasmal Killer — Action; single-target WIS save vs DC 15,
/// fail = `Frightened` for 10 rounds. 5e RAW: level-4 illusion,
/// concentration; the target hallucinates their worst fear and takes
/// 4d10 psychic damage at the start of each of their turns. The scroll
/// drops the damage-ramp (the engine doesn't model recurring per-turn
/// hallucination damage cleanly) and surfaces the Frightened install at
/// the rare DC 15 tier. Sibling to Wand of Fear (also Frightened DC 15)
/// — the scroll is the illusion-flavored variant with the same envelope.
/// Fires through the shared `SingleSaveConditionItem` impl.
pub static READ_PHANTASMAL_KILLER_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read phantasmal killer scroll",
    action_aliases: &["phantasmal killer", "pk scroll"],
    item_name: SCROLL_OF_PHANTASMAL_KILLER_NAME,
    log_text: "{actor} reads a scroll of phantasmal killer; the target's worst fear takes form.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 120 ft RAW; 48 tiles (engine cap).
    reach: 48,
    condition: Condition::Frightened,
    timer: ConditionTimer::Rounds(10),
};

/// Potion of Mirror Image — Action; installs `MirroredImages` on the
/// drinker for 10 rounds. 5e RAW: level-2 illusion, three illusory
/// duplicates intercept attacks until popped one-by-one; the potion
/// drops the spell-slot cost and collapses to the fixed-duration
/// consumable envelope. Defensive consumable on the rare half of the
/// pool — pairs with Potion of Blur and Potion of Invisibility on the
/// attacker-disadvantage lane (Mirror Image trades attack-miss-on-
/// duplicate for a finite stack of free hits). Rejects re-drink while
/// already up. Fires through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_MIRROR_IMAGE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of mirror image",
    action_aliases: &["mirror image", "mi potion"],
    item_name: POTION_OF_MIRROR_IMAGE_NAME,
    log_text: "{actor} drinks a potion of mirror image; three flickering duplicates fan out.",
    condition: Condition::MirroredImages,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

const SCROLL_OF_HELLISH_REBUKE_NAME: &str = "Scroll of Hellish Rebuke";
const WAND_OF_MIND_SPIKE_NAME: &str = "Wand of Mind Spike";
const EYES_OF_CHARMING_NAME: &str = "Eyes of Charming";
const GEM_OF_BRIGHTNESS_NAME: &str = "Gem of Brightness";

/// Scroll of Hellish Rebuke — Action; single-target, DEX save vs DC 13,
/// fail = 2d10 fire damage; pass = half. 5e RAW: level-1 evocation
/// reaction (Tiefling racial / warlock). The scroll surfaces the
/// save-or-half damage half and drops the reaction-timing clause —
/// becomes a standard Action-cost consumable. Routes through the
/// shared `SingleSaveDamageItem` impl.
pub static READ_HELLISH_REBUKE_SCROLL: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "read hellish rebuke scroll",
    action_aliases: &["hellish rebuke", "rebuke"],
    item_name: SCROLL_OF_HELLISH_REBUKE_NAME,
    log_label: "scroll of hellish rebuke",
    dice: Dice::new(2, 10),
    flat_bonus: 0,
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Dexterity,
    dc: 13,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    save_for_half: true,
};

/// Wand of Mind Spike — Action; single-target, WIS save vs DC 15,
/// fail = 3d8 psychic damage; pass = half. 5e RAW: level-2 divination,
/// 3d8 psychic with a tracking rider; the wand surfaces the save-for-
/// half damage half and drops the concentration tracker. Fills the
/// psychic single-target damage niche in the wand family. Routes
/// through the shared `SingleSaveDamageItem` impl.
pub static USE_WAND_OF_MIND_SPIKE: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "use wand of mind spike",
    action_aliases: &["mind spike", "spike wand"],
    item_name: WAND_OF_MIND_SPIKE_NAME,
    log_label: "wand of mind spike",
    dice: Dice::new(3, 8),
    flat_bonus: 0,
    damage_type: DamageType::Psychic,
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    save_for_half: true,
};

/// Eyes of Charming — Action; single-target WIS save vs DC 13, fail =
/// `Charmed` for 10 rounds. 5e RAW (DMG): a pair of crystal lenses that
/// can cast Charm Person three times per day; the engine collapses the
/// 3-charge ladder to a single-use envelope (consumed on use) and uses
/// the standard 10-round CC timer every other Charm consumable rides.
/// Routes through the shared `SingleSaveConditionItem` impl.
pub static USE_EYES_OF_CHARMING: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use eyes of charming",
    action_aliases: &["eyes", "charm eyes"],
    item_name: EYES_OF_CHARMING_NAME,
    log_text: "{actor}'s crystal lenses flash; the target's gaze locks.",
    save: AbilityScoreType::Wisdom,
    dc: 13,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Charmed,
    timer: ConditionTimer::Rounds(10),
};

/// Gem of Brightness — Action; 4-tile burst, CON save vs DC 14, fail =
/// `Blinded` for 10 rounds. 5e RAW (DMG): a prism gem with three charge
/// modes (light, blind one, blind cone); the engine collapses the
/// multi-mode utility to the only combat-relevant clause (cone Blinded)
/// and uses a single-use envelope. Routes through the shared
/// `BurstSaveConditionItem` impl.
pub static USE_GEM_OF_BRIGHTNESS: BurstSaveConditionItem = BurstSaveConditionItem {
    action_name: "use gem of brightness",
    action_aliases: &["gem", "brightness"],
    item_name: GEM_OF_BRIGHTNESS_NAME,
    log_text: "{actor} discharges the gem of brightness; a searing prismatic flare blooms.",
    save: AbilityScoreType::Constitution,
    dc: 14,
    radius: 4,
    // 30 ft RAW for the cone-of-light variant; 12 tiles in the 2.5ft grid.
    reach: 12,
    condition: Condition::Blinded,
    timer: ConditionTimer::Rounds(10),
};

const SCROLL_OF_RESILIENT_SPHERE_NAME: &str = "Scroll of Resilient Sphere";
const SCROLL_OF_TELEKINESIS_NAME: &str = "Scroll of Telekinesis";
const WAND_OF_TELEKINESIS_NAME: &str = "Wand of Telekinesis";
const SCROLL_OF_EARTHEN_GRASP_NAME: &str = "Scroll of Earthen Grasp";
const SCROLL_OF_SLEEP_NAME: &str = "Scroll of Sleep";
const SCROLL_OF_SACRED_FLAME_NAME: &str = "Scroll of Sacred Flame";
const SCROLL_OF_MIND_SLIVER_NAME: &str = "Scroll of Mind Sliver";
const SCROLL_OF_MOONBEAM_NAME: &str = "Scroll of Moonbeam";
const SCROLL_OF_GUIDING_BOLT_NAME: &str = "Scroll of Guiding Bolt";

/// Scroll of Resilient Sphere — Action; single-target, DEX save vs DC 15,
/// fail = `Sphered` for 10 rounds. 5e RAW (Otiluke's Resilient Sphere,
/// level-4 evocation, concentration). The scroll drops the concentration
/// gate. Sphered is a full lockdown envelope (zero movement, action
/// economy blocked, attacks against advantage, holder attacks with
/// disadvantage, no reactions) — top-tier single-target CC consumable
/// alongside Wand of Polymorph / Scroll of Flesh to Stone. Routes
/// through the shared `SingleSaveConditionItem` impl.
pub static READ_RESILIENT_SPHERE_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read resilient sphere scroll",
    action_aliases: &["sphere", "resilient sphere"],
    item_name: SCROLL_OF_RESILIENT_SPHERE_NAME,
    log_text: "{actor} reads a scroll of resilient sphere; a hemisphere of force snaps shut.",
    save: AbilityScoreType::Dexterity,
    dc: 15,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::Sphered,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Telekinesis — Action; single-target, STR save vs DC 15,
/// fail = `Lifted` for 10 rounds. 5e RAW (level-5 transmutation,
/// concentration). The scroll surfaces the lift-and-hold half (the spell
/// also lets the caster fling the target around; we collapse to the
/// movement-zero lift envelope). Mirror of Resilient Sphere on the
/// movement-pin lane — Sphered blocks actions too, Lifted only zeros
/// movement, so this sits at the cheaper CC tier. Fires through the
/// shared `SingleSaveConditionItem` impl.
pub static READ_TELEKINESIS_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read telekinesis scroll",
    action_aliases: &["telekinesis", "tk scroll"],
    item_name: SCROLL_OF_TELEKINESIS_NAME,
    log_text: "{actor} reads a scroll of telekinesis; the target floats helplessly skyward.",
    save: AbilityScoreType::Strength,
    dc: 15,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    condition: Condition::Lifted,
    timer: ConditionTimer::Rounds(10),
};

/// Wand of Telekinesis — Action; single-target, STR save vs DC 17, fail
/// = `Lifted` for 10 rounds. Top tier of the Lifted ladder above the
/// Scroll of Telekinesis (DC 15). Same `SingleSaveConditionItem` envelope,
/// harder DC and longer reach — mirrors the Scroll vs Wand of Hold Monster
/// tier split on the Paralyzed lane.
pub static USE_WAND_OF_TELEKINESIS: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of telekinesis",
    action_aliases: &["tk wand", "telekinesis wand"],
    item_name: WAND_OF_TELEKINESIS_NAME,
    log_text: "{actor} aims the wand of telekinesis; the target is wrenched into the air.",
    save: AbilityScoreType::Strength,
    dc: 17,
    // 90 ft RAW; 36 tiles.
    reach: 36,
    condition: Condition::Lifted,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Earthen Grasp — Action; single-target, STR save vs DC 13,
/// fail = `EarthenGrasped` for 10 rounds. 5e RAW (Maximilian's Earthen
/// Grasp, level-2 transmutation, concentration). The scroll drops the
/// concentration gate. EarthenGrasped is a Restrained envelope plus a
/// 2d6 bludgeoning round-end DoT (see `ROUND_END_DOTS`). Sibling to
/// Scroll of Web (Restrained burst) on the entry-tier CC lane — single-
/// target trade for a DoT rider. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_EARTHEN_GRASP_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read earthen grasp scroll",
    action_aliases: &["earthen grasp", "grasp scroll"],
    item_name: SCROLL_OF_EARTHEN_GRASP_NAME,
    log_text: "{actor} reads a scroll of earthen grasp; a stony fist erupts and clamps shut.",
    save: AbilityScoreType::Strength,
    dc: 13,
    // 30 ft RAW; 12 tiles.
    reach: 12,
    condition: Condition::EarthenGrasped,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Sleep — Action; 4-tile burst centered on a picked tile.
/// Rolls a 5d8 HP pool; sweeps enemy creatures in the burst in ascending
/// current-HP order and puts each to `Asleep` (+ `Prone` for the RAW
/// unconscious clause) until the pool is consumed (each target consumes
/// `current_hp` from the pool). 5e RAW: level-1 enchantment, no save —
/// the HP-bucket IS the gate. Creatures immune to Charmed (the engine's
/// proxy for "mind-affecting") are spared; this protects undead,
/// constructs, and fey ancestry races RAW. Mirrors the SLEEP spell
/// exactly — the scroll is a one-static declaration that reuses the
/// same `pool_sweep_targets` chokepoint.
pub static READ_SLEEP_SCROLL: ReadSleepScrollItem = ReadSleepScrollItem {};

/// Scroll of Sleep — bespoke item action that mirrors the SLEEP spell's
/// pool-sweep envelope. Doesn't fit `BurstSaveConditionItem` because that
/// factor uses the installed condition for the immunity-prune (Asleep
/// here) — Sleep RAW uses "mind-affecting" immunity (Charmed proxy in
/// this engine), so undead / constructs are correctly spared via the
/// Charmed-immunity gate inside `pool_sweep_targets`.
pub struct ReadSleepScrollItem {}

impl Action for ReadSleepScrollItem {
    fn name(&self) -> &str {
        "read sleep scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sleep", "sleep scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft RAW; 36 tiles.
        Some(36)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, SCROLL_OF_SLEEP_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_SLEEP_NAME) {
            return Vec::new();
        }
        const BURST_RADIUS: isize = 4;
        let pool_roll = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!(
            "  scroll of sleep: 5d8({}) = {} HP pool",
            pool_roll, pool_roll
        ));
        // Mirror the SLEEP spell's "Charmed-immune is the no-mind-affecting
        // proxy" filter — undead / constructs / fey ancestry get pruned
        // up-front so the pool isn't burnt on no-ops.
        let hit = crate::actions::action_template::pool_sweep_targets(
            encounter,
            caster_id,
            point,
            BURST_RADIUS,
            pool_roll,
            Condition::Charmed,
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in hit {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }));
            // 5e RAW: Sleep also drops the target prone via the
            // unconscious clause. Matches the spell-side install.
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

/// Scroll of Sacred Flame — Action; single-target, DEX save vs DC 13.
/// On fail, 2d8 radiant damage; on save, nothing (no half). 5e RAW: cantrip
/// (1d8 at level 1, scaling to 2d8 at level 5 / 3d8 at level 11 / 4d8 at
/// level 17); the scroll bakes in the level-5 damage tier (2d8) since
/// scroll consumables don't carry caster-level. Pure radiant single-target
/// damage — sibling to Wand of Mind Spike (psychic) and Scroll of Hellish
/// Rebuke (fire) on the entry-tier damage-scroll lane. Routes through the
/// shared `SingleSaveDamageItem` impl.
pub static READ_SACRED_FLAME_SCROLL: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "read sacred flame scroll",
    action_aliases: &["sacred flame", "flame scroll"],
    item_name: SCROLL_OF_SACRED_FLAME_NAME,
    log_label: "scroll of sacred flame",
    dice: Dice::new(2, 8),
    flat_bonus: 0,
    damage_type: DamageType::Radiant,
    save: AbilityScoreType::Dexterity,
    dc: 13,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    save_for_half: false,
};

/// Scroll of Mind Sliver — Action; single-target, INT save vs DC 13.
/// On fail, 2d6 psychic damage; on save, nothing (no half). 5e RAW
/// (Tasha's): cantrip 1d6 with a "subtract 1d4 from the target's next
/// save" rider; the scroll bakes in the level-5 damage tier (2d6) and
/// drops the rider (no engine-side "subtract from next save" hook today).
/// Pure psychic single-target damage at the cheap tier — sibling to
/// Wand of Mind Spike (3d8 psychic, DC 15) on the psychic-damage ladder.
/// Routes through the shared `SingleSaveDamageItem` impl.
pub static READ_MIND_SLIVER_SCROLL: SingleSaveDamageItem = SingleSaveDamageItem {
    action_name: "read mind sliver scroll",
    action_aliases: &["mind sliver", "sliver"],
    item_name: SCROLL_OF_MIND_SLIVER_NAME,
    log_label: "scroll of mind sliver",
    dice: Dice::new(2, 6),
    flat_bonus: 0,
    damage_type: DamageType::Psychic,
    save: AbilityScoreType::Intelligence,
    dc: 13,
    // 60 ft RAW; 24 tiles.
    reach: 24,
    save_for_half: false,
};

/// Scroll of Moonbeam — Action; 3-tile burst, CON save vs DC 15. 5d10
/// radiant damage on fail; half on pass. 5e RAW: level-2 evocation,
/// concentration, sustained zone. The scroll collapses the persistent
/// drip to a single burst-on-cast cast, dropping concentration. Mirror
/// of Scroll of Ice Storm (cold burst) on the radiant burst lane —
/// fills a single-element niche between the cheap Scroll of Shatter
/// (3d8 thunder DC 13) and the rare Scroll of Synaptic Static (8d6
/// psychic DC 15). Routes through the shared `BurstSaveDamageItem` impl.
pub static READ_MOONBEAM_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read moonbeam scroll",
    action_aliases: &["moonbeam", "moon scroll"],
    item_name: SCROLL_OF_MOONBEAM_NAME,
    log_label: "scroll of moonbeam",
    dice: Dice::new(5, 10),
    damage_type: DamageType::Radiant,
    save: AbilityScoreType::Constitution,
    dc: 15,
    // 5ft cylinder radius RAW collapsed to a 3-tile burst — smaller than
    // Fireball's 4 because Moonbeam is RAW a tight zone, not a wide AoE.
    radius: 3,
    // 120 ft RAW; 48 tiles.
    reach: 48,
};

/// Scroll of Guiding Bolt — Action; single-target, 4d6 radiant damage on
/// hit + installs `GuidingBoltLit` for 1 round (next attack against the
/// target has advantage). 5e RAW: level-1 evocation, spell attack roll.
/// The scroll bakes in the standard "no roll needed, no save" envelope
/// for SRD scroll auto-resolve — collapses to a guaranteed-hit damage
/// rider plus the advantage-marker condition. Fills a unique support
/// niche in the loot pool: the radiant damage sets up the rest of the
/// party for a free advantaged swing on the same target.
pub static READ_GUIDING_BOLT_SCROLL: GuidingBoltScrollItem = GuidingBoltScrollItem {};

const GUIDING_BOLT_LOG_LABEL: &str = "scroll of guiding bolt";

/// Scroll of Guiding Bolt — auto-hit single-target radiant damage plus a
/// 1-round `GuidingBoltLit` install. Distinct from the standard
/// `SingleSaveDamageItem` because the install rider runs AND the
/// "guarantee a hit" envelope is the load-bearing fiction; folding the
/// rider in via a Custom Action keeps the existing factor structs
/// orthogonal. Mirror of `MagicMissileItem`'s "auto-hit, no save" shape
/// with a radiant typing and a condition rider.
pub struct GuidingBoltScrollItem {}

impl Action for GuidingBoltScrollItem {
    fn name(&self) -> &str {
        "read guiding bolt scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["guiding bolt", "bolt scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW; 48 tiles.
        Some(48)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, SCROLL_OF_GUIDING_BOLT_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_GUIDING_BOLT_NAME) {
            return Vec::new();
        }
        let dice = Dice::new(4, 6);
        let amount = encounter.roll(&dice);
        encounter.log(format!(
            "  {}: {}d{} = {} radiant",
            GUIDING_BOLT_LOG_LABEL, dice.count, dice.faces, amount
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount,
                damage_type: DamageType::Radiant,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::GuidingBoltLit,
                // RAW: lasts until the end of the caster's next turn.
                // The condition's existing burn-off-on-attack logic
                // consumes it the moment a follow-up swing connects.
                timer: ConditionTimer::Rounds(1),
            }),
        ]
    }
}

const SCROLL_OF_ENHANCE_ABILITY_NAME: &str = "Scroll of Enhance Ability";
const SCROLL_OF_BLINK_NAME: &str = "Scroll of Blink";
const SCROLL_OF_CONTAGION_NAME: &str = "Scroll of Contagion";

/// Scroll of Enhance Ability — Action; install `Heroic` for 10 rounds on
/// a single ally. 5e RAW (level-2 transmutation, concentration). The
/// scroll drops the concentration gate and surfaces the Heroic-style
/// buff envelope as a fire-and-forget ally buff. Sibling to Scroll of
/// Bless (Blessed) and Scroll of Shield of Faith (ShieldOfFaith) on the
/// single-target ally-buff scroll lane — distinct from Bless by the
/// concentration-free envelope (the scroll already absorbs that cost)
/// and by the choice of Heroic (Frightened immunity) over Blessed
/// (+1d4 attack / saves). Touch range (RAW); fires through the shared
/// `SingleTargetBuffItem` impl.
pub static READ_ENHANCE_ABILITY_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read enhance ability scroll",
    action_aliases: &["ea scroll", "enhance ability scroll"],
    item_name: SCROLL_OF_ENHANCE_ABILITY_NAME,
    log_text: "{actor} reads a scroll of enhance ability; the target's stride steadies.",
    condition: Condition::Heroic,
    timer: ConditionTimer::Rounds(10),
    // Touch range RAW; 1 tile in the 2.5ft grid.
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Blink — Action; install `Displaced` for 10 rounds on the
/// reader. 5e RAW (level-3 transmutation, no concentration). Self-only
/// defensive consumable that gives attackers disadvantage until the
/// reader takes damage (the `Displaced` envelope breaks on hit). Slots
/// alongside Potion of Mirror Image / Potion of Blur on the attacker-
/// disadvantage defensive lane. Distinct from the others by being a
/// scroll (the wizard / sorcerer / cleric can read while concentrating
/// on something else, since Blink doesn't require concentration). Fires
/// through the shared `SelfConditionItem` impl.
pub static READ_BLINK_SCROLL: SelfConditionItem = SelfConditionItem {
    action_name: "read blink scroll",
    action_aliases: &["blink scroll", "blink"],
    item_name: SCROLL_OF_BLINK_NAME,
    log_text: "{actor} reads a scroll of blink; their form flickers between planes.",
    condition: Condition::Displaced,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Scroll of Contagion — Action; single-target, CON save vs DC 15, fail
/// = `Poisoned` for 10 rounds. 5e RAW (level-5 necromancy, touch, 3
/// failed CON saves to disease the target). The scroll collapses the
/// three-save chain to a single save vs the scroll's fixed DC. Slots in
/// the rare half of the single-target CC lane alongside Scroll of Hold
/// Person and Wand of Paralysis — distinct by the CON-save lane (bites
/// low-CON enemies that shrug off the WIS-save Paralyzed installs) and
/// by the Poisoned envelope (disadvantage on attacks + checks, not a
/// full action-economy block). Touch range; fires through the shared
/// `SingleSaveConditionItem` impl.
pub static READ_CONTAGION_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read contagion scroll",
    action_aliases: &["contagion scroll", "contagion"],
    item_name: SCROLL_OF_CONTAGION_NAME,
    log_text: "{actor} reads a scroll of contagion; a foul mist coils around the target.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    // Touch range RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    condition: Condition::Poisoned,
    timer: ConditionTimer::Rounds(10),
};

const SCROLL_OF_SPIDER_CLIMB_NAME: &str = "Scroll of Spider Climb";
const SCROLL_OF_HEROISM_NAME: &str = "Scroll of Heroism";
const WAND_OF_BLESS_NAME: &str = "Wand of Bless";
const NECKLACE_OF_LIGHTNING_BOLTS_NAME: &str = "Necklace of Lightning Bolts";
const SCROLL_OF_MIND_BLANK_NAME: &str = "Scroll of Mind Blank";

/// Scroll of Spider Climb — Action; install `SpiderClimbing` for 10
/// rounds on the reader. 5e RAW (Spider Climb, level-2 transmutation,
/// touch, concentration). The scroll bypasses the concentration gate.
/// The `SpiderClimbing` condition routes through `condition_speed_bonus`
/// for the +20 ft climb speed and through the engine's terrain-walk
/// gates for the wall-climb fiction — same lane Slippers of Spider
/// Climbing rides as a passive. Single-use consumable; rejects re-read
/// while already up so the scroll isn't burned on a no-op refresh.
/// Routes through the shared `SelfConditionItem` impl.
pub static READ_SPIDER_CLIMB_SCROLL: SelfConditionItem = SelfConditionItem {
    action_name: "read spider climb scroll",
    action_aliases: &["sc scroll", "spider climb scroll"],
    item_name: SCROLL_OF_SPIDER_CLIMB_NAME,
    log_text: "{actor} reads a scroll of spider climb; their fingertips tingle with arachnid grip.",
    condition: Condition::SpiderClimbing,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Scroll of Heroism — Action; self-install `Heroic` (Frightened
/// immunity) for 10 rounds + 10 temp HP. 5e RAW: level-1 enchantment,
/// concentration, touch; the scroll bypasses concentration and
/// surfaces the Heroic-style buff envelope as a fire-and-forget self
/// buff. Sibling to Potion of Heroism (same install, bonus-action
/// cost) — distinct from the potion by Action cost and the "any class
/// can read it" envelope. Routes through the shared
/// `SelfConditionItem` impl via the `temp_hp` lane.
pub static READ_HEROISM_SCROLL: SelfConditionItem = SelfConditionItem {
    action_name: "read heroism scroll",
    action_aliases: &["hr scroll", "heroism scroll"],
    item_name: SCROLL_OF_HEROISM_NAME,
    log_text: "{actor} reads a scroll of heroism; a swell of courage steels them.",
    condition: Condition::Heroic,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    // Refresh allowed — a wounded reader whose temp-HP cushion ticked
    // low can re-read for a fresh 10-HP cushion (mirrors the Potion of
    // Heroism refresh stance; the install itself uses the timer-take-max
    // semantics so the 10-round duration doesn't shrink either).
    reject_when_active: false,
    temp_hp: Some(10),
};

/// Wand of Bless — Bonus Action; single-target ally buff. Installs
/// `Blessed` for 10 rounds (+1d4 to attack rolls and saves). 5e RAW:
/// Bless is a level-1 concentration spell at Action cost hitting up to
/// 3 creatures; the wand collapses to a single-target install with the
/// standard fixed-duration timer all consumable buffs ride, AND
/// surfaces the buff at bonus-action cost — distinct from Scroll of
/// Bless (Action cost) in the action-economy lane. Fires through the
/// shared `SingleTargetBuffItem` impl.
pub static USE_WAND_OF_BLESS: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "use wand of bless",
    action_aliases: &["bless wand", "bless+"],
    item_name: WAND_OF_BLESS_NAME,
    log_text: "{actor} waves the wand of bless; a soft golden light settles on the target.",
    condition: Condition::Blessed,
    timer: ConditionTimer::Rounds(10),
    // 30 ft RAW; 12 tiles.
    reach: 12,
    bonus_action: true,
    reject_when_active: true,
};

/// Necklace of Lightning Bolts — 5d6 lightning DEX-save burst (DC 15,
/// 2 radius). Single-bead consumable. Mirror of Necklace of Fireballs
/// on the lightning lane: same payload shape, different damage type so
/// resistance landscape differs (fire-resistant enemies shrug Fireball
/// beads, lightning-resistant enemies shrug these). Tighter burst
/// radius (2 vs 4) matches the Lightning Bolt scroll envelope — the
/// lightning lane RAW is a tight line, not a wide AoE. Routes through
/// the shared `BurstSaveDamageItem` impl.
pub static USE_NECKLACE_OF_LIGHTNING_BOLTS: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "use necklace of lightning bolts",
    action_aliases: &["lightning necklace", "bolt bead"],
    item_name: NECKLACE_OF_LIGHTNING_BOLTS_NAME,
    log_label: "necklace of lightning bolts (bead)",
    dice: Dice::new(5, 6),
    damage_type: DamageType::Lightning,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 2,
    // 100 ft RAW (Lightning Bolt range); 40 tiles.
    reach: 40,
};

/// Scroll of Mind Blank — Action; self-install `MindBlanked` for 10
/// rounds (Charmed-immunity + psychic-immunity). 5e RAW: level-8
/// abjuration, 24-hour duration; the scroll collapses to the engine's
/// combat-scale 10-round envelope. Sibling to Potion of Mind Blank
/// (same install, bonus-action cost) — distinct from the potion by
/// Action cost and the "any caster can read it" envelope. Top-tier
/// mental-defense consumable. Rejects re-read while up to avoid the
/// no-op refresh. Routes through the shared `SelfConditionItem` impl.
pub static READ_MIND_BLANK_SCROLL: SelfConditionItem = SelfConditionItem {
    action_name: "read mind blank scroll",
    action_aliases: &["mb scroll", "mind blank scroll"],
    item_name: SCROLL_OF_MIND_BLANK_NAME,
    log_text: "{actor} reads a scroll of mind blank; a silvery psychic ward forms.",
    condition: Condition::MindBlanked,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Config struct for "multi-target ally buff" consumable items — the
/// shared shape behind Scroll of Mass Bless / Banner of Valor / Drum of
/// Inspiration. Each static instance encodes a single item's per-cast
/// configuration; the `Action` impl below picks the up-to-`max_targets`
/// closest ally-team actors (combat-active or dying) within `range_tiles`
/// of the caster and installs `condition` for `timer` on each. Allies
/// who already have the condition are skipped over so the install never
/// burns a slot on a no-op refresh (mirrors `reject_when_active` on the
/// single-target buff factor — here it filters per-target instead of
/// rejecting the whole cast, since one already-buffed ally shouldn't
/// blackball the other slots).
///
/// Mirror of `MultiTargetHealItem` for the condition-install lane. The
/// caster is included as a candidate so a buff-yourself-plus-your-allies
/// envelope folds through the same impl as the "burst from a banner /
/// drum" envelope. Adding a new mass-buff scroll / banner is a one-static
/// declaration — no new `Action` impl needed.
pub struct MultiTargetBuffItem {
    /// Player-facing action name (e.g. "read mass bless scroll").
    pub action_name: &'static str,
    /// Picker aliases (e.g. ["mass bless", "bless burst"]).
    pub action_aliases: &'static [&'static str],
    /// Inventory item name to gate validate / consume on.
    pub item_name: &'static str,
    /// Full log line emitted on use. The `{actor}` placeholder is
    /// substituted with the caster's name; no other formatting is
    /// performed.
    pub log_text: &'static str,
    /// Condition to install on each picked ally.
    pub condition: Condition,
    /// Timer for the install (typically `Rounds(10)` for combat-scale
    /// buffs).
    pub timer: ConditionTimer,
    /// `true` ⇒ Bonus Action cost; `false` ⇒ Action cost.
    pub bonus_action: bool,
    /// Maximum tile-Chebyshev distance from the caster's footprint to
    /// any ally that qualifies for the buff. Matches the
    /// `MultiTargetHealItem.range_tiles` lane so the loot pool's "30 ft
    /// burst" / "60 ft reach" envelopes both ride one knob.
    pub range_tiles: isize,
    /// Maximum number of allies to buff. 5e RAW caps Bless at 3 targets
    /// — bigger banners / drums sit at 4-6 to match their bardic flavor.
    pub max_targets: usize,
    /// Optional flat temp HP grant alongside the condition. Used by
    /// Aid-flavored mass buffs (Banner of Valor) — `None` for plain
    /// condition installs. Mirrors `SelfConditionItem.temp_hp` so the
    /// two factors share the loot-pool's temp-HP semantics.
    pub temp_hp: Option<u32>,
}

impl Action for MultiTargetBuffItem {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        // The buff install reads as a support action so the AI's heal
        // / buff pipeline can pick the item up alongside genuine heals.
        // Mirrors `SingleTargetBuffItem`'s is_heal=true stance.
        true
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        item_use_cost(e, c, self.bonus_action)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, self.item_name)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::GainTempHp;

        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));

        // Layer a buff-specific priority sort on top of the shared
        // ally-candidates walker:
        //   0 = unbuffed (gets a fresh install)
        //   1 = already buffed (no-op refresh; sort to the back so the
        //       cap budget lands on unbuffed allies first)
        // Skip condition-immune allies entirely (they can't take the
        // install) so they don't eat a slot. Mirrors `MultiTargetHealItem`'s
        // priority shape but with "already has condition" as the priority
        // key instead of HP deficit — the install equivalent of "skip
        // the healthy ally."
        let condition = self.condition;
        let mut candidates: Vec<(u8, isize, usize)> = encounter
            .ally_candidates_in_range(caster_id, self.range_tiles)
            .into_iter()
            .filter_map(|(id, dist)| {
                let a = encounter.actors.get(&id)?;
                if a.effectively_immune_to_condition(condition) {
                    return None;
                }
                let priority = if a.has_condition(condition) { 1u8 } else { 0u8 };
                Some((priority, dist, id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(self.max_targets);

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for (_, _, id) in candidates {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition,
                timer: self.timer,
            }));
            if let Some(amount) = self.temp_hp {
                effects.push(Box::new(GainTempHp {
                    actor_id: id,
                    amount,
                }));
            }
        }
        effects
    }
}

const SCROLL_OF_MASS_BLESS_NAME: &str = "Scroll of Mass Bless";
const BANNER_OF_VALOR_NAME: &str = "Banner of Valor";
const DRUM_OF_INSPIRATION_NAME: &str = "Drum of Inspiration";

/// Scroll of Mass Bless — Action; install `Blessed` for 10 rounds on up
/// to 3 allies within 12 tiles (30 ft RAW burst, matching the level-1
/// Bless target cap). 5e RAW: Bless is a level-1 concentration spell that
/// blesses up to 3 creatures within 30 ft; the scroll collapses to the
/// fixed 10-round non-concentration envelope all support-scroll buffs
/// ride. Sibling to Scroll of Bless (single-target) — distinct by the
/// multi-target lane. Fires through the shared `MultiTargetBuffItem`
/// impl.
pub static READ_MASS_BLESS_SCROLL: MultiTargetBuffItem = MultiTargetBuffItem {
    action_name: "read mass bless scroll",
    action_aliases: &["mass bless", "bless burst"],
    item_name: SCROLL_OF_MASS_BLESS_NAME,
    log_text: "{actor} reads a scroll of mass bless; a wide halo of golden light settles.",
    condition: Condition::Blessed,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    // 30 ft RAW; 12 tiles in the 2.5 ft grid.
    range_tiles: 12,
    max_targets: 3,
    temp_hp: None,
};

/// Banner of Valor — Bonus Action; install `Heroic` for 10 rounds on up
/// to 4 allies within 6 tiles (15 ft burst self-centered) and grant each
/// 5 temp HP. Bardic / paladin trinket flavor — the holder hoists the
/// banner and the nearby line steels up against Frightened. Sits between
/// Scroll of Mass Bless (3 targets, no temp HP, Action cost) and a
/// hypothetical higher-tier mass-buff on the support-burst lane. Fires
/// through the shared `MultiTargetBuffItem` impl via the `temp_hp` lane.
pub static USE_BANNER_OF_VALOR: MultiTargetBuffItem = MultiTargetBuffItem {
    action_name: "raise banner of valor",
    action_aliases: &["banner valor", "raise banner"],
    item_name: BANNER_OF_VALOR_NAME,
    log_text: "{actor} hoists the Banner of Valor; the nearby line steels.",
    condition: Condition::Heroic,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    // 15 ft self-burst; 6 tiles.
    range_tiles: 6,
    max_targets: 4,
    temp_hp: Some(5),
};

/// Drum of Inspiration — Action; install `Inspired` for 10 rounds on up
/// to 4 allies within 12 tiles (30 ft burst). Bardic flavor — the holder
/// beats the drum and the line picks up an Inspired die for their next
/// save / attack roll. Distinct from Banner of Valor (Heroic +
/// temp-HP, bonus action) on the Inspired lane — the drum is the longer-
/// reach Action-cost variant. Fires through the shared
/// `MultiTargetBuffItem` impl.
pub static USE_DRUM_OF_INSPIRATION: MultiTargetBuffItem = MultiTargetBuffItem {
    action_name: "beat drum of inspiration",
    action_aliases: &["drum inspiration", "drum"],
    item_name: DRUM_OF_INSPIRATION_NAME,
    log_text: "{actor} beats the Drum of Inspiration; a rolling cadence steadies the line.",
    condition: Condition::Inspired,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    // 30 ft burst RAW; 12 tiles.
    range_tiles: 12,
    max_targets: 4,
    temp_hp: None,
};

const WAND_OF_STUNNING_NAME: &str = "Wand of Stunning";
const IRON_BANDS_OF_BILARRO_NAME: &str = "Iron Bands of Bilarro";
const SCROLL_OF_SANCTUARY_NAME: &str = "Scroll of Sanctuary";
const WAND_OF_MASS_CURE_WOUNDS_NAME: &str = "Wand of Mass Cure Wounds";
const SCROLL_OF_CRUSADERS_MANTLE_NAME: &str = "Scroll of Crusader's Mantle";

/// Wand of Stunning — Action; single-target CON save vs DC 15, fail =
/// Stunned for 10 rounds. The only consumable in the loot pool that
/// installs Stunned (which blocks Action, Bonus Action, Reaction AND
/// movement RAW — strictly stronger than Paralyzed's Incapacitated +
/// movement-pin since the auto-fail-STR/DEX-saves clause is the only
/// piece Paralyzed adds on top). Sits in the rare half of the single-
/// target lockdown lane alongside Wand of Hold Monster (Paralyzed DC 17)
/// — distinct by Stunned's "no movement either" envelope and the
/// cheaper CON DC tier. Fires through the shared `SingleSaveConditionItem`
/// impl.
pub static USE_WAND_OF_STUNNING: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "use wand of stunning",
    action_aliases: &["stunning wand", "wand of stun"],
    item_name: WAND_OF_STUNNING_NAME,
    log_text: "{actor} flicks the wand of stunning; a sharp concussive pulse strikes.",
    save: AbilityScoreType::Constitution,
    dc: 15,
    // 60 ft range RAW; 24 tiles in the 2.5ft grid.
    reach: 24,
    condition: Condition::Stunned,
    timer: ConditionTimer::Rounds(10),
};

/// Iron Bands of Bilarro — Action; throw at a target, STR save vs DC 17,
/// fail = Restrained for 10 rounds. 5e RAW: the iron bands are a one-shot
/// thrown item that wraps a Large-or-smaller creature in metal bands; we
/// route the install through `SingleSaveConditionItem` for the Restrained
/// install at the rare DC 17 tier. Sibling to Wand of Web (DC 15 burst
/// Restrained) and Scroll of Earthen Grasp (DC 13 single-target
/// Restrained) — the iron bands sit at the top of the Restrained single-
/// target ladder with the meanest save DC. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static USE_IRON_BANDS_OF_BILARRO: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "throw iron bands of bilarro",
    action_aliases: &["iron bands", "bilarro"],
    item_name: IRON_BANDS_OF_BILARRO_NAME,
    log_text: "{actor} hurls the Iron Bands of Bilarro; metal coils whip toward the target.",
    save: AbilityScoreType::Strength,
    dc: 17,
    // 60 ft thrown range RAW; 24 tiles.
    reach: 24,
    condition: Condition::Restrained,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Sanctuary — Bonus Action; install `Sanctuary` for 10 rounds
/// on a single ally within 24 tiles (60 ft RAW). 5e RAW: level-1
/// abjuration, bonus action, target must be willing — the scroll
/// envelope collapses the willing-target clause onto the existing ally-
/// target gate that every `SingleTargetBuffItem` reads. Sibling to
/// Potion of Sanctuary (self-only, bonus-action drink) on the Sanctuary
/// lane — the scroll variant lets a caster ward a different ally (the
/// rogue in the back, the cleric setting up a heal) without making the
/// drinker themselves drop their concentration / action economy. Fires
/// through the shared `SingleTargetBuffItem` impl; rejects re-cast
/// when the target is already Sanctified so the consumable isn't burned
/// on a no-op timer refresh.
pub static READ_SANCTUARY_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read sanctuary scroll",
    action_aliases: &["sanctuary scroll", "sanc scroll"],
    item_name: SCROLL_OF_SANCTUARY_NAME,
    log_text: "{actor} reads a scroll of sanctuary; an unseen ward settles over the ally.",
    condition: Condition::Sanctuary,
    timer: ConditionTimer::Rounds(10),
    // 30 ft range RAW; 12 tiles.
    reach: 12,
    bonus_action: true,
    reject_when_active: true,
};

/// Wand of Mass Cure Wounds — Action; heal up to 6 allies within 12
/// tiles (30 ft burst RAW) for 5d8+5 HP each. Top of the multi-target
/// ally-heal ladder above Scroll of Mass Cure Wounds (3d8+5) — same
/// envelope, larger pool. 5e RAW: 7 charges casting Mass Cure Wounds
/// at the level-5 baseline (5d8 + caster mod, up to 6 targets); we
/// collapse to a single-use cast at the level-5 envelope with the
/// existing "scroll has no caster-ability tie, +5 stand-in" pattern
/// every multi-heal item rides. Fires through the shared
/// `MultiTargetHealItem` impl.
pub static USE_WAND_OF_MASS_CURE_WOUNDS: MultiTargetHealItem = MultiTargetHealItem {
    action_name: "use wand of mass cure wounds",
    action_aliases: &["mcw wand", "mass cure wand"],
    item_name: WAND_OF_MASS_CURE_WOUNDS_NAME,
    log_label: "wand of mass cure wounds",
    dice: Dice::new(5, 8),
    flat_bonus: 5,
    bonus_action: false,
    range_tiles: 12,
    max_targets: 6,
};

/// Scroll of Crusader's Mantle — Action; install `CrusadersMantled` for
/// 10 rounds on up to 4 allies within 12 tiles (30 ft RAW aura). 5e RAW:
/// level-3 evocation, self-aura with a 30-ft radius, concentration; every
/// allied weapon hit gains +1d4 radiant. The scroll variant collapses to
/// the engine's mass-buff envelope — picks up to 4 nearest allies and
/// stamps the condition on each, dropping the concentration gate and
/// the self-only self-aura RAW. The +1d4 radiant rider lives on
/// `ON_HIT_RIDERS` in `engine::attack`, so the install lands the
/// per-attack bonus the same way the self-cast spell already does.
/// Sibling to Scroll of Mass Bless (Blessed mass install) on the
/// multi-target offensive buff lane — distinct by the radiant damage
/// rider vs. Bless's flat +1d4 attack / save modifier. Fires through
/// the shared `MultiTargetBuffItem` impl.
pub static READ_CRUSADERS_MANTLE_SCROLL: MultiTargetBuffItem = MultiTargetBuffItem {
    action_name: "read crusader's mantle scroll",
    action_aliases: &["mantle scroll", "crusader scroll"],
    item_name: SCROLL_OF_CRUSADERS_MANTLE_NAME,
    log_text: "{actor} reads a scroll of crusader's mantle; a holy radiance suffuses the line.",
    condition: Condition::CrusadersMantled,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    // 30 ft self-aura RAW; 12 tiles.
    range_tiles: 12,
    max_targets: 4,
    temp_hp: None,
};

const SCROLL_OF_PYROTECHNICS_NAME: &str = "Scroll of Pyrotechnics";
const SCROLL_OF_FLAME_ARROWS_NAME: &str = "Scroll of Flame Arrows";
const POTION_OF_ASHARDALONS_STRIDE_NAME: &str = "Potion of Ashardalon's Stride";
const POTION_OF_OTHERWORLDLY_GUISE_NAME: &str = "Potion of Otherworldly Guise";
const HORN_OF_BLASTING_NAME: &str = "Horn of Blasting";
const JAVELIN_OF_LIGHTNING_NAME: &str = "Javelin of Lightning";
const BEAD_OF_FORCE_NAME: &str = "Bead of Force";
const SCROLL_OF_FLY_NAME: &str = "Scroll of Fly";
const SCROLL_OF_BESTOW_CURSE_NAME: &str = "Scroll of Bestow Curse";
const SCROLL_OF_CONJURE_ANIMALS_NAME: &str = "Scroll of Conjure Animals";
const SCROLL_OF_LONGSTRIDER_NAME: &str = "Scroll of Longstrider";
const SCROLL_OF_BARKSKIN_NAME: &str = "Scroll of Barkskin";
const SCROLL_OF_MAGNIFY_GRAVITY_NAME: &str = "Scroll of Magnify Gravity";
const SCROLL_OF_ELEMENTAL_WEAPON_NAME: &str = "Scroll of Elemental Weapon";
const SCROLL_OF_TRUE_SEEING_NAME: &str = "Scroll of True Seeing";
const SCROLL_OF_PROTECTION_FROM_POISON_NAME: &str = "Scroll of Protection from Poison";

/// Scroll of Pyrotechnics — Action; 1d8 fire DEX-save burst (DC 13,
/// 2-radius / 10 ft RAW) at a point within 24 tiles (60 ft RAW). 5e RAW:
/// XGE level-2 transmutation, Fireworks variant. The scroll collapses the
/// rider Blinded clause of the spell to keep the consumable on the
/// entry-tier elemental-burst lane alongside Scroll of Burning Hands /
/// Scroll of Thunderwave. Fires through the shared `BurstSaveDamageItem`
/// impl.
pub static READ_PYROTECHNICS_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read pyrotechnics scroll",
    action_aliases: &["pyrotechnics scroll", "pyro scroll"],
    item_name: SCROLL_OF_PYROTECHNICS_NAME,
    log_label: "scroll of pyrotechnics",
    dice: Dice::new(1, 8),
    damage_type: DamageType::Fire,
    save: AbilityScoreType::Constitution,
    dc: 13,
    radius: 2,
    // 60 ft RAW range; 24 tiles.
    reach: 24,
};

/// Scroll of Flame Arrows — Action; install `FlamingArrowed` for 10
/// rounds on the reader. 5e RAW: XGE level-3 transmutation, concentration,
/// touch — the scroll bypasses the concentration gate and surfaces the
/// per-ranged-hit +1d6 fire rider via the `ON_HIT_RIDERS` table. Self-
/// only ranged weapon buff; rejects re-read while up to avoid the no-op
/// refresh. Sibling to Scroll of Crusader's Mantle / Spirit Shroud on
/// the on-hit weapon buff scroll lane — distinct by the ranged-only
/// gate (a melee fallback can't burn the buff). Routes through the
/// shared `SelfConditionItem` impl.
pub static READ_FLAME_ARROWS_SCROLL: SelfConditionItem = SelfConditionItem {
    action_name: "read flame arrows scroll",
    action_aliases: &["flame arrows scroll", "fa scroll"],
    item_name: SCROLL_OF_FLAME_ARROWS_NAME,
    log_text: "{actor} reads a scroll of flame arrows; their quiver ignites with magical fire.",
    condition: Condition::FlamingArrowed,
    timer: ConditionTimer::Rounds(10),
    bonus_action: false,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Ashardalon's Stride — Bonus Action; install `AshardalonStriding`
/// for 10 rounds on the drinker. 5e RAW: TCE level-3 transmutation,
/// concentration, self; the potion bypasses concentration and surfaces
/// the +20 ft speed bump + per-step adjacent-enemy fire trail damage as
/// a fire-and-forget combat buff. Sibling to Potion of Longstrider
/// (passive +10 ft speed) / Potion of Climbing (climb-speed bump) on
/// the mobility-consumable lane — distinct by the combat-flavored
/// trail-damage hook (the spell's signature mechanic). Rejects re-drink
/// while up. Fires through the shared `SelfConditionItem` impl.
pub static DRINK_POTION_OF_ASHARDALONS_STRIDE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of ashardalon's stride",
    action_aliases: &["ashardalon", "stride potion"],
    item_name: POTION_OF_ASHARDALONS_STRIDE_NAME,
    log_text: "{actor} drinks a potion of ashardalon's stride; their body crackles with elemental fire.",
    condition: Condition::AshardalonStriding,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Potion of Otherworldly Guise — Bonus Action; install `OtherworldlyGuised`
/// for 10 rounds on the drinker. 5e RAW: TCE level-6 transmutation,
/// concentration, self; the potion bypasses concentration and surfaces
/// the full envelope (+2 AC, +60 ft fly speed, radiant + poison
/// resistance, Charmed / Frightened / Poisoned dynamic immunity, +2d6
/// radiant melee weapon rider) as a fire-and-forget combat buff. Sibling
/// to Potion of Foresight on the legendary-tier offensive + defensive
/// consumable lane. Rejects re-drink while up. Fires through the shared
/// `SelfConditionItem` impl.
pub static DRINK_POTION_OF_OTHERWORLDLY_GUISE: SelfConditionItem = SelfConditionItem {
    action_name: "drink potion of otherworldly guise",
    action_aliases: &["guise potion", "og potion"],
    item_name: POTION_OF_OTHERWORLDLY_GUISE_NAME,
    log_text: "{actor} drinks a potion of otherworldly guise; their form shifts into a celestial avatar.",
    condition: Condition::OtherworldlyGuised,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
    temp_hp: None,
};

/// Horn of Blasting — Action; self-centered 6-tile (15 ft) thunder burst,
/// 5d6 damage, CON save vs DC 15 for half. 5e RAW: emits a thunderous
/// note in a 30-ft cone dealing 5d6 thunder; creatures that fail also
/// take a `Deafened` rider. We collapse the cone to a friend-or-foe
/// burst centered on the wielder (the engine has no cone primitive
/// today; the 6-tile burst's radius approximates the cone's footprint
/// for combat purposes) and ride the existing Deafened install on a
/// failed save. The horn is a multi-use trinket in RAW; we collapse to
/// a single-use consumable so the loot pool keeps a flat "one fire per
/// drop" semantics — matching Necklace of Fireballs / Lightning Bolts.
///
/// Schema is `NoArgs` (self-centered like Thunderwave); the burst sits
/// at the wielder's footprint center. Does NOT use the
/// `BurstSaveDamageItem` factor because that factor requires a picked
/// target tile; folding a self-centered burst variant into the factor
/// would muddy its single-purpose semantics, so a small custom impl
/// keeps both lanes orthogonal.
pub struct HornOfBlastingItem {}

impl Action for HornOfBlastingItem {
    fn name(&self) -> &str {
        "blow horn of blasting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["horn", "blast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, HORN_OF_BLASTING_NAME)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !consume_caster_item(encounter, caster_id, HORN_OF_BLASTING_NAME) {
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        const RADIUS: isize = 6;
        const DC: i32 = 15;
        let damage = encounter.roll(&Dice::new(5, 6));
        let name = encounter.actor_name(caster_id);
        encounter.log(format!(
            "{} sounds the Horn of Blasting; a thunderous note erupts.",
            name
        ));
        encounter.log(format!(
            "  horn of blasting: 5d6 = {} damage",
            damage
        ));
        // 5e Sorcerer Careful Spell metamagic: shielded allies in the
        // burst auto-pass AND take 0 damage. Mirrors the `resolve_burst_save_damage`
        // chokepoint; we re-route to it for parity with every other
        // burst item.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let shielded = encounter.auto_pass_shielded_allies(caster_id, center, RADIUS);
        for tid in encounter.neutral_burst_targets(caster_id, center, RADIUS) {
            if shielded.contains(&tid) {
                continue;
            }
            let save = encounter.roll_save_against_caster(
                tid,
                AbilityScoreType::Constitution,
                DC,
                caster_id,
            );
            let passed = save.passed();
            // No evasion shortcut — CON saves never trigger evasion
            // (which is DEX-only RAW).
            let dmg = SaveDamagePolicy::HalfOnSave.apply(damage, passed);
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Thunder,
                }));
            }
            // Deafened rider on failed save (RAW: 1 minute → 10 rounds).
            // Independent of damage rolls landing — even a 0-damage fail
            // still installs the deafness.
            if !passed {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Deafened,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        effects
    }
}

pub static BLOW_HORN_OF_BLASTING: HornOfBlastingItem = HornOfBlastingItem {};

/// Javelin of Lightning — Action; thrown 120 ft (48 tiles), 4d6 lightning
/// in a 2-tile (10 ft) burst centered on the targeted tile, DEX save vs
/// DC 13 for half. 5e RAW: the javelin transforms into a 5-ft-wide bolt
/// of lightning along the throw path; we collapse the line to a small
/// burst centered on the target tile (matching the Necklace of Lightning
/// Bolts shape so the lightning lane stays consistent). Single-use
/// consumable — the javelin is destroyed on hit per RAW's "the javelin
/// reverts to its normal form after the bolt" clause (we just consume
/// the item to keep the loot pool flat). Fires through the shared
/// `BurstSaveDamageItem` impl.
pub static THROW_JAVELIN_OF_LIGHTNING: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "throw javelin of lightning",
    action_aliases: &["javelin lightning", "lightning javelin"],
    item_name: JAVELIN_OF_LIGHTNING_NAME,
    log_label: "javelin of lightning",
    dice: Dice::new(4, 6),
    damage_type: DamageType::Lightning,
    save: AbilityScoreType::Dexterity,
    dc: 13,
    radius: 2,
    // 120 ft thrown RAW; 48 tiles in the 2.5ft grid.
    reach: 48,
};

/// Bead of Force — Action; a single bead torn from a Necklace of Beads
/// is hurled at a tile within 60 ft (24 tiles). 5d4 force damage, DEX
/// save vs DC 15 for half, in a small 2-tile burst. 5e RAW: the bead
/// creates a 10-ft-diameter sphere of force that bursts on landing,
/// trapping failed-save targets inside; we collapse to a damage burst
/// (the trapping clause would need a new condition — and the load-bearing
/// effect is the force damage). Fills the force-burst niche in the loot
/// pool: force is the rarely-resisted typed-damage lane (vs Fireball's
/// fire-resisted lane), so the bead lands consistently against most
/// resistance loadouts. Fires through the shared `BurstSaveDamageItem`
/// impl.
pub static THROW_BEAD_OF_FORCE: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "throw bead of force",
    action_aliases: &["bead force", "force bead"],
    item_name: BEAD_OF_FORCE_NAME,
    log_label: "bead of force",
    dice: Dice::new(5, 4),
    damage_type: DamageType::Force,
    save: AbilityScoreType::Dexterity,
    dc: 15,
    radius: 2,
    // 60 ft RAW; 24 tiles.
    reach: 24,
};

/// Scroll of Fly — Action; install `Flying` for 10 rounds on a single
/// ally within 1 tile (touch RAW). 5e RAW: Fly is a level-3 transmutation,
/// concentration; the scroll bypasses concentration and surfaces the
/// +60 ft fly speed bump as a fire-and-forget ally buff. Sibling to
/// Winged Boots (passive Flying trinket) and Potion of Flying
/// (self-only consumable) on the flight-buff lane — distinct from
/// both by the ally-target envelope (a non-flying martial can hand
/// the scroll to the rogue / fighter and let them sky-dance). Rejects
/// re-cast when the target is already Flying so the consumable isn't
/// burned on a no-op refresh. Fires through the shared
/// `SingleTargetBuffItem` impl.
pub static READ_FLY_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read fly scroll",
    action_aliases: &["fly scroll", "scroll fly"],
    item_name: SCROLL_OF_FLY_NAME,
    log_text: "{actor} reads a scroll of fly; their target's feet rise from the ground.",
    condition: Condition::Flying,
    timer: ConditionTimer::Rounds(10),
    // Touch RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Bestow Curse — Action; touch a single target, WIS save vs
/// DC 15. On fail, install `Baned` (–1d4 / –2 to attack rolls and saves)
/// for 10 rounds. 5e RAW: Bestow Curse is a level-3 necromancy,
/// concentration, that lets the caster pick one of four cursed effects
/// (one of which is the "attack rolls have disadvantage" clause we proxy
/// here via Baned's flat penalty). The scroll bypasses concentration
/// and bakes in the Baned proxy so dispel sweeps can strip it cleanly.
/// Sibling to Scroll of Bane (burst Baned at DC 13) on the Baned lane —
/// distinct from the burst-scroll by the single-target shape and the
/// meaner DC 15 tier. Routes through the shared `SingleSaveConditionItem`
/// impl.
pub static READ_BESTOW_CURSE_SCROLL: SingleSaveConditionItem = SingleSaveConditionItem {
    action_name: "read bestow curse scroll",
    action_aliases: &["bestow curse scroll", "curse scroll"],
    item_name: SCROLL_OF_BESTOW_CURSE_NAME,
    log_text: "{actor} reads a scroll of bestow curse; a malign whisper coils around the target.",
    save: AbilityScoreType::Wisdom,
    dc: 15,
    // Touch RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    condition: Condition::Baned,
    timer: ConditionTimer::Rounds(10),
};

/// Scroll of Conjure Animals — Action; summons two spectral wolves on
/// the caster's team adjacent to them, concentration-bound. 5e RAW:
/// Conjure Animals is a level-3 conjuration; the scroll surfaces the
/// same envelope without needing a slot. Validates that the caster
/// still has at least one free adjacent slot for a Medium wolf
/// footprint (the second wolf is best-effort). On success the scroll
/// is consumed and the caster takes concentration tracking the
/// `Conjured` flag on each summon — dropping concentration despawns
/// the cohort via the shared cleanup path. Distinct from `ANIMATE_DEAD`
/// (no concentration, skeleton team-permanent minion) in that the
/// scroll's wolves are concentration-bound and vanish when the spell
/// ends.
pub static READ_CONJURE_ANIMALS_SCROLL: ReadConjureAnimalsScrollItem = ReadConjureAnimalsScrollItem {};

/// Scroll of Longstrider — Action; install `Longstriding` (+10 ft
/// walking speed) for 100 rounds on a single ally within 1 tile (touch
/// RAW). 5e RAW: Longstrider is a level-1 transmutation; the scroll
/// surfaces the Tasha's-tier mobility buff without needing a slot.
/// Sibling to `READ_FLY_SCROLL` on the movement-buff lane — distinct
/// from Fly by sitting at the lower tier (speed bump rather than
/// vertical lift). Rejects re-cast when the target is already
/// Longstriding so the consumable isn't burned on a no-op refresh.
/// Fires through the shared `SingleTargetBuffItem` impl.
pub static READ_LONGSTRIDER_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read longstrider scroll",
    action_aliases: &["longstrider scroll", "scroll longstrider"],
    item_name: SCROLL_OF_LONGSTRIDER_NAME,
    log_text: "{actor} reads a scroll of longstrider; their target's stride lengthens.",
    condition: Condition::Longstriding,
    timer: ConditionTimer::Rounds(100),
    // Touch RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Barkskin — Action; install `Barkskinned` (AC floor of 16
/// while active — see the condition impl for the exact math) for 100
/// rounds on a single ally within 1 tile (touch RAW). 5e RAW: Barkskin
/// is a level-2 transmutation, concentration; the scroll bypasses
/// concentration and surfaces the AC-floor envelope as a fire-and-
/// forget ally buff. Common ranger / druid trinket — pairs naturally
/// with `READ_LONGSTRIDER_SCROLL` for a low-tier mobility + defense
/// kit. Rejects re-cast when the target is already Barkskinned so
/// the consumable isn't burned on a no-op refresh.
pub static READ_BARKSKIN_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read barkskin scroll",
    action_aliases: &["barkskin scroll", "scroll barkskin"],
    item_name: SCROLL_OF_BARKSKIN_NAME,
    log_text: "{actor} reads a scroll of barkskin; their target's hide hardens like oak.",
    condition: Condition::Barkskinned,
    timer: ConditionTimer::Rounds(100),
    // Touch RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

pub struct ReadConjureAnimalsScrollItem {}

impl Action for ReadConjureAnimalsScrollItem {
    /// Queues a `StartConcentration`. Declared so the AI's
    /// summon and area-control rungs can price this cast before
    /// trading a landed concentration effect for an unlanded one
    /// — and so the assertion in `Action::execute` stays quiet.
    fn holds_concentration(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "read conjure animals scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["conjure scroll", "wolf scroll"]
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

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, SCROLL_OF_CONJURE_ANIMALS_NAME) {
            return false;
        }
        // At least one free Medium slot adjacent to the caster.
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 3)
            .is_some()
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::wolves::WOLF_TEMPLATE;
        use crate::engine::side_effects::StartConcentration;
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_CONJURE_ANIMALS_NAME) {
            return Vec::new();
        }
        // Reuse the same spawn → tag → concentration path the spell-
        // side `CONJURE_ANIMALS.side_effects` uses. The scroll fires
        // the spell envelope without spending a slot.
        let team = match encounter.actors.get(&caster_id) {
            Some(c) => c.team(),
            None => return Vec::new(),
        };
        let mut spawned: Vec<usize> = Vec::new();
        for _ in 0..2 {
            let Some(anchor) = encounter
                .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 3)
            else {
                break;
            };
            match encounter.instantiate_creature(&WOLF_TEMPLATE, anchor, team, 90 + spawned.len())
            {
                Ok(new_id) => {
                    encounter.log(format!(
                        "  scroll of conjure animals: a spectral wolf appears at {} (actor #{})",
                        anchor, new_id
                    ));
                    spawned.push(new_id);
                }
                Err(e) => {
                    encounter.log(format!("  scroll of conjure animals failed: {}", e));
                    break;
                }
            }
        }
        if spawned.is_empty() {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tags = Vec::new();
        for id in &spawned {
            effects.push(Box::new(ApplyCondition {
                actor_id: *id,
                condition: Condition::Conjured,
                timer: ConditionTimer::Rounds(100),
            }));
            tags.push((*id, Condition::Conjured));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Scroll of Conjure Animals", tags),
        }));
        effects
    }
}

/// Scroll of Magnify Gravity — Action; 1-tile burst (5 ft RAW), STR save
/// vs DC 13, save-for-half 2d8 force damage. 5e RAW (TCE): the spell also
/// halves failed-save targets' speed until the end of the caster's next
/// turn; the scroll variant collapses to damage-only since the engine's
/// `BurstSaveDamageItem` chassis is the cleanest factor for the shape.
/// The follow-up Slowed rider sits exclusively on the spell-side
/// `MAGNIFY_GRAVITY` impl — accepting the small consumable / spell delta
/// keeps the shared scroll factor on its single-effect chokepoint
/// (mirrors how `READ_PYROTECHNICS_SCROLL` drops the spell's Blinded
/// rider to stay on the damage-only `BurstSaveDamageItem` lane).
/// Friend-or-foe agnostic via `resolve_burst_save_damage` (the gravity
/// well doesn't discriminate); 24-tile reach matches the spell.
pub static READ_MAGNIFY_GRAVITY_SCROLL: BurstSaveDamageItem = BurstSaveDamageItem {
    action_name: "read magnify gravity scroll",
    action_aliases: &["magnify gravity scroll", "gravity scroll", "mg scroll"],
    item_name: SCROLL_OF_MAGNIFY_GRAVITY_NAME,
    log_label: "scroll of magnify gravity",
    dice: crate::engine::dice::Dice::new(2, 8),
    damage_type: DamageType::Force,
    save: AbilityScoreType::Strength,
    dc: 13,
    radius: 1,
    // 60 ft RAW = 24 tiles.
    reach: 24,
};

/// Scroll of Elemental Weapon — Action; install `ElementallyWeaponed` for
/// 100 rounds (≈ 10 minutes engine time) on a single ally within 1 tile
/// (touch RAW). 5e RAW: Elemental Weapon is a level-3 transmutation,
/// concentration; the scroll bypasses concentration and surfaces the
/// per-hit damage rider (the `ON_HIT_RIDERS` entry adds +1d4 fire per
/// melee weapon hit) as a fire-and-forget buff. The spell's +1 attack-
/// roll bonus is dropped on the scroll variant since the `SingleTargetBuffItem`
/// chassis is condition-only — accepting the small consumable / spell delta
/// keeps the shared factor on its single-effect chokepoint (mirrors how
/// `READ_BARKSKIN_SCROLL` drops the spell's concentration anchor while
/// keeping the load-bearing AC-floor condition). Rejects re-cast when
/// the target already carries the buff so the consumable isn't burned on
/// a no-op refresh.
pub static READ_ELEMENTAL_WEAPON_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read elemental weapon scroll",
    action_aliases: &["elemental weapon scroll", "ew scroll", "scroll ew"],
    item_name: SCROLL_OF_ELEMENTAL_WEAPON_NAME,
    log_text: "{actor} reads a scroll of elemental weapon; flickering flames sheathe their target's blade.",
    condition: Condition::ElementallyWeaponed,
    timer: ConditionTimer::Rounds(100),
    // Touch RAW; 1 tile.
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of True Seeing — Action; install `TrueSighted` for ~100 rounds
/// (≈ 1 hour engine time) on a single ally within 1 tile (touch RAW).
/// 5e RAW: True Seeing is a level-6 divination (no concentration); the
/// scroll surfaces the buff envelope without a slot cost. The condition
/// is read by `compute_attack_mode` via the `countered_by_truesight`
/// cohort: holder ignores the target's `Invisible` / `Blurred` /
/// `Displaced` attack-mode disadvantage, and an attacker can't ride
/// their own invisibility advantage against the holder. Slots into the
/// touch-ally-buff scroll lane next to Scroll of Death Ward / Scroll of
/// Barkskin — the anti-illusion defensive option.
pub static READ_TRUE_SEEING_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read true seeing scroll",
    action_aliases: &["true seeing scroll", "ts scroll", "scroll truesight"],
    item_name: SCROLL_OF_TRUE_SEEING_NAME,
    log_text: "{actor} reads a scroll of true seeing; their target's vision pierces every veil.",
    condition: Condition::TrueSighted,
    timer: ConditionTimer::Rounds(100),
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

/// Scroll of Protection from Poison — Action; install `Purified` for
/// ~100 rounds (≈ 1 hour engine time) on a single ally within 1 tile
/// (touch RAW). 5e RAW: Protection from Poison is a level-2 abjuration
/// (no concentration), and the spell-side `PROTECTION_FROM_POISON` impl
/// also cleanses any active `Poisoned` install. The scroll variant
/// drops the cleanse rider to fit the `SingleTargetBuffItem` chassis —
/// the lingering `Purified` buff still carries poison-damage resistance
/// and poison/charm/frightened dynamic-immunity coverage. Sibling to
/// Scroll of Death Ward / Scroll of Barkskin on the touch-ally
/// defensive-buff lane; distinguished by the poison-focused envelope.
pub static READ_PROTECTION_FROM_POISON_SCROLL: SingleTargetBuffItem = SingleTargetBuffItem {
    action_name: "read protection from poison scroll",
    action_aliases: &["protection from poison scroll", "pfp scroll", "scroll antitoxin"],
    item_name: SCROLL_OF_PROTECTION_FROM_POISON_NAME,
    log_text: "{actor} reads a scroll of protection from poison; their target's veins glimmer with cleansing light.",
    condition: Condition::Purified,
    timer: ConditionTimer::Rounds(100),
    reach: crate::actions::action_template::MELEE_REACH,
    bonus_action: false,
    reject_when_active: true,
};

const TORCH_NAME: &str = "Torch";

/// **Light a torch** — the non-caster's answer to an unlit board.
///
/// 5e's torch is a 5-piece of adventuring gear that "burns for 1 hour,
/// providing bright light in a 20-foot radius and dim light for an
/// additional 20 feet", and until the lighting layer existed there was
/// nothing in the engine for it to provide. It is deliberately the one
/// light source that costs no spell slot and no cantrip known: a
/// fighter, a barbarian and a rogue between them have no way to make
/// light, and a dark encounter that only casters can see in would be a
/// worse encounter rather than a harder one.
///
/// Its own struct rather than a `SelfConditionItem` row because what it
/// installs is not a condition. Light is a property of tiles, held on
/// the light layer, and the actor merely carries the anchor — the same
/// distinction that keeps `zones` off the condition map.
///
/// Consumed on use, like every other one-shot in the pack. RAW's hour
/// outlasts any fight, so the source it lights carries no timer; see
/// `LightSource::rounds_remaining`.
pub struct LightTorch {}

impl Action for LightTorch {
    fn name(&self) -> &str {
        "light torch"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["torch", "lt torch", "kindle"]
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
        e: &EncounterInstance,
        c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // A bonus action for everybody rather than an Action. RAW's
        // "interact with one object for free" is the rule a torch is
        // actually lit under at a table, and the engine has no free
        // interaction to spend; a bonus action is the cheapest price it
        // can express, and charging a whole Action for the ability to
        // see would make the item not worth carrying.
        item_use_cost(e, c, true)
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same two refusals the Light cantrip makes, and for the same
        // reasons: don't light a second torch, and don't light one at
        // noon. The third is the one every consumable makes — you have
        // to still be holding it.
        caster_holds(encounter, caster_id, TORCH_NAME)
            && !encounter.actor_carries_light(caster_id)
            && encounter.ambient_light().level() != crate::engine::lighting::LightLevel::Bright
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::lighting::{
            LightAnchor, LightSource, TORCH_BRIGHT_TILES, TORCH_DIM_TILES,
        };
        if !consume_caster_item(encounter, caster_id, TORCH_NAME) {
            return Vec::new();
        }
        encounter.add_light_source(LightSource {
            id: 0,
            name: "torch",
            anchor: LightAnchor::Carried(caster_id),
            bright_tiles: TORCH_BRIGHT_TILES,
            dim_tiles: TORCH_DIM_TILES,
            rounds_remaining: None,
            // Nonmagical flame, and therefore the first thing a
            // Darkness sphere puts out.
            spell_level: 0,
            // A spell's light and a lit torch are both things a
            // creature carries, not things it is. See
            // `LightSource::innate`.
            innate: false,
        });
        let name = encounter.actor_name(caster_id);
        encounter.log(format!("{} lights a torch.", name));
        Vec::new()
    }
}

pub static LIGHT_TORCH: LightTorch = LightTorch {};
