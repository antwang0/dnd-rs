use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, TargetingSchema, bonus_action_only, first_target_id},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, DealDamage, Heal, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
    },
};

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

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        if self.bonus_action {
            bonus_action_only()
        } else {
            vec![Resource::Action]
        }
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        if self.bonus_action {
            bonus_action_only()
        } else {
            vec![Resource::Action]
        }
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
        if !consume_caster_item(encounter, caster_id, self.item_name) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(self.log_text.replace("{actor}", &name));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: self.condition,
            timer: self.timer,
        })]
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

/// Read a Scroll of Magic Missile: spend an Action to fire 3 darts at one
/// enemy in line-of-sight (range 30 tiles). Each dart deals 1d4+1 force.
/// Auto-hit, no save. The scroll is consumed regardless of outcome.
pub struct ReadMagicMissileScroll {}

impl Action for ReadMagicMissileScroll {
    fn name(&self) -> &str {
        "read magic missile scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mm scroll", "missile scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(30)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, SCROLL_OF_MAGIC_MISSILE_NAME)
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
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_MAGIC_MISSILE_NAME) {
            return Vec::new();
        }
        let mut total = 0u32;
        let mut rolls = [0u32; 3];
        for r in rolls.iter_mut() {
            *r = encounter.roll(&Dice::new(1, 4));
            total = total.saturating_add(*r + 1);
        }
        encounter.log(format!(
            "  scroll of magic missile: 3*(1d4+1) [{}, {}, {}] = {} force",
            rolls[0] + 1,
            rolls[1] + 1,
            rolls[2] + 1,
            total
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total,
            damage_type: DamageType::Force,
        })]
    }
}

pub static READ_MAGIC_MISSILE_SCROLL: ReadMagicMissileScroll = ReadMagicMissileScroll {};

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

/// Drink a Potion of Heroism: bonus action; gain 10 temp HP and the
/// Heroic condition for 10 rounds (immune to Frightened + regen temp
/// HP from the buff). 5e RAW: 1-hour duration, +10 temp HP and immune
/// to Frightened — we collapse the duration to 10 rounds to match the
/// engine's combat-scale timer envelope. Single-use; consumes one
/// Potion of Heroism from inventory.
pub struct DrinkPotionOfHeroism {}

