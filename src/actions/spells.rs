use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Heal, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// Sacred Flame — cleric cantrip. Range 60ft (24 tiles), DEX save vs the
/// caster's WIS-based spell save DC. On fail: 1d8 radiant. On success:
/// nothing (cantrips don't half-on-save). No spell slot consumed.
pub struct SacredFlame {}

impl Action for SacredFlame {
    fn name(&self) -> &str {
        "sacred flame"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sf", "flame"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
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
    ) -> Vec<Resource> {
        // Cantrip — costs an Action only, no spell slot consumed.
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);

        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
        if save.passed() {
            return Vec::new();
        }

        let raw = encounter.roll(&Dice::new(1, 8));
        encounter.log(format!("  sacred flame: 1d8({}) = {} radiant", raw, raw));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Radiant,
        })]
    }
}

pub static SACRED_FLAME: LazyLock<SacredFlame> = LazyLock::new(|| SacredFlame {});

/// Healing Word — cleric spell. Bonus action, range 60ft (24 tiles), no
/// save: target regains 1d4 + caster's WIS modifier HP. We currently treat
/// it as a cantrip (no spell slot) until spell-slot levels are wired.
/// LOS not strictly required in 5e (audibly heard), but we require it for
/// simplicity until "audible reach" is a thing.
pub struct HealingWord {}

impl Action for HealingWord {
    fn name(&self) -> &str {
        "healing word"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hw", "heal"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Healing Word — bonus-action level-1 spell.
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(1, 4)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  healing word: 1d4({}){:+} = {} HP",
            raw, wis_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static HEALING_WORD: LazyLock<HealingWord> = LazyLock::new(|| HealingWord {});

/// Sacred Burst — generic cleric AoE cantrip. Pick a tile within 30ft;
/// every actor (friend or foe) whose footprint touches the burst takes
/// 2d6 radiant on a failed DEX save vs the caster's WIS-based DC, half
/// on success. Damage is rolled once and shared. Requires LOS to the
/// burst origin (not to each target).
pub struct SacredBurst {}

impl Action for SacredBurst {
    fn name(&self) -> &str {
        "sacred burst"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sb", "burst"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
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
    ) -> Vec<Resource> {
        // Cantrip — Action only, no spell slot.
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let radius: isize = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };

        // Roll damage once for the whole burst (5e: shared damage roll).
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  sacred burst: 2d6({}) = {} radiant area",
            raw, raw
        ));

        // Snapshot affected ids in actor-id order for deterministic save
        // sequencing — order matters because the roller is shared and each
        // save consumes a d20.
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in ids {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            // The caster is exempt — sacred-flavored AoE wouldn't burn its
            // own caster. Inactive actors (dying/stable) also skip.
            if target_id == caster_id || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                point,
                1,
            );
            if dist > radius {
                continue;
            }

            let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg == 0 {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Radiant,
            }));
        }
        effects
    }
}

pub static SACRED_BURST: LazyLock<SacredBurst> = LazyLock::new(|| SacredBurst {});

/// Hold Person — 5e-flavored single-target paralysis. WIS save vs the
/// caster's WIS-based DC; on fail, target is Stunned (close enough to
/// Paralyzed for our model, since we don't yet model auto-fail STR/DEX
/// saves or melee crit-on-hit) for 10 rounds. Concentration: when the
/// caster takes damage and fails a CON save (or hits 0 HP), the spell
/// drops and Stunned clears immediately.
pub struct HoldPerson {}

impl Action for HoldPerson {
    fn name(&self) -> &str {
        "hold person"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hp", "hold"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
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
    ) -> Vec<Resource> {
        // Level-2 leveled spell — Action + a level-2 spell slot.
        vec![Resource::Action, Resource::SpellSlot(2)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);

        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }

        // Apply Stunned for up to 10 rounds, plus install concentration
        // tracking so a damage-failed CON save will release the target.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Stunned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Hold Person".to_string(),
                    conditions: vec![(target_id, Condition::Stunned)],
                },
            }),
        ]
    }
}

pub static HOLD_PERSON: LazyLock<HoldPerson> = LazyLock::new(|| HoldPerson {});

/// Cure Wounds — touch-range single-target heal. 1d8 + caster's
/// spellcasting ability modifier (we use WIS for clerics by convention;
/// a wider class system would key off the caster's primary spell stat).
/// Action + level-1 spell slot.
pub struct CureWounds {}

impl Action for CureWounds {
    fn name(&self) -> &str {
        "cure wounds"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cw", "cure"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Touch range = melee reach in 5e.
        Some(crate::actions::action_template::MELEE_REACH)
    }

