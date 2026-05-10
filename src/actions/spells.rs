use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{first_target_id, Action, TargetingSchema},
    actors::actor_template::ConcentrationData,
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, Heal, Resource,
            StartConcentration,
        },
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// Resolve a spell attack roll vs `target_id`'s AC. Logs the breakdown,
/// rolls damage on hit (with crit-doubled dice on a nat-20), returns the
/// queued `DealDamage` (or nothing on a miss). Centralized so single-target
/// damaging spells (Guiding Bolt, Inflict Wounds, future spells) don't
/// each re-implement attack-roll + crit + log glue.
#[allow(clippy::too_many_arguments)]
fn spell_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let target_ac = encounter
        .actors
        .get(&target_id)
        .map(|a| a.armor_class() as i32)
        .unwrap_or(10);
    let mode = encounter.compute_attack_mode(caster_id, target_id, is_melee);
    let raw = encounter.roll_d20_with_mode(mode) as i32;
    let total = raw + attack_bonus;
    let is_crit = raw == 20;
    let hit = is_crit || total >= target_ac;
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
        raw,
        attack_bonus,
        total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let dmg = encounter.roll(&damage_dice);
    let crit_extra = if is_crit { encounter.roll(&damage_dice) } else { 0 };
    let total_dmg = dmg + crit_extra;
    encounter.log(format!(
        "  {}: {}({}) = {} {:?}{}",
        action_name,
        damage_dice,
        dmg,
        total_dmg,
        damage_type,
        if is_crit {
            format!(" (+{} crit)", crit_extra)
        } else {
            String::new()
        }
    ));
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: total_dmg,
        damage_type,
    })]
}

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

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
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
        let Some(target_id) = first_target_id(target_ids) else {
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

/// Single-target healing spell driven by configuration. Replaces the
/// per-spell impls of Healing Word and Cure Wounds — they only differ
/// in name, reach, action-economy slot, and dice. Heal amount = roll +
/// caster's `ability` modifier.
pub struct HealSpell {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub reach: isize,
    pub requires_los: bool,
    /// Action / BonusAction. Plus a level-`spell_slot_lvl` slot.
    pub action_cost: Resource,
    pub spell_slot_lvl: u32,
    pub heal_dice: Dice,
    /// Spellcasting ability whose modifier is added to the heal roll.
    pub ability: AbilityScoreType,
}

impl Action for HealSpell {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }
    fn requires_los(&self) -> bool {
        self.requires_los
    }
    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
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
        vec![self.action_cost, Resource::SpellSlot(self.spell_slot_lvl)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let ability_mod = modifier_from_score(caster.ability_score(self.ability));
        let raw = encounter.roll(&self.heal_dice) as i32;
        let amount = (raw + ability_mod).max(1) as u32;
        encounter.log(format!(
            "  {}: {}({}){:+} = {} HP",
            self.display_name, self.heal_dice, raw, ability_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

/// Healing Word — bonus-action level-1 heal at 60ft (24 tiles).
pub static HEALING_WORD: HealSpell = HealSpell {
    display_name: "healing word",
    aliases: &["hw", "heal"],
    reach: 24,
    requires_los: true,
    action_cost: Resource::BonusAction,
    spell_slot_lvl: 1,
    heal_dice: Dice::new(1, 4),
    ability: AbilityScoreType::Wisdom,
};

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

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
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

        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  sacred burst: 2d6({}) = {} radiant area",
            raw, raw
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in encounter.actors_in_burst(point, radius) {
            // The caster is exempt — sacred-flavored AoE wouldn't burn its
            // own caster.
            if target_id == caster_id {
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
/// caster's WIS-based DC; on fail, target is Stunned for 10 rounds.
/// Stunned models paralysis: blocks actions/movement, auto-fails STR/DEX
/// saves, and attackers roll with advantage. Missing: the 5-foot crit rule
/// (melee hits auto-crit vs paralyzed targets). Concentration: when the
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

        let Some(target_id) = first_target_id(target_ids) else {
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
                data: ConcentrationData::with_conditions(
                    "Hold Person",
                    vec![(target_id, Condition::Stunned)],
                ),
            }),
        ]
    }
}

pub static HOLD_PERSON: LazyLock<HoldPerson> = LazyLock::new(|| HoldPerson {});

/// Cure Wounds — 5e level-1 cleric/druid/bard spell. Touch range, no save:
/// target regains 1d8 + caster's WIS modifier HP. Compared to Healing
/// Word: Cure Wounds is a full Action (not bonus action) but heals more
/// on average. Both consume a level-1 slot.
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
        // Touch range — must be footprint-adjacent to the target.
        Some(1)
    }

    fn requires_los(&self) -> bool {
        // Touch implicitly requires LOS, but the reach check already
        // guarantees adjacency, so the LOS check is harmless overhead.
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

/// Fire Bolt — 5e wizard cantrip. Ranged spell attack: d20 + caster's
/// INT modifier vs target AC. On hit: 1d10 fire damage. No save (it's
/// an attack roll, not a save spell). Crits double the damage dice.
pub struct FireBolt {}

impl Action for FireBolt {
    fn name(&self) -> &str {
        "fire bolt"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fb", "bolt"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft range — well past any current map.
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
        // Cantrip — Action only.
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
        use crate::engine::attack::{AttackParams, resolve_attack};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 10),
                damage_bonus: 0,
                damage_type: DamageType::Fire,
                is_melee: false,
            },
        )
    }
}

