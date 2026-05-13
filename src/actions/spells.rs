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
///
/// The default flavor takes no flat damage bonus; for spells that add an
/// ability modifier to damage (Spiritual Weapon, etc.) call
/// `spell_attack_with_bonus` instead. For spells that need the resolved
/// damage value (Vampiric Touch's half-as-heal rider), use
/// `spell_attack_outcome` directly.
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
    spell_attack_outcome(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        damage_dice,
        0,
        damage_type,
        is_melee,
    )
    .0
}

/// Same as `spell_attack` but adds a flat `damage_bonus` (e.g. caster's
/// WIS modifier for Spiritual Weapon) to the rolled damage. Crit doubles
/// only the dice — flat bonuses are added once per RAW.
#[allow(clippy::too_many_arguments)]
fn spell_attack_with_bonus(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    spell_attack_outcome(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        damage_dice,
        damage_bonus,
        damage_type,
        is_melee,
    )
    .0
}

/// Lower-level spell-attack resolver. Returns both the queued side-effects
/// (DealDamage on hit, empty on miss) and the post-crit damage value that
/// will land — `0` on a miss. Useful for spells that need to chain off
/// the dealt damage value (e.g. Vampiric Touch's half-as-heal rider)
/// without re-rolling the damage dice and double-consuming the RNG.
#[allow(clippy::too_many_arguments)]
fn spell_attack_outcome(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let target_ac = encounter
        .actors
        .get(&target_id)
        .map(|a| a.armor_class() as i32)
        .unwrap_or(10);
    // Spell attacks are attack rolls per 5e RAW, so the full rider stack
    // applies: Help, Hidden, Bless, Mocked, etc. Route through
    // attack_mode_with_riders so a one-shot Help grant on the caster is
    // consumed exactly once (matching weapon-attack semantics in
    // resolve_attack).
    let mode = encounter.attack_mode_with_riders(caster_id, target_id, is_melee, true);
    // Burn through the one-shot rider stack (Helped, Hidden, per-target
    // help grant, Invisibility concentration). Same hook as weapon
    // attacks — kept identical so a Helped wizard firing Fire Bolt
    // consumes their help-grant exactly like a Helped fighter swinging
    // a longsword.
    encounter.clear_attack_advantage_riders(caster_id, target_id);
    let raw = encounter.roll_d20_with_mode(mode) as i32;
    // Pull through the same caster-side flat buffs (Bless / Bane d4,
    // attack_bonus_buff) that weapon attacks get via `resolve_attack`.
    // This keeps spell-attack rolls consistent with weapon swings.
    let buff = encounter
        .actors
        .get(&caster_id)
        .map(|a| a.attack_bonus_buff())
        .unwrap_or(0);
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(caster_id);
    let total = raw + attack_bonus + buff + bless_die;
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
        "  {}: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
        action_name,
        raw,
        attack_bonus + buff,
        bless_note,
        total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return (Vec::new(), 0);
    }
    let dmg = encounter.roll(&damage_dice) as i32;
    let crit_extra = if is_crit { encounter.roll(&damage_dice) as i32 } else { 0 };
    let total_dmg = (dmg + crit_extra + damage_bonus).max(0) as u32;
    encounter.log(format!(
        "  {}: {}({}){} = {} {:?}{}",
        action_name,
        damage_dice,
        dmg,
        if damage_bonus != 0 {
            format!("{:+}", damage_bonus)
        } else {
            String::new()
        },
        total_dmg,
        damage_type,
        if is_crit {
            format!(" (+{} crit)", crit_extra)
        } else {
            String::new()
        }
    ));
    (
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total_dmg,
            damage_type,
        })],
        total_dmg,
    )
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
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Radiant,
        )
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
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Fire,
        )
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

/// Faerie Fire — level-1 evocation, concentration. Pick a tile; every
/// actor in a 4-tile burst makes a DEX save. On fail, the target is
/// Outlined: attacks against them have advantage and they can't benefit
/// from invisibility. No damage. Helpful for breaking up clumped enemies
/// or marking a tough single target. Caster takes the WIS-based DC.
pub struct FaerieFire {}

impl Action for FaerieFire {
    fn name(&self) -> &str {
        "faerie fire"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ff", "faerie"]
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
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        for tid in encounter.burst_targets(caster_id, point, radius) {
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Outlined,
                    timer: ConditionTimer::Rounds(10),
                }));
                conditions.push((tid, Condition::Outlined));
            }
        }
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Faerie Fire", conditions),
            }));
        }
        effects
    }
}

pub static FAERIE_FIRE: LazyLock<FaerieFire> = LazyLock::new(|| FaerieFire {});

/// Ray of Frost — wizard cantrip. Ranged spell attack: d20 + INT vs AC.
/// On hit: 1d8 cold damage AND the target's speed is reduced by 10ft
/// until the start of the caster's next turn (we approximate with a
/// 1-round timer on a save-buff penalty rather than a full speed
/// override; the effect is small enough that the simpler model is fine).
pub struct RayOfFrost {}

impl Action for RayOfFrost {
    fn name(&self) -> &str {
        "ray of frost"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rof", "frost"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft range.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let Some(target_id) = first_target_id(target_ids) else {
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
                damage_dice: Dice::new(1, 8),
                damage_bonus: 0,
                damage_type: DamageType::Cold,
                is_melee: false,
            },
        )
    }
}

pub static RAY_OF_FROST: LazyLock<RayOfFrost> = LazyLock::new(|| RayOfFrost {});

/// Thunderwave — level-1 evocation. 15-ft cube around the caster (we
/// approximate with a 2-tile burst centered on the caster's tile). Each
/// creature in the burst makes a CON save vs the caster's INT-DC; on
/// fail, takes 2d8 thunder and is pushed 10 ft (we don't model the
/// push). On success, half damage and no push.
pub struct Thunderwave {}

impl Action for Thunderwave {
    fn name(&self) -> &str {
        "thunderwave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tw", "thunder"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let center = caster.location();
        const RADIUS: isize = 2;
        let raw = encounter.roll(&Dice::new(2, 8));
        encounter.log(format!(
            "  thunderwave: 2d8({}) = {} thunder area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            RADIUS,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Thunder,
        )
    }
}

pub static THUNDERWAVE: LazyLock<Thunderwave> = LazyLock::new(|| Thunderwave {});

/// Misty Step — level-2 conjuration. Bonus action; teleport the caster
/// up to 30 ft (12 tiles) to an unoccupied spot you can see. No save,
/// no concentration, no damage. Pure repositioning.
pub struct MistyStep {}

impl Action for MistyStep {
    fn name(&self) -> &str {
        "misty step"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "step"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::BonusAction, Resource::SpellSlot(2)]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Destination must be a legal landing spot for this caster's full
        // footprint — same constraint as Move's pathing, minus the budget
        // check (Misty Step bypasses movement entirely).
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        // Misty Step is RAW explicitly OA-free since the caster doesn't
        // traverse intervening tiles. TeleportActor bypasses the per-step
        // OA dispatch that MoveActor uses.
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static MISTY_STEP: LazyLock<MistyStep> = LazyLock::new(|| MistyStep {});

/// Bane — level-1 enchantment, concentration. Symmetric counterpart to
/// Bless: enemy targets in range each make a CHA save vs the caster's
/// spell save DC. On fail, they're Baned for 10 rounds: -1d4 to attacks
/// and saves (we model as -2 flat via the existing condition pipeline).
/// Concentration; all stacked debuffs drop when the caster's
/// concentration drops.
pub struct Bane {}

impl Action for Bane {
    fn name(&self) -> &str {
        "bane"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Single-target form for simplicity. RAW lets the caster pick up
        // to 3 creatures, but the picker UI doesn't have a multi-actor
        // schema yet — start with one.
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Charisma, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Bane",
                    vec![(target_id, Condition::Baned)],
                ),
            }),
        ]
    }
}

