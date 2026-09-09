use std::collections::HashSet;
use crate::engine::areas::AreaShape;
use std::sync::LazyLock;

use crate::engine::attack::{CHARGE_RUN_TILES, ChargeRider, charge_run_tiles};
use crate::{
    actions::action_template::{
        Action, MELEE_REACH, TargetingSchema, actor_has_recharge, bonus_action_only,
        first_target_id, first_target_location, install_condition_on_failed_saves,
        resolve_burst_targets, target_has_condition,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        attack::{AttackParams, resolve_attack},
        dice::Dice,
        encounter::EncounterInstance,
        mastery::{MasteryRider, WeaponMastery},
        saves::SaveDamagePolicy,
        side_effects::{ApplicableSideEffect, DealDamage, Resource},
        swallow::{RegurgitationClause, SwallowProfile},
        types::{AbilityScoreType, Coordinate, DamageType, Size},
        util::tiles_from_feet,
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
    // A rider of no dice is not a rider. Nine of SRD 5.2's forty
    // dragons print a Rend with no elemental clause on it — the
    // metallic wyrmlings and young, whose scales have not caught fire
    // yet — and they ride the same `WeaponWithRider` chassis as the
    // thirty-one that do, because the alternative is two parallel
    // tables of forty rows differing in one field. `Dice::new(0, _)` is
    // how such a row says "no clause", and without this it would log a
    // "0d1(0) = 0 fire rider" line and push a zero-damage `DealDamage`
    // through the whole pipeline — a resistance line, a concentration
    // check and an absorption payout, all for nothing.
    if rider_dice.count == 0 {
        return 0;
    }
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
    // The weapon's own normal range, or `None` for a melee swing. Two
    // rules read it — 5e's long-range disadvantage and Underwater
    // Combat's automatic miss past normal range — and a helper that
    // hard-coded `None` here made both invisible to every ranged
    // weapon on the chassis above it.
    long_range: Option<isize>,
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
        long_range,
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

/// Whether a rider gated on target size reaches `target_id`, logging the
/// refusal when it doesn't.
///
/// SRD 5.2 prints the clause fifty-odd times and always the same way —
/// *"If the target is a Large or smaller creature, it has the Grappled
/// condition"* — so the whole family of rider chassis asks this one
/// question, in the one shape, and says so in one voice. `None` is a
/// clause RAW leaves ungated; every target clears it.
///
/// The log line is why this is a helper rather than an `is_none_or`
/// inline at each chassis. A rider that silently does not fire is
/// indistinguishable at the table from a rider the engine forgot to
/// implement, and this is a rule players actively plan around: a druid
/// who wild-shapes into something Huge specifically to stop being
/// grappled wants to see the hold fail to take.
///
/// A missing actor reads as "no" — the same fail-closed answer every
/// other rider gate in the file gives when the target has left the
/// board mid-resolution.
pub fn rider_reaches_size(
    encounter: &mut EncounterInstance,
    target_id: usize,
    max_target_size: Option<Size>,
    rider_name: &str,
) -> bool {
    let Some(max) = max_target_size else {
        return true;
    };
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    if target.size().is_at_most(max) {
        return true;
    }
    let (name, size) = (target.name().to_string(), target.size());
    encounter.log(format!(
        "  {}: {} is {}, too big for it ({} or smaller only)",
        rider_name, name, size, max
    ));
    false
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
    let save = encounter.roll_save_vs_condition(target_id, save_ability, dc, condition);
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
    let save = encounter.roll_save_vs_condition(target_id, save_ability, dc, Condition::Charmed);
    if save.passed() {
        encounter.log(format!("  {}: target resists the enchantment", rider_name));
        return Vec::new();
    }
    encounter.log(format!("  {}: target is enthralled", rider_name));
    install_condition_with_link(Condition::Charmed, target_id, caster_id, timer)
}

/// The chassis for a single-target save-or-charm — a whole action whose
/// entire payload is one saving throw and, on a failure, the `Charmed`
/// condition pointing back at whoever cast it.
///
/// Five stat blocks print this and the only things that differ between
/// them are the name, the range, the DC and how long it lasts. The
/// vampire's Charming Gaze, the dryad's Fey Charm and the lamia's
/// Intoxicating Touch each used to be forty lines of trait impl around
/// one call to `save_or_charmed_by_caster`, and the pirates' two are
/// what made that indefensible: the same forty lines a fourth and a
/// fifth time would have been most of what those stat blocks are.
///
/// `bonus_action` is the one field that is about tempo rather than
/// flavour, and it earns its place: the Pirate Captain's Captain's
/// Charm is printed under **Bonus Actions**, which means the captain
/// charms *and* takes three swings in the same turn, and a charm that
/// cost it the Action would be a different and much weaker creature.
///
/// No to-hit roll, by RAW and by choice — every printing of this in the
/// book is "Wisdom Saving Throw: DC N, one creature the {monster} can
/// see within {range} feet", one roll and one outcome. The touch-range
/// printings (the lamia) are the same clause with a smaller number, not
/// a melee attack; giving them an attack roll would double-gate a
/// single-payload effect.
pub struct SaveOrCharm {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Range in tiles. RAW's "within 30 feet" is 12 of them; a touch
    /// printing passes `MELEE_REACH`.
    pub reach: isize,
    pub save_ability: AbilityScoreType,
    pub save_dc: i32,
    pub timer: ConditionTimer,
    /// Log tag for the install line, so the log reads "fey charm:
    /// target is enthralled" rather than naming the chassis.
    pub rider_name: &'static str,
    pub bonus_action: bool,
}

impl SaveOrCharm {
    /// The common shape: an Action, a Wisdom save, a range in tiles.
    pub const fn action(
        display_name: &'static str,
        aliases: &'static [&'static str],
        reach: isize,
        save_dc: i32,
        timer: ConditionTimer,
        rider_name: &'static str,
    ) -> Self {
        Self {
            display_name,
            aliases,
            reach,
            save_ability: AbilityScoreType::Wisdom,
            save_dc,
            timer,
            rider_name,
            bonus_action: false,
        }
    }

    /// Builder tail for the printings filed under **Bonus Actions**.
    pub const fn as_bonus_action(self) -> Self {
        Self {
            bonus_action: true,
            ..self
        }
    }
}

impl Action for SaveOrCharm {
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
        // RAW: "one creature the {monster} **can see**". Load-bearing —
        // a charm through a wall is the difference between a lockdown
        // the party can break line of sight to escape and one it can't.
        true
    }
    fn deals_damage(&self) -> bool {
        // Pure install — no HP loss. Keeps the AI's focus-fire pipeline
        // from costing the charm out as a damage lane and reaching for
        // it when whittling would serve better.
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
        if self.bonus_action {
            crate::actions::action_template::bonus_action_only()
        } else {
            crate::actions::action_template::action_only()
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        save_or_charmed_by_caster(
            encounter,
            caster_id,
            target_id,
            self.save_ability,
            self.save_dc,
            self.timer,
            self.rider_name,
        )
    }
}

/// The chassis for a single-target saving throw that hurts — RAW's
/// "{Ability} Saving Throw: DC N, one creature the {monster} can see
/// within {range} feet. Failure: {dice} {type} damage[, and the target
/// has the {condition} condition]. Success: Half damage only."
///
/// The burst family has had a data-only home for this since the dragons
/// arrived; the single-target family did not, so the storm giant's
/// Lightning Strike was sixty lines of `impl Action` around one save and
/// one `DealDamage`, and the guardian naga's Poisonous Spittle would
/// have been sixty more with a condition tacked on the end.
///
/// `condition` hangs off the *same* save as the damage, for the reason
/// the breath chassis gives at more length: a target that made the save
/// took half and kept its eyes; re-rolling would let it do the
/// opposite, which is not a rule anybody wrote.
///
/// `half_on_save` is what separates the two shapes RAW prints. Most of
/// these say "Success: Half damage only", but some say nothing at all
/// on a success, and the difference is a third of the ability's value.
pub struct SingleTargetSaveDamage {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Range in tiles.
    pub reach: isize,
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    /// Installed on a failed save, with the damage.
    pub condition: Option<(Condition, ConditionTimer)>,
    pub half_on_save: bool,
    pub bonus_action: bool,
}

impl SingleTargetSaveDamage {
    pub const fn new(
        display_name: &'static str,
        aliases: &'static [&'static str],
        reach: isize,
        save_ability: AbilityScoreType,
        dc: i32,
        damage_dice: Dice,
        damage_type: DamageType,
    ) -> Self {
        Self {
            display_name,
            aliases,
            reach,
            save_ability,
            dc,
            damage_dice,
            damage_type,
            condition: None,
            half_on_save: true,
            bonus_action: false,
        }
    }

    /// Builder tail for the printings whose failure clause has a second
    /// half — the naga's spittle blinds as well as poisons.
    pub const fn and_condition(self, condition: Condition, timer: ConditionTimer) -> Self {
        Self {
            condition: Some((condition, timer)),
            ..self
        }
    }

    /// Builder tail for the printings whose success clause is silent —
    /// RAW says what happens on a failure and nothing at all about a
    /// save, so a target that makes it takes nothing.
    ///
    /// `new` defaults the other way because "Success: Half damage only"
    /// is the shape the bestiary prints far more often, and the doc on
    /// `half_on_save` has always said both shapes exist. This is the
    /// second one finally arriving as a builder rather than as a struct
    /// literal: Faithful Hound's bite is the first caller, and spelling
    /// eleven fields out by hand to flip one boolean is how the next
    /// caller would quietly have picked up a different default for
    /// something else.
    pub const fn save_negates(self) -> Self {
        Self {
            half_on_save: false,
            ..self
        }
    }

    pub const fn as_bonus_action(self) -> Self {
        Self {
            bonus_action: true,
            ..self
        }
    }
}

impl Action for SingleTargetSaveDamage {
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
        // RAW: "one creature the {monster} can see".
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }
    /// Three quarters of the pool, the same estimate every other
    /// save-for-half action in the engine makes: half the time it lands
    /// whole and half the time it lands halved, which averages three
    /// quarters, and that is the number the attack picker needs to rank
    /// this honestly against a swing. A save-for-nothing printing gets
    /// the full pool, because on a failure it is the full pool and on a
    /// success it is a wasted action either way.
    fn expected_damage(&self, _encounter: &EncounterInstance, _caster_id: usize) -> Option<f32> {
        let pool = self.damage_dice.average_roll();
        Some(if self.half_on_save { pool * 0.75 } else { pool })
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        if self.bonus_action {
            crate::actions::action_template::bonus_action_only()
        } else {
            crate::actions::action_template::action_only()
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
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let raw = encounter.roll(&self.damage_dice);
        let save = encounter.roll_save_against_caster(
            target_id,
            self.save_ability,
            self.dc,
            caster_id,
        );
        let passed = save.passed();
        // Through the shared post-save chokepoint, so Evasion and the
        // caster-side damage primes read this action the way they read
        // every other save-for-half.
        let damage = encounter.resolve_post_save_damage(
            caster_id,
            target_id,
            self.save_ability,
            if self.half_on_save {
                crate::engine::saves::SaveDamagePolicy::HalfOnSave
            } else {
                crate::engine::saves::SaveDamagePolicy::NoneOnSave
            },
            raw,
            passed,
        );
        encounter.log(format!(
            "  {}: {}({}) = {} {}{}",
            self.display_name,
            self.damage_dice,
            raw,
            damage,
            self.damage_type,
            if passed { " (saved)" } else { "" }
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if damage > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: damage,
                damage_type: self.damage_type,
            }));
        }
        if let Some((condition, timer)) = self.condition
            && !passed
        {
            effects.extend(crate::engine::side_effects::install_condition_with_link(
                condition, target_id, caster_id, timer,
            ));
        }
        effects
    }
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

/// Resolve one swing of `weapon`, mastery property and all.
///
/// The `SimpleWeapon` chassis's own entry point, and the reason it takes
/// the weapon rather than thirteen of its fields: `simple_weapon_attack_ranged`
/// below already carries eleven parameters for the bespoke monster
/// swings that have no struct to hand over, and a mastery-aware sibling
/// spelled the same way would have carried thirteen — two of which
/// (`mastery`, `reach`) exist only for the chassis that has them
/// sitting in a field.
///
/// The mastery property is still inert for a wielder without the class
/// feature; that gate lives inside `MasteryRider`, so this function
/// hands the tag over unconditionally and the rider decides.
fn simple_weapon_swing(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    weapon: &SimpleWeapon,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(target_id) = first_target_id(target_ids) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let attack_bonus = caster.spell_attack_modifier(weapon.attack_ability);
    let damage_bonus = weapon
        .damage_ability
        .map(|a| caster.ability_modifier(a))
        .unwrap_or(0);
    // SRD 5.2's "or N (XdY + mod) damage if the target is Bloodied"
    // clause — a die *swap*, not a rider, so the bigger die replaces
    // the smaller one rather than joining it. Read here rather than at
    // the attack chokepoint because it is a property of the weapon
    // (`SimpleWeapon::bloodied_dice`) and every other weapon chassis
    // that wanted it would be handing `resolve_attack` the same already-
    // resolved `damage_dice` field this one does.
    let damage_dice = match weapon.bloodied_dice {
        Some(escalated)
            if encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.is_bloodied()) =>
        {
            escalated
        }
        _ => weapon.damage_dice,
    };
    crate::engine::attack::resolve_attack_with_rider(
        encounter,
        AttackParams {
            caster_id,
            target_id,
            action_name: weapon.display_name,
            attack_bonus,
            damage_dice,
            damage_bonus,
            damage_type: weapon.damage_type,
            is_melee: weapon.is_melee,
            long_range: weapon.normal_range,
            min_range: weapon.min_effective_range,
            is_spell: false,
        },
        // The reach the rider needs is Cleave's "within your reach",
        // which is the weapon's own — a glaive carries further into the
        // second creature than a greataxe does.
        &MasteryRider::with_reach(weapon.mastery, weapon.attack_ability, weapon.reach),
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
    prone_follow_up: None,
    max_target_size: Some(Size::Medium),
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
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
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
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
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
    prone_follow_up: None,
    max_target_size: None,
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
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
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
    prone_follow_up: None,
    max_target_size: None,
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
    prone_follow_up: None,
    max_target_size: None,
};

/// Triceratops **Trampling Charge** (RAW): no extra damage, "that target
/// must succeed on a DC 13 Strength saving throw or be knocked prone. If
/// the target is prone, the triceratops can make one stomp attack
/// against it as a bonus action."
///
/// Both halves now. The stomp is reach 1 against a gore that reaches 2,
/// so a triceratops that flattened somebody at the far edge of its horns
/// gets the knockdown and not the stamp — `try_fire_charge_follow_up`
/// measures the follow-up limb's own reach rather than assuming the
/// charge's.
pub const TRICERATOPS_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("gore"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "trampling charge",
    knockdown_label: "trampling charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("stomp"),
    max_target_size: Some(Size::Huge),
};

/// Warhorse **Trampling Charge** (RAW): DC 14 Strength or prone, "and if
/// that target is prone, the horse can make another attack with its
/// hooves against it as a bonus action."
///
/// The one clause in the bestiary whose follow-up is the same limb the
/// charge rides, which makes it the one that could re-enter the charge
/// path. It terminates on the bonus action rather than on a flag — see
/// `try_fire_charge_follow_up`.
pub const WARHORSE_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("warhorse hooves"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "warhorse trampling charge",
    knockdown_label: "warhorse trampling charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("warhorse hooves"),
    max_target_size: Some(Size::Large),
};

/// Tiger **Pounce** (RAW): "If the tiger moves at least 20 feet straight
/// toward a creature and then hits it with a claw attack on the same
/// turn, that target must succeed on a DC 13 Strength saving throw or be
/// knocked prone. If the target is prone, the tiger can make one bite
/// attack against it as a bonus action."
///
/// The cats are the reason `prone_follow_up` names its limb instead of
/// reusing the charge's: the pounce rides the claws and pays out in the
/// bite, which is a strictly bigger die on every one of them.
pub const TIGER_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "tiger pounce",
    knockdown_label: "tiger pounce knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("bite"),
    max_target_size: None,
};

/// Lion **Pounce** (RAW): identical to the tiger's, on the lion's own
/// claw, down to the bite it pays out in. Two constants rather than one
/// shared `POUNCE` because the log line names the cat, and because the
/// two stat blocks are free to drift apart the way the boar and the
/// giant boar already have.
pub const LION_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "lion pounce",
    knockdown_label: "lion pounce knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("bite"),
    max_target_size: None,
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
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
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
    prone_follow_up: None,
    max_target_size: Some(Size::Medium),
};

/// Saber-toothed Tiger **Pounce** (RAW): twenty feet, claw attack, DC 14
/// Strength or prone, then one bite as a bonus action against a target
/// it flattens. The same clause the ordinary tiger and the lion carry,
/// on a much heavier cat — and the reason the follow-up is named per row
/// rather than shared: this cat's limbs are `"saber claws"` and
/// `"saber bite"`, not the plain pair its smaller cousins swing.
pub const SABER_TIGER_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("saber claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "saber-toothed pounce",
    knockdown_label: "saber-toothed pounce knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("saber bite"),
    max_target_size: None,
};

