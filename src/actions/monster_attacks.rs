use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema, first_target_id},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        attack::{AttackParams, resolve_attack},
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// Resolve a vanilla "roll d20 vs AC, on hit roll damage" weapon swing.
/// Loads attack/damage modifiers from the caster, supports the SingleActor
/// schema, and delegates the actual resolution to `resolve_attack` so the
/// log shape and crit / advantage handling stays uniform across every
/// weapon implementation. Returns the side effects list `resolve_attack`
/// produced (DealDamage on hit, empty on miss).
#[allow(clippy::too_many_arguments)]
pub fn simple_weapon_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    action_name: &str,
    attack_ability: AbilityScoreType,
    damage_ability: Option<AbilityScoreType>,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(target_id) = first_target_id(target_ids) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let attack_mod =
        modifier_from_score(caster.ability_score(attack_ability)) + caster.proficiency_bonus();
    let damage_mod = damage_ability
        .map(|a| modifier_from_score(caster.ability_score(a)))
        .unwrap_or(0);
    resolve_attack(
        encounter,
        AttackParams {
            caster_id,
            target_id,
            action_name,
            attack_bonus: attack_mod,
            damage_dice,
            damage_bonus: damage_mod,
            damage_type,
            is_melee,
        },
    )
}

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
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            self.attack_ability,
            self.damage_ability,
            self.damage_dice,
            self.damage_type,
            self.is_melee,
        )
    }
}

/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// Reach is in tiles (not feet); 20 tiles = 50ft on this 2.5ft grid, which is
/// short of the 5e 80/320 normal/long range but plenty for our 40×20 maps.
pub static LONGBOW: SimpleWeapon = SimpleWeapon {
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
};

/// Generic STR-based 2d6 bludgeoning slam used by zombies. Stays as the
/// canonical "monster fist" attack so multislams (and tests) reference it.
pub static SLAM: SimpleWeapon = SimpleWeapon {
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
};

/// Scimitar — generic STR-based 1d6 slashing melee attack. Used by
/// goblins and other light melee creatures that don't have a flashy
/// rider effect.
pub static SCIMITAR: SimpleWeapon = SimpleWeapon {
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
};

/// Shortbow — DEX-based 1d4 piercing ranged attack on a *bonus action*.
/// Pairs with a primary action attack; reach 12 tiles (≈30ft).
pub static SHORTBOW: SimpleWeapon = SimpleWeapon {
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
};

/// Dagger — finesse 1d4 piercing melee weapon. STR-or-DEX choice;
/// we use DEX which is the typical kobold / rogue stat. Cost 1 Action.
pub static DAGGER: SimpleWeapon = SimpleWeapon {
    display_name: "dagger",
    aliases: &["dag"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
};

/// Greatclub — Ogre's signature weapon. STR-based 1d10 bludgeoning with
/// **reach 2** (10ft) — first polearm-style attack in the codebase.
pub static GREATCLUB: SimpleWeapon = SimpleWeapon {
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
};

/// Generic STR-based bite attack — 1d6+STR piercing, no rider. Use this
/// for creatures whose bite is pure damage (Troll, most beasts). Creatures
/// that also trip or grapple on a bite should use WolfBite or a dedicated
/// variant instead.
pub struct Bite {}

impl Action for Bite {
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
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 6),
            DamageType::Piercing,
            true,
        )
    }
}

pub static BITE: LazyLock<Bite> = LazyLock::new(|| Bite {});

/// Melee attack that, on a hit, forces a STR save (DC = 8 + prof + STR mod) or knocks the
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
        // Melee reach so trip behaves like a normal melee attack — but
        // wide enough that the test placing target at gap 1 still works.
        Some(MELEE_REACH)
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
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
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;

        let target_id = first_target_id(target_ids);
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 6),
            DamageType::Bludgeoning,
            true,
        );
        // simple_weapon_attack returns empty Vec on miss — only roll
        // the save if damage was queued (the attack landed).
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else { return effects };
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

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
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

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.ability_attack_bonus(AbilityScoreType::Dexterity);
        if !encounter.actors.contains_key(&target_id) {
            return Vec::new();
        }

        // Primary attack — reuse the canonical attack resolver so the
        // log shape matches every other weapon. Returns DealDamage on
        // hit, empty Vec on miss.
        let mut effects = resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 6),
                damage_bonus: 0,
                damage_type: DamageType::Acid,
                is_melee: false,
            },
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
        let ids = encounter.sorted_actor_ids();
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

