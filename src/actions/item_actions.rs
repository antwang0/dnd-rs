use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, TargetingSchema, bonus_action_only, first_target_id},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Heal, Resource},
        types::{Coordinate, DamageType},
    },
};

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

/// Drink a Potion of Healing. Self-targeted, costs an Action, heals
/// 2d4+2 and removes one potion from inventory. The validate hook
/// rejects the action if the caster has no potion left (so a duplicate
/// "drink" entry with zero stock can't fire), and the side-effects
/// step pops one potion before the Heal applies.
pub struct DrinkHealingPotion {}

impl Action for DrinkHealingPotion {
    fn name(&self) -> &str {
        "drink healing potion"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["potion", "drink"]
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

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, POTION_OF_HEALING_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(2, 4)) as i32;
        let amount = (raw + 2).max(1) as u32;
        // Pop the potion *now* — if the action was queued, validate
        // already confirmed at least one was carried, and consuming
        // before the Heal side-effect runs keeps inventory consistent
        // even if the heal somehow fails (e.g. caster died mid-stack).
        if !consume_caster_item(encounter, caster_id, POTION_OF_HEALING_NAME) {
            return Vec::new();
        }
        encounter.log(format!(
            "  potion of healing: 2d4({}){:+} = {} HP",
            raw, 2, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static DRINK_HEALING_POTION: DrinkHealingPotion = DrinkHealingPotion {};

/// Drink a Potion of Greater Healing. Self-targeted, costs an Action,
/// heals 4d4+4. Same lifecycle as the standard healing potion (validate
/// requires the item in inventory; remove on use).
pub struct DrinkGreaterHealingPotion {}

impl Action for DrinkGreaterHealingPotion {
    fn name(&self) -> &str {
        "drink greater healing potion"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["potion+", "drink+"]
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

    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Greater Healing is a bonus action — distinct from the regular
        // Healing Potion's full Action cost. Lets a wounded martial drink
        // and still swing in the same turn.
        vec![Resource::BonusAction]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, POTION_OF_GREATER_HEALING_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(4, 4)) as i32;
        let amount = (raw + 4).max(1) as u32;
        if !consume_caster_item(encounter, caster_id, POTION_OF_GREATER_HEALING_NAME) {
            return Vec::new();
        }
        encounter.log(format!(
            "  potion of greater healing: 4d4({})+4 = {} HP",
            raw, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static DRINK_GREATER_HEALING_POTION: DrinkGreaterHealingPotion = DrinkGreaterHealingPotion {};

/// Read a Scroll of Fireball: pick a target tile, every actor whose
/// footprint touches the burst takes 6d6 fire on a failed DEX save vs
/// DC 15, half on success. Consumes the scroll. No spell-slot cost
/// (the scroll *is* the slot).
pub struct ReadFireballScroll {}

impl Action for ReadFireballScroll {
    fn name(&self) -> &str {
        "read fireball scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fireball", "scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft range — well past any current map.
        Some(60)
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
        caster_holds(encounter, caster_id, SCROLL_OF_FIREBALL_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::saves::SaveOutcome;
        use crate::engine::types::AbilityScoreType;

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_FIREBALL_NAME) {
            return Vec::new();
        }

        // Roll damage once and share across the burst so everyone in the
        // blast takes the same number — matches Sacred Burst's pattern.
        let damage = encounter.roll(&Dice::new(6, 6));
        encounter.log(format!("  scroll of fireball: 6d6 = {} damage", damage));

        const BLAST_RADIUS: isize = 4;
        let dc: i32 = 15;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in encounter.actors_in_burst(center, BLAST_RADIUS) {
            let outcome = encounter.roll_save(id, AbilityScoreType::Dexterity, dc);
            let final_damage = match outcome {
                SaveOutcome::Pass => damage / 2,
                SaveOutcome::Fail => damage,
            };
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: final_damage,
                damage_type: DamageType::Fire,
            }));
        }
        effects
    }
}

pub static READ_FIREBALL_SCROLL: ReadFireballScroll = ReadFireballScroll {};

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

/// Drink a Potion of Invisibility: action; gain the Invisible condition
/// for 10 rounds (a flat duration close to RAW's "1 hour or until you
/// attack/cast"). The Invisible condition gives the holder advantage on
/// their next attack and imposes disadvantage on attackers, with the
/// flag dropping on attack via the standard `breaks_on_attack`
/// concentration-style hook (we use a Rounds timer here since the
/// potion isn't a concentration spell — the attack-clear semantics
/// live elsewhere for spell Invisibility). Single-use; consumes one
/// Potion of Invisibility from inventory.
pub struct DrinkPotionOfInvisibility {}

impl Action for DrinkPotionOfInvisibility {
    fn name(&self) -> &str {
        "drink potion of invisibility"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["invisibility", "invis"]
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
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        caster_holds(encounter, caster_id, POTION_OF_INVISIBILITY_NAME)
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
        use crate::engine::side_effects::ApplyCondition;
        if !consume_caster_item(encounter, caster_id, POTION_OF_INVISIBILITY_NAME) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(format!("{} drinks a potion of invisibility.", name));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Invisible,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static DRINK_POTION_OF_INVISIBILITY: DrinkPotionOfInvisibility =
    DrinkPotionOfInvisibility {};

const SCROLL_OF_LIGHTNING_BOLT_NAME: &str = "Scroll of Lightning Bolt";

pub struct ReadLightningBoltScroll {}

impl Action for ReadLightningBoltScroll {
    fn name(&self) -> &str {
        "read lightning bolt scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["lb scroll", "lightning scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(40)
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
        caster_holds(encounter, caster_id, SCROLL_OF_LIGHTNING_BOLT_NAME)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::saves::SaveOutcome;
        use crate::engine::types::AbilityScoreType;

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_LIGHTNING_BOLT_NAME) {
            return Vec::new();
        }

        let damage = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!("  scroll of lightning bolt: 8d6 = {} damage", damage));

        const BLAST_RADIUS: isize = 2;
        let dc: i32 = 15;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in encounter.actors_in_burst(center, BLAST_RADIUS) {
            let outcome = encounter.roll_save(id, AbilityScoreType::Dexterity, dc);
            let final_damage = match outcome {
                SaveOutcome::Pass => damage / 2,
                SaveOutcome::Fail => damage,
            };
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: final_damage,
                damage_type: DamageType::Lightning,
            }));
        }
        effects
    }
}

