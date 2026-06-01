use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        Action, MELEE_REACH, TargetingSchema, bonus_action_only, first_target_id,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        attack::{AttackParams, resolve_attack},
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
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
    simple_weapon_attack_ranged(
        encounter, caster_id, target_ids, action_name, attack_ability,
        damage_ability, damage_dice, damage_type, is_melee, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn simple_weapon_attack_ranged(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    action_name: &str,
    attack_ability: AbilityScoreType,
    damage_ability: Option<AbilityScoreType>,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
    normal_range: Option<isize>,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(target_id) = first_target_id(target_ids) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let attack_mod = caster.ability_modifier(attack_ability) + caster.proficiency_bonus();
    let damage_mod = damage_ability
        .map(|a| caster.ability_modifier(a))
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
            long_range: normal_range,
                is_spell: false,
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
    /// 5e normal range (in tiles) for ranged weapons. Attacks beyond this
    /// distance but within `reach` (max range) impose disadvantage. `None`
    /// means no long-range penalty (melee weapons). Longbow: 12 tiles
    /// (30ft normal), reach 20 tiles (50ft max). Shortbow: 8 tiles, reach 12.
    pub normal_range: Option<isize>,
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
        let mut effects = simple_weapon_attack_ranged(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            self.attack_ability,
            self.damage_ability,
            self.damage_dice,
            self.damage_type,
            self.is_melee,
            self.normal_range,
        );
        // 5e Extra Attack: when the Attack action costs an Action resource
        // and the caster has Extra Attack, resolve a second swing against
        // the same target as part of the same action.
        if self.cost_resource == Resource::Action
            && encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.has_extra_attack())
        {
            encounter.log("  Extra Attack:");
            effects.extend(simple_weapon_attack_ranged(
                encounter,
                caster_id,
                target_ids,
                self.name(),
                self.attack_ability,
                self.damage_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                self.normal_range,
            ));
        }
        effects
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
    normal_range: Some(12),
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
    normal_range: None,
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
    normal_range: None,
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
    normal_range: Some(8),
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
    normal_range: None,
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
    normal_range: None,
};

/// Warhammer — STR-based 1d8 bludgeoning martial weapon. The classic
/// dwarven sidearm; in our engine the versatile-2H clause collapses to
/// the simpler 1d8 base (the 2H 1d10 alternative would need a per-action
/// grip toggle the picker doesn't surface). Slots between scimitar (1d6)
/// and greataxe (1d12) for STR-build martials who want a bludgeoning
/// option (some creatures resist slashing / piercing).
pub static WARHAMMER: SimpleWeapon = SimpleWeapon {
    display_name: "warhammer",
    aliases: &["wh", "hammer"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
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
                long_range: None,
                is_spell: false,
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 12),
            DamageType::Slashing,
            true,
        );
        // 5e Extra Attack: Greataxe costs an Action, so if the caster has
        // Extra Attack, resolve a second swing against the same target.
        if encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_extra_attack())
        {
            encounter.log("  Extra Attack:");
            effects.extend(simple_weapon_attack(
                encounter,
                caster_id,
                target_ids,
                self.name(),
                AbilityScoreType::Strength,
                Some(AbilityScoreType::Strength),
                Dice::new(1, 12),
                DamageType::Slashing,
                true,
            ));
        }
        effects
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
        bonus_action_only()
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

/// Heterogeneous multi-attack wrapper. Bundles multiple distinct
/// sub-attacks (each with its own count) into a single Action — used
/// by creatures whose multi mixes limbs (Pit Fiend: 1 bite + 2 claws,
/// Hippogriff: 1 beak + 2 talons by MM RAW, etc.). The standard
/// `Multiattack` struct is the same-sub-attack-twice case; this one
/// supports the more general N×A + M×B + K×C pattern without each
/// creature reaching for a bespoke `impl Action`.
///
/// Targeting / reach / requires_los are inherited from the first
/// sub-attack — every entry in `parts` is expected to share these
/// (mixed melee/ranged multis aren't a thing in 5e); the engine's
/// reach + LOS validation runs once per action.
pub struct CompoundAttack {
    pub display_name: &'static str,
    /// List of (sub_attack, count). Each entry produces `count` calls
    /// to the sub-attack's `side_effects` for the same target. Order
    /// of resolution mirrors declaration so log lines read top-down.
    /// Stored as a `Vec` rather than a slice so the trait-object
    /// coercion inside the array literal works cleanly — the cost is
    /// one heap allocation per `CompoundAttack` (we wrap them in
    /// `LazyLock` anyway, so it's a one-shot cost at startup).
    pub parts: Vec<(&'static (dyn Action + Send + Sync), u32)>,
}

impl Action for CompoundAttack {
    fn name(&self) -> &str {
        self.display_name
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["multi", "ma"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        // Inherit from the first sub-attack — every entry is expected
        // to use the same schema.
        self.parts
            .first()
            .map(|(a, _)| a.targeting_schema())
            .unwrap_or(TargetingSchema::SingleActor)
    }

    fn reach_tiles(&self) -> Option<isize> {
        self.parts.first().and_then(|(a, _)| a.reach_tiles())
    }

    fn requires_los(&self) -> bool {
        self.parts.first().is_some_and(|(a, _)| a.requires_los())
    }

    fn damage_types(&self) -> Vec<DamageType> {
        // Union of damage types across all parts. Useful for the UI
        // resistance hint — a bite + claws Pit Fiend strike surfaces
        // both Piercing and Slashing.
        let mut out = Vec::new();
        for (a, _) in &self.parts {
            for dt in a.damage_types() {
                if !out.contains(&dt) {
                    out.push(dt);
                }
            }
        }
        out
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Inherit the first sub-attack's cost shape (filtering movement
        // for the same reason as `Multiattack`). Mixed-cost compounds
        // aren't supported — the cost is the wrapper's single envelope.
        self.parts
            .first()
            .map(|(a, _)| {
                a.cost(encounter, caster_id, target_ids, target_locations, overrides)
                    .into_iter()
                    .filter(|r| !matches!(r, Resource::Movement(_)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut all = Vec::new();
        for (sub, count) in &self.parts {
            for _ in 0..*count {
                all.extend(sub.side_effects(
                    encounter,
                    caster_id,
                    target_ids,
                    target_locations,
                    overrides,
                ));
            }
        }
        all
    }
}

/// Goblin Boss multiattack: 2 scimitar swings per Action. Distinct from
/// the standard goblin's single swing — the boss hits twice as often,
/// which combined with the higher base AC makes the encounter pop.
pub static GOBLIN_BOSS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double scimitar",
    sub_attack: &SCIMITAR,
    count: 2,
});

/// Bandit Captain multiattack: 3 scimitar swings per Action — a tougher
/// version of the goblin boss's pattern. Pair with a heavy crossbow for
/// the bonus-action ranged finisher.
pub static BANDIT_CAPTAIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "triple scimitar",
    sub_attack: &SCIMITAR,
    count: 3,
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
        bonus_action_only()
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

        const RADIUS: isize = 6;
        const DC: i32 = 11;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();

        // 5e: a successful save against Frightful Presence makes you
        // immune for 24h. We model the simpler "don't re-roll for actors
        // already carrying the condition" — same end state without the
        // per-target immunity bookkeeping.
        let victims: Vec<usize> = encounter
            .enemy_burst_targets(caster_id, caster_loc, RADIUS)
            .into_iter()
            .filter(|id| {
                encounter
                    .actors
                    .get(id)
                    .is_some_and(|a| !a.has_condition(Condition::Frightened))
            })
            .collect();
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.ability_modifier(AbilityScoreType::Strength)
            + caster.proficiency_bonus();
        // resolve_attack_outcome returns the queued DealDamage plus the
        // resolved damage value — we mirror that value into the max-HP
        // drain on a failed CON save.
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "life drain",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(4, 8),
                damage_bonus: 3,
                damage_type: DamageType::Necrotic,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
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

/// Vampiric Bite — Vampire Spawn signature attack. Melee weapon attack
/// (STR + prof to hit), 1d6+STR piercing plus 3d6 necrotic. The Vampire
/// Spawn regains HP equal to the necrotic damage dealt. Distinct from
/// Wraith's Life Drain: no max-HP drain rider, but a much larger heal-
/// per-hit lane. Models the trope of a vampire feeding to top off.
pub struct VampiricBite {}

impl Action for VampiricBite {
    fn name(&self) -> &str {
        "vampiric bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vb", "feed"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Necrotic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::Heal;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let attack_mod = str_mod + caster.proficiency_bonus();
        // Piercing portion goes through the shared resolver; we layer the
        // necrotic rider and self-heal off of the hit/damage result.
        let (mut effects, piercing_damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "vampiric bite",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(1, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Piercing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if piercing_damage == 0 {
            return effects;
        }
        // On hit, 3d6 necrotic rider (no STR bonus, no crit doubling here —
        // RAW: only the weapon damage doubles; the bite's separate
        // necrotic die is added as flat extra damage).
        let necrotic = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  vampiric bite: 3d6({}) = {} necrotic; vampire regains {} HP",
            necrotic, necrotic, necrotic
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: necrotic,
            damage_type: DamageType::Necrotic,
        }));
        // Heal the vampire by the necrotic damage dealt (pre-resistance).
        effects.push(Box::new(Heal {
            actor_id: caster_id,
            amount: necrotic,
        }));
        effects
    }
}

pub static VAMPIRIC_BITE: LazyLock<VampiricBite> = LazyLock::new(|| VampiricBite {});

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

/// Owlbear's signature multiattack rolled into one Action: a beak (1d10+5
/// piercing) and a claws (2d8+5 slashing) swing at the same target. We
/// resolve them sequentially so each rolls independently for hit, crit
/// and damage; both go into the same `Vec<DealDamage>` so the engine
/// applies them in order. Cost is a single Action — the multiattack
/// trade is "spend one Action, get two attack rolls" without a slot.
pub struct OwlbearMultiattack {}

impl Action for OwlbearMultiattack {
    fn name(&self) -> &str {
        "owlbear multiattack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["om", "owl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Slashing]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut all = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "owlbear beak",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 10),
            DamageType::Piercing,
            true,
        );
        all.extend(simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "owlbear claws",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 8),
            DamageType::Slashing,
            true,
        ));
        all
    }
}

pub static OWLBEAR_MULTIATTACK: LazyLock<OwlbearMultiattack> =
    LazyLock::new(|| OwlbearMultiattack {});

/// Will-o-Wisp's shock — at-will incorporeal touch attack. DEX-based
/// melee spell-style swing for 2d8 lightning. The +DEX-to-hit shape
/// matches the MM stat block (the wisp uses its high DEX as the attack
/// stat). Lightning damage typing means undead-immune armor doesn't
/// blunt it; the wisp is fragile but its damage type is unusual.
pub struct WispShock {}

impl Action for WispShock {
    fn name(&self) -> &str {
        "shock"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh", "wisp-shock"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        // No DEX-mod added to damage — wisp's stat block lists no damage
        // ability mod (it's a magical zap, not a weapon swing).
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Dexterity,
            None,
            Dice::new(2, 8),
            DamageType::Lightning,
            true,
        )
    }
}

pub static WISP_SHOCK: LazyLock<WispShock> = LazyLock::new(|| WispShock {});

/// Werewolf claws — 2d4+STR slashing. Pairs with Werewolf bite as a
/// multiattack option. The bite carries the lycanthropy flavor; claws
/// are the steady damage lane that doesn't need any rider.
pub static WEREWOLF_CLAWS: SimpleWeapon = SimpleWeapon {
    display_name: "werewolf claws",
    aliases: &["ww-claws"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 4),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Werewolf bite — 1d8+STR piercing. On a hit against a humanoid (we
/// drop the species gate — every PC pool is humanoid enough), the
/// target makes a DC 12 CON save or contracts an exhaustion-like
/// debuff (Poisoned for 3 rounds, modeling the early-stage lycanthropy
/// fever). The save DC matches the MM stat block; the rider is a
/// simplification of the full lycanthropy curse so we don't have to
/// model multi-day transformations.
pub struct WerewolfBite {}

impl Action for WerewolfBite {
    fn name(&self) -> &str {
        "werewolf bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ww-bite"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 8),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Lycanthropy bite rider: DC 12 CON save or Poisoned 3 rounds.
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 12);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(3),
            }));
        }
        effects
    }
}

pub static WEREWOLF_BITE: LazyLock<WerewolfBite> = LazyLock::new(|| WerewolfBite {});

/// Werewolf multiattack: one bite + one claws swing per Action. Pattern
/// matches Owlbear's multi: two separate `simple_weapon_attack` calls
/// against the same target, both into one DealDamage list.
pub struct WerewolfMultiattack {}

impl Action for WerewolfMultiattack {
    fn name(&self) -> &str {
        "werewolf multiattack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ww-multi", "wwma"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Slashing]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Delegate to WerewolfBite (which carries the lycanthropy rider)
        // and then layer a claws swing on top. Both calls roll their own
        // d20 / damage; this is exactly the Owlbear pattern.
        let mut all = WEREWOLF_BITE.side_effects(
            encounter,
            caster_id,
            target_ids,
            None,
            None,
        );
        all.extend(simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "werewolf claws",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 4),
            DamageType::Slashing,
            true,
        ));
        all
    }
}

pub static WEREWOLF_MULTIATTACK: LazyLock<WerewolfMultiattack> =
    LazyLock::new(|| WerewolfMultiattack {});

/// Mimic adhesive bite — 1d8+STR piercing + 1d8 acid. On hit, the target
/// is stuck (`Adhered` condition) until they break free; the condition
/// zeros movement so the AI can't shake free without an explicit
/// escape action (we don't yet model an escape DC — the duration is
/// short so it self-resolves). Captures the "object disguise that
/// snaps shut on adventurers" trope without the lure-mechanic.
pub struct MimicBite {}