pub static FIRE_BOLT: LazyLock<FireBolt> = LazyLock::new(|| FireBolt {});

/// Bless — 5e level-1 concentration spell. For up to 3 targets, each
/// gets +1d4 to attack rolls and saving throws while the spell lasts
/// (we approximate the d4 as a flat +2 — average roll on a d4 = 2.5,
/// rounded to keep math integer). Concentration; drops cleanly via the
/// existing concentration cleanup hook.
///
/// Schema is SingleActor for simplicity — the AI / picker can cast it
/// once per ally per round. Models the multi-target version on top of
/// the single-target schema by buffing the target chosen.
pub struct Bless {}

impl Action for Bless {
    fn name(&self) -> &str {
        "bless"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bl"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        // Custom: accepts no args (auto-target all nearby allies) or
        // a single SingleActor (just bless that one ally + caster).
        TargetingSchema::Custom
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
        use crate::engine::side_effects::{AdjustAttackBuff, AdjustSaveBuff};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        // 5e Bless: pick targets in range. If `target_ids` was supplied,
        // bless those (typical UI flow). Otherwise auto-pick the caster
        // and every combat-active ally within 12 tiles.
        let mut targets: Vec<usize> = match target_ids {
            Some(v) if !v.is_empty() => v.clone(),
            _ => {
                let Some(caster) = encounter.actors.get(&caster_id) else {
                    return Vec::new();
                };
                let caster_team = caster.team();
                let caster_loc = caster.location();
                let caster_size = get_tiles_from_size(caster.size());
                const REACH: isize = 12;
                let mut out = Vec::new();
                let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
                ids.sort_unstable();
                for tid in ids {
                    let Some(target) = encounter.actors.get(&tid) else {
                        continue;
                    };
                    if target.team() != caster_team || !target.is_combat_active() {
                        continue;
                    }
                    let dist = footprint_chebyshev(
                        target.location(),
                        get_tiles_from_size(target.size()),
                        caster_loc,
                        caster_size,
                    );
                    if dist > REACH {
                        continue;
                    }
                    out.push(tid);
                }
                out
            }
        };
        // Always include the caster — Bless can target the caster too.
        if !targets.contains(&caster_id) {
            targets.insert(0, caster_id);
        }
        if targets.is_empty() {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        let mut attack_buffs = Vec::new();
        let mut save_buffs = Vec::new();
        for tid in &targets {
            effects.push(Box::new(AdjustAttackBuff {
                actor_id: *tid,
                delta: 2,
            }));
            effects.push(Box::new(AdjustSaveBuff {
                actor_id: *tid,
                delta: 2,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: *tid,
                condition: Condition::Blessed,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((*tid, Condition::Blessed));
            attack_buffs.push((*tid, 2));
            save_buffs.push((*tid, 2));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData {
                spell_name: "Bless".to_string(),
                conditions,
                attack_buffs,
                save_buffs,
            },
        }));
        effects
    }
}

pub static BLESS: LazyLock<Bless> = LazyLock::new(|| Bless {});

/// Burning Hands — 5e level-1 evocation. 15-foot cone (we approximate as
/// a 3-tile burst centered on the target tile, since cones aren't yet
/// modeled). Every actor in the burst takes 3d6 fire on a failed DEX
/// save, half on success. Consumes a level-1 slot.
pub struct BurningHands {}

impl Action for BurningHands {
    fn name(&self) -> &str {
        "burning hands"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bh", "hands"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 15 ft cone — short range. We treat the burst origin as the
        // far edge of the cone.
        Some(6)
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let radius: isize = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };

        let raw = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!("  burning hands: 3d6({}) = {} fire area", raw, raw));

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

/// Magic Missile — 5e level-1 evocation. Three darts, each auto-hitting
/// (no attack roll, no save) for 1d4+1 force damage. We model it as a
/// single-target spell that fires all three darts at the chosen target;
/// the multi-target split-fire variant is an extension.
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
        // Three darts; each auto-hits for 1d4+1 force. We emit three
        // separate DealDamage effects so concentration-on-hit saves
        // trigger per-dart (RAW: each dart counts as a separate hit
        // for concentration check purposes).
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::with_capacity(3);
        let mut totals = [0u32; 3];
        for (i, t) in totals.iter_mut().enumerate() {
            let raw = encounter.roll(&Dice::new(1, 4));
            *t = raw + 1;
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: *t,
                damage_type: DamageType::Force,
            }));
            let _ = i;
        }
        encounter.log(format!(
            "  magic missile: 3 darts [{}, {}, {}] force",
            totals[0], totals[1], totals[2]
        ));
        effects
    }
}