pub static BANE: LazyLock<Bane> = LazyLock::new(|| Bane {});

/// Mage Armor — level-1 abjuration. Self-only; while active, the caster's
/// AC becomes 13 + DEX modifier (we model as a floor; existing AC wins
/// if higher). 8-hour duration; we use a generous 100-round timer so it
/// sticks for the whole encounter. No concentration.
pub struct MageArmor {}

impl Action for MageArmor {
    fn name(&self) -> &str {
        "mage armor"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ma", "mage-armor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::Action, Resource::SpellSlot(1)]
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
            condition: Condition::MageArmored,
            // 8 hours = effectively permanent for any single encounter.
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static MAGE_ARMOR: LazyLock<MageArmor> = LazyLock::new(|| MageArmor {});

/// Aid — level-2 abjuration. Bumps each target's max HP by 5 and
/// restores 5 HP to each. We collapse the multi-target form into a
/// single ally pick for now (the picker UI doesn't yet support multi-
/// actor selection). The HP boost is permanent for the encounter
/// (8-hour 5e duration, longer than any combat).
pub struct Aid {}

impl Action for Aid {
    fn name(&self) -> &str {
        "aid"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["a"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
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
        // Aid only affects allies — reject hostile targets at side-effect
        // time as a safety net (the harmful=false flag should already
        // steer the picker UI here).
        let caster_team = caster.team();
        if encounter
            .actors
            .get(&target_id)
            .is_none_or(|t| t.team() != caster_team)
        {
            return Vec::new();
        }
        // RAW Aid: "the target's hit point maximum and current hit
        // points increase by 5." Our `bump_max_hp` raises the base by
        // `delta` and the current HP by the same amount, capped at the
        // new max — no separate Heal needed.
        if let Some(target) = encounter.actors.get_mut(&target_id) {
            target.bump_max_hp(5);
        }
        encounter.log("  aid: +5 max HP, +5 HP".to_string());
        Vec::new()
    }
}

pub static AID: LazyLock<Aid> = LazyLock::new(|| Aid {});

/// Acid Splash — wizard cantrip. Pick a target; that creature (and one
/// adjacent creature) makes a DEX save vs spell DC. On fail: 1d6 acid.
/// Cantrips don't half-on-save. We use a tiny burst (radius 1) at the
/// target's tile to model the splash to one neighbor.
pub struct AcidSplash {}

impl Action for AcidSplash {
    fn name(&self) -> &str {
        "acid splash"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["as-spell", "splash"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Single-tile burst — simulates the "pick a creature; an
        // adjacent creature is also affected" wording with a 1-tile
        // splash radius.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Cantrip — no spell slot cost.
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!("  acid splash: 1d6({}) = {} acid", raw, raw));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.actors_in_burst(point, 1) {
            // Caster exempt — they wouldn't splash themselves.
            if tid == caster_id {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Acid,
            }));
        }
        effects
    }
}

pub static ACID_SPLASH: LazyLock<AcidSplash> = LazyLock::new(|| AcidSplash {});

/// Chill Touch — wizard cantrip. Ranged spell attack: d20 + INT vs AC.
/// On hit: 1d8 necrotic. Auxiliary RAW rider (target can't regain HP
/// until the start of caster's next turn) is omitted for now — the
/// engine doesn't yet model "no-heal" gates. Crit doubles the dice.
pub struct ChillTouch {}

impl Action for ChillTouch {
    fn name(&self) -> &str {
        "chill touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ct", "chill"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "chill touch",
            attack_bonus,
            Dice::new(1, 8),
            DamageType::Necrotic,
            false,
        )
    }
}

pub static CHILL_TOUCH: LazyLock<ChillTouch> = LazyLock::new(|| ChillTouch {});

/// Spiritual Weapon — level-2 evocation. Bonus action; the caster makes
/// a melee spell attack (using WIS modifier + proficiency, no STR) against
/// a target within reach (we collapse the floating-weapon range to
/// melee reach since we don't yet model summoned terrain). On hit:
/// 1d8 + WIS mod force damage. Reach 1 tile (5 ft). No concentration.
pub struct SpiritualWeapon {}

impl Action for SpiritualWeapon {
    fn name(&self) -> &str {
        "spiritual weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sw-spell", "spirit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // We model the floating weapon as caster-melee for now.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction, Resource::SpellSlot(2)]
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "spiritual weapon",
            attack_bonus,
            Dice::new(1, 8),
            wis_mod,
            DamageType::Force,
            true,
        )
    }
}

pub static SPIRITUAL_WEAPON: LazyLock<SpiritualWeapon> = LazyLock::new(|| SpiritualWeapon {});

/// Hunter's Mark — level-1 divination, concentration. Mark a target;
/// while marked, the caster's weapon attacks against them deal an
/// extra 1d6 of weapon damage (handled by `resolve_attack`). Bonus
/// action to cast.
pub struct HuntersMark {}

impl Action for HuntersMark {
    fn name(&self) -> &str {
        "hunters mark"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm", "mark"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
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
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
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
                condition: Condition::HuntersMarked,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hunter's Mark",
                    vec![(target_id, Condition::HuntersMarked)],
                ),
            }),
        ]
    }
}

pub static HUNTERS_MARK: LazyLock<HuntersMark> = LazyLock::new(|| HuntersMark {});

/// Poison Spray — wizard / druid / sorcerer / warlock cantrip. The caster
/// extends a hand toward an adjacent creature; the target must make a
/// CON save against the caster's spell save DC or take 1d12 poison. No
/// damage on a successful save (cantrip saves are binary). Touch range
/// (1 tile) per 5e RAW.
pub struct PoisonSpray {}

impl Action for PoisonSpray {
    fn name(&self) -> &str {
        "poison spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "poison"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if save.passed() {
            return Vec::new();
        }
        let raw = encounter.roll(&Dice::new(1, 12));
        encounter.log(format!("  poison spray: 1d12({}) = {} poison", raw, raw));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Poison,
        })]
    }
}

pub static POISON_SPRAY: LazyLock<PoisonSpray> = LazyLock::new(|| PoisonSpray {});

/// Inflict Wounds — level-1 necromancy. Melee spell attack; on hit, 3d10
/// necrotic damage. The dark mirror of Cure Wounds — used by warlocks,
/// evil clerics, and necromancers. Crit doubles the dice per RAW. Uses
/// the caster's WIS spell-attack mod by default (cleric flavor); we pick
/// WIS because Inflict Wounds is the cleric domain spell.
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
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "inflict wounds",
            attack_bonus,
            Dice::new(3, 10),
            DamageType::Necrotic,
            true,
        )
    }
}

pub static INFLICT_WOUNDS: LazyLock<InflictWounds> = LazyLock::new(|| InflictWounds {});

/// Ray of Sickness — level-1 necromancy. Ranged spell attack vs target's
/// AC; on hit, 2d8 poison. Then the target rolls a CON save vs the
/// caster's spell save DC; on fail, gains the Poisoned condition until
/// the end of the caster's next turn (we use a 1-round timer for
/// approximation). No effect on a passed save (other than the damage).
pub struct RayOfSickness {}