/// Mammoth **Trampling Charge** (RAW): twenty feet, gore attack, DC 18
/// Strength or prone — and then a bonus stomp against a target it
/// flattens, the same second half the triceratops and the warhorse
/// carry.
///
/// The mammoth's stomp is the one follow-up that is *also* prone-gated
/// on its own account (`MammothStomp::custom_validate_input`), which is
/// what forced the follow-up to be queued behind the knockdown rather
/// than swung beside it: fired inline it would have found an upright
/// target and declined.
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
    prone_follow_up: Some("mammoth stomp"),
    max_target_size: Some(Size::Huge),
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
    /// 5e's **light** weapon property — the one that opens two-weapon
    /// fighting. Set on the small armoury weapons RAW marks light
    /// (dagger, shortsword, scimitar, handaxe, light hammer, sickle,
    /// club) and left `false` on everything else, including every
    /// natural weapon: RAW's light property belongs to objects you
    /// hold in a hand, and a bite is not one.
    ///
    /// Read through `Action::is_light_melee_weapon` at the stack's
    /// execution chokepoint, which stamps the swinger's per-turn
    /// ledger; `OffHandAttack` gates on that ledger. Nothing else
    /// reads it, and in particular the swing itself is unchanged — a
    /// light weapon rolls exactly as it did before.
    ///
    /// Defaulted to `false` by every constructor and turned on with
    /// the `light()` builder, for the same reason `gated_on` is a
    /// builder: the property is orthogonal to all four shapes above
    /// it, and pairing it with each would be four more near-identical
    /// constructors to keep in step.
    pub is_light: bool,
    /// 5e (2024 / SRD 5.2) **weapon mastery** property — the one clause
    /// printed beside this weapon in the armoury table, or `None` for
    /// the natural weapons and improvised objects RAW never lists.
    ///
    /// Inert unless the wielder has the Weapon Mastery class feature,
    /// which is checked at the single chokepoint
    /// `engine::mastery::effective_mastery` rather than here: these
    /// statics are shared between the bestiary and the class templates
    /// — the fighter's `SCIMITAR` *is* the goblin's — so a property
    /// that fired off weapon data alone would hand the whole bestiary a
    /// feature RAW gives five classes.
    ///
    /// Defaulted to `None` by every constructor and set with the
    /// `mastery()` builder, for the same reason `is_light` is: the tag
    /// is orthogonal to all four weapon shapes, and pairing it with
    /// each would be four more near-identical constructors.
    pub mastery: Option<WeaponMastery>,
    /// The die this weapon rolls *instead of* `damage_dice` when the
    /// target is Bloodied, or `None` for the ordinary weapon that hits
    /// a wounded creature exactly as hard as a fresh one.
    ///
    /// SRD 5.2 writes this as a second half on the Hit line rather than
    /// as a trait — *"Hit: 4 (1d4 + 2) Piercing damage, or 6 (1d8 + 2)
    /// Piercing damage if the target is Bloodied"* — which is why it
    /// belongs to the weapon and not to the creature holding it. It is
    /// a swap and not a rider: the bigger die replaces the smaller one,
    /// so a bloodied blood hawk beak is `1d8 + DEX` and never
    /// `1d4 + 1d8 + DEX`. Modeling it as a `+1d4` rider would have been
    /// the easier change and would have been wrong by a point of
    /// average damage in the direction that matters — upward, on the
    /// swing that finishes people.
    ///
    /// Read once, in `simple_weapon_swing`, against the target's own
    /// `is_bloodied` at the moment the swing resolves. That timing is
    /// the rule: a hawk that opens a turn against a healthy target and
    /// closes it against a bloodied one escalates on the second swing,
    /// because RAW asks about the target and not about the turn.
    ///
    /// Deliberately absent from `expected_damage`, which the AI's attack
    /// picker ranks on. That estimate is handed a caster and no target,
    /// so a clause that asks about the target cannot be priced there at
    /// all — the escalation reads as a pure under-estimate. Harmless
    /// for the one carrier (the blood hawk has a single attack, so
    /// there is nothing for the estimate to lose a ranking to), and
    /// worth writing down before a second carrier arrives with a choice
    /// to make.
    ///
    /// Defaulted to `None` by every constructor and set with the
    /// `escalating_vs_bloodied()` builder, for the same reason
    /// `mastery` and `is_light` are.
    pub bloodied_dice: Option<Dice>,
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
            is_light: false,
            mastery: None,
            bloodied_dice: None,
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
            is_light: false,
            mastery: None,
            bloodied_dice: None,
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
            is_light: false,
            mastery: None,
            bloodied_dice: None,
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
    /// keep in step.
    ///
    /// Every field but the one being set is copied across verbatim.
    /// That is worth stating because it did not used to be true:
    /// `min_effective_range` was written as a literal `None` here
    /// rather than as `self.min_effective_range`, so gating a weapon
    /// silently discarded its point-blank penalty. Nothing in the
    /// bestiary paired the two — the only gated weapon is the Astral
    /// Self Monk's arms and the only min-range weapon is the lance —
    /// so the bug had no victim yet, which is exactly the kind that
    /// waits for the first lance somebody has to summon.
    pub const fn gated_on(self, condition: Condition) -> Self {
        Self {
            requires_condition: Some(condition),
            ..self
        }
    }

    /// Const builder that marks an already-constructed weapon 5e
    /// **light** — `SimpleWeapon::melee(...).light()`.
    ///
    /// A builder for the same reason `gated_on` is one: the property
    /// is orthogonal to every shape above it. See `is_light` for what
    /// reads it.
    pub const fn light(self) -> Self {
        Self {
            is_light: true,
            ..self
        }
    }

    /// Const builder that stamps a weapon with its 5e mastery property
    /// — `SimpleWeapon::melee(...).mastery(WeaponMastery::Sap)`.
    ///
    /// A builder for the same reason `light` and `gated_on` are: the
    /// property is orthogonal to every shape above it, and RAW prints
    /// one for melee weapons, ranged weapons and thrown weapons alike.
    /// Carried across `thrown()` untouched — a thrown handaxe still
    /// vexes, because RAW's mastery is a property of the object rather
    /// than of what was done with it.
    pub const fn mastery(self, mastery: WeaponMastery) -> Self {
        Self {
            mastery: Some(mastery),
            ..self
        }
    }

    /// Const builder that gives a weapon a second, larger damage die it
    /// rolls against a Bloodied target — `SimpleWeapon::melee(...)
    /// .escalating_vs_bloodied(Dice::new(1, 8))`.
    ///
    /// SRD 5.2's *"or 6 (1d8 + 2) Piercing damage if the target is
    /// Bloodied"* clause, which the blood hawk's beak is the roster's
    /// only carrier of today. A builder rather than a constructor for
    /// the same reason `mastery` and `light` are: the clause is
    /// orthogonal to all four weapon shapes, and RAW could as easily
    /// print it on a reach weapon or a bow as on the melee swing that
    /// happens to have it.
    ///
    /// See `bloodied_dice` for why the die is swapped rather than added.
    pub const fn escalating_vs_bloodied(self, bloodied_dice: Dice) -> Self {
        Self {
            bloodied_dice: Some(bloodied_dice),
            ..self
        }
    }

    /// Const builder that turns a melee weapon into its **thrown**
    /// twin — `SimpleWeapon::melee(...).thrown("thrown dagger",
    /// &["td"], 8, 24)`.
    ///
    /// 5e's thrown property: *"you can throw the weapon to make a
    /// ranged attack. If the weapon is a melee weapon, you use the
    /// same ability modifier for that attack roll and damage roll that
    /// you would use for a melee attack with the weapon."* That last
    /// sentence is why this is a builder off the melee weapon rather
    /// than a separate `ranged()` call: the ability, the die and the
    /// damage type are not chosen for the throw, they are *inherited*
    /// from the swing, and a second declaration is a second place for
    /// them to drift. A javelin whose thrown twin rolled DEX would be
    /// a bug nobody would find, because both halves would look right
    /// in isolation.
    ///
    /// What the throw does change: it is a shot rather than a swing
    /// (`is_melee: false`), it needs to see where it is going
    /// (`requires_los: true`), and it carries 5e's two ranges — beyond
    /// `normal_range` the shot is at disadvantage, beyond `reach` it
    /// cannot be made at all. Both in tile-gap units on the engine's
    /// 2.5-ft grid, so RAW's 20/60 ft is `(8, 24)`.
    ///
    /// `is_light` is deliberately carried across rather than cleared.
    /// A thrown dagger is still a light weapon — RAW's properties are
    /// properties of the object, not of what you did with it — and
    /// `is_light_melee_weapon` already ands the flag with `is_melee`,
    /// so a throw opens no off-hand swing regardless.
    ///
    /// `min_effective_range` is cleared, and that one is not
    /// bookkeeping: it is the lance's "disadvantage inside 5 feet",
    /// which is a fact about a weapon braced under an arm at a gallop
    /// and means nothing about the same weapon in flight. No thrown
    /// weapon in RAW has a minimum range.
    ///
    /// Ammunition is not tracked, here or anywhere in the engine, so a
    /// thrown weapon can be thrown every round without running out.
    /// That is a real divergence and a deliberate one: RAW's answer is
    /// that you walk over and pick it up, which is a move action in a
    /// system with no inventory to put it back into.
    pub const fn thrown(
        self,
        display_name: &'static str,
        aliases: &'static [&'static str],
        normal_range: isize,
        long_range: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            reach: long_range,
            is_melee: false,
            requires_los: true,
            normal_range: Some(normal_range),
            min_effective_range: None,
            ..self
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
    fn normal_range(&self) -> Option<isize> {
        self.normal_range
    }
    fn is_weapon_attack(&self) -> bool {
        true
    }
    /// Declared rather than derived, because this weapon *knows*.
    ///
    /// `Action::is_melee_attack`'s default infers the answer from
    /// `requires_los` and a reach band, which is the best a bespoke
    /// `impl Action` can do and strictly worse than reading the field
    /// the weapon already carries — the same `is_melee` that
    /// `resolve_attack` is handed as `AttackParams::is_melee`. Two
    /// consumers were reading the inference where the truth was one
    /// field away, and they disagreed with the die.
    ///
    /// The kraken found it. Its tentacle reaches six tiles, and
    /// `MELEE_BAND_REACH` stops at four — so the longest arm in the
    /// bestiary read as a *ranged* attack to everything that asked the
    /// trait. Three things followed from that, none of them visible as
    /// an error: `has_ranged_attack` counted the kraken as a shooter,
    /// so the kiting rungs backed the engine's heaviest melee thresher
    /// out of contact to use a weapon it swings; the attack picker
    /// scored the tentacle under `compute_attack_mode`'s *ranged*
    /// clauses, so a kraken with something in its face believed its own
    /// tentacle was at disadvantage when the die had never said so; and
    /// the underwater verdict judged a tentacle by the rules for bows.
    /// The swing itself was always resolved correctly — only every
    /// decision leading up to it was made on a wrong answer.
    fn is_melee_attack(&self) -> bool {
        self.is_melee
    }
    /// Both halves of the question, and the `is_melee` half is not
    /// redundant: a dagger and a handaxe are light *and* throwable, so
    /// the same weapon can be declared twice — once as a swing and
    /// once as a toss — and only the swing opens the bonus attack.
    fn is_light_melee_weapon(&self) -> bool {
        self.is_light && self.is_melee
    }
    fn weapon_mastery(&self) -> Option<WeaponMastery> {
        self.mastery
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
            simple_weapon_swing(e, caster_id, target_ids, self)
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
    /// 5e's "normal range" — the band beyond which a shot rolls at
    /// disadvantage, and beyond which Underwater Combat says it misses
    /// outright. `None` for every melee entry, where reach *is* the
    /// range; `Some` for every ranged one, which
    /// `every_ranged_weapon_declares_the_normal_range_two_rules_read`
    /// enforces across the whole bestiary.
    pub normal_range: Option<isize>,
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
            normal_range: None,
            rider_dice,
            rider_type,
            rider_name,
        }
    }

    /// Const constructor for the "shot with a flat typed rider" shape —
    /// the Hobgoblin Captain's poisoned longbow, and the first thing on
    /// this chassis that is not a swing.
    ///
    /// The chassis has carried an `is_melee` field and an
    /// `is_melee`-gated line-of-sight rule since it was written; what it
    /// had no way to build was a `WeaponWithRider` with that field set
    /// to false, so every ranged weapon whose hit adds a second damage
    /// type had to route through `WeaponWithSaveDamage` and invent a
    /// save the stat block does not print.
    ///
    /// `normal_range` is not optional at this constructor, and the
    /// engine has a sweep that says so: two separate rules read it —
    /// 5e's long-range disadvantage and Underwater Combat's automatic
    /// miss past normal range — and a ranged weapon that left it unset
    /// would be a bow with no falloff that also works at the bottom of
    /// a lake. It would look like a weapon that was simply good.
    #[allow(clippy::too_many_arguments)]
    pub const fn ranged(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        rider_dice: Dice,
        rider_type: DamageType,
        rider_name: &'static str,
        reach: isize,
        normal_range: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: false,
            normal_range: Some(normal_range),
            rider_dice,
            rider_type,
            rider_name,
        }
    }

    /// Builder tail turning a swing into the same weapon in flight —
    /// the sibling of `SimpleWeapon::thrown`, and here for the same
    /// reason.
    ///
    /// RAW's "Melee or Ranged Attack Roll" weapons are one weapon with
    /// two ranges, and everything else about them — the ability, the
    /// die, the damage type, and on this chassis the rider — is shared
    /// by definition. Writing the throw as a second literal is writing
    /// those facts twice, and a vampire familiar's thrown Umbral Dagger
    /// that had quietly lost its necrotic rider would read as correct
    /// in either half taken alone.
    ///
    /// Consumes `self`, so the source is a `const` profile rather than
    /// a `static`; see `DAGGER_PROFILE` for why that distinction is
    /// what makes the two statics two distinct objects.
    pub const fn thrown(
        self,
        display_name: &'static str,
        aliases: &'static [&'static str],
        normal_range: isize,
        long_range: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            reach: long_range,
            is_melee: false,
            normal_range: Some(normal_range),
            ..self
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

    fn requires_los(&self) -> bool {
        // Ranged variants need LOS like every other ranged attack; melee
        // doesn't. Matches `SimpleWeapon`'s `is_melee`-gated LOS rule.
        !self.is_melee
    }
    fn normal_range(&self) -> Option<isize> {
        self.normal_range
    }

    /// Declared rather than derived, for the same reason
    /// `SimpleWeapon` declares it: this chassis *knows*, and the
    /// trait's default is a guess made by something that doesn't.
    ///
    /// Every swing on this chassis resolves through `resolve_attack`
    /// with `is_spell: false`, which is the engine's own definition of
    /// a weapon attack — and the attack site reads exactly that when it
    /// asks 5e's Underwater Combat rules what the water does to the
    /// swing. This method is the *other* copy of that answer, the one
    /// the AI's attack picker has to use because it holds a `&dyn
    /// Action` and never sees an `AttackParams`.
    ///
    /// So the bug this fixes is a divergence rather than a missing
    /// rule: four whole chassis — every save-rider and flat-rider
    /// natural weapon in the bestiary, plus the drow's hand crossbow —
    /// answered `false` here while the die answered `true` down there.
    /// The picker therefore ranked a shark's bite in its own ocean as
    /// though the water cost it nothing, and then the die rolled it at
    /// disadvantage; and a ranged shot the water makes impossible was
    /// never dropped from the candidate list, because the clause that
    /// drops it is the one clause the picker owns outright.
    ///
    /// Prediction and resolution disagreeing is worse than either being
    /// wrong alone. The picker's whole job is to choose between swings
    /// by what they will do, and it was choosing by a rule the engine
    /// does not use.
    fn is_weapon_attack(&self) -> bool {
        true
    }

    /// Declared from the field rather than inferred from `requires_los`
    /// and a reach band — see `SimpleWeapon::is_melee_attack` for the
    /// same argument. The inference is right for every entry on this
    /// chassis today; the field is right by construction.
    fn is_melee_attack(&self) -> bool {
        self.is_melee
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
                self.normal_range,
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
    /// The largest target the rider reaches, or `None` for a clause RAW
    /// leaves ungated. Set with `against_at_most`.
    ///
    /// Same field, same meaning, and the same RAW sentence as
    /// `WeaponWithCondition::max_target_size` — the difference between
    /// the two chassis is only whether the target gets a save, and the
    /// size clause sits outside that either way. The gate is checked
    /// *before* the save is rolled, because a target the clause cannot
    /// reach is not a target that rolled well: no die should be spent,
    /// and no "target fails the save" line should appear for a hold
    /// that was never on offer.
    pub max_target_size: Option<Size>,
    /// A second condition the same failed save installs, or `None` for
    /// the forty-odd riders that land one thing.
    ///
    /// RAW writes some riders as one sentence with two consequences —
    /// the bearded devil's beard is "the target has the Poisoned
    /// condition… Until this poison ends, the target can't regain Hit
    /// Points", which is one save, one duration, and two flags. The
    /// alternative to this column was a second chassis, or a
    /// hand-written `Action` impl per creature, for a clause that is
    /// otherwise identical to the one already here.
    ///
    /// Installed with the *same* timer as `condition`, deliberately:
    /// every RAW clause of this shape ties the second effect to the
    /// first's duration ("until this poison ends"), and a rider whose
    /// halves could expire separately would need a way to say which
    /// outlives which — which no stat block on the roster asks for.
    ///
    /// Set with `and_also`.
    pub also_installs: Option<Condition>,
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
            max_target_size: None,
            also_installs: None,
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
            max_target_size: None,
            also_installs: None,
        }
    }

    /// Chainable: RAW gates this rider on the target's size. Mirrors
    /// `WeaponWithCondition::against_at_most` — see that method for why
    /// the gate is a builder rather than another positional argument.
    pub const fn against_at_most(mut self, max: Size) -> Self {
        self.max_target_size = Some(max);
        self
    }

    /// Chainable: this rider's failed save lands a second condition
    /// alongside the first, on the same timer. See `also_installs`.
    pub const fn and_also(mut self, extra: Condition) -> Self {
        self.also_installs = Some(extra);
        self
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

    fn requires_los(&self) -> bool {
        // Ranged variants need LOS like every other ranged attack; melee
        // doesn't. Matches `SimpleWeapon`'s `is_melee`-gated LOS rule.
        !self.is_melee
    }

    /// Declared rather than derived, for the same reason
    /// `SimpleWeapon` declares it: this chassis *knows*, and the
    /// trait's default is a guess made by something that doesn't.
    ///
    /// Every swing on this chassis resolves through `resolve_attack`
    /// with `is_spell: false`, which is the engine's own definition of
    /// a weapon attack — and the attack site reads exactly that when it
    /// asks 5e's Underwater Combat rules what the water does to the
    /// swing. This method is the *other* copy of that answer, the one
    /// the AI's attack picker has to use because it holds a `&dyn
    /// Action` and never sees an `AttackParams`.
    ///
    /// So the bug this fixes is a divergence rather than a missing
    /// rule: four whole chassis — every save-rider and flat-rider
    /// natural weapon in the bestiary, plus the drow's hand crossbow —
    /// answered `false` here while the die answered `true` down there.
    /// The picker therefore ranked a shark's bite in its own ocean as
    /// though the water cost it nothing, and then the die rolled it at
    /// disadvantage; and a ranged shot the water makes impossible was
    /// never dropped from the candidate list, because the clause that
    /// drops it is the one clause the picker owns outright.
    ///
    /// Prediction and resolution disagreeing is worse than either being
    /// wrong alone. The picker's whole job is to choose between swings
    /// by what they will do, and it was choosing by a rule the engine
    /// does not use.
    fn is_weapon_attack(&self) -> bool {
        true
    }

    /// Declared from the field rather than inferred from `requires_los`
    /// and a reach band — see `SimpleWeapon::is_melee_attack` for the
    /// same argument. The inference is right for every entry on this
    /// chassis today; the field is right by construction.
    fn is_melee_attack(&self) -> bool {
        self.is_melee
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
            // Ahead of the save, not after it: a target the clause
            // cannot reach never had a save to make, and rolling one
            // would put a "target fails the save" line in the log for a
            // rider that was never on offer.
            if !rider_reaches_size(e, target_id, self.max_target_size, self.rider_name) {
                return effects;
            }
            let save = save_or_condition_rider(
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
            // The second half of a two-flag clause, on the same failed
            // save and the same timer. Queued as an `ApplyCondition`
            // rather than through the linked installer because nothing
            // on this lane carries a back-link — the companion flags are
            // states of the victim, not relationships to the attacker.
            if !save.passed()
                && let Some(extra) = self.also_installs
            {
                effects.push(Box::new(
                    crate::engine::side_effects::ApplyCondition {
                        actor_id: target_id,
                        condition: extra,
                        timer: self.timer,
                    },
                ));
            }
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
    /// 5e **normal range** in tiles for the ranged variants, and `None`
    /// for every melee one. The mirror of `SimpleWeapon::normal_range`,
    /// and it is here for the same two rules: the long-range
    /// disadvantage clause, and Underwater Combat's "automatically
    /// misses a target beyond the weapon's normal range".
    ///
    /// The chassis shipped without it, which made its one ranged entry
    /// — the drow's poisoned hand crossbow — a weapon with no falloff
    /// at any distance it could reach. Nothing looked wrong: an unset
    /// threshold is indistinguishable from a shot that is always inside
    /// its band, so the drow simply never rolled the disadvantage RAW
    /// gives it, and the shape of the miss is that there was nothing to
    /// see.
    pub normal_range: Option<isize>,
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
            normal_range: None,
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
            normal_range: None,
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
            normal_range: None,
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
        normal_range: isize,
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
            normal_range: Some(normal_range),
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
        normal_range: isize,
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
            normal_range: Some(normal_range),
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

    /// Declared rather than derived, for the same reason
    /// `SimpleWeapon` declares it: this chassis *knows*, and the
    /// trait's default is a guess made by something that doesn't.
    ///
    /// Every swing on this chassis resolves through `resolve_attack`
    /// with `is_spell: false`, which is the engine's own definition of
    /// a weapon attack — and the attack site reads exactly that when it
    /// asks 5e's Underwater Combat rules what the water does to the
    /// swing. This method is the *other* copy of that answer, the one
    /// the AI's attack picker has to use because it holds a `&dyn
    /// Action` and never sees an `AttackParams`.
    ///
    /// So the bug this fixes is a divergence rather than a missing
    /// rule: four whole chassis — every save-rider and flat-rider
    /// natural weapon in the bestiary, plus the drow's hand crossbow —
    /// answered `false` here while the die answered `true` down there.
    /// The picker therefore ranked a shark's bite in its own ocean as
    /// though the water cost it nothing, and then the die rolled it at
    /// disadvantage; and a ranged shot the water makes impossible was
    /// never dropped from the candidate list, because the clause that
    /// drops it is the one clause the picker owns outright.
    ///
    /// Prediction and resolution disagreeing is worse than either being
    /// wrong alone. The picker's whole job is to choose between swings
    /// by what they will do, and it was choosing by a rule the engine
    /// does not use.
    fn is_weapon_attack(&self) -> bool {
        true
    }

    /// Declared from the field rather than inferred from `requires_los`
    /// and a reach band — see `SimpleWeapon::is_melee_attack` for the
    /// same argument. The inference is right for every entry on this
    /// chassis today; the field is right by construction.
    fn is_melee_attack(&self) -> bool {
        self.is_melee
    }

    fn normal_range(&self) -> Option<isize> {
        self.normal_range
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
                // The weapon's own normal range, not `None`. Two rules
                // read this number — 5e's long-range disadvantage and
                // Underwater Combat's automatic miss past normal range —
                // and a chassis that declared a `normal_range` field and
                // then handed the die `None` was a ranged weapon with no
                // falloff that also worked at the bottom of a lake. The
                // bestiary sweep that enforces the *field* could not see
                // that the field was never read.
                self.normal_range,
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
    /// What a hit installs, in the order the stat block prints it.
    ///
    /// A slice rather than one `Condition`, because RAW's hold-and-hurt
    /// clauses routinely name more than one: the Rug of Smothering's
    /// victim "is restrained, blinded, and is suffocating", and the
    /// three are one sentence with one duration. A single-condition
    /// field forced those onto three chassis or onto none, and the
    /// bestiary chose none — the rug shipped with the strongest of the
    /// three and a comment about the other two.
    ///
    /// Every entry shares `timer`, which is what makes the slice honest
    /// rather than a bag: these are the conditions that arrive together
    /// and leave together because RAW ends them on the same event.
    /// A rider with its own duration is a different rider.
    pub conditions: &'static [Condition],
    pub timer: ConditionTimer,
    /// Log-friendly tag for the install line ("auto-grapple", "adhesive",
    /// ...). Mirrors the `rider_name` slot on the sibling chassis so log
    /// shapes stay uniform across the weapon-rider family.
    pub rider_name: &'static str,
    /// The largest target the rider reaches, or `None` for a clause RAW
    /// leaves ungated. Set with `against_at_most`.
    ///
    /// SRD 5.2's single most-printed rider clause: *"If the target is a
    /// Large or smaller creature, it has the Grappled condition"*. Half
    /// the entries on this chassis carry one, and before this field
    /// existed every one of them installed on everything — a giant frog
    /// could hold a storm giant in its mouth, and a swarm of crawling
    /// hands could floor a mammoth.
    ///
    /// The gate covers the *rider*, not the swing: RAW puts the size
    /// clause on the sentence after the damage, so an oversized target
    /// takes the hit in full and simply shrugs off what rides on it.
    pub max_target_size: Option<Size>,
    /// A second, flat typed damage instance the hit also deals, with a
    /// label for the log — the remorhaz's *"18 (2d10 + 7) Piercing
    /// damage **plus 14 (4d6) Fire damage**"*, on top of the grapple
    /// that follows it. `None` for the majority whose hit line names one
    /// damage type. Set with `plus_damage`.
    ///
    /// This is where the two rider chassis meet. `WeaponWithRider` is
    /// "damage plus damage" and this one is "damage plus a hold"; three
    /// stat blocks in SRD 5.2 are *both* — the giant toad's venom and
    /// its grapple, the remorhaz's superheated gullet and its coils, the
    /// behir's crushing slash. Each of them was previously written as
    /// whichever half its chassis could express, with the other half in
    /// a docstring apologising for itself.
    ///
    /// It lives here rather than as a condition slot on
    /// `WeaponWithRider` because *this* chassis is the one that already
    /// knows how to install a hold correctly — through the linked
    /// installer, behind a size gate — and a second copy of that
    /// knowledge is how a grapple ends up naming nobody.
    ///
    /// Deliberately not gated by `max_target_size`: RAW's size clause
    /// governs the sentence it is in, and the extra damage is in the
    /// sentence before it.
    pub extra_damage: Option<(Dice, DamageType, &'static str)>,
    /// Forced movement the hit also causes, in tiles, or `None` for the
    /// majority that simply hit. Set with `pushes` or `pulls`.
    ///
    /// The rider-family's third shape, and the one that had no home at
    /// all: *"the merrow pulls the target up to 15 feet straight toward
    /// itself"*, *"the tough pushes the target up to 10 feet straight
    /// away from itself"*, *"the shambling mound pulls the target 5 feet
    /// straight toward itself"*. Six stat blocks print one; every one of
    /// them shipped with the clause in a docstring saying the chassis
    /// had no displacement lane, which was true.
    ///
    /// Gated by `max_target_size` alongside the condition install,
    /// because RAW puts them in the same sentence every time it prints
    /// both — the balor's whip *"pulls the target up to 25 feet straight
    /// toward itself, **and** the target has the Prone condition"*, one
    /// "if", one size clause. A weapon whose push and hold wanted
    /// different ceilings would be a different field; none exists.
    ///
    /// Carried through `PushActor` / `PullActor` rather than by moving
    /// the actor here, so the drag stops at walls and occupied tiles and
    /// provokes no opportunity attacks — 5e treats forced movement as
    /// not a willing move, and those two side-effects are where the
    /// engine writes that rule down.
    pub displacement: Option<Displacement>,
    /// 5e's "normal range" — the band beyond which a shot rolls at
    /// disadvantage, and beyond which Underwater Combat says it misses
    /// outright. `None` for every melee entry, where reach *is* the
    /// range; `Some` for every ranged one, which
    /// `every_ranged_weapon_declares_the_normal_range_two_rules_read`
    /// enforces across the whole bestiary — and which is exactly the
    /// sweep that caught this field being missing the moment the chassis
    /// grew a `ranged` constructor.
    pub normal_range: Option<isize>,
}

/// Forced movement a weapon's hit causes, in tiles.
///
/// Two variants rather than a signed magnitude, because the anchor
/// differs with the direction and the engine spells them as two
/// side-effects: a push measures *away from* the attacker and a pull
/// *toward* it. A signed number would have to be unpacked back into
/// that distinction at the one site that uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Displacement {
    /// Away from the attacker — the tough's warhammer, the force
    /// ballista's bolt.
    Push(u32),
    /// Toward the attacker — the merrow's harpoon, the balor's whip,
    /// the shambling mound's tendril, the water weird's coils.
    Pull(u32),
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
        conditions: &'static [Condition],
        timer: ConditionTimer,
        rider_name: &'static str,
    ) -> Self {
        Self::reach_melee(
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            conditions,
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
        conditions: &'static [Condition],
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
            conditions,
            timer,
            rider_name,
            max_target_size: None,
            extra_damage: None,
            displacement: None,
            normal_range: None,
        }
    }

    /// Const constructor for the ranged shape — a shot whose hit
    /// installs a condition or shoves what it hits.
    ///
    /// The chassis has carried an `is_melee` field and an
    /// `is_melee`-gated line-of-sight rule since it was written; what it
    /// had no way to build was a `WeaponWithCondition` with that field
    /// set to false. The same gap `WeaponWithRider::ranged` closed on
    /// its own chassis, and closed here for the same three clauses:
    /// the stone giant's boulder, the efreeti's Rock Launch and the
    /// Eldritch Cannon's force ballista all print "Ranged Attack Roll"
    /// followed by a rider.
    ///
    /// `normal_range` is not optional here, and the engine has a sweep
    /// that says so: two rules read it — 5e's long-range disadvantage
    /// and Underwater Combat's automatic miss past normal range — and a
    /// ranged weapon that left it unset would be a boulder with no
    /// falloff that also works at the bottom of a lake. It would look
    /// like a weapon that was simply good.
    #[allow(clippy::too_many_arguments)]
    pub const fn ranged(
        display_name: &'static str,
        aliases: &'static [&'static str],
        attack_ability: AbilityScoreType,
        damage_dice: Dice,
        damage_type: DamageType,
        conditions: &'static [Condition],
        timer: ConditionTimer,
        rider_name: &'static str,
        reach: isize,
        normal_range: isize,
    ) -> Self {
        Self {
            display_name,
            aliases,
            attack_ability,
            damage_dice,
            damage_type,
            reach,
            is_melee: false,
            conditions,
            timer,
            rider_name,
            max_target_size: None,
            extra_damage: None,
            displacement: None,
            normal_range: Some(normal_range),
        }
    }

    /// Chainable: RAW gates this rider on the target's size — *"If the
    /// target is a Large or smaller creature…"*.
    ///
    /// A builder rather than a tenth positional argument on both
    /// constructors, for the reason `ZoneClause::also_breaking_
    /// concentration` is one: the clause is a minority case, and
    /// threading `None` through every ungated declaration to serve the
    /// gated ones makes each row harder to read than the rule it
    /// encodes.
    pub const fn against_at_most(mut self, max: Size) -> Self {
        self.max_target_size = Some(max);
        self
    }

    /// Chainable: the hit also deals a second, flat instance of typed
    /// damage — RAW's "plus 14 (4d6) Fire damage". See `extra_damage`.
    pub const fn plus_damage(
        mut self,
        dice: Dice,
        damage_type: DamageType,
        label: &'static str,
    ) -> Self {
        self.extra_damage = Some((dice, damage_type, label));
        self
    }

    /// Chainable: the hit shoves the target `tiles` straight away from
    /// the attacker. See `displacement`.
    pub const fn pushes(mut self, tiles: u32) -> Self {
        self.displacement = Some(Displacement::Push(tiles));
        self
    }

    /// Chainable: the hit drags the target `tiles` straight toward the
    /// attacker. See `displacement`.
    pub const fn pulls(mut self, tiles: u32) -> Self {
        self.displacement = Some(Displacement::Pull(tiles));
        self
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

    fn requires_los(&self) -> bool {
        // Ranged variants need LOS like every other ranged attack; melee
        // doesn't. Matches `SimpleWeapon`'s `is_melee`-gated LOS rule.
        !self.is_melee
    }

    /// Declared rather than derived, for the same reason
    /// `SimpleWeapon` declares it: this chassis *knows*, and the
    /// trait's default is a guess made by something that doesn't.
    ///
    /// Every swing on this chassis resolves through `resolve_attack`
    /// with `is_spell: false`, which is the engine's own definition of
    /// a weapon attack — and the attack site reads exactly that when it
    /// asks 5e's Underwater Combat rules what the water does to the
    /// swing. This method is the *other* copy of that answer, the one
    /// the AI's attack picker has to use because it holds a `&dyn
    /// Action` and never sees an `AttackParams`.
    ///
    /// So the bug this fixes is a divergence rather than a missing
    /// rule: four whole chassis — every save-rider and flat-rider
    /// natural weapon in the bestiary, plus the drow's hand crossbow —
    /// answered `false` here while the die answered `true` down there.
    /// The picker therefore ranked a shark's bite in its own ocean as
    /// though the water cost it nothing, and then the die rolled it at
    /// disadvantage; and a ranged shot the water makes impossible was
    /// never dropped from the candidate list, because the clause that
    /// drops it is the one clause the picker owns outright.
    ///
    /// Prediction and resolution disagreeing is worse than either being
    /// wrong alone. The picker's whole job is to choose between swings
    /// by what they will do, and it was choosing by a rule the engine
    /// does not use.
    fn is_weapon_attack(&self) -> bool {
        true
    }

    /// Declared from the field rather than inferred from `requires_los`
    /// and a reach band — see `SimpleWeapon::is_melee_attack` for the
    /// same argument. The inference is right for every entry on this
    /// chassis today; the field is right by construction.
    fn is_melee_attack(&self) -> bool {
        self.is_melee
    }
    fn normal_range(&self) -> Option<isize> {
        self.normal_range
    }
    /// Both types when the hit carries a second instance — the
    /// remorhaz's piercing *and* its fire. Read by the AI's
    /// "is this worth aiming at that creature?" gate, which would
    /// otherwise write off a fire-immune target's whole bite for the
    /// half of it that is piercing.
    fn damage_types(&self) -> Vec<DamageType> {
        match self.extra_damage {
            Some((_, extra_type, _)) => vec![self.damage_type, extra_type],
            None => vec![self.damage_type],
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
                // The weapon's own normal range, not `None`. Two rules
                // read this number — 5e's long-range disadvantage and
                // Underwater Combat's automatic miss past normal range —
                // and a chassis that declared a `normal_range` field and
                // then handed the die `None` was a ranged weapon with no
                // falloff that also worked at the bottom of a lake. The
                // bestiary sweep that enforces the *field* could not see
                // that the field was never read.
                self.normal_range,
            );
            // Hit/miss gate via the effects vec — empty on miss /
            // Sanctuary / Mirror Image, non-empty on hit (the DealDamage
            // payload is present even when post-mitigation damage is 0).
            // RAW: an auto-install rider fires on hit, not on damage > 0,
            // so a 1-damage swing zeroed by Uncanny Dodge still grapples.
            if effects.is_empty() {
                return effects;
            }
            // Ahead of the size gate, because RAW prints it ahead of
            // the size gate: "plus 14 (4d6) Fire damage. **If** the
            // target is a Large or smaller creature…". A remorhaz that
            // bites something too big to coil around still cooks it.
            if let Some((dice, damage_type, label)) = self.extra_damage {
                add_flat_damage_rider(e, target_id, dice, damage_type, label, &mut effects);
            }
            // RAW puts the size clause on the sentence *after* the
            // damage — "Hit: 21 Piercing damage. If the target is a
            // Large or smaller creature, it has the Grappled condition"
            // — so an oversized target keeps the wound and loses only
            // the hold.
            if !rider_reaches_size(e, target_id, self.max_target_size, self.rider_name) {
                return effects;
            }
            // Behind the same gate as the hold and ahead of it in the
            // vec, so a creature dragged into reach is dragged before it
            // is held rather than after — which matters for the one
            // clause that does both (the water weird pulls you five feet
            // *and* wraps around you) and for nothing else.
            if let Some(displacement) = self.displacement
                && let Some(anchor) = e.actors.get(&caster_id).map(|c| c.location())
            {
                let (verb, tiles) = match displacement {
                    Displacement::Push(tiles) => ("shoved back", tiles),
                    Displacement::Pull(tiles) => ("dragged in", tiles),
                };
                e.log(format!(
                    "  {}: target is {} {} ft",
                    self.rider_name,
                    verb,
                    crate::engine::util::feet_from_tiles(tiles)
                ));
                effects.push(match displacement {
                    Displacement::Push(tiles) => {
                        Box::new(crate::engine::side_effects::PushActor {
                            actor_id: target_id,
                            from: anchor,
                            max_tiles: tiles,
                        }) as Box<dyn ApplicableSideEffect>
                    }
                    Displacement::Pull(tiles) => {
                        Box::new(crate::engine::side_effects::PullActor {
                            actor_id: target_id,
                            toward: anchor,
                            max_tiles: tiles,
                        }) as Box<dyn ApplicableSideEffect>
                    }
                });
            }
            // A displacement-only weapon installs nothing — the
            // merrow's harpoon and the tough's warhammer drag you and
            // stop there — so the install log and the loop below are
            // both skipped rather than printing "target is now ".
            if self.conditions.is_empty() {
                return effects;
            }
            let named: Vec<String> = self
                .conditions
                .iter()
                .map(|c| c.to_string())
                .collect();
            e.log(format!(
                "  {}: target is now {}",
                self.rider_name,
                named.join(" and ")
            ));
            // Through the linked installer rather than a bare
            // `ApplyCondition`, so a rider whose condition carries a
            // back-link records who applied it. For everything on
            // `LINKED_CONDITIONS` that is the difference between a rule
            // the engine can enforce and one it can only approximate:
            // a chuul's pincer grapple now names the chuul, so escaping
            // it is a contest against that creature and stunning the
            // chuul lets go. Unlinked conditions come back as the same
            // one-element vec this used to push.
            for &condition in self.conditions {
                effects.extend(crate::engine::side_effects::install_condition_with_link(
                    condition,
                    target_id,
                    caster_id,
                    self.timer,
                ));
            }
            effects
        };
        let mut effects = swing(encounter);
        maybe_chain_extra_attack(encounter, caster_id, &mut effects, swing);
        effects
    }
}


// ─── Swallow ─────────────────────────────────────────────────────────
//
// The seven SRD 5.2 stat blocks that can eat what they are holding, and
// the one action they share. Kept together rather than filed beside
// each creature's other attacks, because the interesting thing about
// them is the comparison: the same nine clauses, tuned across nineteen
// levels of challenge rating, from a CR-¼ frog that gives up its only
// attack to hold one halfling to a CR-30 titan with six people inside
// it and nothing slowed down at all.

/// The Bonus Action four of the seven print it as — the behir, the
/// purple worm, the remorhaz and the tarrasque all list Swallow under
/// **Bonus Actions**, which is what makes swallowing somebody free on
/// top of a full Multiattack and why those four are so much more
/// frightening once they have hold of you.
///
/// One object rather than four identical ones: nothing about the
/// doorway differs between them, and everything that does differ lives
/// on the creature's `SwallowProfile`.
pub static SWALLOW_BONUS: SwallowAttack = SwallowAttack {
    display_name: "swallow",
    aliases: &["swallow", "gulp", "engulf"],
    cost_resource: Resource::BonusAction,
};

/// The Action the other three spend on it. The frog and the toad give
/// up their whole turn — and, while they are full, the bite they would
/// otherwise be making (`SwallowProfile::blocked_while_full`).
///
/// The kraken is the odd one of the three: RAW folds its Swallow into
/// the Multiattack — *"the kraken makes two Tentacle attacks and uses
/// Fling, Lightning Strike, or Swallow"* — so the cost is the Action,
/// but the two tentacle swings come with it. The engine prices it as
/// the Action alone, which costs the kraken the two swings RAW would
/// have let it keep; the alternative is a Multiattack variant that
/// exists to hold one creature's punctuation.
pub static SWALLOW_ACTION: SwallowAttack = SwallowAttack {
    display_name: "swallow",
    aliases: &["swallow", "gulp", "engulf"],
    cost_resource: Resource::Action,
};

/// Giant Frog — *"The frog swallows a Small or smaller target it is
/// grappling."*
///
/// No save and no cap on how long it holds on, which sounds generous
/// until the size line is read: **Small or smaller**, on a creature
/// whose grapple reaches Medium. The frog can hold a human and can only
/// swallow a halfling, and the moment it does it has given up its bite
/// (`blocked_while_full`) for as long as it keeps them down. That is the
/// whole CR-¼ bargain — one party member removed from the fight, in
/// exchange for the frog removing itself.
///
/// The acid is 2d4 at the start of the frog's turns, where RAW puts it
/// at the end of the frog's *next* one and then disgorges on a timer.
/// The engine has one digest lane; the tick is a round earlier and it
/// keeps ticking, which is the direction that makes a swallowed
/// halfling's friends hurry.
pub static GIANT_FROG_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "gulps down",
    max_target_size: Some(Size::Small),
    digest: &[(Dice::new(2, 4), DamageType::Acid)],
    // The engine's name for the frog's bite, which is not "bite": the
    // giant frog and the giant toad both print "Bite" and the engine
    // needs two distinct action names, so the frog's carries its
    // creature. Matched against `Action::name()`, so a typo fails closed
    // as "this creature has no such attack" rather than as a frog that
    // bites while full.
    blocked_while_full: Some("giant frog bite"),
    ..SwallowProfile::defaults()
};

/// Giant Toad — the frog's clause one size up and with real teeth:
/// **Medium or smaller**, 3d6 acid a round, and the same "can't use
/// Bite while it has a swallowed target" price.
///
/// Where the frog is a nuisance this is a genuine threat to a CR-1
/// party: a swallowed wizard takes an average of ten a round with no
/// way to be healed from outside, and the toad has thirty-nine hit
/// points to chew through before anybody reaches them.
pub static GIANT_TOAD_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Medium),
    digest: &[(Dice::new(3, 6), DamageType::Acid)],
    blocked_while_full: Some("bite"),
    ..SwallowProfile::defaults()
};

/// Behir — DC 18 Dexterity, one at a time, 6d6 acid a round, and DC 14
/// Constitution to keep down thirty damage from the inside.
///
/// The lowest regurgitation DC of the five that have one, against the
/// second-highest threshold-to-hit-points ratio: thirty damage out of a
/// hundred and sixty-eight. A fighter swallowed by a behir who spends
/// their turn hacking at the stomach wall gets out about half the time,
/// which is exactly the shape the clause is for.
pub static BEHIR_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Large),
    save: Some((AbilityScoreType::Dexterity, 18)),
    digest: &[(Dice::new(6, 6), DamageType::Acid)],
    regurgitate: Some(RegurgitationClause {
        threshold: 30,
        dc: 14,
    }),
    ..SwallowProfile::defaults()
};

/// Remorhaz — the only stomach in the book that does two things at
/// once: *"10 (3d6) Acid damage **plus** 10 (3d6) Fire damage at the
/// start of each of the remorhaz's turns."*
///
/// Twenty a round through two damage types is the reason `digest` is a
/// slice, and it is not a technicality: a creature with fire resistance
/// really does last half again as long in there, and the remorhaz is
/// the monster you meet in the one environment where fire resistance is
/// the thing everybody brought.
pub static REMORHAZ_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Large),
    capacity: 2,
    save: Some((AbilityScoreType::Strength, 19)),
    digest: &[
        (Dice::new(3, 6), DamageType::Acid),
        (Dice::new(3, 6), DamageType::Fire),
    ],
    regurgitate: Some(RegurgitationClause {
        threshold: 30,
        dc: 15,
    }),
    ..SwallowProfile::defaults()
};

/// Purple Worm — three at a time, 5d6 acid a round, DC 21 Constitution
/// against a thirty-damage threshold.
///
/// The clause the whole creature is built around. A worm that has eaten
/// half the party is a fight the party is losing from inside a tunnel
/// they cannot see out of, and the only lever is the threshold — thirty
/// damage in one turn from one creature in there, which at CR 15 is
/// one good round from a rogue who knows the rule.
pub static PURPLE_WORM_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Large),
    capacity: 3,
    save: Some((AbilityScoreType::Strength, 19)),
    digest: &[(Dice::new(5, 6), DamageType::Acid)],
    regurgitate: Some(RegurgitationClause {
        threshold: 30,
        dc: 21,
    }),
    ..SwallowProfile::defaults()
};

/// Kraken — four at a time, 7d6 acid a round, and the only stomach that
/// does not blind: *"A swallowed creature has the Restrained
/// condition"*, full stop.
///
/// A creature in a kraken's beak can still see, which is a small mercy
/// worth modeling exactly because it is the one place the seven stat
/// blocks disagree with each other. It is also the highest
/// regurgitation bar in the book — fifty damage in one turn against a
/// DC 25 — which at CR 23 is a bar a swallowed paladin can clear and
/// almost nobody else can.
pub static KRAKEN_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Large),
    capacity: 4,
    save: Some((AbilityScoreType::Dexterity, 25)),
    digest: &[(Dice::new(7, 6), DamageType::Acid)],
    swallowed_conditions: &[Condition::Restrained],
    regurgitate: Some(RegurgitationClause {
        threshold: 50,
        dc: 25,
    }),
    ..SwallowProfile::defaults()
};

/// Tarrasque — six at a time, 16d6 acid a round, DC 27 Strength to
/// avoid, DC 20 Constitution to keep down sixty damage.
///
/// Fifty-six a round inside a creature with six hundred and ninety-seven
/// hit points and Legendary Resistance six times a day. The threshold is
/// the only door and it is a wide one relative to the DC: sixty damage
/// in a turn is a CR-30 party's normal output, and the save that follows
/// is the *lowest* of the five once the creature is that big. The
/// tarrasque eats you and then, quite often, is made to bring you back
/// up — which is the fight that stat block is describing.
pub static TARRASQUE_SWALLOW: SwallowProfile = SwallowProfile {
    verb: "swallows",
    max_target_size: Some(Size::Large),
    capacity: 6,
    save: Some((AbilityScoreType::Strength, 27)),
    digest: &[(Dice::new(16, 6), DamageType::Acid)],
    regurgitate: Some(RegurgitationClause {
        threshold: 60,
        dc: 20,
    }),
    ..SwallowProfile::defaults()
};

/// 5e **Swallow** — the action that turns a grapple you are already
/// holding into a creature inside you.
///
/// Data-only, because all seven SRD 5.2 stat blocks that print one
/// print the same action with different numbers: a target this creature
/// is already Grappling, a size it can get down, room left inside, and
/// on five of the seven a saving throw that avoids it. Everything that
/// happens *afterwards* — the acid, the Total Cover, the regurgitation
/// save, the crawl out of the corpse — belongs to
/// `crate::engine::swallow::SwallowProfile` on the creature rather than
/// to this action, for the same reason the attach clause is split that
/// way: four of the sites that read it never see the action.
///
/// So this struct is only the *doorway*. It carries the cost and the
/// alias list, and hands off to `EncounterInstance::swallow` the moment
/// the save fails.
///
/// **Why the profile is not read off this action.** The tempting shape
/// is one struct with all nine clauses on it. It does not work: the
/// digest tick fires at the start of the swallower's turn from
/// `start_turn_for`, which holds an actor id and no action; the
/// regurgitation sweep runs at the end of *anybody's* turn; the Total
/// Cover gate is asked by every other action in the engine. Three of
/// those four would have had to go looking through the creature's
/// action list for a `SwallowAttack` and downcast it.
pub struct SwallowAttack {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// What the action costs. A Bonus Action on four of the seven — see
    /// `SWALLOW_BONUS` — and the Action on the other three; see
    /// `SWALLOW_ACTION`.
    pub cost_resource: Resource,
}

impl Action for SwallowAttack {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    /// Reach is the grapple's, not a number of its own: RAW names "one
    /// Large or smaller creature **Grappled by** the worm", and a
    /// creature the worm is holding is by definition within reach of it.
    /// Declared anyway so the AI's range filter has something to read,
    /// and generous enough for the kraken's thirty-foot tentacles.
    fn reach_tiles(&self) -> Option<isize> {
        Some(SWALLOW_REACH)
    }
    fn requires_los(&self) -> bool {
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
        vec![self.cost_resource]
    }

    /// Every gate RAW prints, asked through the one predicate that owns
    /// them — `can_swallow`. Declining here rather than inside
    /// `side_effects` is what keeps the action off the AI's candidate
    /// list and out of the player's prompt when there is nobody in the
    /// creature's grip.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        first_target_id(target_ids)
            .is_some_and(|target_id| encounter.can_swallow(caster_id, target_id).is_ok())
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
        // Re-asked rather than trusted from validation: a Multiattack
        // expansion can put a tentacle swing between the two, and a
        // target that died to it is no longer on the menu.
        let profile = match encounter.can_swallow(caster_id, target_id) {
            Ok(profile) => profile,
            Err(refusal) => {
                let name = encounter.actor_name(caster_id);
                encounter.log(format!(
                    "  {} cannot swallow: {}.",
                    name,
                    refusal.describe()
                ));
                return Vec::new();
            }
        };
        if let Some((ability, dc)) = profile.save {
            let target_name = encounter.actor_name(target_id);
            if encounter.roll_save(target_id, ability, dc).passed() {
                encounter.log(format!("  {} squirms free of the gullet.", target_name));
                return Vec::new();
            }
        }
        // Applied here rather than pushed as a side-effect, and it is
        // the one place in the file that does so. The link writes the
        // occupancy grid — a footprint comes off it and a `location` is
        // mirrored — and every queued effect behind this one in the same
        // vec would otherwise resolve against a board that still had the
        // victim standing outside. `swallow` is idempotent-safe on its
        // own gates, so re-entry through a Multiattack is a logged
        // refusal rather than a second stomach.
        let _ = encounter.swallow(caster_id, target_id);
        Vec::new()
    }
}

/// The reach `SwallowAttack` declares, in tiles.
///
/// Thirty feet, which is the kraken's tentacles — the longest grip in
/// the bestiary — because the number is a filter on an action whose real
/// gate is "am I already holding this creature", and a filter that is
/// tighter than the grip it is filtering would make the kraken unable to
/// eat what it is holding.
const SWALLOW_REACH: isize = 12;

/// The weapon-rider family's fourth shape: a melee swing whose hit
/// latches the swinger onto what it bit.
///
/// Sibling of `WeaponWithCondition`, and the difference between them is
/// the whole reason this exists rather than a `Condition::Attached` row
/// on that one. An install-on-hit condition is a fact about the
/// *target*; an attach is a fact about the *pair*, and the pair has a
/// position — see `crate::engine::attachment` for what the link does
/// that a flag could not.
///
/// What the latch does once it holds belongs to the creature's
/// `AttachProfile`, not to this struct: three stat blocks share this
/// chassis and differ on every clause of the ride, and threading eight
/// more fields through the weapon would put the cloaker's damage split
/// on the stirge's proboscis.
pub struct AttachingWeapon {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub attack_ability: AbilityScoreType,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub reach: isize,
}

impl AttachingWeapon {
    /// Const constructor for the only shape any of the three carriers
    /// has: a reach-5-ft swing using one ability for both attack and
    /// damage.
    pub const fn melee(
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
            damage_dice,
            damage_type,
            reach: MELEE_REACH,
        }
    }
}

impl Action for AttachingWeapon {
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
    fn is_weapon_attack(&self) -> bool {
        true
    }
    fn is_melee_attack(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }
    /// Refused only for the carrier RAW stops outright — the stirge's
    /// "while attached, the stirge can't make Proboscis attacks".
    ///
    /// The other two are *not* refused, which is the distinction that
    /// matters and the one this gate originally got wrong. RAW bars the
    /// cloaker only "against other targets" and tells the darkmantle it
    /// "can attack only the target": both are sentences about aim, both
    /// are already enforced by the shared hostility gate in
    /// `Action::validate`, and neither stops the creature hitting the
    /// thing it is wrapped around. A blanket refusal here cost the
    /// cloaker its entire Multiattack the moment it landed one —
    /// `CompoundAttack` validates all of its parts, so one refused part
    /// refuses the Action — and left the darkmantle, whose only Action
    /// this is, with nothing to do at all.
    ///
    /// A repeat swing on the same host re-rolls the damage and no-ops
    /// the latch; see `EncounterInstance::attach`.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        !(encounter.is_attached(caster_id)
            && encounter
                .attach_profile(caster_id)
                .is_some_and(|p| p.blocked_while_attached))
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
        let (mut effects, _dealt) = weapon_swing_with_damage(
            encounter,
            caster_id,
            target_id,
            self.display_name,
            self.attack_ability,
            self.damage_dice,
            self.damage_type,
            true,
            None,
        );
        // Hit/miss gate via the effects vec, the same reading the rest
        // of the weapon-rider family uses: empty on a miss, on
        // Sanctuary and on a Mirror Image, non-empty on a hit even when
        // the damage is later scaled to nothing. RAW hangs the latch on
        // the hit, so a blow zeroed by Uncanny Dodge still attaches.
        if effects.is_empty() {
            return effects;
        }
        // Queued behind the damage rather than performed here, so a
        // swing that drops its target attaches to nobody — see
        // `AttachTo`. No Extra Attack chain: nothing on this chassis
        // has one, and a second swing from a creature that just latched
        // would be refused by its own validator anyway.
        effects.push(Box::new(crate::engine::side_effects::AttachTo {
            attacher_id: caster_id,
            host_id: target_id,
        }));
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
)
.mastery(WeaponMastery::Slow);

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
).light()
.mastery(WeaponMastery::Nick);

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
    is_light: false,
    mastery: Some(WeaponMastery::Vex),
    bloodied_dice: None,
};

/// Dagger — finesse 1d4 piercing melee weapon. STR-or-DEX choice;
/// we use DEX which is the typical kobold / rogue stat. Cost 1 Action.
/// The four weapons below each ship as a `_PROFILE` const and two
/// statics — the swing and the throw — rather than as two independent
/// declarations.
///
/// The const is what makes the throw *the same weapon*. RAW says a
/// thrown melee weapon uses the ability, die and damage type of the
/// swing, so those three facts have exactly one place to be written
/// and the throw derives from it; a second literal would be a second
/// place for them to drift, and a javelin whose throw rolled the wrong
/// ability would look right in both halves read separately.
///
/// It is a `const` rather than a `static` because a static cannot be
/// moved out of, and the builder consumes `self`. Consts are inlined
/// at each use, so the two statics below are two distinct objects with
/// two distinct addresses — which is what the action list wants, since
/// it holds `&'static dyn Action` and the swing and the throw are
/// different entries on it.
const DAGGER_PROFILE: SimpleWeapon = SimpleWeapon::melee(
    "dagger",
    &["dag"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
).light()
.mastery(WeaponMastery::Nick);

pub static DAGGER: SimpleWeapon = DAGGER_PROFILE;

/// The dagger in flight — RAW 20/60 ft, which is 8/24 tiles.
///
/// Inherits DEX from the swing, which is RAW twice over: the throw
/// uses the melee ability, and the melee ability for a finesse weapon
/// is the wielder's choice. The shortest throw on the roster, and the
/// one carried by the builds least able to stand in melee.
pub static THROWN_DAGGER: SimpleWeapon = DAGGER_PROFILE.thrown(
    "thrown dagger",
    &["td", "hurl dagger"],
    THROWN_SHORT_NORMAL,
    THROWN_SHORT_LONG,
);

/// Handaxe — STR-based 1d6 slashing, light, thrown. RAW's simple
/// melee axe, and the Strength build's answer to the dagger: a
/// martial who dual-wields axes throws one at the archer they cannot
/// reach.
///
/// Same die and damage type as the scimitar and distinct from it on
/// the two properties that matter — the scimitar is light but stays
/// in the hand, and this leaves it.
const HANDAXE_PROFILE: SimpleWeapon = SimpleWeapon::melee(
    "handaxe",
    &["ha", "axe"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
).light()
.mastery(WeaponMastery::Vex);

pub static HANDAXE: SimpleWeapon = HANDAXE_PROFILE;

/// The handaxe in flight — RAW 20/60 ft.
pub static THROWN_HANDAXE: SimpleWeapon = HANDAXE_PROFILE.thrown(
    "thrown handaxe",
    &["tha", "hurl axe"],
    THROWN_SHORT_NORMAL,
    THROWN_SHORT_LONG,
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
)
.mastery(WeaponMastery::Push);

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
)
.mastery(WeaponMastery::Push);

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
)
.mastery(WeaponMastery::Sap);

/// Spear — STR-based 1d6 piercing simple weapon. Tribal Warrior /
/// generic-tribal NPC sidearm. RAW the spear is versatile (1d8 two-handed)
/// and thrown (20/60 ft); we collapse to the one-hand 1d6 melee base
/// since the engine doesn't surface per-action grip toggles and the
/// thrown lane is already covered by `JAVELIN`. Shared static so Tribal
/// Warrior and any future spear-wielding humanoid point at one source.
const SPEAR_PROFILE: SimpleWeapon = SimpleWeapon::melee(
    "spear",
    &["sp"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
)
.mastery(WeaponMastery::Sap);

pub static SPEAR: SimpleWeapon = SPEAR_PROFILE;

/// RAW's short throw — 20 ft normal, 60 ft long — in tile-gap units on
/// the 2.5-ft grid. The range every thrown melee weapon in the PHB
/// shares except the javelin, which is built to be thrown and reaches
/// half again as far.
///
/// Named rather than repeated as a pair of literals at each throw,
/// because the two numbers only mean anything together: `24` on its
/// own is indistinguishable from a reach, and a throw whose normal
/// range was accidentally given as its long range would be a weapon
/// that never rolls at disadvantage and nothing would say so.
pub const THROWN_SHORT_NORMAL: isize = 8;
pub const THROWN_SHORT_LONG: isize = 24;

/// The spear in flight — RAW 20/60 ft.
///
/// The one on the underwater ranged cohort that the roster could
/// nearly reach already: `UNDERWATER_RANGED_WEAPONS` has named the
/// spear since it was written, and the spear has been a melee-only
/// weapon the whole time, so the row could never match. It matches
/// now.
pub static THROWN_SPEAR: SimpleWeapon = SPEAR_PROFILE.thrown(
    "thrown spear",
    &["tsp", "hurl spear"],
    THROWN_SHORT_NORMAL,
    THROWN_SHORT_LONG,
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
).light()
.mastery(WeaponMastery::Vex);

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
).light()
.mastery(WeaponMastery::Slow);

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
/// Giant Spider Bite — STR-based 1d10+STR piercing melee with a CON DC 11
/// save-or-2d4-poison-AND-Poisoned-2-rounds rider. The "extra damage AND
/// condition both ride on the same failed save" shape — `also_install`
/// is `Some((Poisoned, Rounds(2)))` so the chassis adds the condition
/// install only when the save fails. Routes through the shared
/// `WeaponWithSaveDamage` chassis alongside Ettercap Bite / Drow
/// Poisoned Crossbow.
pub static GIANT_SPIDER_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::melee_with_condition(
    "giant spider bite",
    &["sbite", "spider-bite"],
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
)
.mastery(WeaponMastery::Cleave);

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
)
.mastery(WeaponMastery::Push);

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
)
.against_at_most(Size::Large);