    fn requires_los(&self) -> bool {
        false
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(1, 8)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  cure wounds: 1d8({}){:+} = {} HP",
            raw, wis_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static CURE_WOUNDS: LazyLock<CureWounds> = LazyLock::new(|| CureWounds {});

/// Magic Missile — three darts of force, each auto-hits and deals 1d4+1.
/// 5e RAW: caster picks each target individually; we simplify by sending
/// all three darts at one target. No attack roll, no save.
/// Action + level-1 spell slot, range 120ft (well past our maps).
pub struct MagicMissile {}

impl Action for MagicMissile {
    fn name(&self) -> &str {
        "magic missile"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mm", "missile"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
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
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        // Each dart rolled separately so each gets its own resistance pass
        // (force damage is resisted by very few creatures, but wiring it
        // through DealDamage keeps the math consistent).
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut total: u32 = 0;
        for _ in 0..3 {
            let raw = encounter.roll(&Dice::new(1, 4));
            let amount = raw + 1;
            total += amount;
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount,
                damage_type: DamageType::Force,
            }));
        }
        encounter.log(format!("  magic missile: 3 darts × 1d4+1 = {} force", total));
        effects
    }
}

pub static MAGIC_MISSILE: LazyLock<MagicMissile> = LazyLock::new(|| MagicMissile {});

/// Burning Hands — 15-ft cone of fire from the caster. 5e RAW is a 15ft
/// cone; we model the cone as a 2-tile-radius burst centered on the
/// targeted point next to the caster, since `TargetingSchema::Burst` is
/// what we have. DEX save vs caster's CHA-based DC for half (we use WIS
/// to match cleric convention until we model class-specific spell stats).
/// 3d6 fire, level-1 spell.
pub struct BurningHands {}

impl Action for BurningHands {
    fn name(&self) -> &str {
        "burning hands"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bh", "fire"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Center must be footprint-adjacent to caster — 15ft cone from
        // the caster, not at distance.
        Some(2)
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
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let radius: isize = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };

        let raw = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!("  burning hands: 3d6({}) = {} fire", raw, raw));

        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in ids {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            if target_id == caster_id || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                point,
                1,
            );
            if dist > radius {
                continue;
            }
            let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg == 0 {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Fire,
            }));
        }
        effects
    }
}

pub static BURNING_HANDS: LazyLock<BurningHands> = LazyLock::new(|| BurningHands {});

/// Bless — concentration buff. Up to 3 allies (we simplify to 1 target)
/// gain a `+save` and `+attack` bonus modeled as a +1d4 worth on save
/// rolls. We model it via the target picking up `Item`-style save bonus
/// directly through a condition flag — but to avoid yet another bonus
/// pipeline we represent it as a permanent-until-concentration-ends
/// condition the engine reads in attack/save rolls.
///
/// For now we keep the implementation simple: Bless applies a concentration
/// hook that gives the target the `Blessed` flag (no condition variant
/// exists yet, so we model it as Dodging-equivalent in compute_attack_mode
/// — i.e. attacks against the target have disadvantage). This is a
/// deliberate-but-imperfect simplification documented inline. Replace
/// once a numeric save bonus pipeline lands.
pub struct Bless {}

impl Action for Bless {
    fn name(&self) -> &str {
        "bless"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bl"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        // Until a numeric "+save" pipeline exists, model Bless as a
        // 10-round Dodging-flavored condition: attacks against the
        // blessed target take disadvantage. Concentration on the
        // caster cleans this up automatically when the spell drops.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Dodging,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Bless".to_string(),
                    conditions: vec![(target_id, Condition::Dodging)],
                },
            }),
        ]
    }
}

pub static BLESS: LazyLock<Bless> = LazyLock::new(|| Bless {});

/// Inflict Wounds — touch-range damaging spell. Melee spell attack vs
/// AC: on hit, 3d10 necrotic. No save. Level-1 slot. Stronger than Magic
/// Missile in raw damage but requires both a hit roll AND being in melee.
pub struct InflictWounds {}

impl Action for InflictWounds {
    fn name(&self) -> &str {
        "inflict wounds"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["iw", "inflict"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::modifier_from_score;
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let attack_bonus = wis_mod + caster.proficiency_bonus();
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        let mode = encounter.compute_attack_mode(caster_id, target_id, true);
        let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
        let attack_total = raw_attack + attack_bonus;
        let is_crit = raw_attack == 20;
        let hit = is_crit || attack_total >= target_ac;
        let outcome = if is_crit {
            "CRIT!"
        } else if hit {
            "hit"
        } else {
            "miss"
        };
        encounter.log(format!(
            "  inflict wounds: 1d20({}){:+} = {} vs AC {}{} \u{2014} {}",
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
        let dice = Dice::new(3, 10);
        let raw = encounter.roll(&dice);
        let extra = if is_crit { encounter.roll(&dice) } else { 0 };
        let damage = raw + extra;
        encounter.log(format!(
            "  inflict wounds: 3d10({}) {} = {} necrotic{}",
            raw,
            if is_crit { format!("+ 3d10({})", extra) } else { String::new() },
            damage,
            if is_crit { " (crit)" } else { "" },
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static INFLICT_WOUNDS: LazyLock<InflictWounds> = LazyLock::new(|| InflictWounds {});