pub static READ_LIGHTNING_BOLT_SCROLL: ReadLightningBoltScroll = ReadLightningBoltScroll {};

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

/// Wear (activate) Boots of Speed: bonus action; gain the `Hasted`
/// condition (+2 AC, advantage on DEX saves, doubled walking speed)
/// for 10 rounds. Single-use consumable — the boots are "spent" after
/// one click and removed from inventory. 5e RAW: 10 minutes per long
/// rest; we cap to combat-scale (10 rounds ≈ 1 minute) and drop the
/// rest cycle since the engine doesn't model multi-encounter rest.
/// Re-uses the Haste condition so the AC / DEX-save / speed bundle
/// flows through the same accessors a normal Haste cast does.
pub struct WearBootsOfSpeed {}

impl Action for WearBootsOfSpeed {
    fn name(&self) -> &str {
        "wear boots of speed"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["boots", "speedboots"]
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
        if !caster_holds(encounter, caster_id, BOOTS_OF_SPEED_NAME) {
            return false;
        }
        // Reject when the holder is already Hasted — installing on top
        // would just refresh the timer and burn the boots for the same
        // mechanical effect. Lets the validator silently no-op the use
        // until the existing Haste drops.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(crate::conditions::Condition::Hasted))
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
        use crate::engine::side_effects::ApplyCondition;
        if !consume_caster_item(encounter, caster_id, BOOTS_OF_SPEED_NAME) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(format!(
            "{} taps the heels of the boots of speed; everything blurs.",
            name
        ));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Hasted,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static WEAR_BOOTS_OF_SPEED: WearBootsOfSpeed = WearBootsOfSpeed {};

const SCROLL_OF_CONE_OF_COLD_NAME: &str = "Scroll of Cone of Cold";
const WAND_OF_MAGIC_MISSILES_NAME: &str = "Wand of Magic Missiles";

/// Read a Scroll of Cone of Cold — pick a target tile, every actor whose
/// footprint touches the burst takes 8d8 cold on a failed CON save vs
/// DC 15, half on success. Consumes the scroll on use. Mirrors the
/// Fireball / Lightning Bolt scrolls' shape with a different element,
/// save ability, and damage profile (CON instead of DEX; bigger d8 pool).
/// Routes through `resolve_burst_save_damage` so evasion / Careful Spell
/// / Heightened Spell shielding all fire through the same chokepoint.
pub struct ReadConeOfColdScroll {}

impl Action for ReadConeOfColdScroll {
    fn name(&self) -> &str {
        "read cone of cold scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["coc scroll", "cone scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        // Match the spell's 6-tile burst approximation of the 60-ft cone.
        TargetingSchema::Burst { radius: 6 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Self-cone in RAW; we cap the picker at the cone's reach (60 ft
        // = 24 tiles) so the targeting reticle doesn't drop across the
        // whole map. Mirrors `CONE_OF_COLD::reach_tiles`.
        Some(24)
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
        caster_holds(encounter, caster_id, SCROLL_OF_CONE_OF_COLD_NAME)
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
        use crate::engine::types::AbilityScoreType;

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, SCROLL_OF_CONE_OF_COLD_NAME) {
            return Vec::new();
        }

        let damage = encounter.roll(&Dice::new(8, 8));
        encounter.log(format!("  scroll of cone of cold: 8d8 = {} damage", damage));

        const BLAST_RADIUS: isize = 6;
        let dc: i32 = 15;
        resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            BLAST_RADIUS,
            AbilityScoreType::Constitution,
            dc,
            damage,
            DamageType::Cold,
        )
    }
}

pub static READ_CONE_OF_COLD_SCROLL: ReadConeOfColdScroll = ReadConeOfColdScroll {};

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

/// Drink a Potion of Flying — action; grants the holder the Flying
/// condition for 10 rounds (≈1 minute RAW, vs the 1-hour RAW timer; we
/// collapse to combat-scale per the engine's existing potion timer
/// envelope). Single-use; consumes one Potion of Flying from inventory.
/// Pairs with the existing Flying condition (which the Fly spell already
/// installs) so the AC / disadvantage-to-ranged-attackers / speed bump
/// flows through the same accessors a normal Fly cast does.
pub struct DrinkPotionOfFlying {}

impl Action for DrinkPotionOfFlying {
    fn name(&self) -> &str {
        "drink potion of flying"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fly", "flying"]
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
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !caster_holds(encounter, caster_id, POTION_OF_FLYING_NAME) {
            return false;
        }
        // Reject when the holder is already Flying — installing on top
        // would just refresh the timer and burn the potion for the same
        // mechanical effect. Mirrors Boots of Speed's "already Hasted"
        // gate.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(crate::conditions::Condition::Flying))
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
        use crate::engine::side_effects::ApplyCondition;
        if !consume_caster_item(encounter, caster_id, POTION_OF_FLYING_NAME) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(format!("{} drinks a potion of flying.", name));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Flying,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static DRINK_POTION_OF_FLYING: DrinkPotionOfFlying = DrinkPotionOfFlying {};

/// Drink a Potion of Climbing — bonus action; grants the holder the
/// `SpiderClimbing` condition for 10 rounds. Pairs with the existing
/// Spider Climb spell condition so the +12-tile speed bump flows through
/// the same accessor. Bonus-action cost (cheaper than the Flying
/// potion's Action cost) since climbing is a lesser mobility
/// envelope than full flight.
pub struct DrinkPotionOfClimbing {}

impl Action for DrinkPotionOfClimbing {
    fn name(&self) -> &str {
        "drink potion of climbing"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["climb", "climbing"]
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
        if !caster_holds(encounter, caster_id, POTION_OF_CLIMBING_NAME) {
            return false;
        }
        // Same "already up" gate as Potion of Flying / Boots of Speed:
        // re-drinking on top of an active climb refreshes the timer and
        // wastes the consumable.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(crate::conditions::Condition::SpiderClimbing))
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
        use crate::engine::side_effects::ApplyCondition;
        if !consume_caster_item(encounter, caster_id, POTION_OF_CLIMBING_NAME) {
            return Vec::new();
        }
        let name = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        encounter.log(format!("{} drinks a potion of climbing.", name));
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::SpiderClimbing,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static DRINK_POTION_OF_CLIMBING: DrinkPotionOfClimbing = DrinkPotionOfClimbing {};

/// Use a Wand of Fireballs — single-use 8d6 fire burst. Mirrors the
/// `READ_FIREBALL_SCROLL` shape (DEX save vs DC 15, halve on pass) but
/// with a bigger damage pool — the wand sits at the level-4 cast tier
/// versus the scroll's level-3 baseline. 5e RAW: the wand has 7 charges
/// and casts at level 3 (+1 per extra charge); we collapse to a single
/// 8d6 cast for the engine's charge-less loot model. Routes through
/// `resolve_burst_save_damage` so evasion / Careful Spell / Heightened
/// Spell shielding all fire through the same chokepoint.
pub struct UseWandOfFireballs {}

impl Action for UseWandOfFireballs {
    fn name(&self) -> &str {
        "use wand of fireballs"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fireballs", "fireball wand"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles, matching the Fireball scroll.
        Some(60)
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
        caster_holds(encounter, caster_id, WAND_OF_FIREBALLS_NAME)
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
        use crate::engine::types::AbilityScoreType;

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
            return Vec::new();
        };
        if !consume_caster_item(encounter, caster_id, WAND_OF_FIREBALLS_NAME) {
            return Vec::new();
        }

        let damage = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!("  wand of fireballs: 8d6 = {} damage", damage));

        const BLAST_RADIUS: isize = 4;
        let dc: i32 = 15;
        resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            BLAST_RADIUS,
            AbilityScoreType::Dexterity,
            dc,
            damage,
            DamageType::Fire,
        )
    }
}

pub static USE_WAND_OF_FIREBALLS: UseWandOfFireballs = UseWandOfFireballs {};
