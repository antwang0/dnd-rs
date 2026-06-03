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
