use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        action_and_slot, bonus_action_and_slot, first_target_id, Action, TargetingSchema,
    },
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
    // 5e Cover: intervening creatures bump the target's effective AC,
    // same as for weapon swings. Spell attacks (Fire Bolt, Guiding Bolt,
    // Scorching Ray, etc.) honor the rule identically.
    let cover_bonus = encounter.cover_ac_bonus(caster_id, target_id);
    let target_ac = target_ac + cover_bonus;
    // 5e Sanctuary: gate spell attacks the same way weapon attacks are
    // gated — attacker rolls a WIS save vs the ward's DC. On fail, the
    // spell silently fizzles against the warded target.
    if encounter.sanctuary_save_blocks(caster_id, target_id) {
        return (Vec::new(), 0);
    }
    encounter.break_sanctuary_on_hostile(caster_id);
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
    // attack_bonus_buff, condition_attack_bonus) that weapon attacks
    // get via `resolve_attack`. This keeps spell-attack rolls
    // consistent with weapon swings — Sacred Weapon's +CHA fires on
    // spell attacks too (e.g. a Sacred-Weapon paladin casting Guiding
    // Bolt as a multiclass with cleric / divine soul). Shared with
    // weapon attacks via `EncounterInstance::caster_attack_buffs`.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(caster_id);
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(caster_id);
    let total = raw + attack_bonus + buff + cond_attack_bonus + bless_die;
    let is_crit = raw == 20;
    let hit = is_crit || total >= target_ac;
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
        action_name,
        raw,
        attack_bonus + buff + cond_attack_bonus,
        bless_note,
        total,
        target_ac,
        cover_note,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return (Vec::new(), 0);
    }
    // 5e Mirror Image: spell attack rolls trigger the deflection too
    // (RAW: "any attack roll against you"). Shared with weapon swings
    // via the engine's `mirror_image_deflect` helper.
    if encounter.mirror_image_deflect(target_id, is_crit) {
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
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: total_dmg,
        damage_type,
    })];
    // 5e Hex rider on spell attacks. The Hex spell RAW says "you deal
    // extra 1d6 necrotic damage to the target whenever you hit it with
    // an attack" — both weapon and spell attacks trigger the rider.
    // Hunter's Mark RAW is weapon-only, so it's not duplicated here.
    if encounter.is_hex_target(caster_id, target_id) {
        let hex_total =
            crate::engine::attack::roll_rider(encounter, Dice::new(1, 6), is_crit);
        encounter.log(format!("  hex: +{} extra Necrotic", hex_total));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: hex_total,
            damage_type: DamageType::Necrotic,
        }));
    }
    (effects, total_dmg)
}

/// Resolve an enemy-only AoE burst where each victim makes a save for
/// half damage off a *shared* damage roll. The roll fires once and is
/// halved on per-target saves — matches 5e's standard AoE semantics
/// (Fireball / Cone of Cold / Aganazzar's Scorcher / Dawn / Fire Storm
/// / Tidal Wave / Mental Prison's burst-variant). Allies inside the
/// radius are spared via `enemy_burst_targets`.
///
/// Returns `(damage_effects, per_target_save_results)`. The save vector
/// is `(target_id, passed)` for every actor that took the save —
/// callers that want to attach a per-target rider on fail (Tidal Wave's
/// Prone, Mental Prison's Restrained, etc.) can iterate the list and
/// queue the follow-up condition without re-walking the burst.
///
/// Logging shape:
/// - One "  {name}: {dice}({roll}) shared {damage_type}" line at the
///   top, identical to the legacy hand-rolled bursts.
/// - Each target's save line is emitted by `roll_save` directly.
///
/// Centralizes the loop body that ~10 enemy-burst spells reimplement.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn enemy_burst_save_for_half(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    let raw = encounter.roll(&dice);
    encounter.log(format!(
        "  {}: {}({}) shared {:?}",
        action_name, dice, raw, damage_type
    ));
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    let mut saves: Vec<(usize, bool)> = Vec::new();
    for tid in encounter.enemy_burst_targets(caster_id, point, radius) {
        let save = encounter.roll_save(tid, save_ability, dc);
        let passed = save.passed();
        let dmg = if passed { raw / 2 } else { raw };
        saves.push((tid, passed));
        if dmg == 0 {
            continue;
        }
        effects.push(Box::new(DealDamage {
            actor_id: tid,
            amount: dmg,
            damage_type,
        }));
    }
    (effects, saves)
}

/// Roll a damage burst against a target's saving throw, halving on
/// success. Returns `(damage, save_passed)` so callers can branch on
/// the save (e.g. attach a rider only on fail). The roll + save log
/// line is emitted with `action_name`; callers don't need to re-log
/// the breakdown. Folds the recurring `let raw = roll(); let dmg = if
/// save.passed() { raw / 2 } else { raw };` shape used by ~6 single-
/// target save-or-half spells (Hellish Rebuke, Mind Whip, Hellfire
/// Orb, the Smite spells, etc.) into one chokepoint.
fn save_for_half_damage(
    encounter: &mut EncounterInstance,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (u32, bool) {
    let raw = encounter.roll(&dice);
    let save = encounter.roll_save(target_id, save_ability, dc);
    let dmg = if save.passed() { raw / 2 } else { raw };
    encounter.log(format!(
        "  {}: {}({}) = {} {:?} ({})",
        action_name,
        dice,
        raw,
        dmg,
        damage_type,
        if save.passed() { "save (half)" } else { "fail (full)" },
    ));
    (dmg, save.passed())
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

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

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

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

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
        action_and_slot(2)
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
        action_and_slot(1)
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

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

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
        action_and_slot(1)
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
                breaks_on_attack: false,
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
                    breaks_on_attack: false,
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
        action_and_slot(1)
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
                    breaks_on_attack: false,
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
        action_and_slot(1)
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
        action_and_slot(2)
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
                    breaks_on_attack: false,
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
        action_and_slot(1)
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
        action_and_slot(2)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        bonus_action_and_slot(2)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(2)
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
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
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
        bonus_action_and_slot(2)
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
        bonus_action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(2)
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
        bonus_action_and_slot(1)
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
        bonus_action_and_slot(3)
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
        action_and_slot(2)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(2)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(1)
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
        action_and_slot(3)
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
        bonus_action_and_slot(2)
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
        let mut data = ConcentrationData::new("Magic Weapon");
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
        action_and_slot(2)
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
        action_and_slot(3)
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
        action_and_slot(3)
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
            data: ConcentrationData::new("Vampiric Touch"),
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
        action_and_slot(3)
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
        bonus_action_and_slot(1)
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
        let mut data = ConcentrationData::new("Divine Favor");
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
        action_and_slot(3)
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
            data: ConcentrationData::new("Spirit Guardians"),
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
        bonus_action_and_slot(1)
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
        action_and_slot(5)
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
        action_and_slot(2)
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
                )
                .breaking_on_attack(),
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
        action_and_slot(3)
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
        action_and_slot(2)
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
        action_and_slot(3)
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
        action_and_slot(3)
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
        action_and_slot(5)
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
        action_and_slot(5)
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
        action_and_slot(3)
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
        const RADIUS: isize = 2;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
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
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
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
        action_and_slot(3)
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
        action_and_slot(4)
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
        action_and_slot(4)
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
        action_and_slot(4)
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
        action_and_slot(3)
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

/// Stoneskin — level-4 abjuration, concentration. Touch. Until the spell
/// ends, the target has resistance to bludgeoning, piercing, and slashing
/// damage. We use the existing `DamageResistant` condition which gives a
/// generic damage-halving effect — close enough to RAW's physical-only
/// resistance for our engine, and the buff drops cleanly when the caster
/// loses concentration. Doesn't stack with creature-template resistance
/// (halving is multiplicative, but we apply DamageResistant once at the
/// take-damage path so re-halving doesn't happen).
pub struct Stoneskin {}

impl Action for Stoneskin {
    fn name(&self) -> &str {
        "stoneskin"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "stone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 1 tile.
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
        action_and_slot(4)
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
                condition: Condition::DamageResistant,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Stoneskin",
                    vec![(target_id, Condition::DamageResistant)],
                ),
            }),
        ]
    }
}

pub static STONESKIN: LazyLock<Stoneskin> = LazyLock::new(|| Stoneskin {});

/// Beacon of Hope — level-3 abjuration, concentration. 30-ft radius cube;
/// up to 6 creatures of the caster's choice gain advantage on Wisdom
/// saves + death saves and regain the maximum from any healing for the
/// duration. We model the lasting buff with the existing `Heroic`
/// condition (already grants Frightened immunity; the additional save /
/// max-heal clauses are not yet engine-modeled but the buff icon is
/// useful flavor and stacks cleanly with concentration drop logic).
///
/// Targeting: AoE around a tile; affected = friendly combat-active actors
/// within radius 6 of the point (≈ 30 ft cube).
pub struct BeaconOfHope {}

impl Action for BeaconOfHope {
    fn name(&self) -> &str {
        "beacon of hope"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["boh", "beacon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self/30-ft origin. We pick a tile within ~30 ft to anchor the
        // cube — generous reach keeps the spell usable from the back row.
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
        action_and_slot(3)
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
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 6;

        let mut buffed = encounter.ally_burst_targets(caster_id, point, RADIUS);
        buffed.truncate(MAX_TARGETS);
        if buffed.is_empty() {
            return Vec::new();
        }

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in &buffed {
            effects.push(Box::new(ApplyCondition {
                actor_id: *tid,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((*tid, Condition::Heroic));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Beacon of Hope", applied),
        }));
        effects
    }
}

pub static BEACON_OF_HOPE: LazyLock<BeaconOfHope> = LazyLock::new(|| BeaconOfHope {});

/// Cloud of Daggers — level-2 conjuration, concentration. 5-ft cube of
/// whirling daggers; any creature that enters or starts its turn in the
/// area takes 4d4 slashing. We model the instantaneous on-cast hit as a
/// guaranteed 4d4 to every enemy currently inside the burst (radius 1 in
/// our tile-gap math); persistent ticks aren't yet modeled, but the
/// concentration is started so a follow-up cast or drop behaves cleanly.
/// No save — RAW autohits creatures in the area.
pub struct CloudOfDaggers {}

impl Action for CloudOfDaggers {
    fn name(&self) -> &str {
        "cloud of daggers"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cod", "daggers"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
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
        vec![DamageType::Slashing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
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
        const RADIUS: isize = 1;
        let damage = encounter.roll(&Dice::new(4, 4));
        encounter.log(format!(
            "  cloud of daggers: 4d4({}) = {} slashing",
            damage, damage
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: damage,
                damage_type: DamageType::Slashing,
            }));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Cloud of Daggers"),
        }));
        effects
    }
}

pub static CLOUD_OF_DAGGERS: LazyLock<CloudOfDaggers> = LazyLock::new(|| CloudOfDaggers {});

/// Witch Bolt — level-1 evocation, concentration. A spell-attack against a
/// single target deals 1d12 lightning on the initial hit. The 5e RAW
/// sustained-damage clause (a free 1d12 each subsequent turn) isn't yet
/// modeled — the engine's concentration ticking happens on round-end but
/// doesn't yet support author-defined per-round damage hooks. We still
/// install concentration so dropping it clears the spell cleanly and to
/// keep the caster from juggling two concentration spells.
pub struct WitchBolt {}

impl Action for WitchBolt {
    fn name(&self) -> &str {
        "witch bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb", "witch"]
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
        action_and_slot(1)
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
        // Wizards (INT) and clerics (WIS) both use this spell in the
        // template loadout. Auto-detect via the caster's higher spell-
        // attack modifier so it works in both pools.
        let int_mod = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let wis_mod = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let attack_bonus = int_mod.max(wis_mod);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "witch bolt",
            attack_bonus,
            Dice::new(1, 12),
            DamageType::Lightning,
            false,
        );
        // Install concentration regardless of hit — the spell can still be
        // sustained on a miss per RAW (the link forms either way).
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Witch Bolt"),
        }));
        effects
    }
}

pub static WITCH_BOLT: LazyLock<WitchBolt> = LazyLock::new(|| WitchBolt {});

/// Phantasmal Killer — level-4 illusion, concentration. Target makes a
/// WIS save. On fail: 4d10 psychic and Frightened. On save: nothing
/// (we skip the RAW repeat-save-each-round mechanic; the damage and
/// fright on the first failure carry the encounter weight). No damage
/// on success per RAW (the illusion never lands).
pub struct PhantasmalKiller {}

impl Action for PhantasmalKiller {
    fn name(&self) -> &str {
        "phantasmal killer"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pk", "phantasm"]
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
        action_and_slot(4)
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        let dmg = encounter.roll(&Dice::new(4, 10));
        encounter.log(format!(
            "  phantasmal killer: 4d10({}) = {} psychic",
            dmg, dmg
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Phantasmal Killer",
                    vec![(target_id, Condition::Frightened)],
                ),
            }),
        ]
    }
}

pub static PHANTASMAL_KILLER: LazyLock<PhantasmalKiller> =
    LazyLock::new(|| PhantasmalKiller {});

/// Banishment — level-4 abjuration, concentration. Target makes a CHA
/// save. On fail, banished to a harmless demiplane for the duration —
/// we model as Incapacitated (cannot take actions or reactions) for
/// the duration since the engine doesn't yet model off-board status.
/// Concentration tracks the lock so dropping it ends the banishment.
pub struct Banishment {}

impl Action for Banishment {
    fn name(&self) -> &str {
        "banishment"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["banish", "banishspell"]
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
        action_and_slot(4)
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
        // Use the caster's strongest spellcasting modifier (INT for
        // wizards, WIS for clerics) so the DC scales with whichever
        // school is firing it.
        let int_dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let wis_dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let dc = int_dc.max(wis_dc);
        let save = encounter.roll_save(target_id, AbilityScoreType::Charisma, dc);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  banishment: target is banished from the field");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Banishment",
                    vec![(target_id, Condition::Incapacitated)],
                ),
            }),
        ]
    }
}

pub static BANISHMENT: LazyLock<Banishment> = LazyLock::new(|| Banishment {});

/// Tasha's Hideous Laughter — level-1 enchantment, concentration. WIS
/// save vs DC. On fail, target falls Prone in fits of laughter and is
/// Incapacitated for the duration. Creatures with Intelligence ≤ 4 are
/// immune (we don't gate this — most enemies in our pool meet the
/// threshold, and the few low-INT ones are usually charm-immune anyway
/// via their template). Concentration tracks both conditions for clean
/// teardown.
pub struct TashasHideousLaughter {}

impl Action for TashasHideousLaughter {
    fn name(&self) -> &str {
        "tasha's hideous laughter"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["thl", "laughter", "hideous laughter"]
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
        action_and_slot(1)
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  hideous laughter: target collapses in fits");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Tasha's Hideous Laughter",
                    vec![
                        (target_id, Condition::Prone),
                        (target_id, Condition::Incapacitated),
                    ],
                ),
            }),
        ]
    }
}

pub static TASHAS_HIDEOUS_LAUGHTER: LazyLock<TashasHideousLaughter> =
    LazyLock::new(|| TashasHideousLaughter {});

/// Heal — level-6 evocation, action, 60 ft range. Restores 70 HP to a
/// single creature and ends Blinded, Deafened, Poisoned (5e RAW), and
/// clears any one Frightened / Charmed via condition cleanse. We pull
/// out a few key debuffs after the heal so it's not just a giant HP
/// patch — the spell is supposed to be a swiss-army-knife emergency
/// button.
pub struct HealSpellHigh {}

impl Action for HealSpellHigh {
    fn name(&self) -> &str {
        "heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["heal6", "high-heal"]
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
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(Heal {
                actor_id: target_id,
                amount: 70,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Blinded,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Deafened,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
            }),
        ]
    }
}

pub static HEAL_SPELL_HIGH: LazyLock<HealSpellHigh> = LazyLock::new(|| HealSpellHigh {});

/// Disintegrate — level-6 transmutation. DEX save vs the caster's spell
/// save DC: on fail 10d6+40 force; on save, nothing. The damage type is
/// Force (rarely resisted in our pool), so a hit is essentially
/// guaranteed to register as raw damage. No save-half.
pub struct Disintegrate {}

impl Action for Disintegrate {
    fn name(&self) -> &str {
        "disintegrate"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dsg", "disint"]
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
        action_and_slot(6)
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
        if save.passed() {
            return Vec::new();
        }
        let dmg = encounter.roll(&Dice::new(10, 6)) + 40;
        encounter.log(format!(
            "  disintegrate: 10d6+40({}) = {} force",
            dmg, dmg
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Force,
        })]
    }
}

pub static DISINTEGRATE: LazyLock<Disintegrate> = LazyLock::new(|| Disintegrate {});

/// Finger of Death — level-7 necromancy. CON save vs the caster's DC.
/// 7d8+30 necrotic on fail; half on success. We follow the half-on-save
/// pattern that big single-target damage spells use (Disintegrate-style
/// no-save-half would be too lethal at this level). The slain-target
/// raising-as-zombie clause from RAW isn't modeled.
pub struct FingerOfDeath {}