impl Action for MimicBite {
    fn name(&self) -> &str {
        "mimic bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Acid]
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 8),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let acid = encounter.roll(&Dice::new(1, 8));
        encounter.log(format!("  adhesive acid: 1d8({}) = {} acid", acid, acid));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: acid,
            damage_type: DamageType::Acid,
        }));
        // Stick the victim in place for 2 rounds — short enough that the
        // mimic can't permanently lock down a single target across a
        // long fight.
        effects.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Adhered,
            timer: ConditionTimer::Rounds(2),
        }));
        effects
    }
}

pub static MIMIC_BITE: LazyLock<MimicBite> = LazyLock::new(|| MimicBite {});

/// Harpy talons — 2d4+STR slashing. Simple natural-weapon strike with no
/// rider; the harpy's real threat is the Luring Song.
pub static HARPY_TALONS: SimpleWeapon = SimpleWeapon {
    display_name: "harpy talons",
    aliases: &["talons"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 4),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Luring Song — harpy's AoE charm. Every creature within 12 tiles (30ft)
/// that can hear the harpy makes a WIS save vs DC 11. On fail, target is
/// Charmed by the harpy for 3 rounds. Charm-immune creatures (undead /
/// constructs / etc.) shrug it off automatically — we let the
/// add_condition guard handle that uniformly. We use SetCharmedBy so the
/// charmed creature can't make hostile actions against the harpy.
pub struct LuringSong {}

impl Action for LuringSong {
    fn name(&self) -> &str {
        "luring song"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sing", "lure"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{ApplyCondition, SetCharmedBy};
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let caster_team = caster.team();
        const SONG_RADIUS: isize = 12;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Sorted-id iteration for deterministic save sequencing.
        for target_id in encounter.sorted_actor_ids() {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            if target.team() == caster_team || !target.is_combat_active() {
                continue;
            }
            if target.is_immune_to_condition(Condition::Charmed) {
                continue;
            }
            let dist = crate::engine::util::footprint_chebyshev(
                target.location(),
                crate::engine::util::get_tiles_from_size(target.size()),
                caster_loc,
                1,
            );
            if dist > SONG_RADIUS {
                continue;
            }
            let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, 11);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Charmed,
                timer: ConditionTimer::Rounds(3),
            }));
            effects.push(Box::new(SetCharmedBy {
                target_id,
                charmer: Some(caster_id),
            }));
        }
        effects
    }
}

pub static LURING_SONG: LazyLock<LuringSong> = LazyLock::new(|| LuringSong {});

/// Longsword — versatile 1d8 slashing melee weapon. STR-based, Action
/// cost, MELEE_REACH. Workhorse weapon for Knights and other armored
/// foot soldiers.
pub static LONGSWORD: SimpleWeapon = SimpleWeapon {
    display_name: "longsword",
    aliases: &["ls", "sword"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Greatsword — STR-based 2d6 slashing melee weapon. The paladin's
/// signature heavy weapon: bigger dice than the longsword (1d8) at the
/// cost of two-handed use, which we don't model explicitly. Pairs with
/// Divine Smite for the load-bearing burst damage.
pub static GREATSWORD: SimpleWeapon = SimpleWeapon {
    display_name: "greatsword",
    aliases: &["gs", "great-sword"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Lance — 1d12 piercing reach-2 melee weapon. Mounted-only RAW, but we
/// drop the mount gate so the Knight gets a polearm option to swing from
/// 10 ft (one tile beyond a standard sword reach).
pub static LANCE: SimpleWeapon = SimpleWeapon {
    display_name: "lance",
    aliases: &["lnc"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 12),
    damage_type: DamageType::Piercing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Knight's double-longsword multiattack — two swings per Action,
/// modeled on top of the existing Multiattack wrapper.
pub static KNIGHT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double longsword",
    sub_attack: &LONGSWORD,
    count: 2,
});

/// Gargoyle claws — 1d6+STR slashing, MELEE_REACH. Plain physical
/// attack; the gargoyle's danger comes from its multiattack and
/// resistances rather than rider effects.
pub static GARGOYLE_CLAWS: SimpleWeapon = SimpleWeapon {
    display_name: "claws",
    aliases: &["clw"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Gargoyle multiattack — claws + bite, two swings per Action. Reuses
/// the generic BITE attack (1d6+STR piercing) since the gargoyle's bite
/// doesn't have a rider in our model.
pub static GARGOYLE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "claws + bite",
    sub_attack: &GARGOYLE_CLAWS,
    count: 2,
});

/// Worg bite — 2d6+STR piercing with a Trip rider (STR save or knocked
/// Prone on a hit). Identical to a Dire Wolf's bite at a smaller damage
/// die — Worgs are mid-tier mounts that hit harder than wolves but
/// without the dire wolf's pack-tactics edge.
pub struct WorgBite {}

impl Action for WorgBite {
    fn name(&self) -> &str {
        "worg bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wbite", "worg"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::attack::{AttackParams, resolve_attack_outcome};
        use crate::engine::side_effects::ApplyCondition;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let prof = caster.proficiency_bonus();
        let attack_bonus = str_mod + prof;
        let (mut effects, dealt) = resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(2, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Piercing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if dealt == 0 {
            // Missed — no rider save.
            return effects;
        }
        // STR save vs DC 8 + STR + prof or be knocked Prone.
        let dc = 8 + str_mod + prof;
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, dc);
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

pub static WORG_BITE: LazyLock<WorgBite> = LazyLock::new(|| WorgBite {});

/// Stirge blood-drain proboscis — DEX attack, on a hit attaches to the
/// target and drains 1d4+1 piercing per turn. We approximate the
/// "attached" rider as a single 1d4+1 piercing strike per Action, with
/// a +5 to-hit (matches the MM stat block at +5).
pub static STIRGE_PROBOSCIS: SimpleWeapon = SimpleWeapon {
    display_name: "blood drain",
    aliases: &["proboscis", "drain"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Cockatrice bite — DEX-flavored melee that deals 1d4 piercing on hit
/// and, more importantly, forces a CON save (DC 11) for a "petrify"
/// rider that applies the Petrified condition for 1 round on a fail.
/// Mirrors the imp-sting "hit, then save-or-suck" shape: the petty
/// damage is the hook for the real threat, which is the lockout.
///
/// 5e's full petrification is permanent and lethal; we cap the rider at
/// `Rounds(1)` so a single hit doesn't game-over the target on a missed
/// save — combined with our action-economy / save-auto-fail clauses on
/// Petrified the round is already brutal enough.
pub struct CockatriceBite {}

impl Action for CockatriceBite {
    fn name(&self) -> &str {
        "petrifying bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cb", "petrify-bite"]
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
            "bite",
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
            encounter.log("  petrifying gaze: target turns to stone");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Petrified,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static COCKATRICE_BITE: LazyLock<CockatriceBite> = LazyLock::new(|| CockatriceBite {});

/// Wight Life Drain — melee attack, +4 to hit, 1d6+2 necrotic on hit and
/// on a failed CON save vs DC 13, the target's max HP drops by the
/// damage dealt. Distinct from the Wraith's Life Drain in stats (lower
/// damage / lower DC / lower attack mod) but mechanically symmetric;
/// reusing the same AdjustMaxHp side-effect so the long-term drain
/// behaves identically.
pub struct WightLifeDrain {}

impl Action for WightLifeDrain {
    fn name(&self) -> &str {
        "life drain"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wld", "wight-drain"]
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.ability_modifier(AbilityScoreType::Strength)
            + caster.proficiency_bonus();
        let damage_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "life drain",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(1, 6),
                damage_bonus: damage_mod,
                damage_type: DamageType::Necrotic,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        if !save.passed() {
            effects.push(Box::new(AdjustMaxHp {
                actor_id: target_id,
                delta: -(damage as i32),
            }));
        }
        effects
    }
}

pub static WIGHT_LIFE_DRAIN: LazyLock<WightLifeDrain> = LazyLock::new(|| WightLifeDrain {});

/// Minotaur Gore — melee attack, STR-based, 2d8+4 piercing on hit. The
/// minotaur's marquee charge attack — a single big slam that benefits
/// from a normal STR attack-mod but lands a notable d8 damage swing.
/// Used as the action option alongside Greataxe in the template.
pub struct MinotaurGore {}

impl Action for MinotaurGore {
    fn name(&self) -> &str {
        "gore"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gor", "horn"]
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
            "gore",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 8),
            DamageType::Piercing,
            true,
        )
    }
}

pub static MINOTAUR_GORE: LazyLock<MinotaurGore> = LazyLock::new(|| MinotaurGore {});

/// Banshee Wail — bonus-action AoE, no-target. Every non-undead creature
/// within 30 ft (radius 12) makes a CON save vs DC 13 or takes 3d6
/// psychic damage and is Frightened for 3 rounds on a fail; half
/// damage on a save (no fright). Banshees are undead so their wail
/// can't catch themselves; we filter by team to keep ally-banshees
/// (rare but possible) from chain-wailing each other.
pub struct BansheeWail {}

impl Action for BansheeWail {
    fn name(&self) -> &str {
        "wail"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wl", "scream"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
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

        const RADIUS: isize = 12;
        const DC: i32 = 13;

        let caster_loc = match encounter.actors.get(&caster_id) {
            Some(a) => a.location(),
            None => return Vec::new(),
        };

        encounter.log("  wail: a chilling shriek tears the air");
        let damage = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  wail: 3d6({}) = {} psychic (failed save) / {} half",
            damage,
            damage,
            damage / 2
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, caster_loc, RADIUS) {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            // Undead are immune to the wail (RAW: "any creature that
            // is not undead"). We proxy by checking for necrotic
            // immunity — matches the wraith / specter / wight pool.
            if t.is_immune_to(DamageType::Necrotic) {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Constitution, DC);
            let dmg = if save.passed() { damage / 2 } else { damage };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Psychic,
                }));
            }
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

pub static BANSHEE_WAIL: LazyLock<BansheeWail> = LazyLock::new(|| BansheeWail {});

/// Banshee Corrupting Touch — melee attack, +4 to hit, 3d6+2 necrotic.
/// The "I'm a ghost up close" basic attack — distinct from the
/// signature wail since the banshee's primary loop is to wail first
/// then close for finisher touches.
pub static CORRUPTING_TOUCH: SimpleWeapon = SimpleWeapon {
    display_name: "corrupting touch",
    aliases: &["ct", "touch"],
    attack_ability: AbilityScoreType::Charisma,
    damage_ability: Some(AbilityScoreType::Charisma),
    damage_dice: Dice::new(3, 6),
    damage_type: DamageType::Necrotic,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Hippogriff Beak — melee, STR-based, 1d10+3 piercing. The bigger
/// half of the hippogriff multiattack — single-strike-feels-meaty stat
/// line tuned to deliver one solid hit per swing.
pub static HIPPOGRIFF_BEAK: SimpleWeapon = SimpleWeapon {
    display_name: "beak",
    aliases: &["bk", "peck"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 10),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Hippogriff Talons — melee, STR-based, 2d6+3 slashing. Companion
/// half of the multiattack — moderately bigger dice spread for the
/// second swing per turn.
pub static HIPPOGRIFF_TALONS: SimpleWeapon = SimpleWeapon {
    display_name: "talons",
    aliases: &["tl", "claws-h"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Hippogriff multiattack — beak + talons in a single action (we
/// approximate by doubling beak; talons routed via a separate Action
/// so the AI alternates). Same `Multiattack` shape used by zombies and
/// other multi-strike creatures.
pub static HIPPOGRIFF_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "beak + talons",
    sub_attack: &HIPPOGRIFF_BEAK,
    count: 2,
});

/// Doppelganger Slam — melee, STR-based, 1d6+4 bludgeoning. Used as
/// the basic at-will attack; pairs with the multiattack for the
/// signature double-slam pattern.
pub static DOPPELGANGER_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "slam",
    aliases: &["dslam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Doppelganger multiattack — 2 slams per Action. Vanilla shape, but
/// the doppelganger's high DEX (and template-side Charm immunity in
/// the creature file) gives the encounter a different feel from a
/// zombie multislam.
pub static DOPPELGANGER_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double slam",
    sub_attack: &DOPPELGANGER_SLAM,
    count: 2,
});

/// Mummy Rotting Fist — STR-based melee, +5 to hit, 2d6+3 bludgeoning
/// plus 3d6 necrotic on hit. The necrotic packet rides regardless of
/// damage-type resistance on the bludgeoning core, so resistant targets
/// still feel the rot. Doesn't carry the mummy-rot disease (we don't
/// model long-form curses) — the necrotic packet is the load-bearing
/// rider.
pub struct MummyRottingFist {}

impl Action for MummyRottingFist {
    fn name(&self) -> &str {
        "rotting fist"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rf", "rot"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Necrotic]
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
        let attack_mod = caster.ability_modifier(AbilityScoreType::Strength)
            + caster.proficiency_bonus();
        let damage_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "rotting fist",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(2, 6),
                damage_bonus: damage_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
        // Necrotic rider: 3d6 typed separately so resistance is checked
        // independently. No additional roll vs AC — the rider rides the
        // hit.
        let necrotic = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  rotting fist: 3d6({}) = {} necrotic rider",
            necrotic, necrotic
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: necrotic,
            damage_type: DamageType::Necrotic,
        }));
        effects
    }
}

pub static MUMMY_ROTTING_FIST: LazyLock<MummyRottingFist> =
    LazyLock::new(|| MummyRottingFist {});

/// Dreadful Glare — mummy's signature gaze attack. Targets every enemy
/// within radius 8 (40 ft) that has line-of-sight to the mummy: WIS
/// save vs DC 11 or be Frightened of the mummy for 1 minute (10
/// rounds). Action cost. Undead are unaffected (we filter by necrotic
/// immunity, the standard undead proxy).
pub struct MummyDreadfulGlare {}

impl Action for MummyDreadfulGlare {
    fn name(&self) -> &str {
        "dreadful glare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dg", "glare"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn deals_damage(&self) -> bool {
        false
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
        const RADIUS: isize = 8;
        const DC: i32 = 11;

        let caster_loc = match encounter.actors.get(&caster_id) {
            Some(a) => a.location(),
            None => return Vec::new(),
        };
        encounter.log("  dreadful glare: the mummy fixes its hollow eyes on the living");

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, caster_loc, RADIUS) {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if t.is_immune_to(DamageType::Necrotic) {
                continue;
            }
            // Requires line-of-sight — a gaze can't bend around corners.
            if !encounter.actor_has_line_of_sight(caster_id, tid) {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static MUMMY_DREADFUL_GLARE: LazyLock<MummyDreadfulGlare> =
    LazyLock::new(|| MummyDreadfulGlare {});

/// Berserker Greataxe — STR-based melee with 1d12+STR slashing. Distinct
/// from the bare GREATAXE in that it's wrapped in an Action impl so the
/// berserker can pair it with its self-buffing "Reckless" stance in
/// future work. Today this is a vanilla greataxe; left as a wrapper
/// for symmetry with the rest of the per-creature attack files.
pub static BERSERKER_GREATAXE: SimpleWeapon = SimpleWeapon {
    display_name: "berserker greataxe",
    aliases: &["bgx", "berserker-axe"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 12),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Reckless Attack — berserker class feature. Free no-cost self-flag:
/// applies the `Helped` condition to the caster (granting advantage on
/// their next melee attack roll this turn), but at the cost of every
/// attacker against them getting `Outlined` for one round (granting
/// advantage on attacks vs the berserker). Models 5e barbarian
/// recklessness — advantage trades for being easier to hit until
/// the start of their next turn.
///
/// Bonus action so the berserker can still swing their greataxe with
/// the resulting Helped advantage on the same turn.
pub struct RecklessAttack {}

impl Action for RecklessAttack {
    fn name(&self) -> &str {
        "reckless attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["reck", "reckless"]
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
        bonus_action_only()
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        // Helped expires on use (clear_attack_advantage_riders), so the
        // next melee swing this turn benefits. Outlined gives attackers
        // advantage vs the berserker until the start of their next turn.
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Helped,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Outlined,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
        ]
    }
}

pub static RECKLESS_ATTACK: LazyLock<RecklessAttack> = LazyLock::new(|| RecklessAttack {});

/// Veteran Longsword — STR-based 1d8 slashing. The veteran's primary
/// melee weapon, paired with a shortsword in multiattack.
pub static VETERAN_LONGSWORD: SimpleWeapon = SimpleWeapon {
    display_name: "longsword",
    aliases: &["ls"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Veteran multiattack — 2 longsword swings per Action. The veteran
/// is the workhorse human soldier: two big swings of a 1d8 weapon
/// outperform the bandit captain's three scimitar swings on average
/// (8.5 vs ~3.5 per swing), without a rider.
pub static VETERAN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double longsword",
    sub_attack: &VETERAN_LONGSWORD,
    count: 2,
});

/// Yeti Claws — STR-based melee, 1d6+STR slashing + 1d6 cold rider on
/// hit. The cold rider plays through resistance separately, like the
/// mummy's necrotic rider, so a fire-resistant target still eats the
/// chill.
pub struct YetiClaws {}

impl Action for YetiClaws {
    fn name(&self) -> &str {
        "yeti claws"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["yc", "yeti"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Cold]
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
        let attack_mod = caster.ability_modifier(AbilityScoreType::Strength)
            + caster.proficiency_bonus();
        let damage_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "yeti claws",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(1, 6),
                damage_bonus: damage_mod,
                damage_type: DamageType::Slashing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
        let cold = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!(
            "  yeti claws: 1d6({}) = {} cold rider",
            cold, cold
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: cold,
            damage_type: DamageType::Cold,
        }));
        effects
    }
}

pub static YETI_CLAWS: LazyLock<YetiClaws> = LazyLock::new(|| YetiClaws {});

/// Yeti multiattack — two claw swings per Action. With the cold rider
/// on each hit, this is comparable to a small-ice-elemental loop —
/// punchy on bare-skin targets but blunted by cold resistance.
pub static YETI_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double claws",
    sub_attack: &*YETI_CLAWS,
    count: 2,
});

/// Chilling Gaze — yeti's signature gaze attack. Targets a single
/// creature within radius 6 (30 ft); CON save vs DC 13 or take 3d6 cold
/// damage *and* be Paralyzed for 1 minute (10 rounds). Cold-immune or
/// blindfolded creatures are immune to the gaze (we proxy "blindfolded"
/// by checking the Blinded condition on the target). Action cost.
pub struct ChillingGaze {}

impl Action for ChillingGaze {
    fn name(&self) -> &str {
        "chilling gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cg", "gaze"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
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
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        const DC: i32 = 13;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Blinded targets can't be affected by gaze attacks (their eyes
        // are squeezed shut). Cold-immune ones shrug off the rider.
        if let Some(t) = encounter.actors.get(&target_id) {
            if t.has_condition(Condition::Blinded) {
                encounter.log("  chilling gaze: target's eyes are shut \u{2014} no effect");
                return Vec::new();
            }
            if t.is_immune_to(DamageType::Cold) {
                encounter.log("  chilling gaze: target is immune to cold \u{2014} no effect");
                return Vec::new();
            }
        } else {
            return Vec::new();
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, DC);
        if save.passed() {
            return Vec::new();
        }
        let damage = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  chilling gaze: 3d6({}) = {} cold + paralyzed",
            damage, damage
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: damage,
                damage_type: DamageType::Cold,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static CHILLING_GAZE: LazyLock<ChillingGaze> = LazyLock::new(|| ChillingGaze {});

/// Manticore Tail Spikes — ranged attack, 6 spike volley collapsed into
/// a single 3d8 piercing roll with +DEX to hit and damage. RAW the
/// manticore can fire up to four spikes per Action; we model the volley
/// as a single attack roll with combined damage to keep the action
/// economy tight (one Action → one rolled outcome) while preserving the
/// "ranged threat at high CR" flavor. Reach 12 tiles (≈30 ft); not melee,
/// requires LOS like every other ranged attack.
pub struct ManticoreSpikes {}

impl Action for ManticoreSpikes {
    fn name(&self) -> &str {
        "tail spikes"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "spikes"]
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "tail spikes",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(3, 8),
            DamageType::Piercing,
            false,
        )
    }
}

pub static MANTICORE_SPIKES: LazyLock<ManticoreSpikes> = LazyLock::new(|| ManticoreSpikes {});

/// Manticore Multiattack — Action: bite (1d8 piercing) + two claws
/// (1d6 slashing each). All strikes share the same target. This is the
/// melee half of the manticore's kit — the ranged Tail Spikes covers the
/// stand-off lane.
pub struct ManticoreMultiattack {}

impl Action for ManticoreMultiattack {
    fn name(&self) -> &str {
        "manticore multiattack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mm", "claws"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Slashing]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut all = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "manticore bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 8),
            DamageType::Piercing,
            true,
        );
        for _ in 0..2 {
            all.extend(simple_weapon_attack(
                encounter,
                caster_id,
                target_ids,
                "manticore claw",
                AbilityScoreType::Strength,
                Some(AbilityScoreType::Strength),
                Dice::new(1, 6),
                DamageType::Slashing,
                true,
            ));
        }
        all
    }
}

pub static MANTICORE_MULTIATTACK: LazyLock<ManticoreMultiattack> =
    LazyLock::new(|| ManticoreMultiattack {});

/// Hill Giant Greatclub — STR-based 3d8 bludgeoning, reach 2 tiles
/// (10 ft). Mirrors the ogre's club but bumped to giant-tier dice; the
/// extra reach is the hill giant's signature spacing advantage.
pub static HILL_GIANT_GREATCLUB: SimpleWeapon = SimpleWeapon {
    display_name: "giant greatclub",
    aliases: &["ggc", "giant-club"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 8),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Hill Giant Boulder — STR-based 3d10 bludgeoning thrown rock with
/// reach 24 (60 ft). Ranged STR throw is unusual but matches the 5e
/// stat block: giants chuck rocks for big damage at long range.
pub static HILL_GIANT_BOULDER: SimpleWeapon = SimpleWeapon {
    display_name: "boulder",
    aliases: &["bld", "rock"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 10),
    damage_type: DamageType::Bludgeoning,
    reach: 24,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
    normal_range: Some(16),
};

/// Treant Slam — STR-based 3d6 bludgeoning, reach 2 (10 ft). The treant
/// is a slow CR-9 wall of HP that swings massive trunks; 3d6+STR per
/// strike, no rider, but the Treant template attaches the Multiattack
/// wrapper to swing twice per Action.
pub static TREANT_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "treant slam",
    aliases: &["tslam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 6),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Treant Multiattack — Action: two Treant Slam swings against the same
/// target. The pair of 3d6+STR slams averages ~25 damage at the treant's
/// stat block — eats through PCs in a couple of rounds and gives the
/// CR-9 frame a believable threat profile.
pub struct TreantMultiattack {}

impl Action for TreantMultiattack {
    fn name(&self) -> &str {
        "treant multiattack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tm", "double-slam"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut all = Vec::new();
        for _ in 0..2 {
            all.extend(simple_weapon_attack(
                encounter,
                caster_id,
                target_ids,
                "treant slam",
                AbilityScoreType::Strength,
                Some(AbilityScoreType::Strength),
                Dice::new(3, 6),
                DamageType::Bludgeoning,
                true,
            ));
        }
        all
    }
}

pub static TREANT_MULTIATTACK: LazyLock<TreantMultiattack> =
    LazyLock::new(|| TreantMultiattack {});

/// Fire Elemental Touch — melee, +DEX to hit, 2d6 fire damage and the
/// target is ignited (Burning, 3 rounds). The elemental's whole-body
/// touch is the signature "stand next to me and you'll cook" mechanic.
/// Fire-immune targets take no damage and skip the ignition; we let the
/// DealDamage path's modifier handle the immunity and apply the Burning
/// condition gated on whether the target is fire-immune (so a fire
/// elemental brushing another fire creature doesn't burst it into
/// flames).
pub struct FireElementalTouch {}

impl Action for FireElementalTouch {
    fn name(&self) -> &str {
        "fire touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ft", "burn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
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
            "fire touch",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(2, 6),
            DamageType::Fire,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // No ignition for fire-immune targets — the Burning DOT is also
        // fire-typed and would tick to 0 anyway, but skipping the
        // ApplyCondition keeps the log clean.
        let immune = encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_immune_to(DamageType::Fire));
        if !immune {
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Burning,
                timer: ConditionTimer::Rounds(3),
            }));
        }
        effects
    }
}

