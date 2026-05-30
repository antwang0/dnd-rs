use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{
    ApplicableSideEffect, ApplyCondition, DealDamage, PushActor, SetGoadedBy,
};
use crate::engine::types::{AbilityScoreType, DamageType};

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
    /// 5e long-range threshold (in tiles). Ranged attacks beyond this
    /// distance impose disadvantage. `None` means no long-range penalty
    /// (melee weapons, spells). Set to the weapon's "normal range"
    /// converted to tiles — attacks between `long_range` and `reach`
    /// roll with disadvantage per RAW.
    pub long_range: Option<isize>,
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
    // 5e Cover: intervening combat-active creatures bump the target's
    // effective AC (+2 for half cover, +5 for three-quarters). Adjacent
    // melee swings are exempt (the cover routine returns 0 at gap ≤ 1).
    let cover_bonus = encounter.cover_ac_bonus(p.caster_id, p.target_id);
    let target_ac = target_ac + cover_bonus;

    // 5e Sanctuary: if the target is sanctified, the attacker first makes
    // a WIS save vs the warding caster's DC. On fail, the attack silently
    // misses (no AC roll, no rider consumption). Pass-through behavior
    // matches RAW's "the attacker must choose a new target or lose the
    // attack" clause — we choose the latter to avoid auto-retargeting.
    if encounter.sanctuary_save_blocks(p.caster_id, p.target_id) {
        return (Vec::new(), 0);
    }
    // Casting a harmful action revokes the holder's own Sanctuary buff.
    // We tag the clear after we know the attack is going through (the
    // sanctuary_save_blocks branch returns early on fail).
    encounter.break_sanctuary_on_hostile(p.caster_id);

    let mut mode = encounter.compute_attack_mode(p.caster_id, p.target_id, p.is_melee);
    // 5e long-range disadvantage: ranged weapon attacks beyond normal
    // range but within max range impose disadvantage. The `long_range`
    // threshold (in tiles) is set by the weapon definition — melee
    // weapons and spells leave it `None`.
    if let Some(nr) = p.long_range
        && !p.is_melee
        && let Some(dist) = encounter.footprint_distance(p.caster_id, p.target_id)
        && dist > nr
    {
        mode = mode.combine(crate::engine::dice::RollMode::Disadvantage);
    }
    // Caster-side flat bonuses. `attack_bonus_buff` is the install-side
    // ledger (Bless's AdjustAttackBuff(+2), etc.). `condition_attack_bonus`
    // is the read-side flag table — Sacred Weapon's +CHA modifier and
    // Bardic Inspiration's +3 ride here. Keeping the two lanes separate
    // makes Bless's "install once, drop on concentration" pattern reuse
    // cleanly with the read-only condition lane. Shared with spell
    // attacks via `EncounterInstance::caster_attack_buffs`.
    //
    // Read these BEFORE clearing the one-shot riders so the Inspired
    // condition (and any other condition that contributes to
    // `condition_attack_bonus`) is still active when we sum the bonus.
    // The rider clear below removes Inspired alongside Helped/Hidden,
    // so swapping the order would zero out the +3.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(p.caster_id);
    // Bless/Bane: roll an actual 1d4 once per attack and add (Bless) or
    // subtract (Bane) from the total. Both: they cancel and no die is
    // rolled. We log the d4 separately so the player can see why the
    // d20 alone doesn't account for the swing's hit.
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(p.caster_id);
    // Burn through the one-shot rider stack (Helped, Hidden,
    // per-target help grant, Invisibility concentration, Inspired)
    // before the d20 lands so a second swing this turn doesn't double-
    // dip. The advantage / disadvantage flags were already picked up
    // into `mode` by `compute_attack_mode` above; the flat bonuses
    // were just read into `cond_attack_bonus` immediately above. Both
    // are safe to clear here without losing this swing's modifiers.
    encounter.clear_attack_advantage_riders(p.caster_id, p.target_id);
    // 5e Lucky: if the holder rolls a nat-1, they may re-roll once. The
    // helper folds the reroll into the same seedable RNG so determinism
    // by seed holds — and falls back to the raw roll for actors without
    // the trait.
    let raw_attack = encounter.roll_d20_lucky(p.caster_id, mode) as i32;
    // 5e Improved Critical: the d20 face that promotes to a crit is
    // template-driven (Champion fighter: 19+; Superior Critical: 18+).
    // The engine-level `crit_threshold` accessor folds in the default of
    // 20 for missing actors / non-Champion builds.
    let nat_crit = raw_attack >= encounter.crit_threshold(p.caster_id);
    let attack_total = raw_attack + p.attack_bonus + buff + cond_attack_bonus + bless_die;
    let is_nat_one = raw_attack == 1;
    let hit = !is_nat_one && (nat_crit || attack_total >= target_ac);
    // 5e Paralyzed / Unconscious clause: any hit from within 5ft is a
    // crit. The promotion happens after we've decided the swing connected
    // so a flat miss still misses — the rider only upgrades a regular
    // hit to a crit (mirrors the RAW "any attack that hits the creature
    // is a critical hit" wording).
    let is_crit = nat_crit
        || (hit
            && encounter.target_grants_melee_auto_crit(p.caster_id, p.target_id, p.is_melee));
    let outcome = if is_nat_one {
        "miss (nat 1)"
    } else if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let cover_note = EncounterInstance::cover_log_suffix(cover_bonus);
    encounter.log(format!(
        "  {}: 1d20({}){:+}{} = {} vs AC {}{}{} \u{2014} {}",
        p.action_name,
        raw_attack,
        p.attack_bonus + buff + cond_attack_bonus,
        bless_note,
        attack_total,
        target_ac,
        cover_note,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return (Vec::new(), 0);
    }
    // 5e Mirror Image: a hit may instead strike a decoy. Shared with
    // spell attacks via `EncounterInstance::mirror_image_deflect` so
    // the deflection rule applies to any attack roll, not just weapon
    // swings (RAW: "any attack roll against you").
    if encounter.mirror_image_deflect(p.target_id, is_crit) {
        return (Vec::new(), 0);
    }
    let raw_damage = encounter.roll(&p.damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&p.damage_dice) as i32
    } else {
        0
    };
    // 5e Brutal Critical (Barbarian level 9 / 13 / 17) + Half-Orc Savage
    // Attacks: both add extra weapon damage dice on a critical melee hit.
    // Spell attacks don't qualify — gated on `is_melee`. The dice counts
    // are template-driven so a level-17 half-orc barbarian rolls
    // 3 (Brutal Critical) + 1 (Savage Attacks) = 4 extra dice without
    // touching this site. Each rider logs separately so the source of
    // the extra dice is legible in the combat log.
    let brutal_extra = if is_crit && p.is_melee {
        let mut total = 0;
        let (brutal_dice_count, savage) = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| (a.brutal_critical_dice(), a.has_savage_attacks()))
            .unwrap_or((0, false));
        if brutal_dice_count > 0 {
            let brutal_dice = Dice::new(brutal_dice_count, p.damage_dice.faces);
            let rolled = encounter.roll(&brutal_dice) as i32;
            encounter.log(format!(
                "  brutal critical: +{}({}) = +{} {:?}",
                brutal_dice, rolled, rolled, p.damage_type
            ));
            total += rolled;
        }
        if savage {
            let savage_dice = Dice::new(1, p.damage_dice.faces);
            let rolled = encounter.roll(&savage_dice) as i32;
            encounter.log(format!(
                "  savage attacks: +{}({}) = +{} {:?}",
                savage_dice, rolled, rolled, p.damage_type
            ));
            total += rolled;
        }
        total
    } else {
        0
    };
    let mut damage = (raw_damage + crit_extra + brutal_extra + p.damage_bonus).max(0) as u32;
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
    // 5e Barbarian Rage: +2 melee weapon damage (scales to +3/+4 at
    // higher levels in RAW, but we use +2 for the base tier). Only
    // applies to STR-based melee attacks.
    if p.is_melee
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_condition(Condition::Raging))
    {
        damage = damage.saturating_add(2);
        encounter.log("  rage: +2 melee damage");
    }
    // Hunter's Mark rider: attacker concentrating on Hunter's Mark with
    // this target marked deals +1d6 (weapon-typed). Crits double the
    // mark die per RAW — the rider folds into the weapon's damage type.
    if encounter.is_hunters_mark_target(p.caster_id, p.target_id) {
        let hm_total = roll_rider(encounter, Dice::new(1, 6), is_crit);
        damage = damage.saturating_add(hm_total);
        encounter.log(format!(
            "  hunter's mark: +{} extra {:?}",
            hm_total, p.damage_type
        ));
    }
    // 5e Uncanny Dodge (Rogue 5): when hit by an attack, spend reaction
    // to halve the damage. Only fires if the target has the feature, a
    // reaction available, and can see the attacker (we approximate sight
    // as "not Blinded").
    if let Some(target) = encounter.actors.get(&p.target_id)
        && target.has_uncanny_dodge()
        && target.has_reaction()
        && !target.has_condition(Condition::Blinded)
        && !target.has_condition(Condition::Unconscious)
    {
        damage /= 2;
        encounter.log(format!("  uncanny dodge: damage halved to {}", damage));
        if let Some(t) = encounter.actors.get_mut(&p.target_id) {
            t.consume_resource(crate::engine::side_effects::Resource::Reaction);
        }
    }
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
        actor_id: p.target_id,
        amount: damage,
        damage_type: p.damage_type,
    })];
    if encounter.is_hex_target(p.caster_id, p.target_id) {
        let hex_total = roll_rider(encounter, Dice::new(1, 6), is_crit);
        encounter.log(format!("  hex: +{} extra Necrotic", hex_total));
        effects.push(Box::new(DealDamage {
            actor_id: p.target_id,
            amount: hex_total,
            damage_type: DamageType::Necrotic,
        }));
    }
    // Caster-side per-hit damage riders. Generalized so any condition
    // that grants "+Xdy damage of type T on hit" plugs in here without
    // re-implementing the attack-roll-to-damage glue. Gates:
    //   - `melee_only`: skip on ranged attacks (5e Smite spells / Divine
    //     Smite RAW: melee weapon only).
    //   - `consume_on_trigger`: strip the condition after the rider
    //     lands (one-shot primes like Divine Smite / the four Smite
    //     spells; persistent aura-style riders like Crusader's Mantle
    //     and Crown of Stars leave their condition in place for the
    //     full spell duration).
    //   - `follow_up`: optional secondary clause that fires only on the
    //     swing that *consumed* the rider — used by Blinding Smite (CON
    //     save or Blinded) and Wrathful Smite (WIS save or Frightened).
    for rider in on_hit_riders() {
        if rider.melee_only && !p.is_melee {
            continue;
        }
        if rider.ranged_only && p.is_melee {
            continue;
        }
        if !encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_condition(rider.condition))
        {
            continue;
        }
        // Roll the rider damage only if the rider actually has dice —
        // primes whose entire effect is the follow-up (Stunning Strike:
        // no damage, just a stun save) declare 0 dice so the damage
        // line and the DealDamage push are skipped. Capture the rolled
        // total so the follow-up's optional `hp_threshold` gate can
        // predict the target's post-damage HP without re-rolling.
        let rider_total = if rider.dice.count > 0 {
            let total = roll_rider(encounter, rider.dice, is_crit);
            encounter.log(format!(
                "  {}: +{} {:?}",
                rider.label, total, rider.damage_type
            ));
            effects.push(Box::new(DealDamage {
                actor_id: p.target_id,
                amount: total,
                damage_type: rider.damage_type,
            }));
            total
        } else {
            0
        };
        if rider.consume_on_trigger
            && let Some(caster) = encounter.actors.get_mut(&p.caster_id)
        {
            caster.remove_condition(rider.condition);
        }
        if let Some(follow) = rider.follow_up {
            apply_smite_follow_up(
                encounter,
                &mut effects,
                p.caster_id,
                p.target_id,
                follow,
                rider_total + damage,
            );
        }
    }
    // Melee-only retaliation table: any condition the *target* holds that
    // bounces damage back at a melee attacker (Fire Shield 2d8 fire,
    // Armor of Agathys 5 cold, Investiture of Flame 1d10 fire). Each
    // entry plugs in here without re-implementing the "target has cond?
    // → roll → log → push DealDamage(attacker)" dance. The reflected
    // damage resolves through the standard damage pipeline so the
    // attacker's typed immunity / resistance / vulnerability is honored.
    if p.is_melee {
        for rider in melee_reflect_riders() {
            if !encounter
                .actors
                .get(&p.target_id)
                .is_some_and(|a| a.has_condition(rider.condition))
            {
                continue;
            }
            let amount = match rider.damage {
                ReflectDamage::Flat(n) => {
                    encounter.log(format!(
                        "  {}: {} {:?} reflected",
                        rider.label, n, rider.damage_type
                    ));
                    n
                }
                ReflectDamage::Dice(dice) => {
                    let rolled = encounter.roll(&dice);
                    encounter.log(format!(
                        "  {}: {}({}) {:?} reflected",
                        rider.label, dice, rolled, rider.damage_type
                    ));
                    rolled
                }
            };
            effects.push(Box::new(DealDamage {
                actor_id: p.caster_id,
                amount,
                damage_type: rider.damage_type,
            }));
        }
    }
    (effects, damage)
}