impl Action for FingerOfDeath {
    fn name(&self) -> &str {
        "finger of death"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fod", "fingerdeath"]
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
        action_and_slot(7)
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
        let dmg_full = encounter.roll(&Dice::new(7, 8)) + 30;
        let dmg = if save.passed() { dmg_full / 2 } else { dmg_full };
        encounter.log(format!(
            "  finger of death: 7d8+30({}) = {} necrotic",
            dmg_full, dmg
        ));
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static FINGER_OF_DEATH: LazyLock<FingerOfDeath> = LazyLock::new(|| FingerOfDeath {});

/// Power Word Stun — level-8 enchantment. If the target has 150 HP or
/// fewer, they are Stunned (no save) for 10 rounds. If they have more,
/// nothing happens. Hard-cap means the spell is a clean executioner
/// against weakened bosses. No damage.
pub struct PowerWordStun {}

impl Action for PowerWordStun {
    fn name(&self) -> &str {
        "power word stun"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pws", "powerstun"]
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
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        if target.hitpoints() > 150 {
            encounter.log("  power word stun: target too healthy — no effect");
            return Vec::new();
        }
        encounter.log("  power word stun: target locks up");
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Stunned,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static POWER_WORD_STUN: LazyLock<PowerWordStun> = LazyLock::new(|| PowerWordStun {});

/// Synaptic Static — level-5 enchantment. 20-ft radius burst (radius 4).
/// Every creature in area makes an INT save against the caster's spell
/// save DC: 8d6 psychic on fail, half on save. Fails also leave the
/// target Baned (–2 to attacks and saves for 1 round) — the load-bearing
/// rider that justifies it as a control spell, not just damage.
pub struct SynapticStatic {}

impl Action for SynapticStatic {
    fn name(&self) -> &str {
        "synaptic static"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["synaptic", "static"]
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
        action_and_slot(5)
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
        const RADIUS: isize = 4;
        let full = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!(
            "  synaptic static: 8d6({}) = {} psychic (each)",
            full, full
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
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
            let save = encounter.roll_save(tid, AbilityScoreType::Intelligence, dc);
            let dmg = if save.passed() { full / 2 } else { full };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Psychic,
                }));
            }
            // Baned only on a fail — the muddled rider that lasts one
            // round per the spell description.
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Baned,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static SYNAPTIC_STATIC: LazyLock<SynapticStatic> = LazyLock::new(|| SynapticStatic {});

/// Crown of Madness — level-2 enchantment, concentration. WIS save vs
/// the caster's spell DC. On fail, target is Charmed (mechanically: their
/// own actions become less useful through the Charmed flag, and the
/// charmer-target hostile-action gate prevents them from attacking the
/// caster). Humanoids only in 5e RAW; we don't enforce the type gate
/// since condition immunities already cover the immune-to-charm cases.
pub struct CrownOfMadness {}

impl Action for CrownOfMadness {
    fn name(&self) -> &str {
        "crown of madness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["com", "crown"]
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
        action_and_slot(2)
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
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  crown of madness: target falls under the caster's sway");
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
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Crown of Madness",
                    vec![(target_id, Condition::Charmed)],
                ),
            }),
        ]
    }
}

pub static CROWN_OF_MADNESS: LazyLock<CrownOfMadness> = LazyLock::new(|| CrownOfMadness {});

/// Word of Radiance — cleric cantrip. 5-ft burst centered on the caster
/// (radius 1 in tile-gap). Every creature in the area except the caster
/// makes a CON save vs the caster's WIS-based spell DC: 1d6 radiant on
/// fail, nothing on success. Pure cantrip — no spell slot.
pub struct WordOfRadiance {}

impl Action for WordOfRadiance {
    fn name(&self) -> &str {
        "word of radiance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wor", "radiance"]
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
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let center = caster.location();
        let damage = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!(
            "  word of radiance: 1d6({}) = {} radiant (each)",
            damage, damage
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            1,
            AbilityScoreType::Constitution,
            dc,
            damage,
            DamageType::Radiant,
        )
    }
}

pub static WORD_OF_RADIANCE: LazyLock<WordOfRadiance> = LazyLock::new(|| WordOfRadiance {});

/// Calm Emotions — level-2 enchantment. 60-ft range, 20-ft radius sphere
/// (radius 4 in tile-gap). Every humanoid in the area makes a CHA save
/// against the caster's spell DC; on fail, the target is suppressed of
/// Charmed and Frightened (5e RAW: "suppress" — we model by removing
/// the conditions, which is the practical equivalent for our model).
/// Concentration in 5e for re-arming the dispelled conditions if it
/// drops — we apply once, no concentration needed for the simple cleanse.
///
/// The spell affects friend and foe alike by RAW; we approximate the
/// caster's intent by aiming the cleanse at every actor in radius,
/// which gives the cleric a tool against enemy fear/charm spells.
pub struct CalmEmotions {}

impl Action for CalmEmotions {
    fn name(&self) -> &str {
        "calm emotions"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ce", "calm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        const RADIUS: isize = 4;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if !t.is_combat_active() {
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
            // 5e: no save = no effect. The save is *against* the cleanse
            // (a fey trying to keep its charm). On fail, the charm /
            // frighten is stripped.
            let save = encounter.roll_save(tid, AbilityScoreType::Charisma, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(RemoveCondition {
                actor_id: tid,
                condition: Condition::Charmed,
            }));
            effects.push(Box::new(RemoveCondition {
                actor_id: tid,
                condition: Condition::Frightened,
            }));
        }
        effects
    }
}

pub static CALM_EMOTIONS: LazyLock<CalmEmotions> = LazyLock::new(|| CalmEmotions {});

/// Suggestion — level-2 enchantment, concentration. 30-ft range, single
/// target. Target makes a WIS save vs the caster's spell DC; on fail, it
/// is Charmed by the caster for up to 8 hours (we use 10 rounds). The
/// charm enforces the "cannot attack the charmer" gate from action
/// validation, mirroring Charm Person / Crown of Madness.
pub struct Suggestion {}

impl Action for Suggestion {
    fn name(&self) -> &str {
        "suggestion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sug", "suggest"]
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
        action_and_slot(2)
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
        encounter.log("  suggestion: target's mind is bent to the caster's words");
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
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Suggestion",
                    vec![(target_id, Condition::Charmed)],
                ),
            }),
        ]
    }
}

pub static SUGGESTION: LazyLock<Suggestion> = LazyLock::new(|| Suggestion {});

/// Mass Suggestion — level-6 enchantment. 60-ft range, 30-ft radius
/// sphere (radius 6). Up to 12 creatures of the caster's choice in the
/// area each make a WIS save vs the caster's spell DC; failures are
/// Charmed by the caster for an extended duration (we use 10 rounds).
/// Unlike Suggestion this does NOT require concentration (RAW: "for up
/// to 24 hours" — no concentration line), so the caster keeps their
/// concentration slot free for other rope-a-dope effects.
pub struct MassSuggestion {}

impl Action for MassSuggestion {
    fn name(&self) -> &str {
        "mass suggestion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["msug", "mass-suggest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
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
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::SetCharmedBy;
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 12;

        // RAW lets the caster pick targets; the burst-targets helper
        // already filters to enemies (the only useful charm victims).
        let candidates = encounter.enemy_burst_targets(caster_id, point, RADIUS);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in candidates.into_iter().take(MAX_TARGETS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Charmed,
                timer: ConditionTimer::Rounds(10),
            }));
            effects.push(Box::new(SetCharmedBy {
                target_id: tid,
                charmer: Some(caster_id),
            }));
        }
        effects
    }
}

pub static MASS_SUGGESTION: LazyLock<MassSuggestion> = LazyLock::new(|| MassSuggestion {});

/// Sunburst — level-8 evocation. 60-ft range, 60-ft radius sphere of
/// brilliant sunlight (we cap the radius at 12 tile-gap for engine
/// sanity). Every creature in the area makes a CON save vs the caster's
/// spell DC: 12d6 radiant on fail, half on success. Failures are also
/// Blinded for 1 minute (10 rounds). Undead and oozes take the burst as
/// normal; the spell's "bright sunlight" tag isn't engine-modeled.
pub struct Sunburst {}

impl Action for Sunburst {
    fn name(&self) -> &str {
        "sunburst"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sun", "sunb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 12 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft per RAW = 60 tiles. We cap at 40 since the map is
        // typically that wide.
        Some(40)
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
        action_and_slot(8)
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
        const RADIUS: isize = 12;
        let full = encounter.roll(&Dice::new(12, 6));
        encounter.log(format!(
            "  sunburst: 12d6({}) = {} radiant (each)",
            full, full
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
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
            let dmg = if save.passed() { full / 2 } else { full };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Radiant,
                }));
            }
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        effects
    }
}

pub static SUNBURST: LazyLock<Sunburst> = LazyLock::new(|| Sunburst {});

/// Mass Heal — level-9 conjuration, action. A pool of 700 HP is divided
/// among any number of allies within range; each chosen ally regains HP
/// up to the pool. We model this by sorting allies by missing HP (most
/// hurt first) and pouring the pool until it's empty or every ally is
/// topped off. Also ends Blinded, Deafened, Poisoned on each target —
/// mirroring the Heal spell's status cleanse.
pub struct MassHeal {}

impl Action for MassHeal {
    fn name(&self) -> &str {
        "mass heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mh", "mass-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Self-targeted: the pool sweeps every ally in line-of-sight.
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        false
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
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let team = caster.team();
        // Allies who are alive (or merely dying — Mass Heal lifts them
        // out) and not already at max HP.
        let mut candidates: Vec<(i64, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let missing = a.max_hitpoints() as i64 - a.hitpoints() as i64;
                if missing <= 0 {
                    return None;
                }
                // Sort key: most missing HP first (negate so .sort is ascending).
                Some((-missing, *id))
            })
            .collect();
        candidates.sort_unstable();

        encounter.log("  mass heal: a 700-HP pool washes over the caster's allies");
        let mut pool: u32 = 700;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for (neg_missing, id) in candidates {
            if pool == 0 {
                break;
            }
            let need = (-neg_missing) as u32;
            let give = need.min(pool);
            pool -= give;
            // Revive first so dying allies are lifted out of Dying (which
            // also clears their auto-Prone) before the bulk heal tops
            // them up. ReviveDying is a no-op for live allies, so the
            // unconditional queue is safe.
            effects.push(Box::new(ReviveDying { actor_id: id }));
            effects.push(Box::new(Heal {
                actor_id: id,
                amount: give,
            }));
            for c in [Condition::Blinded, Condition::Deafened, Condition::Poisoned] {
                effects.push(Box::new(RemoveCondition {
                    actor_id: id,
                    condition: c,
                }));
            }
        }
        effects
    }
}

pub static MASS_HEAL: LazyLock<MassHeal> = LazyLock::new(|| MassHeal {});

/// Power Word Kill — level-9 enchantment. Single target with 100 HP or
/// fewer is killed outright (no save, no attack roll). Targets above
/// 100 HP are unaffected. We model "killed outright" as a direct
/// damage hit of `current_hp` necrotic so the standard death path runs
/// (death save start for PCs that die outright per RAW; instant Dead
/// for monsters). Range 60 ft.
pub struct PowerWordKill {}

impl Action for PowerWordKill {
    fn name(&self) -> &str {
        "power word kill"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwk", "kill"]
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
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let hp = target.hitpoints();
        if hp > 100 {
            encounter.log(format!(
                "  power word kill: target has {} HP (>100) \u{2014} unaffected",
                hp
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  power word kill: target has {} HP \u{2264} 100 \u{2014} struck down",
            hp
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: hp,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static POWER_WORD_KILL: LazyLock<PowerWordKill> = LazyLock::new(|| PowerWordKill {});

/// Meteor Swarm — level-9 evocation. 20-ft radius burst (radius 4 in
/// tile-gap; RAW it's four 40-ft spheres, we collapse to one big sphere
/// for engine simplicity). Every creature in the area makes a DEX save
/// vs the caster's spell DC: 20d6 fire + 20d6 bludgeoning on fail, half
/// on save. The two damage rolls share a single save outcome (RAW: one
/// save vs both packets), but they apply independently so resistance to
/// one type (a fire-resistant elemental) still eats the other half.
pub struct MeteorSwarm {}

impl Action for MeteorSwarm {
    fn name(&self) -> &str {
        "meteor swarm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "meteor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 1 mile per RAW; we cap at the map edge (40 tiles).
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire, DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
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
        const RADIUS: isize = 4;
        let fire = encounter.roll(&Dice::new(20, 6));
        let bludge = encounter.roll(&Dice::new(20, 6));
        encounter.log(format!(
            "  meteor swarm: 20d6({}) fire + 20d6({}) bludgeoning (each)",
            fire, bludge
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
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
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            let (f_dmg, b_dmg) = if save.passed() {
                (fire / 2, bludge / 2)
            } else {
                (fire, bludge)
            };
            if f_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: f_dmg,
                    damage_type: DamageType::Fire,
                }));
            }
            if b_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: b_dmg,
                    damage_type: DamageType::Bludgeoning,
                }));
            }
        }
        effects
    }
}

pub static METEOR_SWARM: LazyLock<MeteorSwarm> = LazyLock::new(|| MeteorSwarm {});

/// Prayer of Healing — level-2 evocation. Pick up to six allies within
/// 30 ft (12 tiles) of the caster; each regains 2d8 + WIS HP. RAW has a
/// 10-minute cast time (so it's strictly out-of-combat per book); we keep
/// it as a one-action in-combat heal because (a) our encounter loop has
/// no out-of-combat phase and (b) it slots cleanly into the cleric's
/// level-2 healing toolkit between Cure Wounds and Mass Cure Wounds.
/// Same dying-allies-included logic as Mass Cure Wounds — a dying ally
/// would just regain HP from the heal and exit the dying state on the
/// next HP roll, but allowing them as targets lets the cleric stabilize
/// a downed party member in bulk with a single 2nd-level slot.
pub struct PrayerOfHealing {}

impl Action for PrayerOfHealing {
    fn name(&self) -> &str {
        "prayer of healing"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["poh", "prayer"]
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
        action_and_slot(2)
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
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        // 30-ft range centered on the caster — reuse the burst helper
        // with the caster's own footprint as the anchor so distance math
        // matches every other ally-burst spell.
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 6;
        let raw = encounter.roll(&Dice::new(2, 8)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  prayer of healing: 2d8({}){:+} = {} HP each",
            raw, wis_mod, amount
        ));
        let mut targets = encounter.ally_burst_targets(caster_id, caster_loc, RADIUS);
        targets.truncate(MAX_TARGETS);
        targets
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static PRAYER_OF_HEALING: LazyLock<PrayerOfHealing> = LazyLock::new(|| PrayerOfHealing {});

/// Sunbeam — level-6 evocation, concentration. RAW is a 60-ft line that
/// blinds + damages each creature inside on a failed CON save (6d8
/// radiant on fail, half on save). Modelled as a single-target ranged
/// spell attack for engine simplicity (line targeting isn't yet a schema)
/// — 6d8 radiant on hit, and the target is Blinded for one round.
/// Concentration lets the caster sustain the spell to fire it on
/// subsequent turns (we don't yet model the action-per-turn repeat
/// rider, but the conc slot prevents stacking with other conc spells
/// and clears on damage like every other concentration effect).
pub struct Sunbeam {}

impl Action for Sunbeam {
    fn name(&self) -> &str {
        "sunbeam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sun", "beam"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60-ft line ≈ 24 tiles.
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
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
        // Caster ability-mod choice: clerics get WIS, wizards get INT.
        // Pick whichever yields the bigger save DC so cross-class users
        // (e.g. a multi-class wizard/cleric) still get their best attack.
        let int_mod = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let wis_mod = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let attack_bonus = int_mod.max(wis_mod);
        let (effs, dealt) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "sunbeam",
            attack_bonus,
            Dice::new(6, 8),
            0,
            DamageType::Radiant,
            false,
        );
        let mut effects = effs;
        // On hit, target is also Blinded for one round (until start of
        // their next turn) — the sun-flare clause from RAW.
        if dealt > 0 {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Blinded,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Sunbeam"),
        }));
        effects
    }
}

pub static SUNBEAM: LazyLock<Sunbeam> = LazyLock::new(|| Sunbeam {});

/// Resurrection — level-7 necromancy. Touch a creature that has been
/// dead no more than a century (in our engine: a Dying actor — Dead
/// actors get removed from `actors` so they can't be targeted). Restores
/// the target to full HP, cures Blinded / Deafened / Poisoned, and
/// strips all conditions that came with the Dying state (Unconscious,
/// Prone). One step beyond Revivify (which restores them to 1 HP) — the
/// cleric pays a 7th-level slot for a full top-up.
pub struct Resurrection {}