pub static FIRE_ELEMENTAL_TOUCH: LazyLock<FireElementalTouch> =
    LazyLock::new(|| FireElementalTouch {});

/// Gelatinous Cube Pseudopod — melee, slow attack, 3d6 acid on hit and
/// on a failed DC 12 STR save the target is Restrained (the cube has
/// engulfed them). The Restrained ends when the cube dies or the target
/// breaks free — modeled by a 5-round timer here, long enough to mimic
/// the engulf duration without locking the target forever if the cube
/// can't be killed in time.
pub struct GelatinousCubeEngulf {}

impl Action for GelatinousCubeEngulf {
    fn name(&self) -> &str {
        "pseudopod"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gp", "engulf"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
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
            "pseudopod",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(3, 6),
            DamageType::Acid,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 12);
        if !save.passed() {
            encounter.log("  pseudopod: target is engulfed and restrained");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Restrained,
                timer: ConditionTimer::Rounds(5),
            }));
        }
        effects
    }
}

pub static GELATINOUS_CUBE_ENGULF: LazyLock<GelatinousCubeEngulf> =
    LazyLock::new(|| GelatinousCubeEngulf {});

/// Dragon Fire Breath — Adult Red Dragon signature. Burst-4 radius
/// (range 6) of searing flame. Every creature in the area makes a DEX
/// save vs DC 21: failed save takes 12d6 fire, success halves. Gated
/// behind the "breath_weapon" recharge ability (Recharge 5-6): the
/// custom_validate_input check blocks the action when spent, and
/// side_effects calls spend_recharge so the dragon must wait for the
/// start-of-turn d6 roll to get it back.
pub struct DragonBreathFire {}

