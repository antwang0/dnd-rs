use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, GainTempHp, Heal, Resource},
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

    fn heals(&self) -> bool {
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
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
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

        // Roll damage once for the whole burst (5e: shared damage roll).
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
        use crate::engine::side_effects::{AdjustAttackBuff, AdjustSaveBuff, StartConcentration};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };

        // Bless adds +1d4 (avg ~2.5) — we apply a flat +2 to keep dice
        // counts steady and the shape simple. Concentration hook drops
        // via the side-effect ordering: install buffs first, then start
        // concentration (which records nothing-to-roll-back; cleanup
        // happens via the encounter's concentration hook below).
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: target_id,
                delta: 2,
            }),
            Box::new(AdjustSaveBuff {
                actor_id: target_id,
                delta: 2,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Bless".to_string(),
                    conditions: Vec::new(),
                    // Record the +2 buff so concentration cleanup can
                    // negate it cleanly when the spell drops.
                    attack_buffs: vec![(target_id, 2)],
                    save_buffs: vec![(target_id, 2)],
                },
            }),
        ]
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
        // Three darts; each auto-hits for 1d4+1 force. We sum the rolls
        // so the target only takes one DealDamage hit (saves a row of
        // log spam and a multi-effect concentration save chain).
        let mut total: u32 = 0;
        for _ in 0..3 {
            let raw = encounter.roll(&Dice::new(1, 4));
            total = total.saturating_add(raw + 1);
        }
        encounter.log(format!("  magic missile: 3 darts \u{2014} {} force", total));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total,
            damage_type: DamageType::Force,
        })]
    }
}

pub static MAGIC_MISSILE: LazyLock<MagicMissile> = LazyLock::new(|| MagicMissile {});