impl Action for Resurrection {
    fn name(&self) -> &str {
        "resurrection"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["res", "resurrect"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same dying-only gate as Revivify — the spell shouldn't be wasted
        // on healthy targets.
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
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        // Revive first (lifts the Dying state to 1 HP, cleans Prone /
        // Unconscious) and then top up to max with a regular heal —
        // ReviveDying is a no-op for non-Dying actors, so the chained
        // queue stays safe even if the gate above passes a borderline
        // case.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        for c in [Condition::Blinded, Condition::Deafened, Condition::Poisoned] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static RESURRECTION: LazyLock<Resurrection> = LazyLock::new(|| Resurrection {});

/// Power Word Heal — level-9 evocation. Single touch target regains all
/// HP, then the spell cleanses every captivating / control condition
/// (Charmed, Frightened, Paralyzed, Stunned) and stands them up from
/// Prone. The 5e RAW also lets the target use a reaction to stand and
/// remove the charmed/frightened/paralyzed/stunned riders; we collapse
/// the reaction step into the heal's side effects since we don't yet
/// have a "trigger reaction on heal" hook. Symmetric counterpart to
/// Power Word Kill — top of the heal tree at the same slot cost.
pub struct PowerWordHeal {}

impl Action for PowerWordHeal {
    fn name(&self) -> &str {
        "power word heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwh", "wordheal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
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
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        // ReviveDying first (idempotent for non-Dying actors); then heal
        // to full; then strip the captivating conditions plus Prone (the
        // RAW reaction lets the target stand up).
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        for c in [
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Stunned,
            Condition::Prone,
        ] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static POWER_WORD_HEAL: LazyLock<PowerWordHeal> = LazyLock::new(|| PowerWordHeal {});

/// Dimension Door — level-4 conjuration. The caster teleports up to 500 ft
/// (we cap to 120 tiles / 300 ft for map-realism) to a tile they can see.
/// No opportunity attacks (5e teleports bypass per-step OAs — handled by
/// `TeleportActor`). Distinct from Misty Step's bonus-action / 30-ft form:
/// long range, full Action cost, level-4 slot. RAW lets the caster bring
/// one willing creature along; we model the single-caster variant since
/// the picker UI doesn't have a "two-actor teleport" schema yet.
pub struct DimensionDoor {}

impl Action for DimensionDoor {
    fn name(&self) -> &str {
        "dimension door"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dd", "dimensiondoor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 500 ft RAW; capped to 120 tiles for map-scale.
        Some(120)
    }
    fn requires_los(&self) -> bool {
        // RAW: a location you can see, or a location you've visited /
        // describable distance. We require LOS for the simple case.
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
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
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
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static DIMENSION_DOOR: LazyLock<DimensionDoor> = LazyLock::new(|| DimensionDoor {});

/// Wall of Fire — level-4 evocation, concentration. The caster picks a
/// tile within 120 ft; every enemy whose footprint touches the chosen
/// point (radius 2 — approximates the 20-ft wall length) takes 5d8 fire
/// damage with no save and gains the Burning condition (DOT: 1d4 fire
/// per round-end until expiry). Friendly creatures inside the burst are
/// skipped — Wall of Fire RAW lets the caster pick which side of the
/// wall burns, so we model the "caster's allies face the cool side"
/// clause by using `enemy_burst_targets`. Concentration: dropping it
/// before the timer expires clears the Burning ride immediately.
pub struct WallOfFire {}

impl Action for WallOfFire {
    fn name(&self) -> &str {
        "wall of fire"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wof", "firewall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
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
        action_and_slot(4)
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
        let raw = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!(
            "  wall of fire: 5d8({}) = {} fire (enemies only)",
            raw, raw
        ));
        // Enemy-only AoE: friendly walkers don't get caught. Each enemy
        // takes the rolled damage and starts Burning for 3 rounds —
        // matches RAW's "spend a turn near the wall = sustained DOT" feel.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tagged: Vec<(usize, Condition)> = Vec::new();
        for id in encounter.enemy_burst_targets(caster_id, point, 2) {
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: raw,
                damage_type: DamageType::Fire,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Burning,
                timer: ConditionTimer::Rounds(3),
            }));
            tagged.push((id, Condition::Burning));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Wall of Fire", tagged),
        }));
        effects
    }
}

pub static WALL_OF_FIRE: LazyLock<WallOfFire> = LazyLock::new(|| WallOfFire {});

/// Cloudkill — level-5 conjuration, concentration. A 20-ft radius (radius
/// 4 on the 2.5 ft grid) cloud of yellow-green fog drifts where the
/// caster points. Every creature whose footprint touches the burst makes
/// a CON save vs the caster's INT-based DC: 5d8 poison on a fail, half on
/// a success. RAW the cloud also persists and re-damages over time — we
/// model the immediate hit but skip the per-turn re-damage to keep the
/// concentration plumbing simple. Poison immunity zeros the damage.
pub struct Cloudkill {}

impl Action for Cloudkill {
    fn name(&self) -> &str {
        "cloudkill"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ck", "poisoncloud"]
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
        action_and_slot(5)
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
        let raw = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!("  cloudkill: 5d8({}) = {} poison area", raw, raw));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Poison,
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Cloudkill"),
        }));
        effects
    }
}

pub static CLOUDKILL: LazyLock<Cloudkill> = LazyLock::new(|| Cloudkill {});

/// Insect Plague — level-5 conjuration, concentration. A 20-ft radius
/// (radius 4) cloud of biting locusts. Every creature in the cloud makes
/// a CON save vs the caster's WIS-based DC: 4d10 piercing on a fail,
/// half on a success. Pierces resistance for most undead/oozes — but
/// since we route through normal damage modifiers, immunity / resistance
/// applies as usual. Concentration: drop ends the swarm.
pub struct InsectPlague {}

impl Action for InsectPlague {
    fn name(&self) -> &str {
        "insect plague"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ip", "locusts"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW; cap to 96 tiles (240 ft).
        Some(96)
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
        action_and_slot(5)
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
        let raw = encounter.roll(&Dice::new(4, 10));
        encounter.log(format!(
            "  insect plague: 4d10({}) = {} piercing area",
            raw, raw
        ));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Piercing,
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Insect Plague"),
        }));
        effects
    }
}

pub static INSECT_PLAGUE: LazyLock<InsectPlague> = LazyLock::new(|| InsectPlague {});

/// Daylight — level-3 evocation. Anchors a 60-ft sphere of bright sunlight
/// to a tile within 120 ft. Every ally inside the radius gets the Daylit
/// condition, which imposes disadvantage on incoming attacks from
/// undead / fiend-flavored enemies (proxied by Necrotic / Poison
/// immunity, like the Warded clause). RAW the spell also dispels magical
/// darkness in the area; we don't model darkness terrain, so the
/// dispel-darkness clause is a no-op today. No concentration, but lasts
/// only ~10 rounds before the timer ticks the condition off each ally.
pub struct Daylight {}

impl Action for Daylight {
    fn name(&self) -> &str {
        "daylight"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["day", "sunlight"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Sphere of radius 6 (≈ 30 ft); anchored to a tile within range.
        TargetingSchema::Burst { radius: 6 }
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
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
        encounter
            .ally_burst_targets(caster_id, point, 6)
            .into_iter()
            .map(|id| {
                Box::new(ApplyCondition {
                    actor_id: id,
                    condition: Condition::Daylit,
                    timer: ConditionTimer::Rounds(10),
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static DAYLIGHT: LazyLock<Daylight> = LazyLock::new(|| Daylight {});

/// Fire Shield — level-4 evocation. The caster ignites in protective flame
/// for 10 rounds: they gain resistance to cold damage and any creature
/// that hits them with a melee attack within reach takes 2d8 fire damage
/// in retaliation. We model this with the FireShielded condition;
/// resolve_attack reads the condition to fire the reflective damage on
/// melee hits, and the holder also gains the generic DamageResistant
/// flag (which halves cold and most other damage — close enough for our
/// purposes). Self-only by default — RAW limits it to the caster.
pub struct FireShield {}

impl Action for FireShield {
    fn name(&self) -> &str {
        "fire shield"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs", "flameshield"]
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
        action_and_slot(4)
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
            condition: Condition::FireShielded,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static FIRE_SHIELD: LazyLock<FireShield> = LazyLock::new(|| FireShield {});

/// Sanctuary — level-1 abjuration, bonus action. Wards one ally so any
/// attacker targeting them must succeed on a WIS save vs the caster's
/// spell DC or pick a different target / lose the attack. We model this
/// via the engine-level `Sanctuary` condition: at attack-resolution
/// time, `resolve_attack` / `spell_attack_outcome` short-circuit on a
/// failed save (the attacker's swing whiffs). Hostile actions by the
/// warded actor end the spell — `Action::execute` clears the buff when
/// the holder casts a harmful spell or attacks.
pub struct Sanctuary {}

impl Action for Sanctuary {
    fn name(&self) -> &str {
        "sanctuary"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sanc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
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
        bonus_action_and_slot(1)
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
        // 10-round duration approximates 1 minute. The ward drops as soon
        // as the holder takes a hostile action (engine hook below).
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Sanctuary,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static SANCTUARY: LazyLock<Sanctuary> = LazyLock::new(|| Sanctuary {});

/// True Resurrection — level-9 necromancy. Restores a Dying creature to
/// full HP and strips every captivating / mind-affecting / wound rider
/// the body might be carrying. Functionally a Resurrection that also
/// drops Charmed / Frightened / Stunned / Paralyzed and stands the
/// target back up (Prone clear) — top-of-the-line cleric panic button.
/// Costs a 9th-level slot, single-target, touch.
pub struct TrueResurrection {}

impl Action for TrueResurrection {
    fn name(&self) -> &str {
        "true resurrection"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["trueres", "tres"]
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        // Top-of-line cleanse: every condition you'd want gone after dying.
        for c in [
            Condition::Blinded,
            Condition::Deafened,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Stunned,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Prone,
        ] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static TRUE_RESURRECTION: LazyLock<TrueResurrection> = LazyLock::new(|| TrueResurrection {});

/// Healing Spirit — level-2 conjuration, bonus action, concentration. A
/// shimmering spirit anchors to a tile; every ally whose footprint
/// touches the burst regains 1d6 HP. RAW the spirit moves and pulses
/// each round; we collapse to a single instant heal-burst at cast time
/// to keep the concentration plumbing simple — the bonus-action cost
/// and the AoE pattern are the load-bearing parts of the spell anyway.
pub struct HealingSpirit {}

impl Action for HealingSpirit {
    fn name(&self) -> &str {
        "healing spirit"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hs", "spirit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
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
        bonus_action_and_slot(2)
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
        let raw = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!(
            "  healing spirit: 1d6({}) = {} HP to each ally in area",
            raw, raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .ally_burst_targets(caster_id, point, 1)
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount: raw,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Healing Spirit"),
        }));
        effects
    }
}

pub static HEALING_SPIRIT: LazyLock<HealingSpirit> = LazyLock::new(|| HealingSpirit {});

/// Aid — level-2 abjuration, action. Boosts up to three creatures' max
/// HP by 5 (level-2 baseline) for 8 hours. We already have the simpler
/// single-target Aid; this aliased version is a no-op stub kept off the
/// spell list for now. (Engine note: see the existing AID for the impl.)
/// Aura of Vitality — level-3 evocation, concentration, bonus action.
/// Anchors a 30-ft radius aura that lets the caster spend a bonus action
/// each round to heal one ally inside the aura for 2d6 HP. We model the
/// instant cast as a single 2d6 heal-burst on every ally inside a radius-
/// 6 sphere at the caster's tile — collapses the per-round bonus-action
/// retrigger into one strong upfront heal that mirrors Mass Healing Word
/// at a slightly lower slot cost.
pub struct AuraOfVitality {}

impl Action for AuraOfVitality {
    fn name(&self) -> &str {
        "aura of vitality"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aov", "vitality"]
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
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster_loc) = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.location())
        else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  aura of vitality: 2d6({}) = {} HP to allies in aura",
            raw, raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .ally_burst_targets(caster_id, caster_loc, 6)
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount: raw,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Aura of Vitality"),
        }));
        effects
    }
}

pub static AURA_OF_VITALITY: LazyLock<AuraOfVitality> = LazyLock::new(|| AuraOfVitality {});

/// Wall of Force — level-5 evocation, concentration. Drops a panel of
/// invisible force at a tile within 120 ft. Anyone footprint-adjacent to
/// the panel at cast time is shoved one tile away (we approximate with a
/// `PullActor` *away from* the wall via negative max_tiles — no, just
/// pick a direction and use TeleportActor). Concrete effect today:
/// everyone in the burst takes 0 damage but is moved one tile away from
/// the anchor. Lasts 10 rounds. Concentration: dropping it doesn't
/// recall the moved actors.
pub struct WallOfForce {}

impl Action for WallOfForce {
    fn name(&self) -> &str {
        "wall of force"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["woforce", "force-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
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
        action_and_slot(5)
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
        // Approximate the panel by knocking enemies adjacent to the anchor
        // prone (no save) — a stand-in for "blocked by an invisible wall."
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .enemy_burst_targets(caster_id, point, 1)
            .into_iter()
            .map(|id| {
                Box::new(ApplyCondition {
                    actor_id: id,
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Wall of Force"),
        }));
        effects
    }
}

pub static WALL_OF_FORCE: LazyLock<WallOfForce> = LazyLock::new(|| WallOfForce {});

/// Spike Growth — level-2 transmutation, concentration, action. Plants
/// spikes across a 20-ft radius (we use radius-3 in our 2.5ft grid).
/// Enemies in the area get the `Spiked` condition; whenever they move,
/// the `MoveActor` side-effect rolls 2d4 piercing damage per step
/// against them (the rider lives in side_effects.rs to keep the damage
/// roll consistent across every motion source — walks, Pulls, etc.).
/// Concentration: ending the spell clears Spiked from every target.
pub struct SpikeGrowth {}

impl Action for SpikeGrowth {
    fn name(&self) -> &str {
        "spike growth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["spikes", "spike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        // Damage is movement-triggered, not on-cast. Surfacing
        // deals_damage=false keeps the AI's focus-fire pipeline from
        // picking this as a "first-strike" damage spell.
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
        action_and_slot(2)
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
        const RADIUS: isize = 3;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Spiked,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Spiked));
        }
        // Always start concentration: even with no current targets the
        // spike field exists and could catch a creature that walks in
        // later. We don't model "actors entering the area get Spiked"
        // (would need a positional re-check tick), but the concentration
        // marker keeps the slot in use.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Spike Growth", applied),
        }));
        effects
    }
}

pub static SPIKE_GROWTH: LazyLock<SpikeGrowth> = LazyLock::new(|| SpikeGrowth {});

/// Telekinesis — level-5 transmutation, concentration, action. Targets a
/// single creature within 60 ft; on a failed STR save vs the caster's
/// spell DC, the target is hoisted into the air — we apply the `Lifted`
/// condition (zeros movement) and pull them one tile toward the caster.
/// Concentration: dropping the spell clears Lifted. Each round the
/// caster could re-hoist a fresh target, but we collapse that into the
/// initial cast for now.
pub struct Telekinesis {}

impl Action for Telekinesis {
    fn name(&self) -> &str {
        "telekinesis"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tk", "lift"]
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
        action_and_slot(5)
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let caster_loc = caster.location();
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Lifted,
                timer: ConditionTimer::Permanent,
            }) as Box<dyn ApplicableSideEffect>,
            // Drag them one tile toward the caster — telekinetic grab.
            Box::new(PullActor {
                actor_id: target_id,
                toward: caster_loc,
                max_tiles: 1,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Telekinesis",
                    vec![(target_id, Condition::Lifted)],
                ),
            }),
        ]
    }
}

pub static TELEKINESIS: LazyLock<Telekinesis> = LazyLock::new(|| Telekinesis {});

/// Polymorph — level-4 transmutation, concentration, action. Targets a
/// creature within 60 ft; on a failed WIS save the target is transformed
/// into a beast form. We model the load-bearing mechanical changes:
/// (1) apply the `Polymorphed` condition (cosmetic; tracked for narrative)
/// and (2) grant a fixed `30` temp HP pool representing the beast form's
/// HP — damage drains the beast pool first, dropping it ends the
/// transformation when concentration ends. Allies targeted with the
/// spell get a +30 temp HP buff (5e RAW: caster picks the beast); we
/// apply uniformly regardless of allegiance for simplicity.
///
/// Concentration: ending the spell strips `Polymorphed` from the target;
/// the temp HP pool is left to natural attrition (we don't reset it on
/// drop — matches 5e where the target reverts to their previous form
/// with their own HP, but the beast pool isn't refunded).
pub struct Polymorph {}

impl Action for Polymorph {
    fn name(&self) -> &str {
        "polymorph"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["poly", "morph"]
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
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
        let caster_team = caster.team();
        let target_team = encounter
            .actors
            .get(&target_id)
            .map(|a| a.team())
            .unwrap_or(caster_team);
        // Enemies get a WIS save; willing allies auto-fail (5e RAW).
        if target_team != caster_team {
            let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
            let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                return Vec::new();
            }
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: 30,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Polymorph",
                    vec![(target_id, Condition::Polymorphed)],
                ),
            }),
        ]
    }
}

pub static POLYMORPH: LazyLock<Polymorph> = LazyLock::new(|| Polymorph {});

/// Globe of Invulnerability — level-6 abjuration, concentration, action.
/// Self-buff: while the caster concentrates, they have generic damage
/// resistance (we approximate the 5e "immune to spells of level 5 or
/// lower" clause with a flat half-damage rider via the `Globed` condition
/// — the condition's `effective_damage` hook halves all incoming damage).
/// Pure self-defense — drops on concentration loss.
pub struct GlobeOfInvulnerability {}

impl Action for GlobeOfInvulnerability {
    fn name(&self) -> &str {
        "globe of invulnerability"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["globe", "invuln"]
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
        action_and_slot(6)
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
                condition: Condition::Globed,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Globe of Invulnerability",
                    vec![(caster_id, Condition::Globed)],
                ),
            }),
        ]
    }
}