impl Action for DragonBreathFire {
    fn name(&self) -> &str {
        "fire breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fb", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
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
            .is_some_and(|a| a.is_recharge_available("breath_weapon"))
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
        // Spend the recharge resource before resolving damage.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        const DC: i32 = 21;
        let raw = encounter.roll(&Dice::new(12, 6));
        encounter.log(format!(
            "  fire breath: 12d6({}) = {} fire area (DC {} DEX, half on save)",
            raw, raw, DC
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Fire,
        )
    }
}

pub static DRAGON_BREATH_FIRE: LazyLock<DragonBreathFire> =
    LazyLock::new(|| DragonBreathFire {});

/// Keep the old name around as an alias so existing templates that
/// reference `DRAGON_FIRE_BREATH` still compile. Points to the same
/// action with recharge gating.
pub static DRAGON_FIRE_BREATH: LazyLock<DragonBreathFire> =
    LazyLock::new(|| DragonBreathFire {});

/// Dragon Cold Breath — burst-4 radius (range 6) of freezing cold.
/// 12d6 cold, DEX save DC 21 for half. Recharge 5-6.
pub struct DragonBreathCold {}

impl Action for DragonBreathCold {
    fn name(&self) -> &str {
        "cold breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cb", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
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
            .is_some_and(|a| a.is_recharge_available("breath_weapon"))
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
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        const DC: i32 = 21;
        let raw = encounter.roll(&Dice::new(12, 6));
        encounter.log(format!(
            "  cold breath: 12d6({}) = {} cold area (DC {} DEX, half on save)",
            raw, raw, DC
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Cold,
        )
    }
}

pub static DRAGON_BREATH_COLD: LazyLock<DragonBreathCold> =
    LazyLock::new(|| DragonBreathCold {});

/// Dragon Lightning Breath — burst-4 radius (range 6) of crackling
/// lightning. 12d6 lightning, DEX save DC 21 for half. Recharge 5-6.
pub struct DragonBreathLightning {}

impl Action for DragonBreathLightning {
    fn name(&self) -> &str {
        "lightning breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lb", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
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
            .is_some_and(|a| a.is_recharge_available("breath_weapon"))
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
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        const DC: i32 = 21;
        let raw = encounter.roll(&Dice::new(12, 6));
        encounter.log(format!(
            "  lightning breath: 12d6({}) = {} lightning area (DC {} DEX, half on save)",
            raw, raw, DC
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Lightning,
        )
    }
}

pub static DRAGON_BREATH_LIGHTNING: LazyLock<DragonBreathLightning> =
    LazyLock::new(|| DragonBreathLightning {});

/// Dragon Poison Breath — burst-4 radius (range 6) of noxious gas.
/// 12d6 poison, CON save DC 21 for half. Recharge 5-6. Note: poison
/// breath uses a CON save (inhaled toxin) rather than the DEX save
/// used by the elemental breath weapons.
pub struct DragonBreathPoison {}

impl Action for DragonBreathPoison {
    fn name(&self) -> &str {
        "poison breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pb", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
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
            .is_some_and(|a| a.is_recharge_available("breath_weapon"))
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
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        const DC: i32 = 21;
        let raw = encounter.roll(&Dice::new(12, 6));
        encounter.log(format!(
            "  poison breath: 12d6({}) = {} poison area (DC {} CON, half on save)",
            raw, raw, DC
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            DC,
            raw,
            DamageType::Poison,
        )
    }
}

pub static DRAGON_BREATH_POISON: LazyLock<DragonBreathPoison> =
    LazyLock::new(|| DragonBreathPoison {});

/// Dragon Bite — Adult Red Dragon's signature melee. d20 + 14 vs AC
/// (STR+prof at CR 17), on hit 2d10+8 piercing + 4d6 fire. The fire
/// rider is a separate `DealDamage` so per-target resistance / immunity
/// applies to it independently from the piercing.
pub struct DragonBite {}

impl Action for DragonBite {
    fn name(&self) -> &str {
        "dragon bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bite-d", "dbite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10 ft reach (Large dragon) — 2 tiles.
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Fire]
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
        // Piercing bite first; on hit, layer 4d6 fire as a separate
        // DealDamage so immunity / resistance applies to each pass.
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod =
            caster.ability_modifier(AbilityScoreType::Strength)
                + caster.proficiency_bonus();
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "dragon bite",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(2, 10),
                damage_bonus: str_mod,
                damage_type: DamageType::Piercing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
        let fire = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!("  dragon bite: 4d6({}) = {} fire rider", fire, fire));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: fire,
            damage_type: DamageType::Fire,
        }));
        effects
    }
}

pub static DRAGON_BITE: LazyLock<DragonBite> = LazyLock::new(|| DragonBite {});

/// Dragon Claw — Adult Red Dragon's swipe. Identical resolution to a
/// `SimpleWeapon` (no rider), tuned to 2d6+8 slashing at the dragon's
/// hit modifier. Two claws + bite = the dragon multiattack; we issue
/// the data-only SimpleWeapon variant so the AI picks Bite for the
/// fire rider and Claw as fallback.
pub static DRAGON_CLAW: SimpleWeapon = SimpleWeapon {
    display_name: "dragon claw",
    aliases: &["dclaw", "claw-d"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    is_melee: true,
    reach: 2,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Lich Paralyzing Touch — touch attack with a paralysis rider. d20 +
/// 12 (INT-cast attack mod at CR 21) vs AC. On hit: 3d6 cold and the
/// target makes a CON save vs DC 18 or is Paralyzed for 5 rounds.
/// Pairs with the rest of the lich kit (Power Word Kill, Finger of
/// Death) for a high-control boss profile.
pub struct LichParalyzingTouch {}

impl Action for LichParalyzingTouch {
    fn name(&self) -> &str {
        "paralyzing touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pt", "lichtouch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Lich uses INT for attack mod (caster bonus action stat).
        let attack_mod =
            caster.ability_modifier(AbilityScoreType::Intelligence)
                + caster.proficiency_bonus();
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "paralyzing touch",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(3, 6),
                damage_bonus: 0,
                damage_type: DamageType::Cold,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if damage == 0 {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 18);
        if !save.passed() {
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
                timer: ConditionTimer::Rounds(5),
            }));
        }
        effects
    }
}

pub static LICH_PARALYZING_TOUCH: LazyLock<LichParalyzingTouch> =
    LazyLock::new(|| LichParalyzingTouch {});

/// Beholder Eye Ray — generic 4d8 force eye-ray. Range 120 ft (48
/// tiles). Single target; spell-attack-style roll vs AC at +9 (INT-prof
/// at CR 13). Light wrapper around `simple_weapon_attack` with a longer
/// reach so the AI considers it a ranged option in addition to bites.
pub struct BeholderEyeRay {}

impl Action for BeholderEyeRay {
    fn name(&self) -> &str {
        "eye ray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["er", "eye"]
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "eye ray",
            AbilityScoreType::Intelligence,
            None,
            Dice::new(4, 8),
            DamageType::Force,
            false,
        )
    }
}

pub static BEHOLDER_EYE_RAY: LazyLock<BeholderEyeRay> = LazyLock::new(|| BeholderEyeRay {});

/// Dragon Multiattack — 3 claw swings in one turn at a single target
/// (a tight stand-in for the Adult Red Dragon's "Bite + 2 Claws" RAW
/// pattern using the existing single-sub-attack Multiattack scaffold).
/// The dragon also has a separate Bite action and Fire Breath option
/// the AI picks between, so the multiattack is the bursty melee lane.
pub static DRAGON_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "dragon multiattack",
    sub_attack: &DRAGON_CLAW,
    count: 3,
});

/// Drow Poisoned Hand Crossbow — DEX-based 1d6 piercing ranged shot with
/// 30/120 ft range (≈12 tiles), Action cost. On hit, the target makes a
/// CON save DC 13: fail = additional 2d4 poison damage and Poisoned for
/// 2 rounds (we collapse the 5e "unconscious for 1 hour on fail-by-5"
/// clause into a simple Poisoned). Mirrors the Drow's signature
/// crossbow-and-venom pattern from the Monster Manual.
pub struct DrowPoisonedCrossbow {}

