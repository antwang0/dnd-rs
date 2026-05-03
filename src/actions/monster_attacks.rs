use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplyCondition, DealDamage, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// Optional save-then-condition on-hit rider for `WeaponAction`. When the
/// underlying attack lands, the target rolls `ability` save vs `dc`; on
/// fail, `condition` is applied with `timer`. Used by Trip and Wolf Bite.
#[derive(Clone, Copy)]
pub struct SaveRider {
    pub ability: AbilityScoreType,
    pub dc: i32,
    pub condition: Condition,
    pub timer: ConditionTimer,
}

/// Generic single-target weapon attack. Replaces the per-weapon Action
/// impls (Slam, Scimitar, Greatclub, Shortbow, Longbow, Trip, Wolf Bite)
/// with a single configuration-driven type. Variations between weapons
/// reduce to data: damage dice / type, reach, melee vs ranged, ability
/// (STR / DEX) for to-hit and damage modifier, action-economy cost, and
/// an optional save-then-condition rider.
pub struct WeaponAction {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub reach: isize,
    pub requires_los: bool,
    pub cost_resource: Resource,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    /// Drives both the to-hit modifier and (when this is the same ability
    /// the weapon would use for damage) the damage modifier. Today every
    /// 5e weapon we model uses one ability for both — finesse-vs-not
    /// could split this into two fields if needed.
    pub ability: AbilityScoreType,
    pub is_melee: bool,
    pub on_hit_rider: Option<SaveRider>,
}

impl Action for WeaponAction {
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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![self.cost_resource]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let ability_mod = modifier_from_score(caster.ability_score(self.ability));
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            ability_mod,
            target_ac,
            self.damage_dice,
            ability_mod,
            self.damage_type,
            self.is_melee,
        );
        // Apply on-hit rider only if the swing landed (weapon_attack
        // returns empty Vec on miss).
        if let Some(rider) = self.on_hit_rider
            && !effects.is_empty()
        {
            let save = encounter.roll_save(target_id, rider.ability, rider.dc);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: rider.condition,
                    timer: rider.timer,
                }));
            }
        }
        effects
    }
}


/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// 20 tiles = 50ft on this 2.5ft grid — short of the 5e 80/320 normal/long
/// range but plenty for our 40×20 maps.
pub static LONGBOW: WeaponAction = WeaponAction {
    display_name: "longbow",
    aliases: &["bow", "shoot"],
    reach: 20,
    requires_los: true,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Piercing,
    ability: AbilityScoreType::Dexterity,
    is_melee: false,
    on_hit_rider: None,
};

/// Slam — vanilla zombie melee. STR-based 2d6 bludgeoning at melee reach.
pub static SLAM: WeaponAction = WeaponAction {
    display_name: "slam",
    aliases: &["slm"],
    reach: MELEE_REACH,
    requires_los: false,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    ability: AbilityScoreType::Strength,
    is_melee: true,
    on_hit_rider: None,
};

/// Trip — STR-based melee swing that forces a STR save (DC 13) on hit;
/// fail = Prone. Damage applies regardless of the rider save.
pub static TRIP: WeaponAction = WeaponAction {
    display_name: "trip",
    aliases: &["tp"],
    reach: MELEE_REACH,
    requires_los: false,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Bludgeoning,
    ability: AbilityScoreType::Strength,
    is_melee: true,
    on_hit_rider: Some(SaveRider {
        ability: AbilityScoreType::Strength,
        dc: 13,
        condition: Condition::Prone,
        timer: ConditionTimer::Permanent,
    }),
};

/// Ranged spit attack with splash. Primary uses an attack roll vs AC; on
/// hit deals 1d6 acid to the primary target AND auto-damages every
/// combat-active actor whose footprint is adjacent (gap ≤ 1) to the
/// primary for 1d4 acid. Splash hits *anyone* in range — friendly or foe
/// — except the caster themselves. The splash damage is rolled once and
/// shared among splash victims (5e-style shared area roll).
pub struct AcidSpit {}