pub static GLOBE_OF_INVULNERABILITY: LazyLock<GlobeOfInvulnerability> =
    LazyLock::new(|| GlobeOfInvulnerability {});

/// Counterspell — level-3 abjuration, reaction. When another creature
/// casts a spell of level 3 or lower within 60 ft, the counterspeller
/// interrupts the cast and the spell fails. We approximate the reaction
/// timing by exposing Counterspell as an Action (not a Reaction trigger)
/// that targets a *currently concentrating* enemy — on cast, the target
/// loses their concentration (the most common "I'm running an active
/// spell" handle in our model). This collapses Counterspell's
/// interrupt-on-cast clause into a "rip the buff" effect since we don't
/// have spell-cast triggers wired into the reaction bus. Slot cost is
/// the level-3 default; targeting an unconcentrating enemy fizzles the
/// cast (validation gate).
pub struct Counterspell {}

impl Action for Counterspell {
    fn name(&self) -> &str {
        "counterspell"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs", "counter"]
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
        // Spell slot only — Counterspell is RAW a reaction, but we don't
        // have a spell-cast trigger to fire from yet, so it gates on
        // Action availability and the level-3 slot to keep the gating
        // symmetric with other slotted spells.
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Valid against any target that is either concentrating or
        // holding a dispellable buff — otherwise the cast does nothing
        // useful and we'd be wasting the slot.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.is_concentrating()
            || target
                .conditions()
                .keys()
                .any(|c| c.is_dispellable_buff())
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
        vec![Box::new(DispelMagicOn {
            target_id,
        })]
    }
}

pub static COUNTERSPELL: LazyLock<Counterspell> = LazyLock::new(|| Counterspell {});

/// Booming Blade — cantrip. Make a melee attack against a target within 5ft;
/// on hit, weapon damage as normal plus the target is marked with
/// `BoomingBladeMarked` — they take an extra 1d8 thunder the *next* time
/// they move voluntarily before the start of the caster's next turn. We
/// reuse the spell-attack pipeline with a 0d0 damage roll for the cantrip
/// itself (the weapon-attack half is folded in via the rider — at cantrip
/// scaling, the headline is the thunder rider, not the swing's main
/// damage). On hit the mark applies with a 1-round timer so it ticks off
/// the holder's turn cleanly. Misses do nothing.
pub struct BoomingBlade {}

impl Action for BoomingBlade {
    fn name(&self) -> &str {
        "booming blade"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bb", "boom"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
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
        // 1d8 thunder on the touch itself (cantrip "weapon" damage at
        // base scaling). The rider hits on movement via the
        // BoomingBladeMarked condition.
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "booming blade",
            attack_bonus,
            Dice::new(1, 8),
            DamageType::Thunder,
            true,
        );
        // Mark on hit only (empty effect list = miss).
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::BoomingBladeMarked,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static BOOMING_BLADE: LazyLock<BoomingBlade> = LazyLock::new(|| BoomingBlade {});

/// Tasha's Mind Whip — level-2 enchantment. Target within 90ft makes an INT
/// save vs the caster's spell DC: pass = half, fail = full 3d6 psychic and
/// the target loses one of action / bonus action / reaction on their next
/// turn. We model the reaction-loss via the `NoReaction` rider and the
/// action-loss via the `MindWhipped` condition (consumed at the start of
/// the next turn by `reset_for_new_round`, zeroing the action slot).
pub struct MindWhip {}

impl Action for MindWhip {
    fn name(&self) -> &str {
        "mind whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mw", "whip"]
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
        action_and_slot(2)
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
        let raw = encounter.roll(&Dice::new(3, 6));
        let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, dc);
        let dmg = if save.passed() { raw / 2 } else { raw };
        encounter.log(format!(
            "  mind whip: 3d6({}) = {} psychic",
            raw, dmg
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        // Action-economy debuff lands only on a fail per RAW.
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::MindWhipped,
                timer: ConditionTimer::Permanent,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::NoReaction,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static MIND_WHIP: LazyLock<MindWhip> = LazyLock::new(|| MindWhip {});

/// Crusader's Mantle — level-3 evocation, concentration. Self-buff aura that
/// makes every weapon hit by the caster (and, RAW, allies within 30ft) deal
/// +1d4 radiant. We model the load-bearing self-cast version: the caster
/// gains the `CrusadersMantled` condition for the duration. The +1d4
/// radiant rider lives on the `resolve_attack` path. We skip the aura-
/// extend-to-allies clause because the aura-tick infrastructure isn't in
/// place; for simplicity any willing caster gets the buff and concentration
/// holds the spell.
pub struct CrusadersMantle {}

impl Action for CrusadersMantle {
    fn name(&self) -> &str {
        "crusader's mantle"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cm", "mantle", "crusader"]
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
        action_and_slot(3)
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
                condition: Condition::CrusadersMantled,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Crusader's Mantle",
                    vec![(caster_id, Condition::CrusadersMantled)],
                ),
            }),
        ]
    }
}

pub static CRUSADERS_MANTLE: LazyLock<CrusadersMantle> = LazyLock::new(|| CrusadersMantle {});

/// Earthquake — level-8 evocation, concentration. Burst at a point within
/// 500ft; every enemy in a 20ft radius (= 4-tile gap) makes a STR save vs
/// the caster's spell DC: fail = knocked Prone and takes 5d6 bludgeoning,
/// pass = no damage / no prone. Allies are spared (caster picks the safe
/// arc, per the spell's RAW "ground rupture" flavor). Damage is rolled
/// once and shared across all victims (matches 5e shared-roll AoE
/// semantics). Concentration so re-casting drops cleanly.
pub struct Earthquake {}

impl Action for Earthquake {
    fn name(&self) -> &str {
        "earthquake"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eq", "quake"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 500 ft RAW, capped to 120 tiles for our map scale.
        Some(120)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
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
        const RADIUS: isize = 4;
        let raw = encounter.roll(&Dice::new(5, 6));
        encounter.log(format!(
            "  earthquake: 5d6({}) shared bludgeoning",
            raw
        ));
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Strength, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Bludgeoning,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Earthquake"),
        }));
        effects
    }
}

pub static EARTHQUAKE: LazyLock<Earthquake> = LazyLock::new(|| Earthquake {});

/// Time Stop — level-9 transmutation. Caster gets an extra Action and an
/// extra Bonus Action *immediately* (the 5e "1d4+1 turns of solo activity"
/// is collapsed to a one-turn burst of action economy). The TimeStopped
/// condition is a flag for the dispel pipeline / UI; the action-economy
/// boost is the load-bearing mechanical effect, delivered via two
/// `GiveResource` side-effects. Self-only; no save / no targeting.
pub struct TimeStop {}

impl Action for TimeStop {
    fn name(&self) -> &str {
        "time stop"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "timestop"]
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
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::GiveResource;
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::TimeStopped,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::Action,
            }),
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::BonusAction,
            }),
        ]
    }
}

pub static TIME_STOP: LazyLock<TimeStop> = LazyLock::new(|| TimeStop {});

/// Wish — level-9 conjuration. The 5e RAW spell can mimic any sub-9 spell or
/// produce one of a small set of canonical effects. We model the "restore
/// up to twenty creatures to full HP" wish: every ally within 60ft is
/// healed to full HP. Self-only cast, no save, no targeting beyond the
/// implicit ally radius.
pub struct Wish {}

impl Action for Wish {
    fn name(&self) -> &str {
        "wish"
    }
    fn aliases(&self) -> Vec<&str> {
        vec![]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
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
        action_and_slot(9)
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
        let center = caster.location();
        // 60ft = 24 tiles. Includes the caster.
        let allies = encounter.ally_burst_targets(caster_id, center, 24);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in allies {
            let Some(ally) = encounter.actors.get(&tid) else {
                continue;
            };
            // Full heal: the "missing HP" delta seeded as the Heal value.
            let missing = ally.max_hitpoints().saturating_sub(ally.hitpoints());
            if missing == 0 {
                continue;
            }
            effects.push(Box::new(Heal {
                actor_id: tid,
                amount: missing,
            }));
        }
        encounter.log(format!(
            "  wish: blessing {} ally(ies) to full HP",
            effects.len()
        ));
        effects
    }
}

pub static WISH: LazyLock<Wish> = LazyLock::new(|| Wish {});

/// Forcecage — level-7 evocation. Target within 100ft makes a CHA save vs
/// the caster's spell DC (RAW: no save if the cage is set up as the
/// "solid cage" variant, but we keep one save for symmetry with other
/// imprisonment spells). On fail the target gains the `Caged` condition
/// for 10 rounds (≈1 minute RAW). Caged zeros movement and blocks
/// reactions via the existing condition wiring.
pub struct Forcecage {}

impl Action for Forcecage {
    fn name(&self) -> &str {
        "forcecage"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fc", "cage"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100 ft = 40 tiles.
        Some(40)
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
        action_and_slot(7)
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Charisma, dc);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Caged,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static FORCECAGE: LazyLock<Forcecage> = LazyLock::new(|| Forcecage {});

/// Crown of Stars — level-7 evocation. Self-cast that grants the caster a
/// halo of seven motes for the duration. Each weapon hit by the caster
/// rolls +1d8 radiant — we model the per-mote charge clause as a flat
/// per-hit rider via `resolve_attack`'s `CrownOfStars` lookup. Lasts
/// 10 rounds (≈1 hour RAW, capped here to a long Rounds timer). Doesn't
/// require concentration.
pub struct CrownOfStarsSpell {}

impl Action for CrownOfStarsSpell {
    fn name(&self) -> &str {
        "crown of stars"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cos", "crown"]
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
        action_and_slot(7)
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
            condition: Condition::CrownOfStars,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static CROWN_OF_STARS: LazyLock<CrownOfStarsSpell> = LazyLock::new(|| CrownOfStarsSpell {});

/// Fear — level-3 illusion, concentration. 30-foot cone of dread (4-tile
/// burst). Each enemy in the burst makes a WIS save vs the caster's DC:
/// fail = Frightened for the spell's duration; pass = no effect. We use
/// the shared `enemy_burst_targets` partition so allies in the blast are
/// spared. Concentration so a re-cast / damage drop cleans up the entire
/// Frightened pool in one shot.
pub struct Fear {}

impl Action for Fear {
    fn name(&self) -> &str {
        "fear"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fr", "terror"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self-origin cone; the burst point sits right in front of the
        // caster. Cap the targeting tile to the caster's footprint so
        // the cone always engulfs them as the cone's origin.
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
        action_and_slot(3)
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
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Frightened));
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Fear", applied),
            }));
        }
        effects
    }
}

pub static FEAR: LazyLock<Fear> = LazyLock::new(|| Fear {});

/// Greater Restoration — level-5 abjuration. Touch-range cleanse + heal.
/// Removes one of: Charmed / Petrified / Paralyzed / Stunned / one
/// exhaustion level (we don't model exhaustion). Then heals 4d8 + caster's
/// spellcasting modifier. Distinct from Lesser Restoration: GR can lift
/// the heavyweight lockdown conditions LR can't touch, and pairs the
/// cleanse with a real heal — paired action-economy efficiency. RAW
/// requires a 100gp diamond as material; we don't model components.
pub struct GreaterRestoration {}

impl Action for GreaterRestoration {
    fn name(&self) -> &str {
        "greater restoration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gr", "grestore"]
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
    fn is_heal(&self) -> bool {
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
        action_and_slot(5)
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
        let mod_bonus = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(4, 8));
        let amount = (raw as i32 + mod_bonus).max(0) as u32;
        encounter.log(format!(
            "  greater restoration: 4d8({}){:+} = {} HP",
            raw, mod_bonus, amount
        ));
        vec![
            Box::new(crate::engine::side_effects::RemoveOneOfConditions {
                actor_id: target_id,
                candidates: Self::CANDIDATES.to_vec(),
            }),
            Box::new(Heal {
                actor_id: target_id,
                amount,
            }),
        ]
    }
}

impl GreaterRestoration {
    /// Heavyweight conditions Greater Restoration is allowed to lift, in
    /// priority order. Distinct from the Lesser Restoration list — GR
    /// targets the lockdown set (Paralyzed, Stunned, Petrified, Charmed)
    /// that LR can't touch. Includes Lesser-Restoration's targets too so
    /// a stuck-with-only-GR caster can still cleanse Poisoned / etc.
    /// Also lifts Exhausted (RAW: GR removes one level of exhaustion;
    /// we model the simplified single-tier flag so cleansing it ends
    /// the condition outright) and Feebled (RAW: GR explicitly removes
    /// Feeblemind's mind-shattering effect — high-priority since it
    /// otherwise locks down INT/WIS/CHA saves indefinitely).
    const CANDIDATES: [Condition; 10] = [
        Condition::Petrified,
        Condition::Paralyzed,
        Condition::Stunned,
        Condition::Feebled,
        Condition::Charmed,
        Condition::Frightened,
        Condition::Exhausted,
        Condition::Poisoned,
        Condition::Blinded,
        Condition::Deafened,
    ];
}

pub static GREATER_RESTORATION: LazyLock<GreaterRestoration> =
    LazyLock::new(|| GreaterRestoration {});

/// Compelled Duel — level-1 enchantment, concentration. The paladin
/// challenges a target to a duel: the target makes a WIS save vs the
/// caster's spell DC. Fail = target is `Dueled` (attacks against anyone
/// other than the caster are at disadvantage — see `compute_attack_mode`).
/// Pass = no effect. The duel is tracked via `dueled_by` so the engine
/// knows the anchor. Caster picks the toughest enemy in melee range so
/// the paladin uses themselves as a tank.
///
/// We use the existing Charm save-immunity gate (undead / constructs)
/// here: a creature that can't be enchanted shrugs off the duel.
pub struct CompelledDuel {}

impl Action for CompelledDuel {
    fn name(&self) -> &str {
        "compelled duel"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cd-spell", "duel"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles.
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
        // Bonus action + level-1 slot — RAW Compelled Duel is bonus-action
        // economy so the paladin can still swing their greatsword on the
        // same turn they open the challenge.
        bonus_action_and_slot(1)
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
        // Charm-immune creatures (undead / constructs in our pool) shrug
        // off the enchantment by RAW — no save needed.
        if let Some(target) = encounter.actors.get(&target_id)
            && target.is_immune_to_condition(Condition::Charmed)
        {
            encounter.log("  compelled duel: target resists enchantment".to_string());
            return Vec::new();
        }
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
                condition: Condition::Dueled,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(crate::engine::side_effects::SetDueledBy {
                target_id,
                duelist: Some(caster_id),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Compelled Duel",
                    vec![(target_id, Condition::Dueled)],
                ),
            }),
        ]
    }
}

pub static COMPELLED_DUEL: LazyLock<CompelledDuel> = LazyLock::new(|| CompelledDuel {});

/// Config-driven Smite spell. Every Smite (Searing / Wrathful / Branding
/// / Blinding) shares the same shape: bonus-action cast, level-N slot,
/// concentration, applies a one-shot "primed" condition to the caster
/// that the on-hit rider table in `engine::attack` consumes on the next
/// melee weapon hit. The four spells differ only in slot level, log
/// name, and which prime they apply — collapsed into one impl so adding
/// a fifth smite is a one-entry table addition.
pub struct SmiteSpell {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub spell_slot_lvl: u32,
    /// Caster-side condition the smite primes — read by the on-hit
    /// rider table to apply the bonus damage and follow-up effect.
    pub prime: Condition,
    /// Spell name string used for concentration tracking. Matches the
    /// 5e RAW spell name so concentration logs read cleanly.
    pub concentration_name: &'static str,
}

impl Action for SmiteSpell {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Like Divine Smite — the rider damage lands on the *next* hit,
        // not on this action's resolution. False keeps the AI's
        // focus-fire pipeline from picking the prime over an actual
        // attack.
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
        bonus_action_and_slot(self.spell_slot_lvl)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't double-prime: re-casting the same smite while already
        // primed wastes a slot. The AI's pipeline doesn't deeply model
        // this; the gate is here for symmetry with Divine Smite's
        // `!has_condition(Smiting)` guard.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(self.prime))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 10-round prime window — long enough that a primed paladin who
        // can't connect on the cast turn still has the better part of a
        // minute (in 5e time) to land it. Concentration anchors the
        // spell so taking damage can break the prime via the CON-save
        // path, matching RAW.
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: self.prime,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    self.concentration_name,
                    vec![(caster_id, self.prime)],
                ),
            }),
        ]
    }
}

/// Searing Smite — 1st-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +1d6 fire and ignites
/// the target (Burning, 3 rounds).
pub static SEARING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "searing smite",
    aliases: &["searing", "smite-fire"],
    spell_slot_lvl: 1,
    prime: Condition::SearingSmiting,
    concentration_name: "Searing Smite",
};

/// Wrathful Smite — 1st-level paladin enchantment, bonus action,
/// concentration. Primes the next melee hit with +1d6 psychic and a
/// WIS save (vs caster CHA-DC) gates Frightened (10 rounds) on fail.
pub static WRATHFUL_SMITE: SmiteSpell = SmiteSpell {
    display_name: "wrathful smite",
    aliases: &["wrathful", "smite-fear"],
    spell_slot_lvl: 1,
    prime: Condition::WrathfulSmiting,
    concentration_name: "Wrathful Smite",
};