/// Damage payload for a melee retaliation rider. Some shields roll dice
/// (Fire Shield 2d8, Investiture of Flame 1d10); others deal flat damage
/// (Armor of Agathys 5). Captured as an enum so the rider table stays a
/// flat array of plain-data entries.
#[derive(Clone, Copy)]
pub enum ReflectDamage {
    Dice(Dice),
    Flat(u32),
}

/// A target-side "creature hit me in melee, take this damage back" rider.
/// Mirrors `OnHitRider` in shape but lives on the *target* of the swing
/// rather than the caster: any actor who holds `condition` reflects
/// `damage` of `damage_type` onto every melee attacker that connects.
#[derive(Clone, Copy)]
pub struct MeleeReflectRider {
    pub condition: Condition,
    pub damage: ReflectDamage,
    pub damage_type: DamageType,
    /// Log-friendly tag ("fire shield", "armor of agathys", ...).
    pub label: &'static str,
}

/// Build the melee retaliation rider table. Symmetric with `on_hit_riders`
/// but consumed at the target side of `resolve_attack_outcome`. Returned
/// by value rather than declared `const` because `Dice::new` isn't a
/// const fn — the runtime cost is one stack-allocated array.
fn melee_reflect_riders() -> [MeleeReflectRider; 3] {
    [
        // 5e Fire Shield — 2d8 fire on every melee contact. Concentration-
        // free, self-only; the warm / cool variant only matters for the
        // resistance lane (we collapse to a single Fire Shield condition).
        MeleeReflectRider {
            condition: Condition::FireShielded,
            damage: ReflectDamage::Dice(Dice::new(2, 8)),
            damage_type: DamageType::Fire,
            label: "fire shield",
        },
        // 5e Armor of Agathys — flat 5 cold on melee contact. We don't
        // scale with slot level (the spell's install site sets the temp
        // HP buffer instead). Mirrors Fire Shield's shape but cheaper.
        MeleeReflectRider {
            condition: Condition::AgathysShielded,
            damage: ReflectDamage::Flat(5),
            damage_type: DamageType::Cold,
            label: "armor of agathys",
        },
        // 5e Investiture of Flame — 1d10 fire on melee contact. Smaller
        // die than Fire Shield (concentration-bound on the caster, paired
        // with the broader fire resistance baked into the install).
        MeleeReflectRider {
            condition: Condition::InvestedInFlame,
            damage: ReflectDamage::Dice(Dice::new(1, 10)),
            damage_type: DamageType::Fire,
            label: "investiture of flame",
        },
    ]
}