impl Action for DrowPoisonedCrossbow {
    fn name(&self) -> &str {
        "poisoned hand crossbow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["phcb", "drowbow"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Poison]
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
            Dice::new(1, 6),
            DamageType::Piercing,
            false,
        );
        // Miss = no rider.
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        if !save.passed() {
            let poison = encounter.roll(&Dice::new(2, 4));
            encounter.log(format!(
                "  drow poison: 2d4({}) = {} poison",
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

pub static DROW_POISONED_CROSSBOW: LazyLock<DrowPoisonedCrossbow> =
    LazyLock::new(|| DrowPoisonedCrossbow {});

/// Frost Giant Greataxe — STR-based 3d12 slashing melee, reach 2 (10 ft).
/// One of the heaviest single-swing weapons in the bestiary: dice on par
/// with the Hill Giant's club but cycled into slashing damage to keep
/// damage-type variety on the giant tier. CR-8 numbers.
pub static FROST_GIANT_GREATAXE: SimpleWeapon = SimpleWeapon {
    display_name: "frost giant greataxe",
    aliases: &["fgx", "frost-axe"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 12),
    damage_type: DamageType::Slashing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Frost Giant Rock — STR-based 4d10 bludgeoning thrown rock at reach
/// 24 (60 ft). Frost Giants chuck boulders like Hill Giants but harder
/// — the extra die is the CR-8 vs CR-5 step. Same template as the Hill
/// Giant Boulder.
pub static FROST_GIANT_ROCK: SimpleWeapon = SimpleWeapon {
    display_name: "frost rock",
    aliases: &["frock"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 10),
    damage_type: DamageType::Bludgeoning,
    reach: 24,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
    normal_range: Some(16),
};

/// Vampire Charming Gaze — Action. Target within 30ft makes a WIS save
/// vs DC 17 (vampire's CHA-based spell DC). Fail = Charmed for 1 minute
/// (10 rounds in our model), and the SetCharmedBy linkage points the
/// target back at the vampire so they can't attack their charmer.
/// Mirrors the structure of MummyDreadfulGlare but Charmed instead of
/// Frightened, single-target (the vampire picks a juicy victim) instead
/// of AoE. RAW gives a "no save again until damaged" clause; we honor
/// it via the 10-round duration and let damage / dispel break the
/// condition naturally.
pub struct VampireCharmingGaze {}

impl Action for VampireCharmingGaze {
    fn name(&self) -> &str {
        "charming gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cg", "gaze"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{ApplyCondition, SetCharmedBy};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Charm-immune creatures (undead / constructs) shrug it off.
        if let Some(target) = encounter.actors.get(&target_id)
            && target.is_immune_to_condition(Condition::Charmed)
        {
            encounter.log("  charming gaze: target is immune".to_string());
            return Vec::new();
        }
        const DC: i32 = 17;
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, DC);
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

pub static VAMPIRE_CHARMING_GAZE: LazyLock<VampireCharmingGaze> =
    LazyLock::new(|| VampireCharmingGaze {});

/// Vampire Multiattack — Action: two vampiric bites at the same target.
/// Re-uses the generic `Multiattack` wrapper around the existing
/// VAMPIRIC_BITE so the lifesteal payoff layers twice without a bespoke
/// Action impl. The vampire's tempo is "charm one ally, then drink from
/// the held victim"; the gaze stays a separate action so the AI can
/// interleave the lockdown.
pub static VAMPIRE_MULTIATTACK: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "vampire multiattack",
    sub_attack: &*VAMPIRIC_BITE,
    count: 2,
});

/// Couatl's Constricting Bite — STR-based 1d6+4 piercing on hit plus a
/// 3d6 poison rider (no save, like the SRD couatl's poison clause). The
/// target also makes a CON save (caster DC) or is Poisoned for up to 10
/// rounds. Reach melee.
pub struct CouatlBite {}

impl Action for CouatlBite {
    fn name(&self) -> &str {
        "couatl bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["couatl"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let attack_mod = str_mod + caster.proficiency_bonus();
        // Primary bite: standard weapon attack roll.
        let mut effects = resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "couatl bite",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(1, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Piercing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        // 5e RAW: poison rider applies on hit only — bail if the bite missed.
        if effects.is_empty() {
            return effects;
        }
        // Poison rider: 3d6 poison + CON save or Poisoned (10 rounds).
        let raw = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!("  couatl bite poison: 3d6({}) poison", raw));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Poison,
        }));
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return effects;
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Constitution);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static COUATL_BITE: LazyLock<CouatlBite> = LazyLock::new(|| CouatlBite {});

/// Couatl's Sleep Gaze — celestial sleep at 30ft (12 tiles). Single
/// target makes a WIS save vs the couatl's WIS-based DC; on fail, the
/// target is Asleep for 10 rounds. Damage wakes the sleeper via the
/// existing DealDamage hook. Unlike Vampire Charming Gaze, Sleep Gaze
/// ignores Charm-immunity but is gated by Sleep-immunity (we route
/// through the standard Asleep condition; immune undead skip silently).
pub struct CouatlSleepGaze {}

impl Action for CouatlSleepGaze {
    fn name(&self) -> &str {
        "sleep gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["slumber", "sg"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
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
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Asleep,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static COUATL_SLEEP_GAZE: LazyLock<CouatlSleepGaze> = LazyLock::new(|| CouatlSleepGaze {});

/// Pit Fiend's Bite — colossal 4d6+8 piercing plus a 3d6 poison rider
/// on hit. The poison damage applies regardless of save (the MM pit
/// fiend's bite is "magical, plus 21 (6d6) poison"). Reach 1 tile
/// (5ft); the pit fiend has reach 2 for its other natural attacks RAW
/// but its bite is the standard 5ft.
pub static PIT_FIEND_BITE: SimpleWeapon = SimpleWeapon {
    display_name: "pit fiend bite",
    aliases: &["pf-bite"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 6),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Pit Fiend's Devil Claw — STR-based 2d8+8 slashing. The companion
/// melee attack to the bite; together they make up the pit fiend's
/// 4-attack multiattack (1 bite + 1 claw + 1 mace + 1 tail in MM RAW).
/// We collapse to bite+claw bursting via the Multiattack wrapper below.
pub static PIT_FIEND_CLAW: SimpleWeapon = SimpleWeapon {
    display_name: "devil claw",
    aliases: &["pf-claw"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 8),
    damage_type: DamageType::Slashing,
    reach: 2, // 10ft reach — the pit fiend's natural reach for non-bite limbs.
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Pit Fiend Multiattack — Action: 1 bite + 2 devil-claw swings,
/// expressed as a single heterogeneous CompoundAttack so the boss's
/// signature mixed-limb burst lands in one action pick (rather than
/// the AI alternating between separate bite / claw multis). RAW
/// gives the pit fiend four attacks; we trim to three to keep the
/// per-turn ceiling tense rather than TPK-machine against level-3
/// PCs.
pub static PIT_FIEND_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "pit fiend multiattack",
    parts: vec![(&PIT_FIEND_BITE, 1), (&PIT_FIEND_CLAW, 2)],
});

/// Pit Fiend's Fear Aura — Action that radiates dread within 20ft (8
/// tiles). Every hostile combat-active creature in range makes a WIS
/// save vs the pit fiend's CHA-based DC; on fail, they're Frightened
/// for 10 rounds. The aura is gated as an explicit Action rather than
/// a passive on-arrival check so the AI can pick when to fire it —
/// usually round 1 when the most allies are still healthy. Mirrors
/// Banshee Wail's "burst-save → condition" shape, but the on-fail
/// effect is Frightened instead of damage.
pub struct PitFiendFearAura {}

impl Action for PitFiendFearAura {
    fn name(&self) -> &str {
        "fear aura"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fa", "aura"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn deals_damage(&self) -> bool {
        false
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let caster_loc = caster.location();
        encounter.log(format!(
            "  fear aura: 20ft burst (DC {} WIS save).",
            dc
        ));
        let candidates = encounter.enemy_burst_targets(caster_id, caster_loc, 8);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in candidates {
            let save = encounter.roll_save(id, AbilityScoreType::Wisdom, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static PIT_FIEND_FEAR_AURA: LazyLock<PitFiendFearAura> = LazyLock::new(|| PitFiendFearAura {});

/// Monk's Martial Arts Strike — DEX-based 1d8+DEX bludgeoning unarmed
/// strike. The signature monk attack: finesse (uses DEX over STR),
/// scales with monk level via the martial-arts die (RAW: 1d4 → 1d6
/// → 1d8 → 1d10). We use a fixed 1d8 to model a mid-level monk
/// (level 5+ baseline). Melee reach.
pub static MONK_UNARMED_STRIKE: SimpleWeapon = SimpleWeapon {
    display_name: "martial arts",
    aliases: &["ma-strike", "unarmed", "punch"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Tarrasque Bite — STR-based 4d12+10 piercing, 10ft reach. The
/// signature one-shot of the apex 5e creature. Hit modifier scales off
/// the tarrasque's massive STR (30 → +10 + prof 9 = +19 RAW; we let
/// the engine compute the modifier from STR + prof so the boss's stat
/// block stays authoritative).
pub static TARRASQUE_BITE: SimpleWeapon = SimpleWeapon {
    display_name: "tarrasque bite",
    aliases: &["t-bite", "tbite"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 12),
    damage_type: DamageType::Piercing,
    reach: 4, // 15ft reach — gargantuan natural reach for the bite.
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Tarrasque Claw — STR-based 3d8 slashing. Companion melee that fills
/// out the multiattack with two swings per Action. Reach matches the
/// tarrasque's body footprint (10ft for the claws — slightly shorter
/// than the bite's 15ft).
pub static TARRASQUE_CLAW: SimpleWeapon = SimpleWeapon {
    display_name: "tarrasque claw",
    aliases: &["t-claw", "tclaw"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 8),
    damage_type: DamageType::Slashing,
    reach: 3, // 10ft reach for the claw lanes.
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Tarrasque Tail Sweep — STR-based 3d8 bludgeoning + Prone-on-hit. The
/// sweep lands at the tarrasque's far edge so the reach is generous; on
/// a successful hit the target is knocked Prone (RAW: STR save half /
/// prone; we simplify to "hit also prones" so the engine doesn't double
/// up the swing's d20 with a save). One sub-attack of the full multi.
pub struct TarrasqueTail {}

impl Action for TarrasqueTail {
    fn name(&self) -> &str {
        "tail sweep"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["t-tail", "sweep"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(4) // 20ft reach for the tail.
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod =
            caster.ability_modifier(AbilityScoreType::Strength)
                + caster.proficiency_bonus();
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, dmg) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "tail sweep",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(3, 8),
                damage_bonus: str_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if dmg > 0 {
            // Knock prone on hit — Permanent timer so standing back up
            // costs the target half movement next turn.
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static TARRASQUE_TAIL: LazyLock<TarrasqueTail> = LazyLock::new(|| TarrasqueTail {});

/// Tarrasque Multiattack — Action: 1 bite + 2 claws + 1 tail sweep.
/// Heterogeneous compound so the tarrasque issues a single burst per
/// turn instead of ping-ponging between separate multis. Numbers tuned
/// to keep the 4-attack burst spirit of MM RAW while skipping the
/// Gore + Horns separate lanes (we collapse to bite-as-piercing).
pub static TARRASQUE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "tarrasque multiattack",
    parts: vec![
        (&TARRASQUE_BITE, 1),
        (&TARRASQUE_CLAW, 2),
        (&*TARRASQUE_TAIL, 1),
    ],
});

/// Aboleth Tentacle — STR-based 2d6 bludgeoning melee, reach 2 tiles
/// (10ft). Iconic MM aboleth attack — paired with the tentacle multi
/// below to deal a brutal melee burst out at near-reach distance.
pub static ABOLETH_TENTACLE: SimpleWeapon = SimpleWeapon {
    display_name: "tentacle",
    aliases: &["tent"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Aboleth Multiattack — 3 tentacle swings per Action. Single same-sub
/// pattern through the `Multiattack` wrapper.
pub static ABOLETH_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "aboleth multiattack",
    sub_attack: &ABOLETH_TENTACLE,
    count: 3,
});

/// Solar Slaying Longsword — STR-based 4d8+8 slashing melee with a
/// permanent +1d6 radiant rider on hit (the angelic weapon glows). The
/// rider runs through the same on-hit damage pipeline as the on_hit_riders
/// table but is baked into the attack itself rather than into a
/// concentration condition, since the Solar always wields it.
pub struct SolarLongsword {}

impl Action for SolarLongsword {
    fn name(&self) -> &str {
        "slaying longsword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sl-sword"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Radiant]
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
        let attack_mod =
            caster.ability_modifier(AbilityScoreType::Strength)
                + caster.proficiency_bonus();
        let damage_mod = caster.ability_modifier(AbilityScoreType::Strength);
        // Roll the swing through the standard pipeline so cover / mirror
        // image / sanctuary all apply, then layer the radiant rider as
        // a separate DealDamage so the target's per-type modifiers
        // honor each component independently.
        let mut effects = resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "slaying longsword",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(4, 8),
                damage_bonus: damage_mod,
                damage_type: DamageType::Slashing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        // Only fire the radiant rider on a successful hit. We detect
        // success via the side-effects list being non-empty (resolve_attack
        // returns no DealDamage on a miss).
        if !effects.is_empty() {
            let rad = encounter.roll(&Dice::new(1, 6));
            encounter.log(format!(
                "  slaying longsword: +{} extra Radiant (angelic glow)",
                rad
            ));
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: rad,
                damage_type: DamageType::Radiant,
            }));
        }
        effects
    }
}

pub static SOLAR_LONGSWORD: LazyLock<SolarLongsword> = LazyLock::new(|| SolarLongsword {});

/// Solar Multiattack — 2 slaying-longsword swings per Action. Both
/// swings deal the radiant rider. Used by the SOLAR_TEMPLATE.
pub static SOLAR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "solar multiattack",
    sub_attack: &*SOLAR_LONGSWORD,
    count: 2,
});

/// Mind Flayer's Mind Blast — Action; 60-foot cone of psychic energy.
/// Every creature in the burst makes an INT save vs the flayer's
/// INT-based DC; on fail, take 4d8 psychic and become Stunned until the
/// end of the flayer's next turn. On pass, half damage and no stun.
/// We resolve the cone as a `radius: 6` burst centered on a targeted
/// tile (the flayer aims) — consistent with how Dragon Fire Breath is
/// modeled. Allies in the cone are spared via `enemy_burst_targets`.
pub struct MindFlayerMindBlast {}

impl Action for MindFlayerMindBlast {
    fn name(&self) -> &str {
        "mind blast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mb", "blast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
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
    // 5e mind flayer's Mind Blast is a recharge 5-6 ability; we
    // collapse to a regular Action (the trait default) with no recharge
    // gate (the AI already paces it via higher-leverage gating heuristics).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll(&Dice::new(4, 8));
        encounter.log(format!(
            "  mind blast: 4d8({}) = {} psychic cone (DC {} INT)",
            raw, raw, dc
        ));
        // Enemy-only — we don't want the flayer Stunning its illithid
        // allies that share the same team.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in encounter.enemy_burst_targets(caster_id, point, 6) {
            let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: target_id,
                    amount: dmg,
                    damage_type: DamageType::Psychic,
                }));
            }
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static MIND_FLAYER_MIND_BLAST: LazyLock<MindFlayerMindBlast> =
    LazyLock::new(|| MindFlayerMindBlast {});

/// Mind Flayer's Tentacles — STR-based 2d10+1 psychic melee. On hit, the
/// target makes an INT save vs the flayer's INT-DC; on fail, the target
/// is grappled by the tentacles. We approximate the grapple with the
/// existing `Adhered` condition (zero movement) since we don't model
/// the "extract brain" follow-up. Reach 1 (5ft).
pub struct MindFlayerTentacles {}

impl Action for MindFlayerTentacles {
    fn name(&self) -> &str {
        "tentacles"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tent", "mf-tentacles"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let attack_mod = str_mod + caster.proficiency_bonus();
        let mut effects = resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "tentacles",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(2, 10),
                damage_bonus: str_mod,
                damage_type: DamageType::Psychic,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return effects;
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, dc);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Adhered,
                timer: ConditionTimer::Rounds(3),
            }));
        }
        effects
    }
}

pub static MIND_FLAYER_TENTACLES: LazyLock<MindFlayerTentacles> =
    LazyLock::new(|| MindFlayerTentacles {});

/// Erinyes Longsword — STR-based 2d8+4 slashing melee with a permanent
/// +3d8 poison rider on hit (Erinyes' weapons are poisoned RAW). The
/// rider mirrors the Solar's radiant rider — the bigger fiendish dice
/// reflect the CR-12 bracket, and poison is a damage type many low-CR
/// PCs lack resistance to, so the rider is load-bearing for the threat.
pub struct ErinyesLongsword {}

impl Action for ErinyesLongsword {
    fn name(&self) -> &str {
        "erinyes longsword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["er-sword", "erinyes-ls"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Poison]
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
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let attack_mod = str_mod + caster.proficiency_bonus();
        let mut effects = resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "erinyes longsword",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(2, 8),
                damage_bonus: str_mod,
                damage_type: DamageType::Slashing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if !effects.is_empty() {
            let poison = encounter.roll(&Dice::new(3, 8));
            encounter.log(format!(
                "  erinyes longsword: +{} extra Poison (envenomed blade)",
                poison
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

pub static ERINYES_LONGSWORD: LazyLock<ErinyesLongsword> =
    LazyLock::new(|| ErinyesLongsword {});

/// Erinyes Multiattack — 3 longsword swings per Action. The flying
/// devil's signature burst at CR 12 — three 2d8+4 slashing + 3d8 poison
/// per hit means a full-connect roughly 60 average damage, enough to
/// drop most squishies in one turn.
pub static ERINYES_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "erinyes multiattack",
    sub_attack: &*ERINYES_LONGSWORD,
    count: 3,
});

/// Hell Hound Bite — STR-based 1d8 piercing melee with a 1d6 fire rider
/// per RAW. The fire is a separate `DealDamage` so per-target resistance
/// / immunity applies independently to the piercing and the fire halves.
pub struct HellHoundBite {}

impl Action for HellHoundBite {
    fn name(&self) -> &str {
        "hellfire bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hhb", "hellbite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Fire]
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
            "bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 8),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let fire = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!(
            "  hellfire bite: 1d6({}) = {} fire rider",
            fire, fire
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: fire,
            damage_type: DamageType::Fire,
        }));
        effects
    }
}

pub static HELL_HOUND_BITE: LazyLock<HellHoundBite> = LazyLock::new(|| HellHoundBite {});

/// Hell Hound Fire Breath — 15ft cone (radius-3 burst). DC 12 DEX save:
/// half damage on pass, full 6d6 fire on fail. Recharge mechanic in RAW
/// (5-6 on d6 at start of each turn); we model the simpler one-shot —
/// the AI's action picker will re-cast the breath when the slot allows.
pub struct HellHoundFireBreath {}

impl Action for HellHoundFireBreath {
    fn name(&self) -> &str {
        "fire breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hhfb", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15ft cone — burst origin sits 15ft from caster.
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
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
        const DC: i32 = 12;
        let raw = encounter.roll(&Dice::new(6, 6));
        encounter.log(format!(
            "  fire breath: 6d6({}) = {} fire (DC {} DEX, half on save)",
            raw, raw, DC
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Fire,
        )
    }
}

pub static HELL_HOUND_FIRE_BREATH: LazyLock<HellHoundFireBreath> =
    LazyLock::new(|| HellHoundFireBreath {});

/// Wyvern Bite — 2d6+STR piercing melee (a chomp; no rider). The
/// stinger is a separate action with its own poison save rider.
pub static WYVERN_BITE: SimpleWeapon = SimpleWeapon {
    display_name: "wyvern bite",
    aliases: &["wbite"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Piercing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Wyvern Stinger — 2d6+STR piercing melee with a brutal poison rider:
/// target makes a DC 15 CON save or takes 7d6 poison (half on save).
/// The wyvern's signature finisher — average ~24 poison on a fail
/// adds up to roughly half a CR-6 HP bar in one swing. Reach 2 because
/// the stinger tail extends past the body's footprint.
pub struct WyvernStinger {}

impl Action for WyvernStinger {
    fn name(&self) -> &str {
        "wyvern stinger"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sting", "wsting"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Poison]
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
            "sting",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        const DC: i32 = 15;
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, DC);
        let raw = encounter.roll(&Dice::new(7, 6));
        let poison = if save.passed() { raw / 2 } else { raw };
        encounter.log(format!(
            "  wyvern poison: 7d6({}) = {} poison{}",
            raw,
            poison,
            if save.passed() { " (saved)" } else { "" }
        ));
        if poison > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: poison,
                damage_type: DamageType::Poison,
            }));
        }
        effects
    }
}