/// Giant-spider melee bite with a poison rider. Hit deals 1d10 piercing
/// (the biting jaws); on hit, the target also makes a CON save vs DC 11
/// — fail = 2d4 poison damage and Poisoned for 2 rounds.
pub struct SpiderBite {}

impl Action for SpiderBite {
    fn name(&self) -> &str {
        "spider bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sbite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        use crate::engine::side_effects::ApplyCondition;
        let target_id = match first_target_id(target_ids) {
            Some(id) => id,
            None => return Vec::new(),
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 10),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Poison rider — separate save. On fail: extra poison damage AND
        // Poisoned for 2 rounds.
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 11);
        if !save.passed() {
            let poison = encounter.roll(&Dice::new(2, 4));
            encounter.log(format!(
                "  spider venom: 2d4({}) = {} poison",
                poison, poison
            ));
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: poison,
                damage_type: DamageType::Poison,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(2),
            }));
        }
        effects
    }
}
pub static SPIDER_BITE: LazyLock<SpiderBite> = LazyLock::new(|| SpiderBite {});


/// Greataxe — Orc-flavored heavy two-hander. STR-based 1d12 slashing,
/// melee reach. Hits harder than a longsword on a single die; pairs
/// with the orc's high STR for a punishing single-attack profile.
pub struct Greataxe {}

impl Action for Greataxe {
    fn name(&self) -> &str {
        "greataxe"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ga"]
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 12),
            DamageType::Slashing,
            true,
        )
    }
}
pub static GREATAXE: LazyLock<Greataxe> = LazyLock::new(|| Greataxe {});

/// Heavy Crossbow — DEX-based 1d10 piercing ranged. Differs from the
/// Longbow in damage die (1d10 vs 1d8) and conceptually loading time
/// (we don't model the loading property today). Used by bandits.
pub struct HeavyCrossbow {}

impl Action for HeavyCrossbow {
    fn name(&self) -> &str {
        "heavy crossbow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hcb", "crossbow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(16)
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 10),
            DamageType::Piercing,
            false,
        )
    }
}
pub static HEAVY_CROSSBOW: LazyLock<HeavyCrossbow> = LazyLock::new(|| HeavyCrossbow {});

/// Wolf-specific bite: 1d4 STR-based piercing with a built-in trip rider.
/// On every hit forces a STR save (DC = 8 + prof + STR mod); fail = Prone.
/// For a plain bite without the trip use BITE instead.
pub struct WolfBite {}

impl Action for WolfBite {
    fn name(&self) -> &str {
        "wolf bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;

        let target_id = first_target_id(target_ids);
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else { return effects };
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

/// Frightful Howl — wolf bonus action. Every enemy within 4 tiles must
/// make a WIS save against DC 11 or be Frightened for 3 rounds.
/// Doesn't deal damage. Demonstrates the AoE-no-damage save pattern.
pub struct FrightfulHowl {}

impl Action for FrightfulHowl {
    fn name(&self) -> &str {
        "howl"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hwl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        const RADIUS: isize = 4;
        const DC: i32 = 11;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        let caster_team = caster.team();
        encounter.log("  howl: enemies in 4 tiles must save vs DC 11 WIS or be frightened");

        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
            Vec::new();
        for tid in ids {
            if tid == caster_id {
                continue;
            }
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if target.team() == caster_team || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                caster_loc,
                caster_size,
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(3),
                }));
            }
        }
        effects
    }
}
pub static FRIGHTFUL_HOWL: LazyLock<FrightfulHowl> = LazyLock::new(|| FrightfulHowl {});

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

    fn damage_types(&self) -> Vec<DamageType> {
        self.sub_attack.damage_types()
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Inherit the sub-attack's cost shape so a `Multiattack { sub:
        // SHORTBOW }` correctly costs a BonusAction, not an Action. We
        // filter out Movement (sub-attacks shouldn't charge per-swing).
        self.sub_attack
            .cost(encounter, caster_id, target_ids, target_locations, overrides)
            .into_iter()
            .filter(|r| !matches!(r, Resource::Movement(_)))
            .collect()
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

/// Goblin Boss multiattack: 2 scimitar swings per Action. Distinct from
/// the standard goblin's single swing — the boss hits twice as often,
/// which combined with the higher base AC makes the encounter pop.
pub static GOBLIN_BOSS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double scimitar",
    sub_attack: &SCIMITAR,
    count: 2,
});