/// Roll a single rider die for an on-hit bonus, doubling on crit per
/// 5e RAW. Used by every "per-hit weapon-bonus damage" effect — Hex /
/// Hunter's Mark (1d6), Crusader's Mantle (1d4), Crown of Stars (1d8).
/// Keeps the crit-doubling rule in one place. Public so the spell-attack
/// resolver can share the same crit-doubling rule for Hex.
pub fn roll_rider(encounter: &mut EncounterInstance, dice: Dice, is_crit: bool) -> u32 {
    let base = encounter.roll(&dice);
    let crit_extra = if is_crit { encounter.roll(&dice) } else { 0 };
    base + crit_extra
}

/// A "+Xdy damage on hit" rider sourced from one of the caster's active
/// conditions. The rider table is consumed once per weapon hit by
/// `resolve_attack_outcome` — any caster who holds `condition` adds the
/// rolled `dice` of `damage_type` to that swing.
#[derive(Clone, Copy)]
pub struct OnHitRider {
    /// Caster-side flag the rider keys off (Smiting / Crusader's
    /// Mantled / Crown of Stars / one of the four Smite-spell primes).
    pub condition: Condition,
    pub dice: Dice,
    /// Log-friendly name ("divine smite", "searing smite", ...).
    pub label: &'static str,
    pub damage_type: DamageType,
    /// True iff the rider only fires on melee swings (every Paladin
    /// Smite, Divine Smite). Ranged carriers like Crown of Stars or
    /// Crusader's Mantle leave this false so they tag arrow hits too.
    pub melee_only: bool,
    /// True iff the rider only fires on ranged swings (5e Lightning
    /// Arrow RAW: "the next attack you make with a ranged weapon").
    /// Symmetric to `melee_only` — both default to false so the rider
    /// fires on either lane. Defaulting to false keeps every existing
    /// rider entry unchanged.
    pub ranged_only: bool,
    /// True iff the condition is stripped from the caster the moment
    /// the rider lands (one-shot primes). Persistent buffs leave this
    /// false so they stay up until the spell ends.
    pub consume_on_trigger: bool,
    /// Optional save-then-condition follow-up that fires only on the
    /// swing that consumed the rider. Powers Blinding Smite (CON save
    /// or Blinded) and Wrathful Smite (WIS save or Frightened). `None`
    /// for damage-only riders.
    pub follow_up: Option<SmiteFollowUp>,
}

