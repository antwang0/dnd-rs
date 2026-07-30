//! Class-flavored attacks. Distinct file from `monster_attacks.rs` so the
//! "vanilla weapon swing" pattern stays in one place and PC class
//! mechanics (sneak attack, smite, etc.) live here.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema, first_target_id},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, DealDamage, GiveResource, Resource},
        types::{Coordinate, DamageType},
        util::{footprint_chebyshev, get_tiles_from_size},
    },
};

/// Rogue's Shortsword + Sneak Attack rider. Finesse weapon: uses DEX for
/// attack and damage. Damage 1d6 + DEX. On hit, if the rogue has
/// advantage on the attack roll OR an ally of the rogue is footprint-
/// adjacent to the target (5e: an enemy of the target within 5ft other
/// than the rogue), deals an extra 1d6 sneak-attack damage. Once per
/// turn (the rogue can take more attacks but only one gets the rider).
pub struct RogueShortsword {}

impl Action for RogueShortsword {
    fn name(&self) -> &str {
        "shortsword"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "stab"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = caster.ability_modifier(AbilityScoreType::Dexterity);
        let attack_bonus = dex_mod;
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        // Resolve the attack roll up-front so the sneak-attack rider
        // can branch on the same d20 result. weapon_attack would re-
        // roll inside; instead we use the engine's mode-with-riders
        // helper and roll inline so we keep the mode visible.
        let mode = encounter.attack_mode_with_riders(caster_id, target_id, true);
        let raw_attack = encounter.roll_d20_lucky(caster_id, mode) as i32;
        let nat_crit = raw_attack >= encounter.crit_threshold(caster_id);
        let total = raw_attack + attack_bonus;
        let hit = nat_crit || total >= target_ac;
        // 5e Paralyzed / Unconscious: any melee hit within 5ft is a crit.
        // Promote *after* deciding hit so a miss stays a miss.
        let is_crit =
            nat_crit || (hit && encounter.target_grants_melee_auto_crit(caster_id, target_id, true));
        let outcome = if is_crit {
            "CRIT!"
        } else if hit {
            "hit"
        } else {
            "miss"
        };
        encounter.log(format!(
            "  shortsword: 1d20({}){:+} = {} vs AC {}{} \u{2014} {}",
            raw_attack,
            attack_bonus,
            total,
            target_ac,
            mode.log_suffix(),
            outcome
        ));
        if !hit {
            return Vec::new();
        }

        // Damage: 1d6 + DEX, doubled on crit (dice only).
        let raw_dmg = encounter.roll(&Dice::new(1, 6)) as i32;
        let crit_extra = if is_crit {
            encounter.roll(&Dice::new(1, 6)) as i32
        } else {
            0
        };
        let mut damage = (raw_dmg + crit_extra + dex_mod).max(0) as u32;

        // Sneak attack rider.
        let sneak_eligible = sneak_attack_eligible(encounter, caster_id, target_id, mode);
        let mut side_effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if sneak_eligible {
            let level = encounter
                .actors
                .get(&caster_id)
                .map(|a| a.level())
                .unwrap_or(1);
            let total_sneak_dice = sneak_attack_dice_for_level(level);
            // 5e 2024 Cunning Strike: deduct dice from the sneak pool for
            // a tactical effect. Walks the active prime table, picks the
            // first match, returns the deduction + a queued side-effect
            // builder. The cost can't drain the whole pool: if the
            // declared deduction would zero the sneak dice, the prime is
            // refused (RAW: "you can't reduce the number of dice rolled to
            // less than 1"). Side-effects from the consumed prime are
            // appended to the swing's vec below.
            let (sneak_dice, mut cunning_effects) =
                consume_cunning_strike(encounter, caster_id, target_id, total_sneak_dice);
            side_effects.append(&mut cunning_effects);

            let sneak_raw = encounter.roll(&Dice::new(sneak_dice, 6));
            let sneak_extra = if is_crit {
                encounter.roll(&Dice::new(sneak_dice, 6))
            } else {
                0
            };
            let sneak_total = sneak_raw + sneak_extra;
            damage = damage.saturating_add(sneak_total);
            encounter.log(format!(
                "  sneak attack: {}d6({}) = {} extra piercing",
                sneak_dice, sneak_raw, sneak_total
            ));
            if let Some(rogue) = encounter.actors.get_mut(&caster_id) {
                rogue.mark_sneak_attack_used();
            }
        }

        encounter.log(format!(
            "  shortsword: total {} piercing damage{}",
            damage,
            if is_crit { " (crit)" } else { "" }
        ));

        // Primary damage lands first so any condition follow-ups (e.g.
        // the Daze rider's MindWhipped) read the post-damage state.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Piercing,
        })];
        effects.append(&mut side_effects);
        effects
    }
}

