use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
use crate::engine::types::DamageType;

/// Inputs to a single attack roll. Lets callers describe attacks without
/// repeating the d20 / crit / damage / log dance for every weapon and
/// damage cantrip.
pub struct AttackParams<'a> {
    pub caster_id: usize,
    pub target_id: usize,
    pub action_name: &'a str,
    /// Static attack-roll modifier (STR/DEX for weapons, INT/WIS/CHA for
    /// spells). Caster-side flat buffs (Bless, etc.) are folded in by
    /// `resolve_attack` itself — pass only the action's own bonus here.
    pub attack_bonus: i32,
    pub damage_dice: Dice,
    /// Static damage-roll modifier (STR/DEX/spellcasting mod). Added once
    /// per swing even on a crit.
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    /// `true` for melee weapons / touch spells, `false` for ranged
    /// attacks. Drives the prone-target advantage / disadvantage clause
    /// in `compute_attack_mode`.
    pub is_melee: bool,
}

/// Resolve a 5e d20 attack roll against a single target's AC. On a hit,
/// returns a `DealDamage` side-effect for the rolled damage; on a miss,
/// returns an empty vec. The d20 result, hit/miss outcome, and damage
/// breakdown are logged in the engine's standard shape.
///
/// Critical hits (natural 20 after advantage/disadvantage) auto-hit and
/// double the damage dice (the modifier is added once). Caster's flat
/// `attack_bonus_buff` (Bless, etc.) is added at roll time so a buff that
/// landed mid-multiattack still picks up later swings correctly.
pub fn resolve_attack(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(target_ac) = encounter
        .actors
        .get(&p.target_id)
        .map(|a| a.armor_class() as i32)
    else {
        return Vec::new();
    };

    let mode = encounter.compute_attack_mode(p.caster_id, p.target_id, p.is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    let buff = encounter
        .actors
        .get(&p.caster_id)
        .map(|a| a.attack_bonus_buff())
        .unwrap_or(0);
    // Bless: roll an actual 1d4 once per attack and add to total. We
    // log the d4 separately so the player can see why the d20 alone
    // doesn't account for the swing's hit.
    let bless_die: i32 = if encounter
        .actors
        .get(&p.caster_id)
        .is_some_and(|a| a.is_blessed())
    {
        encounter.roll(&Dice::new(1, 4)) as i32
    } else {
        0
    };
    let attack_total = raw_attack + p.attack_bonus + buff + bless_die;
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let bless_note = if bless_die > 0 {
        format!(" + bless(1d4={})", bless_die)
    } else {
        String::new()
    };
    encounter.log(format!(
        "  {}: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
        p.action_name,
        raw_attack,
        p.attack_bonus + buff,
        bless_note,
        attack_total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roll(&p.damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&p.damage_dice) as i32
    } else {
        0
    };
    let damage = (raw_damage + crit_extra + p.damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            p.action_name,
            p.damage_dice,
            raw_damage,
            p.damage_dice,
            crit_extra,
            p.damage_bonus,
            damage,
            p.damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            p.action_name, p.damage_dice, raw_damage, p.damage_bonus, damage, p.damage_type,
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: p.target_id,
        amount: damage,
        damage_type: p.damage_type,
    })]
}