/// Secondary save + effect rider tagged onto a Smite-spell hit. When
/// `save_ability` is `Some(ability)`, the target rolls that save against
/// the caster's spell DC (driven by `dc_ability`); on fail, the rider's
/// `effect` lands. When `save_ability` is `None`, the rider auto-applies
/// on hit with no save (Branding Smite's "brand", Searing Smite's ignite).
/// Stored as a value so the on-hit rider table stays a flat array of
/// plain-data entries.
#[derive(Clone, Copy)]
pub struct SmiteFollowUp {
    /// Save the target rolls (CON for Blinding Smite, WIS for Wrathful
    /// Smite). `None` skips the save entirely — the effect lands
    /// unconditionally on the consuming hit (subject to `hp_threshold`).
    pub save_ability: Option<AbilityScoreType>,
    /// Ability whose mod feeds the caster's spell save DC. Ignored when
    /// `save_ability` is `None` (no save means no DC).
    pub dc_ability: AbilityScoreType,
    /// What lands on a failed save: a condition apply, a push, or both.
    pub effect: FollowUpEffect,
    /// Log-friendly tag ("blinding smite blind", "wrathful smite fear",
    /// "pushing attack push").
    pub label: &'static str,
    /// Optional post-damage HP gate. Banishing Smite RAW: the rider lands
    /// only "if this damage reduces the target to 50 hp or fewer." We
    /// fold this into the smite follow-up site by predicting post-damage
    /// HP as `current_hp - rider_damage` and gating the apply on that.
    /// `None` (the default) skips the gate.
    pub hp_threshold: Option<u32>,
}