pub static WYVERN_STINGER: LazyLock<WyvernStinger> = LazyLock::new(|| WyvernStinger {});

/// Storm Giant Greatsword — STR-based 6d6 + STR slashing melee. Reach 3
/// (15ft for a Huge-footprint giant). One of the heaviest single-swing
/// damage dice in the codebase — averages ~30 slashing per hit.
pub static STORM_GIANT_GREATSWORD: SimpleWeapon = SimpleWeapon {
    display_name: "storm greatsword",
    aliases: &["sgs", "sgreatsword"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(6, 6),
    damage_type: DamageType::Slashing,
    reach: 3,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Storm Giant Thrown Rock — STR-based 4d12 + STR bludgeoning ranged
/// attack. Range 240ft RAW; capped at 40 tiles to fit the map. The
/// storm giant's stand-off lane when the front line is buttoned up.
pub static STORM_GIANT_ROCK: SimpleWeapon = SimpleWeapon {
    display_name: "storm rock",
    aliases: &["sgr", "srock"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 12),
    damage_type: DamageType::Bludgeoning,
    reach: 40,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
    normal_range: Some(24),
};

/// Storm Giant Lightning Strike — bonus-action signature ability. Hurls
/// a bolt of lightning at a single target within 500ft. DC 17 DEX save:
/// half damage on pass, full 8d10 lightning on fail. The bonus-action
/// cost makes it free-action-economy alongside the giant's main swing.
pub struct StormGiantLightningStrike {}

impl Action for StormGiantLightningStrike {
    fn name(&self) -> &str {
        "lightning strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lstrike", "sgls"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 500ft RAW — capped to map width.
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
        bonus_action_only()
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
        const DC: i32 = 17;
        let raw = encounter.roll(&Dice::new(8, 10));
        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, DC);
        let damage = if save.passed() { raw / 2 } else { raw };
        encounter.log(format!(
            "  lightning strike: 8d10({}) = {} lightning{}",
            raw,
            damage,
            if save.passed() { " (saved)" } else { "" }
        ));
        if damage == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Lightning,
        })]
    }
}

pub static STORM_GIANT_LIGHTNING_STRIKE: LazyLock<StormGiantLightningStrike> =
    LazyLock::new(|| StormGiantLightningStrike {});

/// Hydra Bite — STR-based 1d10+STR piercing melee, reach 2 (10ft natural
/// reach for the gargantuan head). The Hydra has 5 of these per turn via
/// HYDRA_MULTI. Standalone so the hydra can still bite when only one
/// target is in melee range.
pub static HYDRA_BITE: SimpleWeapon = SimpleWeapon {
    display_name: "hydra bite",
    aliases: &["h-bite", "hbite"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 10),
    damage_type: DamageType::Piercing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Hydra Multiattack — 5 simultaneous bites (one per head). The number
/// of heads is fixed at 5 RAW; the engine doesn't model head-severing
/// dynamics so the multiattack count is constant.
pub static HYDRA_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "hydra multiattack",
    sub_attack: &HYDRA_BITE,
    count: 5,
});

/// Stone Giant Greatclub — STR-based 3d8+STR bludgeoning melee, reach 3.
/// The greatclub is the giant's go-to melee, with the boulder filling
/// the ranged lane.
pub static STONE_GIANT_GREATCLUB: SimpleWeapon = SimpleWeapon {
    display_name: "stone greatclub",
    aliases: &["s-gc", "sgc"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 8),
    damage_type: DamageType::Bludgeoning,
    reach: 3,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Stone Giant Boulder — STR-based 4d10+STR bludgeoning ranged, reach 24.
/// The boulder is the giant's signature ranged threat — paired with the
/// greatclub for melee, the AI picks whichever the action picker validates.
pub static STONE_GIANT_BOULDER: SimpleWeapon = SimpleWeapon {
    display_name: "stone boulder",
    aliases: &["s-boulder", "sboulder"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 10),
    damage_type: DamageType::Bludgeoning,
    reach: 24,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
    normal_range: Some(16),
};

/// Stone Giant Multiattack — 2 greatclub swings per Action. Mirrors the
/// frost giant / hill giant pattern: physical thresher boss melee, no
/// rider effects, raw bludgeoning damage.
pub static STONE_GIANT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "stone giant multiattack",
    sub_attack: &STONE_GIANT_GREATCLUB,
    count: 2,
});

/// Medusa Petrifying Gaze — single-target action. CON save against a
/// fixed DC 14; on fail, the target is Petrified for 1 round. No damage
/// — the petrification is the threat. The gaze is line-of-sight gated
/// (the medusa must see the target). Mirrors the cockatrice bite's
/// shape but without the bite damage: pure stone-lock.
pub struct MedusaPetrifyingGaze {}

impl Action for MedusaPetrifyingGaze {
    fn name(&self) -> &str {
        "petrifying gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gaze", "medusa-gaze"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        const DC: i32 = 14;
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, DC);
        if save.passed() {
            encounter.log("  petrifying gaze: target averts their eyes");
            return Vec::new();
        }
        encounter.log("  petrifying gaze: target turns to stone");
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: target_id,
            condition: Condition::Petrified,
            timer: ConditionTimer::Rounds(1),
        })]
    }
}

pub static MEDUSA_PETRIFYING_GAZE: LazyLock<MedusaPetrifyingGaze> =
    LazyLock::new(|| MedusaPetrifyingGaze {});

/// Medusa Snake Hair — DEX-based 1d4+DEX piercing + 4d6 poison rider on
/// hit (RAW). One of the multiattack lanes; reach 1.
pub struct MedusaSnakeHair {}

impl Action for MedusaSnakeHair {
    fn name(&self) -> &str {
        "snake hair"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh", "snakes"]
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
            "snake hair",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let poison = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!("  snake hair: 4d6({}) poison rider", poison));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: poison,
            damage_type: DamageType::Poison,
        }));
        effects
    }
}

pub static MEDUSA_SNAKE_HAIR: LazyLock<MedusaSnakeHair> =
    LazyLock::new(|| MedusaSnakeHair {});

/// Medusa Multiattack — 1 snake-hair swing + 1 petrifying gaze per
/// Action. CompoundAttack because the lanes are heterogeneous (different
/// targeting, different effects). The gaze targets the same actor as the
/// snake hair RAW (target shared per attack).
pub static MEDUSA_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "medusa multiattack",
    parts: vec![
        (&*MEDUSA_SNAKE_HAIR, 1),
        (&*MEDUSA_PETRIFYING_GAZE, 1),
    ],
});

/// Salamander Tail — STR-based 2d6 bludgeoning + 1d6 fire rider on hit.
/// The salamander wreathes its blows in heat; the fire rider is auto-
/// apply on hit (no save). Reach 3 (gargantuan tail).
pub struct SalamanderTail {}

impl Action for SalamanderTail {
    fn name(&self) -> &str {
        "salamander tail"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tail", "salamander"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(3)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Fire]
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
            "salamander tail",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Bludgeoning,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let fire = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!("  salamander tail: 1d6({}) fire rider", fire));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: fire,
            damage_type: DamageType::Fire,
        }));
        effects
    }
}

pub static SALAMANDER_TAIL: LazyLock<SalamanderTail> = LazyLock::new(|| SalamanderTail {});

/// Salamander Spear — STR-based 2d6 piercing + 1d6 fire rider, reach 2.
/// The salamander's polearm, paired with the tail in the multi.
pub struct SalamanderSpear {}

impl Action for SalamanderSpear {
    fn name(&self) -> &str {
        "salamander spear"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sspear"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Fire]
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
            "salamander spear",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let fire = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!("  salamander spear: 1d6({}) fire rider", fire));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: fire,
            damage_type: DamageType::Fire,
        }));
        effects
    }
}

pub static SALAMANDER_SPEAR: LazyLock<SalamanderSpear> =
    LazyLock::new(|| SalamanderSpear {});

/// Salamander Multiattack — 1 spear + 1 tail per Action. Heterogeneous
/// multi: spear is reach-2 (poke), tail is reach-3 (whip behind targets).
pub static SALAMANDER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "salamander multiattack",
    parts: vec![
        (&*SALAMANDER_SPEAR, 1),
        (&*SALAMANDER_TAIL, 1),
    ],
});

/// Death Knight Longsword — STR-based 1d8 slash + 4d8 necrotic rider on
/// hit (5e Death Knight uses a longsword with a necrotic empowerment).
/// Models the necrotic rider via a direct DealDamage so the engine's
/// resistance / immunity table handles the half-damage on a wraith /
/// other necrotic-immune adjacency cleanly.
pub struct DeathKnightLongsword {}

impl Action for DeathKnightLongsword {
    fn name(&self) -> &str {
        "longsword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ls", "sword"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Necrotic]
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
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let prof = caster.proficiency_bonus();
        // First, resolve the slashing core hit. We re-use resolve_attack
        // for the d20 + log line and pull out the resulting damage to
        // gate the necrotic rider on hit.
        use crate::engine::attack::resolve_attack_outcome;
        let (mut effects, slash_dmg) = resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "longsword",
                attack_bonus: str_mod + prof,
                damage_dice: Dice::new(1, 8),
                damage_bonus: str_mod,
                damage_type: DamageType::Slashing,
                is_melee: true,
                long_range: None,
                is_spell: false,
            },
        );
        if slash_dmg == 0 {
            return effects;
        }
        // Necrotic rider — only fires on a successful slash. Empower
        // tracks the death knight's life-draining edge.
        let raw = encounter.roll(&Dice::new(4, 8));
        encounter.log(format!(
            "  longsword: +{} necrotic empowerment",
            raw
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Necrotic,
        }));
        effects
    }
}

pub static DEATH_KNIGHT_LONGSWORD: LazyLock<DeathKnightLongsword> =
    LazyLock::new(|| DeathKnightLongsword {});

/// Death Knight Hellfire Orb — once-per-encounter signature: a 20 ft
/// radius hell-fire orb hurled to a point within 120 ft. Every creature
/// in the burst makes a DEX save vs the death knight's spell save DC
/// (CHA-based, DC 18 at CR 17): fail = 10d8 fire damage, success = half.
/// Friendly fire applies — the death knight doesn't filter undead allies
/// out of the radius (RAW). Approximated as enemy-only burst via the
/// engine's `resolve_burst_save_damage` helper for AI sanity.
pub struct DeathKnightHellfireOrb {}

impl Action for DeathKnightHellfireOrb {
    fn name(&self) -> &str {
        "hellfire orb"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hfo", "orb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
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
        const DC: i32 = 18;
        let raw = encounter.roll(&Dice::new(10, 8));
        encounter.log(format!(
            "  hellfire orb: 10d8({}) = {} fire area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Fire,
        )
    }
}

pub static DEATH_KNIGHT_HELLFIRE_ORB: LazyLock<DeathKnightHellfireOrb> =
    LazyLock::new(|| DeathKnightHellfireOrb {});

/// Death Knight Multiattack — 3 longsword swings per Action. RAW from
/// the MM. Each swing rolls its own necrotic rider via the rider hook
/// on DeathKnightLongsword.
pub static DEATH_KNIGHT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "death knight multiattack",
    sub_attack: &*DEATH_KNIGHT_LONGSWORD,
    count: 3,
});

/// Ghost Withering Touch — incorporeal melee. d20 + DEX + prof vs AC;
/// on hit 4d6+3 necrotic. The ghost's "withering" name comes from the
/// fact that the damage type is necrotic (RAW), so a necrotic-immune
/// undead ally is safe and a celestial / radiant-resistant adventurer
/// takes full damage. No on-hit rider beyond the necrotic typing —
/// the Horrifying Visage / Possession actions are separate Actions.
pub struct GhostWitheringTouch {}

impl Action for GhostWitheringTouch {
    fn name(&self) -> &str {
        "withering touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wt", "touch"]
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
            "withering touch",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(4, 6),
            DamageType::Necrotic,
            true,
        )
    }
}

pub static GHOST_WITHERING_TOUCH: LazyLock<GhostWitheringTouch> =
    LazyLock::new(|| GhostWitheringTouch {});

/// Ghost Horrifying Visage — 60ft radius burst (centred on the ghost),
/// each non-undead enemy in range makes a WIS save vs DC 13. Fail =
/// Frightened for 5 rounds. Success = immunity to this ghost's Visage
/// for 24 hours (not modeled — single-encounter scope). Undead and
/// fiends are immune to fright already via the engine's condition-
/// immunity table, so the no-target case folds out naturally.
pub struct GhostHorrifyingVisage {}

impl Action for GhostHorrifyingVisage {
    fn name(&self) -> &str {
        "horrifying visage"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hv", "visage"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        // Frightens; never reduces HP. Stays in `is_harmful: true` lane
        // so the AI's burst heuristic still picks it up when 2+ enemies
        // cluster, but the focus-fire path that ranks "does this whittle
        // HP?" skips it cleanly.
        false
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
        const DC: i32 = 13;
        const RADIUS: isize = 24; // 60 ft
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, caster_loc, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(5),
            }));
            encounter.log(format!(
                "  horrifying visage: actor #{} is Frightened",
                tid
            ));
        }
        effects
    }
}

pub static GHOST_HORRIFYING_VISAGE: LazyLock<GhostHorrifyingVisage> =
    LazyLock::new(|| GhostHorrifyingVisage {});

/// Stone Golem Slam — STR-based 3d8+STR bludgeoning melee, reach 1.
/// The golem's only attack (RAW: 2 slams per multi). No rider effects;
/// pure crushing damage. Stays a SimpleWeapon so the multiattack
/// wrapper can re-use it cleanly.
pub static STONE_GOLEM_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "stone slam",
    aliases: &["s-slam", "sslam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(3, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Stone Golem Multiattack — 2 slams per Action. The golem's only
/// non-Slow-Spell action lane.
pub static STONE_GOLEM_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "stone golem multiattack",
    sub_attack: &STONE_GOLEM_SLAM,
    count: 2,
});