/// Frightful Howl — wolf bonus action. Every enemy within 4 tiles must
/// make a WIS save against DC 11 or be Frightened for 3 rounds.
/// Doesn't deal damage. Demonstrates the AoE-no-damage save pattern.
pub struct FrightfulHowl {}

impl FrightfulHowl {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 4;
}

impl Action for FrightfulHowl {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

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
            if dist > Self::RADIUS {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if !save.passed() {
                effects.extend(crate::engine::side_effects::install_condition_with_link(
                    Condition::Frightened,
                    tid,
                    caster_id,
                    ConditionTimer::Rounds(3),
                ));
            }
        }
        effects
    }
}
pub static FRIGHTFUL_HOWL: LazyLock<FrightfulHowl> = LazyLock::new(|| FrightfulHowl {});

/// Emit the `Action` methods that a single-sub-attack wrapper answers
/// by asking the thing it wraps.
///
/// Three wrappers in this module hold one sub-action and add exactly one
/// idea to it — `Multiattack` adds a repeat count, `RechargingAttack`
/// adds a recharge gate, `ProneOnlyAttack` adds a target gate — and all
/// three have to answer the same ten questions about *shape*: how far it
/// reaches, what schema it targets on, whether it is a weapon, what
/// damage it deals. None of those are the wrapper's to answer. A
/// multiattack is its sub-attack `count` times, a recharging rock is a
/// rock that is sometimes not there, and neither of them has a reach of
/// its own.
///
/// Written as a macro rather than left to the trait's defaults because
/// those defaults exist for actions with *no* sub-action to ask, and so
/// they guess — and both guesses have been wrong in exactly this
/// position:
///
///   - `deals_damage` defaults to `is_harmful()`, which is true of
///     every wrapper ever declared, including one whose sub-attack is a
///     damage-free grab. The roper is that case, and it went wrong
///     twice over from one line: `best_attack_against` ranks reach
///     before damage, so a four-tendril flurry claiming to be damage
///     outranked the roper's own 4d6 bite nose to nose; and
///     `has_ranged_attack` read the same claim at twenty tiles and
///     called a creature with a 10 ft walk speed a kiter, so it spent
///     its turns backing away from the thing it had just tied itself to.
///   - `is_melee_attack` infers melee from a reach band, which is the
///     divergence `SimpleWeapon::is_melee_attack` documents at length
///     one layer up: a kraken's triple-tentacle, at six tiles, read as
///     a shot.
///
/// The point of the forwarding is that the wrapper stops guessing —
/// which is why it has to be uniform, and why a wrapper added later
/// should not have to remember ten methods to be correct.
///
/// The bare form emits the ten shape questions. `transparent` adds the
/// three the two gate wrappers also pass straight through — aliases,
/// damage estimate, and resource cost — which `Multiattack` answers for
/// itself (its own aliases, `count` times the estimate, and a cost with
/// Movement filtered out).
macro_rules! forwards_to_sub_attack {
    ($field:ident) => {
        fn targeting_schema(&self) -> TargetingSchema {
            self.$field.targeting_schema()
        }

        fn reach_tiles(&self) -> Option<isize> {
            self.$field.reach_tiles()
        }

        fn min_effective_reach(&self) -> Option<isize> {
            self.$field.min_effective_reach()
        }

        fn normal_range(&self) -> Option<isize> {
            self.$field.normal_range()
        }

        fn requires_los(&self) -> bool {
            self.$field.requires_los()
        }

        fn damage_types(&self) -> Vec<DamageType> {
            self.$field.damage_types()
        }

        fn deals_damage(&self) -> bool {
            self.$field.deals_damage()
        }

        fn is_weapon_attack(&self) -> bool {
            self.$field.is_weapon_attack()
        }

        fn is_melee_attack(&self) -> bool {
            self.$field.is_melee_attack()
        }

        fn underwater_weapon_name(&self) -> &str {
            self.$field.underwater_weapon_name()
        }

        // Both are facts about the shape of the thing being wrapped,
        // so a wrapper reports what it wraps.
        fn spares_allies(&self) -> bool {
            self.$field.spares_allies()
        }
    };
    ($field:ident, transparent) => {
        forwards_to_sub_attack!($field);

        fn aliases(&self) -> Vec<&str> {
            self.$field.aliases()
        }

        fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
            self.$field.expected_damage(encounter, caster_id)
        }

        fn cost(
            &self,
            encounter: &EncounterInstance,
            caster_id: usize,
            target_ids: Option<&Vec<usize>>,
            target_locations: Option<&Vec<Coordinate>>,
            overrides: Option<&HashSet<ActionOverride>>,
        ) -> Vec<Resource> {
            self.$field
                .cost(encounter, caster_id, target_ids, target_locations, overrides)
        }
    };
}

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

    forwards_to_sub_attack!(sub_attack);

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
        // 5e Slow cuts a routine down to one swing — see
        // `EncounterInstance::attack_routine_swings`. Read here rather
        // than gated at `validate`, so a creature whose sheet carries a
        // multiattack and nothing else still gets its one attack.
        let swings = encounter.attack_routine_swings(caster_id, self.count);
        for _ in 0..swings {
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

/// Wraps another action and gates it on a 5e **Recharge** clause —
/// `Rock (Recharge 6)`, `Tail Spike (Recharge 5–6)`.
///
/// The recharge machinery already existed on both ends: a template
/// declares `recharge_abilities: vec![("rock", 6)]`, and
/// `EncounterInstance::start_turn` rolls a d6 per spent entry and hands
/// it back on a qualifying roll. What was missing was the middle — the
/// only things that consulted a recharge key were the two bespoke
/// breath-weapon chassis, each of which open-codes the check inside its
/// own `custom_validate_input` and its own `side_effects`. A third
/// stat block wanting the clause on an ordinary attack roll had a
/// choice between a third copy of that code and going without.
///
/// So this is `Multiattack`'s shape rather than `BreathWeapon`'s: a
/// wrapper that owns *only* the gate and forwards everything else to
/// what it wraps. The sub-action keeps its own reach, targeting schema,
/// damage types, cost, and resolution — a recharging rock is a thrown
/// rock that is sometimes not there, not a different attack.
///
/// The key is matched against `CreatureTemplate::recharge_abilities` by
/// string, so it has to be spelled the same in both places; a
/// mismatched key fails *closed* (`is_recharge_available` finds no
/// entry and answers false), which surfaces as "the monster never uses
/// its rock" rather than as an ability that recharges silently forever.
pub struct RechargingAttack {
    pub display_name: &'static str,
    pub sub_attack: &'static (dyn Action + Send + Sync),
    /// Key passed to `is_recharge_available` / `spend_recharge`, and the
    /// name the template's `recharge_abilities` row must carry. Also
    /// what the recharge log line prints, so it reads as the ability's
    /// name rather than as an internal handle.
    pub recharge_key: &'static str,
}

impl Action for RechargingAttack {
    fn name(&self) -> &str {
        self.display_name
    }

    forwards_to_sub_attack!(sub_attack, transparent);

    /// The wrapper's own, not the sub-attack's: the whole point of this
    /// chassis is to put a recharge gate on something that had none.
    fn recharge_key(&self) -> Option<&'static str> {
        Some(self.recharge_key)
    }

    /// The gate, and the sub-action's own gate underneath it. Both, in
    /// that order, for the reason `Multiattack::custom_validate_input`
    /// gives: a wrapper cannot be legal where the thing it wraps is not.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, self.recharge_key)
            && self.sub_attack.custom_validate_input(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            )
    }

    /// Spend the charge, then resolve the wrapped attack exactly once.
    ///
    /// Spent *first* and unconditionally, matching both breath-weapon
    /// chassis: RAW's recharge is consumed by using the ability, not by
    /// the ability connecting. A rock that misses is still a rock that
    /// has been thrown.
    ///
    /// Resolved inside the multiattack-depth gate, which is the same
    /// mechanism `Multiattack` uses and is here for a reason that is
    /// about the charge rather than about RAW's Attack-action wording:
    /// a `SimpleWeapon` costing an Action chains its wielder's Extra
    /// Attack, and a chained second swing would throw the rock again
    /// off a charge that has already been spent. Nothing in the
    /// bestiary carries both today — Extra Attack lives on the class
    /// chassis and the recharge clauses on the monsters — so this is
    /// the trap being closed rather than a bug being fixed, of exactly
    /// the kind `Multiattack::custom_validate_input`'s own docstring
    /// describes.
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge(self.recharge_key);
        }
        encounter.enter_multiattack();
        let effects = self.sub_attack.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        encounter.exit_multiattack();
        effects
    }
}

/// Wraps another action and refuses it unless the target is already
/// **Prone** — the "I can only hit you once you're down" clause the
/// heavy trampling creatures carry.
///
/// `RechargingAttack`'s sibling in every respect: a gate and nothing
/// else, forwarding the whole of the sub-action's self-description so
/// the AI's picker, the reach check and the log line all see the limb
/// rather than the wrapper. It exists for the same reason too — the
/// gate had been written once as a bespoke `impl Action` (the mammoth's
/// stomp), and the second creature to want it (the elephant's trample)
/// would otherwise have copied thirty lines to change two dice.
///
/// The gate pairs with `ChargeRider::prone_follow_up`, which is how the
/// target gets prone in the first place: the charge knocks them down
/// and hands the trampler a bonus-action swing at a target this gate
/// now admits. Standalone, it is also the fallback the AI reaches for
/// when something *else* put the target on the floor — a Booming Blade
/// follow-up, a wolf's trip bite, a failed Grease save.
pub struct ProneOnlyAttack {
    pub display_name: &'static str,
    pub sub_attack: &'static (dyn Action + Send + Sync),
}

impl Action for ProneOnlyAttack {
    fn name(&self) -> &str {
        self.display_name
    }

    forwards_to_sub_attack!(sub_attack, transparent);

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Routes through the shared `target_has_condition` helper so the
        // missing-target / missing-actor fail-closed convention stays
        // consistent with the recharge gates.
        target_has_condition(encounter, target_ids, Condition::Prone)
            && self.sub_attack.custom_validate_input(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            )
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        self.sub_attack.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        )
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

    /// True when any part of the compound whittles hit points.
    ///
    /// "Any" rather than "all", because a compound whose parts are a
    /// grab and a bite is still worth pointing at a target, and the
    /// question this answers is whether the AI's focus-fire lane should
    /// consider it at all. The homogeneous sibling forwards the same
    /// question to its single sub-attack; see the note there for what
    /// the trait's default got wrong.
    fn deals_damage(&self) -> bool {
        self.parts.iter().any(|(a, _)| a.deals_damage())
    }

    /// True only when *every* part is a weapon attack — "all" here
    /// where `deals_damage` says "any", because the consumers differ.
    /// That one asks whether the Action is worth pointing at somebody;
    /// this one is read by rules that will be applied to the whole
    /// Action, and a rule that should tax two swings out of three
    /// should not be applied to all three.
    ///
    /// Left at the trait default until now, which meant `false`: every
    /// compound in the bestiary read as a non-weapon attack, so the
    /// AI's picker predicted the water would leave a bite-and-claws
    /// alone while the die taxed both swings. Same divergence
    /// `SimpleWeapon::is_weapon_attack` was declared to close, one
    /// wrapper up.
    fn is_weapon_attack(&self) -> bool {
        !self.parts.is_empty() && self.parts.iter().all(|(a, _)| a.is_weapon_attack())
    }

    /// The worst-off part's name, which is the honest single answer to
    /// a question RAW asks per weapon about an Action that swings more
    /// than one.
    ///
    /// 5e's Underwater Combat allowlists are per-weapon, and the die
    /// applies them per swing — correctly, since each part resolves
    /// under its own name. The picker holds only the wrapper and gets
    /// one string, so it gets the name of the first part the water
    /// actually taxes: a compound of a trident and a bite is a
    /// compound the water half-taxes, and half-taxed ranks nearer to
    /// taxed than to free. Falls back to the first part when the water
    /// taxes none of them, which is the case where every part agrees
    /// and any of them would do.
    fn underwater_weapon_name(&self) -> &str {
        use crate::engine::underwater::melee_keeps_edge;
        let worst = self
            .parts
            .iter()
            .map(|(a, _)| a.underwater_weapon_name())
            .find(|name| !melee_keeps_edge(name));
        worst
            .or_else(|| self.parts.first().map(|(a, _)| a.underwater_weapon_name()))
            .unwrap_or_else(|| self.name())
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
        // 5e Slow's "only one attack" — a routine of mixed parts is cut
        // to the first swing of the first part, which is the same
        // clamp `Multiattack` makes and the closest a compound routine
        // gets to "one attack".
        let mut budget = encounter.attack_routine_swings(
            caster_id,
            self.parts.iter().map(|(_, n)| *n).sum(),
        );
        for (sub, count) in &self.parts {
            for _ in 0..*count {
                if budget == 0 {
                    break;
                }
                budget -= 1;
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

/// Bandit Captain multiattack — RAW: "The captain makes three melee
/// attacks: two with its scimitar and one with its dagger."
///
/// It used to be three scimitars, on the reasoning that the captain is
/// "a tougher version of the goblin boss's pattern". Three swings is
/// the right count and one of them was the wrong weapon, which is not
/// a rounding error where it lands: the dagger is *piercing* and the
/// scimitar is *slashing*, and the bestiary is full of creatures that
/// resist one and not the other. A skeleton takes half from every
/// slashing swing and full from a piercing one — so against the
/// captain's own routine the engine was quietly deleting a third of
/// its damage against some targets and inventing it against others.
///
/// A `CompoundAttack` rather than a `Multiattack` for exactly that
/// reason: the two are the same shape until the swings differ, and
/// these differ. Same pairing the bullywug and the merrow already use.
pub static BANDIT_CAPTAIN_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "scimitar flurry",
    parts: vec![(&SCIMITAR, 2), (&DAGGER, 1)],
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

impl FrightfulPresence {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 6;
}

impl Action for FrightfulPresence {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
            .enemy_burst_targets(caster_id, caster_loc, Self::RADIUS)
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
                effects.extend(crate::engine::side_effects::install_condition_with_link(
                    Condition::Frightened,
                    tid,
                    caster_id,
                    ConditionTimer::Rounds(3),
                ));
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

/// Bugbear morningstar — 2d8+STR piercing, carrying RAW's **Surprise
/// Attack** rider: *"if the bugbear surprises a creature and hits it
/// with an attack during the first round of combat, the target takes
/// an extra 7 (2d6) damage."*
///
/// The rider used to fire on any hit in round one, because the engine
/// had no Surprised condition to ask about and the round number was
/// the closest thing to one. It has one now
/// (`EncounterInstance::resolve_opening_surprise`), and the flag is a
/// strictly better gate than the round number in both directions: it
/// stops paying out against a party that walked in with its eyes open,
/// and it names the creature the clause is about rather than the
/// moment. The round-one half needs no separate check — the condition
/// expires at the end of round one, so a surprised target *is* a
/// round-one target.
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
        let Some(target_id) = target_id else {
            return effects;
        };
        let surprised = encounter
            .actors
            .get(&target_id)
            .is_some_and(|t| t.has_condition(Condition::Surprised));
        if !surprised {
            return effects;
        }
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
)
.against_at_most(Size::Large);

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

impl LuringSong {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const SONG_RADIUS: isize = 12;
}

impl Action for LuringSong {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::SONG_RADIUS)
    }

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
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Sorted-id iteration for deterministic save sequencing.
        for target_id in encounter.sorted_actor_ids() {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            if target.team() == caster_team || !target.is_combat_active() {
                continue;
            }
            // A song is heard or it is nothing — RAW charms "each
            // humanoid and giant that can hear the song". The clause
            // was in the quote and not in the code until `can_hear`
            // gave it something to read.
            if target.effectively_immune_to_condition(Condition::Charmed) || !target.can_hear() {
                continue;
            }
            let dist = crate::engine::util::footprint_chebyshev(
                target.location(),
                crate::engine::util::get_tiles_from_size(target.size()),
                caster_loc,
                1,
            );
            if dist > Self::SONG_RADIUS {
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
)
.mastery(WeaponMastery::Sap);

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
)
.mastery(WeaponMastery::Graze);

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
    .mastery(WeaponMastery::Topple)
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

/// Stirge Proboscis — DEX-based 1d6+DEX piercing melee that latches the
/// stirge onto whatever it hit. RAW: "Melee Attack Roll: +5, reach 5
/// ft. Hit: 6 (1d6 + 3) Piercing damage, and the stirge attaches to the
/// target. While attached, the stirge can't make Proboscis attacks, and
/// the target takes 5 (2d4) Necrotic damage at the start of each of the
/// stirge's turns."
///
/// The whole of that sentence lands now, across two files: the latch is
/// this chassis, and the drain plus the no-second-bite clause are the
/// stirge's `AttachProfile`. What shipped before was the first six
/// words of it — a 1d4 jab with a docstring conceding that the attach
/// was "approximated" by re-jabbing every turn, which is a different
/// creature: a stirge you can walk away from.
pub static STIRGE_PROBOSCIS: AttachingWeapon = AttachingWeapon::melee(
    "blood drain",
    &["proboscis", "drain"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Cockatrice **Petrifying Bite** — SRD 5.2: 1d4+1 piercing, and *"if
/// the target is a creature, it is subjected to the following effect.
/// Constitution Saving Throw: DC 11. First Failure: The target has the
/// Restrained condition… Second Failure: The target has the Petrified
/// condition, instead of the Restrained condition."*
///
/// The petty damage is the hook; the ladder is the threat. Routed
/// through `staged_saves::PETRIFICATION`, which is the same sentence
/// the basilisk, the gorgon and the medusa print.
///
/// This used to be one save and a one-round `Petrified`, with a
/// docstring explaining that RAW's petrification is permanent and the
/// timer had been cut *"so a single hit doesn't game-over the target on
/// a missed save."* That is the problem RAW solves with the ladder
/// rather than with the timer: one failure stiffens you and hands your
/// party a round to do something, and only the second turns you to
/// stone. Which is why the second rung is allowed to be five rounds
/// now instead of one.
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
        let effects = simple_weapon_attack(
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
        const DC: i32 = 11;
        let save = encounter.roll_save_vs_condition(
            target_id,
            AbilityScoreType::Constitution,
            DC,
            Condition::Restrained,
        );
        if !save.passed() {
            // Opened on the spot rather than queued as a side effect:
            // the ladder installs a condition *and* writes an encounter
            // ledger, and only the encounter can do the second half.
            encounter.begin_staged_save(
                target_id,
                caster_id,
                DC,
                &crate::engine::staged_saves::PETRIFICATION,
            );
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

impl BansheeWail {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 12;
}

impl Action for BansheeWail {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        for tid in encounter.enemy_burst_targets(caster_id, caster_loc, Self::RADIUS) {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            // Undead are immune to the wail (RAW: "any creature that
            // is not undead"). We proxy by checking for necrotic
            // immunity — matches the wraith / specter / wight pool.
            //
            // And RAW's other clause, which had no engine to enforce it
            // until `can_hear` existed: "each creature within 30 feet
            // of it *that can hear it*". A deafened creature is out of
            // the target set before it touches the dice, so the seeded
            // log shows no save it never made.
            if t.is_immune_to(DamageType::Necrotic) || !t.can_hear() {
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
                effects.extend(crate::engine::side_effects::install_condition_with_link(
                    Condition::Frightened,
                    tid,
                    caster_id,
                    ConditionTimer::Rounds(3),
                ));
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

impl MummyDreadfulGlare {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 8;
}

impl Action for MummyDreadfulGlare {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        const DC: i32 = 11;
        encounter.log("  dreadful glare: the mummy fixes its hollow eyes on the living");
        resolve_los_glare_condition(
            encounter,
            caster_id,
            Self::RADIUS,
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
/// (10 ft), and RAW's knockdown: *"If the target is a Large or smaller
/// creature, it has the Prone condition."*
///
/// Mirrors the ogre's club but bumped to giant-tier dice; the extra
/// reach is the hill giant's signature spacing advantage, and the
/// knockdown is what turns that reach into a lane nobody wants to be
/// in — a Medium creature the club connects with is on the floor, ten
/// feet away, spending half its next turn standing back up inside the
/// swing's envelope.
pub static HILL_GIANT_GREATCLUB: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "giant greatclub",
    &["ggc", "giant-club"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Bludgeoning,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "club sweep",
    2,
)
.against_at_most(Size::Large);

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
/// on a failed DC 12 STR save the target is engulfed. The engulf ends
/// when the cube dies or the target breaks free — modeled by a 5-round
/// timer here, long enough to mimic the engulf duration without locking
/// the target forever if the cube can't be killed in time.
///
/// RAW: *"an engulfed target is suffocating, can't cast spells with a
/// Verbal component, has the Restrained condition, and takes 10 (3d6)
/// Acid damage at the start of each of the cube's turns."* Two of those
/// four clauses ship — the Restrained, and the suffocation as
/// `Condition::Choking`, which is what makes being inside a cube
/// something other than a slow stand-still. The verbal-component clause
/// has no lane (the engine's spellcasting gate is per-caster, not
/// per-component) and the per-turn acid tick is folded into the swing's
/// own 3d6, the same compression the Rug of Smothering uses below.
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
            encounter.log("  pseudopod: target is engulfed, restrained and suffocating");
            // Both conditions on the same timer, because RAW ends them
            // both on the same event: they last "until the grapple
            // ends" and the engine's stand-in for that is the five
            // rounds below. Two installs rather than one loop over a
            // slice, so each carries its own RAW clause in the log
            // above it.
            for condition in [Condition::Restrained, Condition::Choking] {
                effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                    actor_id: target_id,
                    condition,
                    timer: ConditionTimer::Rounds(5),
                }));
            }
        }
        effects
    }
}

pub static GELATINOUS_CUBE_ENGULF: LazyLock<GelatinousCubeEngulf> =
    LazyLock::new(|| GelatinousCubeEngulf {});

/// Generic burst breath weapon — the data-only Action behind every
/// "Recharge 5–6: cone of X, save Y" creature ability in the engine.
/// Each instance carries its own name, alias table, payload, save
/// ability, DC, burst radius, max range and recharge-pool key, so a new
/// breath is a single static declaration rather than a fresh struct and
/// a sixty-line `Action` impl.
///
/// It began as four near-identical `DragonBreath{Fire,Cold,Lightning,
/// Poison}` structs, then grew a fifth sibling — `BreathWeaponCondition`
/// — for the damage-free control cones, and the fifth is what showed
/// the shape was wrong. RAW's breaths are not "damage" *or* "condition":
/// the dust mephit's grit blinds and does nothing else, the ancient
/// dragons' cones only hurt, and the Sphinx of Lore's Mind-Rending Roar
/// does 10d6 psychic **and** leaves everyone it catches Incapacitated
/// off the same save. Two chassis could express the first two and
/// neither could express the third.
///
/// So both payloads are optional and both hang off one saving throw:
///
/// - `damage` — dice and type, rolled **once** and shared across
///   everyone caught, which is what 5e area effects do. Half on a
///   successful save.
/// - `condition` — installed on exactly the creatures that failed *that
///   same save*. Not a second roll: a target that shrugged off the roar
///   shrugged off all of it, and re-rolling would let a creature take
///   full damage and still walk away clean.
///
/// `enemies_only` is the other axis RAW cares about. A dragon's breath
/// is indiscriminate — "each creature in the area", and the engine's
/// neutral resolver agrees, kobolds included — while a celestial's roar
/// is written "each enemy". Getting it backwards has the sphinx
/// stunning the party it was going to question.
///
/// A breath with neither payload is a breath that does nothing;
/// `debug_assert` says so at the one place that can tell.
pub struct BreathWeapon {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Dice and type, or `None` for the pure-control cones.
    pub damage: Option<(Dice, DamageType)>,
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    /// RAW's printed area — "each creature in a 60-foot Cone", "each
    /// creature in a 90-foot-long, 5-foot-wide Line" — as an
    /// `AreaShape`. See `crate::engine::areas`.
    ///
    /// This used to be a `radius` and a `range`, and the comment on
    /// them conceded the substitution: *"RAW dragon breath: 60 ft cone
    /// collapses to a burst-4 / range-6 envelope in this 2.5 ft grid."*
    /// That is not a rounding of a cone, it is a different rule — a
    /// ten-foot ball thrown fifteen feet, which can be dropped behind
    /// the dragon or round a corner, and which the dragon's own escort
    /// stands in. Twenty of the forty dragons breathe a line and twenty
    /// a cone, and under the old model those were the same shape at
    /// four sizes.
    ///
    /// The aim point is a *direction* for both projected shapes, so
    /// `reach_tiles` is the length rather than a separate range: a
    /// breath reaches as far as it reaches.
    pub shape: AreaShape,
    /// Key passed to `is_recharge_available` / `spend_recharge`. Every
    /// vanilla breath weapon shares the `"breath_weapon"` pool so a
    /// chromatic dragon can't double-tap with two different elements.
    pub recharge_key: &'static str,
    /// Installed on everyone the *same* save fails against.
    pub condition: Option<(Condition, ConditionTimer)>,
    /// True for the breaths RAW scopes to enemies rather than to every
    /// creature standing in them.
    pub enemies_only: bool,
}

/// How far from its own body a creature may aim an area that is
/// centred on it — RAW's Emanation, which is not thrown anywhere.
///
/// Two tiles rather than zero because the aim point is still an
/// argument somebody has to supply, and a leash of zero would admit
/// only tiles the creature is standing on. Two is enough slack for a
/// picker to name the tile in front of it and small enough that the
/// area is unmistakably the creature's own.
const EMANATION_AIM_LEASH: isize = 2;

/// RAW's 30-foot Cone, in tiles on the 2.5 ft grid — the length the
/// gaze and breath cones on this roster share (the gorgon's petrifying
/// breath, the medusa's and the basilisk's gaze).
///
/// Named rather than repeated because it is the same sentence in three
/// stat blocks, and because "12" at a call site is the one number in a
/// cone declaration that reads like an accident.
pub const CONE_30_FT: isize = 12;

impl Action for BreathWeapon {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::from_area(self.shape)
    }
    fn reach_tiles(&self) -> Option<isize> {
        // A cone or a line is aimed by naming a tile inside it, so its
        // reach is its own length — `AreaShape::aim_reach` answers for
        // both. A burst's radius says nothing about how far it can be
        // thrown, and the one burst breath on the roster is RAW's
        // *Emanation*, which is centred on the creature rather than
        // thrown at all: hence the short leash.
        Some(self.shape.aim_reach().unwrap_or(EMANATION_AIM_LEASH))
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        self.damage.is_some()
    }
    fn damage_types(&self) -> Vec<DamageType> {
        self.damage.map(|(_, dt)| vec![dt]).unwrap_or_default()
    }
    fn recharge_key(&self) -> Option<&'static str> {
        Some(self.recharge_key)
    }
    fn spares_allies(&self) -> bool {
        self.enemies_only
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
        debug_assert!(
            self.damage.is_some() || self.condition.is_some(),
            "{}: a breath with no payload is a breath that does nothing",
            self.display_name
        );
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the recharge resource before resolving anything, so a
        // mid-resolution bail can't leave the breath both spent AND
        // applied.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge(self.recharge_key);
        }
        // One roll, shared — 5e area effects roll damage once, which is
        // why the roll is here rather than inside the per-target loop.
        let (dice, damage_type) = self.damage.unwrap_or((Dice::new(0, 0), DamageType::Force));
        let raw = if self.damage.is_some() {
            encounter.roll(&dice)
        } else {
            0
        };
        encounter.log(format!(
            "  {}: {} aimed at ({}, {}) (DC {} {}{}{})",
            self.display_name,
            self.shape.label(),
            point.x,
            point.y,
            self.dc,
            self.save_ability,
            self.damage
                .map(|(_, dt)| format!(", {}({}) = {} {}, half on save", dice, raw, raw, dt))
                .unwrap_or_default(),
            self.condition
                .map(|(c, _)| format!(", {} on fail", c))
                .unwrap_or_default(),
        ));
        let targets = if self.enemies_only {
            encounter.enemy_area_targets(caster_id, self.shape, point)
        } else {
            encounter.neutral_area_targets(caster_id, self.shape, point)
        };
        // Shielded allies auto-pass and take nothing (Careful Spell,
        // Sculpt Spells). Enemy-scoped areas exclude allies at the
        // target-list step, so there is nobody left for the sweep to
        // spare.
        let shielded = if self.enemies_only {
            HashSet::new()
        } else {
            encounter.auto_pass_shielded_allies_in(caster_id, self.shape, point)
        };
        // Cover against an area is measured from where the area comes
        // from, which for a breath is the creature's mouth rather than
        // the tile it was aimed through — see `AreaShape::origin`.
        let origin = encounter.area_origin(caster_id, self.shape, point);
        let (mut effects, saves) = crate::actions::action_template::resolve_burst_targets(
            encounter,
            caster_id,
            origin,
            &targets,
            self.save_ability,
            self.dc,
            raw,
            damage_type,
            crate::engine::saves::SaveDamagePolicy::HalfOnSave,
            &shielded,
        );
        let Some((condition, timer)) = self.condition else {
            return effects;
        };
        // The same save decides both halves. A creature that made it
        // took half the damage and none of the condition; re-rolling
        // here would let it do the opposite, which is not a rule
        // anybody wrote.
        for (target_id, passed) in saves {
            if passed {
                continue;
            }
            effects.extend(crate::engine::side_effects::install_condition_with_link(
                condition, target_id, caster_id, timer,
            ));
        }
        effects
    }
}

/// Roll a point-centred burst once, log it, and resolve every save in
/// it — the body shared by `BreathWeapon` and `PointBurstSaveDamage`.
///
/// The two differ in exactly two things: whether a recharge gates the
/// action, and whether allies standing in the blast take it. Everything
/// between "roll the dice" and "hand back the damage effects" is the
/// same rule, and was written twice for about a day.
///
/// Named separately rather than folded into one of them because the
/// eventual shape is visible from here: `BreathWeapon` is
/// `RechargingAttack` wrapped around `PointBurstSaveDamage`, and the
/// only reason it is not spelled that way today is the twenty struct
/// literals across the dragons, the mephits and the gorgon that would
/// have to move at once. Extracting the body first is what makes that
/// migration a rename rather than a rewrite.
#[allow(clippy::too_many_arguments)]
fn resolve_point_burst(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    display_name: &str,
    damage_dice: Dice,
    damage_type: DamageType,
    save_ability: AbilityScoreType,
    dc: i32,
    radius: isize,
    enemies_only: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // 5e area effects roll damage once and share it across everyone
    // caught, which is why the roll is here rather than per target.
    let raw = encounter.roll(&damage_dice);
    encounter.log(format!(
        "  {}: {}({}) = {} {} area (DC {} {}, half on save)",
        display_name, damage_dice, raw, raw, damage_type, dc, save_ability,
    ));
    if enemies_only {
        crate::actions::action_template::resolve_enemy_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            save_ability,
            dc,
            raw,
            damage_type,
            crate::engine::saves::SaveDamagePolicy::HalfOnSave,
        )
    } else {
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            save_ability,
            dc,
            raw,
            damage_type,
        )
    }
}

/// A point-centred, save-for-half damage burst that anybody can use on
/// any turn — `BreathWeapon` with the recharge clause struck out, and
/// with a switch for whose side of the blast counts.
///
/// RAW writes plenty of these: the planetar's Holy Burst is *"each enemy
/// in a 20-foot-radius Sphere centered on a point the planetar can see
/// within 120 feet"*, twice per Multiattack and gated on nothing at all.
/// Until now the engine's only point burst was the breath chassis, so an
/// at-will one had a choice between a recharge RAW does not give it and
/// a bespoke `impl Action`.
///
/// `enemies_only` is the second difference and the one that is not
/// cosmetic. A dragon's breath is indiscriminate — RAW says *each
/// creature in the area* and the engine's neutral resolver agrees — and
/// an angel's is not. Getting that backwards would either have the
/// planetar irradiating the party it came to help or the dragon politely
/// breathing around its own kobolds.
pub struct PointBurstSaveDamage {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    /// Footprint-gap radius of the blast, in the roster's 5-ft grid
    /// squares rather than in 2.5-ft tiles — Fireball's 20-foot sphere
    /// is 4 here and Circle of Death's 30-foot one is 6. Not the same
    /// scale as `range` below; see `PLANETAR_HOLY_BURST` for why both
    /// conventions are load-bearing and what mixing them costs.
    pub radius: isize,
    /// Max distance in 2.5-ft tiles from the caster's footprint to the
    /// burst centre — the scale every spell's range line uses.
    pub range: isize,
    /// True for the bursts RAW scopes to *enemies* rather than to every
    /// creature in the area. See the type docs.
    pub enemies_only: bool,
}