/// Imp's poisoned sting — finesse melee, 1d4+DEX piercing on hit plus
/// a CON save (DC 11) for 2d10 poison rider damage. Showcases the
/// "weapon attack + ability save rider" pattern using the SimpleWeapon
/// + custom side-effect blend.
pub struct ImpSting {}

impl Action for ImpSting {
    fn name(&self) -> &str {
        "sting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["st", "imp-sting"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Poison]
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
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 11);
        if !save.passed() {
            let poison = encounter.roll(&Dice::new(2, 10));
            encounter.log(format!(
                "  imp venom: 2d10({}) = {} poison",
                poison, poison
            ));
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: poison,
                damage_type: DamageType::Poison,
            }));
        }
        effects
    }
}

pub static IMP_STING: LazyLock<ImpSting> = LazyLock::new(|| ImpSting {});

/// Frightful Presence — bonus action AoE save effect: every enemy within
/// 6 tiles makes a WIS save (DC 11) or is Frightened for 3 rounds.
/// No damage. Used by fire imps and mid-tier dragon-flavored creatures.
pub struct FrightfulPresence {}

impl Action for FrightfulPresence {
    fn name(&self) -> &str {
        "frightful presence"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fp", "presence"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
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
        vec![Resource::BonusAction]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        const RADIUS: isize = 6;
        const DC: i32 = 11;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        let caster_team = caster.team();

        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        // Filter once so we know if we have any victims to log about.
        // Saves a log line when every potential target is already
        // Frightened or out of range — keeps the play-by-play clean.
        let mut victims: Vec<usize> = Vec::new();
        for tid in ids {
            if tid == caster_id {
                continue;
            }
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if target.team() == caster_team || !target.is_combat_active() {
                continue;
            }
            // Already-Frightened targets are immune to a re-application —
            // RAW: a successful save against Frightful Presence makes you
            // immune for 24h. We model it as: don't re-roll for actors
            // already carrying the condition.
            if target.has_condition(Condition::Frightened) {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                caster_loc,
                caster_size,
            );
            if dist > RADIUS {
                continue;
            }
            victims.push(tid);
        }
        if victims.is_empty() {
            return Vec::new();
        }
        encounter.log(
            "  frightful presence: nearby enemies make a WIS check vs DC 11",
        );

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in victims {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(3),
                }));
            }
        }
        effects
    }
}

pub static FRIGHTFUL_PRESENCE: LazyLock<FrightfulPresence> =
    LazyLock::new(|| FrightfulPresence {});

/// Wraith Life Drain — melee attack. d20 + STR-prof vs AC; on hit 4d8+3
/// necrotic damage AND the target must make a CON save vs DC 14 or have
/// its max HP reduced by the damage dealt (lasts until long rest in 5e;
/// we just leave the reduction in place — long rest restores baseline
/// `bump_max_hp` does not — so the penalty is durable). Drained max HP
/// floors at 1.
pub struct LifeDrain {}

impl Action for LifeDrain {
    fn name(&self) -> &str {
        "life drain"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["drain", "ld"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
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
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustMaxHp;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Resolve the attack inline (rather than via `resolve_attack`)
        // because we need the rolled damage to mirror into the max-HP
        // reduction on a failed CON save. Keeps the log shape identical
        // to the shared resolver.
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Strength))
            + caster.proficiency_bonus();
        let target_ac = encounter
            .actors
            .get(&target_id)
            .map(|a| a.armor_class() as i32)
            .unwrap_or(10);
        let mode = encounter.compute_attack_mode(caster_id, target_id, true);
        let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
        let is_crit = raw_attack == 20;
        let buff = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.attack_bonus_buff())
            .unwrap_or(0);
        let (bless_die, bless_note) = encounter.bless_bane_attack_die(caster_id);
        let attack_total = raw_attack + attack_mod + buff + bless_die;
        let hit = is_crit || attack_total >= target_ac;
        let outcome = if is_crit {
            "CRIT!"
        } else if hit {
            "hit"
        } else {
            "miss"
        };
        encounter.log(format!(
            "  life drain: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
            raw_attack,
            attack_mod + buff,
            bless_note,
            attack_total,
            target_ac,
            mode.log_suffix(),
            outcome,
        ));
        if !hit {
            return Vec::new();
        }
        let dice = Dice::new(4, 8);
        let raw_dmg = encounter.roll(&dice) as i32;
        let crit_extra = if is_crit { encounter.roll(&dice) as i32 } else { 0 };
        let damage = (raw_dmg + crit_extra + 3).max(0) as u32;
        encounter.log(format!(
            "  life drain: {}({}){:+} = {} necrotic damage{}",
            dice,
            raw_dmg,
            3,
            damage,
            if is_crit { " (crit)" } else { "" }
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Necrotic,
        })];
        // CON save vs DC 14 to avoid max-HP reduction. RAW: the reduction
        // equals the necrotic damage dealt; we use the pre-mitigation
        // amount so resistance to necrotic doesn't double-protect.
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 14);
        if !save.passed() {
            effects.push(Box::new(AdjustMaxHp {
                actor_id: target_id,
                delta: -(damage as i32),
            }));
        }
        effects
    }
}