pub static ROGUE_SHORTSWORD: LazyLock<RogueShortsword> = LazyLock::new(|| RogueShortsword {});

/// 5e Sneak Attack dice scaling: ceil(level / 2) d6.
/// Level 1 = 1d6, level 3 = 2d6, level 5 = 3d6, etc.
/// Exposed for cross-module gates (the Cunning Strike (Daze) prime and
/// the AI's Cunning Strike heuristic both check the pool size before
/// committing the bonus action).
pub fn sneak_attack_dice_for_level(level: u32) -> u32 {
    level.div_ceil(2).max(1)
}

/// 5e 2024 Rogue **Cunning Strike** consume site. Walks the active
/// prime conditions on `rogue_id`, picks the first match, deducts the
/// die cost from `total_sneak_dice`, builds the queued side-effects (a
/// save + condition apply for Poison / Trip / Daze, a free half-speed
/// move-resource grant + Disengaging install for Withdraw), and clears
/// the prime via `remove_condition`.
///
/// Returns `(remaining_sneak_dice, side_effects)`. Refuses to consume
/// the prime if doing so would drop the sneak pool below 1 die (RAW:
/// "you can't reduce the number of dice rolled to less than 1"); the
/// prime stays installed so a later swing with a richer pool can
/// connect.
fn consume_cunning_strike(
    encounter: &mut EncounterInstance,
    rogue_id: usize,
    target_id: usize,
    total_sneak_dice: u32,
) -> (u32, Vec<Box<dyn ApplicableSideEffect>>) {
    use crate::engine::types::AbilityScoreType;
    let Some(rogue) = encounter.actors.get(&rogue_id) else {
        return (total_sneak_dice, Vec::new());
    };
    // First-match wins. Order is fixed (Poison → Trip → Daze → Withdraw)
    // so the consume order is deterministic for tests; the prime-install
    // gates prevent double-priming so only one condition is ever active
    // anyway.
    const PRIORITY: &[Condition] = &[
        Condition::CunningStrikePoison,
        Condition::CunningStrikeTrip,
        Condition::CunningStrikeDaze,
        Condition::CunningStrikeWithdraw,
    ];
    let Some(prime) = PRIORITY.iter().copied().find(|c| rogue.has_condition(*c))
    else {
        return (total_sneak_dice, Vec::new());
    };
    let cost = match prime {
        Condition::CunningStrikeDaze => 2,
        _ => 1,
    };
    if total_sneak_dice <= cost {
        // RAW: can't reduce sneak below 1 die. Leave the prime up — the
        // rogue will get another chance next swing or next round.
        return (total_sneak_dice, Vec::new());
    }
    let remaining = total_sneak_dice - cost;
    let dc = rogue.spell_save_dc(AbilityScoreType::Dexterity);
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    match prime {
        Condition::CunningStrikePoison => {
            encounter.log(format!(
                "  cunning strike (poison): -1d6 sneak \u{2192} CON save vs DC {}",
                dc
            ));
            let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
            if !save.passed() {
                encounter.log("  cunning strike (poison): target is poisoned.");
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Poisoned,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        Condition::CunningStrikeTrip => {
            // Trip RAW: target must be Large or smaller. Sized cap mirrors
            // Shove / Grapple's footprint gate.
            let target_too_big = encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.size().ordinal() > crate::engine::types::Size::Large.ordinal());
            if target_too_big {
                encounter
                    .log("  cunning strike (trip): target too large to knock down.".to_string());
            } else {
                encounter.log(format!(
                    "  cunning strike (trip): -1d6 sneak \u{2192} DEX save vs DC {}",
                    dc
                ));
                let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
                if !save.passed() {
                    encounter.log("  cunning strike (trip): target is knocked prone.");
                    effects.push(Box::new(ApplyCondition {
                        actor_id: target_id,
                        condition: Condition::Prone,
                        timer: ConditionTimer::Permanent,
                    }));
                }
            }
        }
        Condition::CunningStrikeDaze => {
            encounter.log(format!(
                "  cunning strike (daze): -2d6 sneak \u{2192} CON save vs DC {}",
                dc
            ));
            let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
            if !save.passed() {
                encounter.log(
                    "  cunning strike (daze): target loses their next action and reaction.",
                );
                // MindWhipped: action-economy clip on next turn.
                // NoReaction: blocks reactions until start of next turn.
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::MindWhipped,
                    timer: ConditionTimer::Rounds(1),
                }));
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::NoReaction,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        Condition::CunningStrikeWithdraw => {
            // RAW: "Immediately after the attack, you can move up to half
            // your Speed without provoking opportunity attacks."
            // Model: grant Movement budget for half speed + install
            // Disengaging so the granted budget skips OAs.
            let half_speed = rogue.speed() / 2.0;
            encounter.log(format!(
                "  cunning strike (withdraw): -1d6 sneak \u{2192} +{:.1}ft of OA-free movement.",
                half_speed
            ));
            effects.push(Box::new(GiveResource {
                actor_id: rogue_id,
                resource: Resource::Movement(half_speed),
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: rogue_id,
                condition: Condition::Disengaging,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        _ => {}
    }
    // Burn the prime regardless of save outcome — RAW: "the die cost is
    // subtracted whether or not the effect lands."
    if let Some(rogue) = encounter.actors.get_mut(&rogue_id) {
        rogue.remove_condition(prime);
    }
    (remaining, effects)
}

/// 5e Sneak Attack trigger. Fires when:
///   - The rogue has advantage on the attack roll (RAW's canonical
///     surprise / help / hidden path — disadvantage cancels advantage
///     via `RollMode::combine` so the branch trivially subsumes RAW's
///     "no disadvantage" clause). OR
///   - An ally of the rogue is footprint-adjacent to the target AND
///     the rogue's attack is not at disadvantage. RAW: "another enemy
///     of the target is within 5 feet of it, that enemy isn't
///     incapacitated, and you don't have disadvantage on the attack
///     roll". The incapacitated-ally check collapses to the
///     `is_combat_active` gate. OR
///   - The rogue holds `has_rakish_audacity` (Swashbuckler subclass
///     level 3) AND the target is footprint-adjacent to the rogue AND
///     no OTHER creature is within 5 ft of the rogue AND the attack
///     is not at disadvantage. RAW: "you don't need advantage on the
///     attack roll to use your Sneak Attack against a creature if you
///     are within 5 feet of it, no other creatures are within 5 feet
///     of you, and you don't have disadvantage on the attack roll".
///
/// AND the rogue hasn't already used Sneak Attack this turn.
///
/// Takes the full `RollMode` (not just `has_advantage`) so the ally-
/// adjacent and Rakish Audacity paths can enforce RAW's "no
/// disadvantage" clause uniformly — a rogue swinging at Disadvantage
/// (Blinded, Poisoned, prone-target-melee, etc.) no longer picks up
/// Sneak Attack via either path.
fn sneak_attack_eligible(
    encounter: &EncounterInstance,
    rogue_id: usize,
    target_id: usize,
    mode: crate::engine::dice::RollMode,
) -> bool {
    let Some(rogue) = encounter.actors.get(&rogue_id) else {
        return false;
    };
    if rogue.sneak_attack_used() {
        return false;
    }
    if mode == crate::engine::dice::RollMode::Advantage {
        return true;
    }
    // Both remaining paths (ally-adjacent, Rakish Audacity solo-duelist)
    // require the swing to not be at disadvantage per RAW.
    if mode == crate::engine::dice::RollMode::Disadvantage {
        return false;
    }
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    let target_loc = target.location();
    let target_size = get_tiles_from_size(target.size());
    // Ally-adjacent path — the classic "flanker enables sneak" trigger.
    let ally_adjacent = encounter.actors.iter().any(|(id, a)| {
        if *id == rogue_id || a.team() != rogue.team() || !a.is_combat_active() {
            return false;
        }
        let dist = footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            target_loc,
            target_size,
        );
        // 5ft adjacency = footprint-Chebyshev gap of 0 (touching).
        dist == 0
    });
    if ally_adjacent {
        return true;
    }
    // Rakish Audacity path (Swashbuckler subclass level 3) — the rogue
    // stands alone with the target: target footprint-adjacent, no other
    // hostile creature footprint-adjacent to the rogue. "Other creatures"
    // in RAW means anyone besides the rogue and the target — a nearby
    // ally does NOT gate the trigger since the ally-adjacent path above
    // would have already fired. We check for "no other enemies within
    // 5 ft of the rogue" — matching the RAW spirit that the swash
    // duels one target without interference from other threats.
    if rogue.has_rakish_audacity() {
        let rogue_loc = rogue.location();
        let rogue_size = get_tiles_from_size(rogue.size());
        // Target must be adjacent to the swash.
        let target_adjacent = footprint_chebyshev(rogue_loc, rogue_size, target_loc, target_size)
            == 0;
        // No OTHER hostile creature within 5 ft of the swash. RAW says
        // "no other creatures within 5 feet of you"; we scope to
        // hostile-to-swash to avoid a swash allied with the flanking
        // rogue's cover from being locked out of Rakish Audacity when
        // the ally is nearby. This is a mild deviation from strict RAW
        // (which counts any creature) but matches the RAW spirit — the
        // swash duels the target one-on-one, ally presence doesn't
        // cancel the "solo duel" identity.
        let no_other_enemy_adjacent = !encounter.actors.iter().any(|(id, a)| {
            if *id == rogue_id || *id == target_id || !a.is_combat_active() {
                return false;
            }
            if a.team() == rogue.team() {
                return false;
            }
            let dist =
                footprint_chebyshev(a.location(), get_tiles_from_size(a.size()), rogue_loc, rogue_size);
            dist == 0
        });
        if target_adjacent && no_other_enemy_adjacent {
            return true;
        }
    }
    false
}

/// The beast form's Strength modifier, and the reason this attack has
/// its own impl instead of being a `SimpleWeapon` declaration.
///
/// RAW's Wild Shape is explicit: "your game statistics are replaced by
/// the statistics of the beast." Every other weapon in the engine reads
/// its to-hit and damage modifiers off the *wielder*, which is right
/// for a scimitar and wrong for a bear — a Wisdom-primary druid with
/// STR 10 wielding a brown bear's claws at +0 is not the feature. So
/// the modifier is a constant of the form rather than a lookup on the
/// actor: +4, the brown bear's Strength 19.
///
/// The brown bear is the reference form because Circle of the Moon's
/// **Circle Forms** caps the druid at CR 1 at subclass level 2, and the
/// bear is the CR-1 beast the class is famous for taking. Its claws are
/// 2d6+4 slashing, which is what `BEAST_FORM_CLAWS` deals.
///
/// Proficiency still comes from the druid — RAW keeps the character's
/// proficiency bonus through the transformation — so a higher-level
/// moon druid's claws get more accurate without the form changing.
const BEAST_FORM_STR_MOD: i32 = 4;

/// Beast-form claws — the natural weapon a Wild Shaped druid swings.
/// 2d6+4 slashing at melee reach, to-hit at the druid's proficiency
/// bonus plus the form's +4 Strength.
///
/// Gated on `Condition::WildShaped`, which is the whole reason it can
/// sit on the druid's action list permanently: out of form the
/// validator refuses it, in form it is the only attack that matters
/// (the scimitar is still legal but strictly worse, and the spell list
/// is locked out entirely by `blocks_spell_slots`).
///
/// Routes through the shared `resolve_attack` chokepoint rather than
/// rolling inline the way `RogueShortsword` does, so every rider that
/// keys off a weapon hit — Extra Attack chaining is the one that
/// matters here, but also the once-per-turn die cohort and the
/// Eldritch Strike mark — sees the swing as the weapon attack it is.
pub struct BeastFormClaws {}

impl Action for BeastFormClaws {
    fn name(&self) -> &str {
        "beast claws"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bc", "maul"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::WildShaped))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::attack::{AttackParams, resolve_attack};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(prof) = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.proficiency_bonus())
        else {
            return Vec::new();
        };
        // The form's Strength drives both halves; the druid contributes
        // only their proficiency bonus, per RAW's "you retain your
        // proficiency bonuses" clause.
        let swing = |e: &mut EncounterInstance| {
            resolve_attack(
                e,
                AttackParams {
                    caster_id,
                    target_id,
                    action_name: "beast claws",
                    attack_bonus: prof + BEAST_FORM_STR_MOD,
                    damage_dice: Dice::new(2, 6),
                    damage_bonus: BEAST_FORM_STR_MOD,
                    damage_type: DamageType::Slashing,
                    is_melee: true,
                    long_range: None,
                    is_spell: false,
                },
            )
        };
        let mut effects = swing(encounter);
        crate::actions::monster_attacks::maybe_chain_extra_attack(
            encounter,
            caster_id,
            &mut effects,
            swing,
        );
        effects
    }
}

pub static BEAST_FORM_CLAWS: LazyLock<BeastFormClaws> = LazyLock::new(|| BeastFormClaws {});
