use std::collections::HashSet;
use std::sync::LazyLock;

use crate::engine::attack::{CHARGE_RUN_TILES, ChargeRider, charge_run_tiles};
use crate::{
    actions::action_template::{
        Action, MELEE_REACH, TargetingSchema, actor_has_recharge, bonus_action_only,
        first_target_id, first_target_location, target_has_condition,
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
        damage_ability, damage_dice, damage_type, is_melee, None, None,
    )
}

/// On a confirmed weapon-attack hit, roll `rider_dice` of `rider_type`
/// extra damage, log it in the standard rider format, and push the
/// resulting `DealDamage` onto `effects`. Returns the rolled rider amount
/// for callers that need to chain further effects scaled by it.
///
/// Centralizes the "weapon hit + flat typed-damage rider" pattern that
/// recurs across Mummy Lord / Rakshasa / Yeti / Dragon Bite / Death
/// Knight Longsword / Vampiric Bite / etc. Caller is responsible for
/// the upstream `resolve_attack_outcome` and the `damage == 0` early-
/// return — this helper assumes the swing landed. Distinct from
/// `save_or_damage_rider` (which gates the rider on a saving throw):
/// this rider is unconditional on a hit.
pub fn add_flat_damage_rider(
    encounter: &mut EncounterInstance,
    target_id: usize,
    rider_dice: Dice,
    rider_type: DamageType,
    rider_name: &str,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> u32 {
    let amt = encounter.roll(&rider_dice);
    encounter.log(format!(
        "  {}: {}({}) = {} {} rider",
        rider_name, rider_dice, amt, amt, rider_type
    ));
    effects.push(Box::new(DealDamage {
        actor_id: target_id,
        amount: amt,
        damage_type: rider_type,
    }));
    amt
}

/// Resolve a weapon swing AND an unconditional flat typed-damage rider on
/// a confirmed hit, in one call. Wraps the recurring three-step pattern:
///
/// 1. `weapon_swing_with_damage` — resolves the d20 swing, returns the
///    damage actually dealt (post-mitigation) and the side-effects list.
/// 2. Early-return on miss (`effects.is_empty()`) — `resolve_attack_outcome`
///    returns `(Vec::new(), 0)` on miss / Sanctuary block / Mirror Image
///    deflect, so an empty effects list is the cleanest hit/miss signal.
///    (Using `damage == 0` as the gate would incorrectly suppress the
///    rider on the rare "hit but Uncanny Dodge / Deflect Missiles zeroed
///    the post-mitigation damage" case — RAW the swing landed and the
///    rider should fire.)
/// 3. `add_flat_damage_rider` — rolls and pushes the typed-damage rider.
///
/// Used by every "weapon hit + one typed-damage rider, no save, no chain"
/// attack: Dragon Bite (fire), Mummy Lord Rotting Fist (necrotic),
/// Yuan-Ti Bite (poison), Death Knight Longsword (necrotic), Wereboar
/// Tusks (fire), Magmin Touch (fire), Djinni Scimitar (thunder), Efreeti
/// Scimitar (fire), Giant Constrictor Bite (poison). Cuts each impl's
/// `side_effects` block from ~20 lines of plumbing to a single call.
///
/// Use this when:
///   1. The rider's typing differs from the base (the main reason — fire
///      on a piercing bite, necrotic on a slashing claw, etc.), AND
///   2. There's no per-rider gate on damage, save, or downstream chain —
///      the rider is unconditional on a hit and its return value isn't
///      consumed by the caller.
///
/// For save-gated riders, use `save_or_damage_rider`; for chained logic
/// (Wraith Drain's max-HP drop scaled by the necrotic dealt), call
/// `add_flat_damage_rider` directly so the caller can use its return.
#[allow(clippy::too_many_arguments)]
pub fn weapon_swing_with_flat_rider(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &'static str,
    ability: AbilityScoreType,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
    rider_dice: Dice,
    rider_type: DamageType,
    rider_name: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let (mut effects, _damage) = weapon_swing_with_damage(
        encounter,
        caster_id,
        target_id,
        action_name,
        ability,
        damage_dice,
        damage_type,
        is_melee,
        None,
    );
    // Hit/miss gate: an empty effects list means the d20 didn't connect
    // (`resolve_attack_outcome` returns `(Vec::new(), 0)` on miss /
    // Sanctuary-block / Mirror-Image-deflect). Checking the post-mitigation
    // `damage` value as the gate would skip the rider on the rare "hit but
    // Uncanny Dodge / Deflect Missiles reduced damage to 0" case — RAW the
    // swing landed and the rider should fire. Same chassis-wide
    // convention as the other weapon-rider helpers.
    if effects.is_empty() {
        return effects;
    }
    add_flat_damage_rider(
        encounter,
        target_id,
        rider_dice,
        rider_type,
        rider_name,
        &mut effects,
    );
    effects
}

/// On a weapon-attack hit, roll `target_id`'s saving throw against `dc`
/// using `save_ability`. On fail, push a `DealDamage` rider of
/// `rider_dice` typed as `rider_type`, logging the roll with
/// `rider_name` as the prefix (e.g. `"  imp venom: 2d10(7) = 7 poison"`).
/// Returns the save outcome so the caller can chain additional effects
/// (condition installs, max-HP drops, etc.) on the same failed save.
///
/// Centralizes the "weapon swing + on-hit save-or-extra-damage rider"
/// pattern shared by Imp Sting / Spider Bite / Quasit Claws / Giant
/// Scorpion Sting / Drow Hand Crossbow / Wyvern Stinger / etc. — one
/// chokepoint for the save-roll + damage-log + DealDamage push, so a
/// future tweak (e.g. routing the rider through a `condition_damage_
/// taken` hook for Spirit Shroud-style retaliation) lands once instead
/// of being scattered across the ~19 weapon+rider call sites.
#[allow(clippy::too_many_arguments)]
pub fn save_or_damage_rider(
    encounter: &mut EncounterInstance,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    rider_dice: Dice,
    rider_type: DamageType,
    rider_name: &str,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> crate::engine::saves::SaveOutcome {
    let save = encounter.roll_save(target_id, save_ability, dc);
    if !save.passed() {
        let amt = encounter.roll(&rider_dice);
        encounter.log(format!(
            "  {}: {}({}) = {} {}",
            rider_name, rider_dice, amt, amt, rider_type
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: amt,
            damage_type: rider_type,
        }));
    }
    save
}

/// On a weapon-attack hit, roll `target_id`'s saving throw against `dc`
/// using `save_ability`. On fail, push an `ApplyCondition` rider installing
/// `condition` with `timer`, logging the install through `rider_name` so
/// the log line reads e.g. `"  death dog disease: target sickens"`. Returns
/// the save outcome so the caller can chain further per-fail side-effects.
///
/// The condition-immunity check is the install-site responsibility (the
/// engine's `add_condition` chokepoint already swallows immune installs);
/// callers that want to skip the save roll entirely for known-immune
/// targets should short-circuit before calling this helper.
///
/// Mirrors `save_or_damage_rider` in shape — the two together cover the
/// "hit + save or X" pattern shared by Sleep-arrow / Death-dog disease /
/// Spider-bite-poison / Sea-hag death-glare and friends. Distinct from
/// the damage variant because the install path is fundamentally different
/// (DealDamage vs ApplyCondition), and many riders need BOTH (e.g. Spider
/// Bite: extra poison damage AND Poisoned condition on the same failed
/// save) — those callers call both helpers in sequence with shared `save`
/// to compose the per-hit rider.
#[allow(clippy::too_many_arguments)]
pub fn save_or_condition_rider(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    condition: Condition,
    timer: ConditionTimer,
    rider_name: &str,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> crate::engine::saves::SaveOutcome {
    let save = encounter.roll_save(target_id, save_ability, dc);
    if !save.passed() {
        encounter.log(format!("  {}: target fails the save", rider_name));
        // Through the linked installer, so a rider whose condition
        // carries a back-link records who applied it. Every caller
        // already had the attacker in hand and was throwing it away at
        // this line; a roper's tendril that grapples nobody in
        // particular is a hold nothing can end.
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            condition, target_id, caster_id, timer,
        ));
    }
    save
}

/// Resolve a single-target "save-or-charmed-by-caster" install. Returns
/// the side-effects produced (empty when the target is charm-immune or
/// saves). Pre-checks `effectively_immune_to_condition(Charmed)` so the
/// charm-immune log fires before the save roll (matches the canonical
/// short-circuit shared by Vampire Charming Gaze / Dryad Fey Charm).
/// On a failed save, pushes both an `ApplyCondition(Charmed)` and a
/// `SetConditionLink(Charmed ← caster)` so the engine's "can't act hostile against
/// your charmer" gate is wired up correctly.
///
/// Centralizes the "immunity-check + save + Charmed install + SetConditionLink(Charmed)"
/// loop shared by every single-target charm action — keeps the log
/// shape uniform ("{rider}: target's mind is shielded" / "target resists"
/// / "target is enthralled") and the charm-link bookkeeping in one place
/// so a future charm-pipeline tweak (e.g. honoring a "save with advantage
/// while wearing a Charm Amulet" prime) lands once instead of being
/// re-implemented across the three current call sites.
pub fn save_or_charmed_by_caster(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    timer: ConditionTimer,
    rider_name: &str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    use crate::engine::side_effects::install_condition_with_link;
    let Some(target) = encounter.actors.get(&target_id) else {
        return Vec::new();
    };
    if target.effectively_immune_to_condition(Condition::Charmed) {
        encounter.log(format!("  {}: target's mind is shielded", rider_name));
        return Vec::new();
    }
    let save = encounter.roll_save(target_id, save_ability, dc);
    if save.passed() {
        encounter.log(format!("  {}: target resists the enchantment", rider_name));
        return Vec::new();
    }
    encounter.log(format!("  {}: target is enthralled", rider_name));
    install_condition_with_link(Condition::Charmed, target_id, caster_id, timer)
}

/// On an Action-cost weapon swing, conditionally run a second swing if
/// the caster has Extra Attack and this invocation isn't already inside
/// a `Multiattack` / `CompoundAttack` expansion. Logs `"  Extra Attack:"`
/// before the chained swing and extends `effects` with whatever it
/// returns. No-op when either gate fails.
///
/// Centralizes the recurring 7-line tail block on every WeaponWith*
/// chassis's `side_effects` impl (`SimpleWeapon`, `WeaponWithRider`,
/// `WeaponWithSaveCondition`, `WeaponWithSaveDamage`,
/// `WeaponWithCondition`). Before extraction, the same
/// `!in_multiattack() && actor.has_extra_attack()` gate and the same
/// `effects.extend(swing(encounter))` chain were re-stamped at five
/// chassis sites — a future tweak to chain semantics (e.g. a third
/// swing for a hypothetical "Extra Attack (Improved)" feat, gating on
/// a per-swing resource other than `has_extra_attack`, or a different
/// log prefix on the chained line) now lands in one place instead of
/// being scattered across the chassis impls. The
/// `Multiattack` / `CompoundAttack` chassis themselves still gate the
/// chain off via the `in_multiattack()` depth counter so a 3-claw
/// Compound on a creature that also has Extra Attack doesn't silently
/// promote to 6 swings.
///
/// The closure form (`FnMut(&mut EncounterInstance) -> Vec<...>`) lets
/// each chassis package its swing-and-rider chain — including the
/// hit/miss gate via `effects.is_empty()` and the save / install riders
/// — into a single closure that the helper re-invokes verbatim, so the
/// rider lands on each Extra Attack hit too (RAW: Extra Attack is a
/// second swing, not a second action — every rider rides every hit).
pub fn maybe_chain_extra_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    mut swing: impl FnMut(&mut EncounterInstance) -> Vec<Box<dyn ApplicableSideEffect>>,
) {
    // Suppress Extra Attack when this swing was invoked from inside a
    // Multiattack / CompoundAttack expansion — the wrapper already
    // encodes the per-Action swing count, and double-counting it (e.g.
    // Ancient Blue Dragon's 3-claw Multi) silently doubles a boss
    // creature's per-turn damage budget.
    if encounter.in_multiattack() {
        return;
    }
    if !encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.has_extra_attack())
    {
        return;
    }
    encounter.log("  Extra Attack:");
    effects.extend(swing(encounter));
}

/// Resolve a single weapon swing whose attack and damage modifiers both
/// derive from the same ability (the standard "STR-to-hit STR-to-damage"
/// shape), at an arbitrary reach. Returns `(effects, damage_dealt)` so the
/// caller can chain riders that gate on the actual damage (a save-or-
/// extra-damage clause, a max-HP drain equal to the necrotic dealt, a
/// self-heal equal to half the damage, etc.). Returns `(empty, 0)` on a
/// missing caster / target — same fail-quiet contract as
/// `simple_weapon_attack`.
///
/// Centralizes the recurring 4-line `caster.ability_modifier(X) +
/// proficiency_bonus() / caster.ability_modifier(X) / resolve_attack_outcome
/// with AttackParams { ... }` block used by Rakshasa Claw, Mummy Lord
/// Rotting Fist, Vampire Bite, Yeti Claw, and the new Spirit Naga Bite /
/// Otyugh Tentacle. Companion to `simple_weapon_attack` (single-return,
/// vanilla MELEE_REACH); use this variant when you need the damage value
/// for a rider OR a non-standard reach (10ft reach-2 tentacles, etc.).
///
/// `reach` is in tile-gap units — `MELEE_REACH` for a standard 5ft swing,
/// 2 for a 10ft reach weapon, etc. Pass `is_spell = false` (the default
/// for weapon swings); spell-attack variants should still build their
/// own `AttackParams` since they typically need a different attack
/// ability (spellcasting mod) and the metamagic-prime flag.
#[allow(clippy::too_many_arguments)]
pub fn weapon_swing_with_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &'static str,
    ability: AbilityScoreType,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
    long_range: Option<isize>,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return (Vec::new(), 0);
    };
    let attack_mod = caster.spell_attack_modifier(ability);
    let damage_mod = caster.ability_modifier(ability);
    crate::engine::attack::resolve_attack_outcome(
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
            long_range,
            // Nobody reaches this helper with a lance: it is the
            // bespoke-monster-swing lane, and the close-quarters clause
            // lives on `SimpleWeapon`, which has its own path to
            // `resolve_attack`.
            min_range: None,
            is_spell: false,
        },
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
    min_effective_range: Option<isize>,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(target_id) = first_target_id(target_ids) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let attack_mod = caster.spell_attack_modifier(attack_ability);
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
            min_range: min_effective_range,
            is_spell: false,
        },
    )
}

/// Boar **Charge** (RAW): "extra 3 (1d6) slashing damage. If the target
/// is a creature, it must succeed on a DC 11 Strength saving throw or be
/// knocked prone."
pub const BOAR_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("tusks"),
    dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "boar charge",
    knockdown_label: "boar charge knockdown",
    once_per_turn_tag: None,
};

/// Giant Boar **Charge** (RAW): extra 7 (2d6) slashing, DC 13 Strength
/// or prone.
pub const GIANT_BOAR_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("giant boar tusks"),
    dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "giant boar charge",
    knockdown_label: "giant boar charge knockdown",
    once_per_turn_tag: None,
};

/// Elk **Charge** (RAW): extra 7 (2d6) bludgeoning, DC 13 Strength or
/// prone.
pub const ELK_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("elk ram"),
    dice: Dice::new(2, 6),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "elk charge",
    knockdown_label: "elk charge knockdown",
    once_per_turn_tag: None,
};

/// Goat **Charge** (RAW): extra 2 (1d4) bludgeoning, DC 10 Strength or
/// prone.
pub const GOAT_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("goat ram"),
    dice: Dice::new(1, 4),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "goat charge",
    knockdown_label: "goat charge knockdown",
    once_per_turn_tag: None,
};

/// Giant Goat **Charge** (RAW): extra 5 (2d4) bludgeoning, DC 13
/// Strength or prone.
pub const GIANT_GOAT_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("giant goat ram"),
    dice: Dice::new(2, 4),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "giant goat charge",
    knockdown_label: "giant goat charge knockdown",
    once_per_turn_tag: None,
};

/// Unicorn **Charge** (RAW): extra 9 (2d8) piercing, DC 15 Strength or
/// prone. Rides the horn, which is why the horn and not the hooves is
/// the limb worth closing distance for.
pub const UNICORN_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("unicorn horn"),
    dice: Dice::new(2, 8),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "unicorn charge",
    knockdown_label: "unicorn charge knockdown",
    once_per_turn_tag: None,
};

/// Centaur **Charge** (RAW): "If the centaur moves at least 30 feet
/// straight toward a target and then hits it with a pike attack on the
/// same turn, the target takes an extra 10 (3d6) piercing damage." The
/// one clause in the SRD with a longer run-up and no knockdown.
pub const CENTAUR_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("pike"),
    dice: Dice::new(3, 6),
    damage_type: DamageType::Piercing,
    run_tiles: charge_run_tiles(30),
    knocks_prone: false,
    label: "centaur charge",
    knockdown_label: "",
    once_per_turn_tag: None,
};

/// Triceratops **Trampling Charge** (RAW): no extra damage, "that target
/// must succeed on a DC 13 Strength saving throw or be knocked prone."
/// The bonus stomp against a target it flattens is the unmodeled half —
/// a mid-resolution action-economy grant the damage path has no hook
/// for.
pub const TRICERATOPS_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("gore"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "trampling charge",
    knockdown_label: "trampling charge knockdown",
    once_per_turn_tag: None,
};

/// Warhorse **Trampling Charge** (RAW): DC 14 Strength or prone, then a
/// bonus hoof attack against a target it knocks down. Same unmodeled
/// second half as the triceratops.
pub const WARHORSE_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("warhorse hooves"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "warhorse trampling charge",
    knockdown_label: "warhorse trampling charge knockdown",
    once_per_turn_tag: None,
};

/// Tiger **Pounce** (RAW): "If the tiger moves at least 20 feet straight
/// toward a creature and then hits it with a claw attack on the same
/// turn, that target must succeed on a DC 13 Strength saving throw or be
/// knocked prone." The free bite against a flattened target is the
/// unmodeled half.
pub const TIGER_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "tiger pounce",
    knockdown_label: "tiger pounce knockdown",
    once_per_turn_tag: None,
};

/// Lion **Pounce** (RAW): identical to the tiger's, on the lion's own
/// claw. Two constants rather than one shared `POUNCE` because the log
/// line names the cat, and because the two stat blocks are free to drift
/// apart the way the boar and the giant boar already have.
pub const LION_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "lion pounce",
    knockdown_label: "lion pounce knockdown",
    once_per_turn_tag: None,
};

/// Minotaur **Charge** (RAW): "If the minotaur moves at least 10 feet
/// straight toward a target and then hits it with a gore attack on the
/// same turn, the target takes an extra 9 (2d8) piercing damage. If the
/// target is a creature, it must succeed on a DC 14 Strength saving
/// throw or be pushed up to 10 feet away and knocked prone." The push is
/// the unmodeled half — the knockdown is what changes the fight, and a
/// charge row carries one follow-up.
///
/// The shortest run-up in the bestiary, which suits the creature: a
/// minotaur in a labyrinth rarely has twenty feet of corridor to build
/// up in.
pub const MINOTAUR_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("gore"),
    dice: Dice::new(2, 8),
    damage_type: DamageType::Piercing,
    run_tiles: charge_run_tiles(10),
    knocks_prone: true,
    label: "minotaur charge",
    knockdown_label: "minotaur charge knockdown",
    once_per_turn_tag: None,
};

/// Wereboar **Charge** (RAW): fifteen feet, extra 7 (2d6) slashing, DC
/// 13 Strength or prone. Rides the tusks, so a wereboar in humanoid form
/// swinging its maul charges nobody — which is exactly the distinction
/// naming the weapon per clause exists to draw.
pub const WEREBOAR_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("wereboar tusks"),
    dice: Dice::new(2, 6),
    damage_type: DamageType::Slashing,
    run_tiles: charge_run_tiles(15),
    knocks_prone: true,
    label: "wereboar charge",
    knockdown_label: "wereboar charge knockdown",
    once_per_turn_tag: None,
};

/// Saber-toothed Tiger **Pounce** (RAW): twenty feet, claw attack, DC 14
/// Strength or prone. The same clause the ordinary tiger and the lion
/// carry, on a much heavier cat.
pub const SABER_TIGER_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("saber claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "saber-toothed pounce",
    knockdown_label: "saber-toothed pounce knockdown",
    once_per_turn_tag: None,
};

/// Mammoth **Trampling Charge** (RAW): twenty feet, gore attack, DC 18
/// Strength or prone — and then a bonus stomp against a target it
/// flattens, which is the unmodeled half here as it is on the
/// triceratops and the warhorse.
///
/// This one replaced a bespoke `Action`. The mammoth used to carry a
/// second, near-duplicate gore called "trampling charge" — same 4d8, a
/// hand-rolled DC-18 save rider, and a Recharge 5-6 gate standing in for
/// the movement clause because, as its own comment said, "the engine
/// can't introspect path geometry at attack time". It can, so the
/// stand-in is gone: the mammoth has one gore, and the clause fires when
/// the mammoth has actually thundered twenty feet at somebody. Which is
/// also stricter than the recharge was — a mammoth standing still could
/// trample on a lucky d6.
pub const MAMMOTH_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("mammoth gore"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "trampling charge",
    knockdown_label: "trampling charge knockdown",
    once_per_turn_tag: None,
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
    /// 5e normal range (in tiles) for ranged weapons. Attacks beyond this
    /// distance but within `reach` (max range) impose disadvantage. `None`
    /// means no long-range penalty (melee weapons). Longbow: 12 tiles
    /// (30ft normal), reach 20 tiles (50ft max). Shortbow: 8 tiles, reach 12.
    pub normal_range: Option<isize>,
    /// Self-condition the wielder must be holding for the swing to
    /// validate, or `None` for the ordinary weapon that is simply
    /// always there.
    ///
    /// Exists for weapons that are not objects: the Way of the Astral
    /// Self Monk's spectral arms are summoned by a bonus action and go
    /// away again, and while they are up they are a different weapon
    /// from the monk's fists in all four of the ways `SimpleWeapon`
    /// already describes — ability, damage type, dice, reach. Modeling
    /// that as a second weapon which refuses to validate without its
    /// form is strictly less machinery than four runtime overrides
    /// layered onto the first one, and it puts the arms on the same
    /// footing as every other weapon: the AI's attack picker ranks it,
    /// Extra Attack chains it, the prompt parser names it.
    ///
    /// The gate is checked in `custom_validate_input`, so it applies
    /// everywhere validation does — the AI's picker, the human prompt,
    /// and the re-check `execute` makes between enqueue and resolution.
    /// A form that lapses mid-turn therefore cancels the swing it was
    /// going to pay for rather than resolving on arms that are no
    /// longer there.
    pub requires_condition: Option<Condition>,
    /// The footprint gap *below* which this weapon's swing rolls at
    /// disadvantage, or `None` for the ordinary weapon that is equally
    /// happy at any range it can reach.
    ///
    /// The mirror of `normal_range`, and it exists for one weapon: 5e's
    /// **lance**. "You have disadvantage when you use a lance to attack
    /// a target within 5 feet of you" is the clause that makes a lance a
    /// mounted weapon rather than just a long spear, and it is the half
    /// of the entry the engine used to drop — the Knight's lance shipped
    /// with a note saying the mount gate had been removed because there
    /// were no mounts. There are now.
    ///
    /// Read on both sides of the swing: `simple_weapon_attack` passes it
    /// into `AttackParams::min_range` so the die knows, and the AI's
    /// attack picker reads it through `Action::min_effective_reach` so a
    /// knight with a longsword on their belt doesn't jab with the wrong
    /// end of a lance at point-blank.
    pub min_effective_range: Option<isize>,
}

impl SimpleWeapon {
    /// Const constructor for the standard "melee swing using one ability
    /// for both attack and damage" shape: STR-based 1d6 piercing bite,
    /// DEX-based 1d4 piercing dagger, etc. Pins the boilerplate fields
    /// (`is_melee = true`, `reach = MELEE_REACH`, `requires_los = false`,
    /// `cost_resource = Action`, `normal_range = None`,
    /// `damage_ability = Some(attack_ability)`) so a new attack literal
    /// collapses from a 12-field struct expression to a 5-argument call.
    /// Callers who need a non-standard reach can call `reach_melee`; for
    /// the bonus-action / no-mod / no-LOS edge cases the struct form is
    /// the right escape hatch.
    pub const fn melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
    ) -> Self {
        Self::reach_melee(
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            MELEE_REACH,
        )
    }

    /// Const constructor for the "melee swing with an extended reach"
    /// shape — the giant's greatclub (reach 2 = 10ft), the wyvern's tail
    /// stinger (reach 3 = 15ft), the dragon's claw (reach 2). Identical
    /// to `melee` but lets the caller pin a custom reach in tile-gap
    /// units. Shrinks a ~12-field struct literal to a 6-argument call so
    /// reach-2+ natural weapons stop carrying the same `is_melee = true,
    /// requires_los = false, cost_resource = Action, normal_range = None,
    /// damage_ability = Some(attack_ability)` boilerplate at every
    /// declaration site.
    pub const fn reach_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        reach: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_ability: Some(attack_ability),
            damage_dice,
            damage_type,
            reach,
            is_melee: true,
            requires_los: false,
            cost_resource: Resource::Action,
            normal_range: None,
            requires_condition: None,
            min_effective_range: None,
        }
    }

    /// Const constructor for the "flat-dice melee swing" shape —
    /// `damage_ability = None` so the damage roll *omits* the to-hit
    /// ability's modifier. Matches RAW's small handful of natural
    /// attacks where the stat block lists `Hit: N (XdY)` instead of
    /// the standard `Hit: N (XdY + STR)`: Dretch Bite / Dretch Claws
    /// (RAW 3 (1d6) / 5 (2d4) — STR 11 = +0 so the distinction is
    /// moot but the data shape is preserved), Lemure Fist (RAW 2 (1d4)
    /// — STR 10 = +0), Pseudodragon Bite (RAW 1 piercing flat — DEX 15
    /// would have added +2), and Camel Bite (RAW 2 (1d4) — STR 16
    /// would have added +3). Pins `is_melee = true`,
    /// `reach = MELEE_REACH`, `damage_ability = None`, and the rest of
    /// the boilerplate (LOS / Action-cost / no long range) so a flat-
    /// damage natural attack collapses from a 12-field struct literal
    /// to a 5-argument call. Callers who need a custom reach can fall
    /// back to the struct form; the engine-side gate `damage_ability:
    /// None` is what matters.
    pub const fn flat_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_ability: None,
            damage_dice,
            damage_type,
            reach: MELEE_REACH,
            is_melee: true,
            requires_los: false,
            cost_resource: Resource::Action,
            normal_range: None,
            requires_condition: None,
            min_effective_range: None,
        }
    }

    /// Const constructor for the standard "Action-cost ranged weapon
    /// attack" shape — longbow / heavy crossbow / hill giant boulder /
    /// frost giant rock. Pins the boilerplate fields (`is_melee = false`,
    /// `requires_los = true`, `cost_resource = Action`,
    /// `damage_ability = Some(attack_ability)`) so a ranged-weapon literal
    /// collapses from a 12-field struct expression to a 7-argument call.
    /// `reach` is the maximum effective range in tile-gap units (5e's
    /// "long range" — attacks beyond `normal_range` but within `reach`
    /// roll at disadvantage). Callers who need a bonus-action shot
    /// (Shortbow) should keep the struct form.
    #[allow(clippy::too_many_arguments)]
    pub const fn ranged(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        reach: isize,
        normal_range: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_ability: Some(attack_ability),
            damage_dice,
            damage_type,
            reach,
            is_melee: false,
            requires_los: true,
            cost_resource: Resource::Action,
            normal_range: Some(normal_range),
            requires_condition: None,
            min_effective_range: None,
        }
    }

    /// Const builder that gates an already-constructed weapon on a
    /// self-condition — `SimpleWeapon::reach_melee(...).gated_on(
    /// Condition::AstralArms)`.
    ///
    /// A builder rather than a fifth constructor because the gate is
    /// orthogonal to every shape above it: a summoned weapon could be
    /// melee, reach-melee, flat-damage or ranged, and pairing the gate
    /// with each of those would be four near-identical constructors to
    /// keep in step. Written out field-by-field rather than with
    /// functional-record-update syntax, which `const fn` does not
    /// accept.
    pub const fn gated_on(self, condition: Condition) -> Self {
        Self {
            display_name: self.display_name,
            aliases: self.aliases,
            attack_ability: self.attack_ability,
            damage_ability: self.damage_ability,
            damage_dice: self.damage_dice,
            damage_type: self.damage_type,
            reach: self.reach,
            is_melee: self.is_melee,
            requires_los: self.requires_los,
            cost_resource: self.cost_resource,
            normal_range: self.normal_range,
            requires_condition: Some(condition),
            min_effective_range: None,
        }
    }
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
    fn min_effective_reach(&self) -> Option<isize> {
        self.min_effective_range
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
    /// A weapon that has to be summoned before it can be swung — see
    /// `requires_condition`. Ungated weapons (every one but the Astral
    /// Self Monk's arms) short-circuit to `true` without touching the
    /// actor map.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(required) = self.requires_condition else {
            return true;
        };
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(required))
    }
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        crate::actions::action_template::weapon_expected_damage_named(
            encounter,
            caster_id,
            self.display_name,
            self.damage_dice,
            self.damage_ability,
            self.cost_resource,
            0,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Folds the "one swing + optional Extra-Attack second swing"
        // chain into a single closure so the eight `self.*` arguments
        // don't have to be enumerated twice. The Extra Attack rider
        // only fires on Action-cost swings: bonus-action bow shots and
        // reaction strikes don't get the second hit per RAW.
        let swing = |e: &mut EncounterInstance| {
            simple_weapon_attack_ranged(
                e,
                caster_id,
                target_ids,
                self.name(),
                self.attack_ability,
                self.damage_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                self.normal_range,
                self.min_effective_range,
            )
        };
        let mut effects = swing(encounter);
        // Extra Attack chain — only on Action-cost swings (bonus-action
        // bow shots and reaction strikes don't get the second hit per
        // RAW). The helper handles the `!in_multiattack()` and
        // `has_extra_attack()` gates uniformly across every WeaponWith*
        // chassis.
        if self.cost_resource == Resource::Action {
            maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        }
        effects
    }
}

/// A `SimpleWeapon`-shaped attack with one extra flat damage rider on a
/// confirmed hit. Same "STR-to-hit / STR-to-damage" backbone as
/// `SimpleWeapon`, but the swing's `side_effects` adds an unconditional
/// typed-damage rider (e.g. 2d6 fire on top of a 2d6 slashing scimitar)
/// via the shared `weapon_swing_with_flat_rider` helper.
///
/// Collapses a swath of ~25-line `impl Action for FooWeapon` blocks that
/// only differed in their five rider/swing constants — Mummy Fist
/// (bludgeoning + necrotic), Yeti Claw (slashing + cold), Dragon Bite
/// (piercing + fire), Mummy Lord Rotting Fist, Rakshasa Claw, Black
/// Pudding Pseudopod, Djinni Scimitar (slashing + thunder), Efreeti
/// Scimitar (slashing + fire), and Giant Constrictor Bite — into
/// data-only `const` declarations.
///
/// Like `SimpleWeapon`, an Action-cost swing is automatically followed
/// by a second swing if the caster has Extra Attack and the swing isn't
/// inside a `Multiattack` expansion — so the rider lands on each Extra
/// Attack hit too (matches the existing per-impl behavior). The rider
/// fires only on hits (`damage > 0`), so a miss costs only the swing
/// log line, not the rider.
pub struct WeaponWithRider {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
    pub is_melee: bool,
    pub rider_dice: Dice,
    pub rider_type: DamageType,
    pub rider_name: &'static str,
}

impl WeaponWithRider {
    /// Const constructor for the standard "STR-based 1H melee swing with a
    /// typed rider" shape. Pins `reach = MELEE_REACH`, `is_melee = true`,
    /// and uses the same ability for attack + damage. A new weapon-with-
    /// rider literal becomes a single `WeaponWithRider::melee(...)` call
    /// instead of a 10-field struct expression plus a 25-line `impl
    /// Action` block.
    #[allow(clippy::too_many_arguments)]
    pub const fn melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
    ) -> Self {
        Self::reach_melee(
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            MELEE_REACH,
            rider_dice,
            rider_type,
            rider_name,
        )
    }

    /// Const constructor for the "weapon-with-rider with extended melee
    /// reach" shape — the dragon bite (reach 2 = 10ft + fire rider). Mirrors
    /// `SimpleWeapon::reach_melee` but with the additional rider triple
    /// (dice + type + name). Lets reach-2+ natural weapons with a typed
    /// rider drop from a 10-field struct literal to a 9-argument call.
    #[allow(clippy::too_many_arguments)]
    pub const fn reach_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        reach: isize,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: true,
            rider_dice,
            rider_type,
            rider_name,
        }
    }
}

impl Action for WeaponWithRider {
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
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type, self.rider_type]
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
        // Mirror `SimpleWeapon`'s swing-+-Extra-Attack chain so creatures
        // with the `has_extra_attack` flag get a second hit on Action-cost
        // swings (suppressed inside a Multiattack expansion to avoid
        // double-counting). Each swing carries its own rider — RAW: Extra
        // Attack is a second swing, not a second action, so the rider
        // rides every successful hit in the chain.
        let swing = |e: &mut EncounterInstance| {
            weapon_swing_with_flat_rider(
                e,
                caster_id,
                target_id,
                self.display_name,
                self.attack_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                self.rider_dice,
                self.rider_type,
                self.rider_name,
            )
        };
        let mut effects = swing(encounter);
        maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        effects
    }
}

/// A `SimpleWeapon`-shaped attack with a save-or-condition rider on a
/// confirmed hit. Same to-hit / damage backbone as `SimpleWeapon`, but the
/// swing's `side_effects` adds a `save_or_condition_rider` install on hit:
/// roll the target's save against `save_dc`; on fail, queue an
/// `ApplyCondition(condition, timer)`. Per-target condition-immunity is
/// handled by the standard `add_condition` chokepoint.
///
/// Companion to `WeaponWithRider` (which lands a flat typed-damage rider on
/// hit). Collapses the recurring "weapon swing + save_or_condition_rider"
/// shape used by Wolf Bite (DC 11 STR -> Prone), Dire Wolf Bite (DC 13 STR
/// -> Prone), and similar trip-style bites — each previously a hand-rolled
/// ~40-line `impl Action` block that varied only in the seven scalar
/// fields exposed here.
///
/// Extra Attack chains the same way as `WeaponWithRider`: an Action-cost
/// swing rolled outside a `Multiattack` triggers a second swing with its
/// own save-or-condition rider for creatures with `has_extra_attack`.
///
/// Distinct from the parametric `LycanthropeBite` (CON save -> Poisoned 3
/// rounds, used only by the wereXX cohort): that chassis is keyed on the
/// shared lycanthropy flavor (always CON / always Poisoned), while
/// `WeaponWithSaveCondition` exposes the save ability, condition, and
/// timer as fields so a Worg-style STR-vs-Prone bite and a future
/// scorpion-tail-vs-Poisoned variant can share the lane.
pub struct WeaponWithSaveCondition {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
    pub is_melee: bool,
    pub save_ability: AbilityScoreType,
    pub save_dc: i32,
    pub condition: Condition,
    pub timer: ConditionTimer,
    pub rider_name: &'static str,
}

impl WeaponWithSaveCondition {
    /// Const constructor for the standard "STR-based 1H melee swing whose
    /// hit forces a save-or-condition rider" shape (the wolf-style trip
    /// bite). Pins `reach = MELEE_REACH`, `is_melee = true`, and uses the
    /// same ability for attack + damage.
    #[allow(clippy::too_many_arguments)]
    pub const fn melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        condition: Condition,
        timer: ConditionTimer,
        rider_name: &'static str,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach: MELEE_REACH,
            is_melee: true,
            save_ability,
            save_dc,
            condition,
            timer,
            rider_name,
        }
    }

    /// Long-reach melee variant. Same as `melee()` but takes an explicit
    /// `reach` in tiles, for save-or-condition weapons like the giant
    /// constrictor snake's reach-2 coil or the giant octopus's reach-3
    /// tentacles. Mirrors `SimpleWeapon::reach_melee` / `WeaponWithRider::
    /// reach_melee` so the long-reach lane is one declaration on every
    /// save-or-condition chassis instead of a struct-literal sprawl.
    #[allow(clippy::too_many_arguments)]
    pub const fn reach_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        condition: Condition,
        timer: ConditionTimer,
        rider_name: &'static str,
        reach: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: true,
            save_ability,
            save_dc,
            condition,
            timer,
            rider_name,
        }
    }
}

impl Action for WeaponWithSaveCondition {
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Mirror `WeaponWithRider`'s swing-+-Extra-Attack chain so the
        // rider lands on every successful hit in the chain (RAW: Extra
        // Attack is a second swing, not a second action).
        let swing = |e: &mut EncounterInstance| {
            let (mut effects, _dealt) = weapon_swing_with_damage(
                e,
                caster_id,
                target_id,
                self.display_name,
                self.attack_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                None,
            );
            // Hit/miss gate via the effects vec — see
            // `weapon_swing_with_flat_rider` for the same chassis-wide
            // rationale. Skipping on `dealt == 0` would incorrectly
            // suppress the save-or-condition install on the rare
            // "hit but Uncanny Dodge / Deflect Missiles zeroed damage"
            // case (RAW: a trip-prone save fires on hit, not on
            // damage > 0).
            if effects.is_empty() {
                return effects;
            }
            save_or_condition_rider(
                e,
                caster_id,
                target_id,
                self.save_ability,
                self.save_dc,
                self.condition,
                self.timer,
                self.rider_name,
                &mut effects,
            );
            effects
        };
        let mut effects = swing(encounter);
        maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        effects
    }
}

/// Data-only `Action` chassis for the "weapon swing → on hit, target saves
/// vs DC; on fail, extra typed-damage rider (full damage on fail, 0 on
/// save)" cohort: Imp Sting, Quasit Claws, Purple Worm Tail Stinger. The
/// `also_install` field optionally tacks on an `ApplyCondition` install
/// on the SAME failed save — the Spider Bite / Ettercap Bite / Drow
/// Poisoned Crossbow shape (extra damage AND a Poisoned/etc. condition
/// share one save). When `also_install` is `None`, the chassis is pure
/// save-or-damage; when `Some(...)`, the condition only installs on a
/// failed save (matching the canonical "one save gates both riders"
/// semantics).
///
/// Mirrors `WeaponWithSaveCondition`'s shape (constructors `melee` /
/// `reach_melee`, ability-aware swing + Extra Attack chain). Distinct
/// from `WeaponWithRider` (unconditional flat damage rider, no save) and
/// from `WeaponWithSaveCondition` (rider is a condition install, not
/// extra damage).
///
/// Replaces the ~120 lines of hand-rolled `impl Action` blocks that
/// previously sat on each of Imp Sting / Quasit Claws / Purple Worm
/// Tail Stinger / Spider Bite / Ettercap Bite / Drow Poisoned Crossbow.
/// Each was the same 5-step skeleton (swing → bail-on-miss → save_or_
/// damage_rider → optional condition install → return effects); only
/// the dice / DC / typing differed.
pub struct WeaponWithSaveDamage {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
    pub is_melee: bool,
    pub save_ability: AbilityScoreType,
    pub save_dc: i32,
    pub rider_dice: Dice,
    pub rider_type: DamageType,
    pub rider_name: &'static str,
    /// Optional condition install on a failed save. `None` for pure
    /// save-or-damage (Imp Sting / Quasit Claws / Purple Worm Tail
    /// Stinger); `Some((cond, timer))` for the save-damage-plus-
    /// condition variant (Spider Bite / Ettercap Bite / Drow Poisoned
    /// Crossbow). Same single save gates both riders — matches the RAW
    /// shared-roll semantics.
    pub also_install: Option<(Condition, ConditionTimer)>,
}

impl WeaponWithSaveDamage {
    /// Const constructor for the standard "STR-or-DEX based 1H melee
    /// swing whose hit forces a save-or-extra-damage rider" shape. Pins
    /// `reach = MELEE_REACH`, `is_melee = true`, `also_install = None`.
    /// For long-reach or ranged or condition-piggybacked variants, use
    /// `reach_melee` / `ranged` / `melee_with_condition` instead.
    #[allow(clippy::too_many_arguments)]
    pub const fn melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach: MELEE_REACH,
            is_melee: true,
            save_ability,
            save_dc,
            rider_dice,
            rider_type,
            rider_name,
            also_install: None,
        }
    }

    /// Long-reach melee variant — takes an explicit `reach` in tiles.
    /// Pins `is_melee = true`, `also_install = None`. Mirrors
    /// `WeaponWithSaveCondition::reach_melee` so the long-reach lane is
    /// one declaration on every save-or-damage chassis instead of a
    /// struct-literal sprawl.
    #[allow(clippy::too_many_arguments)]
    pub const fn reach_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
        reach: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: true,
            save_ability,
            save_dc,
            rider_dice,
            rider_type,
            rider_name,
            also_install: None,
        }
    }

    /// Const constructor for the "STR-or-DEX based melee swing with a
    /// save-or-extra-damage-AND-condition install" shape (the spider's
    /// venom Poisons on the same failed save the rider damage rides
    /// on, the ettercap's bite Poisons on the same save, etc.). Pins
    /// `reach = MELEE_REACH`, `is_melee = true`, and threads the
    /// caller-supplied `(condition, timer)` straight into
    /// `also_install`. Replaces the hand-rolled 14-field struct
    /// literals at the three current save-damage-plus-condition call
    /// sites (Spider / Ettercap / Drow Poisoned Crossbow uses a ranged
    /// sibling) — same chokepoint benefit as the existing
    /// `melee` / `reach_melee` / `ranged` family. The docstring on
    /// `melee` referenced this constructor by name but it was missing
    /// from the impl; this entry restores the documented surface.
    #[allow(clippy::too_many_arguments)]
    pub const fn melee_with_condition(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
        condition: Condition,
        timer: ConditionTimer,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach: MELEE_REACH,
            is_melee: true,
            save_ability,
            save_dc,
            rider_dice,
            rider_type,
            rider_name,
            also_install: Some((condition, timer)),
        }
    }

    /// Ranged variant — takes an explicit `reach` in tiles and pins
    /// `is_melee = false` so the LOS gate fires. `also_install = None`.
    /// Used for save-or-damage ranged shots without a piggybacked
    /// condition install; for the save-damage-AND-condition ranged
    /// variant (Drow Poisoned Hand Crossbow) use `ranged_with_condition`
    /// instead.
    #[allow(clippy::too_many_arguments)]
    pub const fn ranged(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
        reach: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: false,
            save_ability,
            save_dc,
            rider_dice,
            rider_type,
            rider_name,
            also_install: None,
        }
    }

    /// Ranged save-damage-AND-condition variant — mirrors
    /// `melee_with_condition` on the `is_melee = false` lane.
    /// Threads the caller-supplied `(condition, timer)` into
    /// `also_install` so the failed-save site lands both the rider
    /// damage AND the condition install on the same target.
    /// Replaces the hand-rolled 14-field struct literal at the
    /// single current ranged-save-damage-plus-condition call site
    /// (Drow Poisoned Hand Crossbow) — same chokepoint benefit as
    /// `melee_with_condition` on the melee lane. A new ranged-
    /// venom shot lands as a single constructor call instead of
    /// repeating the full struct expression.
    #[allow(clippy::too_many_arguments)]
    pub const fn ranged_with_condition(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        save_ability: AbilityScoreType,
        save_dc: i32,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
        reach: isize,
        condition: Condition,
        timer: ConditionTimer,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: false,
            save_ability,
            save_dc,
            rider_dice,
            rider_type,
            rider_name,
            also_install: Some((condition, timer)),
        }
    }
}

impl Action for WeaponWithSaveDamage {
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
        // Ranged variants need LOS like every other ranged attack; melee
        // doesn't. Matches `SimpleWeapon`'s `is_melee`-gated LOS rule.
        !self.is_melee
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Surface both the base and the rider type so the AI's damage-
        // type lookahead (resistance / immunity gates) reads correctly.
        // If they collide (rider == base), de-dup for cleanliness.
        if self.rider_type == self.damage_type {
            vec![self.damage_type]
        } else {
            vec![self.damage_type, self.rider_type]
        }
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
        // Mirror `WeaponWithSaveCondition`'s swing-+-Extra-Attack chain so
        // the rider lands on every successful hit in the chain (RAW:
        // Extra Attack is a second swing, not a second action).
        let swing = |e: &mut EncounterInstance| {
            let (mut effects, _dealt) = weapon_swing_with_damage(
                e,
                caster_id,
                target_id,
                self.display_name,
                self.attack_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                None,
            );
            // Hit/miss gate via the effects vec — see
            // `weapon_swing_with_flat_rider` for the same chassis-wide
            // rationale. RAW: the save-or-damage rider fires on a hit,
            // not on damage > 0.
            if effects.is_empty() {
                return effects;
            }
            let save = save_or_damage_rider(
                e,
                target_id,
                self.save_ability,
                self.save_dc,
                self.rider_dice,
                self.rider_type,
                self.rider_name,
                &mut effects,
            );
            if let Some((condition, timer)) = self.also_install
                && !save.passed()
            {
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition,
                    timer,
                }));
            }
            effects
        };
        let mut effects = swing(encounter);
        maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        effects
    }
}

/// A `SimpleWeapon`-shaped attack that unconditionally installs a condition
/// on a confirmed hit — no save gate, no extra damage rider. The 5e auto-
/// grapple shape: Giant Frog Bite (hit → Grappled, escape DC 11), Mimic
/// Adhesive (hit → Adhered), Chuul Tentacle (hit → Grappled). RAW these
/// have an *escape* DC (a later Action), not a *prevention* save, so the
/// install fires the moment the swing lands.
///
/// Companion to `WeaponWithSaveCondition` (which gates the install on a
/// failed save) and `WeaponWithRider` (which lands flat typed damage on
/// hit, no condition). Use this when:
///   1. The condition installs whenever the swing connects (no
///      prevention save), AND
///   2. There's no extra typed-damage rider.
///
/// For save-gated condition installs use `WeaponWithSaveCondition`; for
/// flat-damage riders use `WeaponWithRider`; for "save-or-damage-plus-
/// condition" combinations use `WeaponWithSaveDamage` with `also_install`.
///
/// Per-target condition immunity is handled at the standard
/// `add_condition` chokepoint — a Grapple-immune target (e.g. an actor of
/// Huge+ size beyond the holder's grapple cap) shrugs the install off
/// silently, same as every other condition install path.
///
/// Extra Attack chains the same way as the other weapon chassis: an
/// Action-cost swing rolled outside a `Multiattack` triggers a second
/// swing with its own condition install for creatures with
/// `has_extra_attack`. The install only fires on hits (`damage > 0`).
pub struct WeaponWithCondition {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
    pub is_melee: bool,
    pub condition: Condition,
    pub timer: ConditionTimer,
    /// Log-friendly tag for the install line ("auto-grapple", "adhesive",
    /// ...). Mirrors the `rider_name` slot on the sibling chassis so log
    /// shapes stay uniform across the weapon-rider family.
    pub rider_name: &'static str,
}

impl WeaponWithCondition {
    /// Const constructor for the standard "STR-based 1H melee swing whose
    /// hit auto-installs a condition" shape (the giant-frog auto-grapple
    /// bite). Pins `reach = MELEE_REACH`, `is_melee = true`, and uses the
    /// same ability for attack + damage. For long-reach variants use
    /// `reach_melee`.
    #[allow(clippy::too_many_arguments)]
    pub const fn melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        condition: Condition,
        timer: ConditionTimer,
        rider_name: &'static str,
    ) -> Self {
        Self::reach_melee(
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            condition,
            timer,
            rider_name,
            MELEE_REACH,
        )
    }

    /// Long-reach melee variant. Same as `melee()` but takes an explicit
    /// `reach` in tiles, for auto-install-on-hit weapons like the
    /// chuul's reach-2 tentacles or a future reach-3 net-style attack.
    /// Mirrors `WeaponWithSaveCondition::reach_melee` so the long-reach
    /// lane is one declaration on every install-on-hit chassis instead
    /// of a struct-literal sprawl.
    #[allow(clippy::too_many_arguments)]
    pub const fn reach_melee(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        condition: Condition,
        timer: ConditionTimer,
        rider_name: &'static str,
        reach: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: true,
            condition,
            timer,
            rider_name,
        }
    }
}

impl Action for WeaponWithCondition {
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Mirror the swing-+-Extra-Attack chain on the rest of the
        // weapon-rider family so the auto-install lands on every hit in
        // the chain (RAW: Extra Attack is a second swing, not a second
        // action).
        let swing = |e: &mut EncounterInstance| {
            let (mut effects, _dealt) = weapon_swing_with_damage(
                e,
                caster_id,
                target_id,
                self.display_name,
                self.attack_ability,
                self.damage_dice,
                self.damage_type,
                self.is_melee,
                None,
            );
            // Hit/miss gate via the effects vec — empty on miss /
            // Sanctuary / Mirror Image, non-empty on hit (the DealDamage
            // payload is present even when post-mitigation damage is 0).
            // RAW: an auto-install rider fires on hit, not on damage > 0,
            // so a 1-damage swing zeroed by Uncanny Dodge still grapples.
            if effects.is_empty() {
                return effects;
            }
            e.log(format!("  {}: target is now {}", self.rider_name, self.condition));
            // Through the linked installer rather than a bare
            // `ApplyCondition`, so a rider whose condition carries a
            // back-link records who applied it. For everything on
            // `LINKED_CONDITIONS` that is the difference between a rule
            // the engine can enforce and one it can only approximate:
            // a chuul's pincer grapple now names the chuul, so escaping
            // it is a contest against that creature and stunning the
            // chuul lets go. Unlinked conditions come back as the same
            // one-element vec this used to push.
            effects.extend(crate::engine::side_effects::install_condition_with_link(
                self.condition,
                target_id,
                caster_id,
                self.timer,
            ));
            effects
        };
        let mut effects = swing(encounter);
        maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        effects
    }
}

/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// Reach is in tiles (not feet); 20 tiles = 50ft on this 2.5ft grid, which is
/// short of the 5e 80/320 normal/long range but plenty for our 40×20 maps.
pub static LONGBOW: SimpleWeapon = SimpleWeapon::ranged(
    "longbow",
    &["bow", "shoot"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Piercing,
    20,
    12,
);

/// Generic STR-based 2d6 bludgeoning slam used by zombies. Stays as the
/// canonical "monster fist" attack so multislams (and tests) reference it.
pub static SLAM: SimpleWeapon = SimpleWeapon::melee(
    "slam",
    &["slm"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Scimitar — generic STR-based 1d6 slashing melee attack. Used by
/// goblins and other light melee creatures that don't have a flashy
/// rider effect.
pub static SCIMITAR: SimpleWeapon = SimpleWeapon::melee(
    "scimitar",
    &["sc"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

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
    requires_condition: None,
    min_effective_range: None,
};

/// Dagger — finesse 1d4 piercing melee weapon. STR-or-DEX choice;
/// we use DEX which is the typical kobold / rogue stat. Cost 1 Action.
pub static DAGGER: SimpleWeapon = SimpleWeapon::melee(
    "dagger",
    &["dag"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Greatclub — Ogre's signature weapon. STR-based 1d10 bludgeoning with
/// **reach 2** (10ft) — first polearm-style attack in the codebase.
pub static GREATCLUB: SimpleWeapon = SimpleWeapon::reach_melee(
    "greatclub",
    &["gc"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Bludgeoning,
    2,
);

/// Warhammer — STR-based 1d8 bludgeoning martial weapon. The classic
/// dwarven sidearm; in our engine the versatile-2H clause collapses to
/// the simpler 1d8 base (the 2H 1d10 alternative would need a per-action
/// grip toggle the picker doesn't surface). Slots between scimitar (1d6)
/// and greataxe (1d12) for STR-build martials who want a bludgeoning
/// option (some creatures resist slashing / piercing).
pub static WARHAMMER: SimpleWeapon = SimpleWeapon::melee(
    "warhammer",
    &["wh", "hammer"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Bludgeoning,
);

/// Mace — STR-based 1d6 bludgeoning simple weapon. The canonical Thug /
/// Acolyte / Priest sidearm in 5e — same damage die as the scimitar but
/// a different damage type so creatures that resist slashing (Skeleton,
/// some constructs) still feel a Mace swing. Slots between Dagger (1d4)
/// and Warhammer (1d8) for STR-build mooks who don't carry a martial
/// weapon. Shared static so the Thug / future NPC priest etc. point at
/// one source of truth instead of duplicating the literal.
pub static MACE: SimpleWeapon = SimpleWeapon::melee(
    "mace",
    &["mc"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

/// Spear — STR-based 1d6 piercing simple weapon. Tribal Warrior /
/// generic-tribal NPC sidearm. RAW the spear is versatile (1d8 two-handed)
/// and thrown (20/60 ft); we collapse to the one-hand 1d6 melee base
/// since the engine doesn't surface per-action grip toggles and the
/// thrown lane is already covered by `JAVELIN`. Shared static so Tribal
/// Warrior and any future spear-wielding humanoid point at one source.
pub static SPEAR: SimpleWeapon = SimpleWeapon::melee(
    "spear",
    &["sp"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Shortsword — DEX-based 1d6 piercing finesse weapon. Standard Scout /
/// Spy / Assassin sidearm in 5e. Distinct from the Rogue's bespoke
/// `RogueShortsword` (which carries the Sneak Attack rider) and from
/// the per-creature `SPRITE_SHORTSWORD` / `WERERAT_SHORTSWORD` literals
/// (those use different ability scores / dice). Shared static so the
/// Scout multiattack and any future finesse-using mook point at one
/// source of truth.
pub static SHORTSWORD: SimpleWeapon = SimpleWeapon::melee(
    "shortsword",
    &["ssw", "short"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Club — STR-based 1d4 bludgeoning simple weapon. The peasant's only
/// sidearm — a stick. Lowest damage tier in the weapon pool (tied with
/// Dagger). The canonical Commoner / Acolyte / generic-peasant NPC
/// sidearm. Distinct from `GREATCLUB` (1d10 reach-2 ogre-tier club) and
/// `WARHAMMER` (1d8 martial); slots beneath both as the "bare-bones
/// 1H stick" baseline. Shared static so a Commoner / future Acolyte /
/// generic peasant point at one source of truth.
pub static CLUB: SimpleWeapon = SimpleWeapon::melee(
    "club",
    &["cl"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

/// Generic STR-based bite attack — 1d6+STR piercing, no rider. Use this
/// for creatures whose bite is pure damage (Troll, most beasts). Creatures
/// that also trip or grapple on a bite should use WolfBite or a dedicated
/// variant instead. Vanilla `SimpleWeapon` since the bite is pure damage —
/// the original bespoke `Bite` impl re-stated the same `simple_weapon_attack`
/// call SimpleWeapon already wraps.
pub static BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["bt"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

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
        // Prone from a trip persists until stand-up clears it
        // (ConditionTimer::Permanent).
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Strength,
            13,
            Condition::Prone,
            ConditionTimer::Permanent,
            "trip",
            &mut effects,
        );
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
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Dexterity);
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
                min_range: None,
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
/// Spider Bite — STR-based 1d10+STR piercing melee with a CON DC 11
/// save-or-2d4-poison-AND-Poisoned-2-rounds rider. The "extra damage AND
/// condition both ride on the same failed save" shape — `also_install`
/// is `Some((Poisoned, Rounds(2)))` so the chassis adds the condition
/// install only when the save fails. Routes through the shared
/// `WeaponWithSaveDamage` chassis alongside Ettercap Bite / Drow
/// Poisoned Crossbow.
pub static SPIDER_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::melee_with_condition(
    "spider bite",
    &["sbite"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(2, 4),
    DamageType::Poison,
    "spider venom",
    Condition::Poisoned,
    ConditionTimer::Rounds(2),
);


/// Greataxe — Orc-flavored heavy two-hander. STR-based 1d12 slashing,
/// melee reach. Hits harder than a longsword on a single die; pairs
/// with the orc's high STR for a punishing single-attack profile.
/// Vanilla `SimpleWeapon`: the Extra Attack rider is already handled
/// inside `SimpleWeapon::side_effects`, so the bespoke `Greataxe` impl
/// was duplicating the standard chassis.
pub static GREATAXE: SimpleWeapon = SimpleWeapon::melee(
    "greataxe",
    &["ga"],
    AbilityScoreType::Strength,
    Dice::new(1, 12),
    DamageType::Slashing,
);

/// Heavy Crossbow — DEX-based 1d10 piercing ranged. Differs from the
/// Longbow in damage die (1d10 vs 1d8) and conceptually loading time
/// (we don't model the loading property today). Used by bandits.
/// Vanilla `SimpleWeapon` — the bespoke impl was just `simple_weapon_attack`
/// wrapped in trait methods. Long-range penalty added: 5e crossbow is
/// 100/400ft; the engine's 2.5ft grid caps the indoor reach at 16 tiles
/// (40ft RAW would be 16 tiles) with normal range at 10 (≈25ft) — close
/// to the longbow's 12 (≈30ft) ratio.
pub static HEAVY_CROSSBOW: SimpleWeapon = SimpleWeapon::ranged(
    "heavy crossbow",
    &["hcb", "crossbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 10),
    DamageType::Piercing,
    16,
    10,
);

/// Wolf-specific bite: 1d4 STR-based piercing with a built-in trip rider.
/// On every hit forces a STR save (DC = 8 + prof + STR mod); fail = Prone.
/// For a plain bite without the trip use BITE instead.
/// Wolf bite — 1d4+STR piercing with a Trip rider (DC 11 STR save or
/// knocked Prone on a hit). Routes through the shared
/// `WeaponWithSaveCondition` chassis so the swing + save-and-condition
/// install share one chokepoint with Dire Wolf Bite and future
/// trip-style natural weapons.
pub static WOLF_BITE: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "wolf bite",
    &["wb"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Strength,
    11,
    Condition::Prone,
    ConditionTimer::Permanent,
    "wolf trip",
);

/// Frightful Howl — wolf bonus action. Every enemy within 4 tiles must
/// make a WIS save against DC 11 or be Frightened for 3 rounds.
/// Doesn't deal damage. Demonstrates the AoE-no-damage save pattern.
pub struct FrightfulHowl {}

impl Action for FrightfulHowl {
    fn name(&self) -> &str {
        "howl"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. Without this the AI's focus-fire lane
        // scores it as an attack and picks it over one — a medusa
        // re-gazed an already-petrified succubus four hundred and
        // seventy times running rather than finishing it, and the fight
        // could not end.
        false
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

    fn chains_multiple_attacks(&self) -> bool {
        true
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

    /// `count` copies of the sub-attack's own estimate.
    ///
    /// Delegating this is not optional the way most trait defaults are.
    /// A monster's action list usually carries both the Multiattack and
    /// the single swing it wraps, and the AI's picker ranks the two
    /// against each other; a wrapper that declined to estimate would
    /// score 0.0 against its own sub-attack's positive number and lose
    /// every tie, which would quietly stop every Multiattack creature in
    /// the bestiary from using its Multiattack.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let per = self.sub_attack.expected_damage(encounter, caster_id)?;
        Some(per * self.count as f32)
    }

    /// Inherited from the sub-attack, like the reach and the schema and
    /// the cost above it — a wrapper cannot be legal in a situation
    /// where the thing it wraps is not.
    ///
    /// This delegation was missing, and its absence was a trap rather
    /// than a live bug: every sub-attack wrapped today validates
    /// unconditionally, so nothing misbehaved. But two shapes in the
    /// engine do gate — a `SimpleWeapon` that has to be summoned first
    /// (`ASTRAL_ARMS_STRIKE`, gated on `Condition::AstralArms`) and a
    /// `BreathWeapon` gated on its recharge — and wrapping either in a
    /// `Multiattack` would have swung an unsummoned weapon or breathed
    /// a spent breath, twice, with nothing in the engine objecting. The
    /// Astral Self Monk gets a second attack at RAW level 17, so that
    /// wrapper is a plausible next commit rather than a hypothetical.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        self.sub_attack.custom_validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        )
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
        //
        // Enter / exit the multiattack-depth gate so sub-attacks know to
        // suppress their own Extra Attack rider (a creature with both a
        // Multiattack and `has_extra_attack: true` would otherwise
        // double-count its per-Action swing budget — see
        // `EncounterInstance::in_multiattack`).
        encounter.enter_multiattack();
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
        encounter.exit_multiattack();
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

    fn chains_multiple_attacks(&self) -> bool {
        true
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

    /// Sum of every part's estimate, each times its repeat count.
    ///
    /// Same reason `Multiattack` delegates: the wrapper and its parts
    /// sit on the same action list and are ranked against each other, so
    /// a wrapper that declined to estimate would lose to the single
    /// swing it contains. A part that declines contributes nothing
    /// rather than voiding the whole sum — an under-estimate for a
    /// compound whose pieces are half-annotated is still a better
    /// ranking than none.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let total: f32 = self
            .parts
            .iter()
            .filter_map(|(a, n)| {
                a.expected_damage(encounter, caster_id).map(|d| d * *n as f32)
            })
            .sum();
        Some(total)
    }

    /// Every part has to be legal, not just the first — see
    /// `Multiattack::custom_validate_input` for why the delegation
    /// matters at all.
    ///
    /// `all` rather than `any` because `side_effects` swings every part
    /// unconditionally: a compound that ran with one gate closed would
    /// resolve that part anyway. Stricter than it needs to be for the
    /// compounds that exist (every part of every one validates
    /// unconditionally, so this is a no-op today), and the strict
    /// direction is the safe one — a mixed compound that refuses is a
    /// visible loss of one action, where a mixed compound that resolves
    /// is a silent rules violation.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        self.parts.iter().all(|(a, _)| {
            a.custom_validate_input(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            )
        })
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
        // Same depth-gate as Multiattack — see in_multiattack docs.
        encounter.enter_multiattack();
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
        encounter.exit_multiattack();
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

/// Thug multiattack — 2 mace swings per Action (RAW: "The thug makes
/// two melee attacks"). Slots between the bandit (1 swing) and the
/// bandit captain (3 swings) for the canonical CR ½ humanoid melee
/// pace. Routes through the shared `Multiattack` chassis so the
/// homogeneous double-swing lane lives at the same chokepoint as the
/// goblin boss / hobgoblin warlord doubles.
pub static THUG_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double mace",
    sub_attack: &MACE,
    count: 2,
});

/// Tribal Warrior bonus-spear lane — RAW the warrior's Multiattack is
/// two spear swings per Action when it has nothing else equipped. The
/// homogeneous double-spear here mirrors `THUG_MULTI` / `GOBLIN_BOSS_MULTI`
/// shape, sub'd in for the spear. Slots in between the single-attack
/// CR ⅛ bandit and the CR ½ thug for the canonical Pack-Tactics tribal
/// melee pace.
pub static TRIBAL_WARRIOR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double spear",
    sub_attack: &SPEAR,
    count: 2,
});

/// Scout melee multiattack — 2 shortsword swings per Action. RAW the
/// scout has "Multiattack. The scout makes two melee attacks or two
/// ranged attacks"; this is the melee half. The ranged half is
/// `SCOUT_RANGED_MULTI` (two longbow shots). Both lanes route through
/// the shared `Multiattack` chassis so the homogeneous double-swing
/// pattern lives at one chokepoint.
pub static SCOUT_MELEE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double shortsword",
    sub_attack: &SHORTSWORD,
    count: 2,
});

/// Scout ranged multiattack — 2 longbow shots per Action. The ranged
/// half of the scout's RAW Multiattack ("two melee attacks OR two
/// ranged attacks"). Pairs with `SCOUT_MELEE_MULTI` so the AI / player
/// picks the lane that matches the engagement distance.
pub static SCOUT_RANGED_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double longbow",
    sub_attack: &LONGBOW,
    count: 2,
});

/// Imp Sting — DEX-based 1d4+DEX piercing melee with a CON DC 11
/// save-or-2d10-poison rider. Routes through the shared
/// `WeaponWithSaveDamage` chassis alongside Quasit Claws / Purple Worm
/// Tail Stinger — same "weapon hit + save-or-typed-damage" shape, only
/// the dice / DC / typing differ. The "all damage on fail, zero on
/// save" semantics match the canonical save_or_damage_rider chokepoint.
pub static IMP_STING: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "sting",
    &["st", "imp-sting"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(2, 10),
    DamageType::Poison,
    "imp venom",
);

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
                crate::actions::action_template::actor_lacks_condition(
                    encounter,
                    *id,
                    Condition::Frightened,
                )
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
                min_range: None,
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
                min_range: None,
                is_spell: false,
            },
        );
        if piercing_damage == 0 {
            return effects;
        }
        // On hit, 3d6 necrotic rider (no STR bonus, no crit doubling here —
        // RAW: only the weapon damage doubles; the bite's separate
        // necrotic die is added as flat extra damage). Routes through the
        // shared `add_flat_damage_rider` helper and pulls the returned
        // amount for the heal side of the bite.
        let necrotic = add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(3, 6),
            DamageType::Necrotic,
            "vampiric bite",
            &mut effects,
        );
        // Heal the vampire by the necrotic damage dealt (pre-resistance).
        encounter.log(format!("  vampire regains {} HP", necrotic));
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
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            10,
            Condition::Paralyzed,
            ConditionTimer::Rounds(2),
            "ghoul paralysis",
            &mut effects,
        );
        effects
    }
}

pub static GHOUL_CLAWS: LazyLock<GhoulClaws> = LazyLock::new(|| GhoulClaws {});

/// Ghast Bite — STR-based 2d8+STR piercing melee. The CR-2 ghast's
/// heavier-die secondary swing of the bite + claws compound. Pure
/// damage — the paralysis rider lives on the claws. RAW MM ghast bite:
/// 2d8+3 = ~12 piercing.
pub static GHAST_BITE: SimpleWeapon = SimpleWeapon::melee(
    "ghast bite",
    &["g-bite", "ghast-chomp"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
);

/// Ghast Claws — STR-based 2d6+STR slashing melee with a DC 10 CON
/// save-or-Paralyzed rider on hit. Identical chassis to Wolf Bite /
/// Dire Wolf Bite (save-or-condition on a confirmed hit) routed through
/// the shared `WeaponWithSaveCondition` chassis — replacing the
/// hand-rolled `GhoulClaws` impl shape with the data-only declaration.
/// Distinct from `GHOUL_CLAWS` only in the dice (2d6 vs 2d4) and the
/// paralysis duration (10 rounds = 1 minute RAW vs the ghoul's 2 rounds).
/// The Paralyzed envelope turns subsequent melee hits within 5 ft into
/// auto-crits — a lone ghast that lands a save-fail claw can lock a PC
/// out of multiple turns and feed crits to its allies.
pub static GHAST_CLAWS: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "ghast claws",
    &["g-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    AbilityScoreType::Constitution,
    10,
    Condition::Paralyzed,
    ConditionTimer::Rounds(10),
    "ghast paralysis",
);

/// Ghast multiattack — 1 bite + 1 claws per Action. RAW: "The ghast
/// makes two attacks: one with its bite and one with its claws." Same
/// chassis as the Owlbear (beak + claws) / Wereboar (tusks + slam) /
/// Wererat (bite + shortsword) heterogeneous multis — routes through
/// `CompoundAttack` so the two-limb Action lives at one chokepoint.
/// The claws-second ordering matches RAW so the bite damage lands
/// before any paralysis-induced auto-crit on a same-turn follow-up.
pub static GHAST_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "ghast multiattack",
    parts: vec![(&GHAST_BITE, 1), (&GHAST_CLAWS, 1)],
});

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
/// Dire Wolf bite — 2d6+STR piercing with a Trip rider (DC 13 STR save
/// or knocked Prone on a hit). Same shape as the wolf's bite with a
/// heavier damage die and a stiffer save DC — both ride the shared
/// `WeaponWithSaveCondition` chassis so the trip-rider chokepoint
/// stays uniform across the bestiary.
pub static DIRE_WOLF_BITE: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "dire wolf bite",
    &["dwb"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
    AbilityScoreType::Strength,
    13,
    Condition::Prone,
    ConditionTimer::Permanent,
    "dire wolf trip",
);

/// Re-export the spell-table FIRE_BOLT here so monster files that import
/// `crate::actions::monster_attacks::FIRE_BOLT` keep working — the
/// canonical definition lives with the other spells, this just gives
/// fire-themed monsters a handle into the same Action.
pub use crate::actions::spells::FIRE_BOLT;

/// Owlbear Beak — STR-based 1d10+STR piercing melee. Vanilla
/// `SimpleWeapon` — pairs with the claws in the per-Action compound.
pub static OWLBEAR_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "owlbear beak",
    &["ob-beak"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
);

/// Owlbear Claws — STR-based 2d8+STR slashing melee. The bigger-die
/// secondary swing of the owlbear's beak + claws compound.
pub static OWLBEAR_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "owlbear claws",
    &["ob-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

/// Owlbear's signature multiattack rolled into one Action: a beak (1d10+5
/// piercing) and a claws (2d8+5 slashing) swing at the same target.
/// Heterogeneous `CompoundAttack` — same shape as the wereXX bite +
/// claws compound, the salamander tail + bite, etc. Cost is a single
/// Action — the multiattack trade is "spend one Action, get two attack
/// rolls" without a slot.
pub static OWLBEAR_MULTIATTACK: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "owlbear multiattack",
    parts: vec![(&OWLBEAR_BEAK, 1), (&OWLBEAR_CLAWS, 1)],
});

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
pub static WEREWOLF_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "werewolf claws",
    &["ww-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Generic lycanthrope bite — STR-based piercing melee carrying the
/// "save-or-lycanthropy-curse" rider shared by every wereXX in the
/// monster pool (werewolf, werebear, wereboar, wererat, weretiger). On
/// a confirmed hit the target makes a CON save vs `save_dc`; on fail
/// they pick up Poisoned for 3 rounds, modeling the early-stage curse
/// fever without having to track multi-day transformations.
///
/// Parameterized by display name + alias + damage dice + save DC so
/// each wereXX template plugs in its MM-tuned numbers (werewolf: 1d8 /
/// DC 12, werebear: 1d10 / DC 14, wereboar: 2d6 / DC 12, wererat: 1d4 /
/// DC 11, weretiger: 1d10 / DC 13). Replaces the bespoke `WerewolfBite`
/// and `WerebearBite` `impl Action`s — the only thing that varied
/// across them was the four scalar fields exposed here.
pub struct LycanthropeBite {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub damage_dice: Dice,
    pub save_dc: i32,
}

impl Action for LycanthropeBite {
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
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            self.damage_dice,
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Lycanthropy curse rider: CON save vs `save_dc` or Poisoned 3
        // rounds. DC tuned per wereXX CR — see template comments.
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            self.save_dc,
            Condition::Poisoned,
            ConditionTimer::Rounds(3),
            "lycanthropy",
            &mut effects,
        );
        effects
    }
}

pub static WEREWOLF_BITE: LycanthropeBite = LycanthropeBite {
    display_name: "werewolf bite",
    aliases: &["ww-bite"],
    damage_dice: Dice::new(1, 8),
    save_dc: 12,
};

/// Werewolf multiattack — 1 bite + 1 claws per Action via `CompoundAttack`.
/// Heterogeneous compound (piercing + slashing) — bite carries the
/// lycanthropy rider, claws are the steady damage lane. Same shape as
/// the Werebear multi.
pub static WEREWOLF_MULTIATTACK: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "werewolf multiattack",
    parts: vec![(&WEREWOLF_BITE, 1), (&WEREWOLF_CLAWS, 1)],
});

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
pub static HARPY_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "harpy talons",
    &["talons"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Luring Song — harpy's AoE charm. Every creature within 12 tiles (30ft)
/// that can hear the harpy makes a WIS save vs DC 11. On fail, target is
/// Charmed by the harpy for 3 rounds. Charm-immune creatures (undead /
/// constructs / etc.) shrug it off automatically — we let the
/// add_condition guard handle that uniformly. We use SetConditionLink(Charmed) so the
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
        use crate::engine::side_effects::install_condition_with_link;
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
            if target.effectively_immune_to_condition(Condition::Charmed) {
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
            effects.extend(install_condition_with_link(
                Condition::Charmed,
                target_id,
                caster_id,
                ConditionTimer::Rounds(3),
            ));
        }
        effects
    }
}

pub static LURING_SONG: LazyLock<LuringSong> = LazyLock::new(|| LuringSong {});

/// Longsword — versatile 1d8 slashing melee weapon. STR-based, Action
/// cost, MELEE_REACH. Workhorse weapon for Knights and other armored
/// foot soldiers.
pub static LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "longsword",
    &["ls", "sword"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Greatsword — STR-based 2d6 slashing melee weapon. The paladin's
/// signature heavy weapon: bigger dice than the longsword (1d8) at the
/// cost of two-handed use, which we don't model explicitly. Pairs with
/// Divine Smite for the load-bearing burst damage.
pub static GREATSWORD: SimpleWeapon = SimpleWeapon::melee(
    "greatsword",
    &["gs", "great-sword"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Pact Blade — the Hexblade Warlock's **Hex Warrior** weapon: 1d8
/// slashing, but keyed to **Charisma** rather than Strength.
///
/// RAW's Hex Warrior reads "you can use your Charisma modifier instead
/// of Strength or Dexterity for the attack and damage rolls" of one
/// weapon you've bonded with. Because `SimpleWeapon` already carries its
/// own `attack_ability` (and derives `damage_ability` from it), the
/// feature needs no engine lane at all — it *is* a weapon whose ability
/// is CHA, which is also how it plays at the table: the hexblade swings
/// one specific blade with their casting stat and every other weapon
/// normally.
///
/// Mechanically a longsword with the ability swapped. That swap is the
/// entire subclass identity on the martial half: a CHA-18 hexblade
/// swings at +7 with a blade where the baseline warlock's STR-8 dagger
/// swings at +1, which is the difference between a caster who owns a
/// dagger and one who can stand in the front rank — and standing in the
/// front rank is what makes Armor of Hexes worth having.
pub static PACT_BLADE: SimpleWeapon = SimpleWeapon::melee(
    "pact blade",
    &["pact", "blade", "pb"],
    AbilityScoreType::Charisma,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Lance — 1d12 piercing, reach 2 (RAW's 10 ft), and disadvantage
/// against anything within 5 feet.
///
/// The whole of RAW's entry except the two-handed clause, which needs a
/// hand-occupancy model the engine doesn't have. That last clause is
/// also the least of the three: "a lance requires two hands to wield
/// when you aren't mounted" is a shield tax, and the disadvantage is
/// what actually decides whether you want one.
///
/// This used to ship as a plain reach-2 spear with a note saying the
/// mounted-only gate had been dropped because the engine had no mounts.
/// It has them now, and the lance is the weapon that most wants them: a
/// knight on foot jabs at disadvantage the moment anything closes to
/// contact, and a knight on a warhorse rides at ten feet and never lets
/// it.
pub static LANCE: SimpleWeapon = SimpleWeapon {
    min_effective_range: Some(2),
    ..SimpleWeapon::reach_melee(
        "lance",
        &["lnc"],
        AbilityScoreType::Strength,
        Dice::new(1, 12),
        DamageType::Piercing,
        2,
    )
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
pub static GARGOYLE_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["clw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

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
                min_range: None,
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
pub static STIRGE_PROBOSCIS: SimpleWeapon = SimpleWeapon::melee(
    "blood drain",
    &["proboscis", "drain"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
);

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
        // STR-based melee swing — `weapon_swing_with_damage` collapses the
        // caster-mod / AttackParams boilerplate and returns `damage` so
        // the max-HP-drain rider mirrors the pre-mitigation necrotic
        // packet (resistance to necrotic doesn't double-protect the drain).
        let (mut effects, damage) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            "life drain",
            AbilityScoreType::Strength,
            Dice::new(1, 6),
            DamageType::Necrotic,
            true,
            None,
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
    /// Annotated so the picker can weigh the gore against the greataxe
    /// the same minotaur carries. Without an estimate on both, the
    /// comparison falls through to declaration order — and once the
    /// minotaur has ten feet of run behind it, the charge clause on this
    /// swing is worth two more d8 than the axe.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        crate::actions::action_template::weapon_expected_damage_named(
            encounter,
            caster_id,
            self.name(),
            Dice::new(2, 8),
            Some(AbilityScoreType::Strength),
            Resource::Action,
            0,
        )
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
pub static CORRUPTING_TOUCH: SimpleWeapon = SimpleWeapon::melee(
    "corrupting touch",
    &["ct", "touch"],
    AbilityScoreType::Charisma,
    Dice::new(3, 6),
    DamageType::Necrotic,
);

/// Hippogriff Beak — melee, STR-based, 1d10+3 piercing. The bigger
/// half of the hippogriff multiattack — single-strike-feels-meaty stat
/// line tuned to deliver one solid hit per swing.
pub static HIPPOGRIFF_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "beak",
    &["bk", "peck"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
);

/// Hippogriff Talons — melee, STR-based, 2d6+3 slashing. Companion
/// half of the multiattack — moderately bigger dice spread for the
/// second swing per turn.
pub static HIPPOGRIFF_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "talons",
    &["tl", "claws-h"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

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
pub static DOPPELGANGER_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "slam",
    &["dslam"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

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
pub static MUMMY_ROTTING_FIST: WeaponWithRider = WeaponWithRider::melee(
    "rotting fist",
    &["rf", "rot"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    Dice::new(3, 6),
    DamageType::Necrotic,
    "rotting fist",
);

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
        use crate::actions::action_template::resolve_los_glare_condition;
        const RADIUS: isize = 8;
        const DC: i32 = 11;
        encounter.log("  dreadful glare: the mummy fixes its hollow eyes on the living");
        resolve_los_glare_condition(
            encounter,
            caster_id,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Frightened,
            ConditionTimer::Rounds(10),
            Some(DamageType::Necrotic),
        )
    }
}

pub static MUMMY_DREADFUL_GLARE: LazyLock<MummyDreadfulGlare> =
    LazyLock::new(|| MummyDreadfulGlare {});

/// Berserker Greataxe — STR-based melee with 1d12+STR slashing. Distinct
/// from the bare GREATAXE in that it's wrapped in an Action impl so the
/// berserker can pair it with its self-buffing "Reckless" stance in
/// future work. Today this is a vanilla greataxe; left as a wrapper
/// for symmetry with the rest of the per-creature attack files.
pub static BERSERKER_GREATAXE: SimpleWeapon = SimpleWeapon::melee(
    "berserker greataxe",
    &["bgx", "berserker-axe"],
    AbilityScoreType::Strength,
    Dice::new(1, 12),
    DamageType::Slashing,
);

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
pub static VETERAN_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "longsword",
    &["ls"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

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
pub static YETI_CLAWS: WeaponWithRider = WeaponWithRider::melee(
    "yeti claws",
    &["yc", "yeti"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
    Dice::new(1, 6),
    DamageType::Cold,
    "yeti claws",
);

/// Yeti multiattack — two claw swings per Action. With the cold rider
/// on each hit, this is comparable to a small-ice-elemental loop —
/// punchy on bare-skin targets but blunted by cold resistance.
pub static YETI_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double claws",
    sub_attack: &YETI_CLAWS,
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

/// Manticore Bite — STR-based 1d8+STR piercing melee. Vanilla
/// `SimpleWeapon` — pairs with the claws in the per-Action compound.
pub static MANTICORE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "manticore bite",
    &["m-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Manticore Claw — STR-based 1d6+STR slashing melee. The two claws
/// share this single template — `CompoundAttack` runs the swing twice
/// per Action via the `count: 2` slot.
pub static MANTICORE_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "manticore claw",
    &["m-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Manticore Multiattack — Action: bite (1d8 piercing) + two claws
/// (1d6 slashing each). All strikes share the same target. This is the
/// melee half of the manticore's kit — the ranged Tail Spikes covers the
/// stand-off lane. Heterogeneous `CompoundAttack` (piercing + slashing,
/// uneven counts) — same chassis as the pit fiend's 1 bite + 2 claws
/// and the otyugh's 1 bite + 2 tentacles.
pub static MANTICORE_MULTIATTACK: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "manticore multiattack",
    parts: vec![(&MANTICORE_BITE, 1), (&MANTICORE_CLAW, 2)],
});

/// Hill Giant Greatclub — STR-based 3d8 bludgeoning, reach 2 tiles
/// (10 ft). Mirrors the ogre's club but bumped to giant-tier dice; the
/// extra reach is the hill giant's signature spacing advantage.
pub static HILL_GIANT_GREATCLUB: SimpleWeapon = SimpleWeapon::reach_melee(
    "giant greatclub",
    &["ggc", "giant-club"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
    2,
);

/// Hill Giant Boulder — STR-based 3d10 bludgeoning thrown rock with
/// reach 24 (60 ft). Ranged STR throw is unusual but matches the 5e
/// stat block: giants chuck rocks for big damage at long range.
pub static HILL_GIANT_BOULDER: SimpleWeapon = SimpleWeapon::ranged(
    "boulder",
    &["bld", "rock"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Bludgeoning,
    24,
    16,
);

/// Treant Slam — STR-based 3d6 bludgeoning, reach 2 (10 ft). The treant
/// is a slow CR-9 wall of HP that swings massive trunks; 3d6+STR per
/// strike, no rider, but the Treant template attaches the Multiattack
/// wrapper to swing twice per Action.
pub static TREANT_SLAM: SimpleWeapon = SimpleWeapon::reach_melee(
    "treant slam",
    &["tslam"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    2,
);

/// Treant Multiattack — Action: two Treant Slam swings against the same
/// target. The pair of 3d6+STR slams averages ~25 damage at the treant's
/// stat block — eats through PCs in a couple of rounds and gives the
/// CR-9 frame a believable threat profile. Vanilla `Multiattack` —
/// homogeneous twin-swing of the same sub-attack, matches the zombie
/// multislam / bandit captain triple scimitar shape.
pub static TREANT_MULTIATTACK: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "treant multiattack",
    sub_attack: &TREANT_SLAM,
    count: 2,
});

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

/// Generic burst breath weapon — the data-only Action behind every
/// "Recharge 5-6: cone/area of damage type X, save Y for half" creature
/// ability in the engine. Each instance carries its own name, alias
/// table, damage roll, damage type, save ability, DC, burst radius, max
/// range, and recharge-pool key, so adding a new breath (Behir's
/// lightning line, a Wyvern's poison cone, a future hydra breath, etc.)
/// is a single static declaration rather than a fresh struct + 60-line
/// Action impl pair.
///
/// Replaces the four near-identical `DragonBreath{Fire,Cold,Lightning,
/// Poison}` structs that previously sat here — they all routed through
/// the same `resolve_burst_save_damage` chokepoint and only differed in
/// damage type / save ability / log label, so the per-element struct
/// was pure boilerplate. Recharge gating + DEX-vs-CON save shape lives
/// in this one place now; the abbreviation aliases ("fb", "cb", "lb",
/// "pb") survive verbatim.
///
/// See `BreathWeaponCondition` for the save-or-condition sibling — same
/// chassis shape (radius, range, recharge_key, save_ability, dc) but
/// the resolution path installs a condition on fail instead of dealing
/// half-on-save damage. Use that variant for damage-free control cones
/// like the Dust Mephit's Blinding Breath.
pub struct BreathWeapon {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    pub radius: isize,
    /// Max distance (in tile gaps) from the caster's footprint to the
    /// burst center. RAW dragon breath: 60 ft cone collapses to a
    /// burst-4 / range-6 envelope in this 2.5 ft grid.
    pub range: isize,
    /// Key passed to `is_recharge_available` / `spend_recharge`. Every
    /// vanilla breath weapon shares the `"breath_weapon"` pool so a
    /// chromatic dragon can't double-tap with two different elements.
    pub recharge_key: &'static str,
}

impl Action for BreathWeapon {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst {
            radius: self.radius,
        }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.range)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, self.recharge_key)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the recharge resource before resolving damage so a
        // mid-resolution failure can't leave the breath both spent AND
        // damage-applied.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge(self.recharge_key);
        }
        let raw = encounter.roll(&self.damage_dice);
        encounter.log(format!(
            "  {}: {}({}) = {} {} area (DC {} {}, half on save)",
            self.display_name,
            self.damage_dice,
            raw,
            raw,
            self.damage_type,
            self.dc,
            self.save_ability,
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            self.radius,
            self.save_ability,
            self.dc,
            raw,
            self.damage_type,
        )
    }
}

/// Passive "death burst" trigger — a creature that explodes (or shatters,
/// or releases a final toxic gust) when reduced to 0 HP. Modeled as a
/// data-only struct hung off `CreatureTemplate::death_burst`; the engine's
/// `EncounterInstance::cleanup_dead_actors` fires the burst before removing
/// the actor from the map. The burst is shaped like a `BreathWeapon` but
/// without the action-economy / recharge wiring (it's not a turn-spent
/// ability — it fires automatically on death).
///
/// Caster id passed into `resolve_burst_save_damage` is the dying creature
/// itself; the helper's caster-exclusion gate keeps the corpse from
/// damaging itself (a moot point — the actor is being removed anyway —
/// but it also keeps the log clean of self-targeting noise). Allies and
/// enemies in radius both roll the save; mephits famously can wipe their
/// own kin if the radii overlap.
///
/// New death-burst creatures (mephit cohort, magmin, ash zombie variants,
/// future shaggy-mold style monsters) land as a one-line struct literal
/// on the template instead of a custom on-death hook per species.
#[derive(Clone, Copy)]
pub struct DeathBurst {
    /// Display label for the burst log line ("explodes!", "shatters",
    /// "erupts in icy shards", etc.). Plain English so the same struct
    /// can describe a magmin's fire pop and an ice mephit's shard burst
    /// without a per-creature log path.
    pub display_name: &'static str,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    /// Footprint-Chebyshev gap from the dying actor's tile that the burst
    /// reaches. Mephit death bursts are 5 ft (gap 1) RAW; magmin death
    /// burst is 10 ft (gap 2). Same units as `BreathWeapon::radius`.
    pub radius: isize,
}

/// Recharge-gated burst that imposes a condition rather than dealing
/// damage — the save-or-condition sibling of `BreathWeapon`. Modeled as
/// a data-only struct so a new save-or-blinded / save-or-restrained
/// monster cone lands as a single literal on the template instead of a
/// bespoke `impl Action`.
///
/// Targeting / reach / cost / recharge wiring mirror `BreathWeapon`
/// exactly — what changes is the resolution path: damage is replaced by
/// `resolve_burst_save_condition`, which installs `condition` for `timer`
/// on every enemy in `radius` that fails the save. The `display_name`
/// drives the log line; per-target immunity to the condition is handled
/// by the standard `add_condition` chokepoint (no caller-side gate
/// needed — same as `save_or_condition_rider`).
///
/// Canonical entry: the Dust Mephit's Blinding Breath (5 ft cone of
/// fine grit, DC 10 CON, Blinded on fail). Future Mud Mephit (save-or-
/// Restrained) and Smoke Mephit (save-or-disadvantage) bursts plug into
/// the same chassis with their own `(condition, timer)` pair.
pub struct BreathWeaponCondition {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    pub radius: isize,
    pub range: isize,
    pub recharge_key: &'static str,
    /// Condition installed on every burst target that fails the save.
    pub condition: Condition,
    /// How long the installed condition sticks. Mephit cone-breaths use
    /// `Rounds(1)` (the "until end of [creature]'s next turn" RAW clause
    /// collapses to one round at the encounter's per-round granularity).
    pub timer: ConditionTimer,
}

impl Action for BreathWeaponCondition {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst {
            radius: self.radius,
        }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.range)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, self.recharge_key)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the recharge resource before resolving the save so a
        // mid-resolution failure can't leave the breath both spent AND
        // condition-installed (mirrors the damage variant's order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge(self.recharge_key);
        }
        encounter.log(format!(
            "  {}: burst centered at ({}, {}) (DC {} {}, {} on fail)",
            self.display_name, point.x, point.y, self.dc, self.save_ability, self.condition,
        ));
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            point,
            self.radius,
            self.save_ability,
            self.dc,
            self.condition,
            self.timer,
        )
    }
}

/// Dragon Fire Breath — Adult Red Dragon signature. Burst-4 radius
/// (range 6) of searing flame. 12d6 fire, DC 21 DEX, half on save.
/// Recharge 5-6 via the `"breath_weapon"` pool. The dragon templates
/// reach the action via this static; a fresh Behir breath / Wyvern
/// cone / future poison breath becomes a one-line declaration on the
/// same chassis.
pub static DRAGON_BREATH_FIRE: BreathWeapon = BreathWeapon {
    display_name: "fire breath",
    aliases: &["fb", "breath"],
    damage_dice: Dice::new(12, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 21,
    radius: 4,
    range: 6,
    recharge_key: "breath_weapon",
};

/// Alias for the same fire breath under the older name some sites still
/// reach for. Kept here so legacy references compile; both point at the
/// same `BreathWeapon` value.
pub static DRAGON_FIRE_BREATH: &BreathWeapon = &DRAGON_BREATH_FIRE;

/// Dragon Cold Breath — burst-4 / range-6 of freezing cold. 12d6 cold,
/// DC 21 DEX, half on save. Recharge 5-6.
pub static DRAGON_BREATH_COLD: BreathWeapon = BreathWeapon {
    display_name: "cold breath",
    aliases: &["cb", "breath"],
    damage_dice: Dice::new(12, 6),
    damage_type: DamageType::Cold,
    save_ability: AbilityScoreType::Dexterity,
    dc: 21,
    radius: 4,
    range: 6,
    recharge_key: "breath_weapon",
};

/// Dragon Lightning Breath — burst-4 / range-6 of crackling lightning.
/// 12d6 lightning, DC 21 DEX, half on save. Recharge 5-6.
pub static DRAGON_BREATH_LIGHTNING: BreathWeapon = BreathWeapon {
    display_name: "lightning breath",
    aliases: &["lb", "breath"],
    damage_dice: Dice::new(12, 6),
    damage_type: DamageType::Lightning,
    save_ability: AbilityScoreType::Dexterity,
    dc: 21,
    radius: 4,
    range: 6,
    recharge_key: "breath_weapon",
};

/// Dragon Poison Breath — burst-4 / range-6 of noxious gas. 12d6
/// poison, DC 21 CON, half on save. Recharge 5-6. Note: poison breath
/// uses a CON save (inhaled toxin) rather than the DEX save the
/// elemental breaths route through — the shared chassis carries that
/// per-instance difference cleanly.
pub static DRAGON_BREATH_POISON: BreathWeapon = BreathWeapon {
    display_name: "poison breath",
    aliases: &["pb", "breath"],
    damage_dice: Dice::new(12, 6),
    damage_type: DamageType::Poison,
    save_ability: AbilityScoreType::Constitution,
    dc: 21,
    radius: 4,
    range: 6,
    recharge_key: "breath_weapon",
};

/// Behir lightning breath — burst-3 / range-5, 12d10 lightning, DC 16
/// DEX, half on save. Recharge 5-6. Behir's signature: a 20 ft line
/// of lightning that approximates as a small burst here. CR-11
/// damage with the same recharge-pool key the dragons use so a
/// dragon-led ambush can't double-tap with two breaths from different
/// actors via the same key.
pub static BEHIR_LIGHTNING_BREATH: BreathWeapon = BreathWeapon {
    display_name: "lightning breath",
    aliases: &["lb", "breath", "blast"],
    damage_dice: Dice::new(12, 10),
    damage_type: DamageType::Lightning,
    save_ability: AbilityScoreType::Dexterity,
    dc: 16,
    radius: 3,
    range: 5,
    recharge_key: "breath_weapon",
};

/// Dragon Bite — Adult Red Dragon's signature melee. d20 + 14 vs AC
/// (STR+prof at CR 17), on hit 2d10+8 piercing + 4d6 fire at reach 10ft.
/// The fire rider is a separate `DealDamage` so per-target resistance /
/// immunity applies to it independently from the piercing.
pub static DRAGON_BITE: WeaponWithRider = WeaponWithRider::reach_melee(
    "dragon bite",
    &["bite-d", "dbite"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Piercing,
    2,
    Dice::new(4, 6),
    DamageType::Fire,
    "dragon bite",
);

/// Dragon Claw — Adult Red Dragon's swipe. Identical resolution to a
/// `SimpleWeapon` (no rider), tuned to 2d6+8 slashing at the dragon's
/// hit modifier. Two claws + bite = the dragon multiattack; we issue
/// the data-only SimpleWeapon variant so the AI picks Bite for the
/// fire rider and Claw as fallback.
pub static DRAGON_CLAW: SimpleWeapon = SimpleWeapon::reach_melee(
    "dragon claw",
    &["dclaw", "claw-d"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    2,
);

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
                min_range: None,
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
/// Drow Poisoned Hand Crossbow — DEX-based 1d6+DEX piercing ranged
/// shot at reach 12 tiles (30 ft) with a CON DC 13 save-or-2d4-poison-
/// AND-Poisoned-2-rounds rider. Routes through the shared
/// `WeaponWithSaveDamage::ranged_with_condition` chassis, mirroring
/// the Spider Bite / Ettercap Bite / Giant Wasp Sting shape on the
/// melee side. The 2-round Poisoned timer is a tighter proxy for
/// RAW's 1-hour "magically poisoned by drow knock-out venom"
/// duration; the engine compresses to keep the rider relevant without
/// permanently disabling the target across an encounter.
pub static DROW_POISONED_CROSSBOW: WeaponWithSaveDamage = WeaponWithSaveDamage::ranged_with_condition(
    "poisoned hand crossbow",
    &["phcb", "drowbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    13,
    Dice::new(2, 4),
    DamageType::Poison,
    "drow poison",
    12,
    Condition::Poisoned,
    ConditionTimer::Rounds(2),
);

/// Frost Giant Greataxe — STR-based 3d12 slashing melee, reach 2 (10 ft).
/// One of the heaviest single-swing weapons in the bestiary: dice on par
/// with the Hill Giant's club but cycled into slashing damage to keep
/// damage-type variety on the giant tier. CR-8 numbers.
pub static FROST_GIANT_GREATAXE: SimpleWeapon = SimpleWeapon::reach_melee(
    "frost giant greataxe",
    &["fgx", "frost-axe"],
    AbilityScoreType::Strength,
    Dice::new(3, 12),
    DamageType::Slashing,
    2,
);

/// Frost Giant Rock — STR-based 4d10 bludgeoning thrown rock at reach
/// 24 (60 ft). Frost Giants chuck boulders like Hill Giants but harder
/// — the extra die is the CR-8 vs CR-5 step. Same template as the Hill
/// Giant Boulder.
pub static FROST_GIANT_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "frost rock",
    &["frock"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    24,
    16,
);

/// Vampire Charming Gaze — Action. Target within 30ft makes a WIS save
/// vs DC 17 (vampire's CHA-based spell DC). Fail = Charmed for 1 minute
/// (10 rounds in our model), and the SetConditionLink(Charmed) linkage points the
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        save_or_charmed_by_caster(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            17,
            ConditionTimer::Rounds(10),
            "charming gaze",
        )
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
                min_range: None,
                is_spell: false,
            },
        );
        // 5e RAW: poison rider applies on hit only — bail if the bite missed.
        if effects.is_empty() {
            return effects;
        }
        // Poison rider: 3d6 poison + CON save or Poisoned (10 rounds).
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(3, 6),
            DamageType::Poison,
            "couatl bite poison",
            &mut effects,
        );
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
pub static PIT_FIEND_BITE: SimpleWeapon = SimpleWeapon::melee(
    "pit fiend bite",
    &["pf-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 6),
    DamageType::Piercing,
);

/// Pit Fiend's Devil Claw — STR-based 2d8+8 slashing. The companion
/// melee attack to the bite; together they make up the pit fiend's
/// 4-attack multiattack (1 bite + 1 claw + 1 mace + 1 tail in MM RAW).
/// We collapse to bite+claw bursting via the Multiattack wrapper below.
// 10ft reach — the pit fiend's natural reach for non-bite limbs.
pub static PIT_FIEND_CLAW: SimpleWeapon = SimpleWeapon::reach_melee(
    "devil claw",
    &["pf-claw"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
    2,
);

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
pub static MONK_UNARMED_STRIKE: SimpleWeapon = SimpleWeapon::melee(
    "martial arts",
    &["ma-strike", "unarmed", "punch"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Bludgeoning,
);

/// Radiant Sun Bolt — Way of the Sun Soul Monk (subclass level 3). A
/// ranged attack made with the monk's own body: DEX to hit, the martial
/// arts die for damage, radiant, out to 30 ft.
///
/// A `SimpleWeapon` and not a bespoke impl because that is all it is —
/// RAW's whole text is "you can make the attack as if you were making an
/// unarmed strike, except at range." Everything the subclass is worth
/// falls out of it costing an Action like any other attack: Extra Attack
/// fires it twice, and Flurry of Blows hands over an extra Action, which
/// is RAW's "spend 1 ki as a bonus action to make two more bolts" by a
/// different route and on the chassis's existing button.
///
/// `normal_range == reach`, so there is no disadvantage band. RAW gives
/// the bolt a flat 30 ft with no long range at all, which is exactly
/// what a range band that starts where the weapon stops means.
pub static RADIANT_SUN_BOLT: SimpleWeapon = SimpleWeapon::ranged(
    "radiant sun bolt",
    &["sunbolt", "rsb"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Radiant,
    // 30 ft on the 2.5 ft grid, as both the max and the normal range.
    12,
    12,
);

/// Arms of the Astral Self — Way of the Astral Self Monk (subclass
/// level 3, TCE). Spectral arms of ki settle over the monk's own, and
/// for as long as they hold the monk's unarmed strike changes in four
/// ways at once: Wisdom to hit and to damage instead of Dexterity,
/// force instead of bludgeoning, and 10 ft of reach instead of 5.
///
/// All four of those are things a `SimpleWeapon` already says, which
/// is why this is one — the subclass needed no new attack machinery,
/// only a way for a weapon to be absent until it is summoned. That is
/// `gated_on`, and it is the whole engine cost of the feature.
///
/// The reach is the half that changes how the monk is played. Every
/// other monk on the roster has to be standing in contact to do
/// anything at all, on a d8 hit die with no armour; this one hits from
/// a tile back, which is the difference between taking an opportunity
/// attack on the way out and not being adjacent to take one. The
/// Wisdom swap is what makes the reach affordable — the Astral Self
/// chassis puts its 16 in WIS and its 14 in DEX, so the arms are
/// strictly the better swing while they are up and the ordinary
/// martial-arts fist is the fallback for the round they are not.
///
/// Force is the rarest-resisted damage type in the bestiary, which
/// means the arms also quietly solve the skeleton / zombie /
/// elemental matchups that a bludgeoning fist is bad at. RAW's
/// remaining lv3 clause — Wisdom in place of Strength on Strength
/// checks and saves — has no surface here: the engine rolls no ability
/// checks, and a save-ability substitution would be a lane of its own
/// for one subclass.
pub static ASTRAL_ARMS_STRIKE: SimpleWeapon = SimpleWeapon::reach_melee(
    "astral arms",
    &["arms", "aas"],
    AbilityScoreType::Wisdom,
    Dice::new(1, 8),
    DamageType::Force,
    // 10 ft on the 2.5 ft grid — one tile past `MELEE_REACH`, the same
    // envelope the Ogre's greatclub swings in.
    2,
)
.gated_on(Condition::AstralArms);

/// Tarrasque Bite — STR-based 4d12+10 piercing, 10ft reach. The
/// signature one-shot of the apex 5e creature. Hit modifier scales off
/// the tarrasque's massive STR (30 → +10 + prof 9 = +19 RAW; we let
/// the engine compute the modifier from STR + prof so the boss's stat
/// block stays authoritative).
// 15ft reach — gargantuan natural reach for the bite.
pub static TARRASQUE_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "tarrasque bite",
    &["t-bite", "tbite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
    4,
);

/// Tarrasque Claw — STR-based 3d8 slashing. Companion melee that fills
/// out the multiattack with two swings per Action. Reach matches the
/// tarrasque's body footprint (10ft for the claws — slightly shorter
/// than the bite's 15ft).
// 10ft reach for the claw lanes.
pub static TARRASQUE_CLAW: SimpleWeapon = SimpleWeapon::reach_melee(
    "tarrasque claw",
    &["t-claw", "tclaw"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Slashing,
    3,
);

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
                min_range: None,
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
pub static ABOLETH_TENTACLE: SimpleWeapon = SimpleWeapon::reach_melee(
    "tentacle",
    &["tent"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    2,
);

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
                min_range: None,
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
        let Some(point) = first_target_location(target_locations) else {
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
                min_range: None,
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
                min_range: None,
                is_spell: false,
            },
        );
        if !effects.is_empty() {
            add_flat_damage_rider(
                encounter,
                target_id,
                Dice::new(3, 8),
                DamageType::Poison,
                "erinyes longsword",
                &mut effects,
            );
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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 6),
            DamageType::Fire,
            "hellfire bite",
            &mut effects,
        );
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
        let Some(point) = first_target_location(target_locations) else {
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
pub static WYVERN_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "wyvern bite",
    &["wbite"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
    2,
);

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
pub static STORM_GIANT_GREATSWORD: SimpleWeapon = SimpleWeapon::reach_melee(
    "storm greatsword",
    &["sgs", "sgreatsword"],
    AbilityScoreType::Strength,
    Dice::new(6, 6),
    DamageType::Slashing,
    3,
);

/// Storm Giant Thrown Rock — STR-based 4d12 + STR bludgeoning ranged
/// attack. Range 240ft RAW; capped at 40 tiles to fit the map. The
/// storm giant's stand-off lane when the front line is buttoned up.
pub static STORM_GIANT_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "storm rock",
    &["sgr", "srock"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Bludgeoning,
    40,
    24,
);

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
pub static HYDRA_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "hydra bite",
    &["h-bite", "hbite"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    2,
);

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
pub static STONE_GIANT_GREATCLUB: SimpleWeapon = SimpleWeapon::reach_melee(
    "stone greatclub",
    &["s-gc", "sgc"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
    3,
);

/// Stone Giant Boulder — STR-based 4d10+STR bludgeoning ranged, reach 24.
/// The boulder is the giant's signature ranged threat — paired with the
/// greatclub for melee, the AI picks whichever the action picker validates.
pub static STONE_GIANT_BOULDER: SimpleWeapon = SimpleWeapon::ranged(
    "stone boulder",
    &["s-boulder", "sboulder"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    24,
    16,
);

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

    fn deals_damage(&self) -> bool {
        // Control, not damage. Without this the AI's focus-fire lane
        // scores it as an attack and picks it over one — a medusa
        // re-gazed an already-petrified succubus four hundred and
        // seventy times running rather than finishing it, and the fight
        // could not end.
        false
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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(4, 6),
            DamageType::Poison,
            "snake hair",
            &mut effects,
        );
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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 6),
            DamageType::Fire,
            "salamander tail",
            &mut effects,
        );
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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 6),
            DamageType::Fire,
            "salamander spear",
            &mut effects,
        );
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
                min_range: None,
                is_spell: false,
            },
        );
        if slash_dmg == 0 {
            return effects;
        }
        // Necrotic rider — only fires on a successful slash. Routes through
        // the shared `add_flat_damage_rider` helper so the roll / log /
        // DealDamage trio lives in one chokepoint with the rest of the
        // on-hit typed-damage riders.
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(4, 8),
            DamageType::Necrotic,
            "longsword",
            &mut effects,
        );
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
        let Some(point) = first_target_location(target_locations) else {
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
        const DC: i32 = 13;
        const RADIUS: isize = 24; // 60 ft
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        encounter.log("  horrifying visage: enemies make a WIS save vs DC 13");
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            caster_loc,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Frightened,
            ConditionTimer::Rounds(5),
        )
    }
}

pub static GHOST_HORRIFYING_VISAGE: LazyLock<GhostHorrifyingVisage> =
    LazyLock::new(|| GhostHorrifyingVisage {});

/// Stone Golem Slam — STR-based 3d8+STR bludgeoning melee, reach 1.
/// The golem's only attack (RAW: 2 slams per multi). No rider effects;
/// pure crushing damage. Stays a SimpleWeapon so the multiattack
/// wrapper can re-use it cleanly.
pub static STONE_GOLEM_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "stone slam",
    &["s-slam", "sslam"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
);

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
        const DC: i32 = 17;
        const RADIUS: isize = 2; // 10ft
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        encounter.log("  stone golem slow: enemies make a WIS save vs DC 17");
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            caster_loc,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Slowed,
            ConditionTimer::Rounds(5),
        )
    }
}

pub static STONE_GOLEM_SLOW: LazyLock<StoneGolemSlow> = LazyLock::new(|| StoneGolemSlow {});

/// Bullette Bite — STR-based 4d12+STR piercing melee, reach 1. The
/// bullette's signature crunch — averages ~26 piercing per hit. No
/// rider effects; pure damage. Stays a `SimpleWeapon` so the bullette's
/// loadout can mix this with the Deadly Leap follow-up cleanly.
pub static BULLETTE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bullette bite",
    &["bbite", "bullette-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
);

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
pub static BONE_DEVIL_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "bone devil claws",
    &["bdclaws"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

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
pub static AIR_ELEMENTAL_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "air slam",
    &["aslam"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

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
pub static EARTH_ELEMENTAL_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "earth slam",
    &["eslam"],
    AbilityScoreType::Strength,
    Dice::new(4, 8),
    DamageType::Bludgeoning,
);

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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(3, 8),
            DamageType::Lightning,
            "balor runes",
            &mut effects,
        );
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
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(3, 6),
            DamageType::Lightning,
            "balor whip lightning",
            &mut effects,
        );
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
pub static GLABREZU_PINCER: SimpleWeapon = SimpleWeapon::melee(
    "glabrezu pincer",
    &["gpincer"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Bludgeoning,
);

/// Glabrezu Fist — STR-based 2d4 + STR bludgeoning melee, reach 1. The
/// glabrezu's secondary attack lane; pairs with the pincers in its
/// 4-swing multiattack (2 pincers + 2 fists per Action).
pub static GLABREZU_FIST: SimpleWeapon = SimpleWeapon::melee(
    "glabrezu fist",
    &["gfist"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

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
pub static MARILITH_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "marilith longsword",
    &["mls", "marilith-ls"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

/// Marilith Tail — STR-based 2d10 + STR bludgeoning melee, reach 2 (the
/// snake-body tail extends 10ft per RAW). Final swing of the multiattack
/// envelope. We collapse the RAW "constrict / grapple on hit" rider —
/// the engine's grapple gate doesn't yet model the "creature one size
/// larger or smaller" clause, and the long reach + the multi's volume
/// already make the marilith threatening enough.
pub static MARILITH_TAIL: SimpleWeapon = SimpleWeapon::reach_melee(
    "marilith tail",
    &["mtail", "marilith-t"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Bludgeoning,
    2,
);

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
pub static VROCK_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "vrock talons",
    &["vtalons"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Vrock Beak — STR-based 2d6 + STR piercing melee, reach 1. The vrock's
/// finisher — same damage profile as the talons but piercing rather than
/// slashing, so resistance / vulnerability typing can vary the swing's
/// output across the multi.
pub static VROCK_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "vrock beak",
    &["vbeak"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

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
        // RAW's filter is "each creature within 20 feet of it ... other
        // than demons". The engine's `CreatureType` doesn't split demons
        // from devils — both are `Fiend` — so the gate spares every fiend,
        // which over-spares devils that RAW would catch. Still far tighter
        // than the poison-immunity proxy this used to read, which also
        // spared golems, giant spiders, yuan-ti and the tarrasque.
        for tid in encounter.enemy_burst_targets(caster_id, center, RADIUS) {
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if target.creature_type() == crate::engine::types::CreatureType::Fiend {
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
pub static SHAMBLING_MOUND_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "shambling slam",
    &["sslam", "sm-slam"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

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
            effects.extend(crate::engine::side_effects::install_condition_with_link(
                Condition::Grappled,
                target_id,
                caster_id,
                ConditionTimer::Rounds(10),
            ));
        }
        effects
    }
}

pub static SHAMBLING_MOUND_ENGULF: LazyLock<ShamblingMoundEngulf> =
    LazyLock::new(|| ShamblingMoundEngulf {});

/// Displacer Beast tentacle — STR-based 2d6 bludgeoning melee attack with
/// 10ft reach (2 tiles). The displacer beast lashes out with a barbed
/// tentacle; two of these compose its multiattack.
pub static TENTACLE: SimpleWeapon = SimpleWeapon::reach_melee(
    "tentacle",
    &["tent"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    2,
);

/// Displacer Beast multiattack — two tentacle strikes per Action.
pub static DISPLACER_BEAST_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "tentacle flurry",
    sub_attack: &TENTACLE,
    count: 2,
});

/// Umber Hulk claw — STR-based 1d8 slashing melee attack. The umber hulk
/// rakes with a massive chitinous claw; standard 5ft reach.
pub static UMBER_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "umber claw",
    &["uclaw"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Cloaker tail — STR-based 1d8 slashing melee attack with 10ft reach
/// (2 tiles). The cloaker whips its barbed tail at nearby prey.
pub static CLOAKER_TAIL: SimpleWeapon = SimpleWeapon::reach_melee(
    "cloaker tail",
    &["ctail"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
    2,
);

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

/// Chuul Pincer — STR-based 2d6+STR bludgeoning melee with an
/// auto-Grappled install on hit (no save). The CR-4 lobster-aberration's
/// signature swing: the pincer snaps shut, the target is grappled, and
/// the chuul's bonus-action Tentacles paralyze rider follows up on the
/// pinned target. RAW: "Hit: 11 (2d6 + 4) bludgeoning damage, and the
/// target is grappled (escape DC 14)."
///
/// Routes through the shared `WeaponWithCondition::melee` chassis —
/// same auto-install-on-hit lane as `GIANT_FROG_BITE` / `MIMIC_BITE`-
/// adjacent abilities. Re-installing Grappled on an already-grappled
/// actor is a clean no-op at the `add_condition` chokepoint (timer
/// resolution picks the longer of the two), so the chassis's hit-or-
/// no-install contract folds cleanly into the chuul's per-Action
/// rhythm. Replaced the previous ~70 lines of hand-rolled
/// `impl Action for ChuulPincer` + `LazyLock` boilerplate with the
/// shared data-only literal.
pub static CHUUL_PINCER: WeaponWithCondition = WeaponWithCondition::melee(
    "pincer",
    &["claw"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    Condition::Grappled,
    ConditionTimer::Rounds(10),
    "chuul pincer",
);

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
            add_flat_damage_rider(
                encounter,
                target_id,
                Dice::new(1, 6),
                DamageType::Acid,
                "acid splash",
                &mut effects,
            );
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

/// Giant Scorpion Claw — STR-based 1d8+STR bludgeoning melee with an
/// auto-Grappled install on hit (no save). The CR-3 desert hunter's
/// pincer swing: the claw snaps shut, the target is grappled, and the
/// scorpion's tail-sting follow-up lands on the pinned target. RAW:
/// "Hit: 6 (1d8 + 2) bludgeoning damage. The target is grappled
/// (escape DC 12)."
///
/// Routes through the shared `WeaponWithCondition::melee` chassis —
/// same auto-install-on-hit lane as `CHUUL_PINCER` / `GIANT_FROG_BITE`.
/// Re-installing Grappled on an already-grappled actor is a clean
/// no-op at the `add_condition` chokepoint (timer resolution picks
/// the longer of the two). Replaced ~60 lines of hand-rolled
/// `impl Action for GiantScorpionClaw` + `LazyLock` boilerplate with
/// the shared data-only literal.
pub static GIANT_SCORPION_CLAW: WeaponWithCondition = WeaponWithCondition::melee(
    "claw",
    &["scorpion-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Bludgeoning,
    Condition::Grappled,
    ConditionTimer::Rounds(10),
    "scorpion claw",
);

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
pub static GRICK_TENTACLES_WEAPON: SimpleWeapon = SimpleWeapon::melee(
    "tentacles",
    &["grick-tent"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Slashing,
);

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
pub static GLAIVE: SimpleWeapon = SimpleWeapon::reach_melee(
    "glaive",
    &["glv", "polearm"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Slashing,
    2,
);

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
            None,
        )
    }
}

pub static SPECTATOR_EYE_RAY: LazyLock<SpectatorEyeRay> = LazyLock::new(|| SpectatorEyeRay {});

/// Javelin -- STR-based thrown weapon: 1d6 piercing, 30ft normal / 120ft max.
pub static JAVELIN: SimpleWeapon = SimpleWeapon::ranged(
    "javelin",
    &["jav", "throw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
    48,
    12,
);

/// Hobgoblin Warlord multiattack -- three longsword swings per Action.
pub static HOBGOBLIN_WARLORD_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "triple longsword",
    sub_attack: &LONGSWORD,
    count: 3,
});

/// Giant Eagle beak — STR-based 1d6 piercing melee. Paired with
/// `GIANT_EAGLE_TALONS` in a CompoundAttack: the multi opens with the beak
/// peck and follows with two raking talon strikes.
pub static GIANT_EAGLE_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "beak",
    &["peck"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Giant Eagle talons — STR-based 2d6 slashing melee. Heavier than the
/// beak: the raptor's main damage source. Paired with `GIANT_EAGLE_BEAK`
/// via the eagle's CompoundAttack multi.
pub static GIANT_EAGLE_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "talons",
    &["claws", "rake"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Giant Eagle multiattack — one beak peck + one talon rake per Action,
/// matching the SRD stat block's "one beak, one talons" multi. Implemented
/// via the heterogeneous `CompoundAttack` wrapper since the two limbs
/// have distinct damage dice (1d6 vs 2d6).
pub static GIANT_EAGLE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "beak + talons",
    parts: vec![(&GIANT_EAGLE_BEAK, 1), (&GIANT_EAGLE_TALONS, 1)],
});

/// Sahuagin claws — STR-based 1d4 slashing melee. Paired with
/// `SAHUAGIN_BITE` in the multi. The 1d4 keeps the base damage modest;
/// the Blood Frenzy passive (advantage on melee attacks vs wounded
/// targets) is the actual damage amp.
pub static SAHUAGIN_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["scratch"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Slashing,
);

/// Sahuagin bite — STR-based 1d4 piercing melee. Symmetric with
/// `SAHUAGIN_CLAWS`; together the multi resolves bite + claws.
pub static SAHUAGIN_BITE: SimpleWeapon = SimpleWeapon::melee(
    "shark-tooth bite",
    &["bite", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Sahuagin multiattack — one bite + one claws per Action.
pub static SAHUAGIN_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&SAHUAGIN_BITE, 1), (&SAHUAGIN_CLAWS, 1)],
});

/// Lizardfolk bite — STR-based 1d6 piercing melee. The bite is the
/// reptile's reliable always-available swing; the multi pairs it with a
/// weapon swing for the "claws-and-teeth" hit profile in the SRD stat
/// block.
pub static LIZARDFOLK_BITE: SimpleWeapon = SimpleWeapon::melee(
    "lizard bite",
    &["bite", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Heavy Club — STR-based 1d6 bludgeoning melee. The lizardfolk's
/// signature weapon: hits hard for a CR ½ humanoid when paired with the
/// natural bite via Multiattack.
pub static HEAVY_CLUB: SimpleWeapon = SimpleWeapon::melee(
    "heavy club",
    &["club", "hc"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

/// Lizardfolk multiattack — one bite + one club swing per Action.
pub static LIZARDFOLK_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + club",
    parts: vec![(&LIZARDFOLK_BITE, 1), (&HEAVY_CLUB, 1)],
});

/// Giant Ape fist — STR-based 3d6 bludgeoning melee. Pure punch with no
/// rider; the ape's brute melee is its calling card and dual fists land
/// twice per Action via the multi.
pub static GIANT_APE_FIST: SimpleWeapon = SimpleWeapon::melee(
    "fist",
    &["punch", "slam"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
);

/// Giant Ape rock — STR-based 7d6 bludgeoning thrown rock with extreme
/// range. Big single-die hit when the ape can't close — mirrors the Hill
/// Giant boulder shape but tuned for CR 7 hp budgets.
pub static GIANT_APE_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "rock",
    &["throw", "boulder"],
    AbilityScoreType::Strength,
    Dice::new(7, 6),
    DamageType::Bludgeoning,
    24,
    20,
);

/// Giant Ape multiattack — two fist slams per Action, mirroring the
/// SRD stat block's "Multiattack: makes two fist attacks" entry.
pub static GIANT_APE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double fist",
    sub_attack: &GIANT_APE_FIST,
    count: 2,
});

/// Centaur pike — STR-based 1d10 piercing, reach 2 (10 ft polearm).
/// Outranges every other martial weapon in the centaur's kit and slots
/// neatly into the multi as the heavier of the two limbs. Carries RAW's
/// **Charge** rider (+3d6 piercing) through `CHARGE_RIDERS`, and is the
/// one row there that wants a thirty-foot run-up rather than twenty.
pub static CENTAUR_PIKE: SimpleWeapon = SimpleWeapon::reach_melee(
    "pike",
    &["polearm", "p"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    2,
);

/// Centaur hooves — STR-based 2d6 bludgeoning melee. The kicker
/// follow-up to the pike thrust; pairs with `CENTAUR_PIKE` in the multi.
pub static CENTAUR_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "hooves",
    &["kick", "stomp"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Centaur multiattack — one pike thrust + one hoof kick per Action.
pub static CENTAUR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "pike + hooves",
    parts: vec![(&CENTAUR_PIKE, 1), (&CENTAUR_HOOVES, 1)],
});

/// Brown Bear bite — STR 1d8+4 piercing. The grindy half of the bear's
/// MultiAttack; pairs with the claws for the standard "one bite, one
/// rake" Action turn.
pub static BROWN_BEAR_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Brown Bear claws — STR 2d6+4 slashing. The heavier half of the
/// bear's multi; the rake follow-up after the bite.
pub static BROWN_BEAR_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["c", "rake"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Brown Bear multiattack — one bite + one claw rake per Action.
pub static BROWN_BEAR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&BROWN_BEAR_BITE, 1), (&BROWN_BEAR_CLAWS, 1)],
});

/// Tiger bite — STR 1d10+5 piercing. Bigger jaw than the bear; the
/// damage half of the cat's pounce-and-bite combo.
pub static TIGER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
);

/// Tiger claws — STR 1d8+5 slashing. The lighter half of the multi
/// pair; the rake after the bite lands. Carries RAW's **Pounce** — a
/// STR save vs Prone when the cat reaches the target across a
/// straight-line run — through `CHARGE_RIDERS`. The free bite RAW
/// grants against a target it flattens is the unmodeled half.
pub static TIGER_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["c", "rake"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Tiger multiattack — one bite + one claw per Action.
pub static TIGER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&TIGER_BITE, 1), (&TIGER_CLAWS, 1)],
});

/// Boar tusks — STR 1d6+1 slashing. The CR-1/4 boar's only swing.
/// RAW's **Charge** rider — extra 1d6 plus a STR save vs Prone after a
/// 20 ft straight-line run — rides this weapon through `CHARGE_RIDERS`
/// in `engine::attack`, which reads the run off the attacker's
/// turn-start tile. Nothing on this struct expresses it: a charge is a
/// fact about the board, not about the weapon.
pub static BOAR_TUSKS: SimpleWeapon = SimpleWeapon::melee(
    "tusks",
    &["t", "tusk", "gore"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Giant Toad bite — STR 1d10+2 piercing + 1d10 poison splash on hit
/// (the engine bypasses RAW's "on a successful CON save it takes half
/// the poison" wrinkle and just lands the rider; the toad is a CR-1
/// monster and the rider is the iconic flavor). RAW also grapples
/// Medium-or-smaller targets on hit — that grapple half isn't modeled.
/// Giant Toad bite — STR-based 1d10+STR piercing + flat 1d10 poison
/// rider on hit via the shared `WeaponWithRider` chassis. Same rider
/// chassis as Yuan-Ti Bite / Death Knight Longsword / Wereboar Tusks /
/// Magmin Touch / Djinni Scimitar / Efreeti Scimitar — replaces the
/// previous bespoke `GiantToadBite` Action impl whose `side_effects`
/// was just `weapon_swing_with_flat_rider` plumbing the chassis
/// already centralizes. RAW's grapple-on-hit (Medium-or-smaller) and
/// half-poison-on-CON-pass clauses are deliberately skipped — neither
/// is modeled cleanly here, and the pure piercing-plus-flat-poison
/// envelope captures the load-bearing flavor.
pub static GIANT_TOAD_BITE: WeaponWithRider = WeaponWithRider::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    Dice::new(1, 10),
    DamageType::Poison,
    "bite poison",
);

/// Pseudodragon sting — DEX 1d4+2 piercing + a DC-11 CON save against
/// magical sleep. On fail: target falls Unconscious for 1 hour OR until
/// it takes damage. The engine collapses the "or until damage" clause
/// to a fixed 10-round timer (matching the Sleep cantrip envelope) so
/// the bookkeeping stays simple. The bite + sting pair plus the
/// poison-sleep rider is the iconic pseudodragon kit; the bite half is
/// a SimpleWeapon below.
pub struct PseudodragonSting {}

impl Action for PseudodragonSting {
    fn name(&self) -> &str {
        "sting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["s", "tail"]
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
        use crate::engine::side_effects::ApplyCondition;

        let target_id = first_target_id(target_ids);
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
        let Some(target_id) = target_id else {
            return effects;
        };
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 11);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static PSEUDODRAGON_STING: LazyLock<PseudodragonSting> =
    LazyLock::new(|| PseudodragonSting {});

/// Pseudodragon bite — DEX-based 1d4+0 piercing. Lightweight follow-up
/// to the sting; the dragonling's tiny jaws don't add the DEX modifier
/// to damage (RAW: 1 piercing flat for a CR-1/4 stat block). Routes
/// through the shared `SimpleWeapon::flat_melee` constructor — the
/// `damage_ability: None` chokepoint that keeps a damage roll free of
/// the to-hit ability's modifier.
pub static PSEUDODRAGON_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "bite",
    &["b", "nip"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Behir bite — STR 3d10+6 piercing. The lightning serpent's signature
/// melee chomp; pairs with the constrict in the multi.
pub static BEHIR_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Piercing,
);

/// Behir constrict — STR 2d10+6 bludgeoning + 2d10 slashing on hit.
/// RAW grapples Huge-or-smaller targets on hit; the grapple half isn't
/// modeled — the constrict still lands both damage components via the
/// dedicated action below.
pub struct BehirConstrict {}

impl Action for BehirConstrict {
    fn name(&self) -> &str {
        "constrict"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["c", "coil", "crush"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Slashing]
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
            Dice::new(2, 10),
            DamageType::Bludgeoning,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else {
            return effects;
        };
        let slash_raw = encounter.roll(&Dice::new(2, 10));
        encounter.log(format!(
            "  constrict slash: 2d10({}) = {} slashing",
            slash_raw, slash_raw
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: slash_raw,
            damage_type: DamageType::Slashing,
        }));
        effects
    }
}

pub static BEHIR_CONSTRICT: LazyLock<BehirConstrict> = LazyLock::new(|| BehirConstrict {});

/// Behir multiattack — one bite + one constrict per Action. The
/// signature melee burst the serpent leads with when its lightning
/// breath is on cooldown.
pub static BEHIR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + constrict",
    parts: vec![(&BEHIR_BITE, 1), (&*BEHIR_CONSTRICT, 1)],
});

/// Tiny Animated Object slam — STR-based 1d4+STR force. The signature
/// touch attack of the conjured swarm the Animate Objects spell summons.
/// Force damage (RAW: "magical bludgeoning damage" — force is the
/// closest engine match since it has no resistance lane below
/// Mind-Blanked) keeps the minions relevant against the standard
/// physical-resistant cohort (skeletons, golems, undead with the
/// Bludgeoning-resistant block) and against summons targeting
/// non-magical-weapon-resistant fiends.
pub static TINY_ANIMATED_OBJECT_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "animated slam",
    &["aslam", "object slam"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Force,
);

/// Polar Bear bite — STR 1d8+5 piercing. Bigger jaw than the Brown Bear
/// (CR 1) bite; the grindy half of the polar's multi. Slots between Brown
/// Bear (1d8+4 / 2d6+4) and Tiger (1d10+5 / 1d8+5) on the bear-claws
/// ladder, with the heavier polar-specific +1 STR mod.
pub static POLAR_BEAR_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Polar Bear claws — STR 2d6+5 slashing. Heavier rake than the Brown
/// Bear thanks to the polar's bigger STR (20 vs 19). Pairs with the bite
/// in the standard "bite + claws" Multiattack chassis.
pub static POLAR_BEAR_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["c", "rake"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Polar Bear multiattack — one bite + one claw rake per Action. Same
/// shape as Brown Bear / Tiger; the polar's +5 STR mod is the
/// CR-2-vs-CR-1 step.
pub static POLAR_BEAR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&POLAR_BEAR_BITE, 1), (&POLAR_BEAR_CLAWS, 1)],
});

/// Lion bite — STR 1d8+3 piercing. The first half of the pride hunter's
/// multi. Lions are CR 1 large beasts; pairs with the claws for the
/// standard "bite + rake" turn against a focused target.
pub static LION_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Lion claws — STR 1d6+3 slashing. Lighter rake; the lion compensates
/// with Pack Tactics so an adjacent ally gives advantage on every swing.
pub static LION_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "claws",
    &["c", "rake"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Lion multiattack — one bite + one claw per Action. RAW also has a
/// Pounce option (CR-1 STR-save Prone on a 20 ft straight charge); the
/// engine doesn't track straight-line movement so we collapse to the
/// vanilla bite + claws chassis (Pack Tactics handles the Lion's
/// advantage-on-attack flavor on its own).
pub static LION_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&LION_BITE, 1), (&LION_CLAWS, 1)],
});

/// Fire Giant Greatsword — STR-based 6d6+STR slashing, reach 2 (10ft).
/// Sibling to the Storm Giant Greatsword (6d6 + STR at reach 3): a touch
/// shorter reach because the fire giant's signature is "anvil-and-hammer
/// blacksmith" rather than the storm giant's celestial scale. CR-9 melee
/// damage; mirrors the Frost Giant Greataxe (3d12) at the dice tier but
/// in dice-count-vs-die-size shape.
pub static FIRE_GIANT_GREATSWORD: SimpleWeapon = SimpleWeapon::reach_melee(
    "fire giant greatsword",
    &["fgs", "fire-sword"],
    AbilityScoreType::Strength,
    Dice::new(6, 6),
    DamageType::Slashing,
    2,
);

/// Fire Giant Rock — STR-based 4d10+STR bludgeoning thrown rock, reach
/// 24 (60ft). Same chassis as the Hill/Frost/Stone Giant Rock; the Fire
/// Giant gets the heavier 4d10 die (matches Frost Giant's 4d10) at the
/// CR-9 tier.
pub static FIRE_GIANT_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "fire rock",
    &["fgrock", "firock"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    24,
    16,
);

/// Cyclops Greatclub — STR-based 3d8+STR bludgeoning, reach 3 (15ft).
/// Same dice as Stone Giant's club; the Cyclops sits a tier lower (CR 6
/// vs CR 7) on lower CON/INT but the same melee envelope. The one-eyed
/// brute's single signature swing.
pub static CYCLOPS_GREATCLUB: SimpleWeapon = SimpleWeapon::reach_melee(
    "cyclops greatclub",
    &["cgc", "cyclub"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
    3,
);

/// Cyclops Rock — STR-based 4d10+STR bludgeoning thrown rock, reach 24
/// (60ft). The Cyclops is a notoriously poor shot in 5e (their one eye
/// gives disadvantage on ranged attacks vs distant targets) but we model
/// the rock as a clean ranged option — the AI rarely picks it when
/// melee is available, and the disadvantage flavor reads through the
/// normal_range cap that already imposes disadvantage at long range.
pub static CYCLOPS_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "cyclops rock",
    &["crock"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    24,
    12,
);

/// Cyclops Multiattack — 2 greatclub swings per Action. Mirrors the
/// Stone Giant / Frost Giant Multiattack: pure physical thresher, no
/// rider effects.
pub static CYCLOPS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "cyclops multiattack",
    sub_attack: &CYCLOPS_GREATCLUB,
    count: 2,
});

/// Roc Beak — STR-based 4d8+STR piercing, reach 2 (10ft). The first
/// half of the gargantuan eagle's multi. RAW has the Roc as Gargantuan
/// (4x4 footprint) but Huge (3x3) is the largest size the engine
/// supports cleanly — we use Huge here so the spawn placement code
/// doesn't choke on the 4x4 footprint.
pub static ROC_BEAK: SimpleWeapon = SimpleWeapon::reach_melee(
    "roc beak",
    &["rbeak"],
    AbilityScoreType::Strength,
    Dice::new(4, 8),
    DamageType::Piercing,
    2,
);

/// Roc Talons — STR-based 4d6+STR slashing, reach 2 (10ft). The second
/// half of the multi; the talons rake after the beak strike. Mirrors
/// the Giant Eagle beak + talons shape at much higher dice.
pub static ROC_TALONS: SimpleWeapon = SimpleWeapon::reach_melee(
    "roc talons",
    &["rtalons"],
    AbilityScoreType::Strength,
    Dice::new(4, 6),
    DamageType::Slashing,
    2,
);

/// Roc multiattack — 1 beak + 1 talons per Action. CR-11 dice tier:
/// the Roc bursts an unguarded target down hard in a single round.
pub static ROC_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "beak + talons",
    parts: vec![(&ROC_BEAK, 1), (&ROC_TALONS, 1)],
});

/// Pegasus Hooves — STR-based 2d6+STR bludgeoning, reach 1. Large
/// celestial steed: only one attack lane per turn, so the dice are
/// tuned a hair above a CR-1 brown bear claw to land on the CR-2 line
/// alongside Polar Bear. RAW has Hooves as the only Action; the
/// pegasus's profile leans on movement (90 ft fly) and the Celestial
/// type rather than rider effects.
pub static PEGASUS_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "hooves",
    &["hv", "kick"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Winter Wolf bite — STR-based 2d6+STR piercing + a 1d8 cold rider on
/// hit. RAW: bite + cold rider + trip; we collapse the trip rider here
/// to keep the action shape close to the vanilla Wolf bite (which already
/// has a STR-save Prone rider) — the Winter Wolf's distinguishing
/// signature is the cold breath weapon, not yet-another-prone trigger.
/// The cold damage applies through the same DealDamage chain so target
/// cold resistance / immunity halves / nullifies it independently of
/// the piercing.
pub struct WinterWolfBite {}

impl Action for WinterWolfBite {
    fn name(&self) -> &str {
        "winter wolf bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wwb", "frostbite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Cold]
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
            "winter wolf bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(2, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 8),
            DamageType::Cold,
            "frost rider",
            &mut effects,
        );
        effects
    }
}

pub static WINTER_WOLF_BITE: LazyLock<WinterWolfBite> =
    LazyLock::new(|| WinterWolfBite {});

/// Winter Wolf cold breath — burst-3 / range-4, 4d8 cold, DC 12 CON,
/// half on save. Recharge 5-6 via the shared `"breath_weapon"` pool.
/// RAW: 15-ft cone; we model it as a small burst at moderate range so
/// the AI can still aim it at clustered targets. CR-3 dice tier — well
/// below dragon breath, well above the wolf trip.
pub static WINTER_WOLF_BREATH: BreathWeapon = BreathWeapon {
    display_name: "cold breath",
    aliases: &["wwc", "frost-breath"],
    damage_dice: Dice::new(4, 8),
    damage_type: DamageType::Cold,
    save_ability: AbilityScoreType::Constitution,
    dc: 12,
    radius: 2,
    range: 4,
    recharge_key: "breath_weapon",
};

/// Triceratops Gore — STR-based 4d8+STR piercing, reach 2 (10 ft). The
/// huge ceratopsian's signature charge: high single-die damage that
/// rewards reach over multi-strike spam. RAW's **Trampling Charge** —
/// Prone on a failed STR save after a straight-line move-then-hit —
/// rides this weapon through `CHARGE_RIDERS` in `engine::attack`. The
/// bonus stomp against a target it knocks down is the half that stays
/// unmodeled.
pub static TRICERATOPS_GORE: SimpleWeapon = SimpleWeapon::reach_melee(
    "gore",
    &["gr", "horn-charge"],
    AbilityScoreType::Strength,
    Dice::new(4, 8),
    DamageType::Piercing,
    2,
);

/// Triceratops Stomp — STR-based 3d10+STR bludgeoning, reach 1. RAW
/// only triggers vs Prone targets; we expose it as a vanilla swing the
/// AI can pick when the gore is out of reach (the Triceratops's full
/// envelope: gore at reach 2 OR stomp at reach 1, never both per turn).
pub static TRICERATOPS_STOMP: SimpleWeapon = SimpleWeapon::melee(
    "stomp",
    &["st", "trample"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Bludgeoning,
);

/// Tyrannosaurus Rex Bite — STR-based 4d12+STR piercing, reach 2 (10 ft).
/// The apex predator's marquee swing. RAW also has a Bite-and-Grapple
/// rider (grappled + restrained vs Large or smaller); we collapse to
/// the vanilla high-die hit since grapple-from-monster is a niche the
/// engine doesn't currently use on huge predators.
pub static T_REX_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "rex bite",
    &["rb", "trex-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
    2,
);

/// Tyrannosaurus Rex Tail — STR-based 3d8+STR bludgeoning, reach 2.
/// The second multi-lane attack. Lower dice than the bite (no grapple
/// risk on the RAW lane), so the tail is the "everything not in front
/// of me also dies" sweep.
pub static T_REX_TAIL: SimpleWeapon = SimpleWeapon::reach_melee(
    "rex tail",
    &["rt", "trex-tail"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
    2,
);

/// T-Rex multiattack — 1 bite + 1 tail per Action. RAW: can't target
/// the same creature with both attacks; we don't enforce that because
/// the engine resolves Compound parts as independent target slots so
/// the AI naturally splits the swings when two enemies are in reach.
pub static T_REX_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + tail",
    parts: vec![(&T_REX_BITE, 1), (&T_REX_TAIL, 1)],
});

/// Carrion Crawler tentacles — DEX-based 1d4 + DEX poison melee with a
/// CON save (DC 13) on hit. Fail = Paralyzed for 1 round. The damage
/// is trivial; the paralysis lockout is the threat — same shape as the
/// Cockatrice's petrifying bite or the Ghoul's claws (which apply
/// Paralyzed RAW on a 3d8 ghoul-touch rider). Paralyzed locks the
/// target's action economy AND auto-fails STR/DEX saves, so a single
/// hit can swing the round if the save misses.
pub struct CarrionCrawlerTentacles {}

impl Action for CarrionCrawlerTentacles {
    fn name(&self) -> &str {
        "tentacles"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tn", "lash"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10 ft RAW = 2 tiles. Tentacles are longer than the bite below.
        Some(2)
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
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            "tentacles",
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 4),
            DamageType::Poison,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        if !save.passed() {
            encounter.log("  tentacles: target seizes up, paralyzed");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static CARRION_CRAWLER_TENTACLES: LazyLock<CarrionCrawlerTentacles> =
    LazyLock::new(|| CarrionCrawlerTentacles {});

/// Carrion Crawler bite — STR-based 1d6+STR piercing, reach 1. The
/// crawler's secondary swing once it's already in melee. No rider,
/// just clean-up damage after the tentacle paralysis sticks.
pub static CARRION_CRAWLER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "crawler bite",
    &["cb", "crawl-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Carrion Crawler multiattack — 1 tentacles + 1 bite per Action. RAW
/// uses CompoundAttack so the lock-then-chew rhythm reads as two
/// distinct log lines.
pub static CARRION_CRAWLER_MULTI: LazyLock<CompoundAttack> =
    LazyLock::new(|| CompoundAttack {
        display_name: "tentacles + bite",
        parts: vec![(&*CARRION_CRAWLER_TENTACLES, 1), (&CARRION_CRAWLER_BITE, 1)],
    });

/// Water Elemental Slam — STR-based 2d8 + STR bludgeoning melee, reach 1.
/// Same per-swing dice as the Air Elemental's slam, but the water
/// variant's signature kit is the `WATER_ELEMENTAL_WHELM` burst rather
/// than a stronger single-target hit. Slam is the fallback for situations
/// where Whelm is on cooldown or there's a single target out of burst
/// reach.
pub static WATER_ELEMENTAL_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "water slam",
    &["wslam"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

/// Water Elemental Multiattack — 2 slams per Action. Mirrors the Air /
/// Earth elemental wrappers; the water variant's `WHELM` recharge ability
/// is its distinguishing burst.
pub static WATER_ELEMENTAL_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "water elemental multiattack",
    sub_attack: &WATER_ELEMENTAL_SLAM,
    count: 2,
});

/// Water Elemental Whelm — STR save burst around the elemental: every
/// hostile in a 1-tile radius (RAW: each creature in the elemental's
/// space) makes a STR save vs DC 15 or takes 2d8 + STR bludgeoning
/// (half on save) and is knocked Prone (only on fail — the surge
/// staggers off-balance victims into the muck). Recharge 4-6 per RAW;
/// the recharge key plugs into the shared `"whelm"` slot on the
/// template's recharge_abilities so the start-of-turn roller flips it
/// back on a 4+. The 1-tile radius keeps the burst small (a 5-ft
/// surge around the elemental's footprint) so it functions as a
/// "punish anyone who crowded in" reaction rather than a full AoE
/// wash.
pub struct WaterElementalWhelm {}

impl Action for WaterElementalWhelm {
    fn name(&self) -> &str {
        "whelm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wh", "surge"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Caster-centered Burst with radius 1 — the AI passes the
        // elemental's own footprint as the burst origin so the surge
        // catches anyone crowded into its space.
        TargetingSchema::Burst { radius: 1 }
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Recharge gate. The encounter's start-of-turn roller flips the
        // "whelm" slot back on a 4+; until then the action is hidden
        // from the picker via `validate_input`.
        actor_has_recharge(encounter, caster_id, "whelm")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;

        let origin = first_target_location(target_locations).unwrap_or_else(|| {
            encounter
                .actors
                .get(&caster_id)
                .map(|a| a.location())
                .unwrap_or(Coordinate::new(0, 0))
        });
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        const DC: i32 = 15;
        const RADIUS: isize = 1;

        // Spend the recharge slot up-front so a mid-resolution early
        // return can't leave Whelm both "spent" and "damage applied" out
        // of sync — matches the BreathWeapon shape above.
        if let Some(a) = encounter.actors.get_mut(&caster_id) {
            a.spend_recharge("whelm");
        }

        let victims = encounter.enemy_burst_targets(caster_id, origin, RADIUS);
        encounter.log(format!(
            "  whelm: surge of water bursts around the elemental, {} caught",
            victims.len()
        ));

        // Burst damage rolled once and shared (5e shared-AoE-roll); each
        // victim halves on save. Save failure also tags the victim Prone
        // for the standard 5e knock-down rider that the RAW grapple /
        // restrain clauses collapse to in this engine's simpler movement
        // layer.
        let raw = encounter.roll(&Dice::new(2, 8)) + str_mod.max(0) as u32;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for vid in victims {
            let save = encounter.roll_save(vid, AbilityScoreType::Strength, DC);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: vid,
                    amount: dmg,
                    damage_type: DamageType::Bludgeoning,
                }));
            }
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: vid,
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                }));
            }
        }
        effects
    }
}

pub static WATER_ELEMENTAL_WHELM: LazyLock<WaterElementalWhelm> =
    LazyLock::new(|| WaterElementalWhelm {});

/// Saber-toothed Tiger Bite — STR-based 1d10 + STR piercing melee. The
/// heavier-jawed cousin of the Tiger's bite: same die size but the
/// CR-bump comes from the multi pairing and the higher STR mod from
/// the +1 STR on the stat block (rather than a bigger single die).
pub static SABER_TIGER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "saber bite",
    &["sb", "saber-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
);

/// Saber-toothed Tiger Claws — STR-based 2d6 + STR slashing. Heavier
/// rake than the vanilla Tiger's 1d8 claws — the saber-toothed sibling
/// invests its CR bump into the secondary swing rather than the bite.
pub static SABER_TIGER_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "saber claws",
    &["sc", "saber-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Saber-toothed Tiger multiattack — one bite + one claws per Action.
/// Mirrors the Tiger's compound pair (bite + claws) with the heavier
/// per-limb dice; RAW: 5e MM saber-tooth uses the same multi shape but
/// per-limb dice are upgraded.
pub static SABER_TIGER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&SABER_TIGER_BITE, 1), (&SABER_TIGER_CLAWS, 1)],
});

/// Hyena Bite — STR-based 1d6 + STR piercing melee. The CR-0 pack
/// hunter's only swing. Its `has_pack_tactics: true` template flag
/// converts adjacent allies into advantage; the bite itself stays
/// vanilla so the dice tier matches the CR-0 chassis (kobolds /
/// stirges / sprite-tier monsters).
pub static HYENA_BITE: SimpleWeapon = SimpleWeapon::melee(
    "hyena bite",
    &["hb", "yip"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Giant Hyena Bite — STR-based 2d6 + STR piercing melee. The CR-1 large
/// pack hunter: heavier dice than the vanilla hyena, no rider effects.
/// Combined with `has_pack_tactics: true` for the canonical "if a friend
/// is adjacent, it lands at advantage" loop.
pub static GIANT_HYENA_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant hyena bite",
    &["ghb", "giant-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Giant Rat Bite — STR-based 1d4 + STR piercing melee. The CR-⅛ vermin
/// pack scavenger's only swing. The bite is the lowest dice tier in the
/// monster pool (1d4 — same as a Dagger). Combined with the giant rat's
/// `has_pack_tactics: true` template flag, the bite goes to advantage
/// whenever an ally rat is adjacent — the canonical "swarm in the
/// sewers" multiplier. Standalone the swing is trivial; the swarm IS the
/// threat profile.
pub static GIANT_RAT_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant rat bite",
    &["grb", "rat-bite", "nibble"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Green Hag Claws — STR-based 2d8 + STR slashing melee. The classic
/// fey witch's primary swing: chunky dice paired with the hag's
/// magic-resistance / fey-resistance envelope. No rider — the hag's
/// kit lives in the claws-plus-resistance envelope; spell mimicry and
/// invisible-passage clauses from RAW are skipped (the engine doesn't
/// model the "vanishing into the swamp" exit).
pub static GREEN_HAG_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "hag claws",
    &["ghc", "talons"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

// ─── Gorgon ──────────────────────────────────────────────────────────

/// Gorgon Gore — STR-based 2d12+STR piercing melee, reach 1. The iron
/// bull's signature charge swing. RAW has a Trampling Charge rider
/// (Prone on STR save after a straight-line move); we collapse to the
/// vanilla high-die hit since the engine doesn't track straight-line
/// movement for tramples.
pub static GORGON_GORE: SimpleWeapon = SimpleWeapon::melee(
    "gorgon gore",
    &["gg", "iron-gore"],
    AbilityScoreType::Strength,
    Dice::new(2, 12),
    DamageType::Piercing,
);

/// Gorgon Hooves — STR-based 2d10+STR bludgeoning melee, reach 1. The
/// follow-up trampling stomp paired with Gore in the multi. Slightly
/// lower dice than the gore but typed bludgeoning so a fully-armored
/// target with piercing-resistance still takes full damage from the
/// stomp lane.
pub static GORGON_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "gorgon hooves",
    &["gh", "stomp"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Bludgeoning,
);

/// Gorgon Petrifying Breath — 30-foot cone (radius 2 / range 4 in this
/// 2.5ft grid) targeted at a tile. Every actor caught in the burst
/// makes a CON save vs DC 13; on fail, the target picks up Petrified
/// for 1 round (the action-economy lockout is the threat — we cap at
/// 1 round so a single hit doesn't game-over the target, matching the
/// Cockatrice / Medusa / Basilisk shape).
///
/// Recharge 5-6 via the shared `"breath_weapon"` pool so the gorgon
/// can't double-tap with this and a second breath option (it has none,
/// but the shared key keeps the start-of-turn roller uniform).
pub struct GorgonPetrifyingBreath {}

impl Action for GorgonPetrifyingBreath {
    fn name(&self) -> &str {
        "petrifying breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pb", "stone-breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(4)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "breath_weapon")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(origin) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const DC: i32 = 13;
        const RADIUS: isize = 2;
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        encounter.log("  petrifying breath: gorgon exhales a cloud of stoning vapor");
        // Route through the shared `resolve_burst_save_condition` helper —
        // same chokepoint that future save-or-condition AoEs will use.
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            origin,
            RADIUS,
            AbilityScoreType::Constitution,
            DC,
            Condition::Petrified,
            ConditionTimer::Rounds(1),
        )
    }
}

pub static GORGON_PETRIFYING_BREATH: LazyLock<GorgonPetrifyingBreath> =
    LazyLock::new(|| GorgonPetrifyingBreath {});

/// Gorgon Multiattack — 1 gore + 1 hooves per Action. RAW MM has the
/// gorgon's full Action as "Multiattack: The gorgon makes two attacks
/// with its gore" but the visual reading (charge + stomp) reads better
/// as a heterogeneous compound; we keep one gore + one hooves so the
/// damage envelope (≈2d12+2d10+2*STR ≈ 38 average) matches RAW's
/// 2-gore total cleanly.
pub static GORGON_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "gore + hooves",
    parts: vec![(&GORGON_GORE, 1), (&GORGON_HOOVES, 1)],
});

// ─── Yuan-Ti Malison ─────────────────────────────────────────────────

/// Yuan-Ti Malison Bite — STR-based 1d4+STR piercing melee plus a CON
/// save (DC 12) for Poisoned (1 minute / 10 rounds) on hit. Mirrors the
/// Giant Scorpion's "hit + Poisoned" rider: the petty damage is the
/// hook for the poison lockout (attack-roll disadvantage stacks against
/// the malison's other strikes).
pub struct YuanTiMalisonBite {}

impl Action for YuanTiMalisonBite {
    fn name(&self) -> &str {
        "yuan-ti bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ytb", "fang"]
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
            "yuan-ti bite",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Poison rider: 1d4 typed poison + CON save vs Poisoned. The
        // poison damage typing means a poison-resistant target still
        // halves the rider while taking the full piercing.
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 4),
            DamageType::Poison,
            "yuan-ti bite",
            &mut effects,
        );
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 12);
        if !save.passed() {
            encounter.log("  yuan-ti bite: venom courses through the target");
            effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static YUAN_TI_MALISON_BITE: LazyLock<YuanTiMalisonBite> =
    LazyLock::new(|| YuanTiMalisonBite {});

/// Yuan-Ti Malison Multiattack — 1 scimitar + 1 bite per Action. Hybrid
/// fiend warrior signature: the scimitar lands the load-bearing damage
/// while the bite probes for the Poisoned lockout. Routes through the
/// shared `SCIMITAR` simple weapon (1d6+STR slashing).
pub static YUAN_TI_MALISON_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "scimitar + bite",
    parts: vec![(&SCIMITAR, 1), (&*YUAN_TI_MALISON_BITE, 1)],
});

// ─── Cambion ─────────────────────────────────────────────────────────

/// Cambion Spear — STR-based 1d6+STR piercing melee + 2d6 fire rider on
/// hit (the cambion's weapon glows with infernal flame). The fire rider
/// is a separate `DealDamage` so per-target resistance / immunity
/// applies independently from the piercing (a fire-immune target still
/// takes the spear's piercing damage; a fire-vulnerable one takes
/// double on the rider).
pub struct CambionSpear {}

impl Action for CambionSpear {
    fn name(&self) -> &str {
        "infernal spear"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs", "fire-spear"]
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
            "infernal spear",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(2, 6),
            DamageType::Fire,
            "infernal spear",
            &mut effects,
        );
        effects
    }
}

pub static CAMBION_SPEAR: LazyLock<CambionSpear> = LazyLock::new(|| CambionSpear {});

/// Cambion Fire Ray — ranged spell-attack at 24-tile range. CHA-based
/// attack roll vs target AC, 4d6 fire on hit. No ability mod to damage
/// (it's a pure fire-bolt-style cantrip, scaled to the cambion's CR-5
/// damage tier). Sits alongside the spear in the cambion's loadout so
/// the AI can keep up pressure when the target kites out of melee.
pub struct CambionFireRay {}

impl Action for CambionFireRay {
    fn name(&self) -> &str {
        "fire ray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fr", "cambion-bolt"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // CHA-based ranged spell attack (matches the cambion's primary
        // casting stat). No ability mod to damage — the rider IS the
        // damage, no martial bonus stacks.
        simple_weapon_attack_ranged(
            encounter,
            caster_id,
            target_ids,
            "fire ray",
            AbilityScoreType::Charisma,
            None,
            Dice::new(4, 6),
            DamageType::Fire,
            false,
            Some(24),
            None,
        )
    }
}

pub static CAMBION_FIRE_RAY: LazyLock<CambionFireRay> = LazyLock::new(|| CambionFireRay {});

/// Cambion Multiattack — 2 infernal spears per Action. Mirrors the MM
/// stat block's "two melee attacks" envelope; the AI naturally chains
/// the spear's fire rider twice for clustered burst.
pub static CAMBION_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double infernal spear",
    sub_attack: &*CAMBION_SPEAR,
    count: 2,
});

// ─── Dryad ───────────────────────────────────────────────────────────

/// Dryad Club — STR-based 1d4+STR bludgeoning melee. RAW: the dryad
/// casts Shillelagh as a cantrip to imbue the club with a 1d8+WIS
/// magical force-typed swing; we collapse to the plain 1d4+STR base
/// since the cantrip-prime path requires a separate prime turn and the
/// club is the dryad's fallback when its charm fails (the load-bearing
/// kit is `DRYAD_FEY_CHARM`).
pub static DRYAD_CLUB: SimpleWeapon = SimpleWeapon::melee(
    "dryad club",
    &["dc", "wood-club"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

/// Dryad Fey Charm — single-target action at 12-tile (30 ft) range.
/// WIS save vs DC 14; on fail the target is Charmed by the dryad until
/// the dryad takes damage or the spell drops (10-round timer in our
/// engine; RAW: 24 hours). Mirrors the Vampire Charm shape: rolls
/// `SetConditionLink(Condition::Charmed)` so the charmed target can't take hostile actions
/// against the dryad. Charm-immune creatures (constructs / undead /
/// fey themselves per RAW) shrug it off via the standard add_condition
/// gate.
pub struct DryadFeyCharm {}

impl Action for DryadFeyCharm {
    fn name(&self) -> &str {
        "fey charm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fc", "charm"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        save_or_charmed_by_caster(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            14,
            ConditionTimer::Rounds(10),
            "fey charm",
        )
    }
}

pub static DRYAD_FEY_CHARM: LazyLock<DryadFeyCharm> = LazyLock::new(|| DryadFeyCharm {});

// ─── Bullywug ────────────────────────────────────────────────────────

/// Bullywug Bite — STR-based 1d4+STR piercing melee. Low-CR amphibian
/// raider's secondary swing; combines with the spear in the multi for
/// the "frog warrior" double-tap. No rider effects — the bullywug's
/// kit is the spear + bite multi at a low CR price point.
pub static BULLYWUG_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bullywug bite",
    &["bb", "frog-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Bullywug Spear — STR-based 1d6+STR piercing melee. The amphibian
/// raider's signature weapon — short reach (1 tile) but the primary
/// damage lane in the multi. Paired with the bite for the bullywug's
/// "thrust + chomp" double-hit on a single Action.
pub static BULLYWUG_SPEAR: SimpleWeapon = SimpleWeapon::melee(
    "bullywug spear",
    &["bs", "frog-spear"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Bullywug Multiattack — 1 spear + 1 bite per Action. RAW: the
/// bullywug makes two attacks (one with its bite, one with its spear).
/// We model the heterogeneous pair via `CompoundAttack` so each limb
/// uses its own dice tier.
pub static BULLYWUG_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "spear + bite",
    parts: vec![(&BULLYWUG_SPEAR, 1), (&BULLYWUG_BITE, 1)],
});

// ─── Quasit ──────────────────────────────────────────────────────────

/// Quasit Claws — DEX-based 1d4+DEX piercing melee with a CON save (DC 10)
/// for 2d4 poison rider on fail. Quasits are a chaotic-evil mirror of
/// the Imp's lawful-evil devil chassis — they share the tiny-fiend stat
/// envelope and the poisoned-natural-attack pattern. Routes through the
/// shared `WeaponWithSaveDamage` chassis alongside Imp Sting / Purple
/// Worm Tail Stinger — same "weapon hit + save-or-typed-damage" shape.
pub static QUASIT_CLAWS: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "claws",
    &["cl", "quasit-claws"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    10,
    Dice::new(2, 4),
    DamageType::Poison,
    "quasit venom",
);

/// Quasit Scare — single-target action at 4-tile (20 ft) range. The target
/// makes a WIS save vs DC 10; on fail they're Frightened for 1 round (RAW:
/// until end of next turn). One-Action cost; no damage. Targets the same
/// "fear-cohort" condition as Cause Fear / Frightful Presence but at a
/// shorter range and lower DC, fitting the CR-1 tiny-fiend price point.
pub struct QuasitScare {}

impl Action for QuasitScare {
    fn name(&self) -> &str {
        "scare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sc", "quasit-scare"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 20 ft RAW = 8 tiles.
        Some(8)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
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
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        if target.effectively_immune_to_condition(Condition::Frightened) {
            encounter.log("  scare: target shrugs off the fear");
            return Vec::new();
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, 10);
        if save.passed() {
            encounter.log("  scare: target's nerve holds");
            return Vec::new();
        }
        encounter.log("  scare: target recoils in fear");
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Frightened,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static QUASIT_SCARE: LazyLock<QuasitScare> = LazyLock::new(|| QuasitScare {});

// ─── Shadow Demon ────────────────────────────────────────────────────

/// Shadow Demon Claws — DEX-based 2d6+DEX psychic melee. RAW: the demon's
/// chilling, incorporeal claws deal psychic damage. We model the claws as
/// a vanilla psychic-typed `SimpleWeapon` — the "advantage in dim light /
/// darkness" RAW clause is omitted (the engine has no global lighting
/// model), but the load-bearing psychic typing carries the demon's
/// signature damage profile through `effective_damage`'s resistance lane.
pub static SHADOW_DEMON_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "shadow claws",
    &["sdc", "shadow-claws"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Psychic,
);

// ─── Succubus ────────────────────────────────────────────────────────

/// Succubus Claws — DEX-based 1d6+DEX slashing melee. RAW: the claws are
/// a magic weapon (overcome resistance to non-magical physical). We
/// surface the headline slashing damage; the magical-attack clause is
/// approximated by the demon's general fiend resistances elsewhere.
pub static SUCCUBUS_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "succubus claws",
    &["scl", "succ-claws"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Succubus Draining Kiss — single-target Action at melee reach: 5d10
/// psychic damage on hit AND the target's hit-point maximum is reduced by
/// the same amount until they finish a long rest (we route the max-HP
/// drop through `AdjustMaxHp` so the cap drops alongside the damage; the
/// reduction sticks for the duration of combat). RAW: only affects a
/// Charmed target; we gate via the `Charmed` back-link to the succubus, so
/// the kiss fizzles silently when the target isn't already charmed by
/// the caster. No save — the kiss auto-lands once the target is locked
/// in (the difficulty is getting them charmed in the first place).
pub struct SuccubusDrainingKiss {}

impl Action for SuccubusDrainingKiss {
    fn name(&self) -> &str {
        "draining kiss"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dk", "kiss"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW gate: target must currently be Charmed by the succubus.
        // Failing this silently no-ops via the schema-level reach check;
        // the AI's picker filter falls back to the claws when the kiss
        // can't fire.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.linked_by(Condition::Charmed) == Some(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustMaxHp;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(5, 10));
        encounter.log(format!(
            "  draining kiss: 5d10({}) = {} psychic (and max HP drop)",
            raw, raw
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Psychic,
            }),
            // The max-HP drop matches the damage roll — mirrors the
            // 5e RAW (Wraith Life Drain, Wight Life Drain, Succubus Kiss
            // all share this pattern).
            Box::new(AdjustMaxHp {
                actor_id: target_id,
                delta: -(raw as i32),
            }),
        ]
    }
}

pub static SUCCUBUS_DRAINING_KISS: LazyLock<SuccubusDrainingKiss> =
    LazyLock::new(|| SuccubusDrainingKiss {});

/// Succubus Charm — single-target Action at 12-tile (30 ft) range, WIS
/// save vs the succubus's CHA-based DC (caster.spell_save_dc(CHA)). On
/// fail the target picks up `Charmed` (10 rounds) anchored on the
/// succubus via `SetConditionLink(Condition::Charmed)`. Mirrors the Dryad / Vampire charm shape
/// — the load-bearing setup half of the succubus kit, gating the
/// `SUCCUBUS_DRAINING_KISS` follow-up via the `Charmed` back-link.
pub struct SuccubusCharm {}

impl Action for SuccubusCharm {
    fn name(&self) -> &str {
        "succubus charm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sch", "succ-charm"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::install_condition_with_link;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        if target.effectively_immune_to_condition(Condition::Charmed) {
            encounter.log("  succubus charm: target's will is shielded");
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            encounter.log("  succubus charm: target resists the seduction");
            return Vec::new();
        }
        encounter.log("  succubus charm: target falls under the succubus's sway");
        install_condition_with_link(
            Condition::Charmed,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static SUCCUBUS_CHARM: LazyLock<SuccubusCharm> = LazyLock::new(|| SuccubusCharm {});

// ─── Intellect Devourer ──────────────────────────────────────────────

/// Intellect Devourer Claws — DEX-based 2d4+DEX slashing melee. The
/// brain-on-legs aberration's secondary attack; the load-bearing kit is
/// `INTELLECT_DEVOURER_DEVOUR` (the INT save burst). Claws back up the
/// devour as the round-to-round damage lane while the recharge cools.
pub static INTELLECT_DEVOURER_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "intellect claws",
    &["icl", "id-claws"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Devour Intellect — single-target Action at 4-tile (10 ft) range
/// requiring LOS. The target makes an INT save vs DC 12; on fail, they
/// take 4d10 psychic damage and (if the damage knocks them below half
/// HP) pick up `Stunned` until the end of their next turn — the
/// aberration mentally tears at their mind. Distinct from the standard
/// Mind Sliver lane because the save is INT (rare across the engine)
/// and the stun rider gates on a damage threshold rather than a separate
/// save. Mirrors the Mind-Flayer `MIND_BLAST` shape but with a single-
/// target footprint and an INT save instead of INT-save-burst.
pub struct IntellectDevour {}

impl Action for IntellectDevour {
    fn name(&self) -> &str {
        "devour intellect"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["di", "devour"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10 ft RAW = 4 tiles.
        Some(4)
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
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let save = encounter.roll_save(target_id, AbilityScoreType::Intelligence, 12);
        if save.passed() {
            encounter.log("  devour intellect: target's mind holds firm");
            return Vec::new();
        }
        let raw = encounter.roll(&Dice::new(4, 10));
        encounter.log(format!(
            "  devour intellect: 4d10({}) = {} psychic",
            raw, raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Psychic,
        })];
        // Stun rider: gates on the damage being severe enough to knock
        // the target below half HP. Mirrors the "massive damage" gate
        // shape elsewhere — the engine reads pre-damage HP and the rolled
        // amount, so a partially-resisted hit still gets the right
        // threshold check.
        if let Some(target) = encounter.actors.get(&target_id) {
            let max_hp = target.max_hitpoints();
            let cur_hp = target.hitpoints();
            // Threshold: if the rolled damage equals or exceeds half the
            // target's CURRENT HP, the stun lands. Rolls a sliding scale
            // so a tank still gets stunned by a big hit, and a low-HP
            // squishy gets stunned by even a glancing one — same
            // tactical shape as 5e's "below half HP" gate but adjusted
            // for the engine's psychic damage budget.
            if cur_hp > 0 && raw * 2 >= cur_hp.min(max_hp) {
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Stunned,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                }));
            }
        }
        effects
    }
}

pub static INTELLECT_DEVOURER_DEVOUR: LazyLock<IntellectDevour> =
    LazyLock::new(|| IntellectDevour {});

// ─── Xorn ────────────────────────────────────────────────────────────

/// Xorn Claw — STR-based 1d6+STR slashing melee. The three-pawed earth
/// elemental's secondary swing; combines with the bite via `XORN_MULTI`
/// for the canonical "3 claws + 1 bite" Multiattack. Same dice tier as a
/// shortsword swing but typed as natural claws.
pub static XORN_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "xorn claw",
    &["xcl", "xorn-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Xorn Bite — STR-based 3d6+STR piercing melee. The signature heavy hit
/// in the xorn's kit; pairs with the three claws in `XORN_MULTI` so the
/// per-Action damage budget reads as "1 big chomp + 3 small swipes" — a
/// distinctive earth-elemental damage profile vs the chain-of-claws
/// envelope a bulette or owlbear uses.
pub static XORN_BITE: SimpleWeapon = SimpleWeapon::melee(
    "xorn bite",
    &["xb", "xorn-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
);

/// Xorn Multiattack — 3 claw swings + 1 bite per Action. RAW: a xorn
/// makes three claw attacks AND one bite attack on its turn. We model
/// the heterogeneous chain via `CompoundAttack` so the bite's heavier
/// dice tier doesn't get flattened to the claw's d6.
pub static XORN_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "claws + bite",
    parts: vec![(&XORN_CLAW, 3), (&XORN_BITE, 1)],
});

// ─── Oni ─────────────────────────────────────────────────────────────

/// Oni Glaive — STR-based 2d10 slashing melee with reach 2 (10 ft, one
/// tile beyond the standard MELEE_REACH baseline). The oni's marquee
/// swing: a two-handed polearm with the same reach-2 envelope the ogre's
/// greatclub uses, but with double the die size and the typical
/// large-giant STR bump. Pairs with the claw via `ONI_MULTI` so the
/// per-Action damage budget reads as "one big polearm sweep + one
/// follow-up rake".
pub static ONI_GLAIVE: SimpleWeapon = SimpleWeapon::reach_melee(
    "glaive",
    &["gv", "polearm"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Slashing,
    2,
);

/// Oni Claw — STR-based 1d8 slashing melee. The secondary swing in the
/// oni's kit; combines with the glaive via `ONI_MULTI` for the canonical
/// "polearm + claws" Multiattack. Smaller die than the glaive so the
/// compound budget feels like "heavy + light" rather than two equally
/// crushing strikes.
pub static ONI_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "oni claw",
    &["ocl", "oni-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Oni Multiattack — 2 glaive swings per Action. RAW: an oni makes two
/// weapon attacks per turn, one with a glaive and (when within reach)
/// one with claws — we collapse to the double-glaive shape because the
/// claw's smaller die doesn't carry weight against the boss's HP pool
/// in practice, and the glaive's reach-2 lets the oni land both swings
/// even from a step away. Same factor as `BANDIT_CAPTAIN_MULTI` /
/// `GOBLIN_BOSS_MULTI` — `Multiattack` (single sub-attack repeated).
pub static ONI_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double glaive",
    sub_attack: &ONI_GLAIVE,
    count: 2,
});

// ─── Merrow ──────────────────────────────────────────────────────────

/// Merrow Bite — STR-based 1d8 piercing melee. The aquatic ogre's natural
/// chomp; pairs with the harpoon and claws via `MERROW_MULTI` for the
/// "harpoon + claws/bite" Multiattack RAW prescribes.
pub static MERROW_BITE: SimpleWeapon = SimpleWeapon::melee(
    "merrow bite",
    &["mbite", "merrow-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Merrow Claws — STR-based 2d4 slashing melee. The aquatic ogre's webbed
/// talons; alternate offhand pairing for the multi when the harpoon is
/// already committed.
pub static MERROW_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "merrow claws",
    &["mcl", "merrow-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Merrow Harpoon — STR-based 2d6 piercing melee with reach 2 (10 ft). The
/// merrow's signature ranged-melee hybrid: same reach-2 envelope as the
/// ogre's greatclub, with a slightly heavier die since the polearm is
/// pulled back to drag prey closer. RAW also has a ranged thrown form
/// (range 20/60) and a STR-save "pull 20ft" rider on hit; we surface
/// the melee swing only since the engine's reach-2 covers the load-bearing
/// "I can hit you from a tile away" envelope.
pub static MERROW_HARPOON: SimpleWeapon = SimpleWeapon::reach_melee(
    "harpoon",
    &["hrp", "harpoon"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
    2,
);

/// Merrow Multiattack — 1 harpoon swing + 1 bite per Action via
/// `CompoundAttack`. RAW: the merrow makes two attacks — one bite and one
/// claws / harpoon — per turn. We pick the heavier (harpoon) over the
/// claws for the canonical opener since the reach-2 envelope is the
/// merrow's identity hook.
pub static MERROW_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "harpoon + bite",
    parts: vec![(&MERROW_HARPOON, 1), (&MERROW_BITE, 1)],
});

// ─── Giant Crab ──────────────────────────────────────────────────────

/// Giant Crab Claw — STR-based 1d6 bludgeoning melee. The crab's signature
/// pinch; same dice tier as a heavy club but typed as a natural attack so
/// it slots into the beast template without a weapon import. The 5e MM
/// stat block also tags Grappled on a hit (escape DC 11) — we surface
/// only the damage swing for now since the engine's grapple-on-attack
/// rider plumbing is heavier than the CR ⅛ chassis warrants. New CR ⅛
/// beast joining the low-end fillers (Stirge, Hyena, Boar) — cheapest
/// "ambient creature" slot in the upper pool.
pub static GIANT_CRAB_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "crab claw",
    &["pinch", "crabclaw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

// ─── Cloud Giant ─────────────────────────────────────────────────────

/// Cloud Giant Morningstar — STR-based 3d8 piercing melee with reach 2
/// (10 ft). The Cloud Giant's signature swing — same reach-2 envelope as
/// the Stone / Fire Giant clubs but typed as piercing for the spiked
/// morningstar head. Sits one rung above the Fire Giant on the giant
/// ladder (CR 9 with the same 3d8 die but heavier STR, so the per-swing
/// average lands ~3 higher than Fire Giant Greatsword in practice).
pub static CLOUD_GIANT_MORNINGSTAR: SimpleWeapon = SimpleWeapon::reach_melee(
    "cloud morningstar",
    &["cms", "cloud-club"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Piercing,
    2,
);

/// Cloud Giant Rock — STR-based 4d10 bludgeoning thrown rock with reach 24
/// (60 ft). Same chassis as every other giant rock; the Cloud Giant uses
/// the heavier 4d10 die (matching Frost / Fire / Stone Giant rocks) at
/// the CR-9 tier.
pub static CLOUD_GIANT_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "cloud rock",
    &["cgrock", "clrock"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    24,
    16,
);

/// Cloud Giant Multiattack — 2 morningstar swings per Action. Same chassis
/// as the Stone Giant / Frost Giant / Cyclops Multiattack: pure physical
/// thresher boss-tier melee, no rider effects. The cloud giant's RAW spell
/// list (fog cloud, gust of wind, telekinesis at higher tiers) is omitted —
/// the engine doesn't yet surface monster spellcasting picks, so the
/// load-bearing combat clause is the double morningstar swing.
pub static CLOUD_GIANT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "cloud giant multiattack",
    sub_attack: &CLOUD_GIANT_MORNINGSTAR,
    count: 2,
});

// ─── Hezrou ──────────────────────────────────────────────────────────

/// Hezrou Bite — STR-based 2d10 piercing melee. The toad-demon's heavy
/// chomp; pairs with the claws via `HEZROU_MULTI` for the canonical
/// "bite + 2 claws" Multiattack RAW prescribes. Big die tier matches the
/// CR-8 hezrou stat block.
pub static HEZROU_BITE: SimpleWeapon = SimpleWeapon::melee(
    "hezrou bite",
    &["hbite", "hezrou-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Piercing,
);

/// Hezrou Claw — STR-based 2d6 slashing melee. The secondary swing in the
/// hezrou's kit; combines with the bite via `HEZROU_MULTI` for the canonical
/// "bite + 2 claws" Multiattack RAW prescribes.
pub static HEZROU_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "hezrou claw",
    &["hclaw", "hezrou-claw"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Hezrou Multiattack — 1 bite + 2 claws per Action. RAW canonical attack
/// budget: bite first (heaviest die), then a pair of claw rakes. Mixed-
/// limb pattern routes through `CompoundAttack` like Pit Fiend / Roc /
/// Owlbear Multi. The bite's 2d10 + 2× claws 2d6 lands around 26 average
/// damage per round — strong CR-8 melee thresher in line with the
/// Frost Giant's greataxe + Multi profile.
pub static HEZROU_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + claws",
    parts: vec![(&HEZROU_BITE, 1), (&HEZROU_CLAW, 2)],
});

// ─── Gibbering Mouther ───────────────────────────────────────────────

/// Gibbering Mouther Bites — STR-based 5d6 piercing melee. The mouther
/// has dozens of constantly-shifting mouths gnashing at anything in reach;
/// RAW collapses to a single attack roll dealing massive dice damage. No
/// rider — the load-bearing threat is the 5d6 burst on a single hit.
pub static GIBBERING_MOUTHER_BITES: SimpleWeapon = SimpleWeapon::melee(
    "gibbering bites",
    &["gmb", "mouther-bites"],
    AbilityScoreType::Strength,
    Dice::new(5, 6),
    DamageType::Piercing,
);

/// Gibbering Mouther Blinding Spittle — bonus-action ranged action,
/// recharge 5-6. The mouther coughs up a glob of caustic ichor at a tile
/// within 30 ft (12 tiles). Every creature in a 1-tile (5 ft RAW) burst
/// around the splash point makes a DEX save vs the mouther's WIS-based DC
/// (8 + prof + WIS = 11); on fail they're Blinded until the end of their
/// next turn. No damage — the load-bearing threat is the blind. We model
/// the burst via the shared `resolve_burst_save_condition` helper from
/// `action_template`, mirroring Gorgon's Petrifying Breath and other
/// save-or-condition AoEs.
///
/// RAW is single-target with a one-tile splash; we collapse to the burst
/// form because the engine already has a clean save-or-condition burst
/// helper and the splash is the canonical CR-2 control rider for the
/// mouther's kit.
pub struct GibberingMoutherBlindingSpittle {}

impl Action for GibberingMoutherBlindingSpittle {
    fn name(&self) -> &str {
        "blinding spittle"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["spittle", "blind-spit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
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
        vec![Resource::BonusAction]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Standard recharge gate: only available when the d6 came up high
        // enough this turn. The mouther template registers
        // ("blinding spittle", 5) so the spittle recharges on a d6 ≥ 5
        // at turn start — matching RAW's "Recharge 5-6".
        actor_has_recharge(encounter, caster_id, self.name())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::action_template::resolve_burst_save_condition;
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the recharge resource before resolving damage so a
        // mid-resolution failure can't leave the spittle both spent AND
        // condition-applied. Mirrors the BreathWeapon ordering.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge(self.name());
        }
        // DC 10 RAW — MM stat block uses a flat DC 10 DEX save for the
        // blinding rider. Lower than a typical 8 + prof + ability DC so
        // a high-DEX target can shrug it off but most mid-tier mooks
        // catch the blind.
        const DC: i32 = 10;
        encounter.log(format!(
            "  blinding spittle: 1-tile burst (DC {} DEX, Blinded on fail)",
            DC,
        ));
        resolve_burst_save_condition(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            DC,
            Condition::Blinded,
            ConditionTimer::UntilStartOfNextTurn,
        )
    }
}

pub static GIBBERING_MOUTHER_BLINDING_SPITTLE: LazyLock<GibberingMoutherBlindingSpittle> =
    LazyLock::new(|| GibberingMoutherBlindingSpittle {});

// ─── Mummy Lord ──────────────────────────────────────────────────────

/// Mummy Lord Rotting Fist — STR-based melee, 3d6+STR bludgeoning core
/// plus a 6d6 necrotic rider on hit. The lordly variant of `MummyRottingFist`
/// — twice the bludgeoning dice and twice the necrotic rider, matching
/// the CR-15 stat block's heavier punch. Necrotic packet is typed
/// separately so per-type resistance is checked independently and the
/// rider rides through bludgeoning-resistant targets cleanly.
pub static MUMMY_LORD_ROTTING_FIST: WeaponWithRider = WeaponWithRider::melee(
    "lord rotting fist",
    &["lrf", "lord-rot"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    Dice::new(6, 6),
    DamageType::Necrotic,
    "lord rotting fist",
);

/// Mummy Lord Dreadful Glare — Action; targets every enemy within radius 12
/// (60 ft) with line-of-sight to the lord. WIS save vs DC 17 (RAW for CR-15
/// mummy lord) or be Frightened for 10 rounds. Necrotic-immune targets
/// (proxy for undead / fiend) are filtered out per the canonical glare-
/// affects-living convention; the LOS gate keeps a gaze from bending around
/// corners. Lordly variant of `MummyDreadfulGlare`: same shape, longer range
/// and a tougher DC matching the CR-15 stat block.
pub struct MummyLordDreadfulGlare {}

impl Action for MummyLordDreadfulGlare {
    fn name(&self) -> &str {
        "lord dreadful glare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ldg", "lord-glare"]
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
        use crate::actions::action_template::resolve_los_glare_condition;
        const RADIUS: isize = 12;
        const DC: i32 = 17;
        encounter.log(
            "  lord dreadful glare: the mummy lord's hollow gaze freezes the living",
        );
        resolve_los_glare_condition(
            encounter,
            caster_id,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Frightened,
            ConditionTimer::Rounds(10),
            Some(DamageType::Necrotic),
        )
    }
}

pub static MUMMY_LORD_DREADFUL_GLARE: LazyLock<MummyLordDreadfulGlare> =
    LazyLock::new(|| MummyLordDreadfulGlare {});

/// Mummy Lord Multiattack — 1 Rotting Fist + 1 Dreadful Glare per Action.
/// Mixed-schema multi (a melee `SingleActor` attack + a no-args glare burst)
/// — uses `CompoundAttack` whose first sub-attack drives the reach + LOS
/// gate. The rotting fist's melee reach validates first; the dreadful glare
/// fans out from the lord's own location regardless of where the fist
/// landed. RAW per MM: the lord uses Channel Divinity / spellcasting on
/// alternate rounds, neither of which is modeled — the load-bearing
/// per-round threat is the fist + glare combo.
pub static MUMMY_LORD_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "lord fist + glare",
    parts: vec![
        (&MUMMY_LORD_ROTTING_FIST, 1),
        (&*MUMMY_LORD_DREADFUL_GLARE, 1),
    ],
});

// ─── Iron Golem ──────────────────────────────────────────────────────

/// Iron Golem Slam — STR-based 3d8+STR bludgeoning melee. The golem's
/// secondary melee swing (paired with the sword in `IRON_GOLEM_MULTI`).
/// Same shape as `STONE_GOLEM_SLAM` at the heavier CR-16 die tier. Routes
/// through the SimpleWeapon chassis so the multiattack wrapper composes
/// cleanly with no per-creature glue.
pub static IRON_GOLEM_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "iron slam",
    &["islam", "iron-slam"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
);

/// Iron Golem Sword — STR-based 3d10+STR slashing melee, reach 2 (10 ft RAW).
/// The golem's signature: a massive blade swung in a wide arc. Reach-2 lets
/// the golem threaten an extra ring of tiles around its 2×2 Large footprint,
/// matching the RAW "10 ft. reach" stat-block clause. Pairs with the slam
/// for the mixed-limb multiattack.
// RAW: 10 ft reach on the iron-golem blade — one extra tile-gap beyond
// the standard MELEE_REACH so a flanking PC can't kite the golem at
// 2-tile range with impunity.
pub static IRON_GOLEM_SWORD: SimpleWeapon = SimpleWeapon::reach_melee(
    "iron sword",
    &["isword", "iron-blade"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Slashing,
    2,
);

/// Iron Golem Multiattack — 1 sword + 1 slam per Action. Heterogeneous
/// limb pattern routes through `CompoundAttack` (sword first since it's
/// the heavier die and the reach-2 attack — the engine validates reach off
/// the first part, so the multi inherits the longer reach; the slam falls
/// through cleanly when the target is already inside MELEE_REACH).
pub static IRON_GOLEM_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "iron sword + slam",
    parts: vec![(&IRON_GOLEM_SWORD, 1), (&IRON_GOLEM_SLAM, 1)],
});

/// Iron Golem Poison Breath — burst-3 / range-4 of noxious green vapor.
/// 10d8 poison, DC 19 CON, half on save. Recharge 6 (RAW: "Recharge 6"
/// means the breath only refreshes on a d6 of exactly 6 at start of turn).
/// Shares the standard `"breath_weapon"` recharge pool with the dragons —
/// ensures a multi-monster ambush can't double-tap two breath weapons in
/// the same round. Smaller burst than the dragon's range-6 cone (RAW: 15
/// ft cone vs 60 ft cone) at the higher per-die count.
pub static IRON_GOLEM_BREATH: BreathWeapon = BreathWeapon {
    display_name: "iron poison breath",
    aliases: &["ipb", "iron-breath"],
    damage_dice: Dice::new(10, 8),
    damage_type: DamageType::Poison,
    save_ability: AbilityScoreType::Constitution,
    dc: 19,
    radius: 3,
    range: 4,
    recharge_key: "breath_weapon",
};

// ─── Rakshasa ────────────────────────────────────────────────────────

/// Rakshasa Claw — DEX-based melee, 2d6+DEX slashing core plus a 2d10
/// necrotic rider on hit (the cursed touch that drains life force). The
/// rakshasa is a high-DEX fiend (17), so DEX drives both attack and
/// damage despite the slashing damage type — matching the RAW finesse-
/// like "+7 to hit, 2d6+3 slashing" stat block where the +3 mod equals
/// either STR or DEX (we pick DEX as the higher of the two).
pub static RAKSHASA_CLAW: WeaponWithRider = WeaponWithRider::melee(
    "rakshasa claw",
    &["rclaw", "rakshasa-claw"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Slashing,
    Dice::new(2, 10),
    DamageType::Necrotic,
    "rakshasa claw",
);

/// Rakshasa Multiattack — 2 claws per Action. Vanilla single-sub-attack
/// shape (same as Doppelganger Multi / Zombie Multislam); each claw rolls
/// its core slashing hit plus the necrotic rider independently, so a single
/// multi-action against a stationary target can land up to two slashing +
/// two necrotic packets.
pub static RAKSHASA_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "rakshasa multiattack",
    sub_attack: &RAKSHASA_CLAW,
    count: 2,
});

// ─── Hook Horror ─────────────────────────────────────────────────────

/// Hook Horror Hook — STR-based 1d10+STR piercing melee, reach 2 (10 ft RAW).
/// The bird-of-prey body with twin barbed hooks; the longer reach lets the
/// hook horror tag two adjacent rings of tiles around its Large footprint.
/// Paired through `HOOK_HORROR_MULTI` for the canonical double-strike per
/// Action. Vanilla `SimpleWeapon` since there's no rider on the hook hits —
/// the load-bearing per-round threat is the burst from the double swing,
/// not any per-hit condition / typed-damage payload.
// 10 ft RAW = reach 2 on this 2.5 ft grid.
pub static HOOK_HORROR_HOOK: SimpleWeapon = SimpleWeapon::reach_melee(
    "hook horror hook",
    &["hhh", "hook"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    2,
);

/// Hook Horror Multiattack — 2 hook swings per Action. Vanilla
/// single-sub-attack shape; each swing rolls its own d20 + STR vs AC and
/// the burst sits squarely in the CR-3 band when both hooks connect.
pub static HOOK_HORROR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "hook horror multiattack",
    sub_attack: &HOOK_HORROR_HOOK,
    count: 2,
});

// ─── Dragon Turtle ───────────────────────────────────────────────────

/// Dragon Turtle Bite — STR-based 3d12+STR piercing melee, reach 3 (15 ft
/// RAW). The dragon turtle's massive snapping jaw; heaviest die-count of any
/// melee bite in the engine after the Tarrasque's 4d12. Reach-3 lets the
/// turtle threaten well past its 4×4 Gargantuan footprint so retreating
/// melee PCs eat opportunity attacks. Vanilla `SimpleWeapon` — the load-
/// bearing threat is the raw damage, not a rider.
// 15 ft RAW = reach 3 on this 2.5 ft grid.
pub static DRAGON_TURTLE_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "dragon turtle bite",
    &["dt-bite", "turtle-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 12),
    DamageType::Piercing,
    3,
);

/// Dragon Turtle Claw — STR-based 2d8+STR slashing melee, reach 2 (10 ft
/// RAW). The supplementary swing in the dragon turtle's kit; paired with
/// the bite in the multi for the canonical "bite + 2 claws" Multiattack
/// RAW prescribes. Heterogeneous-reach with the bite (reach 3) so the
/// `CompoundAttack` wrapper validates off the heaviest-reach first
/// sub-attack and the claws fall through cleanly when the target is
/// closer.
pub static DRAGON_TURTLE_CLAW: SimpleWeapon = SimpleWeapon::reach_melee(
    "dragon turtle claw",
    &["dt-claw", "turtle-claw"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
    2,
);

/// Dragon Turtle Multiattack — 1 bite + 2 claws per Action. Mixed-limb
/// `CompoundAttack` (bite first to drive the reach-3 envelope so the
/// multi can land on a target a full tile beyond the claw reach). Matches
/// the canonical MM "Multiattack: The dragon turtle makes three attacks:
/// one with its bite and two with its claws" clause cleanly.
pub static DRAGON_TURTLE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "dragon turtle multiattack",
    parts: vec![(&DRAGON_TURTLE_BITE, 1), (&DRAGON_TURTLE_CLAW, 2)],
});

/// Dragon Turtle Steam Breath — burst-3 / range-4 of scalding vapor.
/// 12d6 fire, DC 18 CON, half on save. Recharge 5-6 via the shared
/// `"breath_weapon"` pool so the dragon turtle can't double-tap with a
/// second breath option (it has none, but the shared key keeps the
/// start-of-turn roller uniform). The RAW per-die count is 15d6, dropped
/// to 12d6 here so the breath profile matches the existing CR-17
/// dragons' 12d6 chassis — the same damage band the engine's other
/// CR-17 boss breaths already calibrate around, and the fire-typing
/// keeps fire-resistant targets (the dragons themselves) immune-to-
/// resistance scaling.
///
/// CON save (inhaled scalding vapor) rather than the DEX save the
/// elemental dragon breaths route through — matches the RAW clause
/// where the steam permeates lungs / armor cracks rather than dodging
/// in flight. Same shape as the Iron Golem's poison breath / the Adult
/// Black Dragon's acid breath — all CON-save breaths share the chassis.
pub static DRAGON_TURTLE_STEAM_BREATH: BreathWeapon = BreathWeapon {
    display_name: "steam breath",
    aliases: &["sb-steam", "steam"],
    damage_dice: Dice::new(12, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Constitution,
    dc: 18,
    radius: 3,
    range: 4,
    recharge_key: "breath_weapon",
};

// ─── Kraken ──────────────────────────────────────────────────────────

/// Kraken Tentacle — STR-based 3d6+STR bludgeoning melee, reach 6 (30 ft
/// RAW). The kraken's signature reach: tentacle whips out across half the
/// arena from its Gargantuan body. Paired through `KRAKEN_MULTI` for the
/// canonical 3-tentacle Multiattack RAW prescribes. Vanilla `SimpleWeapon`
/// — RAW pairs the tentacle hit with a Grappled rider on a STR-vs-Athletics
/// contest, but the engine doesn't yet surface contested grapple rolls
/// (only the spell-cast `Grappled` install lane); the load-bearing per-
/// round threat is the burst from triple 30 ft reach swings, which by
/// itself ranks among the heaviest melee profiles in the engine.
// 30 ft RAW = reach 6 on this 2.5 ft grid.
pub static KRAKEN_TENTACLE: SimpleWeapon = SimpleWeapon::reach_melee(
    "kraken tentacle",
    &["kt", "tentacle-k"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    6,
);

/// Kraken Multiattack — 3 tentacle swings per Action. Vanilla
/// single-sub-attack shape; each tentacle rolls its own d20 + STR vs AC
/// and the burst lands a brutal 3×(3d6+STR) on a single stationary
/// target. The triple-swing chassis at reach 6 makes the kraken the
/// engine's heaviest reach-melee thresher — even a fleeing PC at the
/// kraken's range envelope eats a full burst.
pub static KRAKEN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "kraken multiattack",
    sub_attack: &KRAKEN_TENTACLE,
    count: 3,
});

/// Kraken Lightning Storm — Action: every enemy within radius 12 (60 ft)
/// of the kraken with line-of-sight makes a DC 23 DEX save; failures take
/// 4d10 lightning, passes take half. Recharge 5-6 via the shared
/// `"breath_weapon"` pool — RAW frames this as a "1/Day" or "Recharge
/// after a short or long rest" depending on the printing, so we use the
/// standard 5-6 recharge chassis to fold it into the existing start-of-
/// turn refresher.
///
/// RAW targets up to three creatures within 120 ft (24 tiles here); we
/// collapse to a 12-tile radius AoE save-burst because:
/// 1. The "pick three within long range" semantics doesn't fit the
///    existing TargetingSchema set — every other multi-target ability in
///    the engine is either a burst (Lightning Bolt, breath weapons) or
///    a single-target spam (Magic Missile-style locked picks).
/// 2. A 12-tile radius envelope from the kraken's footprint covers ~60 ft
///    around it — roughly equivalent in practical hit count for the
///    typical 40×20 map to the RAW "any three within 120 ft" picker.
/// 3. Save-burst chassis routes cleanly through `resolve_burst_save_damage`,
///    keeping the per-tier damage / save math consistent with the rest of
///    the AoE lanes.
///
/// The lightning is shared (single damage roll applied to every target via
/// `resolve_burst_save_damage`) — matches the RAW per-target hit since
/// each bolt deals identical 4d10. Lightning-resistant targets halve it
/// through the standard damage pipeline; the kraken's own lightning
/// immunity keeps the burst safe to self-center even though we don't
/// model the "kraken picks safe tile" picker.
pub struct KrakenLightningStorm {}

impl Action for KrakenLightningStorm {
    fn name(&self) -> &str {
        "lightning storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ls-k", "kraken-storm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Standard recharge gate via the shared `"breath_weapon"` pool —
        // mirrors the BreathWeapon / IronGolemBreath / GorgonBreath
        // custom_validate shape.
        actor_has_recharge(encounter, caster_id, "breath_weapon")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        const RADIUS: isize = 12;
        const DC: i32 = 23;
        // Spend the recharge resource before resolving damage so a
        // mid-resolution failure can't leave the storm both spent AND
        // damage-applied. Mirrors the BreathWeapon ordering.
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        let raw = encounter.roll(&Dice::new(4, 10));
        encounter.log(format!(
            "  lightning storm: 4d10({}) = {} shared lightning (DC {} DEX, half on save)",
            raw, raw, DC,
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            caster_loc,
            RADIUS,
            AbilityScoreType::Dexterity,
            DC,
            raw,
            DamageType::Lightning,
        )
    }
}

pub static KRAKEN_LIGHTNING_STORM: LazyLock<KrakenLightningStorm> =
    LazyLock::new(|| KrakenLightningStorm {});

// ─── Helmed Horror ───────────────────────────────────────────────────

/// Helmed Horror Longsword — STR-based 2d8+STR slashing melee, reach 1.
/// The animated armor's enchanted longsword swing. Vanilla `SimpleWeapon`
/// — RAW pairs the construct's two longsword swings per Multiattack with
/// no per-hit rider; the load-bearing combat threat is the spell-immunity
/// envelope (modeled as flat Magic Resistance + force / necrotic / poison
/// damage immunity at the template level), not a damage rider.
pub static HELMED_HORROR_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "helmed horror longsword",
    &["hhl", "hh-sword"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

/// Helmed Horror Multiattack — 2 longsword swings per Action. Vanilla
/// single-sub-attack shape (same as Knight Greatsword / Rakshasa Claw
/// multi); each swing rolls its own d20 + STR vs AC for the canonical
/// CR-4 construct double-strike profile.
pub static HELMED_HORROR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "helmed horror multiattack",
    sub_attack: &HELMED_HORROR_LONGSWORD,
    count: 2,
});

// ─── Pixie ───────────────────────────────────────────────────────────

/// Pixie Sleep Dust — burst-1 (5 ft) save-burst centered on a chosen tile
/// within 6 tiles (30 ft). Every creature in the burst makes a DC 12 WIS
/// save; on fail they fall Asleep for 10 rounds (1 minute RAW). Mirrors
/// the chosen-tile + small radius shape of the bullette's earth tremor
/// or the orcish javelin's drop — single Action cost, no recharge
/// (RAW: 1/day, but we use a tick-down install instead of a per-rest
/// resource so the pixie has a hook in any encounter; the action's
/// once-per-target Asleep install plus sleep's auto-wake-on-damage rider
/// already self-limits abuse).
///
/// Sleep-immune creatures (constructs / undead / elves / etc. via the
/// existing `dynamic_immunity_to` chokepoint) shrug it off without
/// rolling — short-circuit via `effectively_immune_to_condition`.
pub struct PixieSleepDust {}

impl Action for PixieSleepDust {
    fn name(&self) -> &str {
        "sleep dust"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sd", "dust"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const DC: i32 = 12;
        const RADIUS: isize = 1;
        encounter.log(format!(
            "  sleep dust: 5ft burst (DC {} WIS, asleep on fail)",
            DC
        ));
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            point,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Asleep,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static PIXIE_SLEEP_DUST: LazyLock<PixieSleepDust> = LazyLock::new(|| PixieSleepDust {});

// ─── Androsphinx ─────────────────────────────────────────────────────

/// Androsphinx Claw — STR-based 2d8+STR slashing melee. The lion-bodied
/// guardian's signature swing; reach 1 (5 ft RAW). Vanilla `SimpleWeapon`
/// — the load-bearing per-round threat is the burst from 2 claws plus a
/// Roar via `ANDROSPHINX_MULTI`, not a per-hit rider. RAW also tags the
/// claws as magical for the "non-magical resistance bypass" lane; this
/// engine doesn't track magic-weapon typing on attacker side, so the
/// magical-weapon clause collapses to a flat "always counts as magical"
/// without modeling.
pub static ANDROSPHINX_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "androsphinx claw",
    &["asc", "sphinx-claw"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

/// Androsphinx Roar — every enemy within radius 10 (50 ft) with line-of-
/// sight makes a DC 18 WIS save or is Frightened for 10 rounds. Recharge
/// 5-6 via the shared `"breath_weapon"` pool — RAW frames the three Roars
/// (First / Second / Third) as a per-day escalating-effect series; we
/// collapse the three-tier ladder to the Frightened-on-fail base effect
/// since the engine's recharge chassis fits cleanly into the start-of-turn
/// refresher without per-day bookkeeping.
///
/// LOS-gated: a Roar travels by sound but RAW requires the target to hear
/// the sphinx; we approximate by routing through
/// `resolve_los_glare_condition` so an enemy behind a heavy stone wall
/// shrugs off the burst (matches Mummy Lord Dreadful Glare's shape). The
/// `skip_immune_to_damage` arg is `None` — Frightened immunity is checked
/// at the install site by `add_condition` rather than the damage-type
/// proxy used for undead vs glare.
pub struct AndrosphinxRoar {}

impl Action for AndrosphinxRoar {
    fn name(&self) -> &str {
        "roar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["roar-s", "sphinx-roar"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "breath_weapon")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        const RADIUS: isize = 10;
        const DC: i32 = 18;
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        encounter.log(format!(
            "  roar: 50ft burst (DC {} WIS, frightened on fail)",
            DC
        ));
        crate::actions::action_template::resolve_los_glare_condition(
            encounter,
            caster_id,
            RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Frightened,
            ConditionTimer::Rounds(10),
            None,
        )
    }
}

pub static ANDROSPHINX_ROAR: LazyLock<AndrosphinxRoar> = LazyLock::new(|| AndrosphinxRoar {});

/// Androsphinx Multiattack — 2 claw swings per Action. Vanilla single-
/// sub-attack shape mirroring Hook Horror / Rakshasa multis. The Roar
/// is a separate action so the sphinx can either burst-burst with two
/// claws on a single target OR fire a Roar at the broader battlefield
/// (RAW: the sphinx's full action profile is "2 claws AND uses Roar" per
/// turn, but the engine's recharge chassis caps Roar by `"breath_weapon"`
/// so binding them to a single Multi would double-spend the recharge
/// resource and silently zero out the claws on subsequent turns).
pub static ANDROSPHINX_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "androsphinx multiattack",
    sub_attack: &ANDROSPHINX_CLAW,
    count: 2,
});

// ─── Unicorn ─────────────────────────────────────────────────────────

/// Unicorn Hooves — STR-based 2d6+STR bludgeoning melee. The kicking
/// half of the unicorn's multi; pairs with the horn for the standard
/// "kick + gore" double-tap. Vanilla `SimpleWeapon` — no rider effects,
/// the load-bearing combat clauses live on the horn (which carries the
/// charge rider) and the Healing Touch action.
pub static UNICORN_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "unicorn hooves",
    &["uh", "hooves-u"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Unicorn Horn — STR-based 1d8+STR piercing melee. The piercing half
/// of the unicorn's multi — paired with the hooves in
/// `UNICORN_MULTI`. RAW's **Charge** rider — +2d8 piercing and a STR
/// save vs Prone when the unicorn covers 20+ feet in a straight line
/// first — rides this weapon through `CHARGE_RIDERS` in
/// `engine::attack`, which measures the run off the attacker's
/// turn-start tile. The horn stays the high-damage limb and the hooves
/// the low-damage one; the charge is what makes closing the distance
/// worth more than standing and swinging.
pub static UNICORN_HORN: SimpleWeapon = SimpleWeapon::melee(
    "unicorn horn",
    &["horn", "uhorn"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Unicorn Multiattack — 1 hoof kick + 1 horn gore per Action. RAW: the
/// unicorn makes two attacks (one with its hooves and one with its horn).
/// Heterogeneous limbs combine cleanly through `CompoundAttack` so each
/// limb keeps its own dice tier — the horn doesn't share the hooves' 2d6
/// pool and vice versa.
pub static UNICORN_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "hooves + horn",
    parts: vec![(&UNICORN_HOOVES, 1), (&UNICORN_HORN, 1)],
});

/// Unicorn Healing Touch — single-target ally heal. The unicorn touches
/// one creature within melee reach and restores 3d8+CHA HP, cures every
/// condition the holder has, and breaks any charm / curse on them. We
/// model the load-bearing half (the HP heal) — the cure-conditions
/// half rides through the existing recharge chassis to limit
/// over-use. RAW is "3/day" — the engine doesn't track per-day pools,
/// so we approximate via the `"healing_touch"` recharge key (recharge
/// 5-6 on a d6 at start-of-turn). One ally target only; the unicorn
/// chooses based on AI heuristics (heal a wounded teammate).
pub struct UnicornHealingTouch {}

impl Action for UnicornHealingTouch {
    fn name(&self) -> &str {
        "healing touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ht", "touch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !caster.is_recharge_available("healing_touch") {
            return false;
        }
        // Reject hostile targets — the touch only restores allies. The
        // ally check lives here (not just at side_effects) so the AI's
        // picker doesn't surface enemies as legal targets.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter.actors_allied(caster_id, target_id)
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
        let cha_mod = caster.ability_modifier(AbilityScoreType::Charisma);
        let dice = Dice::new(3, 8);
        let raw = encounter.roll(&dice) as i32;
        let amount = (raw + cha_mod).max(1) as u32;
        // Burn the recharge so the touch can't fire again until the d6
        // refresher lands a 5-6 at start-of-turn. Mirrors the breath-
        // weapon recharge chassis used by Androsphinx Roar / dragons.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("healing_touch");
        }
        encounter.log(format!(
            "  healing touch: {}({}){:+} = {} HP",
            dice, raw, cha_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static UNICORN_HEALING_TOUCH: LazyLock<UnicornHealingTouch> =
    LazyLock::new(|| UnicornHealingTouch {});

// ─── Drider ──────────────────────────────────────────────────────────

/// Drider Longsword — STR-based 1d8+STR slashing melee. The melee half
/// of the drider's offensive kit; the longbow handles the ranged lane.
/// Vanilla `SimpleWeapon` — no rider effects, just a sturdy mid-CR melee
/// swing. The drider chassis combines two longsword swings + one bite
/// per Action via `DRIDER_MULTI`.
pub static DRIDER_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "drider longsword",
    &["dls", "drider-ls"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Drider Longbow — DEX-based 1d8+DEX piercing ranged. The ranged half
/// of the drider's kit; pairs with the bite for a hit-and-run profile
/// at mid-range. Range 12 tiles (≈ 60ft normal, well under the 80/320
/// RAW long-range threshold; the drider's longbow stat reads "+5 to
/// hit, range 150/600" — the engine caps reach at 20 for indoor maps).
pub static DRIDER_LONGBOW: SimpleWeapon = SimpleWeapon::ranged(
    "drider longbow",
    &["dlb", "drider-bow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Piercing,
    20,
    12,
);

/// Drider Bite — STR-based 1d4+STR piercing melee with a CON save (DC 13)
/// for 4d8 poison rider on fail (half on save per RAW). Same "weapon +
/// save-rider" shape as Spider Bite but the rider scales much higher
/// (4d8 vs 2d4) and the save is more severe. The drider's bite is the
/// signature spider-half lethality — even on a save the target eats 2d8
/// poison damage.
///
/// We diverge slightly from the per-save shape used by `save_or_damage_rider`
/// (which is binary: full damage on fail, none on save). The drider's
/// poison RAW is "4d8 on fail, half on success", so we model the save
/// gate inline with a `SaveDamagePolicy::HalfOnSave` resolution.
pub struct DriderBite {}

impl Action for DriderBite {
    fn name(&self) -> &str {
        "drider bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dbite", "drider-bite"]
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
        use crate::engine::saves::SaveDamagePolicy;
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
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        // Poison rider applies only on a hit — bail if the bite missed.
        if effects.is_empty() {
            return effects;
        }
        // 4d8 on fail, half (2d8 average) on save — uses the standard
        // SaveDamagePolicy::HalfOnSave for the per-save split.
        let dice = Dice::new(4, 8);
        let raw = encounter.roll(&dice);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        let amount = SaveDamagePolicy::HalfOnSave.apply(raw, save.passed());
        if amount == 0 {
            return effects;
        }
        encounter.log(format!(
            "  drider bite poison: {}({}) = {} poison ({})",
            dice,
            raw,
            amount,
            if save.passed() { "save" } else { "fail" }
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount,
            damage_type: DamageType::Poison,
        }));
        effects
    }
}

pub static DRIDER_BITE: LazyLock<DriderBite> = LazyLock::new(|| DriderBite {});

/// Drider Multiattack — 2 longsword swings + 1 bite per Action. RAW: the
/// drider makes 3 attacks, using its longsword twice and its bite once.
/// Heterogeneous limbs combine cleanly through `CompoundAttack`. The
/// longbow lane is a *separate* standalone action — RAW lets the drider
/// substitute its melee attacks with longbow shots, but the engine's
/// CompoundAttack chassis can't model "either A or B"; the AI picks
/// between the melee multi and a standalone longbow shot based on
/// position (melee in reach → multi, ranged → longbow).
pub static DRIDER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "drider multiattack",
    parts: vec![(&DRIDER_LONGSWORD, 2), (&*DRIDER_BITE, 1)],
});

// ─── Sea Hag ─────────────────────────────────────────────────────────

/// Sea Hag Claws — STR-based 1d4+STR slashing melee. The hag's signature
/// rending swing; the Death Glare is the load-bearing fear-mortality
/// lane and the claws are the steady damage tap. Vanilla `SimpleWeapon`
/// — no rider effects.
pub static SEA_HAG_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "sea hag claws",
    &["shc", "hag-claws"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Slashing,
);

/// Sea Hag Death Glare — single-target WIS save (DC 11) on a target
/// within 30 ft (12 tiles). On fail the target takes 6d6 psychic damage
/// (RAW: reduces a Frightened target to 0 HP outright — we approximate
/// via a heavy psychic hit since the engine's Frightened condition
/// tracking doesn't fold cleanly into a binary kill gate). On save the
/// effect fizzles entirely (NoneOnSave). Requires line-of-sight — a
/// glare can't bend around walls.
///
/// We deviate from the strict RAW "reduce Frightened target to 0 HP" gate
/// because (a) the engine doesn't yet expose a "would-be-killed-by"
/// helper at the side-effect layer, and (b) routing through the standard
/// damage pipeline lets immunity / resistance / temp HP / Death Ward
/// all fire correctly. The 6d6 max damage maps to the hag's CR-2 power
/// curve — heavy single-target burst but not auto-kill.
pub struct SeaHagDeathGlare {}

impl Action for SeaHagDeathGlare {
    fn name(&self) -> &str {
        "death glare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dg", "glare"]
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
        // DC 11 is RAW for the sea hag — flat number, not derived from
        // the caster's stats. The 5e hag stat block ties its save DCs
        // to its CHA modifier; with CHA 13 (+1) and prof +2, the formula
        // would yield 11, so the hardcode here matches the formula's
        // output for the canonical sea hag stat block. We sanity-check
        // the caster exists (gone-mid-action guard) and bail early on
        // a vanished caster rather than trying to fire a spell from
        // nowhere.
        if !encounter.actors.contains_key(&caster_id) {
            return Vec::new();
        }
        const DC: i32 = 11;
        const DICE: Dice = Dice::new(6, 6);
        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, DC);
        if save.passed() {
            encounter.log("  death glare: target shrugs off the soul-rending stare");
            return Vec::new();
        }
        let amount = encounter.roll(&DICE);
        encounter.log(format!(
            "  death glare: {}({}) = {} psychic",
            DICE, amount, amount
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount,
            damage_type: DamageType::Psychic,
        })]
    }
}

pub static SEA_HAG_DEATH_GLARE: LazyLock<SeaHagDeathGlare> =
    LazyLock::new(|| SeaHagDeathGlare {});

// ─── Night Hag ───────────────────────────────────────────────────────

/// Night Hag Claws — STR-based 2d8+STR slashing melee. The hag's
/// signature rending swing in her hag form. Vanilla `SimpleWeapon` —
/// the load-bearing identity is the Magic Resistance + B/P/S resistance
/// envelope plus the multi (2 claws / Action), not any per-hit rider.
pub static NIGHT_HAG_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "night hag claws",
    &["nhc", "hag-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

/// Night Hag Multiattack — 2 claw swings per Action. RAW: "The hag makes
/// two attacks with its claws." Same single-sub shape as Doppelganger /
/// Werewolf / Rakshasa multis.
pub static NIGHT_HAG_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "night hag multiattack",
    sub_attack: &NIGHT_HAG_CLAWS,
    count: 2,
});

// ─── Spirit Naga ─────────────────────────────────────────────────────

/// Spirit Naga Bite — STR-based 1d6+STR piercing melee at reach 2 (10 ft
/// RAW for the naga's coiled-strike posture) with a heavy CON-save poison
/// rider (DC 13, 7d8 fire-and-forget: full on fail, half on save). The
/// poison rider is the load-bearing per-round threat — the d6 base hit is
/// almost cosmetic next to the 7d8 average (~31) poison packet.
///
/// We use `SaveDamagePolicy::HalfOnSave` to land "full on fail, half on
/// save" cleanly — same shape as Drider Bite (different scale: 4d8 there
/// vs 7d8 here). The save rolls AFTER the bite-attack roll lands so
/// `damage == 0` (miss) short-circuits the poison entirely — RAW gates
/// the poison on a hit, so a missed bite shouldn't still poison through.
pub struct SpiritNagaBite {}

impl Action for SpiritNagaBite {
    fn name(&self) -> &str {
        "naga bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nbite", "naga-bite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10 ft RAW = reach 2 on the 2.5 ft grid (the naga's long coiled body
        // lets it strike one tile further than a standard medium attacker).
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
        use crate::engine::saves::SaveDamagePolicy;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Reach-2 (10 ft) STR/STR swing — `weapon_swing_with_damage`
        // collapses the caster-mod / attack-mod / AttackParams boilerplate
        // and returns `damage` so the poison rider can gate cleanly on
        // a hit.
        let (mut effects, damage) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            "naga bite",
            AbilityScoreType::Strength,
            Dice::new(1, 6),
            DamageType::Piercing,
            true,
            None,
        );
        // Poison rider only on a hit — bail out if the bite missed.
        if damage == 0 {
            return effects;
        }
        // 7d8 on fail, half on save — `SaveDamagePolicy::HalfOnSave` lands
        // both the full / half paths cleanly. Roll the dice once and share
        // between the two outcomes (matches RAW shared-roll semantics).
        let dice = Dice::new(7, 8);
        let raw = encounter.roll(&dice);
        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, 13);
        let amount = SaveDamagePolicy::HalfOnSave.apply(raw, save.passed());
        if amount == 0 {
            return effects;
        }
        encounter.log(format!(
            "  naga venom: {}({}) = {} poison ({})",
            dice,
            raw,
            amount,
            if save.passed() { "half on save" } else { "full on fail" }
        ));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount,
            damage_type: DamageType::Poison,
        }));
        effects
    }
}

pub static SPIRIT_NAGA_BITE: LazyLock<SpiritNagaBite> =
    LazyLock::new(|| SpiritNagaBite {});

// ─── Otyugh ──────────────────────────────────────────────────────────

/// Otyugh Bite — STR-based 2d8+STR piercing melee with a CON save
/// (DC 15) or Poisoned on hit. The bite is the chunkier of the otyugh's
/// two attack lanes (heavier dice + the disease rider). The Poisoned
/// condition replaces RAW's "disease that lasts until cured" — the engine
/// doesn't model long-term diseases, so we use a multi-round Poisoned
/// timer (Rounds(5)) as the closest mechanical equivalent. The save is
/// rolled once per bite; multiple bites in the same multi each roll
/// independently (matching the per-attack save shape of every other
/// "weapon hit + save" rider in the codebase). Routes through the shared
/// `WeaponWithSaveCondition` chassis so the save + condition install
/// lives at one chokepoint alongside the bearded-devil-beard / horned-
/// devil-tail / constrictor / giant-octopus cohort.
pub static OTYUGH_BITE: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "otyugh bite",
    &["obite", "otyugh-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    15,
    Condition::Poisoned,
    ConditionTimer::Rounds(5),
    "otyugh disease",
);

/// Otyugh Tentacle — STR-based 1d8+STR bludgeoning + 1d8 piercing rider
/// melee at reach 2 (10 ft RAW for the otyugh's prehensile tentacles).
/// The mixed-damage profile (bludgeon from the slap + pierce from the
/// barbed hooks) means a target with resistance to one type still eats
/// the other half. On a hit the target is also Restrained for 1 round
/// — approximation of RAW's "grappled + restrained" clause. The engine
/// doesn't track per-grappler grapple links, so we use a one-round
/// timer: long enough to lock the target down for one turn but short
/// enough that the otyugh's next round of tentacles can re-apply it.
pub struct OtyughTentacle {}

impl Action for OtyughTentacle {
    fn name(&self) -> &str {
        "otyugh tentacle"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["otentacle", "otyugh-tentacle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10 ft RAW = reach 2 — the prehensile tentacles extend past the
        // otyugh's Large footprint.
        Some(2)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Piercing]
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
        // Reach-2 (10 ft) STR/STR swing — `weapon_swing_with_damage`
        // collapses the caster-mod / attack-mod / AttackParams boilerplate
        // and returns `damage` so the piercing-barb + grapple riders gate
        // cleanly on a hit.
        let (mut effects, damage) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            "otyugh tentacle",
            AbilityScoreType::Strength,
            Dice::new(1, 8),
            DamageType::Bludgeoning,
            true,
            None,
        );
        // Tentacle-barb piercing rider + Restrained grapple only on a hit.
        if damage == 0 {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 8),
            DamageType::Piercing,
            "otyugh barbs",
            &mut effects,
        );
        encounter.log("  otyugh tentacle grapples the target");
        effects.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Restrained,
            timer: ConditionTimer::Rounds(1),
        }));
        effects
    }
}

pub static OTYUGH_TENTACLE: LazyLock<OtyughTentacle> =
    LazyLock::new(|| OtyughTentacle {});

/// Otyugh Multiattack — 1 bite + 2 tentacles per Action. CompoundAttack
/// because the limbs are heterogeneous (different damage types, different
/// reach — but both melee, so the wrapper inherits the longer tentacle
/// reach from the first part). We declare the tentacle first so the
/// wrapper's reach check uses reach 2 (10 ft) — the bite at reach 1
/// will still land cleanly because the target is necessarily within the
/// tentacle's reach envelope.
pub static OTYUGH_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "otyugh multiattack",
    parts: vec![(&*OTYUGH_TENTACLE, 2), (&OTYUGH_BITE, 1)],
});

// ─── Sprite ──────────────────────────────────────────────────────────

/// Sprite Shortsword — DEX-based 1 piercing melee. RAW the sprite's
/// shortsword does a flat 1 damage (the tiny fey has STR 3, and the
/// d6 is replaced by the size-restricted minimum). We model the flat 1
/// via a 1d1 placeholder die because the engine's `Dice` rolls a uniform
/// `[1, n]`; rolling on a 1-sided die always returns 1, matching RAW.
/// Vanilla `SimpleWeapon::melee` — no per-hit rider; the load-bearing
/// threat is the sleep-arrow on the bow, not the melee jab.
pub static SPRITE_SHORTSWORD: SimpleWeapon = SimpleWeapon::melee(
    "sprite shortsword",
    &["ssw", "sprite-sword"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Piercing,
);

/// Sprite Longbow — DEX-based ranged arrow with a sleep-poison rider. On
/// a confirmed hit the target makes a CON save (DC 10); on fail they fall
/// Asleep for 10 rounds (RAW: 1 minute). The base arrow's damage is a
/// flat 1 piercing (same size-restricted die as the shortsword) plus the
/// optional sleep-poison save. The arrow at reach 8 (40 ft RAW) is the
/// sprite's load-bearing tactical clause — opens a fight by knocking out
/// the heaviest melee threat before they close.
///
/// The save shape mirrors Imp Sting / Spider Bite: roll the attack first,
/// gate the save rider on a confirmed hit, and route the Asleep install
/// through the standard `dynamic_immunity_to` chokepoint (constructs,
/// undead, elves all shrug it off automatically). The 10-round timer is
/// the standard "1 minute = 10 rounds" mapping.
pub struct SpriteLongbow {}

impl Action for SpriteLongbow {
    fn name(&self) -> &str {
        "sprite longbow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["slb", "sprite-bow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 40 ft RAW = reach 8 on this 2.5 ft grid (the sprite is tiny but
        // the bow's range is fixed). Long-range to 160 ft (32 tiles) is
        // omitted — the engine routes long-range disadvantage through
        // `normal_range`, but the sprite's tactical envelope is the
        // 8-tile sleep-arrow opener, not a sniper rifle from across the
        // map. Future tuning could promote this to a SimpleWeapon-style
        // (normal 8, reach 32) shape.
        Some(8)
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
            Dice::new(1, 1),
            DamageType::Piercing,
            false,
        );
        // Save rider only on a confirmed hit. Asleep-immune targets
        // (constructs, undead, elves via Fey Ancestry) auto-skip the
        // install at `add_condition` — but routing the save through
        // `save_or_condition_rider` still rolls it; we short-circuit
        // upstream so an immune target doesn't waste a roll.
        if effects.is_empty() {
            return effects;
        }
        let Some(target) = encounter.actors.get(&target_id) else {
            return effects;
        };
        if target.effectively_immune_to_condition(Condition::Asleep) {
            encounter.log("  sleep arrow: target is immune to sleep");
            return effects;
        }
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            10,
            Condition::Asleep,
            ConditionTimer::Rounds(10),
            "sleep arrow",
            &mut effects,
        );
        effects
    }
}

pub static SPRITE_LONGBOW: LazyLock<SpriteLongbow> = LazyLock::new(|| SpriteLongbow {});

// ─── Death Dog ───────────────────────────────────────────────────────

/// Death Dog Bite — STR-based 1d6+STR piercing melee with a disease save
/// rider. On a confirmed hit the target makes a CON save (DC 12); on fail
/// they're Poisoned (RAW: "diseased" until cured — we use Poisoned for
/// Rounds(10) as the closest mechanical proxy, same convention as Otyugh
/// Bite's disease save). The death dog is the two-headed canine of the
/// underdeep; the dual-bite is what makes its CR-1 burst hit twice as
/// often as a vanilla wolf — see `DEATH_DOG_MULTI` for the wrapper.
pub struct DeathDogBite {}

impl Action for DeathDogBite {
    fn name(&self) -> &str {
        "death dog bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ddb", "death-dog-bite"]
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
            self.name(),
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(1, 6),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // 5e RAW: CON 12, on fail "diseased" until cured. The engine
        // doesn't model long-term diseases, so we install Poisoned for
        // 10 rounds — long enough to feel like a real debuff in a
        // protracted fight without the "until cured" indefinite tag
        // (matches Otyugh Bite's disease-save convention).
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            12,
            Condition::Poisoned,
            ConditionTimer::Rounds(10),
            "death dog disease",
            &mut effects,
        );
        effects
    }
}

pub static DEATH_DOG_BITE: LazyLock<DeathDogBite> = LazyLock::new(|| DeathDogBite {});

/// Death Dog Multiattack — 2 bite swings per Action. The two-headed
/// canine's signature: each head rolls its own d20 + STR vs AC, so the
/// per-Action damage budget is ~2 × (1d6 + STR) plus two independent rolls
/// against the disease save. Same single-sub shape as Hook Horror /
/// Iron Golem / Helmed Horror multis.
pub static DEATH_DOG_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "death dog multiattack",
    sub_attack: &*DEATH_DOG_BITE,
    count: 2,
});

// ─── Magmin ──────────────────────────────────────────────────────────

/// Magmin Touch — DEX-based 1d6+DEX fire melee. Pure fire damage (no
/// physical component); on a confirmed hit the target is Burning for
/// 3 rounds (RAW: "the magmin's body bursts into flames" — we use the
/// existing Burning condition for the persistent fire DOT). The magmin
/// is a CR ½ fire elemental, so the touch's fire-typed damage threads
/// neatly through fire-resistant / immune targets via the standard
/// damage pipeline.
///
/// The Burning install at `add_condition` checks fire immunity (via
/// `dynamic_immunity_to` and the typed-immunity condition gate); we
/// short-circuit upstream so a fire-immune target doesn't waste log
/// lines on the install attempt.
pub struct MagminTouch {}

impl Action for MagminTouch {
    fn name(&self) -> &str {
        "magmin touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mt", "magmin-touch", "fiery-touch"]
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
            DamageType::Fire,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Burning DOT install — fire-immune targets shrug it off at the
        // install site, but we short-circuit upstream so the log doesn't
        // record a wasted install attempt.
        let Some(target) = encounter.actors.get(&target_id) else {
            return effects;
        };
        if target.effectively_immune_to_condition(Condition::Burning) {
            return effects;
        }
        encounter.log("  magmin ignites the target");
        effects.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Burning,
            timer: ConditionTimer::Rounds(3),
        }));
        effects
    }
}

pub static MAGMIN_TOUCH: LazyLock<MagminTouch> = LazyLock::new(|| MagminTouch {});

// ─── Galeb Duhr ──────────────────────────────────────────────────────

/// Galeb Duhr Slam — STR-based 3d8+STR bludgeoning melee, reach 1. The
/// stone-creature's signature swing; the heavy 3d8 base die plus a STR-20
/// modifier produces the brute-force per-Action damage budget that anchors
/// the CR-6 slot. Vanilla `SimpleWeapon` — the load-bearing identity is
/// the resistance / immunity envelope plus the double-slam multi, not any
/// per-hit rider (the galeb duhr's RAW "Animate Boulders" recharge is a
/// world-shaping clause the engine doesn't model).
pub static GALEB_DUHR_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "galeb duhr slam",
    &["gds", "duhr-slam"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
);

/// Galeb Duhr Multiattack — 2 slam swings per Action. RAW: "The galeb
/// duhr makes two slam attacks." Same single-sub shape as Stone Golem /
/// Iron Golem / Helmed Horror multis — the per-Action budget is ~2 ×
/// (3d8 + STR mod) = ~38 average against a single target.
pub static GALEB_DUHR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "galeb duhr multiattack",
    sub_attack: &GALEB_DUHR_SLAM,
    count: 2,
});

// ─── Griffon ─────────────────────────────────────────────────────────

/// Griffon Beak — STR-based 1d8+STR piercing melee, the chunkier half
/// of the griffon's per-Action volley. Vanilla `SimpleWeapon` — the
/// griffon's identity is the per-Action beak+talons compound, not any
/// per-swing rider.
pub static GRIFFON_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "griffon beak",
    &["g-beak"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Griffon Talons — STR-based 2d6+STR slashing melee. The bigger of the
/// two swings; pairs with the beak in the per-Action compound for the
/// flying-predator's signature dive-and-rake silhouette.
pub static GRIFFON_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "griffon talons",
    &["g-tal", "claws-g"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Griffon Multiattack — 1 beak + 1 talons per Action via
/// `CompoundAttack`. RAW: "The griffon makes two attacks: one with its
/// beak and one with its claws." Heterogeneous compound (different damage
/// types per swing) is what `CompoundAttack` is for — same shape as the
/// Vrock / Salamander / Medusa multi.
pub static GRIFFON_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "griffon multiattack",
    parts: vec![
        (&GRIFFON_BEAK, 1),
        (&GRIFFON_TALONS, 1),
    ],
});

// ─── Lamia ───────────────────────────────────────────────────────────

/// Lamia Claws — STR-based 2d10+STR slashing melee. The lamia's heavier
/// melee lane; pairs with the intoxicating touch in the per-Action
/// compound. Vanilla `SimpleWeapon` — no per-swing rider; the touch is
/// where the lamia's signature curse rider lives.
pub static LAMIA_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "lamia claws",
    &["l-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Slashing,
);

/// Lamia Intoxicating Touch — single-target curse install at reach 1.
/// Target makes a WIS save vs DC 13; on fail, the target is magically
/// cursed (modeled as Charmed by the lamia for 10 rounds) so they can't
/// take hostile actions against their cursed mistress and the AI's
/// hostile-target gate routes them away from the lamia in the target
/// picker. RAW gives the curse a 1-hour timer and a "disadvantage on
/// WIS saves and ability checks" clause; we collapse to Charmed since
/// the engine doesn't tag "disadvantage on WIS saves only" cleanly and
/// the Charmed condition already encodes the load-bearing tactical
/// implication (can't attack the curser).
///
/// We drop the RAW melee-spell-attack to-hit roll and resolve as a pure
/// save (matching the engine's treatment of similar pure-effect touches
/// like the Medusa's Petrifying Gaze) — the to-hit + on-hit-save shape
/// would double-gate the curse install for what's essentially a single-
/// payload effect; one roll keeps the per-Action tempo legible and the
/// curse-vs-claws lane distinction cleaner.
pub struct LamiaIntoxicatingTouch {}

impl Action for LamiaIntoxicatingTouch {
    fn name(&self) -> &str {
        "intoxicating touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["it", "touch", "lamia-touch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        // Pure curse install — no HP loss on the target. The AI's
        // focus-fire pipeline should prefer the claws for whittling and
        // only reach for the touch when the lockout is more valuable
        // than raw damage tempo.
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        save_or_charmed_by_caster(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            13,
            ConditionTimer::Rounds(10),
            "lamia curse",
        )
    }
}

pub static LAMIA_INTOXICATING_TOUCH: LazyLock<LamiaIntoxicatingTouch> =
    LazyLock::new(|| LamiaIntoxicatingTouch {});

/// Lamia Multiattack — 1 claws + 1 intoxicating touch per Action via
/// `CompoundAttack`. RAW: "Multiattack. The lamia makes two attacks:
/// one with its claws and one with its dagger or Intoxicating Touch."
/// We pick the touch over the dagger because the curse is the lamia's
/// signature (the dagger lane is essentially a tempo-fallback we omit).
/// Same shape as the Medusa multi: heterogeneous parts, single per-
/// Action cost envelope.
pub static LAMIA_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "lamia multiattack",
    parts: vec![
        (&LAMIA_CLAWS, 1),
        (&*LAMIA_INTOXICATING_TOUCH, 1),
    ],
});

// ─── Werebear ────────────────────────────────────────────────────────

/// Werebear Bite — STR-based 1d10+STR piercing melee with a CON save
/// (DC 14) on hit for Poisoned (Rounds(3)), proxy for RAW's lycanthropy
/// curse rider. Higher DC than the werewolf (RAW 12 → 14) since the
/// werebear is a CR-5 threat to the werewolf's CR-3; tuned up one die
/// tier too (1d10 vs 1d8) to match the heftier per-swing damage budget.
pub static WEREBEAR_BITE: LycanthropeBite = LycanthropeBite {
    display_name: "werebear bite",
    aliases: &["wb-bite"],
    damage_dice: Dice::new(1, 10),
    save_dc: 14,
};

/// Werebear Claws — STR-based 2d8+STR slashing melee. The big-die
/// secondary swing in the werebear's bite+claws compound. Vanilla
/// `SimpleWeapon` — the bite carries the lycanthropy curse rider, the
/// claws are the steady damage lane.
pub static WEREBEAR_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "werebear claws",
    &["wb-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
);

/// Werebear Multiattack — 1 bite + 1 claws per Action via
/// `CompoundAttack`. RAW (hybrid form): "Multiattack. In bear or hybrid
/// form, it makes two attacks: one with its bite and one with its
/// claws." Heterogeneous compound — different damage types per swing,
/// the bite carries the curse rider, the claws are the steady damage
/// lane. Same shape as the Griffon / Salamander / Medusa multi.
pub static WEREBEAR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "werebear multiattack",
    parts: vec![(&WEREBEAR_BITE, 1), (&WEREBEAR_CLAWS, 1)],
});

/// Wereboar tusks — STR-based 2d6+STR slashing piercing melee carrying
/// the standard lycanthropy curse rider (DC 12 CON, Poisoned 3 rounds).
/// Same save DC as the werewolf but a heftier 2d6 damage die to match
/// the wereboar's CR-4 power curve (vs werewolf CR 3). RAW Tusks are
/// listed as slashing in 5e MM; we type as Piercing per the
/// `LycanthropeBite` chassis (the curse rider is the load-bearing
/// clause, not the damage type — the wereboar still gets the `Maul`
/// swing for the slashing typing if AC vs damage-type matters).
pub static WEREBOAR_TUSKS: LycanthropeBite = LycanthropeBite {
    display_name: "wereboar tusks",
    aliases: &["wb-tusks", "wereboar-tusks"],
    damage_dice: Dice::new(2, 6),
    save_dc: 12,
};

/// Wereboar Maul — STR-based 2d6+STR bludgeoning melee. The steady
/// damage lane of the wereboar's tusks+maul multi (RAW 5e MM hybrid
/// form: "Multiattack. In humanoid or hybrid form, it makes two attacks,
/// only one of which can be with its tusks."). The maul pairs with the
/// tusks so a per-Action swing both lands the curse rider AND a chunky
/// bludgeon hit.
pub static WEREBOAR_MAUL: SimpleWeapon = SimpleWeapon::melee(
    "wereboar maul",
    &["wb-maul", "wereboar-maul"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Wereboar Multiattack — 1 tusks + 1 maul per Action via
/// `CompoundAttack`. Heterogeneous compound (piercing + bludgeoning) —
/// tusks carry the curse rider, maul is the steady damage lane. Same
/// shape as the Werebear / Werewolf multi.
pub static WEREBOAR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "wereboar multiattack",
    parts: vec![(&WEREBOAR_TUSKS, 1), (&WEREBOAR_MAUL, 1)],
});

/// Wererat bite — STR-based 1d4+STR piercing melee carrying the
/// standard lycanthropy curse rider (DC 11 CON, Poisoned 3 rounds).
/// Lowest DC of the wereXX family — wererat is CR 2 and the curse
/// itself is the weakest of the lycanthrope-curse line per RAW. Same
/// shape as the rest of the family.
pub static WERERAT_BITE: LycanthropeBite = LycanthropeBite {
    display_name: "wererat bite",
    aliases: &["wr-bite", "wererat-bite"],
    damage_dice: Dice::new(1, 4),
    save_dc: 11,
};

/// Wererat Shortsword — DEX-based 1d6+DEX piercing melee. The
/// finesse-weapon lane of the rat-half's humanoid form. RAW (5e MM
/// hybrid form): "Multiattack. The wererat makes two attacks, only one
/// of which can be a bite." Pairs with the bite — Action lands a curse
/// rider + a clean DEX-mod stab.
pub static WERERAT_SHORTSWORD: SimpleWeapon = SimpleWeapon::melee(
    "wererat shortsword",
    &["wr-shortsword"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Wererat Multiattack — 1 bite + 1 shortsword per Action via
/// `CompoundAttack`. Same chassis as Wereboar / Werewolf — bite carries
/// the curse rider, shortsword is the steady damage lane.
pub static WERERAT_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "wererat multiattack",
    parts: vec![(&WERERAT_BITE, 1), (&WERERAT_SHORTSWORD, 1)],
});

/// Weretiger bite — STR-based 1d10+STR piercing melee carrying the
/// standard lycanthropy curse rider (DC 13 CON, Poisoned 3 rounds).
/// Mid-DC of the wereXX family (werewolf 12, weretiger 13, werebear
/// 14). Heaviest single-die in the chassis at 1d10 — matches the
/// weretiger's apex-predator CR 4 niche.
pub static WERETIGER_BITE: LycanthropeBite = LycanthropeBite {
    display_name: "weretiger bite",
    aliases: &["wt-bite", "weretiger-bite"],
    damage_dice: Dice::new(1, 10),
    save_dc: 13,
};

/// Weretiger Claws — STR-based 1d8+STR slashing melee. Pairs with the
/// bite as the per-Action multi. RAW (5e MM hybrid form): "Multiattack.
/// In hybrid form, it can make two scimitar or claw attacks." We
/// collapse to bite + claws to match the rest of the lycanthrope family;
/// the scimitar variant doesn't add a load-bearing tactical clause over
/// the claw swing.
pub static WERETIGER_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "weretiger claws",
    &["wt-claws"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Weretiger Multiattack — 1 bite + 1 claws per Action via
/// `CompoundAttack`. Same chassis as the rest of the lycanthrope
/// family — bite carries the curse rider, claws are the steady damage
/// lane.
pub static WERETIGER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "weretiger multiattack",
    parts: vec![(&WERETIGER_BITE, 1), (&WERETIGER_CLAWS, 1)],
});

// ─── Ettercap ────────────────────────────────────────────────────────

/// Ettercap Bite — STR-based 1d8+STR piercing melee with a CON save
/// (DC 11) for an extra 2d4 poison damage AND a Poisoned condition (2
/// rounds, tighter proxy for RAW's 1-minute / ~10 round duration) on
/// fail. Same "extra damage + condition both ride one save" shape as
/// Spider Bite — routes through the shared `WeaponWithSaveDamage`
/// chassis with `also_install = Some((Poisoned, Rounds(2)))`.
pub static ETTERCAP_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::melee_with_condition(
    "ettercap bite",
    &["ebite", "ettercap-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(2, 4),
    DamageType::Poison,
    "ettercap venom",
    Condition::Poisoned,
    ConditionTimer::Rounds(2),
);


/// Ettercap Claws — STR-based 2d4+STR slashing melee. The chitin-tipped
/// secondary swing of the ettercap's bite + claws multi. Vanilla
/// `SimpleWeapon` — the bite carries the venom rider, the claws are
/// the steady damage lane.
pub static ETTERCAP_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "ettercap claws",
    &["eclaws", "ettercap-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Ettercap Multiattack — 1 bite + 1 claws per Action via
/// `CompoundAttack`. Heterogeneous compound (piercing+poison from the
/// bite, slashing from the claws); the bite carries the venom rider.
pub static ETTERCAP_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "ettercap multiattack",
    parts: vec![(&ETTERCAP_BITE, 1), (&ETTERCAP_CLAWS, 1)],
});

/// Ettercap Web — ranged 30 ft single-target restraint, no attack roll.
/// Target makes a DEX save (DC 11); on fail they're Restrained until
/// they break free. RAW: "Web (Recharge 5–6). Ranged Weapon Attack: +4
/// to hit, range 30/60 ft., one Large or smaller creature. Hit: The
/// creature is Restrained by webbing. As an action, the restrained
/// creature can make a DC 11 STR check, escaping on a success."
///
/// We collapse RAW's "ranged weapon attack roll + saving-throw-to-
/// escape" to a single DEX-save-on-incidence: no attack roll, just the
/// initial DEX check. The escape clause is implicit — the engine's
/// Restrained condition is timer-driven; we set a 3-round timer as a
/// proxy for "spent action breaking free." Recharge 5-6 keeps the AI
/// from spamming the web every turn — gated on the shared
/// `"ettercap_web"` recharge key plugged into the template's
/// `recharge_abilities` list. The standard recharge chassis
/// (`is_recharge_available` validator + `spend_recharge` on resolution)
/// matches the breath-weapon / blinding-spittle / whelm cohort.
pub struct EttercapWeb {}

impl Action for EttercapWeb {
    fn name(&self) -> &str {
        "ettercap web"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eweb", "ettercap-web", "web"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = reach 12 on the 2.5 ft grid.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "ettercap_web")
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
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Spend the recharge resource before resolving the save so a
        // mid-resolution failure can't leave the web both spent AND
        // restraint-applied (mirrors the breath-weapon order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("ettercap_web");
        }
        // Immunity gate — Restrained-immune targets (incorporeal undead,
        // gaseous form holders) short-circuit before the save.
        let Some(target) = encounter.actors.get(&target_id) else {
            return effects;
        };
        if target.effectively_immune_to_condition(Condition::Restrained) {
            encounter.log("  ettercap web: target slips the webbing");
            return effects;
        }
        save_or_condition_rider(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Dexterity,
            11,
            Condition::Restrained,
            ConditionTimer::Rounds(3),
            "ettercap web",
            &mut effects,
        );
        effects
    }
}

pub static ETTERCAP_WEB: LazyLock<EttercapWeb> = LazyLock::new(|| EttercapWeb {});

// ─── Awakened Tree ───────────────────────────────────────────────────

/// Awakened Tree Slam — STR-based 3d6+STR bludgeoning at reach 10ft.
/// Same per-swing shape as `TREANT_SLAM` (3d6+STR, reach 2) — we keep a
/// dedicated constant so the action name reads "awakened tree slam" in
/// the log and the bestiary's CR-2 plant doesn't borrow the CR-9
/// treant's flavor text. Vanilla `SimpleWeapon` — no rider; the tree's
/// load-bearing pressure is the double-slam multi at the awakened
/// tree's STR 19 (+4 mod), not any per-hit effect.
pub static AWAKENED_TREE_SLAM: SimpleWeapon = SimpleWeapon::reach_melee(
    "awakened tree slam",
    &["at-slam", "tree-slam"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    2,
);

/// Awakened Tree Multiattack — 2 slams per Action via the standard
/// `Multiattack` chassis. RAW: "Multiattack. The tree makes two
/// attacks." Same shape as the zombie multislam / bandit captain triple
/// scimitar — homogeneous sub-attack with a fixed count.
pub static AWAKENED_TREE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "awakened tree multiattack",
    sub_attack: &AWAKENED_TREE_SLAM,
    count: 2,
});

// ─── Dretch ──────────────────────────────────────────────────────────

/// Dretch Bite — STR-based 1d6 piercing melee, no STR mod to damage
/// (the dretch is STR 11 / +0 so the distinction is moot, but we leave
/// damage_ability off to keep the manes-tier weakness legible — RAW
/// dretch deals a flat 3 (1d6) bite, not 1d6+STR). Routes through the
/// shared `SimpleWeapon::flat_melee` chokepoint — the per-hit pressure
/// is the claws multi and the Fetid Cloud burst, not the bite.
pub static DRETCH_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "dretch bite",
    &["d-bite", "dretch-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Dretch Claws — STR-based 2d4 slashing melee, no STR mod. RAW: "Hit:
/// 5 (2d4) slashing damage." The dretch's bite + claws multi runs at
/// CR 1/4 budget — the average 5 slashing per Action lane plus the 3
/// piercing bite gives a 7–8 expected per-turn damage envelope before
/// the once-per-day Fetid Cloud lands its Poisoned rider. Same flat-
/// dice constructor as `DRETCH_BITE`.
pub static DRETCH_CLAWS: SimpleWeapon = SimpleWeapon::flat_melee(
    "dretch claws",
    &["d-claws", "dretch-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Dretch Multiattack — 1 bite + 1 claws per Action via `CompoundAttack`.
/// Heterogeneous compound (piercing + slashing) — both swings carry no
/// rider; the Fetid Cloud bonus-action burst is where the dretch's
/// signature condition install lives.
pub static DRETCH_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "dretch multiattack",
    parts: vec![(&DRETCH_BITE, 1), (&DRETCH_CLAWS, 1)],
});

/// Dretch Fetid Cloud — Recharge 6 (RAW: 1/Day; we promote to Recharge
/// 6 so the burst occasionally fires more than once per long combat
/// while still respecting the "exhaustible resource" RAW envelope).
/// 10-ft radius cloud of poisonous fumes centered on the dretch.
/// Each non-demon creature in the burst makes a DC 11 CON save or is
/// Poisoned until the start of the dretch's next turn. We model the
/// "until start of caster's next turn" timer as a 1-round Poisoned
/// install — close enough to the RAW window for a tempo-control engine.
///
/// Demon-immunity gate: dretch is itself a Fiend, but the engine's
/// Poison immunity already short-circuits the install (fiends typically
/// resist or are immune to Poison). We rely on the standard
/// `effectively_immune_to_condition(Poisoned)` chokepoint instead of a
/// per-action creature-type filter — keeps the engine's "the condition
/// install chokepoint handles immunity uniformly" invariant in place.
pub struct DretchFetidCloud {}

impl Action for DretchFetidCloud {
    fn name(&self) -> &str {
        "fetid cloud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fc", "fetid", "dretch-cloud"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Burst centered on self — no point/target args.
        TargetingSchema::NoArgs
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "dretch_fetid_cloud")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the recharge resource up-front so a mid-resolution
        // failure can't leave the cloud both spent AND condition-applied
        // (mirrors the breath-weapon / ettercap-web order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("dretch_fetid_cloud");
        }
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        encounter.log("  fetid cloud: poisonous fumes billow around the dretch");
        // 10 ft radius = 4 tile gap on the 2.5 ft grid.
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            caster_loc,
            4,
            AbilityScoreType::Constitution,
            11,
            Condition::Poisoned,
            ConditionTimer::Rounds(1),
        )
    }
}

pub static DRETCH_FETID_CLOUD: LazyLock<DretchFetidCloud> =
    LazyLock::new(|| DretchFetidCloud {});

// ─── Lemure ──────────────────────────────────────────────────────────

/// Lemure Fist — STR-based 1d4 bludgeoning melee, no STR mod (the lemure
/// has STR 10 / +0). The lowest-tier devil's only attack — a flat
/// 2 (1d4) bludgeoning slap. Routes through the shared
/// `SimpleWeapon::flat_melee` chokepoint (no STR mod to damage — RAW's
/// flat 2 (1d4) shape rather than 1d4+STR).
pub static LEMURE_FIST: SimpleWeapon = SimpleWeapon::flat_melee(
    "lemure fist",
    &["l-fist", "lemure-fist"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Bearded Devil (Barbazu) ─────────────────────────────────────────

/// Bearded Devil Glaive — STR-based 1d10+STR slashing melee at reach
/// 10ft. The polearm-style primary lane — pairs with the beard for the
/// per-Action multi. Vanilla `SimpleWeapon` with reach 2 — no rider;
/// the beard carries the per-Action condition install.
pub static BEARDED_DEVIL_GLAIVE: SimpleWeapon = SimpleWeapon::reach_melee(
    "bearded devil glaive",
    &["bd-glaive", "barbazu-glaive"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Slashing,
    2,
);

/// Bearded Devil Beard — STR-based 1d8+STR piercing melee, reach 5 ft.
/// On hit, target makes a CON save vs DC 12 or is Poisoned for 3
/// rounds. RAW: "While poisoned in this way, the target can't regain
/// hit points. The target can repeat the saving throw at the end of
/// each of its turns, ending the effect on a successful save." We
/// approximate the "no-healing-while-poisoned" RAW clause by leaning on
/// the engine's standard `Poisoned` condition (which already imposes
/// disadvantage on attacks and ability checks); the no-healing clause
/// is dropped since healing isn't a tactically-load-bearing axis in
/// most combat scenarios this engine simulates. Routes through the
/// shared `WeaponWithSaveCondition` chassis so the save + condition
/// install lives at one chokepoint alongside the wolf-trip / constrictor
/// / giant-octopus cohort.
pub static BEARDED_DEVIL_BEARD: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "bearded devil beard",
    &["beard", "barbazu-beard"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    12,
    Condition::Poisoned,
    ConditionTimer::Rounds(3),
    "infernal beard",
);

/// Bearded Devil Multiattack — 1 glaive + 1 beard per Action via
/// `CompoundAttack`. Heterogeneous compound (slashing + piercing) — the
/// beard carries the Poisoned rider, the glaive is the steady damage
/// lane. Same shape as the Werewolf / Werebear / Wereboar multi.
pub static BEARDED_DEVIL_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bearded devil multiattack",
    parts: vec![(&BEARDED_DEVIL_GLAIVE, 1), (&BEARDED_DEVIL_BEARD, 1)],
});

// ─── Blink Dog ───────────────────────────────────────────────────────

/// Blink Dog Bite — STR-based 1d6+STR piercing melee. RAW: "Melee
/// Weapon Attack: +3 to hit, reach 5 ft., one target. Hit: 4 (1d6 + 1)
/// piercing damage." Vanilla `SimpleWeapon::melee` — no rider; the
/// blink dog's signature is the Teleport bonus action, not the bite.
pub static BLINK_DOG_BITE: SimpleWeapon = SimpleWeapon::melee(
    "blink dog bite",
    &["bd-bite", "blink-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Blink Dog Teleport — bonus action short-range teleport. RAW: "The
/// blink dog magically teleports, along with any equipment it is
/// wearing or carrying, up to 40 feet to an unoccupied space it can
/// see." Recharge 4–6 per RAW (the dog has to "phase back in" — once
/// per short rest in 5e).
///
/// We model the teleport as a `SinglePoint` action whose only side
/// effect is a `Move` to the target tile — no damage, no save, no
/// condition install. Range gate: 16 tiles (40 ft on the 2.5 ft grid).
/// LOS required (the dog teleports to a tile it can see). Recharge 4
/// means the dog's d6 roll at start-of-turn restores the teleport on a
/// 4+, matching the "blink-out cooldown" tempo.
pub struct BlinkDogTeleport {}

impl Action for BlinkDogTeleport {
    fn name(&self) -> &str {
        "blink"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bd-blink", "teleport", "blink-dog-teleport"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 40 ft RAW = reach 16 on the 2.5 ft grid.
        Some(16)
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !actor_has_recharge(encounter, caster_id, "blink_dog_teleport") {
            return false;
        }
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        // Destination tile must accept the actor's footprint — `can_move_to`
        // handles bounds, wall, and occupancy in one shot.
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::TeleportActor;
        // Spend the recharge resource up-front so a no-op resolution
        // can't leave the teleport both spent AND not-moved.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("blink_dog_teleport");
        }
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        vec![Box::new(TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static BLINK_DOG_TELEPORT: LazyLock<BlinkDogTeleport> =
    LazyLock::new(|| BlinkDogTeleport {});

// ─── Mephits ─────────────────────────────────────────────────────────
//
// Mephits are CR ¼ – ½ small elementals — the foot-soldiers of the
// elemental planes. Each variant pairs two elements (Ice = water + air,
// Steam = fire + water, etc.) and follows the same chassis:
//   - Single Action: claws (DEX-based slashing melee).
//   - Recharge 6 (or 4-6): breath weapon — a 2-tile cone of typed
//     damage with a DEX or CON save for half.
//   - Passive on-death: detonates via the shared `DeathBurst` chassis.
//
// We expose the variants as `SimpleWeapon` + `BreathWeapon` statics so
// the per-variant template is a one-line `.push()` rather than three
// bespoke Action impls per species.

/// Ice Mephit Claws — DEX-based 1d4+DEX slashing melee with a 1-point
/// cold rider. The rider is rolled as a single d1 (effectively a flat
/// +1) and typed as cold so the damage pipeline applies cold resistance
/// / immunity independently from the slashing portion — keeps fire- and
/// cold-themed allies routing through the same `add_flat_damage_rider`
/// chokepoint without a bespoke struct.
pub struct IceMephitClaws {}

impl Action for IceMephitClaws {
    fn name(&self) -> &str {
        "ice mephit claws"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ice-claws", "mephit-claws"]
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
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Dexterity,
            Some(AbilityScoreType::Dexterity),
            Dice::new(1, 4),
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        // Cold rider — typed separately so per-target cold resistance
        // applies independently from the slashing base. Flat +1 rolled
        // as a d1 so the rider routes through the standard chokepoint.
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 1),
            DamageType::Cold,
            "icy chill",
            &mut effects,
        );
        effects
    }
}

pub static ICE_MEPHIT_CLAWS: LazyLock<IceMephitClaws> = LazyLock::new(|| IceMephitClaws {});

/// Ice Mephit Frost Breath — a 15-ft cone (burst-2 / range-3 in this
/// 2.5 ft grid) of biting cold. 1d8 cold, DC 10 DEX, half on save.
/// Recharge 6 per RAW; we route through the shared `"breath_weapon"`
/// pool so a mephit ambush can't double-tap with two breaths.
pub static ICE_MEPHIT_FROST_BREATH: BreathWeapon = BreathWeapon {
    display_name: "frost breath",
    aliases: &["frost", "ice-breath"],
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Cold,
    save_ability: AbilityScoreType::Dexterity,
    dc: 10,
    radius: 2,
    range: 3,
    recharge_key: "breath_weapon",
};

/// Ice Mephit **Death Burst** — 1d8 slashing in a 5-ft radius (gap 1)
/// when reduced to 0 HP. The mephit shatters into icy shards (RAW: "the
/// mephit explodes, dealing 4 (1d8) slashing damage to each creature
/// within 5 feet of it"). DC 11 DEX halves. Slashing rather than cold
/// so cold-immune targets still take the physical shrapnel — matches
/// the RAW typing exactly.
pub static ICE_MEPHIT_DEATH_BURST: DeathBurst = DeathBurst {
    display_name: "shatters into icy shards",
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Slashing,
    save_ability: AbilityScoreType::Dexterity,
    dc: 11,
    radius: 1,
};

/// Steam Mephit Claws — DEX-based 1d4+DEX slashing melee with a 1-point
/// fire rider. Symmetric to `IceMephitClaws` but with fire on the rider
/// lane (steam = fire + water elemental hybrid). Routes through the
/// same shared `add_flat_damage_rider` chokepoint.
pub struct SteamMephitClaws {}

impl Action for SteamMephitClaws {
    fn name(&self) -> &str {
        "steam mephit claws"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["steam-claws"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Fire]
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
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 1),
            DamageType::Fire,
            "scalding steam",
            &mut effects,
        );
        effects
    }
}

pub static STEAM_MEPHIT_CLAWS: LazyLock<SteamMephitClaws> = LazyLock::new(|| SteamMephitClaws {});

/// Steam Mephit Steam Breath — a 15-ft cone of scalding vapor. 1d6
/// fire, DC 10 DEX, half on save. Recharge 6.
pub static STEAM_MEPHIT_STEAM_BREATH: BreathWeapon = BreathWeapon {
    display_name: "steam breath",
    aliases: &["steam", "vapor-breath"],
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 10,
    radius: 2,
    range: 3,
    recharge_key: "breath_weapon",
};

/// Steam Mephit **Death Burst** — 1d8 fire in a 5-ft radius (gap 1)
/// when reduced to 0 HP. The mephit dissolves into scalding vapor.
/// DC 10 DEX halves.
pub static STEAM_MEPHIT_DEATH_BURST: DeathBurst = DeathBurst {
    display_name: "dissolves in a burst of scalding vapor",
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 10,
    radius: 1,
};

/// Magma Mephit Claws — DEX-based 1d4+DEX slashing melee with a 1-point
/// fire rider. Sibling chassis to `SteamMephitClaws` (fire-themed) but
/// fronts the heavier death-burst variant in the mephit cohort.
pub struct MagmaMephitClaws {}

impl Action for MagmaMephitClaws {
    fn name(&self) -> &str {
        "magma mephit claws"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["magma-claws"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Fire]
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
            DamageType::Slashing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            Dice::new(1, 1),
            DamageType::Fire,
            "molten touch",
            &mut effects,
        );
        effects
    }
}

pub static MAGMA_MEPHIT_CLAWS: LazyLock<MagmaMephitClaws> = LazyLock::new(|| MagmaMephitClaws {});

/// Magma Mephit Fire Breath — a 15-ft cone of searing flame. 1d8 fire,
/// DC 11 DEX, half on save. Recharge 6.
pub static MAGMA_MEPHIT_FIRE_BREATH: BreathWeapon = BreathWeapon {
    display_name: "magma fire breath",
    aliases: &["magma-breath", "lava-breath"],
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 11,
    radius: 2,
    range: 3,
    recharge_key: "breath_weapon",
};

/// Magma Mephit **Death Burst** — 2d6 fire in a 5-ft radius (gap 1) on
/// death. Same damage profile as the Magmin's burst but with the
/// shorter 5-ft mephit radius (RAW). DC 11 DEX halves.
pub static MAGMA_MEPHIT_DEATH_BURST: DeathBurst = DeathBurst {
    display_name: "erupts in a final spray of lava",
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 11,
    radius: 1,
};

// ─── Black Pudding ───────────────────────────────────────────────────

/// Black Pudding Pseudopod — STR-based 1d6+STR bludgeoning melee with a
/// flat 4d8 acid rider on every hit. The acid is the load-bearing damage
/// slice; the bludgeoning is just the flavor of the formless lash. Per-
/// target acid resistance / immunity applies cleanly via the shared
/// `weapon_swing_with_flat_rider` helper. RAW also corrodes the target's
/// armor by -1 AC on hit (non-stacking) — omitted since the engine
/// doesn't model per-item durability; the acid damage is the headline
/// penalty.
pub static BLACK_PUDDING_PSEUDOPOD: WeaponWithRider = WeaponWithRider::melee(
    "black pudding pseudopod",
    &["pudding-pseudopod", "pp", "ooze-pseudopod"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
    Dice::new(4, 8),
    DamageType::Acid,
    "corrosive sludge",
);

// ─── Flesh Golem ─────────────────────────────────────────────────────

/// Flesh Golem Slam — STR-based 2d8+STR bludgeoning melee. The brute-
/// force lane of the flesh golem; the multiattack pairs two of these per
/// Action for a heavy ~25 average damage budget at CR 5. Vanilla
/// SimpleWeapon — no rider; the load-bearing identity lives at the
/// template level (lightning + poison immunity, magic resistance,
/// condition envelope) rather than on the swing itself.
pub static FLESH_GOLEM_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "flesh golem slam",
    &["flesh-slam", "golem-fist"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

/// Flesh Golem Multiattack — 2 slam swings per Action. Classic golem
/// "two heavy hits" envelope; the flesh golem trades the iron golem's
/// reach for sheer per-Action damage budget on a much smaller HP / AC
/// frame.
pub static FLESH_GOLEM_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "flesh golem multiattack",
    sub_attack: &FLESH_GOLEM_SLAM,
    count: 2,
});

// ─── Horned Devil ────────────────────────────────────────────────────

/// Horned Devil Fork — STR-based 2d8+STR piercing melee at reach 10ft
/// (gap 2). The horned devil's two-tined infernal trident; reach 2 lets
/// the devil project the swing through a tile of empty space, matching
/// the RAW reach 10ft envelope. Vanilla SimpleWeapon — no rider; the
/// per-Action damage budget comes from the multi (2 forks + 1 tail).
pub static HORNED_DEVIL_FORK: SimpleWeapon = SimpleWeapon::reach_melee(
    "horned devil fork",
    &["fork", "trident"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
    2,
);

/// Horned Devil Tail — STR-based 1d8+STR piercing melee at reach 10ft
/// (gap 2). On hit, the target makes a DC-17 CON save vs **Infernal
/// Wound**: on fail, picks up the `Poisoned` condition as a stand-in for
/// the RAW "no HP regain + 10 ongoing damage per turn" wound. We
/// approximate the no-regen + DoT clause with the standard Poisoned
/// envelope (disadvantage on attacks / ability checks) for ten rounds —
/// the RAW wound is hard to model cleanly (the engine doesn't yet have
/// a per-actor "blocks healing" flag), and the disadvantage rider is a
/// reasonable proxy for "this wound saps your strength to fight back."
///
/// One save per turn at end-of-turn to shake the wound (rolled by the
/// `ROUND_END_SAVES` table that already wires Poisoned cleanup) would
/// fit cleanly here as future polish.
/// Horned Devil Tail — STR-based 1d8+STR piercing melee at reach 2
/// tiles (10 ft — the horned devil's barbed prehensile tail strikes
/// from outside normal melee range). On hit, target makes a CON save vs
/// DC 17 or picks up the `Poisoned` condition for 10 rounds as the
/// in-engine proxy for RAW's "Infernal Wound" (no HP regain + 10
/// ongoing damage per turn) clause — we approximate the no-regen + DoT
/// clause with the standard Poisoned envelope (disadvantage on attacks
/// / ability checks) since the engine doesn't yet have a per-actor
/// "blocks healing" flag.
///
/// Routes through the shared `WeaponWithSaveCondition` chassis (long-
/// reach variant) so the save + condition install lives at one
/// chokepoint alongside the bearded-devil-beard / constrictor / giant-
/// octopus cohort.
pub static HORNED_DEVIL_TAIL: WeaponWithSaveCondition = WeaponWithSaveCondition::reach_melee(
    "horned devil tail",
    &["tail", "horned-tail"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    17,
    Condition::Poisoned,
    ConditionTimer::Rounds(10),
    "infernal wound",
    2,
);

/// Horned Devil Multiattack — 2 forks + 1 tail per Action via the
/// shared `CompoundAttack` chassis. Heterogeneous compound — the forks
/// run the primary damage budget, the tail carries the Infernal Wound
/// rider. Mixed reach (both legs are reach 2) so the multi can land on
/// a target a tile beyond MELEE_REACH; the CompoundAttack inherits the
/// first sub-attack's reach via `reach_tiles`.
pub static HORNED_DEVIL_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "horned devil multiattack",
    parts: vec![(&HORNED_DEVIL_FORK, 2), (&HORNED_DEVIL_TAIL, 1)],
});

/// Horned Devil Hurled Flame — ranged spell-attack-style fire bolt at 150
/// ft range (we cap at 30 tiles ≈ 75 ft for the 40×20 maps). Attack
/// uses the devil's CHA mod + proficiency (the infernal-spellcaster
/// stat); on hit, 4d6 fire. Doesn't require a recharge — the horned
/// devil can keep flinging hellfire turn after turn (RAW: at will).
pub struct HornedDevilHurledFlame {}

impl Action for HornedDevilHurledFlame {
    fn name(&self) -> &str {
        "hurled flame"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["flame", "hurl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(30)
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
        // CHA-based spell-attack: +CHA mod + proficiency to hit, then 4d6
        // fire on connect. No damage modifier add — the devil's CHA-attack
        // typing doesn't pile a flat CHA mod onto the damage roll (RAW).
        let attack_mod = caster.spell_attack_modifier(AbilityScoreType::Charisma);
        crate::engine::attack::resolve_attack(
            encounter,
            crate::engine::attack::AttackParams {
                caster_id,
                target_id,
                action_name: "hurled flame",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(4, 6),
                damage_bonus: 0,
                damage_type: DamageType::Fire,
                is_melee: false,
                long_range: None,
                min_range: None,
                is_spell: true,
            },
        )
    }
}

pub static HORNED_DEVIL_HURLED_FLAME: LazyLock<HornedDevilHurledFlame> =
    LazyLock::new(|| HornedDevilHurledFlame {});

// ─── Nalfeshnee ──────────────────────────────────────────────────────

/// Nalfeshnee Bite — STR-based 5d10+STR piercing melee. The Type V
/// demon's monstrous boar-tusk chomp; the heaviest single-die attack at
/// CR 13 — `5d10` averages ~27 + STR mod for the bite alone, which the
/// `CompoundAttack` pairs with two claws for a per-Action damage budget
/// in line with the Marilith / Glabrezu profile.
pub static NALFESHNEE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "nalfeshnee bite",
    &["nbite", "nalfeshnee-bite"],
    AbilityScoreType::Strength,
    Dice::new(5, 10),
    DamageType::Piercing,
);

/// Nalfeshnee Claw — STR-based 3d6+STR slashing melee. The supporting
/// swings to the bite's heavy chomp; the `CompoundAttack` runs two of
/// these alongside the bite per Action.
pub static NALFESHNEE_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "nalfeshnee claw",
    &["nclaw", "nalfeshnee-claw"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Slashing,
);

/// Nalfeshnee Multiattack — 1 bite + 2 claws per Action via the shared
/// `CompoundAttack` chassis. Mixed-limb heterogeneous compound matching
/// the canonical Hezrou / Pit Fiend pattern. The 5d10 bite + 2×3d6
/// claws lands around 50 average damage per Action — the load-bearing
/// per-turn budget for a CR-13 boss.
pub static NALFESHNEE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "nalfeshnee multiattack",
    parts: vec![(&NALFESHNEE_BITE, 1), (&NALFESHNEE_CLAW, 2)],
});

/// Nalfeshnee Horror Nimbus — Recharge 5–6 area-of-effect Frighten.
/// RAW: "Each creature within 15 ft of the Nalfeshnee that can see it
/// must succeed on a DC 15 WIS save or be Frightened for 1 minute."
/// Engine model: routes the burst through the shared `resolve_burst_save_
/// condition` helper at the Nalfeshnee's own footprint (radius 3 tiles
/// ≈ 15 ft) with a Frightened (10 round) install on fail. The 24-hour
/// "creature that saves is immune" RAW clause is omitted — the engine's
/// combat envelope is short enough that a single encounter rarely re-
/// triggers Horror Nimbus against the same target enough times to make
/// the immunity clause matter.
pub struct NalfeshneeHorrorNimbus {}

impl Action for NalfeshneeHorrorNimbus {
    fn name(&self) -> &str {
        "horror nimbus"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nimbus", "horror", "hn"]
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "horror_nimbus")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the recharge resource before resolving so a mid-
        // resolution failure can't leave the nimbus both spent AND
        // condition-applied (matches the breath / web order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("horror_nimbus");
        }
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        encounter.log("  horror nimbus: a wave of mind-bending terror radiates outward");
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            caster_loc,
            3,
            AbilityScoreType::Wisdom,
            15,
            Condition::Frightened,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static NALFESHNEE_HORROR_NIMBUS: LazyLock<NalfeshneeHorrorNimbus> =
    LazyLock::new(|| NalfeshneeHorrorNimbus {});

// ─── Djinni ──────────────────────────────────────────────────────────

/// Djinni Scimitar — STR-based 1d6+STR slashing melee with a flat 1d6
/// thunder rider. The air genie's curved blade carries a clap of roaring
/// wind on every strike: the slashing is the headline damage, the thunder
/// rider routes through the standard `weapon_swing_with_flat_rider`
/// chokepoint so per-target thunder resistance applies independently from
/// the slashing base. RAW: the djinni picks lightning OR thunder on each
/// swing — we pin to thunder for log-line consistency (the engine's other
/// thunder-rider creatures all use the same display lane).
pub static DJINNI_SCIMITAR: WeaponWithRider = WeaponWithRider::melee(
    "djinni scimitar",
    &["djinni-scimitar", "dj-scim"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
    Dice::new(1, 6),
    DamageType::Thunder,
    "thunderous wind",
);

/// Djinni Multiattack — 3 scimitar swings per Action via the homogeneous
/// `Multiattack` chassis. Heavier per-Action damage budget than the Bandit
/// Captain's triple-scimitar — the thunder rider stacks on every swing,
/// so a clean three-hit Action lands ~21 slashing + ~10 thunder against a
/// medium-AC target.
pub static DJINNI_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "djinni multiattack",
    sub_attack: &DJINNI_SCIMITAR,
    count: 3,
});

// ─── Efreeti ─────────────────────────────────────────────────────────

/// Efreeti Scimitar — STR-based 2d6+STR slashing melee with a flat 2d6
/// fire rider. The fire genie's massive curved blade is wreathed in
/// continuous flame — every swing carries the hellish heat alongside the
/// physical cut. Heavier dice than the djinni's scimitar (2d6 vs 1d6
/// slashing, 2d6 vs 1d6 elemental) reflecting RAW's bigger STR build
/// (22 vs 21) and the efreeti's signature "burning blade" flavor.
pub static EFREETI_SCIMITAR: WeaponWithRider = WeaponWithRider::melee(
    "efreeti scimitar",
    &["efreeti-scimitar", "ef-scim"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    Dice::new(2, 6),
    DamageType::Fire,
    "burning blade",
);

/// Efreeti Multiattack — 2 scimitar swings per Action via the homogeneous
/// `Multiattack` chassis. Fewer swings than the djinni's triple, but each
/// swing carries the 2d6 fire rider — ~26 average per-Action damage
/// (clean 2-hit) before factoring in the fire rider, which crushes any
/// non-fire-resistant target.
pub static EFREETI_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "efreeti multiattack",
    sub_attack: &EFREETI_SCIMITAR,
    count: 2,
});

/// Efreeti Hurl Flame — ranged spell-attack-style fire bolt at 120 ft range
/// (we cap at 30 tiles ≈ 75 ft for the 40×20 maps, matching the horned
/// devil's same pattern). Attack uses the efreeti's CHA mod + proficiency
/// to hit; on hit, 5d6 fire. At-will (no recharge); the efreeti's "stay
/// out of melee and lob fireballs" stand-off lane mirroring Horned Devil's
/// Hurled Flame at the same CR but on a heftier 5d6 damage die.
pub struct EfreetiHurlFlame {}

impl Action for EfreetiHurlFlame {
    fn name(&self) -> &str {
        "efreeti hurl flame"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hurl-flame", "ef-flame"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(30)
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
        let attack_mod = caster.spell_attack_modifier(AbilityScoreType::Charisma);
        crate::engine::attack::resolve_attack(
            encounter,
            crate::engine::attack::AttackParams {
                caster_id,
                target_id,
                action_name: "efreeti hurl flame",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(5, 6),
                damage_bonus: 0,
                damage_type: DamageType::Fire,
                is_melee: false,
                long_range: None,
                min_range: None,
                is_spell: true,
            },
        )
    }
}

pub static EFREETI_HURL_FLAME: LazyLock<EfreetiHurlFlame> =
    LazyLock::new(|| EfreetiHurlFlame {});

// ─── Constrictor Snake ───────────────────────────────────────────────

/// Constrictor Snake Bite — STR-based 1d6+STR piercing melee. Vanilla
/// SimpleWeapon — the snake's bite is pure damage; the grapple lane lives
/// on the separate `CONSTRICTOR_SNAKE_CONSTRICT` action.
pub static CONSTRICTOR_SNAKE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "constrictor snake bite",
    &["snake-bite", "constrictor-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Constrictor Snake Constrict — STR-based 1d8+STR bludgeoning melee. On
/// a hit, the target makes a DC 14 STR save vs **Constrict**: on fail,
/// picks up the `Grappled` condition for 10 rounds (RAW: "until this
/// grapple ends"). Routes through the shared `WeaponWithSaveCondition`
/// chassis so the save + condition install lives at one chokepoint —
/// the grapple-immune envelope (elementals / oversized creatures) shrugs
/// it off via the standard `add_condition` gate.
pub static CONSTRICTOR_SNAKE_CONSTRICT: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "constrict",
    &["constrict", "constrictor-constrict"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Bludgeoning,
    AbilityScoreType::Strength,
    14,
    Condition::Grappled,
    ConditionTimer::Rounds(10),
    "constrict",
);

/// Giant Constrictor Snake Bite — STR-based 2d6+STR piercing melee with a
/// flat 1d4 poison rider at reach 2 tiles (10ft — the huge serpent's
/// lunge). The huge snake's bite carries a mild venom RAW ("2d4 poison");
/// we collapse to a flat 1d4 rider so per-target poison resistance applies
/// cleanly via the standard `weapon_swing_with_flat_rider` chokepoint.
pub static GIANT_CONSTRICTOR_SNAKE_BITE: WeaponWithRider = WeaponWithRider {
    display_name: "giant constrictor snake bite",
    aliases: &["giant-snake-bite", "gcs-bite"],
    attack_ability: AbilityScoreType::Strength,
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Piercing,
    reach: 2,
    is_melee: true,
    rider_dice: Dice::new(1, 4),
    rider_type: DamageType::Poison,
    rider_name: "snake venom",
};

/// Giant Constrictor Snake Constrict — STR-based 2d8+STR bludgeoning melee
/// at reach 2 tiles (10ft — the huge serpent loops around larger prey).
/// On hit, the target makes a DC 16 STR save vs **Constrict**: on fail,
/// picks up the `Grappled` condition for 10 rounds. Higher DC than the
/// regular constrictor's DC 14 — the giant snake's coils are much harder
/// to break. Routes through the shared `WeaponWithSaveCondition` chassis
/// (long-reach variant) so the save + condition install lives at one
/// chokepoint alongside the regular constrictor.
pub static GIANT_CONSTRICTOR_SNAKE_CONSTRICT: WeaponWithSaveCondition =
    WeaponWithSaveCondition::reach_melee(
        "giant constrict",
        &["giant-constrict", "gcs-constrict"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Bludgeoning,
        AbilityScoreType::Strength,
        16,
        Condition::Grappled,
        ConditionTimer::Rounds(10),
        "giant constrict",
        2,
    );

// ─── Marid ───────────────────────────────────────────────────────────

/// Marid Trident — STR-based 2d6+STR piercing melee. The water genie's
/// signature weapon: a brass-and-coral trident. Vanilla SimpleWeapon —
/// no rider; the marid's elemental punch comes from the Water Jet
/// stand-off attack rather than per-swing damage. Pairs in a 3-trident
/// multi for the standard "triple swing" upper-mid genie envelope
/// mirroring the djinni's 3-scimitar shape (where the djinni adds
/// a 1d6 thunder rider, the marid hits cleanly each time on bigger
/// 2d6 piercing dice).
pub static MARID_TRIDENT: SimpleWeapon = SimpleWeapon::melee(
    "marid trident",
    &["marid-trident", "ma-tri"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Marid Multiattack — 3 trident swings per Action via the homogeneous
/// `Multiattack` chassis. Mirrors the djinni's 3-scimitar shape but on
/// a heavier 2d6 piercing die — the marid trades the djinni's per-swing
/// 1d6 thunder rider for bigger base dice, so the per-Action damage is
/// comparable on a target without thunder resistance but lands more
/// reliably against typed-resistant defenders.
pub static MARID_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "marid multiattack",
    sub_attack: &MARID_TRIDENT,
    count: 3,
});

/// Marid Water Jet — Recharge 4–6 ranged save-for-half attack at 60ft
/// range (24 tiles). The marid blasts a 5ft-wide stream of pressurized
/// water at a single target: DC 17 DEX save, fail → 21 (6d6) bludgeoning
/// damage and pushed 20ft away from the marid; pass → half damage, no
/// push. The recharge gate uses the shared `"water_jet"` key so it
/// doesn't collide with the dragon turtle's `"breath_weapon"` cooldown
/// or the kraken's lightning storm — each upper-tier creature with a
/// recharge ability gets its own pool.
///
/// Save-for-half + push-on-fail is the same shape as Thunderwave but on
/// a single-target ranged attack rather than a friend-or-foe self-burst.
/// Push routes through `PushActor` so wall / occupancy blocking applies
/// — a target pinned against a wall just doesn't move.
pub struct MaridWaterJet {}

impl Action for MaridWaterJet {
    fn name(&self) -> &str {
        "water jet"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wj", "marid-jet"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles. Reaches across most encounter maps without
        // letting the marid plink from off-screen.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Recharge gate — the marid spends its `"water_jet"` recharge
        // resource on cast, refreshed at the start of its turn on a d6
        // roll of 4+. The encounter's recharge table tracks this state
        // per-actor.
        actor_has_recharge(encounter, caster_id, "water_jet")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Spend the recharge resource up-front so a mid-resolution
        // failure can't leave the jet both unspent AND damage-applied
        // (matches the breath / web / horror-nimbus order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("water_jet");
        }
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        const DC: i32 = 17;
        // 20 ft push = 8 tiles on the 2.5ft grid (matching the RAW
        // marid's "pushed up to 20 ft away" rider).
        const PUSH_TILES: u32 = 8;
        encounter.log("  water jet: a hydrant of pressurized water lances out");
        let damage = encounter.roll(&Dice::new(6, 6));
        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, DC);
        let dmg = crate::engine::saves::SaveDamagePolicy::HalfOnSave
            .apply(damage, save.passed());
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Bludgeoning,
            }));
        }
        // Push only on a failed save — RAW: "On a failed save, the target
        // takes 21 (6d6) bludgeoning damage and is pushed up to 20 feet
        // away from the marid and knocked prone." We model the push half;
        // the prone half isn't part of the SRD marid stat block (5e RAW)
        // so we omit it to keep the action's load-bearing rider
        // unambiguous.
        if !save.passed() {
            effects.push(Box::new(PushActor {
                actor_id: target_id,
                from: caster_loc,
                max_tiles: PUSH_TILES,
            }));
        }
        effects
    }
}

pub static MARID_WATER_JET: LazyLock<MaridWaterJet> = LazyLock::new(|| MaridWaterJet {});

// ─── Crocodiles ──────────────────────────────────────────────────────

/// Crocodile Bite — STR-based 1d10+STR piercing melee. On a hit the
/// target picks up the `Grappled` condition for 10 rounds (RAW: "the
/// target is grappled (escape DC 12). Until this grapple ends, the
/// target is restrained, and the crocodile can't bite another
/// target.") We collapse to a clean Grappled install on hit (no save —
/// the bite latches on automatically), routed through the standard
/// `save_or_condition_rider` chokepoint's no-save sibling. Mirrors the
/// constrictor snake's `constrict` shape but folds the bite and the
/// grapple into one swing rather than splitting them into separate
/// actions — the crocodile's RAW bite IS the grapple lock-down.
pub struct CrocodileBite {}

impl Action for CrocodileBite {
    fn name(&self) -> &str {
        "crocodile bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["croc-bite", "crocodile-bite"]
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
        let (mut effects, damage) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            "crocodile bite",
            AbilityScoreType::Strength,
            Dice::new(1, 10),
            DamageType::Piercing,
            true,
            None,
        );
        if damage == 0 {
            return effects;
        }
        // No save — RAW: the bite auto-grapples on hit. Grapple-immune
        // targets (the elemental / construct envelope) shrug it off at
        // the install site via `add_condition`'s immunity check.
        encounter.log("  crocodile bite: jaws latch on, target is grappled");
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            Condition::Grappled,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        ));
        effects
    }
}

pub static CROCODILE_BITE: LazyLock<CrocodileBite> = LazyLock::new(|| CrocodileBite {});

/// Giant Crocodile Bite — STR-based 3d10+STR piercing melee at reach 2
/// tiles (10 ft — the huge croc's lunge). Same "auto-grapple on hit"
/// rider as the regular crocodile but on much heavier dice (3d10 vs
/// 1d10) reflecting the CR-5 huge-beast stat block.
pub struct GiantCrocodileBite {}

impl Action for GiantCrocodileBite {
    fn name(&self) -> &str {
        "giant crocodile bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["giant-croc-bite", "gc-bite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
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
        let (mut effects, damage) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            "giant crocodile bite",
            AbilityScoreType::Strength,
            Dice::new(3, 10),
            DamageType::Piercing,
            true,
            None,
        );
        if damage == 0 {
            return effects;
        }
        encounter.log("  giant crocodile bite: massive jaws clamp shut, target is grappled");
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            Condition::Grappled,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        ));
        effects
    }
}

pub static GIANT_CROCODILE_BITE: LazyLock<GiantCrocodileBite> =
    LazyLock::new(|| GiantCrocodileBite {});

/// Giant Crocodile Tail — STR-based 2d8+STR bludgeoning melee at reach 2
/// tiles. The huge croc's tail sweep — vanilla SimpleWeapon, no rider;
/// pairs with the bite in the multi for a heavier per-Action damage
/// budget. RAW: "Tail. Melee Weapon Attack: +8 to hit, reach 10 ft.,
/// one target not grappled by the crocodile. Hit: 14 (2d8 + 5)
/// bludgeoning damage. If the target is a creature, it must succeed on
/// a DC 16 Strength saving throw or be knocked prone." We collapse to
/// vanilla 2d8+STR — the load-bearing combat clause is the per-Action
/// damage budget, not the conditional prone rider.
pub static GIANT_CROCODILE_TAIL: SimpleWeapon = SimpleWeapon::reach_melee(
    "giant crocodile tail",
    &["giant-croc-tail", "gc-tail"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
    2,
);

/// Giant Crocodile Multiattack — 1 bite + 1 tail per Action via the
/// heterogeneous `CompoundAttack` chassis. The bite carries the auto-
/// grapple rider; the tail is a vanilla heavy slam. ~16 + 14 average
/// per Action against a single target, plus the grapple lock-down on
/// the bite half.
pub static GIANT_CROCODILE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "giant crocodile multiattack",
    parts: vec![
        (&*GIANT_CROCODILE_BITE, 1),
        (&GIANT_CROCODILE_TAIL, 1),
    ],
});

// ─── Dao ─────────────────────────────────────────────────────────────

/// Dao Maul — STR-based 2d6+STR bludgeoning melee with a flat 2d10
/// thunder rider. The earth genie's signature weapon: a massive iron
/// maul that drives the ground itself into the target on impact, the
/// shockwave ringing through the victim's bones. Heavier rider dice
/// than the djinni's scimitar (1d6 thunder) and a different damage
/// shape from the efreeti's burning blade (2d10 thunder vs 2d6 fire)
/// — the dao trades the per-target damage-type defensibility of the
/// efreeti's smaller fire die for a beefier elemental burst that
/// crushes targets without thunder resistance.
///
/// Per-swing average: ~6 + ~11 = ~17 base + ~11 thunder ≈ 28 typed
/// damage per landing swing (RAW: 13 bludgeoning + 11 thunder = 24,
/// matching the MM stat block within rounding tolerance).
pub static DAO_MAUL: WeaponWithRider = WeaponWithRider::melee(
    "dao maul",
    &["dao-maul", "da-maul"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    Dice::new(2, 10),
    DamageType::Thunder,
    "earth-shaking blow",
);

/// Dao Multiattack — 2 maul swings per Action via the homogeneous
/// `Multiattack` chassis. Fewer swings than the djinni's triple-
/// scimitar but each maul carries the 2d10 thunder rider on top of
/// the 2d6 bludgeoning — ~56 per-Action average damage on a clean
/// double-hit, the heaviest melee output among the genie family.
/// Mirrors the efreeti's 2-swing shape but with a thunder rider in
/// place of fire.
pub static DAO_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "dao multiattack",
    sub_attack: &DAO_MAUL,
    count: 2,
});

/// Dao Stone Snare — Recharge 5–6 ranged save-or-restrain attack at
/// 30ft range (12 tiles). The dao stomps the ground and slabs of
/// living stone erupt around a single distant target: DC 17 STR save
/// or take 4d8 bludgeoning damage and become `EarthenGrasped` for one
/// round (RAW: until the dao's next turn). On a successful save: half
/// damage, no restrain. Distinct from the marid's `Water Jet` (DEX
/// save + push) — the dao's ranged tool is a STR-save crowd-control
/// rather than a positioning push, matching the earth genie's
/// "trap-and-pin" combat flavor.
///
/// The save-for-half damage policy mirrors `Water Jet` so a target
/// who passes still eats meaningful chip damage. The
/// `EarthenGrasped` install routes through the standard
/// `ApplyCondition` chokepoint so condition immunities (large
/// elementals, the existing `effectively_immune_to_condition`
/// gate) shrug it off cleanly. The recharge key `"stone_snare"` is
/// distinct from `"water_jet"` so genie templates with both could
/// coexist without resource collision (the dao only declares the
/// `"stone_snare"` recharge slot).
pub struct DaoStoneSnare {}

impl Action for DaoStoneSnare {
    fn name(&self) -> &str {
        "stone snare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "dao-snare"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles. Shorter than the marid's water jet (24
        // tiles) — the dao's stone reach is grounded and doesn't shoot
        // across rooms.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Recharge gate — the dao spends its `"stone_snare"` recharge
        // resource on cast, refreshed at the start of its turn on a d6
        // roll of 5+ (matching the standard "Recharge 5–6" gate on
        // dragon breath / horror nimbus / web).
        actor_has_recharge(encounter, caster_id, "stone_snare")
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
        // Spend the recharge resource up-front so a mid-resolution
        // failure can't leave the snare both unspent AND damage-
        // applied (matches the marid water jet / dragon breath /
        // horror nimbus order-of-ops).
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("stone_snare");
        }
        const DC: i32 = 17;
        encounter.log("  stone snare: slabs of living rock erupt around the target");
        let damage = encounter.roll(&Dice::new(4, 8));
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, DC);
        let dmg = crate::engine::saves::SaveDamagePolicy::HalfOnSave
            .apply(damage, save.passed());
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Bludgeoning,
            }));
        }
        // EarthenGrasped install only on a failed save — mirrors the
        // marid water jet's push-on-fail-only shape but with a
        // condition install instead of a positioning effect. The
        // condition's immunity gate is the install-site responsibility
        // (Restrained-immune elementals shrug it off via
        // `add_condition`'s effective immunity check).
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::EarthenGrasped,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static DAO_STONE_SNARE: LazyLock<DaoStoneSnare> = LazyLock::new(|| DaoStoneSnare {});

// ─── Invisible Stalker ───────────────────────────────────────────────

/// Invisible Stalker Slam — STR-based 2d8+STR bludgeoning melee, reach 1.
/// Same per-swing dice as the Air Elemental's slam (the stalker is, RAW,
/// "an air elemental shaped into a tracking form"). Vanilla `SimpleWeapon`
/// — the stalker's signature defensive envelope (permanent native
/// Invisibility installed via the template's `innate_conditions` lane)
/// is what differentiates it from the plain air elemental, not the
/// swing itself. The standard attacker-side advantage from `Invisible`
/// reads through `compute_attack_mode` so the stalker connects more
/// reliably than the air elemental at the same per-swing damage.
pub static INVISIBLE_STALKER_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "invisible stalker slam",
    &["islam", "stalker-slam"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

/// Invisible Stalker Multiattack — 2 slams per Action. Mirrors the air
/// elemental wrapper exactly; the stalker's per-turn output is two
/// invisible slams to whichever target it's been bound to hunt down.
pub static INVISIBLE_STALKER_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "invisible stalker multiattack",
    sub_attack: &INVISIBLE_STALKER_SLAM,
    count: 2,
});

// ─── Mammoth ─────────────────────────────────────────────────────────

/// Mammoth Gore — STR-based 4d8+STR piercing melee, reach 1 (5 ft).
/// The headline weapon — RAW 4d8+7 averages to ~25 per swing on the
/// CR-6 Huge frame, second only to the Earth Elemental's 4d8 slam
/// among the engine's CR-5 to CR-7 melee strikers. Vanilla
/// `SimpleWeapon` shape; the load-bearing combat clause lives on the
/// `MAMMOTH_MULTI` wrapper that pairs the gore with the stomp.
pub static MAMMOTH_GORE: SimpleWeapon = SimpleWeapon::melee(
    "mammoth gore",
    &["mgore", "tusks-m"],
    AbilityScoreType::Strength,
    Dice::new(4, 8),
    DamageType::Piercing,
);

/// Mammoth Stomp — STR-based 4d10+STR bludgeoning melee, reach 1, but
/// gated on the target being **Prone** (RAW: "The mammoth can only use
/// this attack against a creature that is Prone"). RAW averages to ~29
/// per stomp on the CR-6 Huge frame — the higher-damage limb in the
/// gore + stomp combo, paired in the `MAMMOTH_MULTI` wrapper.
///
/// The Prone gate is enforced via `custom_validate_input` reading the
/// target's condition set: the stomp validates only when the target has
/// the `Prone` condition. Mirrors the gate shape of the Vampire Bite
/// (which validates only against Charmed / Restrained / Incapacitated /
/// Grappled / Unconscious / Willing targets) — the engine's "I can only
/// hit you if you're already down" idiom. Out of the multiattack, the
/// stomp is a single-target standalone the AI can fall back to if the
/// target is already prone from a previous round (Trampling Charge rider
/// install, Booming Blade follow-up, etc.).
pub struct MammothStomp {}

impl Action for MammothStomp {
    fn name(&self) -> &str {
        "mammoth stomp"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mstomp", "stomp-m"]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW: "the mammoth can only make this attack against a target
        // that is prone." Routes through the shared `target_has_condition`
        // helper so the missing-target / missing-actor fail-closed
        // convention stays consistent with the recharge gates.
        target_has_condition(encounter, target_ids, Condition::Prone)
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
            "mammoth stomp",
            AbilityScoreType::Strength,
            Some(AbilityScoreType::Strength),
            Dice::new(4, 10),
            DamageType::Bludgeoning,
            true,
        )
    }
}

pub static MAMMOTH_STOMP: LazyLock<MammothStomp> = LazyLock::new(|| MammothStomp {});

// ─── Dust Mephit ─────────────────────────────────────────────────────

/// Dust Mephit Claws — DEX-based 1d4+DEX slashing melee. The vanilla
/// mephit-claw shape, identical in dice to the Ice / Steam / Magma
/// variants but without a typed-rider tail (the dust mephit's damage
/// envelope is just gritty dust scrapes — its load-bearing pressure is
/// the Blinding Breath save-or-Blinded gate, not the claws themselves).
/// Vanilla `SimpleWeapon`.
pub static DUST_MEPHIT_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "dust mephit claws",
    &["dust-claws", "grit-claws"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Slashing,
);

/// Dust Mephit Blinding Breath — a 15-ft cone (burst-2 / range-3) of
/// fine choking grit. Save-or-Blinded for 1 round on a failed DC 10
/// CON save; no damage. Recharge 6.
///
/// Showcases the new `BreathWeaponCondition` chassis — the damage-free
/// sibling of `BreathWeapon`. RAW's "until the end of the mephit's next
/// turn" timer collapses to `Rounds(1)` at the engine's per-round
/// granularity. The cone is the dust mephit's only ranged threat; the
/// claws are a fallback for adjacent targets after the breath spends.
pub static DUST_MEPHIT_BLINDING_BREATH: BreathWeaponCondition = BreathWeaponCondition {
    display_name: "blinding breath",
    aliases: &["blinding", "dust-breath", "grit-cone"],
    save_ability: AbilityScoreType::Constitution,
    dc: 10,
    radius: 2,
    range: 3,
    recharge_key: "breath_weapon",
    condition: Condition::Blinded,
    timer: ConditionTimer::Rounds(1),
};

/// Dust Mephit **Death Burst** — 1d4 bludgeoning in a 5-ft radius
/// (gap 1) when reduced to 0 HP. The mephit collapses into a spray of
/// fine sand and dust shards (RAW: "the mephit explodes in a burst of
/// dust"). DC 10 CON halves. Bludgeoning rather than a typed energy so
/// elemental-immune kin (other dust mephits in the cohort) still take
/// the physical grit — same convention the Ice Mephit's slashing burst
/// uses to keep the radius lethal to its own cohort.
pub static DUST_MEPHIT_DEATH_BURST: DeathBurst = DeathBurst {
    display_name: "collapses in a burst of dust",
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Bludgeoning,
    save_ability: AbilityScoreType::Constitution,
    dc: 10,
    radius: 1,
};

// ─── Purple Worm ─────────────────────────────────────────────────────

/// Purple Worm Bite — STR-based 3d8+STR piercing melee at reach 2
/// (10 ft) — the gargantuan worm's signature maw can engulf a target
/// from one tile away. Vanilla `SimpleWeapon::reach_melee`; the
/// signature combat clause is the multi-attack pairing with the Tail
/// Stinger and the worm's gargantuan Tunneler trait (which lives on the
/// template as flavor only — the engine isn't 3D and doesn't model
/// underground movement separately from surface speed).
pub static PURPLE_WORM_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "purple worm bite",
    &["pw-bite", "worm-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Piercing,
    2,
);

/// Purple Worm Tail Stinger — STR-based 3d6+STR piercing melee at
/// reach 2 (10 ft), with a CON DC 19 save-or-extra-poison rider. RAW
/// the venom hits for 7d6 poison on a failed save (half on success);
/// we collapse "half on save" to "full on fail, 0 on save" via the
/// shared `save_or_damage_rider` chassis so the typed-resistance lane
/// still applies per-target. The save DC and rider dice are CR-15
/// boss-tier — a failed save by a Medium PC eats ~24 average extra
/// poison, on top of the ~17 average from the swing base.
///
/// Showcases the "weapon hit + save-or-poison-damage" pattern shared
/// by Imp Sting, Spider Bite, Wyvern Stinger, and now Purple Worm
/// Tail Stinger — routes through the shared `WeaponWithSaveDamage`
/// chassis (long-reach variant). Half-on-save is collapsed to all-or-
/// nothing via the shared `save_or_damage_rider` chokepoint (matches
/// Imp Sting / Quasit Claws); per-target poison resistance / immunity
/// is honored by the standard damage pipeline.
pub static PURPLE_WORM_TAIL_STINGER: WeaponWithSaveDamage = WeaponWithSaveDamage::reach_melee(
    "purple worm tail stinger",
    &["pw-stinger", "worm-stinger", "tail-stinger"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    19,
    Dice::new(7, 6),
    DamageType::Poison,
    "purple worm venom",
    2,
);

/// Purple Worm Multiattack — 1 bite + 1 tail stinger per Action via the
/// shared `CompoundAttack` chassis. The bite is the bulk-damage limb
/// (~18 average on a hit); the stinger trails with the save-or-poison
/// rider that opens the burst-damage window. Mixed-limb compound so the
/// AI / player can't pick "two bites" by spamming the bite alone —
/// matches the SRD's per-Action shape exactly.
pub static PURPLE_WORM_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "purple worm multiattack",
    parts: vec![
        (&PURPLE_WORM_BITE, 1),
        (&PURPLE_WORM_TAIL_STINGER, 1),
    ],
});

// ─── Deva ────────────────────────────────────────────────────────────

/// Deva Mace — STR-based 1d6+STR bludgeoning melee with a flat 4d8
/// radiant rider. The lesser angel's blessed weapon — every swing
/// carries the smiting glow of celestial purity. RAW MM stat: "Hit:
/// (1d6+4) bludgeoning damage plus (4d8) radiant damage." Routes
/// through `WeaponWithRider` so the radiant rider's per-target
/// resistance / immunity is honored cleanly — undead and fiends eat
/// the full pile, radiant-resistant outsiders eat half. Heavy rider
/// dice (~18 average) make this the load-bearing burst lane at CR 10.
pub static DEVA_MACE: WeaponWithRider = WeaponWithRider::melee(
    "deva mace",
    &["deva-mace", "blessed-mace"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
    Dice::new(4, 8),
    DamageType::Radiant,
    "celestial smite",
);

/// Deva Multiattack — 2 mace swings per Action via the homogeneous
/// `Multiattack` chassis. Each swing carries the full radiant rider —
/// RAW: "The deva makes two melee attacks." A clean two-hit Action
/// lands ~9 bludgeoning + ~36 radiant against a medium-AC target, the
/// per-round threat envelope that defines the CR-10 celestial slot.
pub static DEVA_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "deva multiattack",
    sub_attack: &DEVA_MACE,
    count: 2,
});

/// Deva Healing Touch — single-target ally heal, 4d8 HP restored, plus
/// cures any disease / poison the holder has. Gated on the
/// `"healing_touch"` recharge key (recharge 4-6 on a d6 at start of
/// turn) so it can't fire every round. RAW is "1/day" — the engine
/// doesn't track per-day pools, so the recharge envelope is the
/// closest approximation. Same chassis as the unicorn's healing touch
/// but the deva's 4d8 envelope is one step heavier (RAW: "The angel
/// touches another creature. The target magically regains 20 (4d8)
/// hit points") and the recharge threshold is one notch lower (4-6
/// vs the unicorn's 5-6) to reflect the deva's higher CR / role as a
/// dedicated celestial healer.
pub struct DevaHealingTouch {}

impl Action for DevaHealingTouch {
    fn name(&self) -> &str {
        "deva healing touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["deva-ht", "deva-touch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
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
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !caster.is_recharge_available("healing_touch") {
            return false;
        }
        // Reject hostile targets — the touch only restores allies. The
        // ally check lives here (not just at side_effects) so the AI's
        // picker doesn't surface enemies as legal targets.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter.actors_allied(caster_id, target_id)
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
        let dice = Dice::new(4, 8);
        let raw = encounter.roll(&dice);
        // Burn the recharge so the touch can't fire again until the d6
        // refresher lands a 4-6 at start-of-turn.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("healing_touch");
        }
        encounter.log(format!(
            "  deva healing touch: {}({}) = {} HP",
            dice, raw, raw
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount: raw,
        })]
    }
}

pub static DEVA_HEALING_TOUCH: LazyLock<DevaHealingTouch> =
    LazyLock::new(|| DevaHealingTouch {});

// ─── Quaggoth ────────────────────────────────────────────────────────

/// Quaggoth Claw — STR-based 1d6+STR slashing melee, vanilla
/// `SimpleWeapon`. The bear-like Underdark thrall's primary natural
/// weapon — paired in the `QUAGGOTH_MULTI` for a two-swing Action.
/// No rider; the quaggoth's whole identity is "berserker that just
/// keeps swinging" — the multiattack chassis carries the per-turn
/// damage envelope. RAW: "Hit: (1d6+3) slashing damage."
pub static QUAGGOTH_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "quaggoth claw",
    &["qclaw", "claw-q"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Quaggoth Multiattack — 2 claw rakes per Action. RAW: "The
/// quaggoth makes two claw attacks." The chassis-driven multi means
/// pack-tactics-style adjacency benefits both swings without per-impl
/// plumbing. Wounded Fury (RAW: "While it has 10 hit points or fewer,
/// the quaggoth has advantage on attack rolls") is omitted as a
/// deliberate scope cut — the engine doesn't yet have a generic
/// "below HP threshold → grant advantage" hook on the attacker side,
/// and adding one for a single low-CR creature would be over-scope.
pub static QUAGGOTH_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "quaggoth multiattack",
    sub_attack: &QUAGGOTH_CLAW,
    count: 2,
});

// ─── Allip ───────────────────────────────────────────────────────────

/// Allip Maddening Touch — STR-based 1d4+STR psychic melee with a
/// DC 13 INT save-or-Charmed rider on hit. The incorporeal spectre of
/// a sage who died from madness drifts close and pushes a sliver of
/// its own broken mind into its victim's. Routes through the shared
/// `WeaponWithSaveCondition` chassis: the save-or-condition install
/// fires only on a confirmed hit, and per-target immunity is handled
/// by the standard `add_condition` chokepoint. INT save (not WIS) per
/// the RAW intelligence-undead flavor — the allip's whisper is a
/// cognitive intrusion, not a fear effect. Charmed is the closest
/// in-engine condition to RAW's "babbling" stun-style debuff: it locks
/// the target out of hostile actions against the allip and routes
/// cleanly through the existing condition pipeline.
pub static ALLIP_MADDENING_TOUCH: WeaponWithSaveCondition =
    WeaponWithSaveCondition::melee(
        "allip maddening touch",
        &["allip-touch", "mad-touch"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Psychic,
        AbilityScoreType::Intelligence,
        13,
        Condition::Charmed,
        ConditionTimer::Rounds(3),
        "babbling madness",
    );

// ─── Giant Octopus ───────────────────────────────────────────────────

/// Giant Octopus Tentacles — STR-based 2d6+STR bludgeoning melee at
/// reach 3 tiles (15 ft — the eight-armed embrace of a Large cephalopod
/// strikes from outside normal melee range). On hit, the target makes a
/// DC 16 STR save vs **Tentacles**: on fail, picks up the `Restrained`
/// condition for 10 rounds. Routes through the shared
/// `WeaponWithSaveCondition` chassis (long-reach variant) alongside the
/// Giant Constrictor Snake's Constrict — same shape, just a different
/// save DC and tile reach.
///
/// RAW: "the target is grappled (escape DC 16). Until this grapple ends,
/// the target is restrained, and the octopus can't use its tentacles on
/// another target." We model the load-bearing portion as `Restrained`
/// directly (which subsumes Grappled's movement-zero and adds the
/// attack-disadvantage + advantage-to-attackers + DEX-save-disadvantage
/// envelope the RAW grapple-then-restrain chain produces). The "can't
/// tentacle another target while holding this one" clause is omitted —
/// the engine has no per-action target-lock and Restrained's stat
/// envelope on the target is the meaningful payoff. Distinct from
/// `Adhered` (Mimic) so cleanse pickers / dispel sweeps target the
/// tentacle grasp specifically.
pub static GIANT_OCTOPUS_TENTACLES: WeaponWithSaveCondition =
    WeaponWithSaveCondition::reach_melee(
        "giant octopus tentacles",
        &["tentacles", "octopus-tentacles", "octo-grab"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Bludgeoning,
        AbilityScoreType::Strength,
        16,
        Condition::Restrained,
        ConditionTimer::Rounds(10),
        "tentacles",
        3,
    );

// ─── Plesiosaurus ────────────────────────────────────────────────────

/// Plesiosaurus Bite — STR-based 3d6+STR piercing melee at reach 2
/// tiles (10 ft — the long-necked aquatic reptile lashes out from
/// outside normal melee range). Vanilla `SimpleWeapon::reach_melee` —
/// the bite is pure damage; the plesiosaurus has no rider clause and
/// relies on its long-necked reach + huge HP bar (CR 2 ~68 HP, the
/// fattest in its CR bracket alongside the Giant Constrictor Snake).
pub static PLESIOSAURUS_BITE: SimpleWeapon = SimpleWeapon::reach_melee(
    "plesiosaurus bite",
    &["plesio-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
    2,
);

// ─── Pteranodon ──────────────────────────────────────────────────────

/// Pteranodon Bite — STR-based 2d4+STR piercing melee. Vanilla
/// `SimpleWeapon::melee` — the flying reptile's snapping beak is pure
/// damage; the pteranodon's threat profile sits on its fly speed
/// (which we collapse to a high ground speed since the engine isn't
/// 3D) rather than a per-swing rider.
pub static PTERANODON_BITE: SimpleWeapon = SimpleWeapon::melee(
    "pteranodon bite",
    &["ptero-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Piercing,
);

// ─── Mastiff ─────────────────────────────────────────────────────────

/// Mastiff Bite — STR-based 1d6+STR piercing melee with a DC 11 STR
/// save-or-Prone trip rider. The CR-⅛ small-dog beast's signature
/// swing: same shape as `WOLF_BITE` (1d4 STR + DC 11 STR-vs-Prone),
/// but a heavier 1d6 die — the mastiff is the "guard dog" upgrade of
/// the wolf-style trip lane. RAW: "If the target is a creature, it
/// must succeed on a DC 11 Strength saving throw or be knocked Prone."
///
/// Routes through the shared `WeaponWithSaveCondition::melee` chassis
/// so the swing + Extra-Attack + save-or-condition loop lives in one
/// place alongside Wolf / Dire Wolf / Worg. Keeps the canonical "bite
/// trips the target" pattern uniform across the four trip-bite holders.
pub static MASTIFF_BITE: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "mastiff bite",
    &["mb", "dog-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
    AbilityScoreType::Strength,
    11,
    Condition::Prone,
    ConditionTimer::Permanent,
    "mastiff trip",
);

// ─── Grimlock ────────────────────────────────────────────────────────

/// Grimlock Spiked Bone Club — STR-based 1d4+STR bludgeoning melee
/// with a flat 1d4 piercing rider on hit. The CR-¼ blind-savage's
/// signature crude weapon: bludgeoning base from the bone shaft +
/// piercing rider from the carved spikes. RAW: +3 to hit, 6 (1d4+3)
/// bludgeoning + 2 (1d4) piercing.
///
/// Routes through the shared `WeaponWithRider::melee` chassis so the
/// "weapon swing + unconditional flat typed-damage rider" loop lives
/// in one place. The bludgeoning/piercing split matters for
/// resistance-aware targets — a fully bludgeoning-resistant skeleton
/// still eats the spike rider at full value, and vice versa for a
/// piercing-resistant target.
pub static GRIMLOCK_SPIKED_CLUB: WeaponWithRider = WeaponWithRider::melee(
    "spiked bone club",
    &["sbc", "bone-club"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
    Dice::new(1, 4),
    DamageType::Piercing,
    "bone spikes",
);

// ─── Giant Frog ──────────────────────────────────────────────────────

/// Giant Frog Bite — STR-based 1d6+STR piercing melee with an
/// auto-Grappled install on hit (no save). The CR-¼ amphibian's
/// signature swing: the tongue snaps out, latches on, and the target
/// is grappled. RAW: "Hit: 4 (1d6 + 1) piercing damage, and the target
/// is grappled (escape DC 11)." The "escape DC" is a later Action the
/// grappled actor can spend, not a prevention save — the install
/// itself is automatic on hit.
///
/// First user of the new `WeaponWithCondition` chassis (companion to
/// `WeaponWithSaveCondition`'s save-gated install). The shape matters:
/// every other "save-or-Grappled" creature in the pool uses a STR DC
/// to *prevent* the grapple at install time, but the frog RAW grapples
/// unconditionally — modeling it as a save-or-grapple would let some
/// targets shrug the install off entirely, breaking the "bite means
/// tongue-stuck" flavor. The Swallow follow-up (RAW: bite again with
/// a grappled Small target → swallow whole) is omitted as scope.
pub static GIANT_FROG_BITE: WeaponWithCondition = WeaponWithCondition::melee(
    "giant frog bite",
    &["gfb", "frog-bite", "tongue"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
    Condition::Grappled,
    ConditionTimer::Permanent,
    "tongue grab",
);

// ─── Hawk ───────────────────────────────────────────────────────────

/// Hawk Talons — DEX-based 1d1 slashing melee. RAW: "Hit: 1 slashing
/// damage." We model the flat 1 via a `Dice::new(1, 1)` so a confirmed
/// crit doubles cleanly to 2 through the engine's uniform crit-
/// doubling chassis instead of needing a flat-1 special case. The
/// load-bearing threat is the hawk's mobility (fly 60, the highest
/// non-dragon flight in the low-CR pool), not the swing — even a
/// crit-doubled talon barely scratches a soft target.
pub static HAWK_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "hawk talons",
    &["talons", "hawk", "rake"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Slashing,
);

// ─── Giant Lizard ───────────────────────────────────────────────────

/// Giant Lizard Bite — STR-based 1d8+STR piercing melee. RAW: "Melee
/// Weapon Attack: +4 to hit, reach 5 ft, one target. Hit: 6 (1d8 + 2)
/// piercing damage." Vanilla `SimpleWeapon` — the CR-¼ large reptile's
/// only swing. The threat profile lives on the 19-HP large frame,
/// not the bite; the lizard is a meat-shield, not a damage dealer.
pub static GIANT_LIZARD_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant lizard bite",
    &["glb", "lizard-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

// ─── Giant Wolf Spider ──────────────────────────────────────────────

/// Giant Wolf Spider Bite — STR-based 1d6+STR piercing melee with a
/// DC 11 CON save-or-2d6-poison rider. Routes through the shared
/// `WeaponWithSaveDamage` chassis alongside Spider Bite / Ettercap
/// Bite — same "single save gates both damage and condition" RAW
/// envelope. RAW: "Hit: 4 (1d6 + 1) piercing damage, and the target
/// must make a DC 11 Constitution saving throw, taking 7 (2d6)
/// poison damage on a failed save, or half as much damage on a
/// successful one." We omit the RAW's "if the poison damage reduces
/// the target to 0 hit points, the target is stable but poisoned for
/// 1 hour" rider — the engine's death-save flow handles 0-HP
/// stabilization separately and the conditional Poisoned install is
/// hard to model through the shared chassis cleanly. The pure
/// save-or-damage envelope captures the load-bearing threat (a
/// failed CON 11 against the venom can drop a wounded target).
pub static GIANT_WOLF_SPIDER_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage {
    display_name: "giant wolf spider bite",
    aliases: &["gwsb", "wolf-spider-bite"],
    attack_ability: AbilityScoreType::Strength,
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    save_ability: AbilityScoreType::Constitution,
    save_dc: 11,
    rider_dice: Dice::new(2, 6),
    rider_type: DamageType::Poison,
    rider_name: "wolf spider venom",
    also_install: None,
};

// ─── Reef Shark ─────────────────────────────────────────────────────

/// Reef Shark Bite — STR-based 1d8+STR piercing melee. RAW: "Hit: 6
/// (1d8 + 2) piercing damage." Vanilla `SimpleWeapon` — the
/// pack-tactics swarmer's only swing. The threat multiplier rides on
/// the template-level `has_pack_tactics: true` flag (advantage when
/// an ally shark is adjacent to the target), not the bite itself,
/// so the dice line matches the CR-½ baseline (sahuagin bite / wolf
/// bite cohort) and three sharks ganging up on the same target roll
/// every bite at advantage.
pub static REEF_SHARK_BITE: SimpleWeapon = SimpleWeapon::melee(
    "reef shark bite",
    &["rsb", "reef-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

// ─── Hunter Shark ───────────────────────────────────────────────────

/// Hunter Shark Bite — STR-based 2d8+STR piercing melee. RAW: "Hit:
/// 13 (2d8 + 4) piercing damage." Vanilla `SimpleWeapon` — the solo
/// hunter's only swing. The snowball multiplier rides on the
/// template-level `BLOOD_FRENZY_TAG` passive (advantage on melee vs
/// wounded targets), not the bite itself. Heavier dice than the
/// reef shark's 1d8 — the hunter shark hits hard alone, the reef
/// shark hits hard in numbers.
pub static HUNTER_SHARK_BITE: SimpleWeapon = SimpleWeapon::melee(
    "hunter shark bite",
    &["hsb", "hunter-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
);

// ─── Giant Shark ────────────────────────────────────────────────────

/// Giant Shark Bite — STR-based 3d10+STR piercing melee. RAW: "Hit:
/// 22 (3d10 + 6) piercing damage." Vanilla `SimpleWeapon` — the apex
/// shark's only swing. The snowball multiplier rides on the
/// template-level `BLOOD_FRENZY_TAG` passive shared with the Hunter
/// Shark / Sahuagin cohort. The 3d10 base die is the heaviest non-
/// reach single-swing in the CR-5 monster pool; combined with Blood
/// Frenzy on a wounded target a giant shark can delete a back-line
/// PC in one Action.
pub static GIANT_SHARK_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant shark bite",
    &["gsb", "giant-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Piercing,
);

// ─── Warhorse ───────────────────────────────────────────────────────

/// Warhorse Hooves — STR-based 2d6+STR bludgeoning melee. RAW: "Hit:
/// 11 (2d6 + 4) bludgeoning damage." Vanilla `SimpleWeapon` — the
/// chunky 2d6 dice carry the warhorse's damage profile alone. No
/// rider on the standalone hooves swing (the RAW Trampling Charge
/// recharge is intentionally omitted as a scope cut at CR ½; the
/// Mammoth at CR 6 already carries the trample-then-stomp two-attack
/// chassis on the heavy huge-beast tier).
pub static WARHORSE_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "warhorse hooves",
    &["wh", "hooves", "stomp"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

// ─── Giant Vulture ──────────────────────────────────────────────────

/// Giant Vulture Beak — STR-based 1d4+STR piercing melee. RAW: "Hit:
/// 4 (1d4 + 2) piercing damage." Light single swing — the carrion bird's
/// damage profile lives in the compound multi with the talons, not the
/// beak alone. Vanilla `SimpleWeapon`.
pub static GIANT_VULTURE_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "giant vulture beak",
    &["gvb", "vulture-beak", "beak"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

/// Giant Vulture Talons — STR-based 2d4+STR slashing melee. RAW: "Hit:
/// 7 (2d4 + 2) slashing damage." Heavier sister swing to the beak —
/// pairs with it in the per-Action compound. Vanilla `SimpleWeapon`.
pub static GIANT_VULTURE_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "giant vulture talons",
    &["gvt", "vulture-talons", "rake"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Giant Vulture's beak + talons compound — same one-Action multi
/// shape as the Owlbear / Werewolf compounds. Routes through the
/// shared `CompoundAttack` chassis so the heterogeneous two-limb
/// pattern lives at one chokepoint.
pub static GIANT_VULTURE_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "giant vulture multiattack",
    parts: vec![(&GIANT_VULTURE_BEAK, 1), (&GIANT_VULTURE_TALONS, 1)],
});

// ─── Giant Bat ──────────────────────────────────────────────────────

/// Giant Bat Bite — STR-based 1d6+STR piercing melee. RAW: "Hit: 5
/// (1d6 + 2) piercing damage." Single swing per Action — the bat's
/// threat profile lives in its mobility (fly 60) and blindsight, not
/// the bite. Vanilla `SimpleWeapon`.
pub static GIANT_BAT_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant bat bite",
    &["gbb", "bat-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

// ─── Giant Centipede ────────────────────────────────────────────────

/// Giant Centipede Bite — DEX-based 1d4+DEX piercing melee with a
/// DC 11 CON save-or-3d6-poison rider via `WeaponWithSaveDamage`. RAW:
/// "Hit: 4 (1d4 + 2) piercing damage, and the target must succeed on
/// a DC 11 Constitution saving throw or take 10 (3d6) poison damage."
/// The "if poison reduces target to 0 HP, the target is stable but
/// poisoned for 1 hour, and paralyzed while poisoned" RAW rider is
/// omitted as a scope cut — the engine's death-save flow handles 0-HP
/// stabilization separately and the conditional Paralyzed install on
/// stable-and-poisoned is hard to model cleanly through the shared
/// chassis. The pure save-or-damage envelope still captures the
/// load-bearing threat (a failed CON 11 against the venom can drop a
/// wounded low-level target in one swing).
pub static GIANT_CENTIPEDE_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "giant centipede bite",
    &["gcb", "centipede-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(3, 6),
    DamageType::Poison,
    "centipede venom",
);

// ─── Vine Blight ────────────────────────────────────────────────────

/// Vine Blight Constrict — STR-based 2d6+STR bludgeoning melee with a
/// DC 12 STR save-or-Grappled+Restrained rider via the shared
/// `WeaponWithSaveCondition` chassis (we model RAW's "grappled and
/// restrained" envelope as the single `Restrained` install — the
/// stronger of the two, since Restrained already zeros movement +
/// gives attackers advantage + DEX-save disadvantage, fully covering
/// the Grappled clause). RAW: "Hit: 9 (2d6 + 2) bludgeoning damage,
/// and a Large or smaller target is grappled (escape DC 12). Until
/// this grapple ends, the target is restrained, and the blight can't
/// constrict another target." The "single-grapple-only" restriction
/// is omitted as a scope cut — the engine's grapple chokepoint doesn't
/// track per-grappler ownership; the install on hit still pins targets
/// for the rest of the pack.
pub static VINE_BLIGHT_CONSTRICT: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "vine blight constrict",
    &["vbc", "constrict", "vines"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    AbilityScoreType::Strength,
    12,
    Condition::Restrained,
    ConditionTimer::Rounds(10),
    "vine constrict",
);

// ─── Twig Blight ────────────────────────────────────────────────────

/// Twig Blight Claws — STR-based 1d4+STR piercing melee. RAW: "+3 to
/// hit, reach 5 ft, one target. Hit: 3 (1d4+1) piercing damage." The
/// CR-⅛ sapling blight's only swing — a fragile dryad-spawn whose
/// threat profile is its 60ft Blindsight (it sees in pitch-black caves
/// or under canopy) plus the per-template fire vulnerability, not the
/// claw dice. Vanilla `SimpleWeapon`. Sister to `NEEDLE_BLIGHT_CLAWS`
/// (2d4 — heavier dice on the medium frame) and the
/// `VINE_BLIGHT_CONSTRICT` grapple-rider entry at CR ½.
pub static TWIG_BLIGHT_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "twig blight claws",
    &["tbc", "twig-claws"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

// ─── Needle Blight ──────────────────────────────────────────────────

/// Needle Blight Claws — STR-based 2d4+STR piercing melee. RAW: "+3 to
/// hit, reach 5 ft, one target. Hit: 6 (2d4+1) piercing damage." The
/// CR-¼ thorn-blight's in-melee swing — pairs with the ranged
/// `NEEDLE_BLIGHT_NEEDLES` shot as the "switch-hitter" plant lane.
/// Heavier than the twig's 1d4 because the needle blight is a Medium
/// upgrade tier, not a fragile sapling. Vanilla `SimpleWeapon`.
pub static NEEDLE_BLIGHT_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "needle blight claws",
    &["nbc", "needle-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Piercing,
);

/// Needle Blight Needles — STR-based 2d6+STR piercing ranged (12 normal
/// / 24 long in tile-gap units = 30/60ft). RAW: "+3 to hit, range
/// 30/60 ft., one target. Hit: 8 (2d6+1) piercing damage." Heavier dice
/// than the claws — the needle volley is the load-bearing ranged option
/// that lets the needle blight pressure back-line targets where the
/// twig blight can't follow. Vanilla `SimpleWeapon::ranged`.
pub static NEEDLE_BLIGHT_NEEDLES: SimpleWeapon = SimpleWeapon::ranged(
    "needle blight needles",
    &["nbn", "needles", "needle-volley"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
    24,
    12,
);

// ─── Giant Boar ─────────────────────────────────────────────────────

/// Giant Boar Tusks — STR-based 2d6+STR slashing melee. RAW: "+6 to
/// hit, reach 5 ft, one target. Hit: 10 (2d6+3) slashing damage." The
/// CR-2 boar's only swing — chunkier dice than the CR-¼ Boar's 1d6
/// shared `BOAR_TUSKS`, on a Large frame with 42 HP. As with the
/// regular Boar, RAW's Charge rider (extra 2d6 + DC-13 STR save vs
/// Prone after a 20 ft straight-line dash) and Relentless trait (drops
/// to 1 HP once per short rest from a lethal hit) are omitted as
/// engine scope cuts — the giant boar still pressures the front line
/// with the chunky 2d6 tusks alone.
pub static GIANT_BOAR_TUSKS: SimpleWeapon = SimpleWeapon::melee(
    "giant boar tusks",
    &["gbt", "giant-tusks"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

// ─── Giant Goat ─────────────────────────────────────────────────────

/// Giant Goat Ram — STR-based 2d4+STR bludgeoning melee. RAW: "+5 to
/// hit, reach 5 ft, one target. Hit: 8 (2d4+3) bludgeoning damage."
/// The CR-½ mountain goat's headbutt. RAW's Charge rider (extra 2d4 +
/// DC-13 STR save vs Prone after a 20 ft straight-line dash) and
/// Sure-Footed trait (advantage on STR/DEX saves vs prone) are omitted
/// for the same straight-line scope reasons that hollow out the Boar's
/// Charge — the engine doesn't model "this turn's move was straight"
/// at attack time. The plain ram swing keeps the goat anchored at the
/// "fast hooved chunky-die melee" silhouette.
pub static GIANT_GOAT_RAM: SimpleWeapon = SimpleWeapon::melee(
    "giant goat ram",
    &["ggr", "ram", "headbutt"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

// ─── Giant Owl ──────────────────────────────────────────────────────

/// Giant Owl Talons — STR-based 2d6+STR slashing melee. RAW: "+3 to
/// hit, reach 5 ft, one creature. Hit: 8 (2d6+1) slashing damage." The
/// CR-¼ aerial scout's only swing — chunky dice on a fragile 19-HP
/// large frame. RAW's Flyby trait (don't provoke OAs when leaving an
/// enemy's reach) and Keen Hearing and Sight (advantage on hearing /
/// sight Perception) are flavor-only at the engine scale: the engine
/// doesn't surface OAs on Disengage-equivalent moves and skill checks
/// don't route through combat. The plain talons swing pinned to the
/// fly-60 speed keeps the giant owl at the "fast aerial harasser"
/// silhouette.
pub static GIANT_OWL_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "giant owl talons",
    &["got", "owl-talons"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

// ─── Giant Poisonous Snake ─────────────────────────────────────────

/// Giant Poisonous Snake Bite — DEX-based 1d4+DEX piercing melee with
/// a DC 11 CON save-or-3d6-poison rider via `WeaponWithSaveDamage`.
/// RAW: "+6 to hit, reach 10 ft, one target. Hit: 6 (1d4+4) piercing
/// damage, and the target must make a DC 11 Constitution saving throw,
/// taking 10 (3d6) poison damage on a failed save, or half as much
/// damage on a successful one." We collapse the half-on-pass to the
/// engine's standard SaveDamagePolicy::HalfOnPass shape implicit in
/// the chassis. Reach 10 ft = 2 tile-gap units — the snake strikes
/// from a coil one tile away (RAW: medium serpent on a 10-ft reach).
pub static GIANT_POISONOUS_SNAKE_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::reach_melee(
    "giant poisonous snake bite",
    &["gpsb", "snake-bite", "venom-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(3, 6),
    DamageType::Poison,
    "serpent venom",
    2,
);

// ─── Killer Whale ───────────────────────────────────────────────────

/// Killer Whale Bite — STR-based 5d6+STR piercing melee. RAW: "+6 to
/// hit, reach 5 ft, one target. Hit: 21 (5d6+4) piercing damage." The
/// CR-3 orca's one-swing apex-bite — the heaviest single-dice die in
/// the CR-3 bench. RAW's Echolocation / Hold Breath (30 min) traits
/// are flavor-only at the encounter scale: the engine doesn't track
/// breath rounds and the Blindsight 60 (echolocation while underwater)
/// reduces to standard Blindsight on the template since most encounters
/// don't gate "underwater". The plain massive bite carries the threat
/// profile alone.
pub static KILLER_WHALE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "killer whale bite",
    &["kwb", "orca-bite"],
    AbilityScoreType::Strength,
    Dice::new(5, 6),
    DamageType::Piercing,
);

// ─── Crawling Claw ──────────────────────────────────────────────────

/// Crawling Claw — STR-based 1d4+STR slashing melee. RAW: "+4 to hit,
/// reach 5 ft, one target. Hit: 4 (1d4+2) slashing damage; or 4 (1d4+2)
/// bludgeoning or piercing damage (claw's choice)." The CR-0 undead
/// minion's only swing — a severed hand scuttling and clawing. We pin
/// the damage type to slashing as the default; the RAW "choose
/// bludgeoning / piercing / slashing per swing" is a minor flavor
/// option the engine doesn't surface. The Turn Immunity trait routes
/// through the template — the Crawling Claw is RAW immune to Turn
/// Undead because it's mindless animated body parts, not a coherent
/// undead spirit. Vanilla `SimpleWeapon`.
pub static CRAWLING_CLAW_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "crawling claw",
    &["cc", "claw-slam"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Slashing,
);

// ─── Riding Horse / Draft Horse ─────────────────────────────────────

/// Riding Horse Hooves — STR-based 2d4+STR bludgeoning melee. RAW: "+4
/// to hit, reach 5 ft, one target. Hit: 8 (2d4+3) bludgeoning damage."
/// The CR-¼ civilian riding-horse's one swing — lighter than the
/// `WARHORSE_HOOVES` 2d6 since the riding horse is bred for transport,
/// not battle. Shared with the Draft Horse template since both ride
/// the same 2d4+STR dice (the draft horse's higher STR mod is the
/// per-template difference). Sister to `WARHORSE_HOOVES` (2d6, CR ½)
/// on the equine ladder.
pub static RIDING_HORSE_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "horse hooves",
    &["hh", "hooves", "kick"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

// ─── Bat ────────────────────────────────────────────────────────────

/// Bat Bite — STR-based 1d1-shape (i.e. flat 1) piercing melee. RAW:
/// "+0 to hit, reach 5 ft, one creature. Hit: 1 piercing damage." The
/// regular Bat's only swing — a CR-0 tiny flier whose threat profile
/// is mobility + echolocation, not damage. 1d1 lets a confirmed crit
/// double cleanly to 2 through the engine's uniform crit-doubling
/// chassis instead of needing a flat-1 special case — same trick the
/// Hawk uses. Sister to `GIANT_BAT_BITE` (1d6, CR ¼) on the bat
/// ladder.
pub static BAT_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bat bite",
    &["bb", "bat", "nip"],
    AbilityScoreType::Strength,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Rat ────────────────────────────────────────────────────────────

/// Rat Bite — STR-based 1d1-shape (flat 1) piercing melee. RAW: "+0
/// to hit, reach 5 ft, one creature. Hit: 1 piercing damage." The
/// CR-0 vermin's only swing — a sewer-rat whose threat profile is
/// "annoying nibble, dies to a stiff breeze." Sister to
/// `GIANT_RAT_BITE` (1d4 + Pack Tactics, CR ⅛) on the rat ladder;
/// the regular rat lacks Pack Tactics because RAW doesn't grant it
/// — flavor "lonely rodent" vs the giant rat's "swarm vermin."
pub static RAT_BITE: SimpleWeapon = SimpleWeapon::melee(
    "rat bite",
    &["rb", "nibble"],
    AbilityScoreType::Strength,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Cat ────────────────────────────────────────────────────────────

/// Cat Claws — DEX-based 1d1-shape (flat 1) slashing melee. RAW: "Hit:
/// 1 slashing damage." The CR-0 tiny climber's only swing — a hearth-
/// cat whose threat profile is mobility + climb 30, not damage. The
/// 1d1 lets a confirmed crit double cleanly to 2 through the engine's
/// uniform crit-doubling chassis, mirroring the Hawk Talons / Bat Bite
/// / Rat Bite shape at this CR tier. Sister to `HAWK_TALONS`
/// (DEX-based slashing) on the CR-0 ladder — same DEX-driven envelope
/// since the cat's load-bearing stat is its Dexterity 15.
pub static CAT_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "cat claws",
    &["cc", "cat", "swipe"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Slashing,
);

// ─── Lizard ─────────────────────────────────────────────────────────

/// Lizard Bite — STR-based 1d1-shape (flat 1) piercing melee. RAW:
/// "Hit: 1 piercing damage." The CR-0 tiny reptile's only swing —
/// the mundane gecko / skink whose threat profile is "ambient
/// dungeon-fauna." Same flat-1 shape as Rat Bite / Bat Bite at this
/// CR tier; sister to `GIANT_LIZARD_BITE` (1d8, CR ¼) on the lizard
/// ladder. Lacks Spider Climb (RAW: 30 climb) — climb speed is
/// flavor-only since the engine collapses ground + climb into a
/// single per-creature speed.
pub static LIZARD_BITE: SimpleWeapon = SimpleWeapon::melee(
    "lizard bite",
    &["lb", "lizard", "nip"],
    AbilityScoreType::Strength,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Weasel ─────────────────────────────────────────────────────────

/// Weasel Bite — DEX-based 1d1-shape (flat 1) piercing melee. RAW:
/// "+5 to hit, reach 5 ft, one creature. Hit: 1 piercing damage."
/// The CR-0 tiny mustelid's only swing. DEX-based (DEX 16 is the
/// load-bearing stat — the weasel is a nimble snake-killer rather
/// than a heavy hitter). Same flat-1 shape as Rat/Bat Bite at this
/// CR tier, but DEX-keyed like the cat's claws / hawk's talons.
pub static WEASEL_BITE: SimpleWeapon = SimpleWeapon::melee(
    "weasel bite",
    &["wb", "weasel", "snap"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Awakened Shrub ─────────────────────────────────────────────────

/// Awakened Shrub Rake — STR-based 1d4-1 slashing melee. RAW: "+1 to
/// hit, reach 5 ft, one target. Hit: 1 (1d4 - 1) slashing damage." The
/// CR-0 small-plant variant of `AWAKENED_TREE_SLAM` — the sapling
/// cousin of the awakened tree. The RAW 1d4-1 die expression makes
/// the floored-at-1 (engine's `max(1)` damage gate) the typical
/// per-swing yield on this CR-0 tier; a confirmed crit doubles the
/// underlying 1d4-1 cleanly through the engine's uniform crit chassis.
/// Vanilla `SimpleWeapon` — no rider; the awakened shrub's threat
/// profile is mobility + plant-resistance envelope, not the swing.
pub static AWAKENED_SHRUB_RAKE: SimpleWeapon = SimpleWeapon::melee(
    "shrub rake",
    &["asr", "rake", "scratch"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Slashing,
);

// ─── Giant Wasp ─────────────────────────────────────────────────────

/// Giant Wasp Sting — DEX-based 1d6+DEX piercing melee with a DC 11
/// CON save-or-3d6-poison-AND-Poisoned rider via the shared
/// `WeaponWithSaveDamage::melee_with_condition` chassis. RAW: "+5 to
/// hit, reach 5 ft, one creature. Hit: 5 (1d6 + 2) piercing damage,
/// and the target must make a DC 11 Constitution saving throw, taking
/// 10 (3d6) poison damage on a failed save, or half as much damage on
/// a successful one. If the poison damage reduces the target to 0
/// hit points, the target is stable but poisoned for 1 hour." We
/// model the conditional 1-hour Poisoned envelope as an unconditional
/// 10-round Poisoned install on a failed save — close enough to the
/// RAW "hit by the venom, attack/ability rolls at disadvantage"
/// envelope, and cheaper than wiring a 0-HP-gated install through the
/// shared chassis. Routes through the same chokepoint as Spider /
/// Ettercap / Drow Poisoned Crossbow on the save-damage-plus-Poisoned
/// chassis lane. The flying-stinger insectoid sibling on the venom
/// bench beside the Giant Centipede / Giant Wolf Spider.
pub static GIANT_WASP_STING: WeaponWithSaveDamage = WeaponWithSaveDamage::melee_with_condition(
    "giant wasp sting",
    &["gws", "wasp-sting", "sting"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    11,
    Dice::new(3, 6),
    DamageType::Poison,
    "wasp venom",
    Condition::Poisoned,
    ConditionTimer::Rounds(10),
);

// ─── Giant Badger ───────────────────────────────────────────────────

/// Giant Badger Bite — STR-based 1d6+STR piercing melee. RAW: "+3 to
/// hit, reach 5 ft, one target. Hit: 4 (1d6 + 1) piercing damage." The
/// single-die half of the badger's bite + 2-claws compound. Vanilla
/// `SimpleWeapon`. Sister to `GIANT_BADGER_CLAWS` (2d4+STR slashing,
/// the heavier rake half) on the same Action.
pub static GIANT_BADGER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant badger bite",
    &["gbb-bite", "badger-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Giant Badger Claws — STR-based 2d4+STR slashing melee. RAW: "+3 to
/// hit, reach 5 ft, one target. Hit: 6 (2d4 + 1) slashing damage." The
/// chunkier rake half of the bite + 2-claws compound. Vanilla
/// `SimpleWeapon`. The 2d4 dice slot the claws as the load-bearing
/// per-swing dice in the badger's compound multi — two claws + a
/// lighter bite per Action averages to ~16 damage at the CR ¼ tier.
pub static GIANT_BADGER_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "giant badger claws",
    &["gbc-claws", "badger-claws"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

/// Giant Badger Multiattack — 1 bite + 2 claws per Action via
/// `CompoundAttack`. RAW: "Multiattack. The badger makes two attacks:
/// one with its bite and one with its claws." We follow the SRD
/// 5.2.1 wording (1 bite + 1 claws) — earlier MM editions varied to
/// "1 bite + 2 claws," but the current SRD is the cleaner shape and
/// avoids overstating the CR-¼ envelope. Heterogeneous compound
/// (piercing + slashing) — both swings carry no rider; the threat
/// profile is the dual-typed damage spread (a piercing-resistant
/// target still eats the slashing claws at full value, and vice
/// versa).
pub static GIANT_BADGER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "giant badger multiattack",
    parts: vec![(&GIANT_BADGER_BITE, 1), (&GIANT_BADGER_CLAWS, 1)],
});

// ─── Camel ──────────────────────────────────────────────────────────

/// Camel Bite — STR-based 1d4 bludgeoning melee, no STR mod to damage.
/// RAW: "+5 to hit, reach 5 ft, one creature. Hit: 2 (1d4) bludgeoning
/// damage" — the SRD entry lists 1d4 flat even though the camel sports
/// STR 16 (+3). Routes through the shared `SimpleWeapon::flat_melee`
/// chokepoint (no damage modifier) — same chassis as Dretch / Lemure /
/// Pseudodragon natural attacks where the RAW damage line is flat dice.
/// Slots beside the Mule / Riding Horse on the docile-pack-animal bench
/// at CR ⅛ — the camel exists as a desert-transport beast, not a
/// combat threat.
pub static CAMEL_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "camel bite",
    &["cb", "camel"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Goat ───────────────────────────────────────────────────────────

/// Goat Ram — STR-based 1d4+STR bludgeoning melee. RAW: "+3 to hit,
/// reach 5 ft, one target. Hit: 3 (1d4 + 1) bludgeoning damage." The
/// CR-0 mundane goat's only swing — a barnyard headbutt that just
/// barely registers on the damage envelope. Sister to
/// `GIANT_GOAT_RAM` (2d4 + chunkier STR mod, CR ½) on the caprid
/// ladder — the regular goat is the lighter-dice / smaller-frame
/// sibling at the floor of the CR ladder. Vanilla `SimpleWeapon` —
/// no Charge rider (same straight-line gap that hollows the Boar /
/// Giant Goat / Warhorse charge ramps).
pub static GOAT_RAM: SimpleWeapon = SimpleWeapon::melee(
    "goat ram",
    // `gr` is already claimed by `greater restoration` (cleric
    // spell) and a horn-charge alias — skip it to keep the alias
    // space unambiguous when a future encounter mixes herbivores
    // and casters.
    &["gtr", "goat", "butt"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Mule ───────────────────────────────────────────────────────────

/// Mule Hooves — STR-based 1d4+STR bludgeoning melee. RAW: "+2 to hit,
/// reach 5 ft, one target. Hit: 4 (1d4 + 2) bludgeoning damage." The
/// CR-⅛ pack mule's only swing — a defensive kick. Lighter dice than
/// the Pony's 2d4 hooves and the Horse cohort's 2d4-with-STR-mod
/// envelope; the mule is bred for hauling cargo, not combat. Vanilla
/// `SimpleWeapon` — RAW's Beast of Burden (counts as Large for carry
/// capacity) and Sure-Footed (advantage on STR/DEX saves vs prone)
/// traits are out-of-scope: the engine doesn't model carry weight and
/// the per-condition save-advantage hook isn't surfaced. Sister to
/// `PONY_HOOVES` (2d4, CR ⅛) and `RIDING_HORSE_HOOVES` (2d4, CR ¼)
/// on the equine / asinine pack-animal ladder.
pub static MULE_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "mule hooves",
    // Skip the would-be "mh" alias — `mh` is already claimed by the
    // `mass heal` spell. The two never sit on the same actor's
    // action list (a mule doesn't cast cleric spells; a cleric
    // doesn't carry mule hooves), but reusing the same short alias
    // across distinct actions is a footgun for the player who maps
    // it to muscle memory across creatures.
    &["mule", "mule-kick", "mhv"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Pony ───────────────────────────────────────────────────────────

/// Pony Hooves — STR-based 2d4+STR bludgeoning melee. RAW: "+2 to hit,
/// reach 5 ft, one target. Hit: 7 (2d4 + 2) bludgeoning damage." The
/// CR-⅛ small mount's only swing. Same 2d4 dice as the Riding Horse
/// chassis, just on a smaller STR mod (+2 vs +3) and a smaller frame.
/// Vanilla `SimpleWeapon`. Sister to `RIDING_HORSE_HOOVES` (2d4, CR ¼)
/// on the equine ladder — the pony is the halfling / gnome-sized
/// civilian mount tier beneath the medium-rider's horse.
pub static PONY_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "pony hooves",
    &["ph", "pony", "pony-kick"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

// ─── Elk ────────────────────────────────────────────────────────────

/// Elk Ram — STR-based 1d6+STR bludgeoning melee. RAW: "+5 to hit,
/// reach 5 ft, one target. Hit: 6 (1d6 + 3) bludgeoning damage." Half
/// of the CR-¼ elk's two-action lane (Ram or Hooves, not both per
/// Action — the elk lacks Multiattack). The lower-dice / harder-hit
/// option vs `ELK_HOOVES` (2d4 — higher average) — the elk tends to
/// pick Hooves when adjacent and Ram when first connecting from a
/// charge. RAW's Charge rider (extra 2d6 + DC-13 STR vs Prone after
/// 20 ft straight-line dash) is omitted as a scope cut alongside the
/// Boar / Giant Boar / Warhorse charge ramps.
pub static ELK_RAM: SimpleWeapon = SimpleWeapon::melee(
    "elk ram",
    // `er` is already claimed by `enlarge` / `expeditious retreat` /
    // beholder `eye ray` — skip the bare short alias and prefix
    // with `elk-` instead so the same actor's action list stays
    // unambiguous when a polymorphed PC or a buffed elk somehow
    // ends up with an `er`-aliased spell too.
    &["elk-ram", "elkr"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

/// Elk Hooves — STR-based 2d4+STR bludgeoning melee. RAW: "+5 to hit,
/// reach 5 ft, one target. Hit: 8 (2d4 + 3) bludgeoning damage. The
/// elk can use this attack only against a prone creature." We drop
/// the prone-only RAW restriction — it would silently take Hooves out
/// of the elk's tool-bag whenever the target isn't already prone, and
/// without the Charge → Ram knock-prone chain the elk has no way to
/// engineer prone targets itself. Promoting Hooves to unconditional
/// keeps the higher-average swing reachable; the engine collapses the
/// Ram-vs-Hooves choice to "whichever the AI picks per turn." Sister
/// to `ELK_RAM` (1d6, lower dice) on the same Action.
pub static ELK_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "elk hooves",
    &["eh", "elk-hooves"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

// ─── Artificer ──────────────────────────────────────────────────────

/// Thunder Gauntlets — Armorer Artificer **Arcane Armor: Guardian**
/// model (subclass level 3, TCE). "Each of the armor's gauntlets counts
/// as a simple melee weapon while you aren't holding anything in it,
/// and it deals 1d8 thunder damage on a hit. A creature hit by the
/// gauntlet has disadvantage on attack rolls against targets other than
/// you until the end of your next turn."
///
/// Intelligence to hit and to damage, because the Armorer's lv3 clause
/// makes the artificer's armor weapons magical and keyed to the class's
/// casting stat — the same "your subclass moves your swing onto your
/// primary" shape the Astral Self Monk's arms and the Battle Smith's
/// infused weapon both take.
///
/// The disadvantage clause is not on the weapon. It is
/// `THUNDER_GAUNTLETS_TAG` on `ON_HIT_CONDITION_MARKS`, stamping
/// `Dueled` — the same back-linked condition Compelled Duel and the
/// Cavalier's Unwavering Mark install, and for the same reason: "you
/// may swing at me freely and at anyone else at disadvantage" is one
/// mechanic, and it already had one home. Routing the gauntlets through
/// the tag rather than the weapon is an approximation with one visible
/// edge — a Guardian armorer holding some *other* melee weapon would
/// also stamp the mark — and the chassis closes it by carrying no other
/// melee weapon at all.
pub static THUNDER_GAUNTLETS: SimpleWeapon = SimpleWeapon::melee(
    "thunder gauntlets",
    &["gauntlets", "tg"],
    AbilityScoreType::Intelligence,
    Dice::new(1, 8),
    DamageType::Thunder,
);

/// Lightning Launcher — Armorer Artificer **Arcane Armor: Infiltrator**
/// model (subclass level 3, TCE). "A gemlike node appears on one of
/// your armored fists or on the chest (your choice). It counts as a
/// simple ranged weapon, with a normal range of 90 feet and a long
/// range of 300 feet, and it deals 1d6 lightning damage on a hit. Once
/// on each of your turns when you hit a creature with it, you can deal
/// an extra 1d6 lightning damage to that target."
///
/// The extra d6 is `LIGHTNING_LAUNCHER_TAG` on
/// `ONCE_PER_TURN_WEAPON_DIE_RIDERS`, which is exactly the cadence RAW
/// writes — "once on each of your turns" — and is the same lane Colossus
/// Slayer, Psychic Blades and Planar Warrior ride. So the launcher's
/// real damage is 2d6 on the turn's first connecting shot and 1d6 on any
/// after it, which is the difference between the Infiltrator model and
/// the Guardian's flat d8: one rewards a single good shot, the other
/// every swing.
///
/// 36 tiles normal / 120 tiles max is RAW's 90/300 ft on the 2.5 ft
/// grid — by a distance the longest reach any PC weapon on the roster
/// carries, and the reason the Infiltrator plays as an artillery
/// chassis while its sibling plays as a tank.
pub static LIGHTNING_LAUNCHER: SimpleWeapon = SimpleWeapon::ranged(
    "lightning launcher",
    &["launcher", "ll"],
    AbilityScoreType::Intelligence,
    Dice::new(1, 6),
    DamageType::Lightning,
    120,
    36,
);

/// Arcane Infused Weapon — Battle Smith Artificer **Battle Ready**
/// (subclass level 3, TCE): "when you attack with a magic weapon, you
/// can use your Intelligence modifier, instead of Strength or Dexterity,
/// for the attack and damage rolls."
///
/// A weapon rather than an attack-ability override lane, for the reason
/// `ASTRAL_ARMS_STRIKE` is: the substitution is total on the chassis
/// that carries it — a Battle Smith swings its infused weapon and
/// nothing else — so a second `SimpleWeapon` says the whole feature in
/// the five fields the struct already has, and every consumer that
/// ranks, chains or names a weapon picks it up for free.
///
/// A longsword's 1d8 slashing, with Intelligence doing the work of
/// Strength. The magic half of "magic weapon" has no separate surface
/// in this engine (nothing on the roster resists non-magical weapon
/// damage as a distinct category), so what survives of RAW is the
/// substitution, which is the half the subclass is played for.
pub static ARCANE_INFUSED_WEAPON: SimpleWeapon = SimpleWeapon::melee(
    "arcane infused weapon",
    &["infused weapon", "aiw"],
    AbilityScoreType::Intelligence,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Force-Empowered Rend — the Battle Smith's **Steel Defender** slam
/// (subclass level 3, TCE). RAW: "+ (your proficiency bonus + your
/// Intelligence modifier) to hit, reach 5 ft, one target. Hit: 1d8 +
/// PB force damage."
///
/// The defender's own Strength stands in for the artificer's modifiers,
/// which is what the engine's stat-block-derived to-hit already
/// computes — the defender template carries STR 14 and the same
/// proficiency band the artificer does, so the number lands where RAW
/// puts it without a bonded-creature modifier lane the engine has no
/// other user for.
pub static FORCE_EMPOWERED_REND: SimpleWeapon = SimpleWeapon::melee(
    "force-empowered rend",
    &["rend", "fer"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Force,
);

/// Force Ballista bolt — the **Eldritch Cannon**'s ranged mode
/// (Artillerist Artificer, subclass level 3, TCE). RAW: "Make a ranged
/// spell attack, originating from the cannon, at one creature or object
/// within 120 feet of it. On a hit, the target takes 2d8 force damage,
/// and if the creature is Large or smaller, it is pushed up to 5 feet
/// away from the cannon."
///
/// The push is dropped and the 2d8 kept: `SimpleWeapon` has no
/// displacement rider, and the cannon is a stationary turret whose
/// whole contribution is a second source of damage on the artificer's
/// team every round. Force typing is the point — nothing on the
/// bestiary's resistance tables blunts it — so the ballista is the
/// cannon mode that keeps working against the elemental and undead
/// matchups that turn the flamethrower off.
///
/// 48 tiles is RAW's 120 ft on the 2.5 ft grid, with no long-range
/// band: RAW gives the attack a flat range rather than a normal/long
/// pair, which is what `normal_range == reach` means.
pub static FORCE_BALLISTA_BOLT: SimpleWeapon = SimpleWeapon::ranged(
    "force ballista",
    &["ballista", "fbb"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 8),
    DamageType::Force,
    48,
    48,
);

// ─── Swarms ─────────────────────────────────────────────────────────
//
// Every swarm's Action is one "Bites" line, and every one of them
// carries the same second clause: "…or half as much damage if the
// swarm has half of its hit points or fewer." None of the statics
// below encode that half. It lives once, on the swinger, at
// `attack::attacker_scoped_damage_reduction`, gated on
// `ActorInstance::is_thinned_swarm` — writing it into five damage
// expressions instead would have meant five places to get the
// threshold wrong.
//
// RAW's reach on all five is 0 ft ("one creature in the swarm's
// space"), which only parses if the swarm is standing in your square.
// The engine's `actor_map` is one id per tile, so a swarm stands
// beside you instead and bites at ordinary melee reach — the closest
// the board can get to being inside your armour.

/// Swarm of Bats Bites — DEX-based 2d4 piercing melee. RAW: "+4 to
/// hit, reach 0 ft., one creature in the swarm's space. Hit: 5 (2d4)
/// piercing damage, or 2 (1d4) piercing damage if the swarm has half
/// of its hit points or fewer."
///
/// The lightest swarm bite on the bench and the only airborne one —
/// the cloud's threat is that it arrives from anywhere on a fly-30
/// speed with blindsight 60, not that any single bat's teeth matter.
pub static SWARM_OF_BATS_BITES: SimpleWeapon = SimpleWeapon::melee(
    "swarm of bats bites",
    &["sbb", "bat-swarm", "bats"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 4),
    DamageType::Piercing,
);

/// Swarm of Rats Bites — STR-based 2d6 piercing melee. RAW: "+2 to
/// hit, reach 0 ft., one target in the swarm's space. Hit: 7 (2d6)
/// piercing damage, or 3 (1d6) piercing damage if the swarm has half
/// of its hit points or fewer."
///
/// The ground-bound cousin of the bat swarm: fatter dice, no flight,
/// no resistances (RAW gives the rat swarm none — a sword swing does
/// cut through rats), which makes it the swarm that actually dies to
/// being hit.
pub static SWARM_OF_RATS_BITES: SimpleWeapon = SimpleWeapon::melee(
    "swarm of rats bites",
    &["srb", "rat-swarm", "rats"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Swarm of Insects Bites — DEX-based 4d4 piercing melee. RAW: "+3 to
/// hit, reach 0 ft., one target in the swarm's space. Hit: 10 (4d4)
/// piercing damage, or 5 (2d4) piercing damage if the swarm has half
/// of its hit points or fewer."
///
/// Four dice on a CR-½ frame — the highest damage-per-CR bite in the
/// swarm family, and the reason a spellcaster who lets one reach them
/// is in real trouble. Four small dice rather than two large ones is
/// also the flattest damage curve on the bench: the insect swarm
/// almost never rolls low.
pub static SWARM_OF_INSECTS_BITES: SimpleWeapon = SimpleWeapon::melee(
    "swarm of insects bites",
    &["sib", "insect-swarm", "insects"],
    AbilityScoreType::Dexterity,
    Dice::new(4, 4),
    DamageType::Piercing,
);

/// Swarm of Quippers Bites — DEX-based 4d6 piercing melee. RAW: "+5
/// to hit, reach 0 ft., one creature in the swarm's space. Hit: 14
/// (4d6) piercing damage, or 7 (2d6) piercing damage if the swarm has
/// has half of its hit points or fewer."
///
/// The heaviest swarm bite, and the one that compounds: the quipper
/// swarm's Blood Frenzy hands it advantage against anything already
/// wounded, so the first bite that lands makes the second likelier.
pub static SWARM_OF_QUIPPERS_BITES: SimpleWeapon = SimpleWeapon::melee(
    "swarm of quippers bites",
    &["sqb", "quipper-swarm", "quippers"],
    AbilityScoreType::Dexterity,
    Dice::new(4, 6),
    DamageType::Piercing,
);

/// Swarm of Poisonous Snakes Bites — DEX-based 2d6 piercing melee
/// with a DC 10 CON save-or-4d6-poison rider. RAW: "+6 to hit, reach 0
/// ft., one creature in the swarm's space. Hit: 7 (2d6) piercing
/// damage, or 3 (1d6) piercing damage if the swarm has half of its hit
/// points or fewer. The target must make a DC 10 Constitution saving
/// throw, taking 14 (4d6) poison damage on a failed save, or half as
/// much damage on a successful one."
///
/// The only swarm whose bite carries a second damage type, and the
/// reason it sits three CR rungs above the bat swarm on nearly the
/// same piercing die. Both halves thin together — RAW attaches the
/// half-strength clause to the piercing line only, but the venom comes
/// out of the same dwindling supply of snakes, and the engine's
/// attacker-scoped lane halves the whole swing rather than picking one
/// damage line out of it.
pub static SWARM_OF_POISONOUS_SNAKES_BITES: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "swarm of poisonous snakes bites",
    &["spsb", "snake-swarm", "snakes"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    10,
    Dice::new(4, 6),
    DamageType::Poison,
    "a knot of venom",
);