/// What a smite follow-up does on a failed save (or auto-trigger).
/// Most riders apply a single condition (`Condition`); the Battle Master
/// Pushing Attack maneuver shoves the target backward instead, and the
/// Sweeping Attack maneuver splashes damage onto an adjacent enemy.
#[derive(Clone, Copy)]
pub enum FollowUpEffect {
    /// Apply a single condition with the given timer. Used by every
    /// Smite-spell rider that produces a debuff (Blinded, Frightened,
    /// Prone, Stunned, Outlined, Burning, ...).
    Condition {
        condition: Condition,
        timer: ConditionTimer,
    },
    /// Push the target `tiles` away from the attacker. Used by the
    /// Battle Master Pushing Attack maneuver. No condition apply —
    /// the displacement IS the effect.
    Push { tiles: u32 },
    /// Splash damage onto one adjacent enemy of the original target.
    /// Used by the Battle Master Sweeping Attack maneuver: the swing's
    /// momentum carries through to a nearby foe for an extra die of
    /// damage. We pick the closest hostile (to the caster) that's
    /// footprint-adjacent to the target and apply the rolled `dice` as
    /// `damage_type`. No save — RAW: the original attack roll is
    /// re-used. If no adjacent enemy exists the effect is a no-op.
    Splash {
        dice: Dice,
        damage_type: DamageType,
    },
}