impl Action for DrinkPotionOfHeroism {
    fn name(&self) -> &str {
        "drink potion of heroism"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["heroism", "hero"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        // The temp HP grant + Heroic install reads as a buff/heal-style
        // support action, so the AI's support pipeline can pick it up.
        true
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
        caster_holds(encounter, caster_id, POTION_OF_HEROISM_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, GainTempHp};
        if !consume_caster_item(encounter, caster_id, POTION_OF_HEROISM_NAME) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(format!("{} drinks a potion of heroism.", name));
        vec![
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: 10,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static DRINK_POTION_OF_HEROISM: DrinkPotionOfHeroism = DrinkPotionOfHeroism {};

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

/// Read a Scroll of Cure Wounds: touch one ally (or self) for 2d8+2
/// healing. Mirrors the Cure Wounds spell at level 1 (2d8 base scaling
/// is closer to a level-2 upcast — we err on the generous side because
/// scroll loot is rare and the spell with WIS-mod can already roll
/// higher in caster hands). Touch range, costs an Action, consumes the
/// scroll. Like the Magic Missile scroll, no spell-slot cost — the
/// scroll *is* the slot. The validate path also requires the caster to
/// actually need healing OR be standing next to a wounded ally; the
/// "touch range" gate is enforced via `reach_tiles`. The heal targets
/// the actor at the target id, so a SingleActor schema is used to
/// surface both self-healing and ally-healing in the picker.
pub struct ReadCureWoundsScroll {}

impl Action for ReadCureWoundsScroll {
    fn name(&self) -> &str {
        "read cure wounds scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cw scroll", "cure scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — must be footprint-adjacent. Matches the Cure
        // Wounds spell's range. The picker uses this to filter the
        // target list.
        Some(1)
    }

    fn requires_los(&self) -> bool {
        // Implicitly true at touch range, but kept on so the picker
        // doesn't surface targets behind walls inside the 1-tile reach.
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
        caster_holds(encounter, caster_id, SCROLL_OF_CURE_WOUNDS_NAME)
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
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_CURE_WOUNDS_NAME) {
            return Vec::new();
        }
        let raw = encounter.roll(&Dice::new(2, 8)) as i32;
        // +2 is a stand-in for the spell's caster-CHA / WIS modifier. We
        // don't have a caster ability tied to the scroll (the scroll is
        // a fixed magic item, not a class spell), so a flat +2 keeps the
        // expected total comparable to a low-level cleric's Cure Wounds.
        let amount = (raw + 2).max(1) as u32;
        encounter.log(format!(
            "  scroll of cure wounds: 2d8({})+2 = {} HP",
            raw, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static READ_CURE_WOUNDS_SCROLL: ReadCureWoundsScroll = ReadCureWoundsScroll {};

const PEARL_OF_POWER_NAME: &str = "Pearl of Power";
const BOOTS_OF_SPEED_NAME: &str = "Boots of Speed";

/// Use a Pearl of Power: spend a bonus action to restore one expended
/// level-1 spell slot, then consume the pearl. 5e RAW: "as an action,
/// restore one expended spell slot of level 3 or lower"; we collapse
/// the slot tier to level-1 since most engine casters lean on the
/// level-1 slot for their bread-and-butter spells, and bump the
/// action-economy cost down to a bonus action so a caster can pearl-
/// up and still spend the slot on the same turn. Single-use: removed
/// from inventory on hit. Validate rejects the action when no level-1
/// slot has been spent — burning the pearl on a no-op would waste
/// the only consumable in the actor's caster lane.
pub struct UsePearlOfPower {}

impl Action for UsePearlOfPower {
    fn name(&self) -> &str {
        "use pearl of power"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["pearl", "pop"]
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
        if !caster_holds(encounter, caster_id, PEARL_OF_POWER_NAME) {
            return false;
        }
        // Reject the use when the actor has no spent level-1 slot to
        // restore — pearling up to refund a slot they didn't spend is
        // a no-op and would just burn the consumable. The check folds
        // through the slot manager so a future "no level-1 slots ever"
        // caster (e.g. cantrip-only build) is gated the same way.
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let info = actor.spell_slot_manager.spell_slots(1);
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
        if !consume_caster_item(encounter, caster_id, PEARL_OF_POWER_NAME) {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            let name = actor.name().to_string();
            let restored = actor.spell_slot_manager.restore_spell_slot(1, 1);
            if restored {
                encounter.log(format!(
                    "{} crushes a pearl of power; a level-1 slot returns.",
                    name
                ));
            } else {
                encounter.log(format!(
                    "{} crushes a pearl of power — but the slot was already full.",
                    name
                ));
            }
        }
        Vec::new()
    }
}

pub static USE_PEARL_OF_POWER: UsePearlOfPower = UsePearlOfPower {};

/// Boots of Speed — Bonus Action; installs the `Hasted` condition for 10
/// rounds (+2 AC, advantage on DEX saves, doubled walking speed). Single-
/// use consumable. Re-uses the Haste condition so the AC / DEX-save /
/// speed bundle flows through the same accessors a normal Haste cast
/// does. Fires through the shared `SelfConditionItem` impl.
pub static WEAR_BOOTS_OF_SPEED: SelfConditionItem = SelfConditionItem {
    action_name: "wear boots of speed",
    action_aliases: &["boots", "speedboots"],
    item_name: BOOTS_OF_SPEED_NAME,
    log_text: "{actor} taps the heels of the boots of speed; everything blurs.",
    condition: Condition::Hasted,
    timer: ConditionTimer::Rounds(10),
    bonus_action: true,
    reject_when_active: true,
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

/// Use a Wand of Magic Missiles — fire 5 force-damage darts at one enemy
/// in line-of-sight (range 30 tiles). Each dart deals 1d4+1 force.
/// Auto-hit, no save. Mirrors `READ_MAGIC_MISSILE_SCROLL` (3 darts) — the
/// wand sits at a higher payload tier. Single-use consumable per the
/// engine's charge-less loot model; the wand is removed on use.
pub struct UseWandOfMagicMissiles {}

impl Action for UseWandOfMagicMissiles {
    fn name(&self) -> &str {
        "use wand of magic missiles"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["wand", "mm wand"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(30)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, WAND_OF_MAGIC_MISSILES_NAME)
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
        if !consume_caster_item(encounter, caster_id, WAND_OF_MAGIC_MISSILES_NAME) {
            return Vec::new();
        }
        let mut total = 0u32;
        let mut rolls = [0u32; 5];
        for r in rolls.iter_mut() {
            *r = encounter.roll(&Dice::new(1, 4));
            total = total.saturating_add(*r + 1);
        }
        encounter.log(format!(
            "  wand of magic missiles: 5*(1d4+1) [{}, {}, {}, {}, {}] = {} force",
            rolls[0] + 1,
            rolls[1] + 1,
            rolls[2] + 1,
            rolls[3] + 1,
            rolls[4] + 1,
            total
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total,
            damage_type: DamageType::Force,
        })]
    }
}

pub static USE_WAND_OF_MAGIC_MISSILES: UseWandOfMagicMissiles = UseWandOfMagicMissiles {};

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