/// Stone Golem Slow — Action; 10ft sphere around the golem. Every
/// creature in the burst makes a WIS save vs DC 17; on fail, they're
/// `Slowed` for 5 rounds (the engine collapses RAW's "halved speed +
/// -2 AC + -2 DEX saves" envelope into our `Slowed` condition).
/// Enemy-only partition matches the burst convention; allies in the
/// area are spared. The golem's signature control action — pairs with
/// the slam multi for raw damage.
pub struct StoneGolemSlow {}

impl Action for StoneGolemSlow {
    fn name(&self) -> &str {
        "stone golem slow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sgs", "golem-slow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn deals_damage(&self) -> bool {
        false
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
        const DC: i32 = 17;
        const RADIUS: isize = 2; // 10ft
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, caster_loc, RADIUS) {
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Slowed,
                timer: ConditionTimer::Rounds(5),
            }));
            encounter.log(format!("  stone golem slow: actor #{} is Slowed", tid));
        }
        effects
    }
}

pub static STONE_GOLEM_SLOW: LazyLock<StoneGolemSlow> = LazyLock::new(|| StoneGolemSlow {});

/// Bullette Bite — STR-based 4d12+STR piercing melee, reach 1. The
/// bullette's signature crunch — averages ~26 piercing per hit. No
/// rider effects; pure damage. Stays a `SimpleWeapon` so the bullette's
/// loadout can mix this with the Deadly Leap follow-up cleanly.
pub static BULLETTE_BITE: SimpleWeapon = SimpleWeapon {
    display_name: "bullette bite",
    aliases: &["bbite", "bullette-bite"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 12),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Bullette Deadly Leap — Action; the bullette jumps onto a target,
/// landing with crushing force. We model as a single melee swing dealing
/// 3d6+STR bludgeoning, plus the target makes a STR save vs DC 16 or
/// is knocked Prone. The leap is RAW reserved for the Bullette's bonus
/// "Deadly Leap" action; we expose it as a regular Action lane so the
/// AI can pick between Bite and Leap based on whether knocking the
/// target prone (e.g. setting up an ally's melee crit window) is worth
/// the lower damage tier.
pub struct BulletteDeadlyLeap {}

impl Action for BulletteDeadlyLeap {
    fn name(&self) -> &str {
        "bullette deadly leap"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["leap", "bleap"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "deadly leap",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(3, 6),
            DamageType::Bludgeoning,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        const DC: i32 = 16;
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, DC);
        if !save.passed() {
            encounter.log(format!(
                "  deadly leap: actor #{} fails STR save and is knocked Prone",
                target_id
            ));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static BULLETTE_DEADLY_LEAP: LazyLock<BulletteDeadlyLeap> =
    LazyLock::new(|| BulletteDeadlyLeap {});

/// Bullette Multiattack — Action: 2 bites. The bullette's RAW
/// multiattack is one Bite; we double it so the CR-5 bullette can keep
/// pace with the other CR-5 boss-tier templates (manticore, etc.)
/// without needing to chain back-to-back Action picks.
pub static BULLETTE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "bullette multiattack",
    sub_attack: &BULLETTE_BITE,
    count: 2,
});

/// Bone Devil Sting — STR-based 2d8+STR piercing melee, reach 2. On
/// hit, the target makes a CON save vs DC 14 or takes an additional
/// 5d6 poison damage AND is Poisoned for 10 rounds. The sting is the
/// bone devil's signature finisher — mirrors the wyvern stinger shape
/// with a smaller poison rider but a lasting Poisoned-on-fail clause.
pub struct BoneDevilSting {}

impl Action for BoneDevilSting {
    fn name(&self) -> &str {
        "bone devil sting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bdsting", "tailsting"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Poison]
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "sting",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 8),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        const DC: i32 = 14;
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, DC);
        let raw = encounter.roll(&Dice::new(5, 6));
        let poison = if save.passed() { raw / 2 } else { raw };
        encounter.log(format!(
            "  bone devil poison: 5d6({}) = {} poison{}",
            raw,
            poison,
            if save.passed() { " (saved)" } else { "" }
        ));
        if poison > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: poison,
                damage_type: DamageType::Poison,
            }));
        }
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static BONE_DEVIL_STING: LazyLock<BoneDevilSting> =
    LazyLock::new(|| BoneDevilSting {});

/// Bone Devil Claws — STR-based 1d8+STR slashing melee, reach 1. The
/// devil's secondary attack lane; pairs with the Sting in a Multiattack
/// (RAW: 2 claws + 1 sting per Action).
pub static BONE_DEVIL_CLAWS: SimpleWeapon = SimpleWeapon {
    display_name: "bone devil claws",
    aliases: &["bdclaws"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Bone Devil Multiattack — 2 claws + 1 sting per Action via the
/// `CompoundAttack` wrapper. The devil's full opening salvo: a Sting +
/// double Claws can ladder up to ~40 damage on a single round.
pub static BONE_DEVIL_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bone devil multiattack",
    // The Sting itself runs the save + Poisoned install; the
    // CompoundAttack wrapper just unspools the swing for one
    // Action's worth of resources.
    parts: vec![(&BONE_DEVIL_CLAWS, 2), (&*BONE_DEVIL_STING, 1)],
});

/// Air Elemental Slam — STR-based 2d8 + STR bludgeoning melee, reach 1.
/// Distinct damage envelope from the fire elemental's burn-touch — air
/// elementals hit harder per swing but lack the ignition rider.
pub static AIR_ELEMENTAL_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "air slam",
    aliases: &["aslam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Air Elemental Multiattack — 2 slams per Action via the standard
/// `Multiattack` wrapper. The air elemental's full opening salvo.
pub static AIR_ELEMENTAL_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "air elemental multiattack",
    sub_attack: &AIR_ELEMENTAL_SLAM,
    count: 2,
});

/// Earth Elemental Slam — STR-based 4d8 + STR bludgeoning melee, reach 1.
/// Much heavier per-swing than the air variant — the earth elemental's
/// signature is its slow-but-brutal slam. Reach is melee per MM RAW.
pub static EARTH_ELEMENTAL_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "earth slam",
    aliases: &["eslam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(4, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Earth Elemental Multiattack — 2 slams per Action. Mirrors the air
/// elemental wrapper but with the heavier per-slam dice.
pub static EARTH_ELEMENTAL_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "earth elemental multiattack",
    sub_attack: &EARTH_ELEMENTAL_SLAM,
    count: 2,
});

/// Balor Longsword — STR-based 3d8 + STR slashing melee, reach 2 (10ft
/// per MM). On hit, the target takes an extra 3d8 lightning damage from
/// the flaming runes on the blade. We collapse the RAW "magical
/// longsword + 3d8 fire on hit" into "longsword damage + 3d8 lightning"
/// to keep the apex demon's damage envelope mixed (lightning is rarely
/// resisted at high CR).
pub struct BalorLongsword {}

impl Action for BalorLongsword {
    fn name(&self) -> &str {
        "balor longsword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["blongsword", "bl-sword"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Lightning]
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
            "balor longsword",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(3, 8),
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Lightning runes rider — fires on hit only (effects non-empty).
        let bolt = encounter.roll(&Dice::new(3, 8));
        encounter.log(format!(
            "  balor runes: 3d8({}) = {} lightning",
            bolt, bolt
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: bolt,
            damage_type: DamageType::Lightning,
        }));
        effects
    }
}

pub static BALOR_LONGSWORD: LazyLock<BalorLongsword> = LazyLock::new(|| BalorLongsword {});

/// Balor Lightning Whip — STR-based 2d6 + STR slashing reach attack,
/// reach 12 (30ft). On hit, deals an extra 3d6 lightning damage. The
/// reach is the balor's stand-off lane: pull targets in or jab past
/// the line. Mirrors the BalorLongsword's slashing + lightning split
/// but with smaller dice and longer reach.
pub struct BalorWhip {}

impl Action for BalorWhip {
    fn name(&self) -> &str {
        "balor whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bwhip", "lwhip"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Lightning]
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
            "balor whip",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let bolt = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  balor whip lightning: 3d6({}) = {} lightning",
            bolt, bolt
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: bolt,
            damage_type: DamageType::Lightning,
        }));
        effects
    }
}

pub static BALOR_WHIP: LazyLock<BalorWhip> = LazyLock::new(|| BalorWhip {});

/// Balor Multiattack — 1 longsword + 1 whip per Action via
/// `CompoundAttack`. The full apex-demon opening salvo: a longsword
/// (3d8 slash + 3d8 lightning) and a whip (2d6 slash + 3d6 lightning)
/// can ladder up to ~50-60 damage on a single target in one round.
pub static BALOR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "balor multiattack",
    parts: vec![(&*BALOR_LONGSWORD, 1), (&*BALOR_WHIP, 1)],
});

/// Balor Fire Aura — bonus-action burst that ignites every actor whose
/// footprint touches the balor's. 5e RAW: "any creature that touches
/// the balor or hits it with a melee attack while within 5ft takes 10
/// fire damage." We hoist the trigger off the per-hit path into a
/// once-per-turn bonus-action burst so the aura is visible in the log
/// and the AI can prioritize it explicitly: every adjacent enemy eats
/// 3d6 fire (no save). Pure burst — no rider, no concentration.
pub struct BalorFireAura {}

impl Action for BalorFireAura {
    fn name(&self) -> &str {
        "balor fire aura"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["faura", "balor-aura"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
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
        // Adjacent enemies only — 1-tile gap = footprint-adjacent.
        // The Fire Aura RAW is "within 5ft", so radius 1 captures the
        // ring of touching footprints.
        let raw = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  balor fire aura: 3d6({}) shared fire to adjacent enemies",
            raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, center, 1) {
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Fire,
            }));
        }
        effects
    }
}

pub static BALOR_FIRE_AURA: LazyLock<BalorFireAura> = LazyLock::new(|| BalorFireAura {});

/// Glabrezu Pincer — STR-based 2d10 + STR bludgeoning melee, reach 1.
/// The Glabrezu has two of these as part of its multiattack. RAW: the
/// pincer crushes for big damage on the front-line tank; we collapse
/// the "grapple on hit" rider — it's a flavor mechanic that doesn't
/// land any new conditions our pool cares about beyond Grappled, and
/// the rider would obscure the more meaningful 4-attack multi.
pub static GLABREZU_PINCER: SimpleWeapon = SimpleWeapon {
    display_name: "glabrezu pincer",
    aliases: &["gpincer"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 10),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Glabrezu Fist — STR-based 2d4 + STR bludgeoning melee, reach 1. The
/// glabrezu's secondary attack lane; pairs with the pincers in its
/// 4-swing multiattack (2 pincers + 2 fists per Action).
pub static GLABREZU_FIST: SimpleWeapon = SimpleWeapon {
    display_name: "glabrezu fist",
    aliases: &["gfist"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 4),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Glabrezu Multiattack — 2 pincers + 2 fists per Action via the
/// `CompoundAttack` wrapper. The glabrezu's signature opening salvo: a
/// 4-swing volley that out-damages most CR-9 monsters by ramming four
/// 2d10 / 2d4 hits onto a single target.
pub static GLABREZU_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "glabrezu multiattack",
    parts: vec![(&GLABREZU_PINCER, 2), (&GLABREZU_FIST, 2)],
});