/// Branding Smite — 2nd-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +2d6 radiant and
/// brands the target (Outlined, 10 rounds — attackers get advantage).
pub static BRANDING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "branding smite",
    aliases: &["branding", "smite-brand"],
    spell_slot_lvl: 2,
    prime: Condition::BrandingSmiting,
    concentration_name: "Branding Smite",
};

/// Blinding Smite — 3rd-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +3d8 radiant and a
/// CON save gates Blinded (10 rounds) on fail.
pub static BLINDING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "blinding smite",
    aliases: &["blinding", "smite-blind"],
    spell_slot_lvl: 3,
    prime: Condition::BlindingSmiting,
    concentration_name: "Blinding Smite",
};

/// Flame Strike — 5th-level evocation. A column of divine fire descends
/// on a tile within 60ft (24 tiles); every creature whose footprint is
/// within a 2-tile (10ft) radius of the point makes a DEX save vs the
/// caster's WIS-based DC. On fail: 4d6 fire + 4d6 radiant. On success:
/// half. The mixed damage type is the spell's signature — it slips past
/// fire-resistant fiends (radiant lands) and undead with radiant
/// resistance (fire lands), making it the cleric's go-to AoE.
pub struct FlameStrike {}

impl Action for FlameStrike {
    fn name(&self) -> &str {
        "flame strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs", "fstrike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire, DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
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
        // Roll the two damage halves separately so the per-actor
        // resistance / immunity lookup applies independently — a fire-
        // immune efreet still eats the radiant half, and a radiant-
        // resistant celestial still takes full fire.
        let fire_raw = encounter.roll(&Dice::new(4, 6));
        let rad_raw = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!(
            "  flame strike: 4d6({}) fire + 4d6({}) radiant",
            fire_raw, rad_raw
        ));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            fire_raw,
            DamageType::Fire,
        );
        effects.extend(crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            rad_raw,
            DamageType::Radiant,
        ));
        effects
    }
}

pub static FLAME_STRIKE: LazyLock<FlameStrike> = LazyLock::new(|| FlameStrike {});

/// Heat Metal — 2nd-level transmutation, concentration, bonus action
/// (RAW: Action on cast, bonus action to repeat the damage each round;
/// we collapse to a one-tap concentration mark that ticks the damage on
/// the holder's turn-start). The target's metal armor / weapon glows
/// red-hot: they take 2d8 fire on cast, and an additional 2d8 fire at
/// the start of each of their turns while concentration holds. They
/// also have disadvantage on attack rolls and ability checks (the
/// HeatMetaled condition feeds `compute_attack_mode`'s disadvantage
/// clause). No save — RAW gives a CON save each turn to drop the gear
/// but we keep the simulation crisp by skipping it.
pub struct HeatMetal {}

impl Action for HeatMetal {
    fn name(&self) -> &str {
        "heat metal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm-fire", "heat"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
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
        action_and_slot(2)
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
        let raw = encounter.roll(&Dice::new(2, 8));
        encounter.log(format!("  heat metal: 2d8({}) fire on cast", raw));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Fire,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::HeatMetaled,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Heat Metal",
                    vec![(target_id, Condition::HeatMetaled)],
                ),
            }),
        ]
    }
}

pub static HEAT_METAL: LazyLock<HeatMetal> = LazyLock::new(|| HeatMetal {});

/// Chain Lightning — 6th-level evocation. A bolt of lightning leaps
/// from the caster to a primary target (DEX save for half, 10d8
/// lightning), then forks to up to 3 additional creatures within
/// 5 tiles (25ft RAW) of the primary — each rolling its own DEX save
/// for half. Selection of secondary targets is deterministic: the 3
/// nearest combat-active actors (other than the primary), excluding
/// the caster. Mixed-team — fork hits allies as well as enemies, so
/// the AI's friendly-fire heuristic gates casting through
/// `try_attack_aoe`'s pool check.
pub struct ChainLightning {}

impl Action for ChainLightning {
    fn name(&self) -> &str {
        "chain lightning"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["chain", "cl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles, but we cap to the engine's standard
        // long-range cantrip reach to keep the targeting picker honest.
        Some(60)
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
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(primary_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(10, 8));
        encounter.log(format!(
            "  chain lightning: 10d8({}) lightning (primary + forks)",
            raw
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Primary target — full DEX save for half.
        let save = encounter.roll_save(primary_id, AbilityScoreType::Dexterity, dc);
        let dmg = if save.passed() { raw / 2 } else { raw };
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: primary_id,
                amount: dmg,
                damage_type: DamageType::Lightning,
            }));
        }

        // Find the 3 nearest combat-active actors within 5 tiles of the
        // primary — caster excluded so the bolt doesn't bite its source.
        let Some(primary) = encounter.actors.get(&primary_id) else {
            return effects;
        };
        let primary_loc = primary.location();
        let primary_size = get_tiles_from_size(primary.size());
        let mut forks: Vec<(isize, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || *id == primary_id || !a.is_combat_active() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    primary_loc,
                    primary_size,
                );
                if dist > 5 {
                    return None;
                }
                Some((dist, *id))
            })
            .collect();
        // Deterministic by (distance, id) so seeded tests are stable.
        forks.sort_unstable();
        for (_, fork_id) in forks.into_iter().take(3) {
            let save = encounter.roll_save(fork_id, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: fork_id,
                    amount: dmg,
                    damage_type: DamageType::Lightning,
                }));
            }
        }
        effects
    }
}

pub static CHAIN_LIGHTNING: LazyLock<ChainLightning> = LazyLock::new(|| ChainLightning {});

/// Goodberry — 5e druid level-1 transmutation. Conjures up to 10 magical
/// berries; eating one restores 1 HP. We collapse the "10 berries over an
/// hour" RAW into a single in-combat heal of 10 HP on a touch-range ally
/// — the caster's WIS modifier isn't added (RAW: berries are a flat 1 HP
/// each). Behaves like a low-cost emergency top-up: cheap level-1 slot,
/// touch range, schemes nicely with Healing Word for ranged backup.
pub struct Goodberry {}

impl Action for Goodberry {
    fn name(&self) -> &str {
        "goodberry"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gb", "berry"]
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
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log("  goodberry: 10 HP restored from magical berries".to_string());
        vec![Box::new(Heal {
            actor_id: target_id,
            amount: 10,
        })]
    }
}

pub static GOODBERRY: LazyLock<Goodberry> = LazyLock::new(|| Goodberry {});

/// Moonbeam — 5e druid level-2 evocation, concentration. A 5ft-radius
/// beam of silvery light strikes the targeted point. Every creature in
/// the beam makes a CON save; fail = 2d10 radiant, pass = half. We treat
/// the cast as a single burst (Spirit Guardians shape) since the engine
/// doesn't yet model "lingering area, re-rolled each round" AoEs. Targets
/// allies and enemies alike (it's an indiscriminate beam) and starts
/// concentration so the AI knows it's holding it.
pub struct Moonbeam {}

impl Action for Moonbeam {
    fn name(&self) -> &str {
        "moonbeam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mb", "moon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
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
        action_and_slot(2)
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
        let raw = encounter.roll(&Dice::new(2, 10));
        encounter.log(format!("  moonbeam: 2d10({}) radiant beam", raw));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Radiant,
        );
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Moonbeam"),
        }));
        effs
    }
}

pub static MOONBEAM: LazyLock<Moonbeam> = LazyLock::new(|| Moonbeam {});

/// Call Lightning — 5e druid level-3 conjuration, concentration. Calls a
/// storm cloud overhead; on cast and on each subsequent Action this turn,
/// a lightning bolt strikes a chosen point dealing 3d10 lightning (DEX
/// save, half on success) to every creature within 5ft of the strike.
/// We model the cast as a single 3d10 burst at the point + concentration
/// install — re-casts of the same spell while concentrating proc the
/// bolt anew (the action picker handles that path since the slot is gone
/// after the initial cast, RAW's "without spending a spell slot" repeat
/// fires the standard concentration channel). Single-tile burst (5ft).
pub struct CallLightning {}

impl Action for CallLightning {
    fn name(&self) -> &str {
        "call lightning"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cl-spell", "lightning", "callbolt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft burst — single-tile strike. Targets in the same tile as
        // the strike point catch the full radius.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft strike radius.
        Some(48)
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
        action_and_slot(3)
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
        let raw = encounter.roll(&Dice::new(3, 10));
        encounter.log(format!("  call lightning: 3d10({}) lightning bolt", raw));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Lightning,
        );
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Call Lightning"),
        }));
        effs
    }
}

pub static CALL_LIGHTNING: LazyLock<CallLightning> = LazyLock::new(|| CallLightning {});

/// Sleet Storm — 5e druid level-3 conjuration, concentration. Freezing
/// rain coats a 20-ft cylinder; creatures inside make a DEX save or be
/// knocked Prone, and concentrating spellcasters in the area must save
/// on a CON check or drop concentration. We model: enemy-only burst
/// (caster + allies stay vertical), DEX save vs Prone on fail, plus an
/// optional concentration-break for any enemy holding concentration. No
/// damage — the storm is pure crowd-control. 20ft radius = 4 tiles.
pub struct SleetStorm {}

impl Action for SleetStorm {
    fn name(&self) -> &str {
        "sleet storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sleet", "ss-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
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
        action_and_slot(3)
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
        const RADIUS: isize = 4;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Prone));
            // Concentration break: any enemy holding concentration must
            // succeed on a CON save or drop. We piggyback on the engine's
            // drop_concentration helper rather than re-rolling here — the
            // CON save uses the same DC as the DEX save (5e RAW: "DC
            // equal to your spell save DC").
            if encounter
                .actors
                .get(&tid)
                .is_some_and(|a| a.is_concentrating())
            {
                let conc_save =
                    encounter.roll_save(tid, AbilityScoreType::Constitution, dc);
                if !conc_save.passed() {
                    encounter.drop_concentration(tid);
                }
            }
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Sleet Storm", applied),
            }));
        }
        effects
    }
}

pub static SLEET_STORM: LazyLock<SleetStorm> = LazyLock::new(|| SleetStorm {});

/// Reverse Gravity — 5e level-7 transmutation, concentration. Gravity
/// reverses in a wide column; creatures inside fall *up*, then crash
/// back down when concentration drops. We model the cast's load-bearing
/// half: a STR save (failure = thrown around, Prone) plus 8d6 bludgeoning
/// to fallen creatures (the fall damage). Allies in the column are
/// included — RAW makes no friend/foe distinction. Concentration is
/// installed so dispel can lift the gravity column.
pub struct ReverseGravity {}

impl Action for ReverseGravity {
    fn name(&self) -> &str {
        "reverse gravity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rg", "reverse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 10 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100 ft = 40 tiles.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
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
        // Druid (WIS) or wizard (INT) — pick the caster's better DC.
        let dc = caster
            .spell_save_dc(AbilityScoreType::Wisdom)
            .max(caster.spell_save_dc(AbilityScoreType::Intelligence));
        let raw = encounter.roll(&Dice::new(8, 6));
        encounter.log(format!(
            "  reverse gravity: 8d6({}) bludgeoning fall damage",
            raw
        ));
        const RADIUS: isize = 10;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        // RAW makes no ally/enemy distinction — every creature in the
        // column rolls a save. Caster is excluded (they cast it; they
        // brace themselves).
        for tid in encounter.burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Strength, dc);
            if save.passed() {
                continue;
            }
            // On a fail: 8d6 bludgeoning + Prone (the crash landing).
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Bludgeoning,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Prone));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Reverse Gravity", applied),
        }));
        effects
    }
}

pub static REVERSE_GRAVITY: LazyLock<ReverseGravity> = LazyLock::new(|| ReverseGravity {});

/// Storm of Vengeance — 5e druid level-9 conjuration, concentration. A
/// massive storm churns above the targeted point. The apex druid spell:
/// huge radius, mixed thunder + lightning damage, plus a CON save vs
/// Deafened. We collapse the multi-round RAW (acid → wind → hail) into
/// a single big burst on cast: 2d6 thunder + 4d6 lightning on every
/// enemy in the 30ft sphere; CON save halves and dodges the Deafened
/// rider. Caster + allies are spared by the enemy_burst_targets filter.
pub struct StormOfVengeance {}

impl Action for StormOfVengeance {
    fn name(&self) -> &str {
        "storm of vengeance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sov", "stormv"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // "Sight" range RAW — we cap to the engine's long-range bracket
        // so the picker stays sane.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder, DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
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
        const RADIUS: isize = 6;

        let thunder = encounter.roll(&Dice::new(2, 6));
        let lightning = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!(
            "  storm of vengeance: 2d6({}) thunder + 4d6({}) lightning",
            thunder, lightning
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut deafened: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Constitution, dc);
            let t_dmg = if save.passed() { thunder / 2 } else { thunder };
            let l_dmg = if save.passed() { lightning / 2 } else { lightning };
            if t_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: t_dmg,
                    damage_type: DamageType::Thunder,
                }));
            }
            if l_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: l_dmg,
                    damage_type: DamageType::Lightning,
                }));
            }
            // Deafened rider only on failed saves — the storm's roar
            // ruptures eardrums on a fail.
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Deafened,
                    timer: ConditionTimer::Rounds(10),
                }));
                deafened.push((tid, Condition::Deafened));
            }
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Storm of Vengeance", deafened),
        }));
        effects
    }
}

pub static STORM_OF_VENGEANCE: LazyLock<StormOfVengeance> =
    LazyLock::new(|| StormOfVengeance {});

/// Hellish Rebuke — 5e level-1 evocation (warlock signature). Single-
/// target bonus-action damage at 60ft: target makes a DEX save vs the
/// caster's CHA-based DC. On fail: 2d10 fire; on save: half. Cast as a
/// bonus action here for engine simplicity — RAW's reaction-on-damage
/// gating doesn't fit the action picker, but the level-1 slot + bonus-
/// action cost matches the spell's combat tempo.
pub struct HellishRebuke {}

impl Action for HellishRebuke {
    fn name(&self) -> &str {
        "hellish rebuke"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hr", "rebuke"]
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
        bonus_action_and_slot(1)
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
        let (dmg, _) = save_for_half_damage(
            encounter,
            target_id,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 10),
            DamageType::Fire,
            "hellish rebuke",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Fire,
        })]
    }
}

pub static HELLISH_REBUKE: LazyLock<HellishRebuke> = LazyLock::new(|| HellishRebuke {});

/// Spirit Shroud — 5e level-3 necromancy / abjuration, concentration.
/// Self-buff that wreathes the caster in deathly mist: their next 10
/// rounds of melee weapon attacks deal +1d8 cold rider per hit (per the
/// OnHitRider table entry). No save, no target — purely a self-prime.
/// The actual rider lives in `attack::on_hit_riders()`.
pub struct SpiritShroud {}

impl Action for SpiritShroud {
    fn name(&self) -> &str {
        "spirit shroud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["shroud", "spirits"]
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
        bonus_action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already wreathed → don't re-cast and burn another slot.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(Condition::SpiritShrouded))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  spirit shroud: ghostly mist coils around you.".to_string());
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::SpiritShrouded,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Spirit Shroud",
                    vec![(caster_id, Condition::SpiritShrouded)],
                ),
            }),
        ]
    }
}

pub static SPIRIT_SHROUD: LazyLock<SpiritShroud> = LazyLock::new(|| SpiritShroud {});

/// Holy Aura — 5e level-8 abjuration cleric, concentration. The caster
/// and every ally inside a 30ft sphere centered on the caster receive a
/// huge defensive buff: advantage on saves + attackers vs them have
/// disadvantage. Single-shot install: every ally in range at cast time
/// picks up the condition. No re-scan per round (cheap approximation —
/// allies who walk in after the cast miss out, but the high-impact
/// half-blast-radius "everyone in the room" cleanse is preserved).
pub struct HolyAura {}

impl Action for HolyAura {
    fn name(&self) -> &str {
        "holy aura"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aura"]
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
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let center = match encounter.actors.get(&caster_id) {
            Some(c) => c.location(),
            None => return Vec::new(),
        };
        // 30ft = 12 tiles. Pick up every ally (including caster) inside.
        const RADIUS: isize = 12;
        let allies = encounter.ally_burst_targets(caster_id, center, RADIUS);
        encounter.log(format!(
            "  holy aura: {} ally{} bathed in light",
            allies.len(),
            if allies.len() == 1 { "" } else { "ies" }
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for id in allies {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::HolyAuraed,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((id, Condition::HolyAuraed));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Holy Aura", applied),
        }));
        effects
    }
}

pub static HOLY_AURA: LazyLock<HolyAura> = LazyLock::new(|| HolyAura {});

/// Foresight — 5e level-9 divination, concentration. Target ally gets
/// the mightiest single-target buff in the SRD: advantage on every
/// attack roll, save, and ability check; attackers vs them have
/// disadvantage. 10-round timer (8 hours RAW). Concentration-bound.
pub struct Foresight {}

impl Action for Foresight {
    fn name(&self) -> &str {
        "foresight"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — the caster lays hands on the recipient.
        Some(1)
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
        action_and_slot(9)
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
        encounter.log("  foresight: glimpse of the future settles over them.".to_string());
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Foreseen,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Foresight",
                    vec![(target_id, Condition::Foreseen)],
                ),
            }),
        ]
    }
}

