use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
use crate::engine::types::DamageType;

/// Roll a d20 attack against `target_ac`, log the breakdown, and on a hit
/// roll `damage_dice + damage_bonus` of `damage_type` against `target_id`.
/// `is_melee` drives advantage/disadvantage clauses against prone targets.
///
/// **Critical hits**: a final d20 of 20 (after advantage / disadvantage)
/// auto-hits regardless of AC and rolls the damage dice twice — the
/// modifier is added once. 5e RAW.
///
/// Returns the side-effect vec (empty on miss). Centralizes the pattern
/// so every weapon-style and spell-attack action logs in the same shape.
#[allow(clippy::too_many_arguments)]
pub fn resolve_d20_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    target_ac: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let mode = encounter.compute_attack_mode(caster_id, target_id, is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    let attack_total = raw_attack + attack_bonus;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    encounter.log(format!(
        "  {}: 1d20({}){:+} = {} vs AC {}{} \u{2014} {}",
        action_name,
        raw_attack,
        attack_bonus,
        attack_total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roll(&damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&damage_dice) as i32
    } else {
        0
    };
    let damage = (raw_damage + crit_extra + damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            action_name,
            damage_dice,
            raw_damage,
            damage_dice,
            crit_extra,
            damage_bonus,
            damage,
            damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            action_name, damage_dice, raw_damage, damage_bonus, damage, damage_type,
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: damage,
        damage_type,
    })]
}
