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
    resolve_attack_outcome(encounter, p).0
}

/// Result of an attack roll. `damage_dealt` is the post-crit, pre-target-
/// resistance damage value that will hit the queue — `0` on a miss or when
/// a Mirror Image absorbed the swing. Use this variant when the caller
/// needs to chain off the rolled damage (e.g. a self-heal rider equal to
/// half the damage, or a max-HP drain equal to the damage on a failed
/// save) without re-rolling and double-consuming the RNG.
pub fn resolve_attack_outcome(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let Some(target_ac) = encounter
        .actors
        .get(&p.target_id)
        .map(|a| a.armor_class() as i32)
    else {
        return (Vec::new(), 0);
    };

    let mode = encounter.compute_attack_mode(p.caster_id, p.target_id, p.is_melee);
    // Burn through the one-shot rider stack (Helped, Hidden,
    // per-target help grant, Invisibility concentration) before the
    // d20 lands so a second swing this turn doesn't double-dip the
    // advantage. We pick up the Hidden / Helped / Invisible flags in
    // `compute_attack_mode` above, then this hook clears them.
    encounter.clear_attack_advantage_riders(p.caster_id, p.target_id);
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
        return (Vec::new(), 0);
    }
    // 5e Mirror Image: a hit may instead strike a decoy. With N duplicates
    // remaining, an extra d20 against the matching threshold (RAW: 6+
    // for 3, 8+ for 2, 11+ for 1) determines whether the swing pops a
    // decoy and misses the caster. Crits bypass the deflection.
    if !is_crit
        && let Some(target) = encounter.actors.get(&p.target_id)
        && target.mirror_images() > 0
    {
        let images = target.mirror_images();
        let dup_threshold = if images >= 3 {
            6
        } else if images == 2 {
            8
        } else {
            11
        };
        let dup_roll = encounter.roll(&Dice::new(1, 20)) as i32;
        if dup_roll >= dup_threshold {
            if let Some(t) = encounter.actors.get_mut(&p.target_id) {
                t.pop_mirror_image();
            }
            let remaining = encounter
                .actors
                .get(&p.target_id)
                .map(|a| a.mirror_images())
                .unwrap_or(0);
            encounter.log(format!(
                "  mirror image: 1d20({}) \u{2265} {} \u{2014} attack strikes a duplicate ({} left)",
                dup_roll, dup_threshold, remaining
            ));
            return (Vec::new(), 0);
        }
        encounter.log(format!(
            "  mirror image: 1d20({}) < {} \u{2014} attack finds the real target",
            dup_roll, dup_threshold
        ));
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
    // Hex rider: 1d6 necrotic on every hit against the Hexed target. Like
    // Hunter's Mark, crits double the rider die. The necrotic typing
    // matters more than HM's weapon-typed rider — it can chip past
    // physical resistance but bounces off necrotic-resistant undead.
    if encounter.is_hex_target(p.caster_id, p.target_id) {
        let hex_raw = encounter.roll(&Dice::new(1, 6));
        let hex_extra = if is_crit { encounter.roll(&Dice::new(1, 6)) } else { 0 };
        let hex_total = hex_raw + hex_extra;
        encounter.log(format!(
            "  hex: 1d6({}) = {} extra Necrotic",
            hex_raw, hex_total
        ));
        // Necrotic is a separate damage application so target resistances
        // / immunities apply correctly. The weapon hit still lands via
        // the DealDamage below.
        return (
            vec![
                Box::new(DealDamage {
                    actor_id: p.target_id,
                    amount: damage,
                    damage_type: p.damage_type,
                }),
                Box::new(DealDamage {
                    actor_id: p.target_id,
                    amount: hex_total,
                    damage_type: DamageType::Necrotic,
                }),
            ],
            damage,
        );
    }
    (
        vec![Box::new(DealDamage {
            actor_id: p.target_id,
            amount: damage,
            damage_type: p.damage_type,
        })],
        damage,
    )
}