pub static FORESIGHT: LazyLock<Foresight> = LazyLock::new(|| Foresight {});

/// Hail of Thorns — 5e level-1 ranger conjuration. A single-target ranged
/// attack that on hit erupts in a 5ft burst of thorns around the target,
/// dealing 1d10 piercing on a failed DEX save (half on save) to every
/// other creature within reach of the target. We approximate as: pick
/// a target tile, every actor within 1-tile gap of that tile (excluding
/// the caster) makes a save against the caster's WIS-based DC.
pub struct HailOfThorns {}

impl Action for HailOfThorns {
    fn name(&self) -> &str {
        "hail of thorns"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hot", "thorns"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Long bow range — 600 ft RAW; we cap to 60 tiles for the map.
        Some(60)
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
        action_and_slot(1)
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
        let raw = encounter.roll(&Dice::new(1, 10));
        encounter.log(format!(
            "  hail of thorns: 1d10({}) piercing around target tile",
            raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Piercing,
        )
    }
}

pub static HAIL_OF_THORNS: LazyLock<HailOfThorns> = LazyLock::new(|| HailOfThorns {});

/// Animate Dead — 5e level-3 necromancy. The caster raises an undead
/// minion adjacent to themselves: a Skeleton joins the caster's team
/// and acts on its own initiative for the rest of the encounter. We
/// approximate "raise from a corpse pile" by spawning a fresh
/// SKELETON_TEMPLATE instance at a footprint-free tile next to the
/// caster (closest spawnable diagonal / orthogonal neighbor). No
/// concentration; the minion is permanent for the encounter.
pub struct AnimateDead {}

impl Action for AnimateDead {
    fn name(&self) -> &str {
        "animate dead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["raise"]
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
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need a free adjacent slot to spawn the Medium skeleton.
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;

        let team = match encounter.actors.get(&caster_id) {
            Some(c) => c.team(),
            None => return Vec::new(),
        };
        let Some(spawn) =
            encounter.find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
        else {
            return Vec::new();
        };
        match encounter.instantiate_creature(&SKELETON_TEMPLATE, spawn, team, 99) {
            Ok(new_id) => encounter.log(format!(
                "  animate dead: raises a skeleton minion at {} (actor #{})",
                spawn, new_id
            )),
            Err(e) => encounter.log(format!("  animate dead failed: {}", e)),
        }
        Vec::new()
    }
}

pub static ANIMATE_DEAD: LazyLock<AnimateDead> = LazyLock::new(|| AnimateDead {});

/// Confusion — 5e level-4 enchantment, concentration, action. Targets a
/// 20ft burst (radius 4) at a point within 90ft (36 tiles). Every
/// creature in the area makes a WIS save vs the caster's spell DC; on
/// fail, they're Confused for up to 10 rounds (1 minute RAW).
///
/// Confused (collapsed from RAW's per-turn chaos table): disadvantage on
/// attack rolls (via `Condition::imposes_attacker_disadvantage`) plus the
/// holder cannot take Reactions (via `Condition::blocks_reactions`).
/// Concentration-bound on the caster — dropping the spell strips Confused
/// from every target it landed on.
pub struct Confusion {}

impl Action for Confusion {
    fn name(&self) -> &str {
        "confusion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["conf", "scramble"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
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
        action_and_slot(4)
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
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Confused,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Confused));
        }
        // Concentration installs even on a no-effect cast — RAW the chaos
        // mist hangs around for the duration regardless of saves.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Confusion", applied),
        }));
        effects
    }
}

pub static CONFUSION: LazyLock<Confusion> = LazyLock::new(|| Confusion {});

/// Fly — 5e level-3 transmutation, concentration, action. Targets one
/// willing creature within 5ft (1 tile reach). The target gains a flying
/// speed bonus (we model as +60ft, applied additively in `speed()`).
/// Concentration-bound; dropping concentration strips the `Flying`
/// condition (and the speed boost).
pub struct Fly {}

impl Action for Fly {
    fn name(&self) -> &str {
        "fly"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["levitate-flight", "wing"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — 5ft = 1 tile.
        Some(1)
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
        action_and_slot(3)
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
                condition: Condition::Flying,
                timer: ConditionTimer::Rounds(10),
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Fly",
                    vec![(target_id, Condition::Flying)],
                ),
            }),
        ]
    }
}

pub static FLY: LazyLock<Fly> = LazyLock::new(|| Fly {});

/// Levitate — 5e level-2 transmutation, concentration, action. Targets
/// one creature or object within 60ft. Target makes a CON save vs the
/// caster's spell DC; on fail, hoisted 20ft into the air and unable to
/// move horizontally. Mechanically we apply the existing `Lifted`
/// condition (zeros movement). Concentration-bound; lighter-weight than
/// Telekinesis (level-5, includes the forced-pull rider).
pub struct Levitate {}

impl Action for Levitate {
    fn name(&self) -> &str {
        "levitate"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lev", "hoist"]
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
        action_and_slot(2)
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
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Lifted,
                timer: ConditionTimer::Permanent,
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Levitate",
                    vec![(target_id, Condition::Lifted)],
                ),
            }),
        ]
    }
}

pub static LEVITATE: LazyLock<Levitate> = LazyLock::new(|| Levitate {});

/// Plant Growth — 5e level-3 transmutation, action. Targets a 20ft burst
/// (radius 4) at a point within 150ft. Every enemy creature in the area
/// is Entangled for 10 rounds — vines snare them in place, zeroing
/// movement for the duration. No save; no concentration. Allies are
/// untouched via the `enemy_burst_targets` partition.
pub struct PlantGrowth {}

impl Action for PlantGrowth {
    fn name(&self) -> &str {
        "plant growth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pg", "vines", "entangle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
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
        action_and_slot(3)
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
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Entangled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static PLANT_GROWTH: LazyLock<PlantGrowth> = LazyLock::new(|| PlantGrowth {});

/// Dominate Person — 5e level-5 enchantment, concentration, action. Targets
/// one creature within 60ft; target makes a WIS save vs the caster's spell
/// DC. On fail, target is Charmed by the caster AND Dominated (disadvantage
/// on all attacks — they hesitate, fight the compulsion) for 10 rounds.
/// The `Charmed` half blocks the target from attacking the dominator (via
/// `charmed_by`); the `Dominated` half folds into
/// `Condition::imposes_attacker_disadvantage`. Concentration-bound on the
/// caster — drop concentration to free the target.
pub struct DominatePerson {}

impl Action for DominatePerson {
    fn name(&self) -> &str {
        "dominate person"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dom", "dominate"]
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
        action_and_slot(5)
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
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Dominated,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(SetCharmedBy {
                target_id,
                charmer: Some(caster_id),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Dominate Person",
                    vec![
                        (target_id, Condition::Charmed),
                        (target_id, Condition::Dominated),
                    ],
                ),
            }),
        ]
    }
}

pub static DOMINATE_PERSON: LazyLock<DominatePerson> = LazyLock::new(|| DominatePerson {});

/// Magic Stone — 5e druid / artificer cantrip. The caster blesses up to
/// three pebbles; flinging one is a ranged spell attack (60ft) with the
/// caster's spellcasting mod, 1d6 + mod bludgeoning on hit. We collapse
/// the "three charges over the day" RAW into a per-cast 1-pebble attack
/// — at cantrip cadence the action-economy gate matters more than the
/// charge pool, and the 60ft range + spell-attack treatment is the
/// load-bearing piece (it bypasses ranged-in-melee disadvantage RAW,
/// which we don't yet model). Druids/artificers cast off WIS in 5e; we
/// reuse the caster's `spell_attack_modifier(Wisdom)` so a sorcerer
/// multiclass via Druidic Warrior would still get a sensible swing
/// (CHA-flavored casters fall back via spell_attack_modifier itself).
pub struct MagicStone {}

impl Action for MagicStone {
    fn name(&self) -> &str {
        "magic stone"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "stone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
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
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        // 1d6 + WIS modifier on hit (RAW). Damage type is bludgeoning —
        // a stone, not a magical force projectile — so per-target
        // resistance / immunity to BPS applies normally.
        spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "magic stone",
            attack_bonus,
            Dice::new(1, 6),
            wis_mod,
            DamageType::Bludgeoning,
            false,
        )
    }
}

pub static MAGIC_STONE: LazyLock<MagicStone> = LazyLock::new(|| MagicStone {});

/// Heroes' Feast — 5e level-6 conjuration. A magical feast appears; every
/// ally that partakes (we model: every ally in the caster's burst at
/// cast time) gains 2d10 + 10 temp HP (we collapse the "2d10 max HP for
/// 24 hours" RAW into a flat temp-HP grant for combat duration), gains
/// immunity to Frightened (we approximate via Heroic, which already
/// includes Frightened immunity), and is healed for 2d10 HP. Caster is
/// included if they're in the burst. Slot-6 = once-per-day apex pre-
/// fight buff for the cleric / druid / paladin loadout.
pub struct HeroesFeast {}

impl Action for HeroesFeast {
    fn name(&self) -> &str {
        "heroes' feast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hf", "feast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft to the burst origin = 12 tiles.
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
        action_and_slot(6)
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
        // Roll the temp-HP grant and heal once and share across all
        // partakers per RAW's shared-feast semantics.
        let temp_pool = encounter.roll(&Dice::new(2, 10)) + 10;
        let heal_amount = encounter.roll(&Dice::new(2, 10));
        encounter.log(format!(
            "  heroes' feast: {} temp HP + {} HP heal + Heroic buff to allies in 30ft",
            temp_pool, heal_amount
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Ally-only burst — friendly fire feasts make no sense.
        let allies = encounter.ally_burst_targets(caster_id, point, 6);
        // Include the caster explicitly if they sit at the table — the
        // ally_burst_targets helper covers the caster naturally via the
        // same-team filter, but we double-include if the caster wasn't
        // in the 30ft window (they cast it on a distant pile of allies).
        let mut targets = allies.clone();
        if !targets.contains(&caster_id) {
            targets.push(caster_id);
        }
        targets.sort_unstable();
        targets.dedup();
        for id in targets {
            effects.push(Box::new(GainTempHp {
                actor_id: id,
                amount: temp_pool,
            }));
            effects.push(Box::new(Heal {
                actor_id: id,
                amount: heal_amount,
            }));
            // Heroic carries the Frightened-immunity clause already via
            // the AdjustSaveBuff lane in the existing Heroism spell. We
            // reuse the condition for symmetry; 10-round timer.
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static HEROES_FEAST: LazyLock<HeroesFeast> = LazyLock::new(|| HeroesFeast {});

/// Spike Stones — 5e level-3 druid transmutation, concentration. Stones
/// in a 20ft square sprout sharp spikes. Every enemy whose footprint
/// touches the burst is tagged with `Spiked` for 10 rounds — the per-
/// step piercing damage rider lives on `MoveActor::apply` and reads the
/// condition. Acts like a slower, larger-area Spike Growth, traded for
/// the higher slot cost. Concentration: dropping it pulls the tags.
pub struct SpikeStones {}

impl Action for SpikeStones {
    fn name(&self) -> &str {
        "spike stones"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "spike-stones"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20ft square ≈ 4-tile radius (we use Chebyshev burst so this is
        // a 9×9 region; close enough to the 4-square RAW footprint).
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the burst origin = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        // Damage lands via the per-step Spiked rider, not at cast time.
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
        action_and_slot(4)
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
        // Enemy-only burst — we don't want allies stepping into the
        // spike field to also bleed (RAW: it's terrain that hits
        // anyone, but the AI's targeting works better as enemy-only).
        let enemies = encounter.enemy_burst_targets(caster_id, point, 4);
        if enemies.is_empty() {
            encounter.log("  spike stones: no enemies in the area".to_string());
        } else {
            encounter.log(format!(
                "  spike stones: {} enemies tagged with spiked",
                enemies.len()
            ));
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut concentration_targets: Vec<(usize, Condition)> = Vec::new();
        for id in enemies {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Spiked,
                timer: ConditionTimer::Rounds(10),
            }));
            concentration_targets.push((id, Condition::Spiked));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Spike Stones",
                concentration_targets,
            ),
        }));
        effects
    }
}

pub static SPIKE_STONES: LazyLock<SpikeStones> = LazyLock::new(|| SpikeStones {});

/// Holy Word — 5e level-7 cleric evocation, 30ft burst centered on the
/// caster. Every enemy in the area makes a CHA save against the cleric's
/// WIS-based DC: on fail, take 5d10 radiant. The save outcome also
/// determines a rider keyed to the target's *remaining* HP after the
/// damage lands (we approximate by reading the pre-damage HP — the engine
/// queues all side effects in one batch so a "post-damage" read would
/// require splitting into two queues, which the rest of the codebase
/// avoids):
/// - HP ≤ 50  → Stunned for 1 round  (the lockdown rider)
/// - HP ≤ 75  → Blinded for 1 round
/// - HP ≤ 100 → Deafened for 1 round
/// - HP > 100 → damage only
///
/// Allies are spared per RAW (the spell explicitly targets enemies).
/// Concentration-free.
pub struct HolyWord {}

impl Action for HolyWord {
    fn name(&self) -> &str {
        "holy word"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hwd", "holy"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        action_and_slot(7)
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
        let damage = encounter.roll(&Dice::new(5, 10));
        encounter.log(format!(
            "  holy word: 5d10({}) = {} radiant to enemies in 30ft",
            damage, damage
        ));
        // Enemy-only burst centered on the caster (radius 6 ≈ 30ft).
        let enemies = encounter.enemy_burst_targets(caster_id, caster_loc, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in enemies {
            let save = encounter.roll_save(tid, AbilityScoreType::Charisma, dc);
            if save.passed() {
                continue;
            }
            // Capture HP *before* the damage queues — used to pick the rider
            // tier per RAW's "if it has X or fewer HP" gate.
            let hp = encounter
                .actors
                .get(&tid)
                .map(|a| a.hitpoints())
                .unwrap_or(0);
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: damage,
                damage_type: DamageType::Radiant,
            }));
            let (cond, label) = if hp <= 50 {
                (Condition::Stunned, "stunned")
            } else if hp <= 75 {
                (Condition::Blinded, "blinded")
            } else if hp <= 100 {
                (Condition::Deafened, "deafened")
            } else {
                continue;
            };
            encounter.log(format!(
                "  holy word: target at {} HP is {}",
                hp, label
            ));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: cond,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static HOLY_WORD: LazyLock<HolyWord> = LazyLock::new(|| HolyWord {});

/// Prismatic Spray — 5e level-7 evocation. 60ft cone (radius-6 burst on
/// the target point). Each enemy in the area rolls 1d8 to determine
/// which colored ray strikes them; the ray's damage type is fixed by the
/// roll. Then they make a DEX save against the caster's INT-based DC:
/// on fail, take 10d6 of the rolled type; on save, half. The 8th color
/// (white/multi) deals all rolled types together — we collapse the rare
/// "force + blinded" rider into the 7-roll table and skip the reroll
/// branch for simplicity.
///   roll → damage type:
///     1 → Fire, 2 → Acid, 3 → Lightning, 4 → Poison,
///     5 → Cold, 6 → Force, 7 → Radiant (also Blinded for 1 round),
///     8 → Necrotic (the indigo ray)
/// Each target gets its own ray roll — RAW lets each pick a different
/// color, and this matches the chaos of the spell. Enemy-only filter:
/// the caster controls the cone aim, so allies in the burst are spared.
pub struct PrismaticSpray {}

impl Action for PrismaticSpray {
    fn name(&self) -> &str {
        "prismatic spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "prismatic"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft cone — the burst origin sits at the cone's far edge.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Random per target — list the full envelope so resistance hints
        // in the UI surface every possibility.
        vec![
            DamageType::Fire,
            DamageType::Acid,
            DamageType::Lightning,
            DamageType::Poison,
            DamageType::Cold,
            DamageType::Force,
            DamageType::Radiant,
            DamageType::Necrotic,
        ]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
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
        encounter.log("  prismatic spray: a rainbow burst erupts");
        let enemies = encounter.enemy_burst_targets(caster_id, point, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in enemies {
            let ray = encounter.roll(&Dice::new(1, 8));
            let (dtype, name, with_blind) = match ray {
                1 => (DamageType::Fire, "red", false),
                2 => (DamageType::Acid, "orange", false),
                3 => (DamageType::Lightning, "yellow", false),
                4 => (DamageType::Poison, "green", false),
                5 => (DamageType::Cold, "blue", false),
                6 => (DamageType::Force, "violet", false),
                7 => (DamageType::Radiant, "white", true),
                _ => (DamageType::Necrotic, "indigo", false),
            };
            let raw = encounter.roll(&Dice::new(10, 6));
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            encounter.log(format!(
                "  prismatic spray: 1d8({}) {} ray \u{2014} 10d6({}) {:?} ({}{})",
                ray,
                name,
                raw,
                dtype,
                dmg,
                if save.passed() { ", saved" } else { "" }
            ));
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: dtype,
                }));
            }
            // The white ray also leaves the target Blinded for 1 round on
            // a failed save (the radiant flash sears their eyes).
            if with_blind && !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static PRISMATIC_SPRAY: LazyLock<PrismaticSpray> = LazyLock::new(|| PrismaticSpray {});

/// Feeblemind — 5e level-8 enchantment. Single-target INT save against
/// the caster's INT-based spell DC. On a failed save, the target takes
/// 4d6 psychic damage and is Feebled (our new condition): their INT and
/// CHA effectively drop to 1, modeled as blanket disadvantage on attack
/// rolls (the target can barely focus), disadvantage on INT/WIS/CHA
/// saves, and they can't cast spells. The condition lasts 10 rounds
/// (RAW: permanent until Greater Restoration / Heal / etc; we cap to a
/// duration the engine can resolve before the encounter ends). Greater
/// Restoration explicitly removes Feebled.
pub struct Feeblemind {}

impl Action for Feeblemind {
    fn name(&self) -> &str {
        "feeblemind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fm", "feeble"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles RAW — capped at 40 to fit common map widths.
        Some(40)
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
        action_and_slot(8)
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
        let damage = encounter.roll(&Dice::new(4, 6));
        // Per RAW the damage lands regardless of the save; only the
        // mind-shatter rider gates on the save outcome.
        encounter.log(format!(
            "  feeblemind: 4d6({}) = {} psychic",
            damage, damage
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Psychic,
        })];
        let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, dc);
        if !save.passed() {
            encounter.log("  feeblemind: target's mind shatters");
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Feebled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static FEEBLEMIND: LazyLock<Feeblemind> = LazyLock::new(|| Feeblemind {});

/// Otto's Irresistible Dance — 5e level-6 enchantment, concentration,
/// action. Single-target WIS save against the caster's spell DC. On a
/// failed save, the target dances helplessly: zero movement, attack
/// disadvantage, auto-fail DEX saves, attackers get advantage. RAW
/// allows the target to spend an Action each turn to reattempt the
/// save; we collapse to a duration-bound install (10 rounds). Cleared
/// on concentration drop. No damage — pure control.
pub struct OttosIrresistibleDance {}

impl Action for OttosIrresistibleDance {
    fn name(&self) -> &str {
        "otto's irresistible dance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dance", "ottos", "irresistible dance"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles.
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
        action_and_slot(6)
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
        // Bard / wizard / sorcerer-list spell — pick the higher of the
        // caster's mental abilities for the DC.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            encounter.log("  otto's dance: target resists the compulsion");
            return Vec::new();
        }
        encounter.log("  otto's dance: target capers helplessly");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Dancing,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Otto's Irresistible Dance",
                    vec![(target_id, Condition::Dancing)],
                ),
            }),
        ]
    }
}