pub static MAGIC_MISSILE: LazyLock<MagicMissile> = LazyLock::new(|| MagicMissile {});

/// Shield of Faith — concentration buff that grants +2 AC to a willing
/// target for the spell's duration (10 rounds). Costs an Action and a
/// level-1 spell slot. Tracked as the `ShieldOfFaith` condition; the
/// engine reads it from `armor_class()` at attack-resolution time.
pub struct ShieldOfFaith {}

impl Action for ShieldOfFaith {
    fn name(&self) -> &str {
        "shield of faith"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sof", "shield-of-faith"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::ShieldOfFaith,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Shield of Faith".to_string(),
                    conditions: vec![(target_id, Condition::ShieldOfFaith)],
                    attack_buffs: Vec::new(),
                    save_buffs: Vec::new(),
                },
            }),
        ]
    }
}

pub static SHIELD_OF_FAITH: LazyLock<ShieldOfFaith> = LazyLock::new(|| ShieldOfFaith {});

/// Cause Fear — single-target WIS save vs spell DC; on fail, target is
/// Frightened for up to 3 rounds (concentration). Costs an Action and a
/// level-1 spell slot.
pub struct CauseFear {}

impl Action for CauseFear {
    fn name(&self) -> &str {
        "cause fear"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cf", "fear"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
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
        let Some(target_id) = first_target_id(target_ids) else {
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
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(3),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Cause Fear".to_string(),
                    conditions: vec![(target_id, Condition::Frightened)],
                    attack_buffs: Vec::new(),
                    save_buffs: Vec::new(),
                },
            }),
        ]
    }
}

pub static CAUSE_FEAR: LazyLock<CauseFear> = LazyLock::new(|| CauseFear {});

/// Guiding Bolt — level-1 ranged spell attack. 4d6 radiant on hit; the
/// next attack against the target before the end of the caster's next
/// turn has advantage. Demonstrates timed-rider conditions.
pub struct GuidingBolt {}

impl Action for GuidingBolt {
    fn name(&self) -> &str {
        "guiding bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gb", "bolt"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_mod,
            Dice::new(4, 6),
            DamageType::Radiant,
            false,
        );
        if !effects.is_empty() {
            // Mark target so the next incoming attack benefits.
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::GuidingBoltLit,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static GUIDING_BOLT: LazyLock<GuidingBolt> = LazyLock::new(|| GuidingBolt {});

/// Web — level-2 conjuration. AoE 4-tile burst, 1-minute concentration.
/// Targets in the burst make a DEX save; fail = Restrained, success =
/// no effect. We don't model the "difficult terrain" clause yet (no
/// terrain-mod system); the Restrained condition does the heavy lifting.
pub struct Web {}

impl Action for Web {
    fn name(&self) -> &str {
        "web"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(2)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        for tid in ids {
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !target.is_combat_active() {
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
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Restrained,
                    timer: ConditionTimer::Rounds(10),
                }));
                conditions.push((tid, Condition::Restrained));
            }
        }
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Web".to_string(),
                    conditions,
                    attack_buffs: Vec::new(),
                    save_buffs: Vec::new(),
                },
            }));
        }
        effects
    }
}

pub static WEB: LazyLock<Web> = LazyLock::new(|| Web {});

/// False Life — level-1 necromancy. Self-target; gain 1d4+4 temp HP.
/// Doesn't require concentration (it's a flat buff). Cleared by long
/// rest with the rest of temp HP.
pub struct FalseLife {}

impl Action for FalseLife {
    fn name(&self) -> &str {
        "false life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fl", "false-life"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(1, 4));
        let amount = raw + 4;
        encounter.log(format!(
            "  false life: 1d4({})+4 = {} temp HP",
            raw, amount
        ));
        vec![Box::new(GainTempHp {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static FALSE_LIFE: LazyLock<FalseLife> = LazyLock::new(|| FalseLife {});

/// Blindness/Deafness — level-2 necromancy. CON save vs spell DC; on
/// fail target is Blinded for 10 rounds (1 minute). Doesn't require
/// concentration in 5e — the duration runs without sustain.
pub struct Blindness {}

impl Action for Blindness {
    fn name(&self) -> &str {
        "blindness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["blind"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Blinded,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static BLINDNESS: LazyLock<Blindness> = LazyLock::new(|| Blindness {});

/// Shield — level-1 abjuration reaction (we model as a normal Action
/// for pipeline simplicity since Reaction-cost actions are also slotted
/// through the Resource enum). Adds the Shielded condition (+5 AC)
/// until the start of your next turn.
pub struct Shield {}

impl Action for Shield {
    fn name(&self) -> &str {
        "shield"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Reaction, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Shielded,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static SHIELD: LazyLock<Shield> = LazyLock::new(|| Shield {});