/// Marilith Longsword — STR-based 2d8 + STR slashing melee. Marilith
/// wields six of these (one per arm) and they all swing per Action via
/// the multiattack lane. Standard reach-1 melee — no rider; the volume
/// of swings IS the threat.
pub static MARILITH_LONGSWORD: SimpleWeapon = SimpleWeapon {
    display_name: "marilith longsword",
    aliases: &["mls", "marilith-ls"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Marilith Tail — STR-based 2d10 + STR bludgeoning melee, reach 2 (the
/// snake-body tail extends 10ft per RAW). Final swing of the multiattack
/// envelope. We collapse the RAW "constrict / grapple on hit" rider —
/// the engine's grapple gate doesn't yet model the "creature one size
/// larger or smaller" clause, and the long reach + the multi's volume
/// already make the marilith threatening enough.
pub static MARILITH_TAIL: SimpleWeapon = SimpleWeapon {
    display_name: "marilith tail",
    aliases: &["mtail", "marilith-t"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 10),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Marilith Multiattack — 6 longswords + 1 tail per Action via the
/// `CompoundAttack` wrapper. The marilith's signature seven-swing volley
/// is the highest single-action attack count in our monster pool; the
/// AI's focus-fire picker concentrates all seven on a single target,
/// which is brutal but consistent with the CR-16 damage envelope.
pub static MARILITH_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "marilith multiattack",
    parts: vec![(&MARILITH_LONGSWORD, 6), (&MARILITH_TAIL, 1)],
});

/// Vrock Talons — STR-based 2d6 + STR slashing melee, reach 1. The
/// vrock's stock melee swing; pairs with the beak in the 3-attack
/// multi (2 talons + 1 beak).
pub static VROCK_TALONS: SimpleWeapon = SimpleWeapon {
    display_name: "vrock talons",
    aliases: &["vtalons"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Vrock Beak — STR-based 2d6 + STR piercing melee, reach 1. The vrock's
/// finisher — same damage profile as the talons but piercing rather than
/// slashing, so resistance / vulnerability typing can vary the swing's
/// output across the multi.
pub static VROCK_BEAK: SimpleWeapon = SimpleWeapon {
    display_name: "vrock beak",
    aliases: &["vbeak"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Vrock Multiattack — 2 talons + 1 beak per Action via `CompoundAttack`.
/// The vrock's standard volley: three swings at reach 1 against one
/// target. Mid-CR damage envelope (CR 6).
pub static VROCK_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "vrock multiattack",
    parts: vec![(&VROCK_TALONS, 2), (&VROCK_BEAK, 1)],
});

/// Vrock Stunning Screech — action that emits a piercing scream. Every
/// non-demon creature within 20 ft (8 tiles) takes 3d6 thunder and must
/// succeed on a CON save vs the vrock's CHA-based DC or be Stunned until
/// the end of the vrock's next turn. We approximate "non-demon" by
/// exempting creatures with Poison immunity (every demon in our pool has
/// Poison immunity; ordinary creatures don't). Once per encounter is the
/// RAW recharge, but we leave the rate-limit to the engine's standard
/// action economy — the vrock will spam it but the AI's heuristic gates
/// on a 2+ cluster so the spam stays meaningful.
pub struct VrockScreech {}

impl Action for VrockScreech {
    fn name(&self) -> &str {
        "vrock screech"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["screech", "vscreech"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let center = caster.location();
        const RADIUS: isize = 8;
        let raw = encounter.roll(&Dice::new(3, 6));
        encounter.log(format!(
            "  vrock screech: 3d6({}) shared thunder area",
            raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Non-demon filter: any creature *without* poison immunity. Every
        // demon in our pool is poison-immune by template; the cohort
        // gates the screech to non-demon targets.
        for tid in encounter.enemy_burst_targets(caster_id, center, RADIUS) {
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if target.is_immune_to(DamageType::Poison) {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Constitution, dc);
            if raw > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: raw,
                    damage_type: DamageType::Thunder,
                }));
            }
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static VROCK_SCREECH: LazyLock<VrockScreech> = LazyLock::new(|| VrockScreech {});

/// Shambling Mound slam — STR-based 2d8+STR bludgeoning melee, reach 1.
/// The single-target swing of the Shambling Mound's two-slam multiattack.
/// No rider — the slam carries the load-bearing damage; the engulf
/// lane handles the grapple rider separately.
pub static SHAMBLING_MOUND_SLAM: SimpleWeapon = SimpleWeapon {
    display_name: "shambling slam",
    aliases: &["sslam", "sm-slam"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 8),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Shambling Mound multiattack — 2 slams per Action via the standard
/// Multiattack wrapper. Single-target heavy melee burst with no
/// engulf rider; the engulf attack is its own action lane (a separate
/// pick the mound can take when a grappled victim is the goal).
pub static SHAMBLING_MOUND_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "shambling mound multiattack",
    sub_attack: &SHAMBLING_MOUND_SLAM,
    count: 2,
});

/// Shambling Mound engulf — a special melee attack that wraps the
/// target in vines. We model the load-bearing half: a STR-based attack
/// roll vs the target's AC (reach 1, melee). On hit: 2d8+STR
/// bludgeoning AND the target makes a DC 14 STR save or is Grappled
/// (zero movement) for 10 rounds. RAW also has the engulfed victim
/// being unable to breathe and taking 2d8 per round, but we collapse
/// to the grapple + initial damage envelope so the rider is one save,
/// one log line, and consistent with the Mimic's Adhered shape.
pub struct ShamblingMoundEngulf {}

impl Action for ShamblingMoundEngulf {
    fn name(&self) -> &str {
        "shambling engulf"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["engulf", "sm-engulf"]
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
        // Resolve the attack as a standard 2d8+STR bludgeoning swing.
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 8),
            DamageType::Bludgeoning,
            true,
        );
        // On a miss the helper returned no damage effects — bail before
        // queueing the save / grapple rider.
        if effects.is_empty() {
            return effects;
        }
        // On hit: STR save vs DC 14 (5e RAW for Shambling Mound's
        // engulf save). Targets that fail get Grappled for 10 rounds —
        // long enough to feel like an engulf, short enough that the
        // engagement can't carry into another encounter.
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 14);
        if !save.passed() {
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Grappled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static SHAMBLING_MOUND_ENGULF: LazyLock<ShamblingMoundEngulf> =
    LazyLock::new(|| ShamblingMoundEngulf {});

/// Displacer Beast tentacle — STR-based 2d6 bludgeoning melee attack with
/// 10ft reach (2 tiles). The displacer beast lashes out with a barbed
/// tentacle; two of these compose its multiattack.
pub static TENTACLE: SimpleWeapon = SimpleWeapon {
    display_name: "tentacle",
    aliases: &["tent"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Displacer Beast multiattack — two tentacle strikes per Action.
pub static DISPLACER_BEAST_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "tentacle flurry",
    sub_attack: &TENTACLE,
    count: 2,
});

/// Umber Hulk claw — STR-based 1d8 slashing melee attack. The umber hulk
/// rakes with a massive chitinous claw; standard 5ft reach.
pub static UMBER_CLAW: SimpleWeapon = SimpleWeapon {
    display_name: "umber claw",
    aliases: &["uclaw"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Cloaker tail — STR-based 1d8 slashing melee attack with 10ft reach
/// (2 tiles). The cloaker whips its barbed tail at nearby prey.
pub static CLOAKER_TAIL: SimpleWeapon = SimpleWeapon {
    display_name: "cloaker tail",
    aliases: &["ctail"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

// ─── Basilisk ────────────────────────────────────────────────────────

/// Basilisk bite — STR-based 2d6+3 piercing melee, plus a CON save
/// (DC 12) for Petrified (1 round) on hit. Same "hit, then save-or-suck"
/// shape as the cockatrice bite but beefier damage and a harder save.
pub struct BasiliskBite {}

impl Action for BasiliskBite {
    fn name(&self) -> &str {
        "bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["basilisk-bite"]
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
            "bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 12);
        if !save.passed() {
            encounter.log("  petrifying gaze: target turns to stone!");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Petrified,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static BASILISK_BITE: LazyLock<BasiliskBite> = LazyLock::new(|| BasiliskBite {});

// ─── Chuul ───────────────────────────────────────────────────────────

/// Chuul pincer — STR-based 2d6+4 bludgeoning melee, grapples on hit.
pub static CHUUL_PINCER_WEAPON: SimpleWeapon = SimpleWeapon {
    display_name: "pincer",
    aliases: &["claw", "pincer"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Chuul pincer with grapple rider.
pub struct ChuulPincer {}

impl Action for ChuulPincer {
    fn name(&self) -> &str {
        "pincer"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["claw"]
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
            "pincer",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Bludgeoning,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let already = encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.has_condition(Condition::Grappled));
        if !already {
            encounter.log("  the chuul grapples with its pincer!");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Grappled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static CHUUL_PINCER: LazyLock<ChuulPincer> = LazyLock::new(|| ChuulPincer {});

/// Chuul tentacles — paralyzing tentacle attack. Deals 1d6+4 poison
/// and forces a CON save (DC 13) or Paralyzed (1 round). In 5e, this
/// only targets grappled creatures, but we allow it on any adjacent
/// target for simplicity.
pub struct ChuulTentacles {}

impl Action for ChuulTentacles {
    fn name(&self) -> &str {
        "tentacles"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["paralyze"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
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
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        let dmg = encounter.roll(&Dice::new(1, 6));
        let caster_str = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Strength))
            .unwrap_or(0);
        let total_dmg = (dmg as i32 + caster_str).max(0) as u32;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total_dmg,
            damage_type: DamageType::Poison,
        })];
        if !save.passed() {
            encounter.log("  tentacles paralyze the target!");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static CHUUL_TENTACLES: LazyLock<ChuulTentacles> = LazyLock::new(|| ChuulTentacles {});

// ─── Ankheg ──────────────────────────────────────────────────────────

/// Ankheg bite — STR-based 2d6+3 slashing + 1d6 acid.
pub struct AnkhegBite {}

impl Action for AnkhegBite {
    fn name(&self) -> &str {
        "bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ankheg-bite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Acid]
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
            "bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Slashing,
            true,
        );
        if !effects.is_empty() {
            let acid = encounter.roll(&Dice::new(1, 6));
            encounter.log(format!("  acid splash: 1d6({}) acid", acid));
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: acid,
                damage_type: DamageType::Acid,
            }));
        }
        effects
    }
}

pub static ANKHEG_BITE: LazyLock<AnkhegBite> = LazyLock::new(|| AnkhegBite {});

/// Ankheg acid spray — 3d6 acid in a 30ft line (6 tiles), DEX save DC 13
/// for half. Recharge-limited (we model as once per encounter via a
/// bonus action cost so the AI doesn't spam it).
pub struct AnkhegAcidSpray {}

impl Action for AnkhegAcidSpray {
    fn name(&self) -> &str {
        "acid spit"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["spray", "acid-spray"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
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
        let dmg = encounter.roll(&Dice::new(3, 6));
        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, 13);
        let actual = if save.passed() { dmg / 2 } else { dmg };
        encounter.log(format!("  acid spray: 3d6({}) acid", dmg));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: actual,
            damage_type: DamageType::Acid,
        })]
    }
}

pub static ANKHEG_ACID_SPRAY: LazyLock<AnkhegAcidSpray> = LazyLock::new(|| AnkhegAcidSpray {});

// ─── Giant Scorpion ──────────────────────────────────────────────────

/// Giant Scorpion claw — STR-based 1d8+2 bludgeoning, grapple on hit.
pub struct GiantScorpionClaw {}

impl Action for GiantScorpionClaw {
    fn name(&self) -> &str {
        "claw"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["scorpion-claw"]
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
            "claw",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 8),
            DamageType::Bludgeoning,
            true,
        );
        if !effects.is_empty() {
            let already = encounter
                .actors
                .get(&target_id)
                .is_some_and(|a| a.has_condition(Condition::Grappled));
            if !already {
                encounter.log("  the scorpion grapples with its claw!");
                effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Grappled,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        effects
    }
}

pub static GIANT_SCORPION_CLAW: LazyLock<GiantScorpionClaw> =
    LazyLock::new(|| GiantScorpionClaw {});

/// Giant Scorpion sting — STR-based 1d10+2 piercing + 4d10 poison
/// (CON save DC 12 for half).
pub struct GiantScorpionSting {}

impl Action for GiantScorpionSting {
    fn name(&self) -> &str {
        "sting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["scorpion-sting"]
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
            "sting",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 10),
            DamageType::Piercing,
            true,
        );
        if !effects.is_empty() {
            let poison_dmg = encounter.roll(&Dice::new(4, 10));
            let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 12);
            let actual = if save.passed() {
                poison_dmg / 2
            } else {
                poison_dmg
            };
            encounter.log(format!("  venom: 4d10({}) poison", poison_dmg));
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: actual,
                damage_type: DamageType::Poison,
            }));
        }
        effects
    }
}

pub static GIANT_SCORPION_STING: LazyLock<GiantScorpionSting> =
    LazyLock::new(|| GiantScorpionSting {});

// ─── Grick ───────────────────────────────────────────────────────────

/// Grick tentacles — DEX-based 2d6+2 slashing melee.
pub static GRICK_TENTACLES_WEAPON: SimpleWeapon = SimpleWeapon {
    display_name: "tentacles",
    aliases: &["grick-tent"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_ability: Some(AbilityScoreType::Dexterity),
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Grick beak — DEX-based 1d6+2 piercing melee (bonus action).
pub struct GrickBeak {}

impl Action for GrickBeak {
    fn name(&self) -> &str {
        "beak"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["grick-beak"]
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
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
            "beak",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 6),
            DamageType::Piercing,
            true,
        )
    }
}

pub static GRICK_BEAK: LazyLock<GrickBeak> = LazyLock::new(|| GrickBeak {});

/// Glaive — STR-based 1d10 slashing melee weapon with reach 2 (10 ft).
/// Two-handed polearm used by gnoll pack lords and similar martial
/// leaders. Reach 2 lets the wielder strike from one tile back, matching
/// the 5e polearm reach property.
pub static GLAIVE: SimpleWeapon = SimpleWeapon {
    display_name: "glaive",
    aliases: &["glv", "polearm"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 10),
    damage_type: DamageType::Slashing,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
};

/// Gnoll Pack Lord multiattack — 2 glaive swings per Action. The pack
/// lord's signature move: two reach-2 slashing strikes that let it
/// command the battle line from behind the front rank.
pub static GNOLL_PACK_LORD_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double glaive",
    sub_attack: &GLAIVE,
    count: 2,
});

/// Spectator Eye Ray — ranged spell attack modeled as a single-target
/// beam. INT-based attack roll, 3d10 force damage, 24-tile range
/// (≈60 ft). The MM spectator has four distinct eye rays (confusion,
/// fear, wounding, paralyzing); we collapse them into one high-damage
/// force beam to keep the action economy simple while preserving the
/// "ranged magical zap" identity.
pub struct SpectatorEyeRay {}

impl Action for SpectatorEyeRay {
    fn name(&self) -> &str {
        "eye ray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["er", "ray"]
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
        // INT-based ranged spell attack — no ability mod to damage (pure
        // magical energy, like a cantrip).
        simple_weapon_attack_ranged(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Intelligence,
            None,
            Dice::new(3, 10),
            DamageType::Force,
            false,
            Some(24),
        )
    }
}

pub static SPECTATOR_EYE_RAY: LazyLock<SpectatorEyeRay> = LazyLock::new(|| SpectatorEyeRay {});

/// Javelin -- STR-based thrown weapon: 1d6 piercing, 30ft normal / 120ft max.
pub static JAVELIN: SimpleWeapon = SimpleWeapon {
    display_name: "javelin",
    aliases: &["jav", "throw"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Piercing,
    reach: 48,
    is_melee: false,
    requires_los: true,
    cost_resource: Resource::Action,
    normal_range: Some(12),
};

/// Hobgoblin Warlord multiattack -- three longsword swings per Action.
pub static HOBGOBLIN_WARLORD_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "triple longsword",
    sub_attack: &LONGSWORD,
    count: 3,
});