pub static LIFE_DRAIN: LazyLock<LifeDrain> = LazyLock::new(|| LifeDrain {});

/// Ghoul claws. 1d4+2 slashing on hit; on hit *against a non-elf*
/// (we don't model lineage; we apply the rider unconditionally) the
/// target makes a DC 10 CON save or is Paralyzed for one round. The
/// rider is the marquee ghoul mechanic — paralyze chains hard with
/// the auto-crit-on-melee-hit clause on paralyzed targets.
pub struct GhoulClaws {}

impl Action for GhoulClaws {
    fn name(&self) -> &str {
        "ghoul claws"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gc", "claw"]
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
        let target_id = first_target_id(target_ids);
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 4),
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else {
            return effects;
        };
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 10);
        if !save.passed() {
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
                timer: ConditionTimer::Rounds(2),
            }));
        }
        effects
    }
}

pub static GHOUL_CLAWS: LazyLock<GhoulClaws> = LazyLock::new(|| GhoulClaws {});

/// Bugbear morningstar — 1d8+2 piercing, with a Surprise-Attack rider
/// that deals an extra 2d6 damage on the first hit of the encounter
/// per RAW. We simplify: extra damage applies on round 1 only (when
/// the encounter is still "fresh"), and only on the first time the
/// bugbear attacks. Tracking a per-actor "has surprise-attacked yet"
/// flag without polluting state: we just gate on `encounter.round()
/// == 1` and skip the second-strike concern (the AI rarely lines up
/// a clean second swing in the first round anyway).
pub struct BugbearMorningstar {}

impl Action for BugbearMorningstar {
    fn name(&self) -> &str {
        "morningstar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "mace"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        let target_id = first_target_id(target_ids);
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 8),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        if encounter.round() != 1 {
            return effects;
        }
        let Some(target_id) = target_id else {
            return effects;
        };
        // Surprise-attack rider: +2d6 on the opening salvo.
        let surprise = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  surprise attack: +2d6({}) extra piercing",
            surprise
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: surprise,
            damage_type: DamageType::Piercing,
        }));
        effects
    }
}

pub static BUGBEAR_MORNINGSTAR: LazyLock<BugbearMorningstar> =
    LazyLock::new(|| BugbearMorningstar {});

/// Dire Wolf bite — 2d6+3 piercing with the same trip rider as
/// `WolfBite` but a higher save DC and bigger dice. Demonstrates how
/// data-flavored copies of an existing pattern can share most of the
/// structure; we don't extract a shared "bite with trip" helper yet
/// because the rider's DC and dice differ per template.
pub struct DireWolfBite {}

impl Action for DireWolfBite {
    fn name(&self) -> &str {
        "dire wolf bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dwb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        let target_id = first_target_id(target_ids);
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else {
            return effects;
        };
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 13);
        if !save.passed() {
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static DIRE_WOLF_BITE: LazyLock<DireWolfBite> = LazyLock::new(|| DireWolfBite {});

/// Re-export the spell-table FIRE_BOLT here so monster files that import
/// `crate::actions::monster_attacks::FIRE_BOLT` keep working — the
/// canonical definition lives with the other spells, this just gives
/// fire-themed monsters a handle into the same Action.
pub use crate::actions::spells::FIRE_BOLT;