impl Action for RayOfSickness {
    fn name(&self) -> &str {
        "ray of sickness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ros", "sickness"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "ray of sickness",
            attack_bonus,
            Dice::new(2, 8),
            DamageType::Poison,
            false,
        );
        // Only run the rider save when the ray landed — `spell_attack`
        // returns an empty Vec on a miss, so checking emptiness keeps the
        // RAW gate ("on a hit, also...") honest.
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static RAY_OF_SICKNESS: LazyLock<RayOfSickness> = LazyLock::new(|| RayOfSickness {});

/// Lesser Restoration — level-2 abjuration. Touch range; remove one of
/// Poisoned / Blinded / Deafened / Paralyzed from an ally. We don't
/// expose the choice to the caller — the engine pops the first present
/// condition in that priority order (poison-first matches the spell's
/// most common 5e use case).
pub struct LesserRestoration {}

impl Action for LesserRestoration {
    fn name(&self) -> &str {
        "lesser restoration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lr", "restore"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW: the spell ends "one disease or condition" — there has to
        // be something to end. Without this gate, the AI's heal-search
        // would happily fire Lesser Restoration on a clean ally and
        // burn a level-2 slot for nothing.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        Self::CANDIDATES.iter().any(|c| target.has_condition(*c))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::RemoveOneOfConditions {
            actor_id: target_id,
            candidates: Self::CANDIDATES.to_vec(),
        })]
    }
}

impl LesserRestoration {
    /// Conditions Lesser Restoration is allowed to lift, in priority
    /// order (the cleanse pops the first match).
    const CANDIDATES: [Condition; 4] = [
        Condition::Poisoned,
        Condition::Blinded,
        Condition::Deafened,
        Condition::Paralyzed,
    ];
}

pub static LESSER_RESTORATION: LazyLock<LesserRestoration> =
    LazyLock::new(|| LesserRestoration {});

/// Thorn Whip — druid cantrip. Ranged spell attack at 30 ft (12 tiles);
/// on hit, 1d6 piercing and the target is pulled up to 10 ft (2 tiles)
/// toward the caster (RAW: Large or smaller; we apply the cap as a size
/// gate). Crit doubles the dice; the pull is unaffected by crit.
pub struct ThornWhip {}

impl Action for ThornWhip {
    fn name(&self) -> &str {
        "thorn whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tw-spell", "thorn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        use crate::engine::side_effects::PullActor;
        use crate::engine::types::Size;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "thorn whip",
            attack_bonus,
            Dice::new(1, 6),
            DamageType::Piercing,
            false,
        );
        // Pull only fires on a hit; on a miss `spell_attack` returns
        // an empty effect list so we'd skip silently anyway.
        if effects.is_empty() {
            return effects;
        }
        // 5e: Large or smaller — Huge / Gargantuan targets ignore the pull.
        let too_big = encounter
            .actors
            .get(&target_id)
            .map(|t| matches!(t.size(), Size::Huge | Size::Gargantuan))
            .unwrap_or(true);
        if !too_big {
            effects.push(Box::new(PullActor {
                actor_id: target_id,
                toward: caster_loc,
                max_tiles: 2,
            }));
        }
        effects
    }
}

pub static THORN_WHIP: LazyLock<ThornWhip> = LazyLock::new(|| ThornWhip {});

/// Spare the Dying — cleric cantrip. Stabilize a dying ally at touch
/// range. No spell slot, no save, no damage. Only valid if the target
/// has 0 HP and is rolling death saves (Dying). Stabilization stops
/// the death-save cycle without restoring HP — the target sits at 0
/// HP / Stable / Unconscious until healed.
pub struct SpareTheDying {}

impl Action for SpareTheDying {
    fn name(&self) -> &str {
        "spare the dying"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["std", "spare"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    // Stabilize is a heal in spirit — it pulls the target off the death-
    // save treadmill. The AI's "find someone to help" pipeline keys off
    // is_heal so this gets considered the same way Cure Wounds does.
    fn is_heal(&self) -> bool {
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
        vec![Resource::Action]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::StabilizeActor {
            actor_id: target_id,
        })]
    }
}

pub static SPARE_THE_DYING: LazyLock<SpareTheDying> = LazyLock::new(|| SpareTheDying {});

/// Toll the Dead — cleric / warlock cantrip. Range 60ft (24 tiles).
/// Target makes a WIS save vs caster's spell save DC; on fail, takes
/// 1d8 necrotic, or 1d12 if the target is already wounded (HP below max).
/// On success, no damage. Cantrip damage doesn't scale here — at higher
/// levels the dice would step, but we keep base.
pub struct TollTheDead {}

impl Action for TollTheDead {
    fn name(&self) -> &str {
        "toll the dead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ttd", "toll"]
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
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        // d12 if target is below max HP, otherwise d8. Checked after the
        // save so the breakdown lands in the log between save and damage.
        let wounded = encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.hitpoints() < a.max_hitpoints());
        let die = if wounded { Dice::new(1, 12) } else { Dice::new(1, 8) };
        let raw = encounter.roll(&die);
        encounter.log(format!(
            "  toll the dead: {}({}) = {} necrotic{}",
            die,
            raw,
            raw,
            if wounded { " (wounded)" } else { "" }
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static TOLL_THE_DEAD: LazyLock<TollTheDead> = LazyLock::new(|| TollTheDead {});

/// Vicious Mockery — bard cantrip. Range 60ft (24 tiles). Target WIS
/// save vs caster's CHA-based DC. On fail: 1d4 psychic AND disadvantage
/// on its next attack roll (we tag with Condition::Mocked, which the
/// engine reads in `compute_attack_mode`). On pass, nothing.
pub struct ViciousMockery {}

impl Action for ViciousMockery {
    fn name(&self) -> &str {
        "vicious mockery"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vm", "mock"]
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
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        let raw = encounter.roll(&Dice::new(1, 4));
        encounter.log(format!("  vicious mockery: 1d4({}) = {} psychic", raw, raw));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Psychic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Mocked,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
        ]
    }
}

pub static VICIOUS_MOCKERY: LazyLock<ViciousMockery> = LazyLock::new(|| ViciousMockery {});

/// Heroism — bard / paladin level-1, concentration. Target ally gains
/// temp HP equal to caster's spellcasting modifier (we use CHA) at the
/// start of each of their turns, and immunity to Frightened while the
/// spell is up. We model the "temp HP each turn" via an immediate
/// grant on cast and rely on concentration cleanup to drop the
/// Heroic condition; ticking the regrant each turn would require a
/// per-actor concentration tick we don't have today.
pub struct Heroism {}

impl Action for Heroism {
    fn name(&self) -> &str {
        "heroism"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hr", "hero"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let amt = modifier_from_score(caster.ability_score(AbilityScoreType::Charisma)).max(1) as u32;
        vec![
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: amt,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Heroism",
                    vec![(target_id, Condition::Heroic)],
                ),
            }),
        ]
    }
}

pub static HEROISM: LazyLock<Heroism> = LazyLock::new(|| Heroism {});

/// Mass Healing Word — cleric level-3 bonus-action heal. Up to six
/// creatures within range, each within line-of-sight of the caster,
/// regain `1d4 + WIS` HP. We implement it with a per-actor radius
/// (60ft = 24 tile gap) and an LOS check; the AI's heal-search
/// pipeline can ignore it for now (it picks single-target).
pub struct MassHealingWord {}

