use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, ApplyCondition, DealDamage};
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

    let mode = encounter.compute_attack_mode(p.caster_id, p.target_id, p.is_melee);
    // Burn through the one-shot rider stack (Helped, Hidden,
    // per-target help grant, Invisibility concentration) before the
    // d20 lands so a second swing this turn doesn't double-dip the
    // advantage. We pick up the Hidden / Helped / Invisible flags in
    // `compute_attack_mode` above, then this hook clears them.
    encounter.clear_attack_advantage_riders(p.caster_id, p.target_id);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    // Caster-side flat bonuses. `attack_bonus_buff` is the install-side
    // ledger (Bless's AdjustAttackBuff(+2), etc.). `condition_attack_bonus`
    // is the read-side flag table — Sacred Weapon's +CHA modifier and
    // Bardic Inspiration's +3 ride here. Keeping the two lanes separate
    // makes Bless's "install once, drop on concentration" pattern reuse
    // cleanly with the read-only condition lane. Shared with spell
    // attacks via `EncounterInstance::caster_attack_buffs`.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(p.caster_id);
    // Bless/Bane: roll an actual 1d4 once per attack and add (Bless) or
    // subtract (Bane) from the total. Both: they cancel and no die is
    // rolled. We log the d4 separately so the player can see why the
    // d20 alone doesn't account for the swing's hit.
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(p.caster_id);
    let attack_total = raw_attack + p.attack_bonus + buff + cond_attack_bonus + bless_die;
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
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
    // mark die per RAW — the rider folds into the weapon's damage type.
    if encounter.is_hunters_mark_target(p.caster_id, p.target_id) {
        let hm_total = roll_rider(encounter, Dice::new(1, 6), is_crit);
        damage = damage.saturating_add(hm_total);
        encounter.log(format!(
            "  hunter's mark: +{} extra {:?}",
            hm_total, p.damage_type
        ));
    }
    // Hex rider: 1d6 necrotic on every hit against the Hexed target. Like
    // Hunter's Mark, crits double the rider die. The necrotic typing
    // matters more than HM's weapon-typed rider — it can chip past
    // physical resistance but bounces off necrotic-resistant undead. The
    // necrotic damage is a separate `DealDamage` so the target's typed
    // resistance / immunity / vulnerability applies to it independently.
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
        // line and the DealDamage push are skipped.
        if rider.dice.count > 0 {
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
        }
        if rider.consume_on_trigger
            && let Some(caster) = encounter.actors.get_mut(&p.caster_id)
        {
            caster.remove_condition(rider.condition);
        }
        if let Some(follow) = rider.follow_up {
            apply_smite_follow_up(encounter, &mut effects, p.caster_id, p.target_id, follow);
        }
    }
    // 5e Fire Shield: if the target is fire-shielded and this was a melee
    // attack, the attacker takes 2d8 fire damage in retaliation. We tag
    // the reflective DealDamage onto the attacker — resolves through the
    // standard damage pipeline (immunity / resistance respected).
    if p.is_melee
        && encounter
            .actors
            .get(&p.target_id)
            .is_some_and(|a| a.has_condition(crate::conditions::Condition::FireShielded))
    {
        let reflect = encounter.roll(&Dice::new(2, 8));
        encounter.log(format!("  fire shield: 2d8({}) fire reflected", reflect));
        effects.push(Box::new(DealDamage {
            actor_id: p.caster_id,
            amount: reflect,
            damage_type: DamageType::Fire,
        }));
    }
    // 5e Armor of Agathys: melee attackers eat flat cold damage in
    // retaliation. Static 5 damage per RAW (we don't scale by slot
    // level — the spell's install site sets the temp HP). Symmetric
    // shape with the Fire Shield reflect above. Skips on miss (no
    // attack roll → no melee contact).
    if p.is_melee
        && encounter
            .actors
            .get(&p.target_id)
            .is_some_and(|a| a.has_condition(crate::conditions::Condition::AgathysShielded))
    {
        encounter.log("  armor of agathys: 5 cold reflected".to_string());
        effects.push(Box::new(DealDamage {
            actor_id: p.caster_id,
            amount: 5,
            damage_type: DamageType::Cold,
        }));
    }
    (effects, damage)
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

/// Secondary save + condition rider tagged onto a Smite-spell hit. When
/// `save_ability` is `Some(ability)`, the target rolls that save against
/// the caster's spell DC (driven by `dc_ability`); on fail, `apply` lands
/// with `timer`. When `save_ability` is `None`, the rider auto-applies
/// on hit with no save (Branding Smite's "brand", Searing Smite's ignite).
/// Stored as a value so the on-hit rider table stays a flat array of
/// plain-data entries.
#[derive(Clone, Copy)]
pub struct SmiteFollowUp {
    /// Save the target rolls (CON for Blinding Smite, WIS for Wrathful
    /// Smite). `None` skips the save entirely — the condition lands
    /// unconditionally on the consuming hit.
    pub save_ability: Option<AbilityScoreType>,
    /// Ability whose mod feeds the caster's spell save DC. Ignored when
    /// `save_ability` is `None` (no save means no DC).
    pub dc_ability: AbilityScoreType,
    pub apply: Condition,
    pub timer: ConditionTimer,
    /// Log-friendly tag ("blinding smite blind", "wrathful smite fear").
    pub label: &'static str,
}

/// Build the caster-side on-hit rider table. Returned by value rather
/// than declared `const` because `Dice::new` isn't a const fn — but the
/// runtime cost is one stack-allocated array of plain data, so the
/// indirection is free.
fn on_hit_riders() -> [OnHitRider; 9] {
    [
        OnHitRider {
            condition: Condition::CrusadersMantled,
            dice: Dice::new(1, 4),
            label: "crusader's mantle",
            damage_type: DamageType::Radiant,
            melee_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::CrownOfStars,
            dice: Dice::new(1, 8),
            label: "crown of stars",
            damage_type: DamageType::Radiant,
            melee_only: false,
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
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::Smiting,
            dice: Dice::new(2, 8),
            label: "divine smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
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
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // Auto-apply — RAW Searing Smite ignites the target on
                // hit with no save. `save_ability: None` skips the save
                // roll in `apply_smite_follow_up`.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                apply: Condition::Burning,
                timer: ConditionTimer::Rounds(3),
                label: "searing smite ignite",
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
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                apply: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
                label: "wrathful smite fear",
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
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // RAW Branding Smite is auto-apply on hit (no save).
                // `save_ability: None` skips the save roll.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                apply: Condition::Outlined,
                timer: ConditionTimer::Rounds(10),
                label: "branding smite brand",
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
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Charisma,
                apply: Condition::Blinded,
                timer: ConditionTimer::Rounds(10),
                label: "blinding smite blind",
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
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Wisdom,
                apply: Condition::Stunned,
                timer: ConditionTimer::Rounds(1),
                label: "stunning strike stun",
            }),
        },
    ]
}

/// Process the optional secondary save-and-apply step that some Smite
/// spells stack on top of their bonus damage. `save_ability: None`
/// auto-applies the condition on hit (Branding Smite, Searing Smite's
/// ignite); `Some(ability)` rolls that save against the caster's DC.
fn apply_smite_follow_up(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    follow: SmiteFollowUp,
) {
    let Some(save_ability) = follow.save_ability else {
        encounter.log(format!("  {}: auto-apply on hit", follow.label));
        effects.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: follow.apply,
            timer: follow.timer,
        }));
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
    effects.push(Box::new(ApplyCondition {
        actor_id: target_id,
        condition: follow.apply,
        timer: follow.timer,
    }));
}
