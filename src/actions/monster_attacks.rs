use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::{Dice, Roller},
        encounter::EncounterInstance,
        side_effects::{DealDamage, Resource},
        types::{Coordinate, DamageType},
        util::modifier_from_score,
    },
};

pub static SLAM: LazyLock<Slam> = LazyLock::new(|| Slam {});

/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// Reach is in tiles (not feet); 20 tiles = 50ft on this 2.5ft grid, which is
/// short of the 5e 80/320 normal/long range but plenty for our 40×20 maps.
pub struct Longbow {}

impl Action for Longbow {
    fn name(&self) -> &str {
        "longbow"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bow", "shoot"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(20)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Option<Resource> {
        Some(Resource::Action)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex = caster.ability_score(crate::engine::types::AbilityScoreType::Dexterity);
        // Bows use DEX for both attack and damage in 5e (finesse / ranged).
        let attack_bonus = modifier_from_score(dex);
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        weapon_attack(
            encounter,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(1, 8),
            modifier_from_score(dex),
            DamageType::Piercing,
        )
    }
}

pub static LONGBOW: LazyLock<Longbow> = LazyLock::new(|| Longbow {});

pub struct Slam {}

impl Action for Slam {
    fn name(&self) -> &str {
        "slam"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["slm"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Option<Resource> {
        Some(Resource::Action)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        weapon_attack(
            encounter,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(2, 6),
            str_mod,
            DamageType::Bludgeoning,
        )
    }
}

/// Roll a d20 attack against `target_ac`, log the breakdown, and on a hit
/// roll `damage_dice + damage_bonus` of `damage_type` against `target_id`.
/// Returns the side-effect vec (empty on miss). Centralizes the pattern so
/// every weapon-style attack logs in the same shape.
#[allow(clippy::too_many_arguments)]
fn weapon_attack(
    encounter: &mut EncounterInstance,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    target_ac: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    let raw_attack = encounter.roller.roll(&Dice::new(1, 20)) as i32;
    let attack_total = raw_attack + attack_bonus;
    let hit = attack_total >= target_ac;
    encounter.log(format!(
        "  {}: 1d20({}){:+} = {} vs AC {} \u{2014} {}",
        action_name,
        raw_attack,
        attack_bonus,
        attack_total,
        target_ac,
        if hit { "hit" } else { "miss" },
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roller.roll(&damage_dice) as i32;
    let damage = (raw_damage + damage_bonus).max(0) as u32;
    encounter.log(format!(
        "  {}: {}({}){:+} = {} {:?} damage",
        action_name, damage_dice, raw_damage, damage_bonus, damage, damage_type,
    ));
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: damage,
        damage_type,
    })]
}