impl Action for MassHealingWord {
    fn name(&self) -> &str {
        "mass healing word"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mhw", "mass-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
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
        vec![Resource::BonusAction, Resource::SpellSlot(3)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(1, 4)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  mass healing word: 1d4({}){:+} = {} HP each",
            raw, wis_mod, amount
        ));
        // RAW: pick up to 6 creatures. We snap to the closest 6 eligible
        // allies (combat-active OR dying — heals revive both).
        const RANGE_TILES: isize = 24;
        const MAX_TARGETS: usize = 6;
        let mut candidates: Vec<(isize, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    caster_loc,
                    caster_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                );
                if dist > RANGE_TILES {
                    return None;
                }
                Some((dist, *id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(MAX_TARGETS);
        candidates
            .into_iter()
            .map(|(_, id)| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static MASS_HEALING_WORD: LazyLock<MassHealingWord> = LazyLock::new(|| MassHealingWord {});

/// Shocking Grasp — wizard / sorcerer cantrip. Melee spell attack; on
/// hit, 1d8 lightning AND the target loses its reactions until the
/// start of its next turn (we install Condition::NoReaction with the
/// `UntilStartOfNextTurn` timer). Has advantage on the attack roll if
/// the target is wearing metal armor — we don't model armor types, so
/// we skip that rider.
pub struct ShockingGrasp {}

impl Action for ShockingGrasp {
    fn name(&self) -> &str {
        "shocking grasp"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sg", "shock"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "shocking grasp",
            attack_bonus,
            Dice::new(1, 8),
            DamageType::Lightning,
            true,
        );
        // Rider applies only on a hit (empty effect list = miss).
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::NoReaction,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects
    }
}

pub static SHOCKING_GRASP: LazyLock<ShockingGrasp> = LazyLock::new(|| ShockingGrasp {});

/// Shatter — level-2 evocation. 10-ft-radius burst centered on a point
/// within 60 ft (24 tiles). Every creature in the burst makes a CON save
/// vs the caster's spell save DC: pass = half, fail = full. 3d8 thunder
/// damage. We share-roll once and route through the burst-save helper —
/// identical pattern to Burning Hands but spherical instead of a cone.
pub struct Shatter {}

impl Action for Shatter {
    fn name(&self) -> &str {
        "shatter"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh-spell", "shat"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft radius = 2-tile burst on the 2.5ft grid.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
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
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(3, 8));
        encounter.log(format!("  shatter: 3d8({}) = {} thunder area", raw, raw));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Thunder,
        )
    }
}

pub static SHATTER: LazyLock<Shatter> = LazyLock::new(|| Shatter {});

/// Sleep — level-1 enchantment. Roll 5d8; the total is a "HP pool".
/// Sweep enemy creatures within range in ascending current-HP order and
/// put each to Asleep until their pool of current HP is fully consumed
/// (each target consumes `current_hp` from the pool). Undead and creatures
/// immune to the Charmed condition (most are) are unaffected. Asleep is
/// stripped by any damage.
pub struct Sleep {}

impl Action for Sleep {
    fn name(&self) -> &str {
        "sleep"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sleep-spell", "slumber"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft = 36 tiles.
        Some(36)
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
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        const BURST_RADIUS: isize = 8; // 20ft radius
        let pool_roll = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!("  sleep: 5d8({}) = {} HP pool", pool_roll, pool_roll));

        // Sort eligible targets by ascending current HP (5e RAW). Undead
        // and Charmed-immune creatures are skipped — they don't dream.
        // The Charmed gate doubles up: in our pool, Charm immunity is
        // the cleanest "no mind-affecting" proxy.
        let hit = crate::actions::action_template::pool_sweep_targets(
            encounter,
            caster_id,
            point,
            BURST_RADIUS,
            pool_roll,
            Condition::Charmed,
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in hit {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }));
            // 5e: Sleep also drops the target prone (unconscious clause).
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static SLEEP: LazyLock<Sleep> = LazyLock::new(|| Sleep {});

/// Charm Person — level-1 enchantment. Target makes a WIS save vs the
/// caster's spell save DC; on fail, the target is Charmed for an hour
/// (we use 10 rounds). The charmed creature can't attack their charmer
/// (enforced in `validate_input`). On a save, the spell fizzles.
pub struct CharmPerson {}

impl Action for CharmPerson {
    fn name(&self) -> &str {
        "charm person"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cp", "charm"]
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
    fn deals_damage(&self) -> bool {
        false
    }
    // Charm is harmful in 5e (it's a hostile mind-affecting spell), so we
    // leave is_harmful at the default true. The validate_input charm-vs-
    // charmer block still works because a freshly-charmed actor can't
    // retaliate against the original charmer.
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
        use crate::engine::side_effects::SetCharmedBy;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Charmed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(SetCharmedBy {
                target_id,
                charmer: Some(caster_id),
            }),
        ]
    }
}

pub static CHARM_PERSON: LazyLock<CharmPerson> = LazyLock::new(|| CharmPerson {});

/// Mirror Image — level-2 illusion. No save, no concentration, no
/// targeting. The caster gains three duplicates that absorb incoming
/// attacks: a hit may instead pop a decoy. Pool count is tracked on the
/// actor and read by `resolve_attack` (engine/attack.rs).
pub struct MirrorImage {}

impl Action for MirrorImage {
    fn name(&self) -> &str {
        "mirror image"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mi", "mirror"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
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
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::SetMirrorImages;
        vec![
            Box::new(SetMirrorImages {
                actor_id: caster_id,
                count: 3,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::MirroredImages,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static MIRROR_IMAGE: LazyLock<MirrorImage> = LazyLock::new(|| MirrorImage {});

/// Eldritch Blast — warlock cantrip. Ranged spell attack: d20 + CHA vs
/// AC. On hit: 1d10 force. We don't model the per-level beam scaling
/// (it adds one beam every few levels in 5e); we keep the single-beam
/// base, which is the right shape for a CR 1-3 warlock.
pub struct EldritchBlast {}

impl Action for EldritchBlast {
    fn name(&self) -> &str {
        "eldritch blast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "blast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Charisma);
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "eldritch blast",
            attack_bonus,
            Dice::new(1, 10),
            DamageType::Force,
            false,
        )
    }
}

pub static ELDRITCH_BLAST: LazyLock<EldritchBlast> = LazyLock::new(|| EldritchBlast {});

/// Protection from Evil and Good — level-1 abjuration, concentration.
/// Target gains the Warded condition: aberrations, celestials, elementals,
/// fey, fiends, and undead have disadvantage on attacks against them. We
/// approximate the creature-type gate via the target's necrotic/poison
/// immunity profile (a rough but reliable proxy for undead / fiend status
/// in our pool). The condition is read by `compute_attack_mode`.
pub struct ProtectionFromEvilAndGood {}

impl Action for ProtectionFromEvilAndGood {
    fn name(&self) -> &str {
        "protection from evil and good"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pfeg", "protection"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
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
                condition: Condition::Warded,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Protection from Evil and Good",
                    vec![(target_id, Condition::Warded)],
                ),
            }),
        ]
    }
}

pub static PROTECTION_FROM_EVIL_AND_GOOD: LazyLock<ProtectionFromEvilAndGood> =
    LazyLock::new(|| ProtectionFromEvilAndGood {});