pub static OTTOS_IRRESISTIBLE_DANCE: LazyLock<OttosIrresistibleDance> =
    LazyLock::new(|| OttosIrresistibleDance {});

/// Maze — 5e level-8 conjuration, concentration, action. Single-target
/// banishment with no save (RAW gives the target an INT check each turn
/// to escape — we collapse to a duration-bound install). The target is
/// removed from the encounter for up to 10 rounds (1 minute RAW) — we
/// model with the `Mazed` condition which blocks all action economy +
/// movement, leaving the actor inert on the map. Concentration-bound.
pub struct Maze {}

impl Action for Maze {
    fn name(&self) -> &str {
        "maze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["banish"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
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
        action_and_slot(8)
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
        encounter.log("  maze: target vanishes into a labyrinthine demiplane");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Mazed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Maze",
                    vec![(target_id, Condition::Mazed)],
                ),
            }),
        ]
    }
}

pub static MAZE: LazyLock<Maze> = LazyLock::new(|| Maze {});

/// Fire Storm — 5e level-7 evocation, action. 20ft-radius sphere
/// centered on a point within 150ft (40 tiles). Every creature in the
/// burst makes a DEX save against the caster's spell DC (WIS for
/// druid/cleric, INT for wizard — we pick the caster's higher one);
/// on fail they take 7d10 fire, half on save. Allies in the radius are
/// spared (caster picks the silhouette of the storm per RAW) — we use
/// the standard `enemy_burst_targets` partition.
pub struct FireStorm {}

impl Action for FireStorm {
    fn name(&self) -> &str {
        "fire storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["firestorm", "storm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(40)
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
        action_and_slot(7)
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
        // Cleric / Druid → WIS, Wizard → INT. Pick the larger.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
        ]);
        // 5e RAW lets the caster shape the storm as ten contiguous 10ft
        // cubes — players use it to skirt allies. We approximate with
        // the enemy-only burst partition so allies in the radius are
        // spared (matches the load-bearing "caster chooses the
        // silhouette" intent of the spell).
        let (effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(7, 10),
            DamageType::Fire,
            "fire storm",
        );
        effects
    }
}

pub static FIRE_STORM: LazyLock<FireStorm> = LazyLock::new(|| FireStorm {});

/// Eyebite — 5e level-6 necromancy, concentration, action. Single-target
/// WIS save against the caster's spell DC. RAW offers three eye options
/// (asleep / panicked / sickened); we pick `asleep` as the load-bearing
/// flavor since it's the strongest control. On fail the target falls
/// Asleep for up to 10 rounds (1 minute RAW). Sleep is woken by damage
/// per the engine's existing damage-on-Asleep hook, so the spell still
/// gives the target an escape. Concentration-bound on the caster.
pub struct Eyebite {}

impl Action for Eyebite {
    fn name(&self) -> &str {
        "eyebite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["evil eye"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
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
        action_and_slot(6)
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
        // Eyebite is on the bard / sorcerer / warlock / wizard list — pick
        // the caster's higher mental ability for the DC.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            encounter.log("  eyebite: target shakes off the evil eye");
            return Vec::new();
        }
        encounter.log("  eyebite: target collapses into a magical slumber");
        // Two parallel conditions: Asleep (load-bearing mechanics —
        // action-economy block + melee-crit-on-hit) and EyebittenSick
        // (concentration mark; only used to link the spell to the
        // target for cleanup on drop).
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::EyebittenSick,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Eyebite",
                    vec![(target_id, Condition::Asleep), (target_id, Condition::EyebittenSick)],
                ),
            }),
        ]
    }
}

pub static EYEBITE: LazyLock<Eyebite> = LazyLock::new(|| Eyebite {});

/// Conjure Animals — 5e level-3 conjuration, concentration, action.
/// Summons two CR-1/4 wolves on adjacent tiles to the caster, joining
/// the caster's team. We collapse the RAW "1 CR-2 / 2 CR-1 / 4 CR-1/2
/// / 8 CR-1/4" option table to the 2-wolf branch since it's the load-
/// bearing flavor for a level-3 cast and our wolf is already on the
/// books. Each conjured wolf gets the Conjured condition so dropping
/// concentration prunes them via the engine's cleanup hook.
/// Concentration-bound on the caster.
pub struct ConjureAnimals {}

impl Action for ConjureAnimals {
    fn name(&self) -> &str {
        "conjure animals"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["conjure", "summon"]
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
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need at least one free Medium slot adjacent to the caster —
        // the second wolf is best-effort (the spell still resolves with
        // one conjured ally if only one slot is available).
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::wolves::WOLF_TEMPLATE;

        let team = match encounter.actors.get(&caster_id) {
            Some(c) => c.team(),
            None => return Vec::new(),
        };
        // Spawn up to two wolves on free adjacent slots — find one,
        // spawn it (it occupies its slot), then look for the next slot.
        let mut spawned: Vec<usize> = Vec::new();
        for _ in 0..2 {
            let Some(anchor) = encounter
                .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 3)
            else {
                break;
            };
            match encounter.instantiate_creature(&WOLF_TEMPLATE, anchor, team, 90 + spawned.len()) {
                Ok(new_id) => {
                    encounter.log(format!(
                        "  conjure animals: a spectral wolf appears at {} (actor #{})",
                        anchor, new_id
                    ));
                    spawned.push(new_id);
                }
                Err(e) => {
                    encounter.log(format!("  conjure animals failed: {}", e));
                    break;
                }
            }
        }
        if spawned.is_empty() {
            return Vec::new();
        }
        // Tag each wolf with the Conjured condition so the engine's
        // concentration drop can prune them, then start concentration.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tags = Vec::new();
        for id in &spawned {
            effects.push(Box::new(ApplyCondition {
                actor_id: *id,
                condition: Condition::Conjured,
                timer: ConditionTimer::Rounds(100),
            }));
            tags.push((*id, Condition::Conjured));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Conjure Animals", tags),
        }));
        effects
    }
}

pub static CONJURE_ANIMALS: LazyLock<ConjureAnimals> = LazyLock::new(|| ConjureAnimals {});

/// Power Word Pain — 5e level-7 enchantment, action. Single target;
/// no save, no attack roll — but the spell only takes effect if the
/// target has 100 HP or fewer at cast time. RAW: target is racked with
/// pain that imposes Slowed-equivalent effects (speed reduced, can't
/// take reactions, takes disadvantage on attacks and CON saves to
/// maintain concentration). We approximate via the existing `Slowed`
/// condition (already wires up speed multiplier, DEX-save disadvantage,
/// and the AC penalty) for 10 rounds — long enough for the encounter
/// without needing a per-turn "CON save to end" loop in the engine.
///
/// HP-threshold spells are rare in the engine — most spells gate on
/// save or HP-percent rather than absolute HP. Power Word Pain is the
/// canonical example so we keep the threshold literal (≤100 HP) and
/// log the gate explicitly so a play-through can see why a Tarrasque
/// shrugs it off and a level-3 fighter doesn't.
pub struct PowerWordPain {}

impl Action for PowerWordPain {
    fn name(&self) -> &str {
        "power word pain"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwp", "pain"]
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let hp = target.hitpoints();
        if hp > 100 {
            encounter.log(format!(
                "  power word pain: target has {} HP (>100), shrugged off",
                hp
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  power word pain: target has {} HP, racked with pain",
            hp
        ));
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Slowed,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static POWER_WORD_PAIN: LazyLock<PowerWordPain> = LazyLock::new(|| PowerWordPain {});

/// Mass Polymorph — 5e level-9 transmutation, concentration, action.
/// Burst variant of Polymorph: every enemy whose footprint touches a
/// 30ft radius around the chosen point makes a WIS save against the
/// caster's spell save DC. On fail, the target is Polymorphed (loses
/// its action economy, AC/speed default, etc., per the condition's
/// existing wiring) and gains 30 temp HP (the beast pool). Allies in
/// the radius are spared — friendly polymorphs are RAW willing, but
/// the AI would auto-fail-save them which makes the burst variant
/// strictly hostile in practice. Concentration drops all polymorphs at
/// once.
pub struct MassPolymorph {}

impl Action for MassPolymorph {
    fn name(&self) -> &str {
        "mass polymorph"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mpoly", "mass morph"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft to burst origin.
        Some(48)
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
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(origin) = target_locations.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let targets = encounter.enemy_burst_targets(caster_id, origin, 6);
        if targets.is_empty() {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tagged: Vec<(usize, Condition)> = Vec::new();
        for target_id in targets {
            let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                encounter.log(format!(
                    "  mass polymorph: actor #{} resists the transformation",
                    target_id
                ));
                continue;
            }
            encounter.log(format!(
                "  mass polymorph: actor #{} morphs into a beast",
                target_id
            ));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Permanent,
            }));
            effects.push(Box::new(GainTempHp {
                actor_id: target_id,
                amount: 30,
            }));
            tagged.push((target_id, Condition::Polymorphed));
        }
        if tagged.is_empty() {
            return effects;
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Mass Polymorph", tagged),
        }));
        effects
    }
}

pub static MASS_POLYMORPH: LazyLock<MassPolymorph> = LazyLock::new(|| MassPolymorph {});

/// Mordenkainen's Sword — 5e level-7 evocation, concentration, action.
/// RAW: a sword of force appears within range; on cast and then each
/// subsequent turn as a bonus action you can swing it for 3d10 force
/// damage on hit. The "bonus-action recurring attack" pattern is
/// awkward in this engine's one-action-per-spell model, so we collapse
/// to a heavier on-cast hit (5d10 force, melee spell attack) plus a
/// concentration mark that the AI can drop and re-cast — close enough
/// in damage budget to one cast + ~3-4 sustained sword swings RAW.
/// The mark also primes the Slowed condition on a hit (force is
/// gravitically dense in 5e flavor — RAW Mordenkainen's "sword" cuts
/// motion as well as flesh). Single-target.
pub struct MordenkainensSword {}

impl Action for MordenkainensSword {
    fn name(&self) -> &str {
        "mordenkainen's sword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mord", "force sword", "sword"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the conjure point + 5ft reach for the sword itself —
        // we collapse to a flat 24-tile spell range.
        Some(24)
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
        action_and_slot(7)
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
        // Spell uses the caster's best mental ability — wizards (INT),
        // sorcerers (CHA), and warlocks (CHA) all get Mordenkainen's
        // Sword on their published lists.
        let attack_bonus = caster
            .spell_attack_modifier(AbilityScoreType::Intelligence)
            .max(caster.spell_attack_modifier(AbilityScoreType::Charisma));
        // 5d10 force on hit — a melee spell attack, so reach + footprint
        // adjacency apply via resolve_attack's melee path.
        let mut effects = spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "mordenkainen's sword",
            attack_bonus,
            Dice::new(5, 10),
            0,
            DamageType::Force,
            true,
        );
        // Concentration mark — drops on damage / next concentration cast.
        // We always install regardless of hit/miss (RAW: the sword
        // persists for the duration even if the first swing whiffs).
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Mordenkainen's Sword"),
        }));
        effects
    }
}

pub static MORDENKAINENS_SWORD: LazyLock<MordenkainensSword> =
    LazyLock::new(|| MordenkainensSword {});

/// Frostbite — 5e cantrip, evocation. Single target, CON save vs the
/// caster's spell save DC. On fail: 1d6 cold damage and the target has
/// disadvantage on its next weapon attack (we approximate via Slowed
/// for a 1-round timer — Slowed already wires up DEX-save disadvantage
/// and the AC penalty, close enough flavor for the cantrip). On
/// success: nothing. Caster's best of INT / WIS / CHA save DC, since
/// druid / sorcerer / wizard / warlock all share the cantrip.
pub struct Frostbite {}

impl Action for Frostbite {
    fn name(&self) -> &str {
        "frostbite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["frost", "fb"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if save.passed() {
            encounter.log("  frostbite: target shrugs off the chill");
            return Vec::new();
        }
        let raw = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!("  frostbite: 1d6({}) = {} cold", raw, raw));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Cold,
            }),
            // Single-round disadvantage on the next attack — Slowed
            // already encodes the AC penalty + DEX save disadvantage.
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Slowed,
                timer: ConditionTimer::Rounds(1),
            }),
        ]
    }
}

pub static FROSTBITE: LazyLock<Frostbite> = LazyLock::new(|| Frostbite {});

/// Negative Energy Flood — 5e level-5 necromancy, action. Single target;
/// CON save vs the caster's spell save DC. On fail: 5d12 necrotic
/// damage. On success: half. The signature flavor is the "rises as a
/// zombie if the target drops" clause — we don't model post-death
/// raise here (the engine's Animate Dead spell covers that path), but
/// the headline 5d12 burst lands either way. Force-resistant /
/// necrotic-immune actors (vampires, wights, etc.) shrug damage off
/// per the engine's resistance table.
pub struct NegativeEnergyFlood {}

