use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{DealDamage, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// A vanilla weapon attack: roll d20 + ability mod vs AC, on hit roll
/// `damage_dice` + (optional) ability mod of `damage_type`. Crit on raw 20
/// doubles the dice. No riders, no splash, no AoE — everything that fits
/// this shape (Slam, Scimitar, Longbow, Shortbow, Greatclub) becomes a
/// data-only `SimpleWeapon` declaration instead of its own Action impl.
///
/// The `damage_ability` field is `Some(stat)` to add `modifier_from_score`
/// to the damage roll (most martial weapons), `None` to skip — matches
/// 5e's "ability modifier to damage" baseline plus the natural-attack
/// exceptions (e.g. an acid-spit's splash that uses no ability mod).
pub struct SimpleWeapon {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_ability: Option<AbilityScoreType>,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
    pub is_melee: bool,
    pub requires_los: bool,
    pub cost_resource: Resource,
}

impl Action for SimpleWeapon {
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
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
        let attack_mod = modifier_from_score(caster.ability_score(self.attack_ability));
        let damage_mod = self
            .damage_ability
            .map_or(0, |ab| modifier_from_score(caster.ability_score(ab)));
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.display_name,
            attack_mod,
            target_ac,
            self.damage_dice,
            damage_mod,
            self.damage_type,
            self.is_melee,
        )
    }
}

/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// Reach is in tiles (not feet); 20 tiles = 50ft on this 2.5ft grid, which is
/// short of the 5e 80/320 normal/long range but plenty for our 40×20 maps.
pub static LONGBOW: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "longbow",
    aliases: &["bow", "shoot"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Piercing,
    reach: 20,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
});

/// Generic STR-based 2d6 bludgeoning slam used by zombies. Stays as the
/// canonical "monster fist" attack so multislams (and tests) reference it.
pub static SLAM: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "slam",
    aliases: &["slm"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
});

/// Scimitar — generic STR-based 1d6 slashing melee attack. Used by
/// goblins and other light melee creatures that don't have a flashy
/// rider effect.
pub static SCIMITAR: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "scimitar",
    aliases: &["sc"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
});

/// Shortbow — DEX-based 1d4 piercing ranged attack on a *bonus action*.
/// Pairs with a primary action attack; reach 12 tiles (≈30ft).
pub static SHORTBOW: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "shortbow",
    aliases: &["sb-bow", "shoot2"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    reach: 12,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::BonusAction,
});

/// Greatclub — Ogre's signature weapon. STR-based 1d10 bludgeoning with
/// **reach 2** (10ft) — first polearm-style attack in the codebase.
pub static GREATCLUB: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "greatclub",
    aliases: &["gc"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 10),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
});

/// Dagger — finesse melee, light. DEX-to-hit and damage; 1d4 piercing.
/// Common bandit / rogue armament. Action-cost (a separate `OFF_HAND_DAGGER`
/// variant could later use BonusAction for two-weapon-fighting).
pub static DAGGER: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "dagger",
    aliases: &["dgr"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
});

/// Greataxe — STR-based 1d12 slashing melee. The classic two-handed
/// martial weapon; pairs with bandit-captain / berserker stat lines.
pub static GREATAXE: LazyLock<SimpleWeapon> = LazyLock::new(|| SimpleWeapon {
    display_name: "greataxe",
    aliases: &["gax"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 12),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
});

/// Melee attack that, on a hit, forces a STR save (DC 13) or knocks the
/// target prone. Demonstrates the save-then-condition pattern: damage
/// applies regardless, the prone condition only on save failure.
pub struct TripAttack {}

impl Action for TripAttack {
    fn name(&self) -> &str {
        "trip"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["tp"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        use crate::conditions::Condition;
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(1, 6),
            str_mod,
            DamageType::Bludgeoning,
            true, // melee
        );
        // weapon_attack returns empty Vec on miss — only roll the save if
        // damage was queued (the attack landed).
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 13);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                // Prone from a trip persists until stand-up clears it.
                timer: crate::conditions::ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static TRIP: LazyLock<TripAttack> = LazyLock::new(|| TripAttack {});

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

/// Wolf bite — built-in trip rider on every successful hit. STR-based
/// 1d4 piercing; on hit forces a STR save vs DC 11, fail = Prone. Fuses
/// the TripAttack rider pattern into a single creature-canonical action.
pub struct WolfBite {}

impl Action for WolfBite {
    fn name(&self) -> &str {
        "bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Strength));
        let attack_bonus = caster.attack_bonus();
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(1, 4),
            str_mod,
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 11);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}
pub static WOLF_BITE: LazyLock<WolfBite> = LazyLock::new(|| WolfBite {});

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
    sub_attack: &*SLAM,
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
fn weapon_attack(
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
    let mode = encounter.compute_attack_mode(caster_id, target_id, is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    let attack_total = raw_attack + attack_bonus;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
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