/// Color Spray — level-1 illusion. Roll a 6d10 HP pool; sweep enemies in
/// a 15ft cone (we approximate with a 2-tile burst centered on the target
/// point) in ascending current-HP order, blinding each one until the end
/// of the caster's next turn until the pool is exhausted. Targets immune
/// to Blinded (constructs, oozes that don't have eyes) are skipped, and a
/// target with more current HP than the remaining pool stops the sweep.
/// Distinct from Sleep: shorter timer, blinds instead of asleep, and
/// undead are *not* exempt — only literal blind-immune creatures are.
pub struct ColorSpray {}

impl Action for ColorSpray {
    fn name(&self) -> &str {
        "color spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-spell", "color"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15ft cone — short range, treat the burst origin as the cone's
        // far edge much like Burning Hands.
        Some(6)
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
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        const BURST_RADIUS: isize = 2;
        let pool = encounter.roll(&Dice::new(6, 10));
        encounter.log(format!(
            "  color spray: 6d10({}) = {} HP pool",
            pool, pool
        ));
        crate::actions::action_template::pool_sweep_targets(
            encounter,
            caster_id,
            point,
            BURST_RADIUS,
            pool,
            Condition::Blinded,
        )
        .into_iter()
        .map(|id| {
            Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Blinded,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect()
    }
}

pub static COLOR_SPRAY: LazyLock<ColorSpray> = LazyLock::new(|| ColorSpray {});

/// Command — level-1 enchantment. The caster barks a one-word command at
/// a target within 60ft; on a failed WIS save the target spends their
/// next turn complying (we model the "Halt" / "Grovel" variants as a
/// simple Stunned for one turn). Save-immune creatures (charm-immune in
/// our pool: undead, constructs) are unaffected. No concentration; the
/// effect is short and self-clearing via the UntilStartOfNextTurn timer.
pub struct Command {}

impl Action for Command {
    fn name(&self) -> &str {
        "command"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cmd", "halt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
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
        // 5e Command: undead and creatures that don't understand the
        // caster's language are immune. Charm immunity is the closest
        // proxy for "won't be cowed" in our pool.
        if let Some(target) = encounter.actors.get(&target_id)
            && target.is_immune_to_condition(Condition::Charmed)
        {
            encounter.log("  command: target is immune".to_string());
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Stunned,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static COMMAND: LazyLock<Command> = LazyLock::new(|| Command {});

/// Fireball — iconic 5e level-3 evocation. 150ft range, 20ft-radius
/// sphere (radius 4 on the 2.5ft grid). DEX save against the caster's
/// INT-based DC: failed save takes 8d6 fire, success takes half. Single
/// shared damage roll for the whole burst (5e shared-roll AoE). Damage
/// scales by +1d6 per spell level above 3 — our Resource::SpellSlot
/// model only reports the base level, so the scaling lane is preserved
/// for the future leveled-cast UI but defaults to 8d6 for now.
pub struct Fireball {}

impl Action for Fireball {
    fn name(&self) -> &str {
        "fireball"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fb-spell", "fire-ball"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!("  fireball: 8d6({}) = {} fire area", raw, raw));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Fire,
        )
    }
}

pub static FIREBALL: LazyLock<Fireball> = LazyLock::new(|| Fireball {});

/// Magic Weapon — level-2 transmutation, concentration up to 1 hour.
/// Touch a single weapon-wielding ally; their attack rolls and damage
/// gain a flat +1 bonus for the duration. We track the buff as an
/// `attack_bonus_buff` delta installed via concentration so dropping
/// concentration cleans up automatically. The damage-half of the buff is
/// lumped into the same +1 attack buff for now (the engine doesn't have
/// a separate damage-roll buff lane). Targeting is touch (1 tile reach).
pub struct MagicWeapon {}

impl Action for MagicWeapon {
    fn name(&self) -> &str {
        "magic weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mw", "magic-weapon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::BonusAction, Resource::SpellSlot(2)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustAttackBuff;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Install a +1 attack buff and register it on the concentration
        // so dropping the spell rolls back the bonus on the right actor.
        let mut data = ConcentrationData::with_conditions("Magic Weapon", Vec::new());
        data.attack_buffs.push((target_id, 1));
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: target_id,
                delta: 1,
            }),
            Box::new(StartConcentration {
                caster_id,
                data,
            }),
        ]
    }
}

pub static MAGIC_WEAPON: LazyLock<MagicWeapon> = LazyLock::new(|| MagicWeapon {});

/// Scorching Ray — level-2 evocation. Three independent ranged spell
/// attack rolls against the same target (or, in 5e RAW, different
/// targets — we don't yet model multi-target selection so they all
/// converge on the chosen actor). Each ray deals 2d6 fire on hit. No
/// save: standard spell attack vs AC per ray. The triple-attack lane
/// rewards a high spell attack mod and gives wizards a reliable
/// concentration-free single-target nuke.
pub struct ScorchingRay {}

impl Action for ScorchingRay {
    fn name(&self) -> &str {
        "scorching ray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "scorch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let mut all: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Three independent rays. Each is its own attack roll → its own
        // hit/miss/crit. If the target falls between rays the later rays
        // still queue DealDamage, which no-ops on a dead actor.
        for i in 0..3 {
            let label = format!("scorching ray (ray {})", i + 1);
            let effs = spell_attack(
                encounter,
                caster_id,
                target_id,
                &label,
                attack_bonus,
                Dice::new(2, 6),
                DamageType::Fire,
                false,
            );
            all.extend(effs);
        }
        all
    }
}

pub static SCORCHING_RAY: LazyLock<ScorchingRay> = LazyLock::new(|| ScorchingRay {});

/// Lightning Bolt — level-3 evocation. A 100ft line / 5ft wide (RAW); we
/// approximate as a burst at the target point: every creature in a
/// 4-tile radius makes a DEX save vs caster's spell save DC for 8d6
/// lightning. Pass = half, fail = full. Shared damage roll across all
/// targets. Distinct from Fireball: lightning damage type and same
/// resource cost — picking between the two is a function of enemy
/// resistances and party positioning (lightning more linear-flavored
/// even if our grid approximation is a sphere).
pub struct LightningBolt {}

impl Action for LightningBolt {
    fn name(&self) -> &str {
        "lightning bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lb", "lightning"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100ft = 40 tiles.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!(
            "  lightning bolt: 8d6({}) = {} lightning area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Lightning,
        )
    }
}

pub static LIGHTNING_BOLT: LazyLock<LightningBolt> = LazyLock::new(|| LightningBolt {});

/// Vampiric Touch — level-3 necromancy, concentration. Melee spell
/// attack; on hit, target takes 3d6 necrotic and the caster heals half
/// (rounded down). Concentration is installed so re-cast within an hour
/// drops the prior buff cleanly. Distinct from Inflict Wounds (1-action
/// burst nuke, no slot scaling); Vampiric Touch trades single-hit damage
/// for sustained self-sustain on a beefy caster.
pub struct VampiricTouch {}

impl Action for VampiricTouch {
    fn name(&self) -> &str {
        "vampiric touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vt", "vamp"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        // spell_attack rolls the d20 vs AC and (on hit) the damage dice,
        // returning a DealDamage we'll merge with the self-heal rider.
        // `dealt` is the post-crit damage queued onto the target; we use
        // it to drive the half-as-heal rider without re-rolling.
        let (mut effs, dealt) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "vampiric touch",
            attack_bonus,
            Dice::new(3, 6),
            0,
            DamageType::Necrotic,
            true,
        );
        // Install concentration regardless of hit/miss — RAW: the spell
        // is active for its full duration once cast, even if the first
        // strike misses. Drop on re-cast keeps memory tight.
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Vampiric Touch", Vec::new()),
        }));
        if dealt > 0 {
            let heal = (dealt / 2).max(1);
            encounter.log(format!(
                "  vampiric touch: caster regains {} HP",
                heal
            ));
            effs.push(Box::new(Heal {
                actor_id: caster_id,
                amount: heal,
            }));
        }
        effs
    }
}

