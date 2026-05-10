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
    // Help is one-shot: a Helped attacker rolls with advantage on their
    // first attack, then the condition clears regardless of hit/miss so
    // a second swing in the same turn doesn't double-dip. We also drain
    // any matching grant from the help-grant map — both bookkeeping
    // tracks have to clear together so the AI's mode peek stays honest.
    //
    // Hidden also drops here: 5e RAW says making an attack reveals you,
    // whether or not the attack hits. We pick up the Hidden-attacker
    // advantage in `compute_attack_mode` above, then clear the condition
    // so a follow-up swing this turn doesn't double-dip.
    if let Some(attacker) = encounter.actors.get_mut(&p.caster_id) {
        attacker.remove_condition(crate::conditions::Condition::Helped);
        attacker.remove_condition(crate::conditions::Condition::Hidden);
        attacker.consume_help_for(p.target_id);
    }
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    let buff = encounter
        .actors
        .get(&p.caster_id)
        .map(|a| a.attack_bonus_buff())
        .unwrap_or(0);
    // Bless/Bane: roll an actual 1d4 once per attack and add (Bless) or
    // subtract (Bane) from the total. Both: they cancel and no die is
    // rolled. We log the d4 separately so the player can see why the
    // d20 alone doesn't account for the swing's hit.
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(p.caster_id);
    let attack_total = raw_attack + p.attack_bonus + buff + bless_die;
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
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
    let mut damage = (raw_damage + crit_extra + p.damage_bonus).max(0) as u32;
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
    // Hunter's Mark rider: attacker concentrating on Hunter's Mark with
    // this target marked deals +1d6 (weapon-typed). Crits double the
    // mark die per RAW.
    if encounter.is_hunters_mark_target(p.caster_id, p.target_id) {
        let hm_raw = encounter.roll(&Dice::new(1, 6));
        let hm_extra = if is_crit { encounter.roll(&Dice::new(1, 6)) } else { 0 };
        let hm_total = hm_raw + hm_extra;
        damage = damage.saturating_add(hm_total);
        encounter.log(format!(
            "  hunter's mark: 1d6({}) = {} extra {:?}",
            hm_raw, hm_total, p.damage_type
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: p.target_id,
        amount: damage,
        damage_type: p.damage_type,
    })]
}