/// Build the caster-side on-hit rider table. Returned by value rather
/// than declared `const` because `Dice::new` isn't a const fn — but the
/// runtime cost is one stack-allocated array of plain data, so the
/// indirection is free.
fn on_hit_riders() -> [OnHitRider; 25] {
    [
        OnHitRider {
            condition: Condition::CrusadersMantled,
            dice: Dice::new(1, 4),
            label: "crusader's mantle",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::CrownOfStars,
            dice: Dice::new(1, 8),
            label: "crown of stars",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Bigby's Hand (level-5 concentration). Persistent +1d10 force
        // rider on every attack the caster lands — not melee-only since
        // the spectral hand can punch at range. Slots cleanly between
        // Crown of Stars (1d8 radiant) and the Smiting one-shot primes.
        OnHitRider {
            condition: Condition::BigbysHanded,
            dice: Dice::new(1, 10),
            label: "bigby's hand",
            damage_type: DamageType::Force,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Spirit Shroud (level-3 concentration). Persistent +1d8 cold
        // rider on every melee swing the holder lands. Mirrors Crown of
        // Stars but cold-typed and melee-only.
        OnHitRider {
            condition: Condition::SpiritShrouded,
            dice: Dice::new(1, 8),
            label: "spirit shroud",
            damage_type: DamageType::Cold,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::Smiting,
            dice: Dice::new(2, 8),
            label: "divine smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Searing Smite — 1st-level paladin evocation, bonus action.
        // +1d6 fire on the primed hit, and the target catches fire
        // (Burning) for 3 rounds. We bake the Burning rider in as a
        // SmiteFollowUp with a permissive save (no save in RAW — the
        // target makes ongoing WIS saves to extinguish; we approximate
        // with a flat 3-round Burning).
        OnHitRider {
            condition: Condition::SearingSmiting,
            dice: Dice::new(1, 6),
            label: "searing smite",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // Auto-apply — RAW Searing Smite ignites the target on
                // hit with no save. `save_ability: None` skips the save
                // roll in `apply_smite_follow_up`.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Burning,
                    timer: ConditionTimer::Rounds(3),
                },
                label: "searing smite ignite",
                hp_threshold: None,
            }),
        },
        // 5e Wrathful Smite — 1st-level. +1d6 psychic on the primed hit;
        // target makes WIS save or is Frightened of the paladin for
        // up to 10 rounds (RAW: 1 minute).
        OnHitRider {
            condition: Condition::WrathfulSmiting,
            dice: Dice::new(1, 6),
            label: "wrathful smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "wrathful smite fear",
                hp_threshold: None,
            }),
        },
        // 5e Branding Smite — 2nd-level. +2d6 radiant; target glows
        // (Outlined for 10 rounds), giving advantage to attackers and
        // ending Invisibility / Hidden status.
        OnHitRider {
            condition: Condition::BrandingSmiting,
            dice: Dice::new(2, 6),
            label: "branding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // RAW Branding Smite is auto-apply on hit (no save).
                // `save_ability: None` skips the save roll.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Outlined,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "branding smite brand",
                hp_threshold: None,
            }),
        },
        // 5e Blinding Smite — 3rd-level. +3d8 radiant; target makes CON
        // save or is Blinded for 10 rounds.
        OnHitRider {
            condition: Condition::BlindingSmiting,
            dice: Dice::new(3, 8),
            label: "blinding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "blinding smite blind",
                hp_threshold: None,
            }),
        },
        // 5e Monk Stunning Strike — bonus action prime; on the next
        // melee hit, the target makes a CON save vs the monk's
        // WIS-based DC or is Stunned for 1 round. Zero rider dice (the
        // stun *is* the effect); the rider loop's `count > 0` guard
        // skips the damage line.
        OnHitRider {
            condition: Condition::StunningStrike,
            dice: Dice::new(0, 1),
            label: "stunning strike",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Wisdom,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "stunning strike stun",
                hp_threshold: None,
            }),
        },
        // 5e Cleric Divine Strike (level 8 class feature, here exposed as
        // a Channel-Divinity-flavored bonus-action prime). +1d8 radiant
        // on the next melee weapon hit; no save. Mirrors the Smiting
        // shape but auto-applies no follow-up condition (the radiant
        // rider IS the effect). Consumed the moment a melee swing lands.
        OnHitRider {
            condition: Condition::DivineStriking,
            dice: Dice::new(1, 8),
            label: "divine strike",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Trip Attack maneuver. No bonus damage in our
        // model (RAW: +superiority die damage; we skip the die since the
        // existing dice infra doesn't carry a per-class scaling pool);
        // the prone-on-fail-STR-save IS the effect. Mirrors Stunning
        // Strike's "zero-damage rider with a save follow-up" shape.
        OnHitRider {
            condition: Condition::TripAttacking,
            dice: Dice::new(0, 1),
            label: "trip attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "trip attack prone",
                hp_threshold: None,
            }),
        },
        // 5e Staggering Smite — 4th-level paladin enchantment, bonus
        // action prime. +4d6 psychic on the primed hit; target makes a
        // WIS save vs the caster's CHA-based DC or is Stunned until the
        // end of the paladin's next turn (we model as 1 round). One-shot.
        OnHitRider {
            condition: Condition::StaggeringSmiting,
            dice: Dice::new(4, 6),
            label: "staggering smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "staggering smite stun",
                hp_threshold: None,
            }),
        },
        // 5e Banishing Smite — 5th-level paladin abjuration, bonus
        // action prime. +5d10 force on the primed hit; if the target
        // ends the swing at 50 HP or fewer they are banished. We model
        // the banishment via the existing `Mazed` envelope (zero
        // movement + blocked action economy + blocked reactions) for
        // 10 rounds — distinct log line, identical end-state. The HP
        // threshold gate is evaluated at the smite-follow-up site (see
        // `apply_smite_follow_up`'s threshold extension below).
        OnHitRider {
            condition: Condition::BanishingSmiting,
            dice: Dice::new(5, 10),
            label: "banishing smite",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // No save — RAW: the banish is auto-apply if HP ≤ 50.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Mazed,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "banishing smite banish",
                // RAW: only banished "if this damage reduces the target
                // to 50 hp or fewer." We honor the gate by predicting
                // post-damage HP at the smite follow-up site.
                hp_threshold: Some(50),
            }),
        },
        // 5e Shillelagh — druid cantrip prime. The caster's club /
        // staff is wreathed in sylvan magic: the next melee weapon hit
        // deals an extra 1d8 force damage. RAW also lets the swing use
        // WIS instead of STR for the to-hit / damage roll; we skip the
        // stat-swap (the +1d8 rider IS the load-bearing buff). One-shot
        // — the rider table strips this flag the moment a melee swing
        // lands. Concentration-free per RAW (1-minute duration).
        OnHitRider {
            condition: Condition::Shillelaghed,
            dice: Dice::new(1, 8),
            label: "shillelagh",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Thunderous Smite — 1st-level paladin evocation, bonus
        // action prime. +2d6 thunder on the primed hit; target makes a
        // STR save vs the caster's CHA-based DC or is knocked Prone.
        // RAW also pushes 10 ft on the failed save; we collapse the
        // push since the smite follow-up table only carries one
        // condition apply — the prone tag is the load-bearing
        // crowd-control bit and the push helper is reserved for spells
        // whose entire effect is repositioning (Thunderwave).
        OnHitRider {
            condition: Condition::ThunderousSmiting,
            dice: Dice::new(2, 6),
            label: "thunderous smite",
            damage_type: DamageType::Thunder,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "thunderous smite prone",
                hp_threshold: None,
            }),
        },
        // 5e Enlarge / Reduce (Enlarge half) — 2nd-level transmutation,
        // concentration. The holder rolls +1d4 bonus damage on every
        // weapon attack hit (RAW: melee or ranged). Persistent — the
        // rider doesn't consume_on_trigger; it stays up until the
        // caster drops concentration. Damage type matches the weapon
        // (no fixed magical type) — we model with Bludgeoning as a
        // baseline since the engine doesn't carry per-attack weapon
        // typing into the rider table. Slots between Crusader's Mantle
        // (1d4 radiant) and Spirit Shroud (1d8 cold) in the rider table.
        OnHitRider {
            condition: Condition::Enlarged,
            dice: Dice::new(1, 4),
            label: "enlarge",
            damage_type: DamageType::Bludgeoning,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Lightning Arrow — 3rd-level ranger evocation, bonus action,
        // concentration. Primes the ranger's next ranged weapon attack
        // with +4d8 lightning. The 10-ft splash (2d8 lightning on
        // adjacent creatures, DEX save for half) is layered on by the
        // spell's `side_effects` at cast time via a follow-up burst — the
        // rider table just carries the per-hit prime damage, mirroring
        // every other Smite-style one-shot. melee_only=false so a
        // longbow swing tags it; consume_on_trigger=true so the first
        // hit consumes the prime.
        OnHitRider {
            condition: Condition::LightningArrowPrimed,
            dice: Dice::new(4, 8),
            label: "lightning arrow",
            damage_type: DamageType::Lightning,
            melee_only: false,
            ranged_only: true,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Holy Weapon — 5th-level paladin evocation, concentration.
        // The caster's weapon glows with radiant light: every weapon
        // attack hit deals an extra 2d8 radiant damage. Persistent (not
        // consumed on trigger) and lane-agnostic — melee or ranged hits
        // both fire the rider. Mirrors the Crusader's Mantle / Spirit
        // Shroud persistent rider shape but tier-5 dice and radiant.
        OnHitRider {
            condition: Condition::HolyWeaponed,
            dice: Dice::new(2, 8),
            label: "holy weapon",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Absorb Elements — 1st-level abjuration, reaction. The caster
        // stores captured elemental energy and releases it on their next
        // melee attack: +1d6 of the absorbed element's damage type. We
        // model the rider as generic Force since the per-element type
        // isn't tracked. One-shot — consumed the moment a melee swing
        // lands. The resistance half is handled by the spell's
        // DamageResistant condition install.
        OnHitRider {
            condition: Condition::AbsorbedElements,
            dice: Dice::new(1, 6),
            label: "absorb elements",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Menacing Attack maneuver. Zero rider damage
        // (RAW: +1 superiority die; we collapse the die since the engine
        // has no per-class scaling pool). On the consuming melee hit the
        // target makes a WIS save vs the fighter's STR-based maneuver DC
        // (8 + prof + STR); on fail, they're Frightened until the end of
        // the fighter's next turn (we model as 1 round). Mirrors Stunning
        // Strike's "no rider damage, save-or-condition" shape.
        OnHitRider {
            condition: Condition::MenacingAttacking,
            dice: Dice::new(0, 1),
            label: "menacing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "menacing attack fear",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Disarming Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Trip). On the consuming melee hit the
        // target makes a STR save vs the fighter's STR-based maneuver DC;
        // on fail, they're Disarmed — attack rolls have disadvantage
        // until the start of their next turn (`UntilStartOfNextTurn`
        // mirrors how Mocked / Dodging clear). One-shot.
        OnHitRider {
            condition: Condition::DisarmingAttacking,
            dice: Dice::new(0, 1),
            label: "disarming attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Disarmed,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "disarming attack disarm",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Pushing Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming). On the consuming melee
        // hit the target makes a STR save vs the fighter's STR-based
        // maneuver DC; on fail, they're shoved 15 ft (4 tiles in our
        // 2.5ft grid — round 15ft/2.5 = 6, but we cap at the engine's
        // PushActor maximum behavior of 4 tiles, matching Thunderwave's
        // 10ft push). The first follow-up to use the `Push` variant of
        // `FollowUpEffect` — no condition apply, just forced movement.
        OnHitRider {
            condition: Condition::PushingAttacking,
            dice: Dice::new(0, 1),
            label: "pushing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Push { tiles: 4 },
                label: "pushing attack shove",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Goading Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming / Pushing). On the
        // consuming melee hit the target makes a WIS save vs the
        // fighter's STR-based maneuver DC; on fail, they're Goaded —
        // attack rolls against anyone *other* than the goading fighter
        // are at disadvantage. The follow-up handler also wires the
        // `goaded_by` link via the auto-chained SetGoadedBy emission in
        // `push_follow_up_effect`. Mirrors Compelled Duel's mechanical
        // envelope, but is per-rest rather than concentration-bound.
        OnHitRider {
            condition: Condition::GoadingAttacking,
            dice: Dice::new(0, 1),
            label: "goading attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Goaded,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "goading attack goad",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Sweeping Attack maneuver. Zero rider damage on
        // the primary target (RAW: damage goes to the secondary creature,
        // not the original); the follow-up's `Splash` variant deals 1d8
        // slashing to one footprint-adjacent enemy of the primary target.
        // `save_ability: None` makes the follow-up auto-apply on hit —
        // RAW: the splash uses the original attack roll, which already
        // hit. If no adjacent enemy exists, the splash is a no-op
        // (logged inside `push_follow_up_effect`).
        OnHitRider {
            condition: Condition::SweepingAttacking,
            dice: Dice::new(0, 1),
            label: "sweeping attack",
            damage_type: DamageType::Slashing,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: None,
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Splash {
                    dice: Dice::new(1, 8),
                    damage_type: DamageType::Slashing,
                },
                label: "sweeping attack splash",
                hp_threshold: None,
            }),
        },
    ]
}

/// Process the optional secondary save-and-apply step that some Smite
/// spells stack on top of their bonus damage. `save_ability: None`
/// auto-applies the condition on hit (Branding Smite, Searing Smite's
/// ignite); `Some(ability)` rolls that save against the caster's DC.
/// `total_damage` is the swing's combined weapon + rider damage value —
/// used to predict the target's post-damage HP for `hp_threshold` gates
/// (Banishing Smite RAW: banishes "if this damage reduces the target to
/// 50 hp or fewer"). `0` is fine for follow-ups with no threshold set.
fn apply_smite_follow_up(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    follow: SmiteFollowUp,
    total_damage: u32,
) {
    // HP-threshold gate (Banishing Smite). Predict post-damage HP as
    // `current_hp - total_damage` and bail if the target would still be
    // above the threshold. Saturating-sub keeps the math clean when the
    // hit would drop them past zero (the threshold still triggers).
    if let Some(threshold) = follow.hp_threshold {
        let Some(target) = encounter.actors.get(&target_id) else {
            return;
        };
        let predicted = target.hitpoints().saturating_sub(total_damage);
        if predicted > threshold {
            encounter.log(format!(
                "  {}: target stays above {} HP threshold (predicted {} HP)",
                follow.label, threshold, predicted
            ));
            return;
        }
    }
    let Some(save_ability) = follow.save_ability else {
        encounter.log(format!("  {}: auto-apply on hit", follow.label));
        push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
        return;
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return;
    };
    let dc = caster.spell_save_dc(follow.dc_ability);
    let save = encounter.roll_save(target_id, save_ability, dc);
    if save.passed() {
        encounter.log(format!("  {}: target saves", follow.label));
        return;
    }
    encounter.log(format!("  {}: target fails save", follow.label));
    push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
}

/// Materialize a `FollowUpEffect` into the side-effect queue. Splits the
/// "what does the rider actually do?" decision from the save / threshold
/// gates above so a future variant (e.g. forced grapple, dispel) plugs
/// in here without re-walking the gates. `caster_id` is the attacker —
/// used by `Push` to anchor the shove on the attacker's tile and by
/// `Splash` to scope the secondary-target search to enemies of the
/// caster.
fn push_follow_up_effect(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    effect: FollowUpEffect,
) {
    match effect {
        FollowUpEffect::Condition { condition, timer } => {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition,
                timer,
            }));
            // Conditions that carry a back-reference to the attacker:
            // chain the link-update side-effect alongside the apply so a
            // re-cast / re-trigger never leaves a stale link in place.
            // Mirrors how Compelled Duel's action pairs ApplyCondition
            // (Dueled) with SetDueledBy at the spell site; here we hide
            // the pairing inside the rider's follow-up so every "goading
            // attack hit" path lands it without re-stating the chain.
            if condition == Condition::Goaded {
                effects.push(Box::new(SetGoadedBy {
                    target_id,
                    goader: Some(caster_id),
                }));
            }
        }
        FollowUpEffect::Push { tiles } => {
            let Some(caster) = encounter.actors.get(&caster_id) else {
                return;
            };
            effects.push(Box::new(PushActor {
                actor_id: target_id,
                from: caster.location(),
                max_tiles: tiles,
            }));
        }
        FollowUpEffect::Splash { dice, damage_type } => {
            // Pick the closest hostile (to the caster) that's
            // footprint-adjacent to the primary target. Closest = lowest
            // (footprint-distance, id) tuple so the choice is
            // deterministic across runs with the same RNG seed.
            let caster_team = match encounter.actors.get(&caster_id) {
                Some(c) => c.team(),
                None => return,
            };
            let mut best: Option<(isize, usize)> = None;
            for (id, other) in encounter.actors.iter() {
                if *id == caster_id || *id == target_id {
                    continue;
                }
                if other.team() == caster_team || !other.is_combat_active() {
                    continue;
                }
                let Some(dist) = encounter.footprint_distance(*id, target_id) else {
                    continue;
                };
                if dist > 0 {
                    continue;
                }
                let dist_from_caster = encounter
                    .footprint_distance(caster_id, *id)
                    .unwrap_or(isize::MAX);
                let key = (dist_from_caster, *id);
                if best.map(|b| key < b).unwrap_or(true) {
                    best = Some(key);
                }
            }
            let Some((_, splash_id)) = best else {
                encounter.log("  sweeping attack: no adjacent enemy to splash".to_string());
                return;
            };
            let rolled = encounter.roll(&dice);
            encounter.log(format!(
                "  sweeping attack: +{} {:?} splashes to {}",
                rolled,
                damage_type,
                encounter
                    .actors
                    .get(&splash_id)
                    .map(|a| a.name().to_string())
                    .unwrap_or_default()
            ));
            effects.push(Box::new(DealDamage {
                actor_id: splash_id,
                amount: rolled,
                damage_type,
            }));
        }
    }
}