impl Action for AcidSpit {
    fn name(&self) -> &str {
        "acid spit"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["spit", "as"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(8)
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
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = modifier_from_score(
            caster.ability_score(AbilityScoreType::Dexterity),
        );
        let attack_bonus = dex_mod;
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        // Primary attack — reuse weapon_attack so logging matches other
        // attacks. weapon_attack returns Vec containing the primary
        // DealDamage on hit, empty on miss.
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(1, 6),
            0, // no DEX-to-damage rider; keep splash potential as the perk
            DamageType::Acid,
            false, // ranged
        );
        if effects.is_empty() {
            return effects;
        }

        // Splash: snapshot target's location & size, then sweep nearby
        // actors. Sorted by id for deterministic order.
        let Some(target) = encounter.actors.get(&target_id) else {
            return effects;
        };
        let target_loc = target.location();
        let target_size = get_tiles_from_size(target.size());
        let splash_dice = Dice::new(1, 4);
        let splash_amount = encounter.roll(&splash_dice);
        let mut hit_anyone = false;
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();
        for sid in ids {
            if sid == caster_id || sid == target_id {
                continue;
            }
            let Some(other) = encounter.actors.get(&sid) else {
                continue;
            };
            if !other.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                other.location(),
                get_tiles_from_size(other.size()),
                target_loc,
                target_size,
            );
            // gap ≤ 1 = footprint-adjacent (touching or one tile of clear
            // space). Anyone outside that radius escapes the splash.
            if dist > 1 {
                continue;
            }
            if !hit_anyone {
                hit_anyone = true;
                encounter.log(format!(
                    "  acid spit splash: 1d4({}) = {} acid",
                    splash_amount, splash_amount
                ));
            }
            effects.push(Box::new(DealDamage {
                actor_id: sid,
                amount: splash_amount,
                damage_type: DamageType::Acid,
            }));
        }
        effects
    }
}

pub static ACID_SPIT: LazyLock<AcidSpit> = LazyLock::new(|| AcidSpit {});

/// Scimitar — generic STR-based 1d6 slashing melee attack. Used by
/// goblins and other light melee creatures that don't have a flashy
/// rider effect.
pub static SCIMITAR: WeaponAction = WeaponAction {
    display_name: "scimitar",
    aliases: &["sc"],
    reach: MELEE_REACH,
    requires_los: false,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
    ability: AbilityScoreType::Strength,
    is_melee: true,
    on_hit_rider: None,
};

/// Shortbow — DEX-based 1d4 piercing ranged attack. Bonus-action economy
/// pairs with an Action attack so a goblin can both stab and shoot in
/// the same round.
pub static SHORTBOW: WeaponAction = WeaponAction {
    display_name: "shortbow",
    aliases: &["sb-bow", "shoot2"],
    reach: 12,
    requires_los: true,
    cost_resource: Resource::BonusAction,
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    ability: AbilityScoreType::Dexterity,
    is_melee: false,
    on_hit_rider: None,
};

/// Greatclub — Ogre's signature weapon. STR-based 1d10 bludgeoning with
/// **reach 2** (10ft), letting Large ogres swing past their footprint.
/// `is_melee = true` even at reach 2 — polearms still trigger the
/// prone-target advantage clause.
pub static GREATCLUB: WeaponAction = WeaponAction {
    display_name: "greatclub",
    aliases: &["gc"],
    reach: 2,
    requires_los: false,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(1, 10),
    damage_type: DamageType::Bludgeoning,
    ability: AbilityScoreType::Strength,
    is_melee: true,
    on_hit_rider: None,
};

