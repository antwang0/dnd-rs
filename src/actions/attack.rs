use crate::engine::{
    dice::Dice,
    encounter::EncounterInstance,
    side_effects::{ApplicableSideEffect, DealDamage},
    types::DamageType,
};

/// Roll a d20 attack against `target_ac`, log the breakdown, and on a hit
/// roll `damage_dice + damage_bonus` of `damage_type` against `target_id`.
/// `is_melee` drives Prone-target advantage / ranged disadvantage clauses
/// (forwarded to `compute_attack_mode`).
///
/// **Critical hits**: a final d20 of 20 (after advantage / disadvantage)
/// auto-hits regardless of AC and rolls the damage dice twice — the
/// modifier is added once. 5e RAW.
///
/// **Bless**: if the caster has the Blessed condition, +2 (average of
/// 1d4) is added to the attack-roll total and tagged in the log.
///
/// Returns the side-effect vec (empty on miss). Centralizes the pattern
/// so every weapon-style attack logs in the same shape; spells that
/// resolve as attack rolls (e.g. Eldritch Blast in the future) can call
/// this directly instead of restating the pipeline.
#[allow(clippy::too_many_arguments)]
pub fn weapon_attack(
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
    // Bless: +2 flat to attack rolls (we model the 1d4 as its average).
    let bless_bonus = encounter
        .actors
        .get(&caster_id)
        .map(|a| a.bless_bonus())
        .unwrap_or(0);
    let attack_total = raw_attack + attack_bonus + bless_bonus;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let bless_tag = if bless_bonus != 0 { " (bless)" } else { "" };
    encounter.log(format!(
        "  {}: 1d20({}){:+} = {} vs AC {}{}{} \u{2014} {}",
        action_name,
        raw_attack,
        attack_bonus + bless_bonus,
        attack_total,
        target_ac,
        mode.log_suffix(),
        bless_tag,
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