pub static VAMPIRIC_TOUCH: LazyLock<VampiricTouch> = LazyLock::new(|| VampiricTouch {});

/// Hypnotic Pattern — level-3 illusion. Burst (we use 4-tile radius for
/// a 30ft cube) of incapacitating fascination. Every creature in the
/// burst makes a WIS save vs caster's spell save DC. On fail, the
/// target is Incapacitated for several rounds (we use Rounds(5)) and
/// concentration tracks the spell so dropping it clears the condition.
/// Charm-immune creatures (undead, constructs, etc.) save automatically.
/// No damage — pure crowd control.
pub struct HypnoticPattern {}

impl Action for HypnoticPattern {
    fn name(&self) -> &str {
        "hypnotic pattern"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hyp", "hypnotic"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft cube ≈ 4-tile radius burst on the 2.5ft grid.
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
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
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let mut conditions_tracked: Vec<(usize, Condition)> = Vec::new();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in encounter.burst_targets(caster_id, point, 4) {
            // Charm-immune creatures shrug off the pattern. We don't
            // log per-target immunity for AoE — would be noisy.
            if encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.is_immune_to_condition(Condition::Charmed))
            {
                continue;
            }
            let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(5),
            }));
            conditions_tracked.push((target_id, Condition::Incapacitated));
        }
        if !conditions_tracked.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hypnotic Pattern",
                    conditions_tracked,
                ),
            }));
        }
        effects
    }
}

pub static HYPNOTIC_PATTERN: LazyLock<HypnoticPattern> = LazyLock::new(|| HypnoticPattern {});

/// Divine Favor — level-1 evocation, concentration. Self-buff: weapon
/// attacks deal +1d4 radiant for the duration. We approximate the +1d4
/// damage rider as a flat +2 attack-buff (the engine doesn't have a
/// per-attack-extra-damage lane for self-buffs yet). Installed via
/// concentration so re-casting another concentration drops it cleanly.
/// Distinct from Bless (allies-only, +1d4 to attack rolls / saves):
/// Divine Favor is self-only and stacks freely with Bless.
pub struct DivineFavor {}

impl Action for DivineFavor {
    fn name(&self) -> &str {
        "divine favor"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["df", "favor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustAttackBuff;
        // +2 attack buff approximates "+1d4 radiant per hit". The buff
        // lives on the concentration so it rolls back automatically.
        let mut data = ConcentrationData::with_conditions("Divine Favor", Vec::new());
        data.attack_buffs.push((caster_id, 2));
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: caster_id,
                delta: 2,
            }),
            Box::new(StartConcentration {
                caster_id,
                data,
            }),
        ]
    }
}

pub static DIVINE_FAVOR: LazyLock<DivineFavor> = LazyLock::new(|| DivineFavor {});

/// Spirit Guardians — level-3 conjuration, concentration. Caster is
/// surrounded by a 15ft-radius aura of spectral guardians; each enemy
/// that starts its turn in the aura makes a WIS save vs the caster's
/// spell save DC. Pass = half, fail = full of 3d8 radiant. We model the
/// aura via a one-shot burst centered on the caster at cast time
/// (immediate damage on cast); the per-turn re-pulse requires
/// per-actor concentration tick we don't have today, so the spell's
/// flavor is collapsed to a powerful single-cast radiant burst that
/// matches a typical first-round application. Concentration tracks the
/// cast so re-casting drops cleanly.
pub struct SpiritGuardians {}

impl Action for SpiritGuardians {
    fn name(&self) -> &str {
        "spirit guardians"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sg", "spirit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        false
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
        vec![Resource::Action, Resource::SpellSlot(3)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let raw = encounter.roll(&Dice::new(3, 8));
        encounter.log(format!(
            "  spirit guardians: 3d8({}) = {} radiant area",
            raw, raw
        ));
        // 15ft radius = 3 tiles on the 2.5ft grid.
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            caster_loc,
            3,
            AbilityScoreType::Wisdom,
            dc,
            raw,
            DamageType::Radiant,
        );
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Spirit Guardians", Vec::new()),
        }));
        effs
    }
}

pub static SPIRIT_GUARDIANS: LazyLock<SpiritGuardians> = LazyLock::new(|| SpiritGuardians {});

/// Hex — level-1 enchantment, concentration. Bonus action to mark a target;
/// the caster's weapon attacks against the hexed target deal an extra 1d6
/// necrotic (handled by `resolve_attack` via `is_hex_target`). Distinct
/// from Hunter's Mark: same on-hit rider but necrotic-typed (so resistant
/// undead shrug it off) and tied to CHA-based warlock casting flavor.
pub struct Hex {}

impl Action for Hex {
    fn name(&self) -> &str {
        "hex"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hex-mark"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles (same as Hunter's Mark).
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
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
                condition: Condition::Hexed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hex",
                    vec![(target_id, Condition::Hexed)],
                ),
            }),
        ]
    }
}

pub static HEX: LazyLock<Hex> = LazyLock::new(|| Hex {});

/// Hold Monster — level-5 enchantment, concentration. Identical mechanic
/// to Hold Person (WIS save vs spell DC, on fail target is Stunned for up
/// to 10 rounds, concentration tracks the lock), but consumes a level-5
/// slot in exchange for working on creatures that would normally shrug
/// off the humanoid-only Hold Person. Charm-immune creatures (undead,
/// constructs) still resist via condition immunity.
pub struct HoldMonster {}

impl Action for HoldMonster {
    fn name(&self) -> &str {
        "hold monster"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm-spell", "holdm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
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
        vec![Resource::Action, Resource::SpellSlot(5)]
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
                condition: Condition::Stunned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hold Monster",
                    vec![(target_id, Condition::Stunned)],
                ),
            }),
        ]
    }
}

pub static HOLD_MONSTER: LazyLock<HoldMonster> = LazyLock::new(|| HoldMonster {});

/// Invisibility — level-2 illusion, concentration. Target becomes Invisible
/// until concentration ends or the target makes an attack / casts a spell.
/// The "drop on attack" rider is enforced by `resolve_attack`: when the
/// caster (or whoever they targeted) attacks while concentrating on
/// Invisibility, the spell's concentration ends and the Invisible condition
/// clears with it.
pub struct Invisibility {}

impl Action for Invisibility {
    fn name(&self) -> &str {
        "invisibility"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["invis"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 5 ft = 1 tile.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
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
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Invisibility",
                    vec![(target_id, Condition::Invisible)],
                ),
            }),
        ]
    }
}

pub static INVISIBILITY: LazyLock<Invisibility> = LazyLock::new(|| Invisibility {});