/// Wolf bite — built-in trip rider on every successful hit. STR-based
/// 1d4 piercing with a STR save (DC 11) or Prone rider.
pub static WOLF_BITE: WeaponAction = WeaponAction {
    display_name: "bite",
    aliases: &["bt"],
    reach: MELEE_REACH,
    requires_los: false,
    cost_resource: Resource::Action,
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    ability: AbilityScoreType::Strength,
    is_melee: true,
    on_hit_rider: Some(SaveRider {
        ability: AbilityScoreType::Strength,
        dc: 11,
        condition: Condition::Prone,
        timer: ConditionTimer::Permanent,
    }),
};

/// Wraps another action and runs it `count` times for one Action-slot
/// expenditure. Reach / LOS / targeting schema are inherited from the
/// sub-attack so creatures can declare e.g. `Multiattack { sub: &SLAM, count: 2 }`
/// without restating constraints. Each sub-attack rolls and logs separately,
/// so a zombie's two slams produce two `slam: 1d20...` lines in the log.
pub struct Multiattack {
    pub display_name: &'static str,
    pub sub_attack: &'static (dyn Action + Send + Sync),
    pub count: u32,
}

impl Action for Multiattack {
    fn name(&self) -> &str {
        self.display_name
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["multi", "ma"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        self.sub_attack.targeting_schema()
    }

    fn reach_tiles(&self) -> Option<isize> {
        self.sub_attack.reach_tiles()
    }

    fn requires_los(&self) -> bool {
        self.sub_attack.requires_los()
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        // 5e: every swing of a multiattack lands even if an earlier one
        // killed the target (extra swings tick failed death saves on a
        // dying creature). cleanup_dead_actors only runs *after* all
        // side-effects in this batch are queued, so the target is always
        // present here regardless.
        let mut all = Vec::new();
        for _ in 0..self.count {
            all.extend(self.sub_attack.side_effects(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            ));
        }
        all
    }
}

/// Zombie multiattack: 2 slams per Action. Hits twice as hard as a vanilla
/// zombie at the cost of nothing (this game's zombies are scarier than MM).
pub static ZOMBIE_MULTISLAM: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "multislam",
    sub_attack: &SLAM,
    count: 2,
});

/// Roll a d20 attack against `target_ac`, log the breakdown, and on a hit
/// roll `damage_dice + damage_bonus` of `damage_type` against `target_id`.
/// `is_melee` drives Prone-target advantage / ranged disadvantage clauses.
///
/// **Critical hits**: a final d20 of 20 (after advantage / disadvantage)
/// auto-hits regardless of AC and rolls the damage dice twice — the
/// modifier is added once. 5e RAW.
///
/// Returns the side-effect vec (empty on miss). Centralizes the pattern
/// so every weapon-style attack logs in the same shape.
#[allow(clippy::too_many_arguments)]
pub(crate) fn weapon_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    target_ac: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    use crate::conditions::Condition;
    let mode = encounter.compute_attack_mode(caster_id, target_id, is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    // Bless rider on the attacker — +1d4 to attack rolls while the
    // condition is active. Rolled separately so the log line can show
    // the bless die explicitly (instead of folding it into the static
    // attack bonus, which would obscure the source).
    let bless_bonus = if encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.has_condition(Condition::Blessed))
    {
        encounter.roll(&Dice::new(1, 4)) as i32
    } else {
        0
    };
    let is_crit = raw_attack == 20;
    let attack_total = raw_attack + attack_bonus + bless_bonus;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let bless_suffix = if bless_bonus > 0 {
        format!(" + bless({})", bless_bonus)
    } else {
        String::new()
    };
    encounter.log(format!(
        "  {}: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
        action_name,
        raw_attack,
        attack_bonus,
        bless_suffix,
        attack_total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roll(&damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&damage_dice) as i32
    } else {
        0
    };
    let damage = (raw_damage + crit_extra + damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            action_name,
            damage_dice,
            raw_damage,
            damage_dice,
            crit_extra,
            damage_bonus,
            damage,
            damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            action_name, damage_dice, raw_damage, damage_bonus, damage, damage_type,
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: damage,
        damage_type,
    })]
}