impl Action for NegativeEnergyFlood {
    fn name(&self) -> &str {
        "negative energy flood"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nef", "negflood"]
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
        action_and_slot(5)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (dmg, _) = save_for_half_damage(
            encounter,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(5, 12),
            DamageType::Necrotic,
            "negative energy flood",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static NEGATIVE_ENERGY_FLOOD: LazyLock<NegativeEnergyFlood> =
    LazyLock::new(|| NegativeEnergyFlood {});

/// Armor of Agathys — 5e level-1 abjuration (Warlock signature). Self-only
/// buff: caster gains 5 temp HP and any creature that hits them with a
/// melee attack takes 5 cold damage in retaliation. The temp HP IS the
/// shield — once the pool is drained, the retaliation rider drops with
/// it (handled in `DealDamage::apply` — when temp HP is exhausted and
/// `AgathysShielded` is up, the condition is stripped so subsequent
/// melee hits don't free-trigger off a depleted shield).
///
/// We don't scale by slot level (RAW: +5 temp HP and +5 cold per slot
/// level above 1). The single-level baseline keeps the side-effect path
/// flat and the AI heuristic ("am I about to be swarmed?") legible.
/// Concentration-free; flat Rounds timer.
pub struct ArmorOfAgathys {}

impl Action for ArmorOfAgathys {
    fn name(&self) -> &str {
        "armor of agathys"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["agathys", "aoa"]
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
        action_and_slot(1)
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
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: 5,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::AgathysShielded,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static ARMOR_OF_AGATHYS: LazyLock<ArmorOfAgathys> = LazyLock::new(|| ArmorOfAgathys {});

/// Sickening Radiance — 5e level-4 evocation, concentration. RAW: a 30ft
/// sphere of dim radiant light persists for the spell's duration; every
/// creature inside that fails a CON save each round takes 4d10 radiant
/// and gains a level of exhaustion. We collapse the sustained zone into
/// a one-shot burst at cast time: every enemy in the 30ft radius rolls
/// CON; on fail they eat the full 4d10 radiant AND gain `Exhausted`
/// (engine's single-tier exhaustion). The concentration mark holds so
/// dropping it can prune the exhaustion later if the AI swaps focus.
/// Excludes allies (typical 5e gotcha — RAW hits everyone in the zone,
/// but enemy-only is the load-bearing tactical use). Damage and save are
/// rolled per-target (independent CON saves per RAW); the `Exhausted`
/// install is paired with the `SickeningRadiated` marker for the
/// concentration cleanup hook.
pub struct SickeningRadiance {}

impl Action for SickeningRadiance {
    fn name(&self) -> &str {
        "sickening radiance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "sickening"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft range to the burst origin.
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
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(center) = target_locations.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        // 30ft = 6 tile gap. Enemy-only partition matches the burst's
        // tactical use; allies caught in the zone are spared per
        // standard engine convention.
        let targets = encounter.enemy_burst_targets(caster_id, center, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in targets {
            let raw = encounter.roll(&Dice::new(4, 10));
            let save = encounter.roll_save(tid, AbilityScoreType::Constitution, dc);
            encounter.log(format!(
                "  sickening radiance: 4d10({}) radiant ({})",
                raw,
                if save.passed() { "save" } else { "fail" }
            ));
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Radiant,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Exhausted,
                timer: ConditionTimer::Rounds(10),
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::SickeningRadiated,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::Exhausted));
            conditions.push((tid, Condition::SickeningRadiated));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Sickening Radiance", conditions),
        }));
        effects
    }
}

pub static SICKENING_RADIANCE: LazyLock<SickeningRadiance> =
    LazyLock::new(|| SickeningRadiance {});

/// Bigby's Hand — level-5 evocation, concentration. The caster summons a
/// fist-sized spectral force-hand that follows them around mauling
/// targets. RAW exposes four activation modes (clenched fist, grasping
/// hand, forceful hand, interposing hand); we collapse to the headline
/// "+force damage on every attack" envelope — a persistent +1d10 force
/// rider on every weapon swing the caster lands (slots into the
/// OnHitRider table next to Crown of Stars). Self-buff, no targeting.
/// Concentration so re-casting drops cleanly.
pub struct BigbysHand {}

impl Action for BigbysHand {
    fn name(&self) -> &str {
        "bigby's hand"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bh", "bigby", "hand"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Damage lands via the per-hit rider, not directly on cast.
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
        action_and_slot(5)
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
                condition: Condition::BigbysHanded,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Bigby's Hand",
                    vec![(caster_id, Condition::BigbysHanded)],
                ),
            }),
        ]
    }
}

pub static BIGBYS_HAND: LazyLock<BigbysHand> = LazyLock::new(|| BigbysHand {});

/// Tenser's Transformation — level-6 transmutation, concentration. The
/// caster channels arcane force into their body: they gain 50 temp HP
/// (the explicit RAW "temporary hit points" lane) AND advantage on
/// weapon attack rolls for the duration (the `Transformed` condition
/// joins `grants_self_attack_advantage`). RAW additionally gives +2d12
/// force damage on weapon hits and proficiency in all weapons / CON
/// saves; we skip those clauses since they'd need per-class weapon
/// bookkeeping. The 50 temp HP + advantage envelope is load-bearing
/// enough to make the spell shine. Self-only.
pub struct TensersTransformation {}

impl Action for TensersTransformation {
    fn name(&self) -> &str {
        "tenser's transformation"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tt", "tenser", "transformation"]
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
        action_and_slot(6)
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
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: 50,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Transformed,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Tenser's Transformation",
                    vec![(caster_id, Condition::Transformed)],
                ),
            }),
        ]
    }
}

pub static TENSERS_TRANSFORMATION: LazyLock<TensersTransformation> =
    LazyLock::new(|| TensersTransformation {});

/// Aura of Life — level-4 paladin abjuration, concentration. The paladin
/// emits a 30-foot aura (6-tile radius); the caster and every ally
/// inside the aura gain the DeathWarded condition for the duration —
/// the next hit that would drop them to 0 HP instead leaves them at 1.
/// We collapse the RAW "max-HP restoration if reduced to 0" half into
/// the existing DeathWard mechanic so the aura plays well with the
/// engine's killing-blow interception path. Concentration-bound on the
/// caster; the aura's ally-only partition uses `ally_burst_targets`.
pub struct AuraOfLife {}

impl Action for AuraOfLife {
    fn name(&self) -> &str {
        "aura of life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aol", "aura-life"]
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
        // The aura grants death-ward protection — not strictly a heal,
        // but the AI's support pipeline should consider it alongside
        // healing actions when the party is low.
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
        action_and_slot(4)
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
        let center = caster.location();
        const RADIUS: isize = 6;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        // Allies (including the caster) in radius receive DeathWarded.
        // `ally_burst_targets` includes the caster if they sit in the
        // burst — and they always do, since the aura is centered on
        // them.
        for tid in encounter.ally_burst_targets(caster_id, center, RADIUS) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::DeathWarded,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::DeathWarded));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Aura of Life", conditions),
        }));
        effects
    }
}

pub static AURA_OF_LIFE: LazyLock<AuraOfLife> = LazyLock::new(|| AuraOfLife {});

/// Aganazzar's Scorcher — level-2 evocation. Roaring flames erupt in a
/// 30ft line (RAW); we approximate as a 3-tile burst at the target
/// point. Every creature in the area makes a DEX save vs the caster's
/// spell DC: fail = full 3d8 fire damage, pass = half. Shared damage
/// roll across all victims. Enemy-only partition keeps allies safe
/// inside the line — same simplification as Burning Hands / Cone of
/// Cold use. No concentration.
pub struct AganazzarsScorcher {}

impl Action for AganazzarsScorcher {
    fn name(&self) -> &str {
        "aganazzar's scorcher"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["scorcher", "aganazzar"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW line — origin must be close to caster.
        Some(12)
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
        action_and_slot(2)
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
        let (effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 8),
            DamageType::Fire,
            "aganazzar's scorcher",
        );
        effects
    }
}

pub static AGANAZZARS_SCORCHER: LazyLock<AganazzarsScorcher> =
    LazyLock::new(|| AganazzarsScorcher {});

/// Acid Arrow — level-2 evocation. Ranged spell attack against one target:
/// 4d4 acid on hit, half damage on miss. Per RAW the spell also splashes
/// 2d4 acid "at the end of its next turn" on hit — we collapse that into
/// a single combined damage roll at cast time (4d4 immediate + 2d4
/// follow-up) so the engine doesn't need a per-actor deferred-damage
/// queue. Miss still drops the splash (consistent with the engine's
/// "miss does nothing but core damage" model).
///
/// Spellcasting ability defaults to INT (wizard primary); sorcerer
/// multiclass would CHA, but acid arrow lives on the wizard/sorcerer
/// list and INT is the safe default for both.
pub struct AcidArrow {}

impl Action for AcidArrow {
    fn name(&self) -> &str {
        "acid arrow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["arrow", "aa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft RAW = 36 tiles.
        Some(36)
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
        action_and_slot(2)
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
        // Combined 4d4 immediate + 2d4 splash → 6d4 on hit (5e RAW: full
        // immediate + full splash). Half damage on miss only covers the
        // splash dice per RAW: "half damage on a miss and no splash" →
        // 2d4 on miss. We model the miss-half as a manual fallback below
        // because `spell_attack` discards the miss path entirely.
        let (effects, dmg) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "acid arrow",
            attack_bonus,
            Dice::new(4, 4),
            0,
            DamageType::Acid,
            false,
        );
        if dmg > 0 {
            // Hit: append the 2d4 splash. Logged separately so the
            // breakdown stays legible.
            let splash = encounter.roll(&Dice::new(2, 4));
            encounter.log(format!("  acid arrow splash: 2d4({}) acid", splash));
            let mut all = effects;
            all.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: splash,
                damage_type: DamageType::Acid,
            }));
            return all;
        }
        // Miss: the splash still drips for half damage per RAW.
        let half_splash = encounter.roll(&Dice::new(2, 4)) / 2;
        if half_splash == 0 {
            return effects;
        }
        encounter.log(format!(
            "  acid arrow miss splash: 2d4/2 = {} acid",
            half_splash
        ));
        let mut all = effects;
        all.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: half_splash,
            damage_type: DamageType::Acid,
        }));
        all
    }
}

pub static ACID_ARROW: LazyLock<AcidArrow> = LazyLock::new(|| AcidArrow {});

/// Tidal Wave — level-3 conjuration. A 30-foot cube of water crashes
/// down: every creature in the area makes a DEX save vs the caster's
/// spell DC. Fail: 4d8 bludgeoning + Prone (the wave sweeps them off
/// their feet). Pass: half damage, no prone. Allies are spared via the
/// enemy_burst_targets partition (the AI-friendliest simplification of
/// RAW's "everyone in the cube"). No concentration.
pub struct TidalWave {}

impl Action for TidalWave {
    fn name(&self) -> &str {
        "tidal wave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wave", "tw"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft cube ≈ 3-tile radius (Chebyshev).
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft to the burst origin = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(4, 8),
            DamageType::Bludgeoning,
            "tidal wave",
        );
        // Failed-save targets are knocked Prone by the breaker wave.
        for (tid, passed) in saves {
            if !passed {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                }));
            }
        }
        effects
    }
}

pub static TIDAL_WAVE: LazyLock<TidalWave> = LazyLock::new(|| TidalWave {});

/// Dawn — level-5 evocation, concentration. The caster summons a 30ft-
/// radius cylinder of sunlight. Every enemy in the area makes a CON
/// save: fail = full 4d10 radiant, pass = half. We collapse the
/// per-round sustained-cylinder RAW into a one-shot install at cast
/// time (matches our Sickening Radiance simplification). Concentration-
/// bound on the caster; dropping concentration ends the dawn.
pub struct Dawn {}

impl Action for Dawn {
    fn name(&self) -> &str {
        "dawn"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sunlight"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft radius ≈ 6-tile Chebyshev burst.
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the burst origin = 24 tiles.
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(4, 10),
            DamageType::Radiant,
            "dawn",
        );
        // Concentration mark — dropping cleans up the dawn marker.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Dawn"),
        }));
        effects
    }
}

pub static DAWN: LazyLock<Dawn> = LazyLock::new(|| Dawn {});

/// Mental Prison — level-6 illusion, concentration. Single target makes
/// an INT save vs the caster's spell DC. Fail: 5d10 psychic + the target
/// is `MentallyImprisoned` for the duration (movement zero, attacks with
/// disadvantage, attackers gain advantage — the full Restrained envelope
/// plus the illusory-prison flavor). Pass: half damage, no prison.
/// Concentration-bound on the caster. A target who fails the save can
/// try again at the end of each of their turns RAW — we collapse to a
/// duration-bound install for simplicity. The 5d10 hits psychic, so
/// psychic immunity (Mind Flayer / Death Knight) cleanly zero-ifies the
/// damage without breaking the imprison effect.
pub struct MentalPrison {}

impl Action for MentalPrison {
    fn name(&self) -> &str {
        "mental prison"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mp", "prison"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
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
        action_and_slot(6)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (dmg, passed) = save_for_half_damage(
            encounter,
            target_id,
            AbilityScoreType::Intelligence,
            dc,
            Dice::new(5, 10),
            DamageType::Psychic,
            "mental prison",
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        if !passed {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::MentallyImprisoned,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((target_id, Condition::MentallyImprisoned));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Mental Prison", conditions),
        }));
        effects
    }
}

pub static MENTAL_PRISON: LazyLock<MentalPrison> = LazyLock::new(|| MentalPrison {});

/// Investiture of Flame — level-6 transmutation, concentration. The
/// caster wreathes themselves in flames: they gain resistance to fire
/// (read by `effective_damage`'s Invested-in-Flame branch), and every
/// melee attacker takes 1d10 fire damage in retaliation (handled by the
/// `resolve_attack` reflect alongside Fire Shield / Armor of Agathys).
/// Self-only, concentration-bound; no save / no target. The 4d8 fire
/// emanation rider in RAW is omitted — the load-bearing buff is the
/// resistance + melee retaliation envelope.
pub struct InvestitureOfFlame {}

impl Action for InvestitureOfFlame {
    fn name(&self) -> &str {
        "investiture of flame"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["iof", "flameinvest"]
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
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already invested → don't re-cast and burn a level-6 slot.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(Condition::InvestedInFlame))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  investiture of flame: your body erupts in flame.".to_string());
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::InvestedInFlame,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Investiture of Flame",
                    vec![(caster_id, Condition::InvestedInFlame)],
                ),
            }),
        ]
    }
}

pub static INVESTITURE_OF_FLAME: LazyLock<InvestitureOfFlame> =
    LazyLock::new(|| InvestitureOfFlame {});

/// Grease — level-1 conjuration. A 10-foot square of slick grease coats
/// the ground at a point within 60 ft. Every enemy whose footprint
/// touches the burst makes a DEX save vs the caster's spell DC; failure
/// knocks them Prone (the difficult-terrain half of RAW is omitted — the
/// load-bearing penalty is the prone). Allies are spared via the
/// enemy_burst_targets partition (the spell is centered by the caster,
/// not a friendly-fire AoE in our model). No concentration; the slick
/// surface lasts a flat 10-round Rounds timer (1 minute RAW). The
/// caster doesn't *need* to do anything else — the prone is the entire
/// payload, matching the spell's reputation as a cheap lv1 disabler.
pub struct Grease {}

impl Action for Grease {
    fn name(&self) -> &str {
        "grease"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["slick", "slip"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft square ≈ 2-tile Chebyshev burst (the grease covers a
        // 2x2-tile patch in 2.5ft squares).
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
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
        action_and_slot(1)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        const RADIUS: isize = 2;
        encounter.log(format!("  grease: slick patch at {} (DC {})", point, dc));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static GREASE: LazyLock<Grease> = LazyLock::new(|| Grease {});

/// Flaming Sphere — level-2 conjuration, concentration. The caster
/// conjures a 5-ft-radius ball of flame at a tile within 60 ft. Every
/// enemy whose footprint touches the burst makes a DEX save vs the
/// caster's spell DC: fail = full 2d6 fire, pass = half. RAW lets the
/// sphere be re-positioned each turn as a bonus action; we collapse the
/// per-round re-roll to the cast-time install (matches our Sickening
/// Radiance / Dawn simplification). Concentration-bound so the slot is
/// committed; dropping concentration ends the sphere cleanly.
pub struct FlamingSphere {}

impl Action for FlamingSphere {
    fn name(&self) -> &str {
        "flaming sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sphere", "fs-spell", "flameball"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft-radius sphere ≈ 1-tile burst.
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
        action_and_slot(2)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 6),
            DamageType::Fire,
            "flaming sphere",
        );
        // Concentration mark — dropping cleans up the sphere marker.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Flaming Sphere"),
        }));
        effects
    }
}

pub static FLAMING_SPHERE: LazyLock<FlamingSphere> = LazyLock::new(|| FlamingSphere {});

/// Guardian of Faith — level-4 conjuration. A spectral Large guardian
/// appears at a tile within 30 ft; any enemy that enters its 10-foot
/// reach takes 20 radiant damage (RAW: fiends / undead take 20, others
/// take 10 — we collapse to the high tier since our pool is
/// fiend / undead -heavy and the load-bearing flavor is "the guardian
/// punishes intruders"). DEX save vs the caster's spell DC halves the
/// damage. The guardian has a 60-HP / 8-hour budget in RAW; we model the
/// cast as a one-shot burst rather than a sustained presence. No
/// concentration in RAW.
pub struct GuardianOfFaith {}

impl Action for GuardianOfFaith {
    fn name(&self) -> &str {
        "guardian of faith"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["guardian", "gof"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft reach ≈ 2-tile Chebyshev burst.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        // Flat 20 radiant on fail, 10 on save — no dice roll per RAW.
        encounter.log(format!(
            "  guardian of faith: 20 radiant at {} (DC {})",
            point, dc
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, 2) {
            let save = encounter.roll_save(tid, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { 10 } else { 20 };
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: dmg,
                damage_type: DamageType::Radiant,
            }));
        }
        effects
    }
}

pub static GUARDIAN_OF_FAITH: LazyLock<GuardianOfFaith> = LazyLock::new(|| GuardianOfFaith {});

/// Blade Barrier — level-6 evocation, concentration. A vertical wall of
/// whirling, razor-sharp blades springs into existence at a tile within
/// 90 ft. Every enemy whose footprint touches the burst makes a DEX save
/// vs the caster's spell DC: fail = full 6d10 slashing, pass = half. The
/// wall lingers (10 minutes RAW) — we collapse to the cast-time install
/// and use concentration as the sustainment anchor. Allies are spared via
/// the enemy_burst_targets partition (the wall is a vertical surface; in
/// RAW the caster chooses its orientation so allies stand on the safe
/// side).
pub struct BladeBarrier {}

impl Action for BladeBarrier {
    fn name(&self) -> &str {
        "blade barrier"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bb", "blades", "barrier"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 100ft long, 20ft high wall ≈ 4-tile Chebyshev burst (we treat
        // the wall as a wide damage zone rather than a literal line so
        // the cast picker has a single tile to aim at).
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
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
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(6, 10),
            DamageType::Slashing,
            "blade barrier",
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Blade Barrier"),
        }));
        effects
    }
}

pub static BLADE_BARRIER: LazyLock<BladeBarrier> = LazyLock::new(|| BladeBarrier {});