/// Bestow Curse — level-3 necromancy, concentration. WIS save vs the
/// caster's spell save DC; on fail, the target has disadvantage on
/// attack rolls and saving throws (we model via the Baned condition,
/// which already implements the symmetric -2 to both lanes — close
/// enough to RAW's "disadvantage on saves vs this caster's spells"
/// without spinning a per-source debuff lane). Concentration tracks
/// the curse so dropping it lifts the debuff cleanly.
pub struct BestowCurse {}

impl Action for BestowCurse {
    fn name(&self) -> &str {
        "bestow curse"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bc", "curse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 5 ft = 1 tile.
        Some(crate::actions::action_template::MELEE_REACH)
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
        vec![Resource::Action, Resource::SpellSlot(3)]
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
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Bestow Curse",
                    vec![(target_id, Condition::Baned)],
                ),
            }),
        ]
    }
}

pub static BESTOW_CURSE: LazyLock<BestowCurse> = LazyLock::new(|| BestowCurse {});

/// Mind Sliver — enchantment cantrip. INT save vs caster's spell save DC;
/// on fail, target takes 1d6 psychic AND has a -1d4 penalty (modeled via
/// the Baned condition, which is -2 to saves / attacks; close enough for
/// the single-round window). The save-debuff rider lasts one round per
/// RAW. No damage on a save (cantrip binary).
pub struct MindSliver {}

impl Action for MindSliver {
    fn name(&self) -> &str {
        "mind sliver"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "sliver"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, dc);
        if save.passed() {
            return Vec::new();
        }
        let dmg = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!("  mind sliver: 1d6({}) = {} psychic", dmg, dmg));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(1),
            }),
        ]
    }
}

pub static MIND_SLIVER: LazyLock<MindSliver> = LazyLock::new(|| MindSliver {});

/// Blur — level-2 illusion, self-buff, concentration. Attacks against the
/// caster have disadvantage while the spell is up (handled by
/// `compute_attack_mode` via the Blurred condition). Drops on the usual
/// concentration triggers; cleanup clears the Blurred flag.
pub struct Blur {}

impl Action for Blur {
    fn name(&self) -> &str {
        "blur"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["blur-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
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
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Blurred,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Blur",
                    vec![(caster_id, Condition::Blurred)],
                ),
            }),
        ]
    }
}

pub static BLUR: LazyLock<Blur> = LazyLock::new(|| Blur {});

/// Haste — level-3 transmutation, concentration. Target one willing
/// ally (or self with no args): they gain +2 AC, advantage on DEX saves,
/// and double walking speed for up to 10 rounds. We don't model the
/// extra-Action rider (would require a second Action slot the engine
/// doesn't currently track); the AC + DEX save + speed half is the
/// load-bearing part for repositioning support casters.
///
/// Targeting is `SingleActor` (the AI typically buffs the lead melee
/// attacker). Cleared cleanly when concentration drops.
pub struct Haste {}

impl Action for Haste {
    fn name(&self) -> &str {
        "haste"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ha"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tile gap.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::Action, Resource::SpellSlot(3)]
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
                condition: Condition::Hasted,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Haste",
                    vec![(target_id, Condition::Hasted)],
                ),
            }),
        ]
    }
}

pub static HASTE: LazyLock<Haste> = LazyLock::new(|| Haste {});

/// Slow — level-3 transmutation, concentration. Target a 40-ft cube;
/// every enemy within takes a WIS save vs the caster's spell DC. On
/// fail: Slowed for up to 10 rounds (−2 AC, halved speed, disadvantage
/// on DEX saves; we skip the action-economy half of the 5e effect to
/// keep AI behavior predictable). Hostile-only — allies in the cube
/// are spared by the caster-team filter.
///
/// Like Faerie Fire, this picks the burst origin via SinglePoint and
/// iterates the radius itself so the friendly-fire filter can run.
pub struct Slow {}

impl Action for Slow {
    fn name(&self) -> &str {
        "slow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
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
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let caster_team = caster.team();
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        const RADIUS: isize = 4;
        const MAX_TARGETS: usize = 6;

        // Enumerate enemy actors in the burst, sort by id for determinism,
        // cap at 6 targets per RAW.
        let mut victims: Vec<usize> = encounter
            .sorted_actor_ids()
            .into_iter()
            .filter(|id| {
                let Some(t) = encounter.actors.get(id) else {
                    return false;
                };
                if !t.is_combat_active() || t.team() == caster_team {
                    return false;
                }
                let dist = footprint_chebyshev(
                    t.location(),
                    get_tiles_from_size(t.size()),
                    point,
                    1,
                );
                dist <= RADIUS
            })
            .collect();
        victims.truncate(MAX_TARGETS);

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in victims {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Slowed,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Slowed));
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Slow", applied),
            }));
        }
        effects
    }
}

pub static SLOW: LazyLock<Slow> = LazyLock::new(|| Slow {});

/// Cone of Cold — level-5 evocation. A 60-ft cone of frigid air from
/// the caster: 8d8 cold damage, CON save for half. We approximate the
/// cone with a burst of radius 6 centered on the target tile (RAW is a
/// 60-ft cone — the engine doesn't yet model directional cones, so a
/// generous radius approximates the area). Damage is rolled once and
/// shared via `resolve_burst_save_damage`.
pub struct ConeOfCold {}