impl Action for PointBurstSaveDamage {
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
    fn spares_allies(&self) -> bool {
        self.enemies_only
    }
    /// Three quarters of the pool, the same estimate `AtWillEnemyBurst`
    /// makes and for the same reason: save-for-half against one target
    /// at even odds averages three quarters, which is the number the
    /// attack picker wants when ranking this against a swing.
    fn expected_damage(&self, _encounter: &EncounterInstance, _caster_id: usize) -> Option<f32> {
        Some(self.damage_dice.average_roll() * 0.75)
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
        resolve_point_burst(
            encounter,
            caster_id,
            point,
            self.display_name,
            self.damage_dice,
            self.damage_type,
            self.save_ability,
            self.dc,
            self.radius,
            self.enemies_only,
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


/// Behir lightning breath — RAW's 90-foot-long, 5-foot-wide Line. 12d10 lightning, DC 16
/// DEX, half on save. Recharge 5-6. Behir's signature: a 20 ft line
/// of lightning that approximates as a small burst here. CR-11
/// damage with the same recharge-pool key the dragons use so a
/// dragon-led ambush can't double-tap with two breaths from different
/// actors via the same key.
pub static BEHIR_LIGHTNING_BREATH: BreathWeapon = BreathWeapon {
    display_name: "lightning breath",
    aliases: &["lb", "breath", "blast"],
    damage: Some((Dice::new(12, 10), DamageType::Lightning)),
    save_ability: AbilityScoreType::Dexterity,
    dc: 16,
    shape: AreaShape::Line {
        length: 36,
        half_width: 1,
    },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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
    // RAW 30/120 ft. Both numbers compress onto a board two dozen tiles
    // wide: 12 tiles is the normal band, 20 the outer one the bolt can
    // still reach at disadvantage.
    20,
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
pub static VAMPIRE_CHARMING_GAZE: SaveOrCharm = SaveOrCharm::action(
    "charming gaze",
    &["cg", "gaze"],
    // 30 ft RAW = 12 tiles.
    12,
    17,
    ConditionTimer::Rounds(10),
    "charming gaze",
);

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

/// Draconic Strike — Way of the Ascendant Dragon Monk (subclass level
/// 3, FTD). RAW: "when you deal damage with your Unarmed Strike, you
/// can change its damage type to the damage type associated with your
/// Draconic Ancestry."
///
/// A second `SimpleWeapon` beside the monk's fist rather than a runtime
/// retype of it, because the feature *is* a choice and the engine
/// already has a lane that makes choices between swings: the AI's
/// attack picker ranks candidate weapons by damage-type matchup, and a
/// human at the prompt picks by name. Two weapons on the sheet is
/// therefore the same decision RAW asks for, made by the same machinery
/// that decides between a longsword and a lance — where a retype would
/// have needed a per-hit override channel neither controller has.
///
/// Identical to `MONK_UNARMED_STRIKE` in every other respect (DEX to
/// hit and to damage, the 1d8 martial-arts die, 5 ft of reach), so
/// Extra Attack chains it and Flurry of Blows throws it exactly as it
/// does the fist. The one difference is the whole feature: fire instead
/// of bludgeoning, which is the right swing against the skeletons and
/// the trolls and the wrong one against everything that lives in a
/// volcano — and having both on the sheet is what makes that a
/// decision rather than a fixed downgrade.
///
/// Fire because the chassis that carries it declares a fire
/// `draconic_ancestry`, which is also what Breath of the Dragon reads.
/// A second ancestry ships as a second template pairing a different
/// `SimpleWeapon` with a different ancestry, the way the fifteen
/// Dragonborn ancestries already do.
pub static DRACONIC_STRIKE: SimpleWeapon = SimpleWeapon::melee(
    "draconic strike",
    &["dstrike", "draconic"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Fire,
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
pub static TARRASQUE_BITE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "tarrasque bite",
    &["t-bite", "tbite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "tarrasque jaws",
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

/// Aboleth **Tentacle** — 2d6+STR bludgeoning at reach 2 (10 ft), and
/// RAW's hold: *"If the target is a Large or smaller creature, it has
/// the Grappled condition (escape DC 14) from one of four tentacles."*
///
/// Four tentacles is the number that matters. The aboleth's Consume
/// Memories only targets a creature that is *"Charmed or Grappled by
/// the aboleth"*, so the tentacles are not a damage lane at all — they
/// are the setup for the action that actually kills you, and without
/// the grapple the creature's whole turn structure came apart into two
/// unrelated attacks.
pub static ABOLETH_TENTACLE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "tentacle",
    &["tent"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    &[Condition::Grappled],
    ConditionTimer::Permanent,
    "one of four tentacles",
    2,
)
.against_at_most(Size::Large);

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
/// a bolt of lightning at a single target within 500 ft (capped to the
/// board's 40 tiles). DC 17 DEX save: half on a pass, the full 8d10
/// lightning on a fail. The bonus-action cost is what makes it matter —
/// the giant throws it *and* swings, every turn.
pub static STORM_GIANT_LIGHTNING_STRIKE: SingleTargetSaveDamage = SingleTargetSaveDamage::new(
    "lightning strike",
    &["lstrike", "sgls"],
    // 500 ft RAW — capped to map width.
    40,
    AbilityScoreType::Dexterity,
    17,
    Dice::new(8, 10),
    DamageType::Lightning,
)
.as_bonus_action();

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

/// Stone Giant **Boulder** — 4d10+STR bludgeoning at reach 24, and
/// RAW's knockdown: *"If the target is a Large or smaller creature, it
/// has the Prone condition."*
///
/// The first entry on the rider chassis's new ranged constructor, and
/// the clause is what makes the stone giant's ranged lane a threat
/// rather than a fallback: a boulder that lands puts somebody sixty feet
/// away on the floor, which costs them half their movement on a turn
/// they were going to spend closing.
pub static STONE_GIANT_BOULDER: WeaponWithCondition = WeaponWithCondition::ranged(
    "stone boulder",
    &["s-boulder", "sboulder"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "boulder impact",
    24,
    16,
)
.against_at_most(Size::Large);

/// Stone Giant Multiattack — 2 greatclub swings per Action. Mirrors the
/// frost giant / hill giant pattern: physical thresher boss melee, no
/// rider effects, raw bludgeoning damage.
pub static STONE_GIANT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "stone giant multiattack",
    sub_attack: &STONE_GIANT_GREATCLUB,
    count: 2,
});

/// Medusa **Petrifying Gaze** — a CON save at DC 13 (RAW's number) on
/// SRD 5.2's two-stage ladder: a first failure stiffens the target, and
/// a second, at the end of its next turn, turns it to stone. No damage;
/// the ladder is the entire threat.
///
/// **Delivery diverges.** RAW makes this a Bonus Action against a
/// 30-foot Cone, on Recharge 5–6. The engine keeps it as a single
/// target the medusa can see, which is the shape it has always had, and
/// the reason is the AI rather than the geometry: the area pickers want
/// two bodies in the shape before they will spend a turn on it, so a
/// medusa facing one adventurer would simply stop gazing. Worth
/// revisiting when the area lane grows a single-target rung.
///
/// What was wrong and is now right is the *ladder*, which is where the
/// gaze's whole character lives — see `staged_saves::PETRIFICATION`.
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // A creature already climbing a ladder gains nothing from being
        // pushed onto one: `begin_staged_save` declines a second entry,
        // so the gaze would roll, fail, and do nothing at all.
        //
        // This gate is what the `LOCKDOWNS` marker used to do on its
        // own. That marker names `Petrified` — the ladder's *second*
        // rung — and a creature on the first rung does not have it, so
        // the AI would re-gaze the same victim every round for the rest
        // of the fight. Which is a softer replay of the bug this
        // action's `deals_damage` declaration was added to fix, and the
        // reason the state is asked about here rather than encoded in a
        // second marker: the ledger is the truth, and a marker is a
        // copy of it.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        if encounter.staged_save_pending(target_id) {
            return false;
        }
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| !a.has_condition(Condition::Petrified))
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
        const DC: i32 = 13;
        let save = encounter.roll_save_vs_condition(
            target_id,
            AbilityScoreType::Constitution,
            DC,
            Condition::Restrained,
        );
        if save.passed() {
            encounter.log("  petrifying gaze: target averts their eyes");
            return Vec::new();
        }
        encounter.begin_staged_save(
            target_id,
            caster_id,
            DC,
            &crate::engine::staged_saves::PETRIFICATION,
        );
        Vec::new()
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

/// Ghost **Horrific Visage** — SRD 5.2: *"Wisdom Saving Throw: DC 13,
/// each creature in a 60-foot Cone that can see the ghost and isn't an
/// Undead. Failure: 10 (2d6 + 3) Psychic damage, and the target has the
/// Frightened condition until the start of the ghost's next turn.
/// Success: The target is immune to this ghost's Horrific Visage for 24
/// hours."*
///
/// Four clauses, and the version this replaces had one of them. It was
/// a `NoArgs` sixty-foot *sphere* — the ghost frightening everything in
/// every direction, which at CR 4 is most of a generated board — with
/// no damage, no sight gate, no type filter, and a five-round timer in
/// place of RAW's one. The docstring conceded the immunity clause was
/// "not modeled — single-encounter scope", which was true of the engine
/// when it was written and is no longer: the ledger the ghast's Stench
/// banks its saves in is the same sentence, so this banks in it too.
///
/// The two gates are what make the ghost a thing you can *do something
/// about*. A cone is faced, so a party spread around the room takes it
/// in ones; the sight clause means a blinded fighter, or one round a
/// corner, walks through it; and the success clause means a party that
/// makes its saves is finished with this ghost's face for good.
///
/// `deals_damage` still answers false. The psychic damage is real but
/// incidental — the flag is what stops the AI's focus-fire lane
/// treating a fear cone as a way to whittle somebody down, which is the
/// bug that had a medusa re-gazing a petrified corpse four hundred
/// times.
pub struct GhostHorrifyingVisage {}

impl GhostHorrifyingVisage {
    const DC: i32 = 13;
    /// RAW's 60-foot Cone.
    const CONE: isize = 24;
    /// RAW's "10 (2d6 + 3) Psychic damage" — the dice and the flat
    /// term, which is the ghost's own Charisma modifier written out.
    const DAMAGE: Dice = Dice::new(2, 6);
    const DAMAGE_BONUS: u32 = 3;
    /// The name the 24-hour immunity is banked under. A `&'static str`
    /// because the ledger is keyed by trait name — see
    /// `EncounterInstance::bank_trait_immunity`.
    const TRAIT: &'static str = "Horrific Visage";
}

impl Action for GhostHorrifyingVisage {
    fn name(&self) -> &str {
        "horrifying visage"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hv", "visage"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Cone { length: Self::CONE }
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn deals_damage(&self) -> bool {
        // Frightens first and foremost; see the type docs for why the
        // psychic damage does not flip this.
        false
    }
    fn spares_allies(&self) -> bool {
        // The cone resolves through `enemy_area_targets`, so the
        // ghost's own side is never in it. Newly load-bearing: this was
        // a `NoArgs` self-centred burst before it was a cone, so it did
        // not pass through the AI's area placement search at all — and
        // that search vetoes any placement catching an ally unless the
        // action declares it spares them.
        true
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::areas::AreaShape;
        use crate::engine::side_effects::DealDamage;

        let Some(aim) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let shape = AreaShape::Cone { length: Self::CONE };
        // One roll, shared, the way every area in the engine rolls.
        let rolled = encounter.roll(&Self::DAMAGE) + Self::DAMAGE_BONUS;
        encounter.log(format!(
            "  horrific visage: a 60 ft cone of dread (DC {} WIS, {}{:+} = {} psychic on a fail)",
            Self::DC,
            Self::DAMAGE,
            Self::DAMAGE_BONUS,
            rolled
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_area_targets(caster_id, shape, aim) {
            // "…and isn't an Undead." One dead thing does not frighten
            // another.
            if encounter
                .actors
                .get(&tid)
                .is_some_and(|a| a.creature_type().is_undead())
            {
                continue;
            }
            // "…that can see the ghost."
            if !encounter.viewer_can_see(tid, caster_id) {
                continue;
            }
            // "…is immune to this ghost's Horrific Visage for 24 hours",
            // banked by a previous success.
            if encounter.trait_immunity_banked(tid, caster_id, Self::TRAIT) {
                continue;
            }
            let save = encounter.roll_save_against_caster_vs_condition(
                tid,
                AbilityScoreType::Wisdom,
                Self::DC,
                caster_id,
                Condition::Frightened,
            );
            if save.passed() {
                encounter.bank_trait_immunity(tid, caster_id, Self::TRAIT);
                let name = encounter.actor_name(tid);
                encounter.log(format!(
                    "  {} holds their nerve, and will not flinch at this ghost again.",
                    name
                ));
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: rolled,
                damage_type: DamageType::Psychic,
            }));
            // Through the linked installer rather than a bare
            // `ApplyCondition`, because Frightened is one of the
            // conditions that has to remember *who*: without the link a
            // frightened creature reads as unable to approach any enemy
            // at all, and cannot run away from the ghost, which is the
            // one thing being frightened of a ghost should make it do.
            effects.extend(crate::engine::side_effects::install_condition_with_link(
                Condition::Frightened,
                tid,
                caster_id,
                // RAW's "until the start of the ghost's next turn" — a
                // round, measured from the ghost's slot. The engine's
                // `UntilStartOfNextTurn` is holder-relative and would
                // lift it at the top of the *victim's* turn, which is
                // the turn the fear is supposed to ruin.
                ConditionTimer::Rounds(1),
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

impl StoneGolemSlow {
    /// How far the ability reaches, in tiles — RAW's 10 feet.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 2;
}

impl Action for StoneGolemSlow {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        encounter.log("  stone golem slow: enemies make a WIS save vs DC 17");
        crate::actions::action_template::resolve_burst_save_condition(
            encounter,
            caster_id,
            caster_loc,
            Self::RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Slowed,
            ConditionTimer::Rounds(5),
        )
    }
}

pub static STONE_GOLEM_SLOW: LazyLock<StoneGolemSlow> = LazyLock::new(|| StoneGolemSlow {});

/// Bulette Bite — STR-based 4d12+STR piercing melee, reach 1. The
/// bulette's signature crunch — averages ~26 piercing per hit. No
/// rider effects; pure damage. Stays a `SimpleWeapon` so the bulette's
/// loadout can mix this with the Deadly Leap follow-up cleanly.
pub static BULETTE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bulette bite",
    &["bbite", "bulette-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
);

/// Bulette Deadly Leap — Action; the bulette jumps onto a target,
/// landing with crushing force. We model as a single melee swing dealing
/// 3d6+STR bludgeoning, plus the target makes a STR save vs DC 16 or
/// is knocked Prone. The leap is RAW reserved for the Bulette's bonus
/// "Deadly Leap" action; we expose it as a regular Action lane so the
/// AI can pick between Bite and Leap based on whether knocking the
/// target prone (e.g. setting up an ally's melee crit window) is worth
/// the lower damage tier.
pub struct BuletteDeadlyLeap {}

impl Action for BuletteDeadlyLeap {
    fn name(&self) -> &str {
        "bulette deadly leap"
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

pub static BULETTE_DEADLY_LEAP: LazyLock<BuletteDeadlyLeap> =
    LazyLock::new(|| BuletteDeadlyLeap {});

/// Bulette Multiattack — Action: 2 bites. The bulette's RAW
/// multiattack is one Bite; we double it so the CR-5 bulette can keep
/// pace with the other CR-5 boss-tier templates (manticore, etc.)
/// without needing to chain back-to-back Action picks.
pub static BULETTE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "bulette multiattack",
    sub_attack: &BULETTE_BITE,
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

/// Balor **Whip** — 2d6+STR slashing plus a flat 3d6 lightning at reach
/// 12 (30 ft), and the haul that comes with it: the target is dragged
/// twenty-five feet toward the balor and put on the floor.
///
/// The reach is the whole tactic. A balor does not close; it stands
/// thirty feet back, hooks somebody, drags them into the middle of its
/// own melee and knocks them flat there — and the party's ranged line
/// loses whoever was hooked. Without the drag this was a long-reach
/// swing that did rather a lot of damage, which is a different and much
/// duller creature.
///
/// The drag and the knockdown are printed on both the 2014 and the SRD
/// 5.2 balor and both gate them on size; 5.2's Flame Whip gates at
/// **Huge or smaller** and makes both automatic, which is what this
/// carries. The dice and damage types are the ones this stat block
/// already shipped with — 5.2 rewrites them as Force plus Fire, which is
/// a change to the balor's whole damage identity and belongs with a
/// pass over its resistance table rather than with this clause.
///
/// This was a fifty-line hand-rolled `impl Action` whose body was one
/// `simple_weapon_attack` and one `add_flat_damage_rider`, for the
/// reason the behir's Constrict was: no chassis could say "damage, plus
/// damage, plus a rider". One now can.
pub static BALOR_WHIP: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "balor whip",
    &["bwhip", "lwhip"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "whip haul",
    12,
)
.against_at_most(Size::Huge)
.plus_damage(Dice::new(3, 6), DamageType::Lightning, "balor whip lightning")
.pulls(tiles_from_feet(25));

/// Balor Multiattack — 1 longsword + 1 whip per Action via
/// `CompoundAttack`. The full apex-demon opening salvo: a longsword
/// (3d8 slash + 3d8 lightning) and a whip (2d6 slash + 3d6 lightning)
/// can ladder up to ~50-60 damage on a single target in one round.
pub static BALOR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "balor multiattack",
    parts: vec![(&*BALOR_LONGSWORD, 1), (&BALOR_WHIP, 1)],
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
    fn self_burst_radius(&self) -> Option<isize> {
        // RAW: "each creature within 5 feet of it". Not the balor's
        // other 8-tile clause on the same stat block.
        Some(1)
    }
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
///
/// The damage type used to be `Bludgeoning`, against RAW, against the
/// weapon's own name, and against the line of documentation directly
/// above it — six longswords doing blunt-force damage. It mattered
/// more than a typo usually does, because the marilith's whole threat
/// is volume: every one of those six swings read the wrong column of
/// the target's resistance table, so a skeleton (vulnerable to
/// bludgeoning, resistant to slashing) took the demon's routine at
/// double rate instead of half, and everything carrying the ordinary
/// physical-resistance package shrugged off the wrong half of it.
pub static MARILITH_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "marilith longsword",
    &["mls", "marilith-ls"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Slashing,
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

impl VrockScreech {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 8;
}

impl Action for VrockScreech {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        for tid in encounter.enemy_burst_targets(caster_id, center, Self::RADIUS) {
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

/// Umber Hulk Mandibles — STR-based 2d8+STR piercing melee. RAW: "Hit:
/// 14 (2d8 + 5) piercing damage." The heavy half of the hulk's
/// multiattack, and the reason its claws are only 1d8 apiece.
pub static UMBER_MANDIBLES: SimpleWeapon = SimpleWeapon::melee(
    "umber mandibles",
    &["umandibles", "mandibles"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
);

/// Umber Hulk Multiattack — "The umber hulk makes three attacks: one
/// with its mandibles and two with its claws." Heterogeneous, so
/// `CompoundAttack`; without it the hulk swung once a turn for 1d8+5,
/// which is a CR 2's output on a CR 5 chassis.
pub static UMBER_HULK_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "umber hulk multiattack",
    parts: vec![(&UMBER_MANDIBLES, 1), (&UMBER_CLAW, 2)],
});

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
        let effects = simple_weapon_attack(
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
        const DC: i32 = 12;
        let save = encounter.roll_save_vs_condition(
            target_id,
            AbilityScoreType::Constitution,
            DC,
            Condition::Restrained,
        );
        if !save.passed() {
            encounter.begin_staged_save(
                target_id,
                caster_id,
                DC,
                &crate::engine::staged_saves::PETRIFICATION,
            );
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
    &[Condition::Grappled],
    ConditionTimer::Rounds(10),
    "chuul pincer",
)
.against_at_most(Size::Large);

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
    &[Condition::Grappled],
    ConditionTimer::Rounds(10),
    "scorpion claw",
)
.against_at_most(Size::Large);

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
)
.mastery(WeaponMastery::Graze);

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
)
.mastery(WeaponMastery::Slow);

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
/// straight-line run, and the free bite against the target it
/// flattens — through `TIGER_POUNCE`.
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

/// Giant Toad **Bite** — 1d10+STR piercing, a flat 1d10 poison, and
/// RAW's hold: *"If the target is a Medium or smaller creature, it has
/// the Grappled condition (escape DC 12)."*
///
/// All three at once, which took a chassis that could say so. This
/// carried two stacked docstrings for a while, one apologising for the
/// grapple and one for the poison, because the weapon-rider family was
/// split down the middle: `WeaponWithRider` did "damage plus damage" and
/// `WeaponWithCondition` did "damage plus a hold", and the toad is both.
/// `plus_damage` closed that seam.
///
/// The hold is also what the toad's Swallow is waiting on — RAW eats "a
/// Medium or smaller target it is grappling" — so a bite that only dealt
/// damage left the creature's signature clause with nothing to fire at.
///
/// Still skipped: RAW's "on a successful CON save it takes half the
/// poison" wrinkle. The toad is CR 1 and the rider is the flavour.
pub static GIANT_TOAD_BITE: WeaponWithCondition = WeaponWithCondition::melee(
    "bite",
    &["b", "chomp"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Piercing,
    &[Condition::Grappled],
    ConditionTimer::Permanent,
    "sticky tongue",
)
.against_at_most(Size::Medium)
.plus_damage(Dice::new(1, 10), DamageType::Poison, "bite poison");

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

/// Behir **Constrict** — 2d10+STR bludgeoning plus a flat 2d10
/// slashing, and the coils that follow: RAW's *"the target has the
/// Grappled condition (escape DC 16), and it has the Restrained
/// condition until the grapple ends"*, gated at Large or smaller.
///
/// The grapple half used to be the missing half, and it was missing for
/// a structural reason rather than an oversight: this was a hand-rolled
/// fifty-line `impl Action` because no chassis could say "damage, plus
/// damage, plus a hold". `WeaponWithCondition::plus_damage` is that
/// chassis now, and the fifty lines are one declaration.
///
/// It is also the attack the behir's Swallow is waiting on. Nothing on
/// the sheet could grapple, so the Bonus Action that eats a grappled
/// creature had nothing to eat.
///
/// SRD 5.2 prints Constrict as a Strength save rather than an attack
/// roll; the engine keeps the attack roll this shipped with, which is
/// the 2014 wording and the convention every other grapple-on-hit in
/// the bestiary follows. The difference is who rolls the die, not what
/// happens when it lands.
pub static BEHIR_CONSTRICT: WeaponWithCondition = WeaponWithCondition::melee(
    "constrict",
    &["c", "coil", "crush"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Bludgeoning,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "behir coils",
)
.against_at_most(Size::Large)
.plus_damage(Dice::new(2, 10), DamageType::Slashing, "constrict slash");

/// Behir multiattack — one bite + one constrict per Action. The
/// signature melee burst the serpent leads with when its lightning
/// breath is on cooldown.
pub static BEHIR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "bite + constrict",
    parts: vec![(&BEHIR_BITE, 1), (&BEHIR_CONSTRICT, 1)],
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

/// Roc **Talons** — 4d6+STR slashing at reach 2 (10 ft), and RAW's
/// grip: *"If the target is a Huge or smaller creature, it has the
/// Grappled condition (escape DC 19) from both of the roc's talons, and
/// it has the Restrained condition until the grapple ends."*
///
/// The widest ceiling in the bestiary, and it is the creature's whole
/// silhouette: a roc is the thing that picks up an ogre. Everything
/// short of Gargantuan is inside the clause, which is a very different
/// statement from the Large-or-smaller most of the book prints and the
/// reason `max_target_size` is per-row rather than a constant.
pub static ROC_TALONS: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "roc talons",
    &["rtalons"],
    AbilityScoreType::Strength,
    Dice::new(4, 6),
    DamageType::Slashing,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "both talons",
    2,
)
.against_at_most(Size::Huge);

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

/// Winter Wolf cold breath — RAW's 15-foot Cone, 4d8 cold, DC 12 CON,
/// half on save. Recharge 5-6 via the shared `"breath_weapon"` pool.
/// CR-3 dice tier — well below dragon breath, well above the wolf trip,
/// and six tiles of cone is short enough that the pack has to be on top
/// of somebody to use it.
pub static WINTER_WOLF_BREATH: BreathWeapon = BreathWeapon {
    display_name: "cold breath",
    aliases: &["wwc", "frost-breath"],
    damage: Some((Dice::new(4, 8), DamageType::Cold)),
    save_ability: AbilityScoreType::Constitution,
    dc: 12,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
};

/// Triceratops Gore — STR-based 4d8+STR piercing, reach 2 (10 ft). The
/// huge ceratopsian's signature charge: high single-die damage that
/// rewards reach over multi-strike spam. RAW's **Trampling Charge** —
/// Prone on a failed STR save after a straight-line move-then-hit, then
/// a bonus-action stomp on the target it flattens — rides this weapon
/// through `TRICERATOPS_CHARGE` in `engine::attack`.
pub static TRICERATOPS_GORE: SimpleWeapon = SimpleWeapon::reach_melee(
    "gore",
    &["gr", "horn-charge"],
    AbilityScoreType::Strength,
    Dice::new(4, 8),
    DamageType::Piercing,
    2,
);

/// Triceratops Stomp — STR-based 3d10+STR bludgeoning, reach 1. RAW
/// reaches for it in two places: as the bonus-action follow-up a
/// Trampling Charge earns against the target it flattened (wired by
/// `TRICERATOPS_CHARGE::prone_follow_up`), and — as we expose it — as a
/// vanilla swing the AI can pick when the gore is out of reach.
pub static TRICERATOPS_STOMP: SimpleWeapon = SimpleWeapon::melee(
    "stomp",
    &["st", "trample"],
    AbilityScoreType::Strength,
    Dice::new(3, 10),
    DamageType::Bludgeoning,
);

/// Tyrannosaurus Rex **Bite** — 4d12+STR piercing at reach 2 (10 ft),
/// and RAW's jaws: *"If the target is a Large or smaller creature, it
/// has the Grappled condition (escape DC 17). While Grappled, the target
/// has the Restrained condition and can't be targeted by the
/// tyrannosaurus's Tail."*
///
/// The hold is what makes the two limbs a *choice*. RAW's tail is the
/// sweep for everything the jaws are not holding, and the bite is the
/// lane that takes one creature out of the fight and keeps it — which is
/// the whole shape of fighting a tyrannosaurus, and was previously a
/// docstring saying grapple-from-monster was "a niche the engine doesn't
/// currently use on huge predators".
///
/// The "can't be targeted by the Tail" clause is still not modeled; the
/// engine has no per-action target lock.
pub static T_REX_BITE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "rex bite",
    &["rb", "trex-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 12),
    DamageType::Piercing,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "rex jaws",
    2,
)
.against_at_most(Size::Large);

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
/// (half on save), is knocked Prone (only on fail — the surge staggers
/// off-balance victims into the muck), and — RAW's own clause, not the
/// engine's compression — *"is suffocating unless it can breathe
/// water"*. Recharge 4-6 per RAW;
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
                // RAW: "the target has the Restrained condition, **is
                // suffocating unless it can breathe water**, and takes
                // 9 (2d8) Bludgeoning damage at the start of each of
                // the elemental's turns."
                //
                // The "unless" is asked here rather than inside
                // `Condition::Choking`, which is where RAW puts it: a
                // merfolk held under a wave is breathing, and the same
                // merfolk with a darkmantle over its face is not. The
                // condition means the airway is blocked; who gets one
                // is this attack's business. See `engine::breath`.
                let drowns = encounter
                    .actors
                    .get(&vid)
                    .is_some_and(|a| !a.breathes_underwater());
                if drowns {
                    effects.push(Box::new(ApplyCondition {
                        actor_id: vid,
                        condition: Condition::Choking,
                        // Two rounds, the same window the engine's
                        // other hold-and-crush riders use. RAW's is
                        // "until the grapple ends" and the grapple
                        // itself is compressed into the knockdown
                        // above, so the timer is what stands in for the
                        // victim pulling free.
                        timer: ConditionTimer::Rounds(2),
                    }));
                }
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


// ─── Metallic dragon second breaths ──────────────────────────────────

/// What a metallic dragon's *second* breath does to a creature that
/// fails its save.
///
/// Five colours, five clauses, one chassis — because everything else
/// about them is identical. Each is a cone out of the dragon's mouth
/// with a save and no damage at all; what differs is the sentence after
/// "Failure:".
#[derive(Debug, Clone, Copy)]
pub enum MetallicBreathEffect {
    /// Brass and silver: SRD 5.2's two-stage ladder — see
    /// `crate::engine::staged_saves`.
    Ladder(&'static crate::engine::staged_saves::StagedSave),
    /// Bronze's **Repulsion Breath**: *"the target is pushed up to N
    /// feet straight away from the dragon and has the Prone
    /// condition."*
    Repulse { push_feet: u32 },
    /// Copper's **Slowing Breath**: *"the target can't take Reactions;
    /// its Speed is halved; and it can take either an action or a Bonus
    /// Action on its turn, not both. This effect lasts until the end of
    /// its next turn."*
    ///
    /// All three clauses are the `Slowed` condition's, which the engine
    /// grew for the Slow spell. Slowed carries two the breath does not
    /// — a −2 to AC and to Dexterity saves — so the copper's version
    /// lands a little harder than RAW prints it. Named rather than
    /// worked around: the alternative is a second condition that
    /// differs from this one by two numbers and duplicates its five
    /// consumers, and the breath is a one-round effect where the
    /// overshoot is worth about one point of expected damage.
    Slow,
    /// Gold's **Weakening Breath**: *"the target has Disadvantage on
    /// Strength-based D20 Tests and subtracts 1dN from its damage
    /// rolls."*
    ///
    /// Modelled as `Enfeebled`, which halves the holder's weapon damage
    /// rather than subtracting a die. A halving is the harsher reading
    /// at the top of the ladder and the gentler one at the bottom, and
    /// it is the clause the engine already has; the disadvantage half
    /// is not modelled, because nothing in the engine rolls a
    /// Strength-based check often enough for it to be felt.
    Weaken,
}

impl MetallicBreathEffect {
    /// The condition a creature already carrying it would gain nothing
    /// from — RAW's *"each creature that isn't currently affected by
    /// this breath"* on the gold's version, and the honest answer for
    /// the other four, whose second application is a no-op the dragon
    /// should not be spending its Action on.
    ///
    /// `None` for the repulsion breath, which does something to a
    /// creature every single time: there is no state to be already in,
    /// only sixty feet of floor to be thrown across.
    fn redundant_when(self) -> Option<Condition> {
        match self {
            MetallicBreathEffect::Ladder(l) => Some(l.first),
            MetallicBreathEffect::Repulse { .. } => None,
            MetallicBreathEffect::Slow => Some(Condition::Slowed),
            MetallicBreathEffect::Weaken => Some(Condition::Enfeebled),
        }
    }
}

/// The second breath every **metallic** dragon has and no chromatic one
/// does — the one that does not deal damage.
///
/// SRD 5.2 gives brass, bronze, copper, gold and silver dragons two
/// breath actions apiece: the elemental one on Recharge 5–6, and a
/// control one with no recharge at all. The engine had the first of
/// each and none of the second, which is half of twenty stat blocks'
/// action lists — and the more characterful half. A brass dragon that
/// cannot put anybody to sleep is a red dragon that breathes fire in a
/// line.
///
/// **No recharge, deliberately.** RAW prints these without one, and the
/// limiter it prints instead is the effect itself: a creature already
/// asleep, already slowed, already weakened or already stiffening gains
/// nothing from a second dose, and `custom_validate_input` refuses the
/// cast when *nobody* in the cone would be newly affected. That is
/// RAW's own gate on the gold's version ("each creature that isn't
/// currently affected by this breath") applied to all five, and it is
/// also what stops the AI spending a dragon's Action every round
/// re-breathing on two creatures it has already put down.
///
/// `deals_damage` is false on all five. They deal none, and the flag is
/// what keeps the focus-fire lane from ranking a sleep cone as a way to
/// whittle somebody's hit points down.
pub struct MetallicBreath {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub save_ability: AbilityScoreType,
    pub dc: i32,
    /// The cone's length in tiles. RAW's ladder for four of the five is
    /// the age ladder — 15 / 30 / 60 / 90 feet — and the bronze's
    /// repulsion is thirty feet at every age.
    pub cone: isize,
    pub effect: MetallicBreathEffect,
}

impl MetallicBreath {
    /// Brass — Sleep Breath.
    pub const fn sleep(dc: i32, cone: isize) -> Self {
        Self {
            display_name: "sleep breath",
            aliases: &["sleep", "sb"],
            save_ability: AbilityScoreType::Constitution,
            dc,
            cone,
            effect: MetallicBreathEffect::Ladder(
                &crate::engine::staged_saves::SLEEP_BREATH,
            ),
        }
    }

    /// Silver — Paralyzing Breath.
    pub const fn paralyzing(dc: i32, cone: isize) -> Self {
        Self {
            display_name: "paralyzing breath",
            aliases: &["paralyze", "pzb"],
            save_ability: AbilityScoreType::Constitution,
            dc,
            cone,
            effect: MetallicBreathEffect::Ladder(
                &crate::engine::staged_saves::PARALYZING_BREATH,
            ),
        }
    }

    /// Bronze — Repulsion Breath. Thirty feet of cone at every age; what
    /// grows is how far it throws you.
    pub const fn repulsion(dc: i32, push_feet: u32) -> Self {
        Self {
            display_name: "repulsion breath",
            aliases: &["repulse", "rb"],
            save_ability: AbilityScoreType::Strength,
            dc,
            cone: CONE_30_FT,
            effect: MetallicBreathEffect::Repulse { push_feet },
        }
    }

    /// Copper — Slowing Breath.
    pub const fn slowing(dc: i32, cone: isize) -> Self {
        Self {
            display_name: "slowing breath",
            aliases: &["slowbreath", "slb"],
            save_ability: AbilityScoreType::Constitution,
            dc,
            cone,
            effect: MetallicBreathEffect::Slow,
        }
    }

    /// Gold — Weakening Breath.
    pub const fn weakening(dc: i32, cone: isize) -> Self {
        Self {
            display_name: "weakening breath",
            aliases: &["weaken", "wkb"],
            save_ability: AbilityScoreType::Strength,
            dc,
            cone,
            effect: MetallicBreathEffect::Weaken,
        }
    }

    fn shape(&self) -> AreaShape {
        AreaShape::Cone { length: self.cone }
    }

    /// Everyone in the cone this breath could still do something to.
    /// Shared by the validator and the resolver so the two cannot
    /// disagree about who is worth breathing on.
    fn live_targets(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        aim: Coordinate,
    ) -> Vec<usize> {
        let redundant = self.effect.redundant_when();
        encounter
            .enemy_area_targets(caster_id, self.shape(), aim)
            .into_iter()
            .filter(|id| {
                let Some(a) = encounter.actors.get(id) else {
                    return false;
                };
                let Some(c) = redundant else {
                    return true;
                };
                if a.has_condition(c) {
                    return false;
                }
                // A ladder's opener also has nothing to offer somebody
                // already climbing one, because `begin_staged_save`
                // declines a second entry. Asked only of the ladder
                // breaths: a creature stiffening under a gorgon's gaze
                // is still perfectly slowable by a copper dragon.
                match self.effect {
                    MetallicBreathEffect::Ladder(_) => {
                        !encounter.staged_save_pending(*id)
                    }
                    _ => true,
                }
            })
            .collect()
    }
}

impl Action for MetallicBreath {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Cone { length: self.cone }
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
    fn spares_allies(&self) -> bool {
        // Resolved entirely through `enemy_area_targets`, so an ally
        // standing in the cone is not in the breath. Declared, because
        // the AI's placement search reads the flag rather than the
        // resolver: without it a copper dragon would refuse to breathe
        // anywhere a kobold happened to be standing, on a cone that
        // could never have touched the kobold.
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(aim) = first_target_location(target_locations) else {
            return false;
        };
        !self.live_targets(encounter, caster_id, aim).is_empty()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;

        let Some(aim) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let origin = encounter.area_origin(caster_id, self.shape(), aim);
        encounter.log(format!(
            "  {}: a {} ft cone (DC {} {})",
            self.display_name,
            crate::engine::util::feet_from_tiles(self.cone.max(0) as u32),
            self.dc,
            self.save_ability,
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in self.live_targets(encounter, caster_id, aim) {
            let against = match self.effect {
                MetallicBreathEffect::Ladder(l) => l.first,
                MetallicBreathEffect::Repulse { .. } => Condition::Prone,
                MetallicBreathEffect::Slow => Condition::Slowed,
                MetallicBreathEffect::Weaken => Condition::Enfeebled,
            };
            let save = encounter.roll_save_against_caster_vs_condition(
                tid,
                self.save_ability,
                self.dc,
                caster_id,
                against,
            );
            if save.passed() {
                continue;
            }
            match self.effect {
                MetallicBreathEffect::Ladder(ladder) => {
                    // Opened on the spot rather than queued: the ladder
                    // writes an encounter ledger as well as installing
                    // a condition, and only the encounter can do that.
                    encounter.begin_staged_save(tid, caster_id, self.dc, ladder);
                }
                MetallicBreathEffect::Repulse { push_feet } => {
                    // Pushed away from the cone's apex, which is where
                    // the wind is coming from, and then knocked down.
                    effects.push(Box::new(PushActor {
                        actor_id: tid,
                        from: origin,
                        max_tiles: crate::engine::util::tiles_from_feet(push_feet),
                    }));
                    effects.push(Box::new(
                        crate::engine::side_effects::ApplyCondition {
                            actor_id: tid,
                            condition: Condition::Prone,
                            timer: ConditionTimer::Permanent,
                        },
                    ));
                }
                MetallicBreathEffect::Slow => {
                    effects.push(Box::new(
                        crate::engine::side_effects::ApplyCondition {
                            actor_id: tid,
                            condition: Condition::Slowed,
                            // RAW's "until the end of its next turn".
                            timer: ConditionTimer::Rounds(1),
                        },
                    ));
                }
                MetallicBreathEffect::Weaken => {
                    // RAW: "It repeats the save at the end of each of
                    // its turns, ending the effect on itself on a
                    // success. After 1 minute, it succeeds
                    // automatically."
                    //
                    // Both halves ship. The minute is the condition's
                    // own ten-round timer and the repeat is a row on
                    // `engine::repeat_saves` — the lane that exists
                    // precisely because a dragon is not concentrating
                    // on its own breath. This used to be a flat three
                    // rounds with a comment saying three rounds "is
                    // about what a middling save buys", which is true
                    // of the median and false of everything either side
                    // of it: a Strength-save fighter should shrug this
                    // off on the first try and a wizard should
                    // sometimes still be weakened a minute later.
                    //
                    // Opened on the spot rather than queued, for the
                    // same reason the ladder arm above is: the escape
                    // writes an encounter ledger alongside the
                    // condition, and only the encounter can do that.
                    encounter.begin_repeat_save(
                        tid,
                        caster_id,
                        self.dc,
                        &WEAKENING_BREATH,
                        ConditionTimer::Rounds(10),
                    );
                }
            }
        }
        effects
    }
}

/// Gorgon **Petrifying Breath** — RAW's 30-foot Cone at DC 15, on the
/// two-stage ladder every petrification in SRD 5.2 shares: a first
/// failure stiffens, and a second, at the end of the victim's next
/// turn, turns it to stone. See `staged_saves::PETRIFICATION`.
///
/// The cone is what makes the gorgon's version the dangerous one. The
/// cockatrice bites one creature and the medusa gazes at one; a gorgon
/// puts four people on the ladder at once, and each of them rolls their
/// own second die a turn later.
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
        TargetingSchema::Cone { length: CONE_30_FT }
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
    fn spares_allies(&self) -> bool {
        // Resolved through `enemy_area_targets`; declared so the AI's
        // placement search does not veto a cone an ally is standing in
        // but could never be caught by.
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !actor_has_recharge(encounter, caster_id, "breath_weapon") {
            return false;
        }
        // And there has to be somebody in the cone the ladder could
        // still catch — see the medusa's gaze for why a creature
        // already climbing one is not that somebody. Worth more here
        // than there: this spends a recharge as well as an Action.
        let Some(aim) = first_target_location(target_locations) else {
            return false;
        };
        encounter
            .enemy_area_targets(
                caster_id,
                AreaShape::Cone { length: CONE_30_FT },
                aim,
            )
            .into_iter()
            .any(|id| {
                !encounter.staged_save_pending(id)
                    && encounter
                        .actors
                        .get(&id)
                        .is_some_and(|a| !a.has_condition(Condition::Petrified))
            })
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
        const DC: i32 = 15;
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("breath_weapon");
        }
        encounter.log("  petrifying breath: gorgon exhales a cone of stoning vapour");
        // Resolved inline rather than through
        // `resolve_area_save_condition`, which installs one condition on
        // a failure and has nowhere to put the escalation this needs.
        // Same target list, same save; a different failure branch.
        let shape = AreaShape::Cone { length: CONE_30_FT };
        for tid in encounter.enemy_area_targets(caster_id, shape, origin) {
            let save = encounter.roll_save_against_caster_vs_condition(
                tid,
                AbilityScoreType::Constitution,
                DC,
                caster_id,
                Condition::Restrained,
            );
            if !save.passed() {
                encounter.begin_staged_save(
                    tid,
                    caster_id,
                    DC,
                    &crate::engine::staged_saves::PETRIFICATION,
                );
            }
        }
        Vec::new()
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
pub static DRYAD_FEY_CHARM: SaveOrCharm = SaveOrCharm::action(
    "fey charm",
    &["fc", "charm"],
    // 30 ft RAW = 12 tiles.
    12,
    14,
    ConditionTimer::Rounds(10),
    "fey charm",
);

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

/// Bullywug Multiattack — 1 spear + 1 bite per Action. RAW: the
/// bullywug makes two attacks (one with its bite, one with its spear).
/// We model the heterogeneous pair via `CompoundAttack` so each limb
/// uses its own dice tier.
pub static BULLYWUG_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "spear + bite",
    parts: vec![(&SPEAR, 1), (&BULLYWUG_BITE, 1)],
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
        caster_id: usize,
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
        crate::engine::side_effects::install_condition_with_link(
            Condition::Frightened,
            target_id,
            caster_id,
            ConditionTimer::UntilStartOfNextTurn,
        )
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

/// Merrow **Harpoon** — 2d6+STR piercing at reach 2 (10 ft), and the
/// clause the weapon is named for: *"If the target is a Large or
/// smaller creature, the merrow pulls the target up to 15 feet straight
/// toward itself."*
///
/// The drag is the whole point of a harpoon, and it is what makes the
/// merrow a merrow rather than an ogre that swims: it fights at the
/// edge of the water and hauls people into it. The clause used to be a
/// sentence in this docstring saying the chassis had no displacement
/// lane — see `WeaponWithCondition::displacement`, which is now that
/// lane.
///
/// RAW's thrown form (range 20/60) is still not surfaced; the reach-2
/// swing carries the envelope.
pub static MERROW_HARPOON: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "harpoon",
    &["hrp", "harpoon"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
    &[],
    ConditionTimer::Permanent,
    "harpoon line",
    2,
)
.against_at_most(Size::Large)
.pulls(tiles_from_feet(15));

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

/// Gibbering Mouther **Bites** — 5d6+STR piercing, and RAW's knockdown:
/// *"If the target is a Medium or smaller creature, it has the Prone
/// condition."*
///
/// Dozens of constantly-shifting mouths gnashing at anything in reach,
/// collapsed by RAW into a single attack roll — and the thing they do
/// besides bite is drag you down among them, which is what the mouther
/// is *for*. A creature on the floor in a mouther's space is one that
/// spends its next turn standing up inside the swing.
pub static GIBBERING_MOUTHER_BITES: WeaponWithCondition = WeaponWithCondition::melee(
    "gibbering bites",
    &["gmb", "mouther-bites"],
    AbilityScoreType::Strength,
    Dice::new(5, 6),
    DamageType::Piercing,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "gnashing mouths",
)
.against_at_most(Size::Medium);

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

impl MummyLordDreadfulGlare {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 12;
}

impl Action for MummyLordDreadfulGlare {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
        const DC: i32 = 17;
        encounter.log(
            "  lord dreadful glare: the mummy lord's hollow gaze freezes the living",
        );
        resolve_los_glare_condition(
            encounter,
            caster_id,
            Self::RADIUS,
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

// ─── Clay Golem ──────────────────────────────────────────────────────

/// Once-per-turn ledger key for the Clay Golem's **Hasten**, read by
/// `CLAY_GOLEM_MULTI` to decide whether this turn's multiattack is two
/// slams or three. Shares the actor's `once_per_turn_marks` ledger with
/// the charge and weapon-mastery clauses, so `reset_for_new_round`
/// clears it and a golem that hastened last turn does not collect the
/// third swing this one.
pub const CLAY_GOLEM_HASTEN_TAG: &str = "clay golem: hasten";

/// Clay Golem **Slam** (RAW): "Melee Attack Roll: +9, reach 5 ft. Hit:
/// 10 (1d10 + 5) Bludgeoning damage plus 6 (1d12) Acid damage, and the
/// target's Hit Point maximum decreases by an amount equal to the Acid
/// damage taken."
///
/// Two damage instances and a drain sized off the second of them, which
/// is why this is a bespoke `Action` rather than a `WeaponWithRider`
/// row: the chassis can lay a second typed die on a hit, but nothing on
/// it can then measure a third effect against that die's result.
///
/// The drain reads *taken*, not *dealt* — so it is sized off what the
/// acid actually does to this target, after their resistances. RAW's
/// wording differs from the Wraith's Life Drain a few hundred lines up
/// ("equal to the necrotic damage dealt"), and the two implementations
/// differ with it: an acid-resistant target loses half as much of its
/// ceiling here, and a target immune to acid loses none. That is a
/// meaningfully different rule from the one the wraith applies, and the
/// stat blocks are the reason to keep both.
///
/// No save. The wraith's drain hangs off a CON save because RAW gives it
/// one; the golem's does not, which is a large part of why a clay golem
/// is a CR-9 problem that a party cannot simply out-heal — every slam
/// permanently lowers the ceiling the healing is aiming at.
pub struct ClayGolemSlam {}

impl Action for ClayGolemSlam {
    fn name(&self) -> &str {
        "clay slam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cslam", "clay-slam"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Acid]
    }
    /// The bludgeoning half through the shared weapon estimator, plus
    /// the acid die on top. The acid is a flat addition rather than a
    /// second estimator call because it is one die with no ability
    /// modifier and no Extra Attack multiplier of its own — it rides the
    /// swing the estimator already counted.
    ///
    /// The ceiling drain is deliberately left out. It is not damage: it
    /// takes nothing off the target's current hit points, and folding it
    /// in here would have the picker rate the slam as roughly half again
    /// as lethal as it is, against a `Multiattack` estimate that is
    /// measuring real hit points.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let bludgeon = crate::actions::action_template::weapon_expected_damage_named(
            encounter,
            caster_id,
            "clay slam",
            Dice::new(1, 10),
            Some(AbilityScoreType::Strength),
            Resource::Action,
            0,
        )?;
        Some(bludgeon + Dice::new(1, 12).average_roll())
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
        let str_mod = caster.ability_modifier(AbilityScoreType::Strength);
        let attack_mod = str_mod + caster.proficiency_bonus();
        let (mut effects, damage) = crate::engine::attack::resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "clay slam",
                attack_bonus: attack_mod,
                damage_dice: Dice::new(1, 10),
                damage_bonus: str_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
                long_range: None,
                min_range: None,
                is_spell: false,
            },
        );
        // A miss deals no acid and drains nothing — both clauses of the
        // Hit line hang off the same hit.
        if damage == 0 {
            return effects;
        }
        let acid = encounter.roll(&Dice::new(1, 12));
        // What the target *takes*, which is the number the drain is
        // measured in. Asked of the target rather than assumed, so an
        // acid-resistant creature loses half the ceiling and an
        // acid-immune one loses none — and so a clay golem swinging at
        // its own kind achieves nothing at all.
        let taken = encounter
            .actors
            .get(&target_id)
            .map_or(0, |t| t.effective_damage(acid, DamageType::Acid));
        effects.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: acid,
            damage_type: DamageType::Acid,
        }));
        if taken > 0 {
            encounter.log(format!(
                "  clay slam: {} acid sears the target's ceiling by {}",
                acid, taken
            ));
            effects.push(Box::new(AdjustMaxHp {
                actor_id: target_id,
                delta: -(taken as i32),
            }));
        }
        effects
    }
}

pub static CLAY_GOLEM_SLAM: LazyLock<ClayGolemSlam> = LazyLock::new(|| ClayGolemSlam {});

/// Clay Golem **Multiattack** (RAW): "The golem makes two Slam attacks,
/// or it makes three Slam attacks if it used Hasten this turn."
///
/// A count that depends on what the creature already did this turn,
/// which the shared `Multiattack` chassis cannot express — its `count`
/// is a constant, and rightly so for the forty-odd rows that use it.
/// The dependency is real and worth honouring: Hasten costs a bonus
/// action and a recharge, and the third slam is most of what it buys.
pub struct ClayGolemMulti {}

impl Action for ClayGolemMulti {
    fn name(&self) -> &str {
        "clay golem multiattack"
    }
    fn chains_multiple_attacks(&self) -> bool {
        true
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["multi", "ma"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        CLAY_GOLEM_SLAM.damage_types()
    }
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let per_swing = CLAY_GOLEM_SLAM.expected_damage(encounter, caster_id)?;
        Some(per_swing * clay_golem_slam_count(encounter, caster_id) as f32)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let swings = clay_golem_slam_count(encounter, caster_id);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // The depth counter is what stops the sub-attack from collecting
        // an Extra Attack chain of its own — same guard every other
        // multiattack wrapper in this file uses.
        encounter.enter_multiattack();
        for _ in 0..swings {
            effects.extend(CLAY_GOLEM_SLAM.side_effects(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            ));
        }
        encounter.exit_multiattack();
        effects
    }
}

/// Two slams, or three off a Hasten already spent this turn. The one
/// place the ledger key is read, so the resolver and the AI's damage
/// estimate cannot disagree about how big this action is.
fn clay_golem_slam_count(encounter: &EncounterInstance, caster_id: usize) -> u32 {
    let hastened = encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.once_per_turn_used(CLAY_GOLEM_HASTEN_TAG));
    // 5e Slow cuts the routine to one swing, whichever end of the
    // ledger it started at. Folded in here rather than at the caller so
    // the AI's damage estimate — which reads this same function — prices
    // a slowed golem's Action honestly.
    encounter.attack_routine_swings(caster_id, if hastened { 3 } else { 2 })
}

pub static CLAY_GOLEM_MULTI: LazyLock<ClayGolemMulti> = LazyLock::new(|| ClayGolemMulti {});

/// Clay Golem **Hasten** (RAW, Recharge 5–6, bonus action): "The golem
/// takes the Dash and Disengage actions."
///
/// Both halves reuse the default actions' own side effects rather than
/// restating them — a Dash is `GiveResource(Movement(travel_speed))` and
/// a Disengage is `SetDisengaging`, and a copy of either here would be
/// a copy that stops tracking the original. (The Dash half in particular:
/// `travel_speed` is the mount's speed for a mounted creature, and a
/// clay golem is a plausible thing to be riding.)
///
/// It also marks the once-per-turn ledger key the multiattack reads, so
/// the third slam is a consequence of having hastened rather than a
/// separate roll of the same dice.
pub struct ClayGolemHasten {}

impl Action for ClayGolemHasten {
    fn name(&self) -> &str {
        "hasten"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hst", "clay-hasten"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        actor_has_recharge(encounter, caster_id, "clay_golem_hasten")
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(golem) = encounter.actors.get_mut(&caster_id) else {
            return Vec::new();
        };
        golem.spend_recharge("clay_golem_hasten");
        golem.mark_once_per_turn_used(CLAY_GOLEM_HASTEN_TAG);
        encounter.log("  hasten: the clay quickens — dash and disengage");
        let mut effects = crate::actions::default_actions::DASH.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        effects.extend(crate::actions::default_actions::DISENGAGE.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ));
        effects
    }
}

pub static CLAY_GOLEM_HASTEN: LazyLock<ClayGolemHasten> = LazyLock::new(|| ClayGolemHasten {});

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

/// Iron Golem Poison Breath — SRD 5.2's 60-foot Cone of noxious green
/// vapour. 10d8 poison, DC 19 CON, half on save. Recharge 6 (RAW:
/// "Recharge 6" means the breath only refreshes on a d6 of exactly 6 at
/// the start of its turn). Shares the standard `"breath_weapon"`
/// recharge pool with the dragons, so a multi-monster ambush can't
/// double-tap two breath weapons in the same round.
///
/// The same sixty feet an adult dragon breathes, at a heavier
/// per-die count and a stingier recharge — which is the trade the stat
/// block is making, and which the old docstring had backwards: it
/// described the golem's cone as "15 ft" against the dragon's 60 and
/// gave it the smaller burst to match.
pub static IRON_GOLEM_BREATH: BreathWeapon = BreathWeapon {
    display_name: "iron poison breath",
    aliases: &["ipb", "iron-breath"],
    damage: Some((Dice::new(10, 8), DamageType::Poison)),
    save_ability: AbilityScoreType::Constitution,
    dc: 19,
    shape: AreaShape::Cone { length: 24 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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

/// Dragon Turtle Steam Breath — RAW's 60-foot Cone of scalding vapour.
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
    damage: Some((Dice::new(12, 6), DamageType::Fire)),
    save_ability: AbilityScoreType::Constitution,
    dc: 18,
    shape: AreaShape::Cone { length: 24 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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
pub static KRAKEN_TENTACLE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "kraken tentacle",
    &["kt", "tentacle-k"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "one of ten tentacles",
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

impl KrakenLightningStorm {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 12;
}

impl Action for KrakenLightningStorm {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

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
            Self::RADIUS,
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
/// the chosen-tile + small radius shape of the bulette's earth tremor
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

/// The androsphinx's **Roar** — SRD 5.2's three-in-one, in full.
///
/// > *Roar (3/Day). The sphinx emits a magical roar. Whenever it roars,
/// > the roar has a different effect, as detailed below (the sequence
/// > resets when it takes a Long Rest).*
///
/// | roar | save | on a failure |
/// |------|------|--------------|
/// | first | WIS DC 18 | Frightened for a minute |
/// | second | WIS DC 18 | Paralyzed, repeating the save every turn |
/// | third | CON DC 18 | 8d10 Thunder and Prone; half damage on a success |
///
/// The escalation is the stat block. A sphinx that roars three times in
/// a fight opens with fear, follows with a paralysis half the party
/// will not shake off for two rounds, and finishes by putting whoever
/// is left on the floor — and it is the only creature on this roster
/// whose signature ability is *different each time it is used*.
///
/// It had been collapsed to the first roar on a recharge, with the
/// collapse written down in its own docstring: *"we collapse the
/// three-tier ladder to the Frightened-on-fail base effect since the
/// engine's recharge chassis fits cleanly into the start-of-turn
/// refresher without per-day bookkeeping."* Two things had changed
/// since. The per-day bookkeeping exists — `FEATURE_CHARGES` rations
/// half the roster — and so does the paralysis clause's other half:
/// `engine::repeat_saves` is what lets a creature nobody is
/// concentrating on grant its victims a way out.
///
/// The collapse was also load-bearing in a way nobody intended. The AI
/// finds area actions by asking for their `targeting_schema`'s shape,
/// and the old Roar declared `NoArgs` — so a CR-17 boss's signature
/// ability was unreachable by every AI rung in the engine, and had
/// been since it was written. It is a `Burst` now, which is what RAW's
/// Emanation has always been, and the sphinx roars unprompted.
///
/// ## Where it diverges
///
/// **The radius.** RAW's is a 500-foot Emanation — the whole dungeon.
/// `ROAR_RADIUS` is 24 tiles, which covers any board the engine
/// generates, and is the same compression the Sphinx of Lore's
/// Mind-Rending Roar already makes for the same reason.
///
/// **The DC.** RAW's Sphinx of Valor roars at DC 20 off a +6
/// proficiency bonus. This chassis keeps the DC 18 the ability shipped
/// with, which is what the rest of its stat line is priced against.
///
/// **The audience.** RAW scopes every roar to *enemies*, and so does
/// this: `spares_allies` is true and the target list comes from
/// `enemy_burst_targets`. A sphinx that paralysed its own guardians
/// would be reading the wrong word in its own stat block.
///
/// Hearing, not sight. RAW's clause is an Emanation with no line-of-
/// sight requirement at all, and the engine's nearest reading is the
/// audible-burst lane — a pillar does not stop a roar.
pub struct AndrosphinxRoar {}

/// How far the roar carries, in tiles.
///
/// RAW's 500-foot Emanation is 200 tiles on the 2.5 ft grid, which is
/// larger than any board the terrain generator makes. 24 is the same
/// number the Sphinx of Lore's roar uses and means the same thing:
/// everything hostile hears it.
const ROAR_RADIUS: isize = 24;

/// The roar's save DC, shared by all three stages.
///
/// One constant because RAW gives the three roars one DC, and the two
/// abilities they roll against — Wisdom for the first two, Constitution
/// for the third — are the only thing that changes.
const ROAR_DC: i32 = 18;

/// SRD 5.2's metallic **Weakening Breath**: *"It repeats the save at
/// the end of each of its turns, ending the effect on itself on a
/// success. After 1 minute, it succeeds automatically."*
///
/// Strength, because that is the save the breath opened with — RAW says
/// *repeats* the save, and the gold and brass dragons' breath is a
/// Strength save.
pub static WEAKENING_BREATH: crate::engine::repeat_saves::RepeatSave =
    crate::engine::repeat_saves::RepeatSave {
        name: "weakening breath",
        condition: Condition::Enfeebled,
        ability: AbilityScoreType::Strength,
        escaped_flavor: "finds their strength again",
    };

/// SRD 5.2's *"the target has the Paralyzed condition, and it repeats
/// the save at the end of each of its turns, ending the effect on
/// itself on a success. After 1 minute, it succeeds automatically."*
///
/// The whole of the Second Roar's failure branch, and the first
/// occupant of the `repeat_saves` lane. The minute is the condition's
/// own ten-round timer; this is what happens before it runs out.
pub static PARALYSING_ROAR: crate::engine::repeat_saves::RepeatSave =
    crate::engine::repeat_saves::RepeatSave {
        name: "paralysing roar",
        condition: Condition::Paralyzed,
        ability: AbilityScoreType::Wisdom,
        escaped_flavor: "shakes the ringing out of their head and moves again",
    };

/// Which roar this is. The sphinx's charge pool counts down, so the
/// stage counts up from what is left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RoarStage {
    /// *"Wisdom Saving Throw … Failure: The target has the Frightened
    /// condition for 1 minute."*
    Fear,
    /// *"Failure: The target has the Paralyzed condition, and it
    /// repeats the save at the end of each of its turns."*
    Paralysis,
    /// *"Constitution Saving Throw … Failure: 44 (8d10) Thunder damage,
    /// and the target has the Prone condition. Success: Half damage
    /// only."*
    Thunder,
}

impl RoarStage {
    /// Read the stage off what is left of the pool.
    ///
    /// Derived rather than stored, which is what keeps the sequence and
    /// the resource from ever disagreeing: there is one number, the
    /// charge count, and both "may I roar" and "which roar is this"
    /// read it. A separate stage counter would need its own reset on a
    /// long rest, and RAW resets both in the same sentence.
    /// Called with what is left *before* the roar is spent, so the
    /// difference from the full pool is the sequence position: nothing
    /// spent is the first roar, one spent is the second, and everything
    /// after is the third.
    ///
    /// Subtracting rather than matching the count directly is what
    /// keeps this honest if the pool ever changes size. RAW's three
    /// uses and three roars line up exactly; a fourth use would be a
    /// fourth *roar*, and RAW does not print one, so the last one
    /// repeats rather than the sequence wrapping back to fear.
    fn from_remaining(remaining: u32) -> RoarStage {
        match crate::actions::class_features::ROAR_USES.saturating_sub(remaining) {
            0 => RoarStage::Fear,
            1 => RoarStage::Paralysis,
            _ => RoarStage::Thunder,
        }
    }
}

impl Action for AndrosphinxRoar {
    fn name(&self) -> &str {
        "roar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["roar-s", "sphinx-roar"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst {
            radius: ROAR_RADIUS,
        }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // An Emanation is not thrown: the aim point names the sphinx's
        // own body. Same leash the Mind-Rending Roar uses, and for the
        // same reason.
        Some(EMANATION_AIM_LEASH)
    }
    fn requires_los(&self) -> bool {
        // Of the aim point, which is the sphinx's own tile. The roar's
        // *targets* are not sight-gated — see the docstring.
        true
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn spares_allies(&self) -> bool {
        // RAW: "each enemy in a 500-foot Emanation".
        true
    }
    fn deals_damage(&self) -> bool {
        // The third roar does. Declared unconditionally because the
        // trait answers per action rather than per use, and the
        // conservative direction here is the honest one: an AI rung
        // that prices this as a damage option is right one time in
        // three and wrong in the direction of using a boss's best
        // ability.
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.feature_available(crate::actions::class_features::ROAR_TAG)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::class_features::ROAR_TAG;
        let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        let stage = RoarStage::from_remaining(
            encounter
                .actors
                .get(&caster_id)
                .map(|a| a.feature_charges_remaining(ROAR_TAG))
                .unwrap_or(0),
        );
        // Spent before anything resolves, so a mid-resolution bail
        // cannot leave the roar both used and unspent — and so the next
        // roar is the next stage even if this one caught nobody.
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_feature(ROAR_TAG);
        }
        let targets = encounter.enemy_burst_targets(caster_id, caster_loc, ROAR_RADIUS);
        match stage {
            RoarStage::Fear => {
                encounter.log(format!(
                    "  roar: the first roar rolls out (DC {} WIS, frightened on a failure)",
                    ROAR_DC
                ));
                install_condition_on_failed_saves(
                    encounter,
                    caster_id,
                    &targets,
                    AbilityScoreType::Wisdom,
                    ROAR_DC,
                    Condition::Frightened,
                    // RAW's minute.
                    ConditionTimer::Rounds(10),
                )
            }
            RoarStage::Paralysis => {
                encounter.log(format!(
                    "  roar: the second roar rolls out (DC {} WIS, paralyzed on a failure)",
                    ROAR_DC
                ));
                // Applied here rather than queued, because the escape
                // ledger and the condition are installed together by
                // `begin_repeat_save` and an entry queued behind a
                // condition that has not landed yet would be an escape
                // from nothing. Nothing else in the roar is ordered
                // against it.
                for tid in targets {
                    if encounter
                        .roll_save_vs_condition(
                            tid,
                            AbilityScoreType::Wisdom,
                            ROAR_DC,
                            Condition::Paralyzed,
                        )
                        .passed()
                    {
                        continue;
                    }
                    encounter.begin_repeat_save(
                        tid,
                        caster_id,
                        ROAR_DC,
                        &PARALYSING_ROAR,
                        // RAW's "after 1 minute, it succeeds
                        // automatically" — the cap the repeats race.
                        ConditionTimer::Rounds(10),
                    );
                }
                Vec::new()
            }
            RoarStage::Thunder => {
                let damage = encounter.roll(&Dice::new(8, 10));
                encounter.log(format!(
                    "  roar: the third roar rolls out (DC {} CON, {} thunder and prone on a failure)",
                    ROAR_DC, damage
                ));
                let origin = encounter.area_origin(
                    caster_id,
                    AreaShape::Burst {
                        radius: ROAR_RADIUS,
                    },
                    caster_loc,
                );
                let (mut effects, saves) = resolve_burst_targets(
                    encounter,
                    caster_id,
                    origin,
                    &targets,
                    AbilityScoreType::Constitution,
                    ROAR_DC,
                    damage,
                    DamageType::Thunder,
                    SaveDamagePolicy::HalfOnSave,
                    &HashSet::new(),
                );
                // RAW's Prone rides the *same* save the damage did —
                // "Failure: … damage, and the target has the Prone
                // condition" — so it reads the outcomes the burst
                // already rolled rather than asking for a second one.
                for (tid, passed) in saves {
                    if passed {
                        continue;
                    }
                    effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                        actor_id: tid,
                        condition: Condition::Prone,
                        timer: ConditionTimer::Permanent,
                    }));
                }
                effects
            }
        }
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
pub static LAMIA_INTOXICATING_TOUCH: SaveOrCharm = SaveOrCharm::action(
    "intoxicating touch",
    &["it", "touch", "lamia-touch"],
    MELEE_REACH,
    13,
    ConditionTimer::Rounds(10),
    "lamia curse",
);

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
        (&LAMIA_INTOXICATING_TOUCH, 1),
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
)
// SRD 5.2: "the target has the Poisoned condition until the start of
// the devil's next turn. **Until this poison ends, the target can't
// regain Hit Points.**"
//
// The second sentence is the devil, and it was dropped for years under
// a comment saying healing was not a tactically load-bearing axis in
// this simulator. That was never quite true and is plainly untrue now —
// the engine has clerics with Healing Word, potions on the loot table,
// and an AI rung that drinks them — so a devil that stops the healing is
// a different fight from a devil that does not.
//
// One condition on the same timer, which is exactly what "until this
// poison ends" asks for. See `Condition::Wounded`.
.and_also(Condition::Wounded);

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

/// Ice Mephit Frost Breath — a 15-ft cone (six tiles on this
/// 2.5 ft grid) of biting cold. 1d8 cold, DC 10 DEX, half on save.
/// Recharge 6 per RAW; we route through the shared `"breath_weapon"`
/// pool so a mephit ambush can't double-tap with two breaths.
pub static ICE_MEPHIT_FROST_BREATH: BreathWeapon = BreathWeapon {
    display_name: "frost breath",
    aliases: &["frost", "ice-breath"],
    damage: Some((Dice::new(1, 8), DamageType::Cold)),
    save_ability: AbilityScoreType::Dexterity,
    dc: 10,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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
    damage: Some((Dice::new(1, 6), DamageType::Fire)),
    save_ability: AbilityScoreType::Dexterity,
    dc: 10,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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
    damage: Some((Dice::new(1, 8), DamageType::Fire)),
    save_ability: AbilityScoreType::Dexterity,
    dc: 11,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "breath_weapon",
    condition: None,
    enemies_only: false,
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
    fn self_burst_radius(&self) -> Option<isize> {
        // RAW's 15-foot Emanation — the 3 the resolver passes.
        Some(3)
    }
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
)
.against_at_most(Size::Medium);

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
    normal_range: None,
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
        // The RAW marid's "pushed up to 20 ft away" rider.
        const PUSH_TILES: u32 = tiles_from_feet(20);
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
/// The swing itself, without the gate. Declared separately from
/// `MAMMOTH_STOMP` below because the gate is the wrapper's job now: it
/// used to be a bespoke `impl Action` whose only difference from a
/// `SimpleWeapon` was six lines of `custom_validate_input`, and the
/// elephant wanting the same clause is what turned that into a chassis.
/// Never put on a template directly — `MAMMOTH_STOMP` is what the
/// mammoth carries, and an ungated 4d10 stomp is not a thing RAW has.
static MAMMOTH_STOMP_SWING: SimpleWeapon = SimpleWeapon::melee(
    "mammoth stomp",
    &["mstomp", "stomp-m"],
    AbilityScoreType::Strength,
    Dice::new(4, 10),
    DamageType::Bludgeoning,
);

/// The mammoth's stomp as the mammoth actually has it: the swing above,
/// admitted only against a target already on the floor.
///
/// Out of the multiattack, the stomp is a single-target standalone the
/// AI can fall back to if the target is already prone from a previous
/// round (Trampling Charge rider install, Booming Blade follow-up,
/// etc.) — and inside it, `MAMMOTH_CHARGE::prone_follow_up` names this
/// action so the charge that flattens a target immediately earns the
/// swing that only a flattened target admits.
pub static MAMMOTH_STOMP: LazyLock<ProneOnlyAttack> = LazyLock::new(|| ProneOnlyAttack {
    display_name: "mammoth stomp",
    sub_attack: &MAMMOTH_STOMP_SWING,
});

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

/// Dust Mephit Blinding Breath — a 15-ft cone of
/// fine choking grit. Save-or-Blinded for 1 round on a failed DC 10
/// CON save; no damage. Recharge 6.
///
/// The pure-control end of the breath chassis: `damage: None`, which
/// is the whole stat block. RAW's "until the end of the mephit's next
/// turn" timer collapses to `Rounds(1)` at the engine's per-round
/// granularity. The cone is the dust mephit's only ranged threat; the
/// claws are a fallback for adjacent targets after the breath spends.
pub static DUST_MEPHIT_BLINDING_BREATH: BreathWeapon = BreathWeapon {
    display_name: "blinding breath",
    aliases: &["blinding", "dust-breath", "grit-cone"],
    damage: None,
    save_ability: AbilityScoreType::Constitution,
    dc: 10,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "breath_weapon",
    condition: Some((Condition::Blinded, ConditionTimer::Rounds(1))),
    enemies_only: false,
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

/// Purple Worm Bite — STR-based 3d8+STR piercing at reach 2 (10 ft),
/// and RAW's hold: *"If the target is a Large or smaller creature, it
/// has the Grappled condition (escape DC 19), and it has the Restrained
/// condition until the grapple ends."*
///
/// The hold is the whole point of the limb. It is what the worm's
/// Bonus Action Swallow is waiting on — RAW eats "one Large or smaller
/// creature Grappled by the worm" and nothing else — so a bite that
/// only dealt damage left the creature's signature clause with nothing
/// to fire at. The Tunneler trait stays flavour on the template; the
/// engine isn't 3D.
pub static PURPLE_WORM_BITE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "purple worm bite",
    &["pw-bite", "worm-bite"],
    AbilityScoreType::Strength,
    Dice::new(3, 8),
    DamageType::Piercing,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "worm jaws",
    2,
)
.against_at_most(Size::Large);

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
    )
    .against_at_most(Size::Medium);

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
)
.against_at_most(Size::Medium);

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
    &[Condition::Grappled],
    ConditionTimer::Permanent,
    "tongue grab",
)
.against_at_most(Size::Medium);

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
/// `WeaponWithSaveDamage` chassis alongside Giant Spider Bite / Ettercap
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
    normal_range: None,
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
)
.against_at_most(Size::Large);

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
/// DC-13 STR save vs Prone after a 20 ft straight-line dash) is *not*
/// omitted and has not been for some time: it ships as
/// `GIANT_GOAT_CHARGE` on the stat block, priced off
/// `ActorInstance::straight_run_tiles`, which is the "this turn's move
/// was straight" the scope cut said the engine lacked. Only the
/// Sure-Footed trait (advantage on STR/DEX saves vs prone) is still
/// dropped. The plain ram swing keeps the goat anchored at the
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
/// enemy's reach) rides `FLYBY_TAG` on the stat block rather than
/// anything here — it is a property of the owl's movement, not of its
/// swing. Keen Hearing and Sight (advantage on hearing / sight
/// Perception) stays flavor-only: skill checks don't route through
/// combat. The plain talons swing pinned to the
/// fly-60 speed keeps the giant owl at the "fast aerial harasser"
/// silhouette.
pub static GIANT_OWL_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "giant owl talons",
    &["got", "owl-talons"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

// ─── Giant Venomous Snake ─────────────────────────────────────────

/// Giant Venomous Snake Bite — DEX-based 1d4+DEX piercing melee with
/// a DC 11 CON save-or-3d6-poison rider via `WeaponWithSaveDamage`.
/// RAW: "+6 to hit, reach 10 ft, one target. Hit: 6 (1d4+4) piercing
/// damage, and the target must make a DC 11 Constitution saving throw,
/// taking 10 (3d6) poison damage on a failed save, or half as much
/// damage on a successful one." We collapse the half-on-pass to the
/// engine's standard SaveDamagePolicy::HalfOnPass shape implicit in
/// the chassis. Reach 10 ft = 2 tile-gap units — the snake strikes
/// from a coil one tile away (RAW: medium serpent on a 10-ft reach).
pub static GIANT_VENOMOUS_SNAKE_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::reach_melee(
    "giant venomous snake bite",
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

/// Swarm of Piranhas Bites — DEX-based 4d6 piercing melee. RAW: "+5
/// to hit, reach 0 ft., one creature in the swarm's space. Hit: 14
/// (4d6) piercing damage, or 7 (2d6) piercing damage if the swarm has
/// half of its hit points or fewer."
///
/// The heaviest swarm bite, and the one that compounds: the swarm's
/// Blood Frenzy hands it advantage against anything already wounded,
/// so the first bite that lands makes the second likelier.
pub static SWARM_OF_PIRANHAS_BITES: SimpleWeapon = SimpleWeapon::melee(
    "swarm of piranhas bites",
    &["sqb", "quipper-swarm", "quippers"],
    AbilityScoreType::Dexterity,
    Dice::new(4, 6),
    DamageType::Piercing,
);

/// Swarm of Venomous Snakes Bites — DEX-based 2d6 piercing melee
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
pub static SWARM_OF_VENOMOUS_SNAKES_BITES: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "swarm of venomous snakes bites",
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

// ─── Acolyte ────────────────────────────────────────────────────────

/// Acolyte Club — STR-based 1d4+STR bludgeoning melee. RAW: "Club.
/// Melee Weapon Attack: +2 to hit, reach 5 ft., one target. Hit: 2
/// (1d4) bludgeoning damage."
///
/// A creature-prefixed club rather than the armoury's shared `CLUB`
/// because the acolyte is the first template in the bestiary whose
/// weapon is an afterthought: the stat block exists for the
/// spellcasting, and the club is what it does when the slots run out.
/// Naming it after its wielder keeps the AI's log line ("Acolyte swings
/// acolyte club") legible next to the priest's mace and the cultist's
/// scimitar, which are the two other entries in the same tier.
pub static ACOLYTE_CLUB: SimpleWeapon = SimpleWeapon::melee(
    "acolyte club",
    &["ac-club", "acolyte-club"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Cultist ────────────────────────────────────────────────────────

/// Cultist Scimitar — DEX-based 1d6+DEX slashing melee. RAW: "Scimitar.
/// Melee Weapon Attack: +3 to hit, reach 5 ft., one target. Hit: 4 (1d6
/// + 1) slashing damage."
///
/// DEX rather than STR: the cultist's RAW +3 to hit comes off DEX 12 and
/// a +2 proficiency bonus, and the finesse property is what lets a
/// scimitar read off the higher of the two. Marked `light` so a cultist
/// pair fighting with two blades reaches the engine's two-weapon lane —
/// RAW's scimitar carries the property and nothing about being a cultist
/// takes it away.
pub static CULTIST_SCIMITAR: SimpleWeapon = SimpleWeapon::melee(
    "cultist scimitar",
    &["cs-scim", "cultist-scimitar"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Slashing,
)
.light();

// ─── Noble ──────────────────────────────────────────────────────────

/// Noble Rapier — DEX-based 1d8+DEX piercing melee. RAW: "Rapier. Melee
/// Weapon Attack: +3 to hit, reach 5 ft., one target. Hit: 5 (1d8 + 1)
/// piercing damage."
///
/// The bestiary's first rapier, and the reason it is DEX-based rather
/// than STR-based is the same reason the noble carries one: RAW's rapier
/// is a finesse weapon and the noble's STR is 11. Not `light` — RAW's
/// rapier is explicitly not a light weapon, which is what stops it
/// pairing with a second blade.
pub static NOBLE_RAPIER: SimpleWeapon = SimpleWeapon::melee(
    "noble rapier",
    &["nb-rapier", "noble-rapier"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Piercing,
);

// ─── Spy ────────────────────────────────────────────────────────────

/// Spy Shortsword — DEX-based 1d6+DEX piercing melee. RAW: "Shortsword.
/// Melee Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 5 (1d6
/// + 2) piercing damage." Light, so the spy's off-hand lane opens.
pub static SPY_SHORTSWORD: SimpleWeapon = SimpleWeapon::melee(
    "spy shortsword",
    &["spy-ss", "spy-shortsword"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
)
.light();

/// Spy Hand Crossbow — DEX-based 1d6+DEX piercing shot. RAW: "Hand
/// Crossbow. Ranged Weapon Attack: +4 to hit, range 30/120 ft., one
/// target. Hit: 5 (1d6 + 2) piercing damage."
///
/// 12 tiles normal / 20 max on the 2.5 ft grid — the same compressed
/// band the longbow uses, because the map is the constraint rather than
/// the bowstring: a board twenty-four tiles wide cannot express 120 feet
/// as anything but "the whole board", and a shot nothing can be out of
/// range of is a shot with no long-range clause at all.
pub static SPY_HAND_CROSSBOW: SimpleWeapon = SimpleWeapon::ranged(
    "spy hand crossbow",
    &["spy-hc", "hand-crossbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
    20,
    12,
);

// ─── Priest ─────────────────────────────────────────────────────────

/// Priest Mace — STR-based 1d6+STR bludgeoning melee. RAW: "Mace. Melee
/// Weapon Attack: +2 to hit, reach 5 ft., one target. Hit: 3 (1d6)
/// bludgeoning damage."
pub static PRIEST_MACE: SimpleWeapon = SimpleWeapon::melee(
    "priest mace",
    &["pr-mace", "priest-mace"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

// ─── Gladiator ──────────────────────────────────────────────────────

/// Gladiator Spear — STR-based 2d6+STR piercing melee at reach 5 ft.
/// RAW: "Spear. Melee or Ranged Weapon Attack: +7 to hit, reach 5 ft.
/// or range 20/60 ft., one target. Hit: 11 (2d6 + 4) piercing damage,
/// or 13 (2d8 + 4) piercing damage if used with two hands to make a
/// melee attack."
///
/// The two-handed clause is folded into the single die rather than
/// modeled as a versatile toggle: the gladiator's Multiattack spends
/// all three swings in melee, so the versatile half is the one that
/// always applies, and a toggle nothing ever flips is a rule that reads
/// as a feature.
pub static GLADIATOR_SPEAR: SimpleWeapon = SimpleWeapon::melee(
    "gladiator spear",
    &["gl-spear", "gladiator-spear"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Gladiator Shield Bash — STR-based 2d4+STR bludgeoning melee whose hit
/// forces a STR save (DC 15) or the target is knocked Prone. RAW: "Shield
/// Bash. Melee Weapon Attack: +7 to hit, reach 5 ft., one target. Hit: 9
/// (2d4 + 4) bludgeoning damage. If the target is a Medium or smaller
/// creature, it must succeed on a DC 15 Strength saving throw or be
/// knocked prone."
///
/// RAW's size gate is `max_target_size` on the chassis, checked ahead
/// of the save: an ogre is not a Large creature that made its save
/// against the shield, it is a creature the shield was never going to
/// trip, and the log says so rather than spending a die on it.
pub static GLADIATOR_SHIELD_BASH: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "gladiator shield bash",
    &["gl-bash", "shield-bash"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
    AbilityScoreType::Strength,
    15,
    Condition::Prone,
    ConditionTimer::Permanent,
    "shield bash",
)
.against_at_most(Size::Medium);

/// Gladiator Multiattack — 2 spears + 1 shield bash per Action. RAW:
/// "The gladiator makes three melee attacks or two ranged attacks."
///
/// The shield bash takes the third slot rather than a third spear
/// because RAW's three attacks are drawn from the whole stat block and
/// the bash is the only one of the two that does anything a spear does
/// not. Spending it first would waste the prone on a target the two
/// spears then have to hit anyway; spending it last means both spears
/// land before the target is on the floor, and whoever swings next
/// inherits the advantage.
pub static GLADIATOR_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "gladiator multiattack",
    parts: vec![(&GLADIATOR_SPEAR, 2), (&GLADIATOR_SHIELD_BASH, 1)],
});

// ─── Assassin ───────────────────────────────────────────────────────

/// Assassin Shortsword — DEX-based 1d6+DEX piercing melee whose hit
/// forces a CON save (DC 15) for 7d6 poison. RAW: "Shortsword. Melee
/// Weapon Attack: +6 to hit, reach 5 ft., one target. Hit: 6 (1d6 + 3)
/// piercing damage, and the target must make a DC 15 Constitution
/// saving throw, taking 24 (7d6) poison damage on a failed save, or
/// half as much damage on a successful one."
///
/// The venom is the stat block. 7d6 dwarfs the blade three times over,
/// which is what makes the assassin's CR 8 out of a CR 3 body, and it
/// is the reason this routes through `WeaponWithSaveDamage` rather than
/// carrying a flat rider: the save is not a garnish on the hit, it is
/// where nearly all of the damage lives.
pub static ASSASSIN_SHORTSWORD: WeaponWithSaveDamage = WeaponWithSaveDamage::melee(
    "assassin shortsword",
    &["as-ss", "assassin-shortsword"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    15,
    Dice::new(7, 6),
    DamageType::Poison,
    "assassin's blood",
);

/// Assassin Light Crossbow — DEX-based 1d8+DEX piercing shot with the
/// same DC 15 / 7d6 venom on the bolt. RAW: "Light Crossbow. Ranged
/// Weapon Attack: +6 to hit, range 80/320 ft., one target. Hit:
/// 7 (1d8 + 3) piercing damage, and the target must make a DC 15
/// Constitution saving throw, taking 24 (7d6) poison damage on a failed
/// save, or half as much damage on a successful one."
///
/// The ranged half of the same poison. It matters that both lanes carry
/// it: an assassin whose crossbow was a plain bow would be a melee
/// creature the AI shoots with only when cornered, and RAW's assassin is
/// exactly as lethal at range as in reach.
pub static ASSASSIN_LIGHT_CROSSBOW: WeaponWithSaveDamage = WeaponWithSaveDamage::ranged(
    "assassin light crossbow",
    &["as-lc", "assassin-crossbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    15,
    Dice::new(7, 6),
    DamageType::Poison,
    "assassin's blood",
    // RAW 80/320 ft, compressed onto the board: 16 tiles of clean band,
    // 24 of reachable-but-taxed. The widest range band in the bestiary
    // outside the giants' thrown rocks, which is the point of an
    // assassin — the venom arrives from somewhere the party is not
    // looking.
    24,
    16,
);

/// Assassin Multiattack — 2 shortswords per Action. RAW: "The assassin
/// makes two shortsword attacks."
pub static ASSASSIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "assassin multiattack",
    sub_attack: &ASSASSIN_SHORTSWORD,
    count: 2,
});

// ─── Mage ───────────────────────────────────────────────────────────

/// Mage Arcane Burst — INT-based 3d8 + INT force at 120 ft. RAW:
/// "Arcane Burst. Melee or Ranged Attack Roll: +6, reach 5 ft. or
/// range 120 ft. Hit: 16 (3d8 + 3) Force damage."
///
/// The clause that turned SRD 5.2's Mage from a caster with a dagger
/// into a caster with a gun. The 2014 stat block had no at-will attack
/// worth the name — a dagger, and a Fire Bolt if you gave it one — so a
/// mage out of slots was a creature the party could ignore. Three
/// bursts a turn at 3d8+3 apiece is a CR 6 damage lane that never runs
/// out, and it is the whole reason this stat block is a fight.
///
/// Declared as the ranged half only, the same compression the duergar
/// javelin gets: a weapon that is both is a weapon the AI has to be
/// taught to pick a mode for, and RAW prints the two on one line
/// because a stat block is a page, not because they are one attack.
///
/// `normal_range` equals the reach, because RAW gives this no
/// short/long split — 120 feet is the range, not the near band. The
/// attack ability is Intelligence, which is both RAW's ability and, on
/// this sheet, exactly the +3 that makes the printed +6 and the printed
/// 3d8 + 3 come out on their own.
pub static MAGE_ARCANE_BURST: SimpleWeapon = SimpleWeapon::ranged(
    "arcane burst",
    &["burst", "arcane-burst"],
    AbilityScoreType::Intelligence,
    Dice::new(3, 8),
    DamageType::Force,
    48,
    48,
);

/// Mage Multiattack — three Arcane Bursts per Action. RAW: "The mage
/// makes three Arcane Burst attacks."
pub static MAGE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "mage multiattack",
    sub_attack: &MAGE_ARCANE_BURST,
    count: 3,
});

// ─── Archmage ───────────────────────────────────────────────────────

/// Archmage Dagger — DEX-based 1d4+DEX piercing melee. RAW: "Dagger.
/// Melee or Ranged Weapon Attack: +6 to hit, reach 5 ft. or range 20/60
/// ft., one target. Hit: 4 (1d4 + 2) piercing damage."
///
/// A CR 12 creature's melee lane is not a threat and is not meant to be;
/// it exists so that an archmage with no slots left has something to do
/// other than stand still, which is the difference between a caster the
/// party has beaten and a caster the engine has to skip.
pub static ARCHMAGE_DAGGER: SimpleWeapon = SimpleWeapon::melee(
    "archmage dagger",
    &["am-dagger", "archmage-dagger"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
)
.light();

// ─── Azer ───────────────────────────────────────────────────────────

/// Azer Warhammer — STR-based 1d8+STR bludgeoning melee with a flat 1d6
/// fire rider. RAW: "Warhammer. Melee Weapon Attack: +5 to hit, reach 5
/// ft., one target. Hit: 7 (1d8 + 3) bludgeoning damage, or 8 (1d10 + 3)
/// bludgeoning damage if used with two hands, plus 3 (1d6) fire damage."
///
/// The fire is **Heated Weapon**, and it is not a rider the azer chooses:
/// the smith's hammer glows because the smith does, which is the same
/// reason the azer's body burns anyone who touches it. Modeled through
/// `WeaponWithRider` rather than as a second damage type on one die, so
/// a target resistant to fire and not to bludgeoning takes the halving
/// on exactly the half RAW halves.
pub static AZER_WARHAMMER: WeaponWithRider = WeaponWithRider::melee(
    "azer warhammer",
    &["az-hammer", "azer-hammer"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Bludgeoning,
    Dice::new(1, 6),
    DamageType::Fire,
    "heated weapon",
);

// ─── Barbed Devil ───────────────────────────────────────────────────

/// Barbed Devil Claw — STR-based 1d6+STR piercing melee. RAW: "Claw.
/// Melee Weapon Attack: +6 to hit, reach 5 ft., one target. Hit:
/// 6 (1d6 + 3) piercing damage." Piercing rather than slashing because
/// the hamatula's hands end in spikes rather than blades — the same
/// spikes that make its hide dangerous to touch.
pub static BARBED_DEVIL_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "barbed devil claw",
    &["bdv-claw", "hamatula-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Barbed Devil Tail — STR-based 2d6+STR piercing melee. RAW: "Tail.
/// Melee Weapon Attack: +6 to hit, reach 5 ft., one target. Hit: 10
/// (2d6 + 3) piercing damage." The heavy half of the multiattack.
pub static BARBED_DEVIL_TAIL: SimpleWeapon = SimpleWeapon::melee(
    "barbed devil tail",
    &["bdv-tail", "hamatula-tail"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Barbed Devil Hurl Flame — CHA-based 3d6 fire at range. RAW: "Hurl
/// Flame. Ranged Spell Attack: +5 to hit, range 150 ft., one target.
/// Hit: 10 (3d6) fire damage. If the target is a flammable object that
/// isn't being worn or carried, it also catches fire."
///
/// Filed as a ranged weapon rather than as a spell, which is a
/// deliberate divergence from RAW's "Ranged Spell Attack" label. The
/// engine's spell lane exists to carry slot cost, school, concentration
/// and counterspell exposure, and this attack has none of those — it is
/// an at-will innate the devil throws all day. What the weapon lane
/// gives it instead is the range band, which is the clause that
/// actually shapes how the hamatula fights: it opens at distance and
/// closes to claw only when something reaches it.
pub static BARBED_DEVIL_HURL_FLAME: SimpleWeapon = SimpleWeapon::ranged(
    "hurl flame",
    &["bdv-flame", "hurl-flame"],
    AbilityScoreType::Charisma,
    Dice::new(3, 6),
    DamageType::Fire,
    24,
    16,
);

/// Barbed Devil Multiattack — 2 claws + 1 tail per Action. RAW: "The
/// devil makes three melee attacks: one with its tail and two with its
/// claws."
pub static BARBED_DEVIL_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "barbed devil multiattack",
    parts: vec![(&BARBED_DEVIL_CLAW, 2), (&BARBED_DEVIL_TAIL, 1)],
});

// ─── Chain Devil ────────────────────────────────────────────────────

/// Chain Devil Chain — STR-based 2d6+STR slashing melee at reach 10 ft.
/// whose hit forces a DEX save (DC 15) or the target is Restrained.
/// RAW: "Chain. Melee Weapon Attack: +8 to hit, reach 10 ft., one
/// target. Hit: 11 (2d6 + 4) slashing damage. The target is grappled
/// (escape DC 14) if the devil isn't already grappling a creature.
/// Until this grapple ends, the target is restrained and takes 7 (2d6)
/// piercing damage at the start of each of its turns."
///
/// Three RAW clauses collapse into one Restrained install. The grapple
/// and the restraint are the same event here — a creature wrapped in
/// animate chains is not going anywhere by either name — and the engine
/// reads Restrained as the stronger of the two, so installing both
/// would be one condition doing the other's work. The start-of-turn
/// piercing tick is the clause genuinely dropped; it belongs to the
/// chain, not the devil, and there is no per-holder damage-over-time
/// lane on the weapon chassis to hang it from.
///
/// The DEX save replaces RAW's contested escape check for the same
/// reason every other grapple rider in the bestiary does: the engine
/// prices a grab as a save against the grabber's DC, and a second
/// mechanic for one creature would be a rule only the chain devil
/// obeys.
pub static CHAIN_DEVIL_CHAIN: WeaponWithSaveCondition = WeaponWithSaveCondition::reach_melee(
    "chain devil chain",
    &["cdv-chain", "animated-chain"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    AbilityScoreType::Dexterity,
    15,
    Condition::Restrained,
    ConditionTimer::Rounds(2),
    "animated chains",
    2,
)
.against_at_most(Size::Large);

/// Chain Devil Multiattack — 2 chains per Action. RAW: "The devil makes
/// two attacks with its chains."
pub static CHAIN_DEVIL_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "chain devil multiattack",
    sub_attack: &CHAIN_DEVIL_CHAIN,
    count: 2,
});

// ─── Darkmantle ─────────────────────────────────────────────────────

/// Darkmantle Crush — STR-based 1d6+STR bludgeoning melee that wraps
/// the darkmantle around whatever it hit. RAW: "Crush. Melee Attack
/// Roll: +5, reach 5 ft. Hit: 6 (1d6 + 3) Bludgeoning damage, and the
/// darkmantle attaches to the target. If the target is a Medium or
/// smaller creature and the darkmantle had Advantage on the attack
/// roll, it covers the target, which has the Blinded condition and is
/// suffocating while the darkmantle is attached in this way."
///
/// The blindness *and* the suffocation are the darkmantle's
/// `AttachProfile` now rather than a two-round `Blinded` install on
/// this weapon, which is the difference between a creature that
/// smothers you and one that flashes your eyes and lets go. The
/// suffocation half spent a long time struck out for want of a breath
/// clock; `engine::breath` is that clock, and the clause rides
/// `Condition::Choking` on the same `host_conditions_max_size` gate the
/// blindness does.
///
/// One clause is still dropped, and was dropped before: RAW's advantage
/// gate on covering. The darkmantle earns that advantage by dropping
/// from a ceiling the engine has no vertical axis to hang it from, so
/// gating on it would mean the signature clause of the creature almost
/// never fired. The *size* half of RAW's two-part gate is kept.
pub static DARKMANTLE_CRUSH: AttachingWeapon = AttachingWeapon::melee(
    "darkmantle crush",
    &["dm-crush", "darkmantle-crush"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
);

// ─── Duergar ────────────────────────────────────────────────────────

/// Duergar War Pick — STR-based 1d8+STR piercing melee. RAW: "War Pick.
/// Melee Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 6 (1d8
/// + 2) piercing damage, or 11 (2d8 + 2) piercing damage while enlarged."
///
/// The enlarged clause is not restated on the weapon: the duergar's
/// Enlarge routes through the engine's `Enlarged` condition, which
/// already adds its own die to every weapon hit the holder lands. Two
/// implementations of the same sentence would double it.
pub static DUERGAR_WAR_PICK: SimpleWeapon = SimpleWeapon::melee(
    "duergar war pick",
    &["dg-pick", "war-pick"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// Duergar Javelin — STR-based 1d6+STR piercing throw. RAW: "Javelin.
/// Melee or Ranged Weapon Attack: +4 to hit, reach 5 ft. or range
/// 30/120 ft., one target. Hit: 5 (1d6 + 2) piercing damage, or 9 (2d6
/// + 2) piercing damage while enlarged."
///
/// Declared as the ranged half only. The duergar has a war pick for
/// contact, and a weapon that is both is a weapon the AI has to be
/// taught to choose a mode for; RAW's dual-profile javelin is one
/// entry because a stat block is a page, not because the two modes are
/// the same attack.
pub static DUERGAR_JAVELIN: SimpleWeapon = SimpleWeapon::ranged(
    "duergar javelin",
    &["dg-jav", "duergar-javelin"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
    20,
    12,
);

// ─── Ochre Jelly ────────────────────────────────────────────────────

/// Ochre Jelly Pseudopod — STR-based 2d6+STR acid melee. RAW:
/// "Pseudopod. Melee Weapon Attack: +4 to hit, reach 5 ft., one target.
/// Hit: 9 (2d6 + 2) bludgeoning damage plus 3 (1d6) acid damage."
///
/// Typed acid outright rather than split into a bludgeoning body and an
/// acid rider, which is the one place this stat block deliberately
/// leaves RAW. The jelly *is* the acid — it has no mass to speak of and
/// nothing to swing — and the creature it most often gets compared
/// against, the black pudding, is typed the same way for the same
/// reason. Naming the whole hit acid means a target immune to acid
/// takes nothing from an ooze made of it, which is the answer at the
/// table even when it is not the answer on the page.
pub static OCHRE_JELLY_PSEUDOPOD: SimpleWeapon = SimpleWeapon::melee(
    "ochre jelly pseudopod",
    &["oj-pod", "jelly-pseudopod"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Acid,
);

// ─── Satyr ──────────────────────────────────────────────────────────

/// Satyr Ram — STR-based 2d4+STR bludgeoning melee. RAW: "Ram. Melee
/// Weapon Attack: +3 to hit, reach 5 ft., one target. Hit: 6 (2d4 + 1)
/// bludgeoning damage."
pub static SATYR_RAM: SimpleWeapon = SimpleWeapon::melee(
    "satyr ram",
    &["sy-ram", "satyr-ram"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Bludgeoning,
);

/// Satyr Shortsword — DEX-based 1d6+DEX piercing melee. RAW: "+5 to
/// hit, reach 5 ft., one target. Hit: 5 (1d6 + 2) piercing damage."
pub static SATYR_SHORTSWORD: SimpleWeapon = SimpleWeapon::melee(
    "satyr shortsword",
    &["sy-ss", "satyr-shortsword"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
)
.light();

/// Satyr Shortbow — DEX-based 1d6+DEX piercing shot. RAW: "+5 to hit,
/// range 80/320 ft., one target. Hit: 5 (1d6 + 2) piercing damage."
pub static SATYR_SHORTBOW: SimpleWeapon = SimpleWeapon::ranged(
    "satyr shortbow",
    &["sy-bow", "satyr-shortbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
    20,
    12,
);

// ─── Shield Guardian ────────────────────────────────────────────────

/// Shield Guardian Fist — STR-based 2d6+STR bludgeoning melee. RAW:
/// "Fist. Melee Weapon Attack: +7 to hit, reach 5 ft., one target. Hit:
/// 11 (2d6 + 4) bludgeoning damage."
pub static SHIELD_GUARDIAN_FIST: SimpleWeapon = SimpleWeapon::melee(
    "shield guardian fist",
    &["sg-fist", "guardian-fist"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Shield Guardian Multiattack — 2 fists per Action. RAW: "The guardian
/// makes two fist attacks."
pub static SHIELD_GUARDIAN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "shield guardian multiattack",
    sub_attack: &SHIELD_GUARDIAN_FIST,
    count: 2,
});

// ─── Violet Fungus ──────────────────────────────────────────────────

/// Violet Fungus Rotting Touch — STR-based 1d8 necrotic melee, flat
/// damage. RAW: "Rotting Touch. Melee Weapon Attack: +2 to hit, reach 10
/// ft., one creature. Hit: 4 (1d8) necrotic damage."
///
/// Flat rather than STR-scaled because RAW's line has no ability
/// modifier on it, and the fungus's STR 3 would subtract four from every
/// hit if the modifier were added — a stat block whose whole threat is
/// three lashes a turn would deal nothing at all.
pub static VIOLET_FUNGUS_ROTTING_TOUCH: SimpleWeapon = SimpleWeapon {
    display_name: "violet fungus rotting touch",
    aliases: &["vf-touch", "rotting-touch"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: None,
    damage_dice: Dice::new(1, 8),
    damage_type: DamageType::Necrotic,
    reach: 2,
    is_melee: true,
    requires_los: false,
    cost_resource: Resource::Action,
    normal_range: None,
    requires_condition: None,
    min_effective_range: None,
    is_light: false,
    mastery: None,
    bloodied_dice: None,
};

/// Violet Fungus Multiattack — 3 rotting touches per Action. RAW: "The
/// fungus makes 1d4 Rotting Touch attacks."
///
/// Three rather than a fresh 1d4 each turn. The engine's multiattack
/// chassis takes a fixed count, and a variable one would have to be a
/// bespoke action whose only difference from this is that the number of
/// swings is itself a die roll — three is the round average of 1d4 and
/// the fungus is a CR ¼ hazard, not a stat block anybody plans around.
pub static VIOLET_FUNGUS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "violet fungus multiattack",
    sub_attack: &VIOLET_FUNGUS_ROTTING_TOUCH,
    count: 3,
});

// ─── Warhorse Skeleton ──────────────────────────────────────────────

/// Warhorse Skeleton Hooves — STR-based 2d6+STR bludgeoning melee. RAW:
/// "Hooves. Melee Weapon Attack: +6 to hit, reach 5 ft., one target.
/// Hit: 11 (2d6 + 4) bludgeoning damage."
pub static WARHORSE_SKELETON_HOOVES: SimpleWeapon = SimpleWeapon::melee(
    "warhorse skeleton hooves",
    &["whs-hooves", "skeletal-hooves"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

// ─── Winged Kobold ──────────────────────────────────────────────────

/// Winged Kobold Dagger — DEX-based 1d4+DEX piercing melee. RAW:
/// "Dagger. Melee Weapon Attack: +4 to hit, reach 5 ft., one target.
/// Hit: 4 (1d4 + 2) piercing damage."
pub static WINGED_KOBOLD_DAGGER: SimpleWeapon = SimpleWeapon::melee(
    "winged kobold dagger",
    &["wk-dagger", "kobold-dagger"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
)
.light();

/// Winged Kobold Dropped Rock — DEX-based 1d6 bludgeoning shot, flat
/// damage. RAW: "Dropped Rock. Ranged Weapon Attack: +5 to hit, one
/// target directly under the kobold. Hit: 6 (1d6 + 2) bludgeoning
/// damage."
///
/// RAW's "directly under the kobold" is a vertical clause on a board
/// with no vertical axis, so the rock becomes an ordinary short-range
/// shot: 4 tiles of clean band, 8 of reach. Keeping the range tight is
/// what preserves the shape of the clause — a winged kobold has to be
/// nearly on top of something to drop anything on it.
pub static WINGED_KOBOLD_DROPPED_ROCK: SimpleWeapon = SimpleWeapon::ranged(
    "winged kobold dropped rock",
    &["wk-rock", "dropped-rock"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Bludgeoning,
    8,
    4,
);

// ─── Panther ────────────────────────────────────────────────────────

/// Panther Bite — STR-based 1d6+STR piercing melee. RAW: "Bite. Melee
/// Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 5 (1d6 + 2)
/// piercing damage."
pub static PANTHER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "panther bite",
    &["pn-bite", "panther-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Panther **Claw** — 1d4+STR slashing, and SRD 5.2's knockdown: *"If
/// the target is a Large or smaller creature, it has the Prone
/// condition."*
///
/// Small dice and a big consequence, which is the cat exactly: the claw
/// is not how a panther kills you, it is how it puts you where its bite
/// can. The pounce this creature also carries (`PANTHER_POUNCE`) is the
/// same idea paid for with a run-up; RAW gives it both, and they stack
/// the way a reader would expect — the run-up version is what the AI
/// spends movement to earn.
pub static PANTHER_CLAW: WeaponWithCondition = WeaponWithCondition::melee(
    "panther claw",
    &["pn-claw", "panther-claw"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Slashing,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "raking claw",
)
.against_at_most(Size::Large);

/// Panther **Pounce** (RAW): "If the panther moves at least 20 feet
/// straight toward a creature and then hits it with a claw attack on
/// the same turn, that target must succeed on a DC 12 Strength saving
/// throw or be knocked prone. If the target is prone, the panther can
/// make one bite attack against it as a bonus action."
///
/// Damage-free — the whole clause is the knockdown and the bite it
/// buys, which is what separates a pounce from the boar's charge. The
/// engine does have the conditional bonus-action grant this row's
/// docstring used to say it lacked; see `ChargeRider::prone_follow_up`.
pub const PANTHER_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("panther claw"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "panther pounce",
    knockdown_label: "panther pounce knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("panther bite"),
    max_target_size: None,
};

// ─── Remorhaz ───────────────────────────────────────────────────────

/// Remorhaz Bite — STR-based 6d10+STR piercing melee with a flat 3d6
/// fire rider. RAW: "Bite. Melee Weapon Attack: +11 to hit, reach 10
/// ft., one target. Hit: 40 (6d10 + 7) piercing damage plus 10 (3d6)
/// fire damage. If the target is a Large or smaller creature, it is
/// grappled (escape DC 17). Until this grapple ends, the target is
/// restrained, and the remorhaz can't bite another target."
///
/// The heaviest single die pool in the bestiary short of the tarrasque,
/// and the fire is not a garnish: the remorhaz's body runs hot enough
/// that swallowing something cooks it. The grapple / swallow half is
/// dropped — the engine has no swallow lane, and a Restrained install
/// on top of forty average damage would make the bite a save-or-lose at
/// a CR that already has enough. `WeaponWithRider` rather than a
/// bespoke action, so the two damage types meet the target's resistance
/// table separately, which is the whole reason a fire-immune creature
/// takes forty from this and not fifty.
pub static REMORHAZ_BITE: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "remorhaz bite",
    &["rz-bite", "remorhaz-bite"],
    AbilityScoreType::Strength,
    Dice::new(6, 10),
    DamageType::Piercing,
    &[Condition::Grappled, Condition::Restrained],
    ConditionTimer::Permanent,
    "molten coils",
    2,
)
.against_at_most(Size::Large)
.plus_damage(Dice::new(3, 6), DamageType::Fire, "superheated gullet");

// ─── Water Weird ────────────────────────────────────────────────────

/// Water Weird Constrict — STR-based 3d6+STR bludgeoning melee at reach
/// 10 ft. whose hit Restrains the target. RAW: "Constrict. Melee Weapon
/// Attack: +6 to hit, reach 10 ft., one creature. Hit: 13 (3d6 + 3)
/// bludgeoning damage. If the target is Medium or smaller, it is
/// grappled (escape DC 13) and pulled 5 feet toward the water weird.
/// Until this grapple ends, the target is restrained, the water weird
/// tries to drown it, and the water weird can't constrict another
/// target."
///
/// An auto-install rather than a save, which is the one place this
/// leaves the engine's usual grapple shape and does so on RAW's word:
/// the water weird's grab is not contested at the moment it lands — the
/// escape DC is what the *victim* rolls against later, on their own
/// turn, which is a lane the engine spells as a timer rather than as an
/// opposed check. Two rounds is that timer.
///
/// The pull is RAW's five feet, on the chassis's displacement lane —
/// which is what makes the water weird a *puller* rather than a
/// long-armed grappler: it drags whoever it catches off the bank and
/// into the water it lives in. The drowning is still dropped; that one
/// needs a breath clock.
pub static WATER_WEIRD_CONSTRICT: WeaponWithCondition = WeaponWithCondition::reach_melee(
    "water weird constrict",
    &["ww-constrict", "water-constrict"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    &[Condition::Restrained],
    ConditionTimer::Rounds(2),
    "coiling water",
    2,
)
.against_at_most(Size::Medium)
.pulls(tiles_from_feet(5));

// ─── Rug of Smothering ──────────────────────────────────────────────

/// Rug of Smothering Smother — STR-based 2d6+STR bludgeoning melee
/// whose hit pins and smothers at once. RAW: "Smother. Melee Attack
/// Roll: +5, reach 5 ft. Hit: 10 (2d6 + 3) Bludgeoning damage. If the
/// target is a Medium or smaller creature, the rug can give it the
/// Grappled condition (escape DC 13) instead of dealing damage. Until
/// the grapple ends, the target has the Blinded and Restrained
/// conditions, **is suffocating**, and takes 10 (2d6 + 3) Bludgeoning
/// damage at the start of each of its turns."
///
/// RAW's grapple deals no damage on the hit that lands it — the
/// crushing is a per-turn tick, which the weapon chassis has no lane
/// for. Folding one round of it into the swing is the honest
/// compression: the rug still trades a hit for roughly ten bludgeoning
/// and a smothered victim, and the arithmetic across a two-round hold
/// comes out close.
///
/// Restrained rather than Blinded, because the engine reads Restrained
/// as the stronger of the two and installing both would be one
/// condition doing the other's work — a Restrained creature already
/// hands out advantage and attacks at disadvantage, which is where the
/// blinding was going.
///
/// The suffocation is not a rephrasing of either, and it ships now that
/// there is a clock to read it. It was the clause this stat block was
/// named for and the one it did not have: a rug that pins you is a
/// wolf, and a rug that pins you and stops your breathing is a rug of
/// smothering. Two rounds under the weave is two rungs of exhaustion —
/// half speed and disadvantage on every check — bought without rolling
/// a single point of damage for it. See `engine::breath`.
pub static RUG_OF_SMOTHERING_SMOTHER: WeaponWithCondition = WeaponWithCondition::melee(
    "rug of smothering smother",
    &["rug-smother", "smother"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    &[Condition::Restrained, Condition::Choking],
    ConditionTimer::Rounds(2),
    "smothering weave",
)
.against_at_most(Size::Medium);

// ─── Merfolk ────────────────────────────────────────────────────────

/// Merfolk Spear — STR-based 1d6+STR piercing melee. RAW: "Spear. Melee
/// or Ranged Weapon Attack: +2 to hit, reach 5 ft. or range 20/60 ft.,
/// one target. Hit: 3 (1d6) piercing damage, or 4 (1d8) piercing damage
/// if used with two hands to make a melee attack."
///
/// A spear rather than a trident, and the difference matters exactly
/// once: 5e's Underwater Combat rules exempt both, and the merfolk is
/// the creature most likely to be swinging one in a lake.
pub static MERFOLK_SPEAR: SimpleWeapon = SimpleWeapon::melee(
    "merfolk spear",
    &["mf-spear", "merfolk-spear"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

// ─── Homunculus ─────────────────────────────────────────────────────

/// Homunculus Bite — DEX-based 1d4 piercing melee, flat damage, with a
/// CON save (DC 10) for 2d4 poison. RAW: "Bite. Melee Weapon Attack: +4
/// to hit, reach 5 ft., one target. Hit: 1 piercing damage, and the
/// target must succeed on a DC 10 Constitution saving throw or be
/// poisoned for 1 minute. If the saving throw fails by 5 or more, the
/// target is instead poisoned for 5 (1d10) minutes and unconscious
/// while poisoned in this way."
///
/// RAW's bite deals one point. The venom is the whole attack, and the
/// fail-by-5 unconsciousness clause is what makes a CR 0 construct
/// worth putting on a board at all — the engine has no fail-by-N lane,
/// so the Poisoned install carries the threat and the 2d4 stands in for
/// the rest.
pub static HOMUNCULUS_BITE: WeaponWithSaveDamage = WeaponWithSaveDamage::melee_with_condition(
    "homunculus bite",
    &["hm-bite", "homunculus-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    AbilityScoreType::Constitution,
    10,
    Dice::new(2, 4),
    DamageType::Poison,
    "alchemical venom",
    Condition::Poisoned,
    ConditionTimer::Rounds(3),
);

// ─── Giant Fire Beetle ──────────────────────────────────────────────

/// Giant Fire Beetle Bite — STR-based 1d6 slashing melee, flat damage.
/// RAW: "Bite. Melee Weapon Attack: +1 to hit, reach 5 ft., one target.
/// Hit: 2 (1d6) slashing damage."
///
/// Slashing off a bite, which is the one place this beetle breaks the
/// naming convention and does so on RAW's word — the mandibles shear
/// rather than puncture. Flat damage: RAW's line carries no ability
/// modifier and the beetle's STR 8 would subtract one from every hit.
pub static GIANT_FIRE_BEETLE_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "giant fire beetle bite",
    &["gfb-bite", "beetle-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

// ─── Jackal ─────────────────────────────────────────────────────────

/// Jackal Bite — STR-based 1d4 piercing melee, flat damage. RAW: "Bite.
/// Melee Weapon Attack: +1 to hit, reach 5 ft., one target. Hit:
/// 1 (1d4 - 1) piercing damage." The minus is the jackal's STR 8; flat
/// 1d4 is the engine's nearest expression of a bite that barely breaks
/// skin.
pub static JACKAL_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "jackal bite",
    &["jk-bite", "jackal-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

// ─── Raven ──────────────────────────────────────────────────────────

/// Raven Beak — DEX-based 1 piercing melee, flat. RAW: "Beak. Melee
/// Weapon Attack: +4 to hit, reach 5 ft., one target. Hit: 1 piercing
/// damage." A single point, expressed as the smallest die the engine
/// has — there is no `Dice::new(0, 0) + 1` and a d4 that averages two
/// and a half is closer to one than a d6 is.
pub static RAVEN_BEAK: SimpleWeapon = SimpleWeapon::flat_melee(
    "raven beak",
    &["rv-beak", "raven-beak"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 2),
    DamageType::Piercing,
);

// ─── Vulture ────────────────────────────────────────────────────────

/// Vulture Beak — STR-based 1d4 piercing melee, flat damage. RAW:
/// "Beak. Melee Weapon Attack: +2 to hit, reach 5 ft., one target. Hit:
/// 2 (1d4) piercing damage."
pub static VULTURE_BEAK: SimpleWeapon = SimpleWeapon::flat_melee(
    "vulture beak",
    &["vl-beak", "vulture-beak"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

// ─── Quipper ────────────────────────────────────────────────────────

/// Giant Weasel Bite — DEX-based 1d4+DEX piercing melee. RAW: "Bite.
/// Melee Weapon Attack: +5 to hit, reach 5 ft., one target. Hit:
/// 5 (1d4 + 3) piercing damage." DEX-based off the weasel's 16, which
/// is where RAW's +3 comes from and why a creature with STR 11 hits
/// like this.
pub static GIANT_WEASEL_BITE: SimpleWeapon = SimpleWeapon::melee(
    "giant weasel bite",
    &["gw-bite", "weasel-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
);

// ─── Umber Hulk ─────────────────────────────────────────────────────

/// Umber Hulk Confusing Gaze — the trait the stat block is named for and
/// the one the bestiary shipped without.
///
/// RAW (MM p.292) is a start-of-turn trigger: "When a creature starts
/// its turn within 30 feet of the umber hulk and is able to see the
/// umber hulk's eyes, the umber hulk can magically force it to make a
/// DC 15 Charisma saving throw… On a failed save, the creature can't
/// take reactions until the start of its next turn and rolls a d8 to
/// determine what it does during that turn."
///
/// Rendered here as an Action-cost gaze rather than a per-target
/// turn-start hook, which is the same shape every other stare in the
/// bestiary already uses — the Mummy's Dreadful Glare, the Medusa's
/// Petrifying Gaze, the Sea Hag's Death Glare. The engine has no
/// "when a creature starts its turn near X" hook to hang the RAW
/// wording on, and inventing one for a single monster would buy a
/// worse version of what `resolve_los_glare_condition` already does:
/// sweep the enemies that can actually see the hulk, roll one save
/// each, and install on the failures.
///
/// The d8 chaos table collapses to `Confused`, which is where this
/// engine already puts Confusion's table — disadvantage on attack rolls
/// and no reactions. RAW's own "can't take reactions" clause is
/// therefore modeled exactly and the movement half of the table is the
/// part that rounds off.
///
/// Radius 12 tiles is RAW's 30 ft. The LOS filter is not decoration
/// here: "able to see the umber hulk's eyes" is the whole of the
/// trait's counterplay, and a hulk burrowing up behind a wall should
/// get nothing for it.
pub struct UmberHulkConfusingGaze {}

impl UmberHulkConfusingGaze {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 12;
}

impl Action for UmberHulkConfusingGaze {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

    fn name(&self) -> &str {
        "confusing gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cg", "gaze", "confusing-gaze"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::action_template::resolve_los_glare_condition;
        // 30 ft RAW on the 2.5 ft grid.
        const DC: i32 = 15;
        encounter.log("  confusing gaze: the hulk's compound eyes swivel and scatter the mind");
        resolve_los_glare_condition(
            encounter,
            caster_id,
            Self::RADIUS,
            AbilityScoreType::Charisma,
            DC,
            Condition::Confused,
            // RAW's window is "during that turn" — one round, which is
            // exactly as long as it takes the victim to waste it.
            ConditionTimer::Rounds(1),
            // No creature-type exemption: RAW's only gate is sight, and
            // the LOS filter above is it.
            None,
        )
    }
}

pub static UMBER_HULK_CONFUSING_GAZE: LazyLock<UmberHulkConfusingGaze> =
    LazyLock::new(|| UmberHulkConfusingGaze {});

// ─── Roper ──────────────────────────────────────────────────────────

/// Roper Bite — STR-based 4d6+STR piercing melee. RAW: "Bite. Melee
/// Weapon Attack: +7 to hit, reach 5 ft., one target. Hit: 22 (4d6 + 4)
/// piercing damage." The payoff at the end of the roper's whole
/// sequence: the tendrils catch, the reel drags the catch into reach,
/// and this is what waits there.
pub static ROPER_BITE: SimpleWeapon = SimpleWeapon::melee(
    "roper bite",
    &["rp-bite", "roper-bite"],
    AbilityScoreType::Strength,
    Dice::new(4, 6),
    DamageType::Piercing,
);

/// Roper Tendril — the reach-50-ft grab that makes a roper a roper.
///
/// RAW: "Tendril. Melee Weapon Attack: +7 to hit, reach 50 ft., one
/// creature. Hit: The target is grappled (escape DC 15). Until the
/// grapple ends, the target is restrained and has disadvantage on
/// Strength checks and Strength saving throws, and the roper can't use
/// the same tendril on another target."
///
/// Damage-free by design — the tendril's entire output is the hold, and
/// that is why this is a bespoke action rather than a
/// `WeaponWithCondition` declaration. That chassis installs exactly one
/// condition and rolls damage dice to decide whether it landed; the
/// tendril installs *two* (the `Grappled` that Reel reads the back-link
/// off, and the `Restrained` that carries RAW's disadvantage clauses)
/// and has no damage to gate on. The hit/miss question is therefore
/// asked directly of `resolve_attack_outcome`, with a 0d0 pool, and
/// answered by whether the swing produced any effects at all — the same
/// gate the rider chassis use one layer up.
///
/// The `Grappled` install goes through `install_condition_with_link`,
/// which is not optional: without the back-link a Reel would find
/// nobody, and the roper's own docstring used to say so — "a roper's
/// tendril that grapples nobody in particular is a hold nothing can
/// end."
///
/// RAW's per-tendril bookkeeping (six tendrils, one target each) is not
/// modeled. The engine has no per-limb state, and the multiattack below
/// is capped at four swings, so the only thing the rule would change is
/// whether all four can land on one already-held victim — which the
/// `Grappled` re-install would swallow anyway.
pub struct RoperTendril {}

impl Action for RoperTendril {
    fn name(&self) -> &str {
        "tendril"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tendril", "rp-tendril"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 50 ft RAW = 20 tiles on the 2.5 ft grid.
        Some(20)
    }
    /// A swing, not a shot — RAW says "Melee Weapon Attack: … reach 50
    /// ft.", and the kraken's six-tile tentacle already established
    /// that reach is not what decides this. Getting it wrong the other
    /// way costs more than it looks: a ranged tendril would take
    /// disadvantage for every hostile in the roper's own reach, would
    /// need a normal range for the falloff and underwater rules to
    /// read, and would make `has_ranged_attack` call a creature with a
    /// 10 ft walk speed a kiter.
    fn is_melee_attack(&self) -> bool {
        true
    }
    /// Declared against the melee default, because at twenty tiles the
    /// clause the default encodes stops being true. `requires_los` is
    /// false for melee on the argument that contact does not need
    /// sight; a strand thrown the length of a cavern is not contact,
    /// and without this a roper would fish through walls.
    fn requires_los(&self) -> bool {
        true
    }
    fn is_weapon_attack(&self) -> bool {
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
        let swing = simple_weapon_attack_ranged(
            encounter,
            caster_id,
            target_ids,
            "tendril",
            AbilityScoreType::Strength,
            // No damage ability, and a 0d0 pool: RAW's tendril line
            // has no damage entry at all, and routing through the
            // ranged helper is what lets the STR modifier be left off
            // the damage side while still being on the attack side.
            None,
            Dice::new(0, 0),
            DamageType::Bludgeoning,
            // Melee, per RAW and per `is_melee_attack` above — the die
            // and the picker have to agree about this or the roper
            // swings under one rule and is ranked under another.
            true,
            None,
            None,
        );
        // The helper returns an empty vec on a miss and a `DealDamage`
        // on a hit — a zero one here, which is why the vec is read as a
        // hit flag and then dropped rather than returned. That is the
        // same gate every rider chassis reads one layer up.
        if swing.is_empty() {
            encounter.log("  tendril: the strand whips past");
            return Vec::new();
        }
        encounter.log("  tendril: the strand wraps tight and holds");
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            Condition::Grappled,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        ));
        // Linked too, and for the same reason the `Grappled` is: RAW
        // makes the restraint a *consequence* of the hold ("until the
        // grapple ends, the target is restrained"), so it has to name
        // the same holder or it will outlive the strand it came from.
        // See `GrappleEscape`, which reads both links.
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            Condition::Restrained,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        ));
        effects
    }
}

pub static ROPER_TENDRIL: LazyLock<RoperTendril> = LazyLock::new(|| RoperTendril {});

/// Roper Multiattack — four tendrils per Action, per RAW's "The roper
/// makes four attacks with its tendrils, uses Reel, and makes one attack
/// with its bite."
///
/// Split from the reel and the bite rather than compounded with them,
/// because the three parts want different turns. A roper that has
/// nobody held should be throwing tendrils; one that has somebody held
/// forty feet away should be reeling; one with a victim in its mouth
/// should be biting. Folding all three into one `CompoundAttack` would
/// make every roper turn identical and mostly wasted — four tendrils at
/// a target already wrapped, a reel of nobody, a bite at empty air.
/// The AI picks between them; see `ai::simple::try_reel`.
pub static ROPER_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "tendril flurry",
    sub_attack: &*ROPER_TENDRIL,
    count: 4,
});

/// Roper Reel — "The roper pulls each creature grappled by it up to 25
/// feet straight toward it."
///
/// The other half of the tendril, and the reason the tendril bothers to
/// link its `Grappled` install: this walks every actor whose hold traces
/// back to *this* roper and drags them ten tiles closer. Nothing else
/// in the engine reads a grapple back-link to decide who moves, which
/// is why this is a bespoke action and not a chassis — one caller, one
/// shape, and a second one would be a different monster's rules.
///
/// Free of cost by RAW's reading (the reel is part of the multiattack,
/// not a separate action), but priced here as a Bonus Action so the
/// roper can throw tendrils *and* reel on the same turn, which is what
/// the RAW sequence amounts to. Charging it a full Action would make
/// the roper choose between catching and pulling, and a roper that
/// never pulls is a roper with a slightly long bite.
///
/// `PullActor` carries the movement, so the drag stops at walls and
/// occupied tiles and fires no opportunity attacks — 5e treats forced
/// movement as not a willing move, and this is the engine's one place
/// that rule is written down.
pub struct RoperReel {}

impl RoperReel {
    /// Every combat-active actor currently grappled *by* `roper_id`, in
    /// ascending id order so the drag sequence is deterministic across
    /// a seed sweep.
    ///
    /// `linked_by` folds the "is it held at all" check into the link
    /// read, so a stale id whose grapple already lapsed can't be
    /// reeled — see `ActorInstance::linked_by` for why that pairing is
    /// the only read path onto the links.
    fn caught_by(encounter: &EncounterInstance, roper_id: usize) -> Vec<usize> {
        encounter
            .sorted_actor_ids()
            .into_iter()
            .filter(|&id| {
                encounter.actors.get(&id).is_some_and(|a| {
                    a.is_combat_active() && a.linked_by(Condition::Grappled) == Some(roper_id)
                })
            })
            .collect()
    }
}

impl Action for RoperReel {
    fn self_burst_radius(&self) -> Option<isize> {
        // The reel reaches whatever the tendrils reached, which is the
        // roper's own 60-foot envelope.
        Some(24)
    }
    fn name(&self) -> &str {
        "reel"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["reel", "rp-reel"]
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
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        !Self::caught_by(encounter, caster_id).is_empty()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(anchor) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
            return Vec::new();
        };
        let caught = Self::caught_by(encounter, caster_id);
        if caught.is_empty() {
            return Vec::new();
        }
        encounter.log(format!(
            "  reel: the roper hauls in {} caught {}",
            caught.len(),
            if caught.len() == 1 {
                "creature"
            } else {
                "creatures"
            }
        ));
        caught
            .into_iter()
            .map(|id| {
                Box::new(crate::engine::side_effects::PullActor {
                    actor_id: id,
                    toward: anchor,
                    // 25 ft RAW = 10 tiles.
                    max_tiles: 10,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static ROPER_REEL: LazyLock<RoperReel> = LazyLock::new(|| RoperReel {});

// ─── Cloaker ────────────────────────────────────────────────────────

/// Cloaker Moan — "the cloaker emits a terrifying moan. Each creature
/// within 60 feet of the cloaker that can hear the moan and that isn't
/// an aberration must succeed on a DC 13 Wisdom saving throw or become
/// frightened until the end of the cloaker's next turn."
///
/// The half of the cloaker's stat block that made it worth a CR 8 slot
/// and that the bestiary shipped without — a cloaker with only its tail
/// is a slow Large creature with 78 hit points and a 10 ft walk.
///
/// Routed through `resolve_audible_burst_condition`, which is RAW's own
/// gate: a moan is sound, so it does not need line of sight and does
/// need an ear. The engine had no hearing model when the cloaker
/// arrived, so this used the gaze helper and paid for it with the one
/// clause a gaze carries and a moan does not — a cloaker behind a
/// corner got nothing. `ActorInstance::can_hear` is what made the
/// honest version available.
///
/// The aberration exemption is proxied by psychic immunity, which is
/// this engine's standing shorthand for "mind like the cloaker's" the
/// same way necrotic immunity stands in for undead on the Mummy's
/// glare. It is a proxy and not the rule: it exempts a few non-
/// aberrations that happen to be psychically immune, and it catches
/// every aberration that matters here.
pub struct CloakerMoan {}

impl CloakerMoan {
    /// How far the ability reaches, in tiles.
    ///
    /// An associated const rather than a `const` inside `side_effects`,
    /// because two things read it now: the resolver, and
    /// `self_burst_radius`, which is what the AI gates on. A literal in
    /// each would be two numbers to keep in step, and a drift between
    /// them is invisible — the ability would simply start being chosen
    /// in the wrong situations.
    const RADIUS: isize = 24;
}

impl Action for CloakerMoan {
    fn self_burst_radius(&self) -> Option<isize> {
        // The radius the ability resolves at, declared so the AI's
        // self-centred-burst rung stops guessing at it. See
        // `Action::self_burst_radius`.
        Some(Self::RADIUS)
    }

    fn name(&self) -> &str {
        "moan"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["moan", "cl-moan"]
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
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::action_template::resolve_audible_burst_condition;
        // 60 ft RAW on the 2.5 ft grid.
        const DC: i32 = 13;
        encounter.log("  moan: a subsonic wail rolls out of the dark");
        resolve_audible_burst_condition(
            encounter,
            caster_id,
            Self::RADIUS,
            AbilityScoreType::Wisdom,
            DC,
            Condition::Frightened,
            // "Until the end of the cloaker's next turn" — two rounds
            // on the engine's clock, the same window every other
            // short monster fear uses.
            ConditionTimer::Rounds(2),
            Some(DamageType::Psychic),
        )
    }
}

pub static CLOAKER_MOAN: LazyLock<CloakerMoan> = LazyLock::new(|| CloakerMoan {});

/// Cloaker Attach — STR-based 3d6+STR piercing melee that wraps the
/// cloaker around whatever it hit. RAW: "Attach. Melee Attack Roll: +6,
/// reach 5 ft. Hit: 13 (3d6 + 3) Piercing damage. If the target is a
/// Large or smaller creature, the cloaker attaches to it. While the
/// cloaker is attached, the target has the Blinded condition, and the
/// cloaker can't make Attach attacks against other targets. In
/// addition, the cloaker halves the damage it takes (round down), and
/// the target takes the same amount of damage."
///
/// All four clauses land now. The latch is this chassis; the blindness,
/// the size gate and the damage split are the cloaker's
/// `AttachProfile`; and the "can't make Attach attacks against other
/// targets" half is the shared hostility gate every latched creature
/// answers. The docstring this replaced conceded the whole rider — "the
/// engine has no riding-on-a-creature's-back state" — which was true
/// when it was written, and is what `engine::attachment` is.
///
/// Renamed from "cloaker bite" to match SRD 5.2, which calls the attack
/// **Attach** and gives it 3d6 rather than the 2d6 the older printing
/// did; the aliases keep the old name reachable from the prompt.
pub static CLOAKER_ATTACH: AttachingWeapon = AttachingWeapon::melee(
    "cloaker attach",
    &["cl-bite", "cloaker-bite", "cl-attach"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
);

/// Cloaker Multiattack — SRD 5.2: "The cloaker makes one Attach attack
/// and two Tail attacks." Heterogeneous, so `CompoundAttack` rather
/// than the homogeneous chassis.
///
/// The tail count was one and RAW's is two, which was the other half of
/// the same stale printing `CLOAKER_ATTACH` came from — the bite was
/// 2d6 there and the Action was two swings rather than three.
///
/// The Attach leads, which is the order RAW prints and the order that
/// matters: the latch is tried before the tails, so a cloaker that
/// wraps somebody spends the rest of its Action on a Blinded target.
///
/// The Action stays available once the cloaker has landed it. RAW bars
/// its Attach only "against other targets", so a wrapped cloaker keeps
/// swinging all three attacks at the creature it is holding — the
/// Attach re-rolls its 3d6 and no-ops the latch, and the tails follow.
/// That is load-bearing for the `CompoundAttack` chassis, which
/// validates `all` of its parts and would otherwise refuse the whole
/// Multiattack because one part of it had succeeded.
pub static CLOAKER_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "cloaker multiattack",
    parts: vec![(&CLOAKER_ATTACH, 1), (&CLOAKER_TAIL, 2)],
});

// ═══════════════════════════════════════════════════════════════════
// SRD 5.2 beast roster — the ambient fauna ladder
//
// Everything below this line belongs to the block of stat blocks the
// SRD files under "Animals": the CR-0 scenery, the CR-⅛ to CR-4
// predators a druid's Conjure Animals draws from, and the four
// dinosaurs. They share a shape — one or two natural weapons, no
// spellcasting, a trait or two of pure positioning — so they are
// gathered here rather than scattered by size or habitat.
// ═══════════════════════════════════════════════════════════════════

// ─── Ape ─────────────────────────────────────────────────────────────

/// Ape Fist — STR-based 1d4+STR bludgeoning melee. RAW: "Fist. Melee
/// Attack Roll: +5, reach 5 ft. Hit: 5 (1d4 + 3) Bludgeoning damage."
/// The smaller of the ape's two limbs by damage, and the one it throws
/// twice a turn through `APE_MULTI`.
pub static APE_FIST: SimpleWeapon = SimpleWeapon::melee(
    "ape fist",
    &["fist", "ape-punch"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

/// Ape Multiattack — "The ape makes two Fist attacks."
pub static APE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "ape multiattack",
    sub_attack: &APE_FIST,
    count: 2,
});

/// Ape Rock — the thrown-rock swing, without its recharge. RAW: "Rock
/// (Recharge 6). Ranged Attack Roll: +5, range 25/50 ft. Hit: 10 (2d6
/// + 3) Bludgeoning damage."
///
/// Ranges converted at the engine's 2.5 ft per tile: 25 ft normal → 10
/// tiles, 50 ft maximum → reach 20. Beyond the normal band and inside
/// the maximum the shot rolls at disadvantage, which is the whole point
/// of a stat block printing two numbers.
///
/// STR-based on both halves rather than DEX, matching RAW's +5 to hit
/// on a STR 16 / DEX 14 frame: a thrown rock is a heavy-object toss,
/// and the ape's own stat line prices it off its arms.
static APE_ROCK_THROW: SimpleWeapon = SimpleWeapon::ranged(
    "ape rock",
    &["rock", "hurl-rock"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    20,
    10,
);

/// Ape Rock as the ape has it — the throw above, gated on RAW's
/// **Recharge 6**.
///
/// The roster's first user of `RechargingAttack`, and the reason that
/// chassis exists: before it, the only things in the engine that could
/// read a recharge key were the two breath-weapon shapes, neither of
/// which rolls an attack. The key is `"rock"`, which the ape's template
/// must spell identically in `recharge_abilities` — see the chassis
/// docs for why a mismatch fails closed.
pub static APE_ROCK: LazyLock<RechargingAttack> = LazyLock::new(|| RechargingAttack {
    display_name: "ape rock",
    sub_attack: &APE_ROCK_THROW,
    recharge_key: "rock",
});

// ─── Baboon ──────────────────────────────────────────────────────────

/// Baboon Bite — STR-based 1d4+STR piercing melee. RAW: "Bite. Melee
/// Attack Roll: +1, reach 5 ft. Hit: 1 (1d4 − 1) Piercing damage."
///
/// The minus one is the baboon's own Strength 8, not a flat penalty on
/// the weapon, so this is an ordinary STR-based swing and the stat
/// block's arithmetic falls out of the template. What makes a baboon
/// dangerous is never the die — it is that there are six of them and
/// they all have Pack Tactics.
pub static BABOON_BITE: SimpleWeapon = SimpleWeapon::melee(
    "baboon bite",
    &["baboon", "bab-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Piercing,
);

// ─── Badger ──────────────────────────────────────────────────────────

/// Badger Bite — flat 1 piercing. RAW: "Bite. Melee Attack Roll: +2,
/// reach 5 ft. Hit: 1 Piercing damage."
///
/// A `1d1`-shaped die rather than a literal, so a confirmed crit
/// doubles to 2 through the engine's uniform crit chassis instead of a
/// special-case flat path — the same shape `HAWK_TALONS` uses.
/// STR-based on the roll (STR 10, so +0 + PB 2 = the printed +2) and
/// flat on the damage, which is what `flat_melee` is for.
pub static BADGER_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "badger bite",
    &["badger", "bdg-bite"],
    AbilityScoreType::Strength,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Black Bear ──────────────────────────────────────────────────────

/// Black Bear Rend — STR-based 1d6+STR slashing melee. RAW: "Rend.
/// Melee Attack Roll: +4, reach 5 ft. Hit: 5 (1d6 + 2) Slashing
/// damage." SRD 5.2 collapses the older claw/bite pair into one limb
/// swung twice, which is what `BLACK_BEAR_MULTI` does.
pub static BLACK_BEAR_REND: SimpleWeapon = SimpleWeapon::melee(
    "black bear rend",
    &["rend", "bb-rend"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Slashing,
);

/// Black Bear Multiattack — "The bear makes two Rend attacks."
pub static BLACK_BEAR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "black bear multiattack",
    sub_attack: &BLACK_BEAR_REND,
    count: 2,
});

// ─── Blood Hawk ──────────────────────────────────────────────────────

/// Blood Hawk Beak — DEX-based 1d4+DEX piercing melee that swaps to
/// **1d8** against a Bloodied target. RAW: "Beak. Melee Attack Roll:
/// +4, reach 5 ft. Hit: 4 (1d4 + 2) Piercing damage, or 6 (1d8 + 2)
/// Piercing damage if the target is Bloodied."
///
/// The roster's only carrier of `SimpleWeapon::bloodied_dice`, and the
/// stat block that motivated the field. It is the mechanical statement
/// of the bird's name: a blood hawk is not much of a threat to a
/// healthy creature and is a serious one to a wounded one, which —
/// paired with Pack Tactics on the template — is what makes a flock
/// the thing that finishes a fight somebody else started.
pub static BLOOD_HAWK_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "blood hawk beak",
    &["beak", "bh-beak"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
)
.escalating_vs_bloodied(Dice::new(1, 8));

// ─── Crab ────────────────────────────────────────────────────────────

/// Crab Claw — flat 1 bludgeoning. RAW: "Claw. Melee Attack Roll: +2,
/// reach 5 ft. Hit: 1 Bludgeoning damage." DEX-based on the roll (DEX
/// 11 → +0 + PB 2 = +2); the crab's Strength 6 would have made a
/// STR-based swing miss the printed number by two.
pub static CRAB_CLAW: SimpleWeapon = SimpleWeapon::flat_melee(
    "crab claw",
    &["claw-c", "pinch"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Bludgeoning,
);

// ─── Deer ────────────────────────────────────────────────────────────

/// Deer Ram — STR-based 1d4+STR bludgeoning melee. RAW: "Ram. Melee
/// Attack Roll: +2, reach 5 ft. Hit: 2 (1d4) Bludgeoning damage."
/// Strength 11 makes the modifier zero, so the printed `(1d4)` and an
/// ordinary STR swing are the same number.
///
/// The deer's actual combat contribution is the **Agile** trait on its
/// template, not this — a deer that is being swung at leaves, and the
/// leaving is free.
pub static DEER_RAM: SimpleWeapon = SimpleWeapon::melee(
    "deer ram",
    &["ram-d", "deer"],
    AbilityScoreType::Strength,
    Dice::new(1, 4),
    DamageType::Bludgeoning,
);

// ─── Eagle ───────────────────────────────────────────────────────────

/// Eagle Talons — DEX-based 1d4+DEX slashing melee. RAW: "Talons. Melee
/// Attack Roll: +4, reach 5 feet. Hit: 4 (1d4 + 2) Slashing damage."
/// The bigger cousin of `HAWK_TALONS` — same limb, a real die behind it.
pub static EAGLE_TALONS: SimpleWeapon = SimpleWeapon::melee(
    "eagle talons",
    &["talons-e", "eagle"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Slashing,
);

// ─── Octopus ─────────────────────────────────────────────────────────

/// Octopus Tentacles — flat 1 bludgeoning. RAW: "Tentacles. Melee
/// Attack Roll: +4, reach 5 ft. Hit: 1 Bludgeoning damage." DEX-based
/// on the roll; the octopus's Strength 4 is not what it hits with.
pub static OCTOPUS_TENTACLES: SimpleWeapon = SimpleWeapon::flat_melee(
    "octopus tentacles",
    &["tentacles-o", "octopus"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Bludgeoning,
);

// ─── Owl ─────────────────────────────────────────────────────────────

/// Owl Talons — flat 1 slashing. RAW: "Talons. Melee Attack Roll: +3,
/// reach 5 ft. Hit: 1 Slashing damage." The owl is a Flyby creature
/// with one hit point and a hundred and twenty feet of darkvision; the
/// talons are a formality.
pub static OWL_TALONS: SimpleWeapon = SimpleWeapon::flat_melee(
    "owl talons",
    &["talons-o", "owl"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Slashing,
);

// ─── Piranha ─────────────────────────────────────────────────────────

/// Piranha Bite — flat 1 piercing. RAW: "Bite. Melee Attack Roll: +5
/// (with Advantage if the target doesn't have all its Hit Points),
/// reach 5 ft. Hit: 1 Piercing damage."
///
/// The parenthetical is the shark family's **Blood Frenzy**, spelled
/// inline on the Hit line instead of as a trait, and it rides the same
/// `BLOOD_FRENZY_TAG` the sahuagin and the three sharks already carry
/// — one wounded swimmer turns every fish in the shoal into an
/// advantaged attacker, which is the entire reason a CR-0 fish with one
/// hit point is worth writing down.
pub static PIRANHA_BITE: SimpleWeapon = SimpleWeapon::flat_melee(
    "piranha bite",
    &["piranha", "pir-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Piercing,
);

// ─── Rhinoceros ──────────────────────────────────────────────────────

/// Rhinoceros Gore — STR-based 2d8+STR piercing melee. RAW: "Gore.
/// Melee Attack Roll: +7, reach 5 ft. Hit: 14 (2d8 + 5) Piercing
/// damage."
pub static RHINOCEROS_GORE: SimpleWeapon = SimpleWeapon::melee(
    "rhinoceros gore",
    &["gore-r", "rhino"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
);

/// Rhinoceros **Charge** (RAW): "If the target is a Large or smaller
/// creature and the rhinoceros moved 20+ feet straight toward it
/// immediately before the hit, the target takes an extra 9 (2d8)
/// Piercing damage and has the Prone condition."
///
/// The knockdown is a Strength save at the rhino's own derived DC
/// rather than RAW's automatic Prone, which is the convention every
/// other charge on the chassis follows. The size clause is
/// `max_target_size`: a rhinoceros can flatten anything up to Large and
/// bounces off a giant.
pub const RHINOCEROS_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("rhinoceros gore"),
    dice: Dice::new(2, 8),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "rhinoceros charge",
    knockdown_label: "rhinoceros charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
};

// ─── Scorpion ────────────────────────────────────────────────────────

/// Scorpion Sting — flat 1 piercing plus 1d6 poison. RAW: "Sting. Melee
/// Attack Roll: +2, reach 5 ft. Hit: 1 Piercing damage plus 3 (1d6)
/// Poison damage."
///
/// DEX-based (DEX 11 → +0 + PB 2 = the printed +2), which also zeroes
/// the damage modifier so the `1d1` piercing half lands as RAW's flat 1.
/// The venom is the whole stat block: a tiny scorpion's puncture is
/// nothing and its poison averages more than three times the puncture.
/// The engine's `WeaponWithRider` adds the rider on a hit with no save,
/// which is what SRD 5.2 prints here — the save-gated venom belongs to
/// the *giant* scorpion, and that one is already on the roster.
pub static SCORPION_STING: WeaponWithRider = WeaponWithRider::melee(
    "scorpion sting",
    &["sting-s", "scorpion"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Piercing,
    Dice::new(1, 6),
    DamageType::Poison,
    "scorpion venom",
);

// ─── Venomous Snake ──────────────────────────────────────────────────

/// Venomous Snake Bite — DEX-based 1d4+DEX piercing plus 1d6 poison.
/// RAW: "Bite. Melee Attack Roll: +4, reach 5 ft. Hit: 4 (1d4 + 2)
/// Piercing damage plus 3 (1d6) Poison damage." The small sibling of
/// `GIANT_VENOMOUS_SNAKE_BITE`; same venom die, a quarter of the frame.
pub static VENOMOUS_SNAKE_BITE: WeaponWithRider = WeaponWithRider::melee(
    "venomous snake bite",
    &["vs-bite", "snake-bite"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    Dice::new(1, 6),
    DamageType::Poison,
    "snake venom",
);

// ─── Flying Snake ────────────────────────────────────────────────────

/// Flying Snake Bite — flat 1 piercing plus 2d4 poison. RAW: "Bite.
/// Melee Attack Roll: +4, reach 5 ft. Hit: 1 Piercing damage plus 5
/// (2d4) Poison damage."
///
/// The venom is five times the bite, which is the flying snake in one
/// line: a Tiny monstrosity with a sixty-foot fly speed and Flyby whose
/// only job is to touch somebody once a turn and leave before anything
/// can swing back.
pub static FLYING_SNAKE_BITE: WeaponWithRider = WeaponWithRider::melee(
    "flying snake bite",
    &["fs-bite", "flying-snake"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 1),
    DamageType::Piercing,
    Dice::new(2, 4),
    DamageType::Poison,
    "flying snake venom",
);

// ─── Giant Elk ───────────────────────────────────────────────────────

/// Giant Elk Ram — STR-based 2d6+STR bludgeoning at reach 2 (10 ft),
/// with a flat 2d4 radiant rider. RAW: "Ram. Melee Attack Roll: +6,
/// reach 10 ft. Hit: 11 (2d6 + 4) Bludgeoning damage plus 5 (2d4)
/// Radiant damage."
///
/// The radiant half is the giant elk's Celestial type made mechanical —
/// this is not a large elk, it is a good creature that looks like one,
/// and the undead it is usually pointed at feel the difference.
pub static GIANT_ELK_RAM: WeaponWithRider = WeaponWithRider::reach_melee(
    "giant elk ram",
    &["ram-ge", "giant-elk"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
    2,
    Dice::new(2, 4),
    DamageType::Radiant,
    "celestial ram",
);

/// Giant Elk **Charge** (RAW): "If the target is a Huge or smaller
/// creature and the elk moved 20+ feet straight toward it immediately
/// before the hit, the target takes an extra 5 (2d4) Bludgeoning damage
/// and has the Prone condition."
pub const GIANT_ELK_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("giant elk ram"),
    dice: Dice::new(2, 4),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "giant elk charge",
    knockdown_label: "giant elk charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: None,
    max_target_size: Some(Size::Huge),
};

// ─── Giant Seahorse ──────────────────────────────────────────────────

/// Giant Seahorse Ram — STR-based 2d6+STR bludgeoning melee. RAW:
/// "Ram. Melee Attack Roll: +4, reach 5 ft. Hit: 9 (2d6 + 2)
/// Bludgeoning damage, or 11 (2d8 + 2) Bludgeoning damage if the
/// seahorse moved 20+ feet straight toward the target immediately
/// before the hit."
pub static GIANT_SEAHORSE_RAM: SimpleWeapon = SimpleWeapon::melee(
    "giant seahorse ram",
    &["ram-gs", "seahorse-ram"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Bludgeoning,
);

/// Giant Seahorse **Charge** — RAW's escalation clause, expressed as
/// the engine's charge rider.
///
/// RAW swaps 2d6 for 2d8 rather than adding a die, a difference of two
/// points of average, so the rider carries `1d4` (average 2.5) and no
/// knockdown. A die swap is what `SimpleWeapon::bloodied_dice` does for
/// the blood hawk, but that field asks about the *target* and this
/// clause asks how far the attacker ran — which is precisely the
/// question the charge chassis already answers, and re-answering it on
/// the weapon would put the run-distance check in two places.
pub const GIANT_SEAHORSE_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("giant seahorse ram"),
    dice: Dice::new(1, 4),
    damage_type: DamageType::Bludgeoning,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: false,
    label: "giant seahorse charge",
    knockdown_label: "giant seahorse charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: None,
    max_target_size: None,
};

// ─── Axe Beak ────────────────────────────────────────────────────────

/// Axe Beak Beak — STR-based 1d8+STR slashing melee. RAW: "Beak. Melee
/// Attack Roll: +4, reach 5 ft. Hit: 6 (1d8 + 2) Slashing damage." One
/// swing, fifty feet of speed, and nothing else: the axe beak is a
/// mount-shaped monstrosity whose stat block is a chase.
pub static AXE_BEAK_BEAK: SimpleWeapon = SimpleWeapon::melee(
    "axe beak beak",
    &["beak-ab", "axe-beak"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

// ─── Elephant ────────────────────────────────────────────────────────

/// Elephant Gore — STR-based 2d8+STR piercing melee. RAW: "Gore. Melee
/// Attack Roll: +8, reach 5 ft. Hit: 15 (2d8 + 6) Piercing damage."
pub static ELEPHANT_GORE: SimpleWeapon = SimpleWeapon::melee(
    "elephant gore",
    &["gore-e", "elephant"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
);

/// Elephant Multiattack — "The elephant makes two Gore attacks."
pub static ELEPHANT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "elephant multiattack",
    sub_attack: &ELEPHANT_GORE,
    count: 2,
});

/// The elephant's trample, without its Prone gate. RAW: "Trample
/// (Bonus Action). Dexterity Saving Throw: DC 16, one creature within 5
/// feet that has the Prone condition. Failure: 17 (2d10 + 6)
/// Bludgeoning damage. Success: Half damage."
///
/// Resolved as a bonus-action attack roll rather than as RAW's
/// Dexterity save, which is the same collapse `MAMMOTH_STOMP_SWING`
/// makes and for the same reason: the engine has no single-target
/// save-for-half melee chassis, and the two shapes agree closely on a
/// prone target — Prone already grants the attacker advantage, which is
/// roughly where a DC-16 save against a flattened creature lands.
///
/// Never put on a template directly; `ELEPHANT_TRAMPLE` is what the
/// elephant carries.
static ELEPHANT_TRAMPLE_STOMP: SimpleWeapon = SimpleWeapon {
    display_name: "elephant trample",
    aliases: &["trample", "stomp-e"],
    attack_ability: AbilityScoreType::Strength,
    damage_ability: Some(AbilityScoreType::Strength),
    damage_dice: Dice::new(2, 10),
    damage_type: DamageType::Bludgeoning,
    reach: MELEE_REACH,
    is_melee: true,
    requires_los: false,
    // RAW prints Trample as a Bonus Action, which is what makes the
    // elephant's turn "gore twice, then step on whatever fell over"
    // rather than a choice between the two.
    cost_resource: Resource::BonusAction,
    normal_range: None,
    requires_condition: None,
    min_effective_range: None,
    is_light: false,
    mastery: None,
    bloodied_dice: None,
};

/// The elephant's trample as the elephant has it: the stomp above,
/// admitted only against a target that already has the Prone condition
/// — RAW's "one creature within 5 feet that has the Prone condition".
///
/// Paired with `ELEPHANT_CHARGE::prone_follow_up`, so a gore that came
/// off a twenty-foot run knocks the target down and immediately earns
/// the bonus action that only a knocked-down target admits. That
/// sequence is the elephant's whole damage spike, and neither half of
/// it does anything alone.
pub static ELEPHANT_TRAMPLE: LazyLock<ProneOnlyAttack> = LazyLock::new(|| ProneOnlyAttack {
    display_name: "elephant trample",
    sub_attack: &ELEPHANT_TRAMPLE_STOMP,
});

/// Elephant **Charge** (RAW): "If the target is a Huge or smaller
/// creature and the elephant moved 20+ feet straight toward it
/// immediately before the hit, the target has the Prone condition."
///
/// Damage-free — RAW's clause is the knockdown and nothing else, which
/// is what `Dice::new(0, 0)` says here. The payoff is the follow-up,
/// not the rider.
pub const ELEPHANT_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("elephant gore"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "elephant charge",
    knockdown_label: "elephant charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("elephant trample"),
    max_target_size: Some(Size::Huge),
};

// ─── Hippopotamus ────────────────────────────────────────────────────

/// Hippopotamus Bite — STR-based 2d10+STR piercing melee. RAW: "Bite.
/// Melee Attack Roll: +7, reach 5 ft. Hit: 16 (2d10 + 5) Piercing
/// damage." Two of these a turn is thirty-two points of average damage
/// off a CR-4 stat block with no rider and no gate — the hippo is the
/// roster's plainest heavy hitter, and the reason people who know
/// rivers are afraid of them.
pub static HIPPOPOTAMUS_BITE: SimpleWeapon = SimpleWeapon::melee(
    "hippopotamus bite",
    &["hippo-bite", "hippo"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Piercing,
);

/// Hippopotamus Multiattack — "The hippopotamus makes two Bite attacks."
pub static HIPPOPOTAMUS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "hippopotamus multiattack",
    sub_attack: &HIPPOPOTAMUS_BITE,
    count: 2,
});

// ─── Allosaurus ──────────────────────────────────────────────────────

/// Allosaurus Bite — STR-based 2d10+STR piercing melee. RAW: "Bite.
/// Melee Attack Roll: +6, reach 5 ft. Hit: 15 (2d10 + 4) Piercing
/// damage." The pounce's payoff limb rather than its trigger.
pub static ALLOSAURUS_BITE: SimpleWeapon = SimpleWeapon::melee(
    "allosaurus bite",
    &["allo-bite", "allosaurus"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Piercing,
);

/// Allosaurus Claws — STR-based 1d8+STR slashing melee. RAW: "Claws.
/// Melee Attack Roll: +6, reach 5 ft. Hit: 8 (1d8 + 4) Slashing
/// damage. If the target is a Large or smaller creature and the
/// allosaurus moved 30+ feet straight toward it immediately before the
/// hit, the target has the Prone condition, and the allosaurus can make
/// one Bite attack against it."
pub static ALLOSAURUS_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "allosaurus claws",
    &["allo-claws", "claws-a"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Slashing,
);

/// Allosaurus **Pounce** — the claws' second half, as a charge rider.
///
/// Thirty feet of run rather than the usual twenty, which is the one
/// number that separates this clause from every other pounce in the
/// bestiary — a sixty-foot speed makes the longer run cheap, and RAW
/// prices it accordingly. Damage-free: the whole payoff is the
/// knockdown and the free bite behind it.
pub const ALLOSAURUS_POUNCE: ChargeRider = ChargeRider {
    weapon: Some("allosaurus claws"),
    dice: Dice::new(0, 0),
    damage_type: DamageType::Slashing,
    run_tiles: charge_run_tiles(30),
    knocks_prone: true,
    label: "allosaurus pounce",
    knockdown_label: "allosaurus pounce knockdown",
    once_per_turn_tag: None,
    prone_follow_up: Some("allosaurus bite"),
    max_target_size: Some(Size::Large),
};

// ─── Ankylosaurus ────────────────────────────────────────────────────

/// Ankylosaurus Tail — STR-based 1d10+STR bludgeoning at reach 2
/// (10 ft), with a save-or-Prone rider. RAW: "Tail. Melee Attack Roll:
/// +6, reach 10 ft. Hit: 9 (1d10 + 4) Bludgeoning damage. If the target
/// is a Huge or smaller creature, it has the Prone condition."
///
/// RAW's Prone is automatic and this is a Strength save against a fixed
/// DC 14 (`8 + PB 2 + STR 4`, the ankylosaurus's own derived number
/// written out), which is the convention every other knockdown on the
/// `WeaponWithSaveCondition` chassis follows. Two tail swings a turn,
/// each of which can put a target on the floor, is the whole stat
/// block: an ankylosaurus does not out-damage a tyrannosaurus, it
/// out-*positions* one.
pub static ANKYLOSAURUS_TAIL: WeaponWithSaveCondition = WeaponWithSaveCondition::reach_melee(
    "ankylosaurus tail",
    &["anky-tail", "tail-a"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Bludgeoning,
    AbilityScoreType::Strength,
    14,
    Condition::Prone,
    ConditionTimer::Permanent,
    "ankylosaurus knockdown",
    2,
)
.against_at_most(Size::Huge);

/// Ankylosaurus Multiattack — "The ankylosaurus makes two Tail attacks."
pub static ANKYLOSAURUS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "ankylosaurus multiattack",
    sub_attack: &ANKYLOSAURUS_TAIL,
    count: 2,
});

// ─── Archelon ────────────────────────────────────────────────────────

/// Archelon Bite — STR-based 3d6+STR piercing melee. RAW: "Bite. Melee
/// Attack Roll: +6, reach 5 ft. Hit: 14 (3d6 + 4) Piercing damage."
pub static ARCHELON_BITE: SimpleWeapon = SimpleWeapon::melee(
    "archelon bite",
    &["arch-bite", "archelon"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
);

/// Archelon Multiattack — "The archelon makes two Bite attacks."
pub static ARCHELON_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "archelon multiattack",
    sub_attack: &ARCHELON_BITE,
    count: 2,
});

// ═══════════════════════════════════════════════════════════════════
// The top of the SRD 5.2 extraplanar ladders
//
// Four stat blocks that had rungs waiting for them: the ice devil at
// CR 14 between the erinyes and the pit fiend, the planetar at CR 16
// between the deva and the solar, the sphinx of wonder at the very
// bottom of the celestial shelf, and the gray ooze at the bottom of
// the ooze one.
// ═══════════════════════════════════════════════════════════════════

// ─── Sphinx of Wonder ────────────────────────────────────────────────

/// Sphinx of Wonder Rend — DEX-based 1d4+DEX slashing plus a flat 2d6
/// radiant rider. RAW: "Rend. Melee Attack Roll: +5, reach 5 ft. Hit: 5
/// (1d4 + 3) Slashing damage plus 7 (2d6) Radiant damage."
///
/// The radiant half is more than twice the physical half, which is the
/// whole joke of the stat block: a house-cat-sized celestial whose
/// scratch is mostly holy fire.
pub static SPHINX_OF_WONDER_REND: WeaponWithRider = WeaponWithRider::melee(
    "sphinx rend",
    &["rend-sw", "sphinx-rend"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Slashing,
    Dice::new(2, 6),
    DamageType::Radiant,
    "celestial rend",
);

// ─── Gray Ooze ───────────────────────────────────────────────────────

/// Gray Ooze Pseudopod — STR-based 2d8+STR acid melee. RAW:
/// "Pseudopod. Melee Attack Roll: +3, reach 5 ft. Hit: 10 (2d8 + 1)
/// Acid damage."
///
/// RAW's second sentence — *"Nonmagical armor worn by the target takes
/// a −1 penalty to the AC it offers"* — is not modeled, for the reason
/// the rust monster's corrosive sludge already records one file over:
/// the engine has no equipment-degradation lane, and an AC penalty that
/// accumulates across a fight and is cleared by a spell nothing casts
/// in combat is two systems rather than a rider. Ten average acid on a
/// CR-½ frame is a real threat without it; the corrosion is what makes
/// a gray ooze a *problem* rather than a fight, and that half of the
/// creature lives outside an encounter's scope anyway.
pub static GRAY_OOZE_PSEUDOPOD: SimpleWeapon = SimpleWeapon::melee(
    "gray ooze pseudopod",
    &["pseudopod-g", "gray-ooze"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Acid,
);

// ─── Ice Devil ───────────────────────────────────────────────────────

/// Ice Devil Ice Spear — STR-based 2d8+STR piercing plus a flat 3d6
/// cold rider. RAW: "Ice Spear. Melee or Ranged Attack Roll: +10, reach
/// 5 ft. or range 30/120 ft. Hit: 14 (2d8 + 5) Piercing damage plus 10
/// (3d6) Cold damage."
///
/// Modeled as the melee half only. The spear's ranged mode returns to
/// the devil's hand on hit or miss, so RAW's two modes are one weapon
/// used at two distances rather than two weapons — and the engine's
/// `SimpleWeapon`-family chassis carries a single reach band. A devil
/// that must close is a materially different creature from one that can
/// stand off at 120 feet, and the melee reading is the one that keeps
/// the CR-14 damage profile honest; the alternative reading would have
/// given a boss with blindsight 120 an unanswerable kite.
///
/// RAW's third sentence — the hit costing the target its Bonus Action
/// and Reaction, ten feet of Speed, and the ability to both move *and*
/// act — is not modeled. It is four simultaneous debuffs on one rider,
/// three of which have no condition in this engine that means only that
/// one thing, and approximating it with the nearest single condition
/// would have been either far too little (Slowed's speed clause alone)
/// or far too much.
pub static ICE_DEVIL_SPEAR: WeaponWithRider = WeaponWithRider::melee(
    "ice spear",
    &["spear-id", "ice-spear"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
    Dice::new(3, 6),
    DamageType::Cold,
    "ice spear frost",
);

/// Ice Devil Tail — STR-based 3d6+STR bludgeoning at reach 2 (10 ft)
/// plus a flat 4d8 cold rider. RAW: "Tail. Melee Attack Roll: +10,
/// reach 10 ft. Hit: 15 (3d6 + 5) Bludgeoning damage plus 18 (4d8)
/// Cold damage."
///
/// The single hardest limb on the stat block — thirty-three average
/// against the spear's twenty-four — and the reason the multiattack
/// below spends one of its three swings on it.
pub static ICE_DEVIL_TAIL: WeaponWithRider = WeaponWithRider::reach_melee(
    "ice devil tail",
    &["tail-id", "devil-tail"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Bludgeoning,
    2,
    Dice::new(4, 8),
    DamageType::Cold,
    "ice devil frost",
);

/// Ice Devil Multiattack — RAW: "The devil makes three Ice Spear
/// attacks. It can replace one attack with a Tail attack."
///
/// Resolved as two spears and the tail, which is the substitution taken
/// rather than declined. RAW leaves the choice to the devil and the tail
/// is strictly the better swing by nine average damage, so a devil that
/// never took it would be one the engine had quietly made worse than the
/// book; the heterogeneous `CompoundAttack` chassis is what lets the
/// choice be made once, in data, instead of every turn in the AI.
pub static ICE_DEVIL_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "ice devil multiattack",
    parts: vec![(&ICE_DEVIL_SPEAR, 2), (&ICE_DEVIL_TAIL, 1)],
});

// ─── Planetar ────────────────────────────────────────────────────────

/// Planetar Radiant Sword — STR-based 2d6+STR slashing at reach 2
/// (10 ft) plus a flat 4d8 radiant rider. RAW: "Radiant Sword. Melee
/// Attack Roll: +12, reach 10 ft. Hit: 14 (2d6 + 7) Slashing damage
/// plus 18 (4d8) Radiant damage."
///
/// Three of these per Action is ninety-six average damage, which is
/// what a CR-16 angel is for.
pub static PLANETAR_SWORD: WeaponWithRider = WeaponWithRider::reach_melee(
    "radiant sword",
    &["sword-p", "planetar-sword"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    2,
    Dice::new(4, 8),
    DamageType::Radiant,
    "radiant sword flare",
);

/// Planetar Multiattack — "The planetar makes three Radiant Sword
/// attacks."
///
/// RAW's other half — *"or uses Holy Burst twice"* — is the choice
/// between the two lanes rather than a third lane, and the engine's
/// action list already gives the AI that choice: `PLANETAR_HOLY_BURST`
/// sits beside this wrapper and the picker ranks the two. What is not
/// modeled is the *doubling* — a planetar that commits to the burst
/// gets one rather than two, because the multiattack chassis wraps one
/// sub-action and a second burst centred somewhere else is a second
/// targeting decision, not a repeat.
pub static PLANETAR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "planetar multiattack",
    sub_attack: &PLANETAR_SWORD,
    count: 3,
});

/// Planetar Holy Burst — RAW: "Dexterity Saving Throw: DC 20, each
/// enemy in a 20-foot-radius Sphere centered on a point the planetar
/// can see within 120 feet. Failure: 24 (7d6) Radiant damage. Success:
/// Half damage."
///
/// The roster's first `PointBurstSaveDamage`, and the stat block that
/// motivated the chassis: an at-will point burst had nowhere to live
/// while the engine's only one was gated on a recharge. `enemies_only`
/// is RAW's "each enemy" — an angel's holy fire is the one area effect
/// in the bestiary that knows whose side it is on.
///
/// The two numbers convert on two different scales, which is a trap
/// worth naming because both are already established here and neither
/// is wrong. **Range** is tiles, at 2.5 ft each, so RAW's 120 feet is
/// `tiles_from_feet(120)` = 48 — the same scale every spell's range
/// line uses (Fireball's 150 ft is 60). **Radius** is the footprint
/// *gap* the burst helpers measure, and the roster spells it in 5-ft
/// grid squares: Fireball's 20-foot sphere is radius 4 and Circle of
/// Death's 30-foot one is radius 6. Holy Burst is a 20-foot sphere, so
/// it is radius 4 and not the 8 a naive `tiles_from_feet` would give —
/// which would have made an angel's burst twice the width of a
/// fireball and larger than an ancient dragon's breath.
pub static PLANETAR_HOLY_BURST: PointBurstSaveDamage = PointBurstSaveDamage {
    display_name: "holy burst",
    aliases: &["burst-p", "holy"],
    damage_dice: Dice::new(7, 6),
    damage_type: DamageType::Radiant,
    save_ability: AbilityScoreType::Dexterity,
    dc: 20,
    radius: 4,
    range: tiles_from_feet(120) as isize,
    enemies_only: true,
};

// ─── The officers and the minions ────────────────────────────────────
//
// SRD 5.2 gives most of its rank-and-file humanoids a rung above and a
// rung below the one the bestiary already carried. What follows is the
// weapons those rungs swing; the stat blocks are in
// `actors::creatures`, filed beside the creature they outrank.

/// Bugbear Stalker Morningstar — STR-based 2d8+STR piercing at reach 2.
/// RAW: "Morningstar. Melee Attack Roll: +5 (with Advantage if the
/// target is Grappled by the bugbear), reach 10 ft. Hit: 12 (2d8 + 3)
/// Piercing damage."
///
/// A separate static from `BUGBEAR_MORNINGSTAR` rather than a bigger
/// die on it, because the two are different weapons in every way that
/// matters: the CR-1 bugbear's carries a Surprise Attack rider and
/// swings once, and this one carries none and swings twice. Sharing a
/// name would have meant one of the two stat blocks quietly getting the
/// other's clause.
///
/// The grapple-advantage clause is dropped. It reads on the sheet as a
/// bonus for a bugbear that has used Quick Grapple, and Quick Grapple
/// is a bonus-action save-or-grapple the engine would need a new
/// chassis for; a to-hit bonus conditioned on a grapple that never
/// happens is a clause that would never fire.
pub static BUGBEAR_STALKER_MORNINGSTAR: SimpleWeapon = SimpleWeapon::reach_melee(
    "stalker morningstar",
    &["sm", "stalker-morningstar"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Piercing,
    2,
);

/// Bugbear Stalker Javelin — STR-based 3d6+STR piercing, thrown. RAW:
/// "Javelin. Melee or Ranged Attack Roll: +5, reach 10 ft. or range
/// 30/120 ft. Hit: 13 (3d6 + 3) Piercing damage."
///
/// Declared as the ranged half only, the same compression every
/// dual-mode weapon in this file gets. Three d6 rather than the
/// armoury javelin's one: the stalker throws these the way the CR-1
/// bugbear swings its club, and the die is the difference between a
/// creature with a ranged option and one with a ranged threat.
pub static BUGBEAR_STALKER_JAVELIN: SimpleWeapon = SimpleWeapon::ranged(
    "stalker javelin",
    &["sj", "stalker-javelin"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
    48,
    12,
);

/// Bugbear Stalker Multiattack — two morningstar swings per Action.
/// RAW: "The bugbear makes two Javelin or Morningstar attacks."
///
/// Pinned to the morningstar, with the javelin left on the action list
/// as its own entry — the shape every "X or Y in any combination"
/// multiattack in this file takes, because the chassis chains one
/// sub-attack and the AI picks between the wrapper and the loose
/// weapon by what is in range.
pub static BUGBEAR_STALKER_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "bugbear stalker multiattack",
    sub_attack: &BUGBEAR_STALKER_MORNINGSTAR,
    count: 2,
});

/// Hobgoblin Captain Greatsword — STR-based 2d6+STR slashing with a
/// flat 1d6 poison rider. RAW: "Greatsword. Melee Attack Roll: +4,
/// reach 5 ft. Hit: 9 (2d6 + 2) Slashing damage plus 3 (1d6) Poison
/// damage."
///
/// The poison is the captain's whole identity in 5.2 and is new to the
/// printing: the 2014 hobgoblin hit for plain steel. Rides
/// `WeaponWithRider` as an unconditional second damage type, which is
/// what the stat block says — no save, no condition, just a blade that
/// has been dipped.
pub static HOBGOBLIN_CAPTAIN_GREATSWORD: WeaponWithRider = WeaponWithRider::melee(
    "captain greatsword",
    &["cgs", "captain-greatsword"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
    Dice::new(1, 6),
    DamageType::Poison,
    "envenomed blade",
);

/// Hobgoblin Captain Longbow — DEX-based 1d8+DEX piercing with a flat
/// 2d4 poison rider. RAW: "Longbow. Ranged Attack Roll: +4, range
/// 150/600 ft. Hit: 6 (1d8 + 2) Piercing damage plus 5 (2d4) Poison
/// damage."
///
/// Note which die is bigger. The captain's arrows carry *more* venom
/// than its sword does, which is the stat block telling you what it
/// would rather be doing: a hobgoblin captain that has been allowed to
/// stand at range is a hobgoblin captain that is winning.
pub static HOBGOBLIN_CAPTAIN_LONGBOW: WeaponWithRider = WeaponWithRider::ranged(
    "captain longbow",
    &["clb", "captain-longbow"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Piercing,
    Dice::new(2, 4),
    DamageType::Poison,
    "envenomed arrow",
    20,
    12,
);

/// Hobgoblin Captain Multiattack — two attacks per Action. RAW: "The
/// hobgoblin makes two attacks, using Greatsword or Longbow in any
/// combination."
pub static HOBGOBLIN_CAPTAIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "hobgoblin captain multiattack",
    sub_attack: &HOBGOBLIN_CAPTAIN_GREATSWORD,
    count: 2,
});

/// Guard Captain Longsword — STR-based 2d10+STR slashing. RAW:
/// "Longsword. Melee Attack Roll: +6, reach 5 ft. Hit: 15 (2d10 + 4)
/// Slashing damage."
///
/// Two d10 where the armoury longsword rolls one d8, which is 5.2's way
/// of pricing an officer: the captain is not carrying a better sword
/// than the guards outside its door, it is a better swordsman, and a
/// stat block has one place to put that.
pub static GUARD_CAPTAIN_LONGSWORD: SimpleWeapon = SimpleWeapon::melee(
    "captain longsword",
    &["cls", "captain-longsword"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Slashing,
);

/// Guard Captain Javelin — STR-based 3d6+STR piercing, thrown. RAW:
/// "Javelin. Melee or Ranged Attack Roll: +6, reach 5 ft. or range
/// 30/120 ft. Hit: 14 (3d6 + 4) Piercing damage."
pub static GUARD_CAPTAIN_JAVELIN: SimpleWeapon = SimpleWeapon::ranged(
    "captain javelin",
    &["cj", "captain-javelin"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Piercing,
    48,
    12,
);

/// Guard Captain Multiattack — two attacks per Action. RAW: "The guard
/// makes two attacks, using Javelin or Longsword in any combination."
pub static GUARD_CAPTAIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "guard captain multiattack",
    sub_attack: &GUARD_CAPTAIN_LONGSWORD,
    count: 2,
});

/// Tough Boss Warhammer — STR-based 2d8+STR bludgeoning. RAW:
/// "Warhammer. Melee Attack Roll: +5, reach 5 ft. Hit: 12 (2d8 + 3)
/// Bludgeoning damage. If the target is a Large or smaller creature,
/// the tough pushes the target up to 10 feet straight away from
/// itself."
///
/// The push ships through the weapon's **Push** mastery property rather
/// than as a bespoke rider — the armoury warhammer already carries it,
/// and `engine::mastery` is where "this weapon shoves what it hits"
/// lives. RAW's ten feet is the mastery's five, which is the one place
/// the translation loses something; the alternative is a second shove
/// implementation that only the boss can reach.
pub static TOUGH_BOSS_WARHAMMER: SimpleWeapon = SimpleWeapon::melee(
    "boss warhammer",
    &["bwh", "boss-warhammer"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
)
.mastery(WeaponMastery::Push);

/// Tough Boss Heavy Crossbow — DEX-based 2d10+DEX piercing. RAW:
/// "Heavy Crossbow. Ranged Attack Roll: +4, range 100/400 ft. Hit: 13
/// (2d10 + 2) Piercing damage."
pub static TOUGH_BOSS_CROSSBOW: SimpleWeapon = SimpleWeapon::ranged(
    "boss crossbow",
    &["bcb", "boss-crossbow"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 10),
    DamageType::Piercing,
    40,
    16,
);

/// Tough Boss Multiattack — two attacks per Action. RAW: "The tough
/// makes two attacks, using Warhammer or Heavy Crossbow in any
/// combination."
pub static TOUGH_BOSS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "tough boss multiattack",
    sub_attack: &TOUGH_BOSS_WARHAMMER,
    count: 2,
});

// ─── Ogre Zombie ─────────────────────────────────────────────────────

/// Ogre Zombie Slam — STR-based 2d8+STR bludgeoning. RAW: "Slam. Melee
/// Attack Roll: +6, reach 5 ft. Hit: 13 (2d8 + 4) Bludgeoning damage."
///
/// One swing where the ordinary zombie takes two, and harder than
/// either. That trade is the stat block: an ogre zombie hits like the
/// ogre it used to be and swings like the zombie it is now.
pub static OGRE_ZOMBIE_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "ogre zombie slam",
    &["oz-slam", "ogre-slam"],
    AbilityScoreType::Strength,
    Dice::new(2, 8),
    DamageType::Bludgeoning,
);

// ─── Minotaur Skeleton ───────────────────────────────────────────────

/// Minotaur Skeleton Gore — STR-based 2d6+STR piercing, and the horn
/// half of the charge below. RAW: "Gore. Melee Attack Roll: +6, reach 5
/// ft. Hit: 11 (2d6 + 4) Piercing damage. If the target is a Large or
/// smaller creature and the skeleton moved 20+ feet straight toward it
/// immediately before the hit, the target takes an extra 9 (2d8)
/// Piercing damage and has the Prone condition."
///
/// A plain weapon rather than a bespoke Action, because the whole
/// conditional clause is `MINOTAUR_SKELETON_CHARGE` — the engine has a
/// charge lane and this is exactly the shape it takes.
pub static MINOTAUR_SKELETON_GORE: SimpleWeapon = SimpleWeapon::melee(
    "skeleton gore",
    &["sk-gore", "skeleton-gore"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Piercing,
);

/// Minotaur Skeleton Slam — STR-based 2d10+STR bludgeoning. RAW: "Slam.
/// Melee Attack Roll: +6, reach 5 ft. Hit: 15 (2d10 + 4) Bludgeoning
/// damage."
///
/// The heavier of the skeleton's two swings, and the one it uses
/// standing still: 2d10 flat beats 2d6 every round the skeleton has
/// not run. The gore only overtakes it on a charge, which is the choice
/// the pair is there to give the picker.
pub static MINOTAUR_SKELETON_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "skeleton slam",
    &["sk-slam", "skeleton-slam"],
    AbilityScoreType::Strength,
    Dice::new(2, 10),
    DamageType::Bludgeoning,
);

/// Minotaur Skeleton **Charge** — RAW's twenty feet of run for an extra
/// 2d8 piercing and a knockdown.
///
/// Twenty feet is `CHARGE_RUN_TILES`, the bar nearly every charge clause
/// in the bestiary asks for; the living minotaur's own Gore wants ten,
/// which is one of the two places the ladder disagrees with itself and
/// is RAW both times.
///
/// RAW's "Large or smaller" size gate rides `max_target_size`, so the
/// Huge creatures the clause was never meant to reach stay standing.
pub const MINOTAUR_SKELETON_CHARGE: ChargeRider = ChargeRider {
    weapon: Some("skeleton gore"),
    dice: Dice::new(2, 8),
    damage_type: DamageType::Piercing,
    run_tiles: CHARGE_RUN_TILES,
    knocks_prone: true,
    label: "skeleton charge",
    knockdown_label: "skeleton charge knockdown",
    once_per_turn_tag: None,
    prone_follow_up: None,
    max_target_size: Some(Size::Large),
};

// ─── Animated Flying Sword ───────────────────────────────────────────

/// Animated Flying Sword Slash — DEX-based 1d8+DEX slashing. RAW:
/// "Slash. Melee Attack Roll: +4, reach 5 ft. Hit: 6 (1d8 + 2) Slashing
/// damage."
///
/// DEX rather than STR, which is unusual for a melee swing and is what
/// the stat block says: the sword has no arm behind it, so what decides
/// whether it lands is how fast it moves.
pub static FLYING_SWORD_SLASH: SimpleWeapon = SimpleWeapon::melee(
    "flying sword slash",
    &["fs-slash", "sword-slash"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 8),
    DamageType::Slashing,
);

// ─── Swarm of Ravens ─────────────────────────────────────────────────

/// Swarm of Ravens Beaks — DEX-based 1d6+DEX piercing. RAW: "Beaks.
/// Melee Attack Roll: +4, reach 5 ft. Hit: 5 (1d6 + 2) Piercing damage,
/// or 2 (1d4) Piercing damage if the swarm is Bloodied."
///
/// The half-strength-when-thinned clause is carried, and not here: it
/// is the same sentence on every swarm in the book, so it lives once on
/// `ActorInstance::is_thinned_swarm` and is read at the damage
/// chokepoint rather than as a second die on each of seven attack
/// lines. Distinct from `SimpleWeapon::bloodied_dice`, which swaps a die
/// when the *target* is bloodied — the blood hawk's clause, and the
/// other creature entirely.
pub static SWARM_OF_RAVENS_BEAKS: SimpleWeapon = SimpleWeapon::melee(
    "raven beaks",
    &["rb", "beaks"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Piercing,
);

// ─── Pirates ─────────────────────────────────────────────────────────

/// Pirate Multiattack — two Dagger attacks.
///
/// RAW: "The pirate makes two Dagger attacks. It can replace one attack
/// with a use of Enthralling Panache." The replacement clause is not a
/// third entry on the action list — it is the reason the panache is its
/// own Action-priced entry rather than a rider, so the pirate's turn is
/// a genuine choice between two swings and one swing's worth of charm.
/// The AI reads both off the list and prices them the way it prices
/// every other damage-versus-lockdown pick.
///
/// The dagger itself is the shared `DAGGER` static, which is already
/// exactly what the stat block prints: DEX, 1d4, piercing, finesse.
/// RAW's "+5, reach 5 ft. or range 20/60 ft." is DEX 16 plus a +2
/// proficiency bonus, and the throw is `THROWN_DAGGER` at the same
/// 8/24 tiles every other short throw uses.
pub static PIRATE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double dagger",
    sub_attack: &DAGGER,
    count: 2,
});

/// Pirate **Enthralling Panache** — WIS save DC 12 within 30 ft, or the
/// target is Charmed until the start of the pirate's next turn.
///
/// One round, which is the shortest charm in the bestiary and the whole
/// character of the stat block. A vampire's gaze takes somebody out of
/// the fight for a minute; a pirate's swagger takes them out for their
/// next turn, and then they are back and annoyed. It is a tempo tax
/// rather than a lockout, which is what a CR-1 charm should be, and it
/// is why the pirate would rather spend the panache on the party's
/// heaviest hitter than on whoever is closest.
pub static ENTHRALLING_PANACHE: SaveOrCharm = SaveOrCharm::action(
    "enthralling panache",
    &["ep", "panache"],
    // RAW 30 ft = 12 tiles.
    12,
    12,
    // "Until the start of the pirate's next turn" — one round.
    ConditionTimer::Rounds(1),
    "enthralling panache",
);

/// Pirate Captain Rapier — DEX-based 2d8+DEX piercing melee that vexes.
///
/// RAW: "Hit: 13 (2d8 + 4) Piercing damage, and the pirate has Advantage
/// on the next attack roll it makes before the end of this turn." That
/// clause is the Vex mastery, word for word, and the engine already has
/// it — so the captain's signature "first cut sets up the second" reads
/// off the shared mastery lane rather than a bespoke rider, and it
/// chains inside the three-attack Multiattack the way RAW intends.
pub static PIRATE_CAPTAIN_RAPIER: SimpleWeapon = SimpleWeapon::melee(
    "captain's rapier",
    &["cr", "rapier"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 8),
    DamageType::Piercing,
)
.mastery(WeaponMastery::Vex);

/// Pirate Captain Pistol — DEX-based 2d10+DEX piercing at range.
///
/// RAW 30/90 ft. The bestiary's only firearm, and it is priced like
/// one: the biggest single die on any CR-6 ranged attack, on a stat
/// block that would rather be in melee. Compressed to 12 tiles normal
/// and 20 long, the same band the drow's hand crossbow reads, so the
/// captain's choice between rapier and pistol is a real one on a board
/// two dozen tiles wide rather than a formality.
pub static PIRATE_CAPTAIN_PISTOL: SimpleWeapon = SimpleWeapon::ranged(
    "pistol",
    &["pist"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 10),
    DamageType::Piercing,
    20,
    12,
);

/// Pirate Captain Multiattack — three attacks per Action.
///
/// RAW: "three attacks, using Rapier or Pistol in any combination."
/// Three rapiers, because the free choice is one the action list
/// already offers: the pistol is its own entry, and a captain that
/// wants to shoot takes it. What a mixed chassis would buy is the
/// ability to shoot twice and stab once in the same Action, which is
/// worth less than it sounds — the pistol out-damages the rapier only
/// at range the captain is trying to close — and would cost the AI a
/// combinatorial pick it has no way to price.
///
/// Three swings with Vex on each is the point: the first sets up the
/// second, the second sets up the third, and a captain that connects
/// once is very likely to connect three times.
pub static PIRATE_CAPTAIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "triple rapier",
    sub_attack: &PIRATE_CAPTAIN_RAPIER,
    count: 3,
});

/// Pirate Captain **Captain's Charm** — WIS save DC 14 within 30 ft, or
/// the target is Charmed until the start of the captain's next turn.
///
/// A **Bonus Action**, which is the whole difference between this and
/// the Pirate's panache and is worth more than the two points of DC.
/// The captain charms *and* takes three swings in the same turn: the
/// charm costs it nothing it was going to spend, so there is never a
/// turn where charming is the wrong call. That is a genuinely nastier
/// stat block than one with a stronger charm that cost an Action.
pub static CAPTAINS_CHARM: SaveOrCharm = SaveOrCharm::action(
    "captain's charm",
    &["cc", "captains-charm"],
    // RAW 30 ft = 12 tiles.
    12,
    14,
    ConditionTimer::Rounds(1),
    "captain's charm",
)
.as_bonus_action();

// ─── Vampire Familiar ────────────────────────────────────────────────

/// The familiar's dagger, as a profile — see `DAGGER_PROFILE` for why
/// the swing and the throw come from one const rather than two
/// literals. Here the shared fact is the necrotic rider, which is
/// larger than the weapon damage it rides on and is the entire reason
/// the stat block is CR 3.
const UMBRAL_DAGGER_PROFILE: WeaponWithRider = WeaponWithRider::melee(
    "umbral dagger",
    &["ud", "umbral"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 4),
    DamageType::Piercing,
    Dice::new(3, 4),
    DamageType::Necrotic,
    "umbral chill",
);

/// Vampire Familiar Umbral Dagger — DEX 1d4 piercing plus a flat 3d4
/// necrotic, no save.
///
/// RAW: "Hit: 5 (1d4 + 3) Piercing damage plus 7 (3d4) Necrotic
/// damage." The rider is half again the swing and it is unconditional,
/// which is what a thrall's borrowed power looks like — the familiar
/// cannot fight, so its master lent it something that does not need to.
///
/// The clause that is **not** modeled is the tail: "If the target is
/// reduced to 0 Hit Points by this attack, the target becomes Stable
/// but has the Poisoned condition for 1 hour. While it has the
/// Poisoned condition, the target has the Paralyzed condition." That is
/// a kidnapping, not a kill — the vampire wants the body — and it needs
/// a side-effect lane that fires *on the downing blow specifically*,
/// which the damage pipeline does not currently hand to the weapon that
/// dealt it. Two conditions with a shared one-hour timer and an
/// auto-stabilise would be straightforward once that hook exists; the
/// hook is the work. Until then the familiar's dagger drops people the
/// ordinary way, which is strictly worse for the familiar and strictly
/// better for the party, so the omission errs in the safe direction.
pub static UMBRAL_DAGGER: WeaponWithRider = UMBRAL_DAGGER_PROFILE;

/// The umbral dagger in flight — RAW 20/60 ft, which is 8/24 tiles.
///
/// The same 8/24 band every other short throw on the roster reads, and
/// the same weapon: ability, die, damage type and the necrotic rider
/// all come from the profile, so the throw cannot quietly become a
/// mundane dagger.
pub static THROWN_UMBRAL_DAGGER: WeaponWithRider = UMBRAL_DAGGER_PROFILE.thrown(
    "thrown umbral dagger",
    &["tud", "hurl umbral"],
    THROWN_SHORT_NORMAL,
    THROWN_SHORT_LONG,
);

/// Vampire Familiar Multiattack — two Umbral Dagger attacks.
///
/// Fourteen necrotic before the piercing is counted, which is most of
/// what a CR-3 stat block is allowed and all of what this one has.
pub static VAMPIRE_FAMILIAR_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double umbral dagger",
    sub_attack: &UMBRAL_DAGGER,
    count: 2,
});

// ─── Guardian Naga ───────────────────────────────────────────────────

/// Guardian Naga Bite — STR 2d12 piercing at reach 2, plus a flat 4d10
/// poison rider.
///
/// RAW: "Hit: 17 (2d12 + 4) Piercing damage plus 22 (4d10) Poison
/// damage." The rider is larger than the bite, which is the shape of
/// every naga in the book and the reason a CR-10 stat block with two
/// attacks is a serious fight: forty-odd damage a round lands whether
/// or not anybody fails a save, because the venom asks for none.
pub static GUARDIAN_NAGA_BITE: WeaponWithRider = WeaponWithRider::reach_melee(
    "guardian naga bite",
    &["gn-bite", "naga-bite"],
    AbilityScoreType::Strength,
    Dice::new(2, 12),
    DamageType::Piercing,
    2,
    Dice::new(4, 10),
    DamageType::Poison,
    "naga venom",
);

/// Guardian Naga Multiattack — two bites.
///
/// RAW adds "It can replace any attack with a use of Poisonous
/// Spittle", which is the same clause the pirate's Multiattack carries
/// and is handled the same way: the spittle is its own Action-priced
/// entry on the list, so the AI weighs it against the bites rather than
/// being handed a combinatorial choice it cannot price.
pub static GUARDIAN_NAGA_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double naga bite",
    sub_attack: &GUARDIAN_NAGA_BITE,
    count: 2,
});

/// Guardian Naga **Poisonous Spittle** — CON save DC 16 at 60 ft: 7d8
/// poison, half on a success, and Blinded until the naga's next turn on
/// a failure.
///
/// The naga's answer to a party that stays out of reach, and the reason
/// it is a guardian rather than an ambusher: it does not have to close.
/// Blinded off the same save is what makes the spittle worth an Action
/// against two bites — a blinded fighter is a fighter swinging at
/// disadvantage into a creature it cannot see.
pub static GUARDIAN_NAGA_SPITTLE: SingleTargetSaveDamage = SingleTargetSaveDamage::new(
    "poisonous spittle",
    &["spittle", "gn-spit"],
    // RAW 60 ft = 24 tiles.
    24,
    AbilityScoreType::Constitution,
    16,
    Dice::new(7, 8),
    DamageType::Poison,
)
.and_condition(Condition::Blinded, ConditionTimer::Rounds(1));

// ─── Sphinx of Lore ──────────────────────────────────────────────────

/// Sphinx of Lore Claw — STR 3d6 slashing at reach 1.
///
/// Three of these an Action, which is fourteen a swing and forty-two a
/// round before anything interesting happens. The sphinx is a riddler
/// in the fiction and a straightforward beating in the initiative
/// order; the roar is what makes it a puzzle.
pub static SPHINX_OF_LORE_CLAW: SimpleWeapon = SimpleWeapon::melee(
    "sphinx claw",
    &["sl-claw", "lore-claw"],
    AbilityScoreType::Strength,
    Dice::new(3, 6),
    DamageType::Slashing,
);

/// Sphinx of Lore Multiattack — three claws.
pub static SPHINX_OF_LORE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "triple sphinx claw",
    sub_attack: &SPHINX_OF_LORE_CLAW,
    count: 3,
});

/// Sphinx of Lore **Mind-Rending Roar** (Recharge 5–6) — WIS save DC 16
/// against 10d6 psychic *and* Incapacitated until the start of the
/// sphinx's next turn, for every enemy that hears it.
///
/// The stat block that made the breath chassis grow a condition slot.
/// RAW hangs both halves off one saving throw, and off one *Wisdom*
/// saving throw at that, which is the save the front line is worst at:
/// a party that fails it takes thirty-five psychic each and then loses
/// its Actions, which on a CR-11 boss with three 14-damage claws is the
/// difference between a fight and a rout.
///
/// RAW's shape is a 300-foot Emanation — the whole dungeon, in effect —
/// scoped to *enemies*. The radius here is 24, which covers any board
/// the engine generates, and `enemies_only` is the half that is not
/// cosmetic: a sphinx that stunned its own guardians would be reading
/// the wrong word in its own stat block.
pub static MIND_RENDING_ROAR: BreathWeapon = BreathWeapon {
    display_name: "mind-rending roar",
    aliases: &["roar", "mrr"],
    damage: Some((Dice::new(10, 6), DamageType::Psychic)),
    save_ability: AbilityScoreType::Wisdom,
    dc: 16,
    // RAW's 300-foot Emanation, compressed to something a generated
    // board can contain. Everything hostile hears it.
    //
    // A Burst rather than one of the two projected shapes, and it is
    // the one breath on the roster where that is right: an Emanation
    // is centred on the creature and radiates in every direction, so
    // it is the *sphere* the burst has always modelled. The sphinx
    // aims it at its own tile.
    shape: AreaShape::Burst { radius: 24 },
    recharge_key: "breath_weapon",
    condition: Some((Condition::Incapacitated, ConditionTimer::Rounds(1))),
    enemies_only: true,
};

// ─── Troll Limb ──────────────────────────────────────────────────────

/// Troll Limb Rend — STR 2d4 slashing.
///
/// RAW: "+6, reach 5 ft. Hit: 9 (2d4 + 4)". STR 18 on a creature with
/// fourteen hit points, which is the joke: the arm hits as hard as the
/// troll it fell off and dies to a torch.
pub static TROLL_LIMB_REND: SimpleWeapon = SimpleWeapon::melee(
    "limb rend",
    &["tl-rend", "rend"],
    AbilityScoreType::Strength,
    Dice::new(2, 4),
    DamageType::Slashing,
);

// ─── Swarm of Crawling Claws ─────────────────────────────────────────

/// Swarm of Grasping Hands — DEX 4d8 necrotic that also knocks the
/// target down, no save.
///
/// RAW: "Hit: 20 (4d8 + 2) Necrotic damage… If the target is a Medium
/// or smaller creature, it has the Prone condition." Twenty necrotic a
/// round with a free trip attached is a great deal for CR 3, and the
/// price is that it is the swarm's only action — the claws have no
/// second lane, no ranged option, and nothing to do about a party that
/// stays six feet up.
///
/// Both clauses are carried. The **size gate** — Prone only against a
/// Medium or smaller target — is `max_target_size` on the chassis, so
/// the Large-and-up creatures RAW spares stay on their feet while still
/// taking the necrotic in full. The **bloodied clause**, the
/// half-damage-when-thinned line every swarm shares, is
/// `ActorInstance::is_thinned_swarm` read at the damage chokepoint;
/// see `crate::actors::creatures::swarms` for why it lives there rather
/// than as a second die on each of the seven attack lines.
pub static SWARM_OF_CRAWLING_CLAWS_HANDS: WeaponWithCondition = WeaponWithCondition::melee(
    "grasping hands",
    &["gh", "claws"],
    AbilityScoreType::Dexterity,
    Dice::new(4, 8),
    DamageType::Necrotic,
    &[Condition::Prone],
    ConditionTimer::Permanent,
    "grasping hands",
)
.against_at_most(Size::Medium);

// ─── Incubus ─────────────────────────────────────────────────────────

/// Incubus **Restless Touch** — CHA 3d6 psychic at reach 1.
///
/// A fiend that hits with its Charisma, which is the whole joke of the
/// stat block and also the arithmetic: RAW's "+7" against CHA 20 and a
/// +2 proficiency bonus is the Charisma modifier, not the Strength one
/// (STR 8, which would be +1). Fifteen psychic a swing on a creature
/// with a −1 to hit anything physically.
///
/// RAW's tail — "the target is cursed for 24 hours or until the incubus
/// dies. Until the curse ends, the target gains no benefit from
/// finishing Short Rests" — is not modeled. It is a between-fights
/// clause: the engine's short rest exists, but a curse that outlives
/// the encounter has nowhere to be recorded, and the party that took
/// the touch has already stopped rolling initiative by the time it
/// would matter.
pub static INCUBUS_RESTLESS_TOUCH: SimpleWeapon = SimpleWeapon::melee(
    "restless touch",
    &["rt", "touch"],
    AbilityScoreType::Charisma,
    Dice::new(3, 6),
    DamageType::Psychic,
);

/// Incubus Multiattack — two Restless Touches.
pub static INCUBUS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "double restless touch",
    sub_attack: &INCUBUS_RESTLESS_TOUCH,
    count: 2,
});

/// Incubus **Nightmare** (Recharge 6) — a bonus-action WIS save DC 15
/// at 60 ft that puts a wounded creature to sleep for an hour.
///
/// RAW: "Failure: If the target has 20 Hit Points or fewer, it has the
/// Unconscious condition for 1 hour, until it takes damage, or until a
/// creature within 5 feet of it takes an action to wake it."
///
/// The hit-point gate is the thing worth writing an impl for, and it is
/// why this is not a row on the save-or-condition chassis. Every other
/// save-or-suffer in the engine asks one question; this one asks two,
/// in order, and the second is about the target's current state rather
/// than its saving throw. It makes the ability a *finisher*: an incubus
/// cannot open with it, and a party that keeps everybody above twenty
/// hit points never sees it at all. Checked against the target's
/// current hit points at resolution rather than at targeting, which is
/// RAW's reading — the save comes first and the threshold second.
///
/// A bonus action, so the incubus does this *and* takes its two
/// touches, and Recharge 6 is what stops that being every turn.
///
/// The one-hour duration collapses to the engine's Permanent timer,
/// which is the honest translation: an hour is longer than any fight,
/// so what actually ends it is RAW's other two clauses — taking damage,
/// or an ally spending an action — and both of those the engine already
/// resolves for `Unconscious`.
pub struct IncubusNightmare {}

impl Action for IncubusNightmare {
    fn name(&self) -> &str {
        "nightmare"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nm", "bad-dream"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // RAW 60 ft = 24 tiles.
        Some(24)
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
    fn recharge_key(&self) -> Option<&'static str> {
        Some("nightmare")
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        crate::actions::action_template::bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !actor_has_recharge(encounter, caster_id, "nightmare") {
            return false;
        }
        // The threshold, asked here as well as at resolution, so the
        // AI's picker never spends the recharge on somebody it cannot
        // affect. RAW does not forbid the attempt — a failed save
        // against a healthy target simply does nothing — but an
        // incubus that burns its once-a-fight ability on the full-health
        // fighter is not playing its own stat block.
        let Some(target_id) = first_target_id(target_ids) else {
            return true;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.hitpoints() <= NIGHTMARE_HP_THRESHOLD)
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
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_recharge("nightmare");
        }
        let save = encounter.roll_save_vs_condition(
            target_id,
            AbilityScoreType::Wisdom,
            NIGHTMARE_DC,
            Condition::Unconscious,
        );
        if save.passed() {
            encounter.log("  nightmare: the target shakes it off".to_string());
            return Vec::new();
        }
        // The second question, and the order matters: RAW asks for the
        // save first and the hit points second, so a healthy creature
        // that fails still spends the incubus's recharge and takes
        // nothing.
        let hp = encounter
            .actors
            .get(&target_id)
            .map(|a| a.hitpoints())
            .unwrap_or(u32::MAX);
        if hp > NIGHTMARE_HP_THRESHOLD {
            encounter.log(format!(
                "  nightmare: the target is too strong to be dragged under ({} hit points)",
                hp
            ));
            return Vec::new();
        }
        encounter.log("  nightmare: the target sinks into it".to_string());
        crate::engine::side_effects::install_condition_with_link(
            Condition::Unconscious,
            target_id,
            caster_id,
            ConditionTimer::Permanent,
        )
    }
}

/// RAW's "20 Hit Points or fewer" — the gate that makes Nightmare a
/// finisher rather than an opener.
const NIGHTMARE_HP_THRESHOLD: u32 = 20;
/// The incubus's CHA-based spell save DC at CR 4.
const NIGHTMARE_DC: i32 = 15;

pub static INCUBUS_NIGHTMARE: LazyLock<IncubusNightmare> = LazyLock::new(|| IncubusNightmare {});