impl Action for ConeOfCold {
    fn name(&self) -> &str {
        "cone of cold"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["coc", "cone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self-cone, but we cap range at the cone reach (60 ft = 24 tiles)
        // so the picker doesn't drop pins on the far side of the map.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(5)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(8, 8));
        encounter.log(format!(
            "  cone of cold: 8d8({}) = {} cold area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Cold,
        )
    }
}

pub static CONE_OF_COLD: LazyLock<ConeOfCold> = LazyLock::new(|| ConeOfCold {});

/// Mass Cure Wounds — cleric level-5 action heal. Up to six creatures
/// in a 30-ft (12-tile-gap) sphere around a target point regain
/// `3d8 + WIS` HP. Unlike Mass Healing Word (bonus action, level-3,
/// 1d4 die), this is the cleric's emergency-button heal: bigger dice,
/// bigger slot, full Action.
pub struct MassCureWounds {}

impl Action for MassCureWounds {
    fn name(&self) -> &str {
        "mass cure wounds"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mcw", "mass-cure"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Pick a tile; we sweep that point's burst for allies.
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
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
        vec![Resource::Action, Resource::SpellSlot(5)]
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
        let caster_team = caster.team();
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(3, 8)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  mass cure wounds: 3d8({}){:+} = {} HP each",
            raw, wis_mod, amount
        ));
        const RADIUS: isize = 3;
        const MAX_TARGETS: usize = 6;
        // Pick allies inside the burst, sorted by current HP ascending so
        // the lowest-HP allies get healed first if we exceed the cap.
        let mut candidates: Vec<(u32, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist > RADIUS {
                    return None;
                }
                Some((a.hitpoints(), *id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(MAX_TARGETS);
        candidates
            .into_iter()
            .map(|(_, id)| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static MASS_CURE_WOUNDS: LazyLock<MassCureWounds> = LazyLock::new(|| MassCureWounds {});

/// Stinking Cloud — level-3 conjuration, concentration. A 20-ft sphere
/// of yellow vapor at a point; every creature inside makes a CON save
/// vs the caster's spell DC or becomes Incapacitated until the start
/// of their next turn (RAW: lose your action and your bonus action).
/// Re-rolls happen each round as the cloud lingers; we model that as
/// an `UntilStartOfNextTurn` timer, which gives the failed save a
/// one-round impact and lets the spell hit again next round if the
/// caster sustains concentration.
///
/// Unlike Fireball / Cone of Cold this is non-damaging — it doesn't
/// trigger concentration saves, it just shuts down enemy turns.
pub struct StinkingCloud {}

impl Action for StinkingCloud {
    fn name(&self) -> &str {
        "stinking cloud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sc-spell", "stink"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
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
        vec![Resource::Action, Resource::SpellSlot(3)]
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
        let caster_team = caster.team();
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        const RADIUS: isize = 2;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if !t.is_combat_active() || t.team() == caster_team {
                continue;
            }
            let dist = footprint_chebyshev(
                t.location(),
                get_tiles_from_size(t.size()),
                point,
                1,
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Constitution, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
            applied.push((tid, Condition::Incapacitated));
        }
        // Even with no failed saves, we still start concentration so the
        // cloud lingers — but only when at least one enemy is inside the
        // burst (otherwise the cast was wasted). Without applied targets
        // there's nothing to clear on concentration drop, but we'd want
        // re-rolls each round if we modeled cloud persistence. Today the
        // engine doesn't tick area effects across rounds; install
        // concentration only when at least one target was caught so
        // dropping is a clean no-op when the wind blows over.
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Stinking Cloud", applied),
            }));
        }
        effects
    }
}

pub static STINKING_CLOUD: LazyLock<StinkingCloud> = LazyLock::new(|| StinkingCloud {});

/// True Strike — divination cantrip. Targets a creature within 30 ft;
/// the caster gains advantage on their next attack roll against that
/// target before the end of their next turn. We model the rider by
/// applying the existing `Helped` condition to the caster — that's the
/// same one-shot advantage hook the Help action installs (consumed on
/// next attack, cleared after one swing). Single-target only.
pub struct TrueStrike {}

impl Action for TrueStrike {
    fn name(&self) -> &str {
        "true strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "true"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
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
        // Cantrip — Action only.
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // The Helped condition gives one-shot advantage on the next
        // attack made by the holder. RAW True Strike confers advantage
        // only against the targeted creature; we approximate by giving
        // a generic advantage flag since the engine's Helped condition
        // is consumed on the first attack regardless of target.
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Helped,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static TRUE_STRIKE: LazyLock<TrueStrike> = LazyLock::new(|| TrueStrike {});

/// Dispel Magic — level-3 abjuration, action. Touch range in 5e RAW is
/// 120 ft; we use 24 tiles. Drops the target's concentration outright;
/// if the target wasn't concentrating, strips one beneficial buff
/// instead (the engine picks deterministically — see `DispelMagicOn`).
/// No save: the spell auto-succeeds against effects from a slot of level
/// ≤ the cast level (which is the only level we track today). No damage.
pub struct DispelMagic {}

impl Action for DispelMagic {
    fn name(&self) -> &str {
        "dispel magic"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dm", "dispel"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // Dispel against a friendly buffed creature is technically harmful
        // (it removes their buff), but the AI's harmful-action picker uses
        // this as "should I target an enemy?" — Dispel is enemy-facing
        // when targeting concentrators, so true matches the intended use.
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
        vec![Resource::Action, Resource::SpellSlot(3)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::DispelMagicOn;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(DispelMagicOn { target_id })]
    }
}

pub static DISPEL_MAGIC: LazyLock<DispelMagic> = LazyLock::new(|| DispelMagic {});

/// Greater Invisibility — level-4 illusion, concentration. Functionally
/// the same as Invisibility (target gains the Invisible condition and
/// attacks against them have disadvantage, while their attacks have
/// advantage), but **does NOT drop on attack**. The plain Invisibility
/// spell's break-on-attack rider lives in
/// `EncounterInstance::clear_attack_advantage_riders`, which compares
/// the active concentration name against `"Invisibility"`; Greater
/// Invisibility uses a distinct concentration name so that hook leaves
/// it alone — the target stays invisible until concentration drops.
pub struct GreaterInvisibility {}

impl Action for GreaterInvisibility {
    fn name(&self) -> &str {
        "greater invisibility"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ginv", "greater-invis"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 1 tile.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::Action, Resource::SpellSlot(4)]
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
        // 10-round concentration — same as the rest of our buff-spells.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Greater Invisibility",
                    vec![(target_id, Condition::Invisible)],
                ),
            }),
        ]
    }
}

pub static GREATER_INVISIBILITY: LazyLock<GreaterInvisibility> =
    LazyLock::new(|| GreaterInvisibility {});

/// Ice Storm — level-4 evocation. 20-ft radius cylinder (we use a tile
/// burst, radius 4). Every creature in the area makes a DEX save vs the
/// caster's INT-based DC: 2d8 bludgeoning + 4d6 cold on a fail, half on a
/// success. Two damage types means resistance / immunity has to apply
/// twice to halve / null the full hit; the dual lane is what makes the
/// spell distinct from Fireball / Lightning Bolt at the same level slot.
pub struct IceStorm {}

impl Action for IceStorm {
    fn name(&self) -> &str {
        "ice storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["is", "icestorm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW; we cap to a map-realistic 48 tiles (120 ft).
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(4)]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let bludg = encounter.roll(&Dice::new(2, 8));
        let cold = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!(
            "  ice storm: 2d8({}) bludgeoning + 4d6({}) cold area",
            bludg, cold
        ));
        // Two passes through resolve_burst_save_damage so each damage
        // type interacts with target resistance / immunity independently.
        // The save is rolled once per pass — we accept the small RNG
        // cost (two save rolls per target) in exchange for keeping the
        // helper signature simple.
        let mut all = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            bludg,
            DamageType::Bludgeoning,
        );
        all.extend(crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            cold,
            DamageType::Cold,
        ));
        all
    }
}

pub static ICE_STORM: LazyLock<IceStorm> = LazyLock::new(|| IceStorm {});

/// Death Ward — level-4 abjuration, action, touch. Applies the
/// `DeathWarded` condition to one ally. The first time the holder
/// would drop to 0 HP, they instead drop to 1 HP and the ward clears
/// (engine hook lives in `ActorInstance::take_damage`). No
/// concentration — it's a fire-and-forget hard save.
pub struct DeathWard {}

impl Action for DeathWard {
    fn name(&self) -> &str {
        "death ward"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dw", "ward"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
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
        vec![Resource::Action, Resource::SpellSlot(4)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 5e RAW: 8 hour duration. We cap at a long Rounds timer so the
        // condition has a definite expiry even if combat drags on.
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::DeathWarded,
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static DEATH_WARD: LazyLock<DeathWard> = LazyLock::new(|| DeathWard {});

/// Revivify — level-3 necromancy, action, touch. Brings a Dying actor
/// (rolling death saves at 0 HP) back at 1 HP. Stabilized actors are
/// already alive at 0 HP and don't need this; vanilla Heal can pick
/// them back up. Dead actors are gone from the table by the time the
/// spell could resolve, so we don't try to chase them.
pub struct Revivify {}

impl Action for Revivify {
    fn name(&self) -> &str {
        "revivify"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rev", "revive"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Only valid against a Dying ally — the spell shouldn't be
        // wasted on healthy targets or actors whose status doesn't need
        // a revive (Stable / Active actors heal via normal spells).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(3)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ReviveDying;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(ReviveDying {
            actor_id: target_id,
        })]
    }
}

pub static REVIVIFY: LazyLock<Revivify> = LazyLock::new(|| Revivify {});
