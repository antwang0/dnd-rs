use crate::actors::actor_template::ActorInstance;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, RollMode};
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{
    ApplicableSideEffect, ApplyCondition, DealDamage, PushActor,
};
use crate::engine::types::{AbilityScoreType, DamageType};

/// Caster-side attack-bump source: a labeled scalar computed from the
/// caster's template flags / conditions. `u32` because every current
/// bump is a non-negative magnitude (flat damage bump or extra dice
/// count) — a future signed bump (a hypothetical debuff-driven -1 to
/// melee damage) would land as a sibling alias with an `i32` return
/// rather than widening this one. Both attack-side cohort tables
/// (`MELEE_CASTER_BUMPS` flat-damage and `CRIT_MELEE_EXTRA_DICE_SOURCES`
/// extra-dice) share this shape — factoring out the `fn` pointer type
/// keeps the row literal readable at a glance and drops the two "very
/// complex type" clippy warnings the raw signature triggered.
type AttackBumpFn = fn(&ActorInstance) -> u32;

/// Inputs to a single attack roll. Lets callers describe attacks without
/// repeating the d20 / crit / damage / log dance for every weapon and
/// damage cantrip.
pub struct AttackParams<'a> {
    pub caster_id: usize,
    pub target_id: usize,
    pub action_name: &'a str,
    /// Static attack-roll modifier (STR/DEX for weapons, INT/WIS/CHA for
    /// spells). Caster-side flat buffs (Bless, etc.) are folded in by
    /// `resolve_attack` itself — pass only the action's own bonus here.
    pub attack_bonus: i32,
    pub damage_dice: Dice,
    /// Static damage-roll modifier (STR/DEX/spellcasting mod). Added once
    /// per swing even on a crit.
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    /// `true` for melee weapons / touch spells, `false` for ranged
    /// attacks. Drives the prone-target advantage / disadvantage clause
    /// in `compute_attack_mode`.
    pub is_melee: bool,
    /// 5e long-range threshold (in tiles). Ranged attacks beyond this
    /// distance impose disadvantage. `None` means no long-range penalty
    /// (melee weapons, spells). Set to the weapon's "normal range"
    /// converted to tiles — attacks between `long_range` and `reach`
    /// roll with disadvantage per RAW.
    pub long_range: Option<isize>,
    /// `true` for spell-attack rolls (Fire Bolt, Eldritch Blast, Magic
    /// Stone, ...), `false` for weapon swings. Drives the 5e Tasha's
    /// Sorcerer Seeking Spell rider: on a miss, a spell-attack roll can
    /// burn a sorcery-point prime to reroll the d20. Weapon attacks must
    /// stay opted-out — the metamagic is RAW spell-attacks only. The
    /// flag also gates any future "this is a spell" sites that the
    /// engine grows (e.g. counterspell triggers, anti-magic field).
    pub is_spell: bool,
}

/// An extra clause a specific action layers onto its own swing, run at
/// the end of `resolve_attack_outcome` once the shared pipeline has
/// finished with the hit.
///
/// The three cohort tables above it — `ON_HIT_RIDERS`,
/// `ONCE_PER_TURN_WEAPON_DIE_RIDERS`, `ON_HIT_CONDITION_MARKS` — cover
/// riders that belong to the *attacker* and fire on any swing they make.
/// This covers the other kind: a rider that belongs to one action and
/// needs context those rows can't carry. Sneak Attack is the case that
/// motivated it — its die count scales with level and is negotiable
/// (Cunning Strike trades dice for effects), its eligibility depends on
/// the attack's `RollMode` and on who is standing next to the target,
/// and it emits side effects of its own. None of that fits a
/// `{tag, dice, target_gate}` row.
///
/// Riders return the extra damage they added so the caller's
/// `damage_dealt` figure stays honest, and push their own side effects
/// onto the swing's vec.
///
/// The alternative was for such an action to open-code the whole attack
/// roll, and the reason not to is what the Rogue's shortsword
/// demonstrated for as long as it did: an open-coded roll silently opts
/// out of cover, Bless and Bane, the caster's attack buffs, the
/// one-shot advantage riders (and their clearing), Multiattack Defense,
/// the reactive attack taxes, Sanctuary, the interception cohort, the
/// hit-this-turn mark, the reactive damage clamps, Hunter's Mark, Hex,
/// and every smite prime. Every one of those is a rule the action's
/// author never decided to skip.
pub trait ActionOnHitRider {
    /// Called on a landed swing, after the shared pipeline's own riders.
    /// `mode` is the roll mode the d20 was actually rolled at.
    fn apply(
        &self,
        encounter: &mut EncounterInstance,
        p: &AttackParams,
        mode: RollMode,
        is_crit: bool,
        effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32;
}

/// The rider every attack that doesn't have one uses. Adds nothing.
struct NoRider;

impl ActionOnHitRider for NoRider {
    fn apply(
        &self,
        _encounter: &mut EncounterInstance,
        _p: &AttackParams,
        _mode: RollMode,
        _is_crit: bool,
        _effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32 {
        0
    }
}

/// Caster-side flat melee-only damage bumps read at
/// `resolve_attack_outcome` after the base damage roll lands. Each
/// entry is a (label, amount_fn) tuple: the amount_fn reads the
/// caster's features/conditions and returns the flat bonus (`0`
/// skips the log line). All entries stack additively on holders that
/// carry multiple flags.
///
/// Entries:
///   - **Rage (+2)**: Barbarian's Raging condition.
///   - **Dueling (+2)**: Fighting Style flag; RAW's "one-handed and
///     no other weapon" clause collapses to "melee weapon attack" in
///     this engine.
///   - **Two-Weapon Fighting (+STR mod)**: Fighting Style flag; RAW's
///     "second attack" clause collapses to "every melee swing on the
///     holder" since we don't distinguish off-hand swings at the
///     action-list level.
///   - **Aura of Hate (+CHA mod, min +1)**: Oathbreaker Paladin lv7
///     subclass feature. RAW's ally-side aura on adjacent fiends /
///     undead is dropped since the engine doesn't tag those as an
///     aura-eligible cohort — the self-side +CHA damage is the
///     mechanical core.
///
/// A new melee-side bump (Ancestral Guardians retribution, Rage
/// tier-scaling to +3/+4, a Warlock's Lifedrinker) drops in here as
/// a new tuple.
const MELEE_CASTER_BUMPS: &[(&str, AttackBumpFn)] = &[
    ("rage", |a| {
        if a.has_condition(Condition::Raging) { 2 } else { 0 }
    }),
    ("dueling", |a| {
        if a.has_dueling_style() { 2 } else { 0 }
    }),
    ("two-weapon fighting", |a| {
        if !a.has_two_weapon_fighting_style() {
            return 0;
        }
        a.ability_modifier(AbilityScoreType::Strength).max(0) as u32
    }),
    // 5e Bladesinging Wizard **Song of Victory** (subclass level 14):
    // "add your Intelligence modifier (minimum of +1) to the damage of
    // your melee weapon attacks" while Bladesong is active. The clause
    // that makes the trance offensive as well as defensive, and the
    // only reason a wizard's dagger is worth swinging at all.
    ("song of victory", |a| {
        if !a.has_condition(Condition::Bladesinging) {
            return 0;
        }
        a.ability_modifier(AbilityScoreType::Intelligence).max(1) as u32
    }),
    ("aura of hate", |a| {
        if !a.has_aura_of_hate() {
            return 0;
        }
        // RAW: minimum +1 even if the paladin's CHA modifier is
        // zero or negative. Matches the Aura of Protection floor
        // shape (`aura_of_protection_bonus` clamps to `max(1)`).
        a.ability_modifier(AbilityScoreType::Charisma).max(1) as u32
    }),
];

/// Caster-side crit-only melee extra-dice sources read at
/// `resolve_attack_outcome` when a critical hit lands with a melee
/// weapon. Each entry is a (label, count_fn) tuple: the count_fn
/// reads the caster's template flag / dice-count field and returns
/// the number of extra weapon-face dice to roll (`0` skips the log
/// line and the roll).
///
/// Entries stack additively on holders that carry multiple sources —
/// a level-17 half-orc barbarian reads Brutal Critical's 3 dice AND
/// Savage Attacks' 1 die for a +4 dice crit bump.
///
/// Entries:
///   - **Brutal Critical**: Barbarian level 9 / 13 / 17 template-
///     driven dice count. `0` for non-barbarians (the default).
///   - **Savage Attacks**: Half-Orc racial flag; one flat extra die.
///
/// A new crit-extra-dice source (Piercer feat's +1 die, a hypothetical
/// Champion "Superior Critical" bonus die) drops in as a new tuple.
const CRIT_MELEE_EXTRA_DICE_SOURCES: &[(&str, AttackBumpFn)] = &[
    ("brutal critical", |a| a.brutal_critical_dice()),
    ("savage attacks", |a| if a.has_savage_attacks() { 1 } else { 0 }),
];

/// Shared eligibility gate for target-side reactive self-clamp damage
/// reducers — Uncanny Dodge (Rogue lv5), Deflect Missiles (Monk lv3),
/// and Parry (Fighter Battle Master maneuver). All three fire on an
/// incoming attack against the target and share the same gate shape:
///
/// 1. Target exists and is combat-active.
/// 2. Target holds the passive flag (`passive_ok(target)` returns true).
/// 3. Target has an unspent reaction slot.
/// 4. Target has an unspent charge of `feature_tag` (skipped when the
///    reducer is un-gated, i.e. Uncanny Dodge / Deflect Missiles).
/// 5. Target can perceive the attacker — routes through
///    `viewer_can_see` so an Invisible / Blurred / Displaced attacker
///    (or a Blinded / Sphered target) can't provoke the reaction.
///
/// Returns `true` iff all gates pass. Kept as a `&self`-only read so
/// callers can borrow the target for stat lookups (DEX / level) before
/// swapping to `&mut encounter` for the die roll.
///
/// Centralizes the five-clause chain that each of the three reducers
/// previously open-coded so that adding a future reactive reducer
/// (Shield Master's shove-on-hit, a hypothetical Deflect Missiles
/// analog for spell attacks) drops in as a one-line `is_eligible` call
/// plus the reduction body.
fn reactive_reducer_eligible(
    encounter: &EncounterInstance,
    target_id: usize,
    attacker_id: usize,
    passive_ok: fn(&ActorInstance) -> bool,
    feature_tag: Option<&'static str>,
) -> bool {
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    if !target.is_combat_active() || !passive_ok(target) || !target.has_reaction() {
        return false;
    }
    if feature_tag.is_some_and(|t| !target.feature_available(t)) {
        return false;
    }
    encounter.viewer_can_see(target_id, attacker_id)
}

/// Consume the shared side of a reactive self-clamp damage reducer —
/// spend the target's reaction slot and (optionally) the associated
/// per-rest feature charge. Called after Uncanny Dodge / Deflect
/// Missiles / Parry actually fire and reduce the swing's damage.
///
/// Pairs with `reactive_reducer_eligible` (the read-side gate) so the
/// two ends of the reactive-reducer lifecycle share one chokepoint per
/// side. Silent no-op if the target has already been cleaned up
/// mid-swing (matches the defensive pattern the sibling reducers used
/// before the refactor).
fn spend_reactive_reducer(
    encounter: &mut EncounterInstance,
    target_id: usize,
    feature_tag: Option<&'static str>,
) {
    if let Some(t) = encounter.actors.get_mut(&target_id) {
        t.consume_resource(crate::engine::side_effects::Resource::Reaction);
        if let Some(tag) = feature_tag {
            t.spend_feature(tag);
        }
    }
}

/// How a reactive damage-clamp row turns the damage a landed swing would
/// deal into a smaller number. The two shapes cover every clamp in the
/// engine: a proportional halving and a "roll a die, subtract the roll
/// plus a stat" reduction.
///
/// Kept separate from `ClampLane` / `ClampScope` because the three axes
/// are independent — Uncanny Dodge is `Halve` + `AnyAttack` + `Holder`
/// while Interception is `RollMinus` + `AnyAttack` + `Ally`, and a
/// future clamp can mix any combination.
#[derive(Clone, Copy)]
enum ClampFormula {
    /// Halve the damage, rounding down — Uncanny Dodge's RAW "halve the
    /// attack's damage against you". Rolls no dice, so it never touches
    /// the RNG stream.
    Halve,
    /// Roll `dice` and subtract `roll + bonus(reactor)`, floored at 0 —
    /// the superiority-die / psionic-die shape shared by Deflect
    /// Missiles (1d10 + DEX + monk level), Parry (1d8 + DEX), and
    /// Interception (1d10 + proficiency). `bonus` reads the *reactor*
    /// (who spends the reaction), not the damaged actor — the two differ
    /// on the ally-scoped rows.
    RollMinus {
        dice: Dice,
        bonus: fn(&ActorInstance) -> i32,
    },
}

/// Which incoming swings a clamp row may fire against. Checked against
/// the `(is_melee, is_spell)` pair the two attack chokepoints already
/// carry, so a row's RAW trigger wording maps to exactly one variant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClampLane {
    /// Any attack roll — melee or ranged, weapon or spell. Uncanny
    /// Dodge ("when an attacker that you can see hits you with an
    /// attack") and Interception ("hits a target with a weapon or spell
    /// attack") both read this broadly in RAW.
    AnyAttack,
    /// Melee attack rolls only, weapon *or* spell — Parry's RAW "when
    /// another creature damages you with a melee attack" doesn't
    /// exclude a melee spell attack (Vampiric Touch, Shocking Grasp).
    Melee,
    /// Ranged **weapon** attack rolls only — Deflect Missiles' RAW
    /// "when you are hit by a ranged weapon attack" explicitly excludes
    /// ranged spell attacks (Fire Bolt, Guiding Bolt), so this variant
    /// gates on `!is_spell` as well as `!is_melee`.
    RangedWeapon,
}

impl ClampLane {
    /// True if a swing described by `(is_melee, is_spell)` falls in this
    /// lane. `is_spell` distinguishes the two ranged sub-lanes; melee
    /// spell attacks and melee weapon swings share the `Melee` lane.
    fn admits(self, is_melee: bool, is_spell: bool) -> bool {
        match self {
            ClampLane::AnyAttack => true,
            ClampLane::Melee => is_melee,
            ClampLane::RangedWeapon => !is_melee && !is_spell,
        }
    }
}

/// Who may spend the reaction for a clamp row. The damaged actor and the
/// reactor are the same actor on the self-clamp rows and different
/// actors on the ally-shield rows; `pick_clamp_reactor` resolves the
/// distinction once so `fire_clamp` only ever sees a concrete reactor id.
#[derive(Clone, Copy)]
enum ClampScope {
    /// Only the damaged actor themselves (Uncanny Dodge, Deflect
    /// Missiles, Parry). Gated by `reactive_reducer_eligible`.
    Holder,
    /// Only a *different* actor on the damaged actor's team, within `n`
    /// footprint tiles of them — Fighting Style: Interception's RAW
    /// "hits a target, other than you, within 5 feet of you" → `Ally(0)`
    /// (footprint distance 0 means touching). Gated by
    /// `first_reactive_ally_within`.
    Ally(isize),
    /// The damaged actor if they carry the row, otherwise the first
    /// eligible ally within `n` footprint tiles — Psi Warrior's
    /// Protective Field, whose RAW trigger is "when you or a creature
    /// you can see within 30 feet of you takes damage" → the only
    /// scope covering both. Prefers the damaged actor so a Psi Warrior
    /// shields themselves before an ally spends a charge on them.
    HolderOrAlly(isize),
}

/// Cohort row shape for a reactive per-swing damage clamp: a feature
/// whose holder spends a reaction (and optionally a per-rest charge) to
/// shrink the damage a landed attack is about to deal.
///
/// Every clamp in the engine is one row here, and the shared walker
/// `apply_reactive_damage_clamps` owns the gate → roll → log → spend
/// body. Before this cohort existed the four clamps were four
/// near-identical open-coded blocks in `resolve_attack_outcome`, and
/// only Interception had been mirrored onto the spell-attack path — so
/// a Rogue hit by a Fire Bolt silently lost Uncanny Dodge even though
/// RAW's "hits you with an attack" covers it. Routing both chokepoints
/// through one walker closed that hole for every row at once and makes
/// a new clamp a single row addition.
struct ReactiveDamageClamp {
    /// Passive-feature predicate the reactor must satisfy. Reads the
    /// template-installed flag (`has_uncanny_dodge()`, `has_parry()`,
    /// …), *not* the per-rest charge — that's `tag`'s job.
    flag: fn(&ActorInstance) -> bool,
    /// `Some(tag)` if the row also burns a `feature_available(tag)`
    /// per-rest charge on top of the reaction (Parry's superiority
    /// die); `None` for the always-on rows (Uncanny Dodge, Deflect
    /// Missiles, Interception).
    tag: Option<&'static str>,
    /// Log-friendly identity ("uncanny dodge", "parry"). Distinct from
    /// `tag`, which carries a noisy `class.feature` namespace prefix.
    label: &'static str,
    /// Which swings the row can fire against — see `ClampLane`.
    lane: ClampLane,
    /// `Some(list)` if the row only answers a fixed set of damage types
    /// (Nature Domain's Dampen Elements: the five elemental types);
    /// `None` if it answers any damage, which is every RAW clamp that
    /// keys off *how* it was hit rather than *what with*.
    damage_types: Option<&'static [DamageType]>,
    /// Who spends the reaction — see `ClampScope`.
    scope: ClampScope,
    /// How much damage comes off — see `ClampFormula`.
    formula: ClampFormula,
}

/// Ordered cohort of every reactive per-swing damage clamp, walked by
/// `apply_reactive_damage_clamps` from both attack chokepoints
/// (`resolve_attack_outcome` for weapons, `spell_attack_outcome` for
/// spells).
///
/// Unlike the "stop on first firing" cohorts (`FAILED_SAVE_ADD_DIE_SOURCES`,
/// `REACTIVE_ATTACK_DISADVANTAGE_SOURCES`), every eligible row fires:
/// 5e reactions are independent features and a monk/rogue multiclass
/// deflecting *and* dodging the same arrow is RAW-legal. Each row spends
/// its own reactor's reaction, so in practice a single actor can only
/// contribute one row per round — the stacking only shows up across
/// different reactors or across a multiclass with two flags and a
/// reaction still in hand.
///
/// Order is the order damage flows through the clamps. It's observable:
/// halving before subtracting yields less final damage than the reverse,
/// so `Halve` rows come first (RAW leaves the ordering to the table, and
/// this is the target-favorable reading). Ally-scoped rows come last so
/// the ally clamps whatever survived the target's own defenses.
///
/// Entries:
///   - **Uncanny Dodge** (Rogue lv5): halve, any attack, self.
///   - **Deflect Missiles** (Monk lv3): 1d10 + DEX + monk level, ranged
///     weapon attacks only, self.
///   - **Parry** (Fighter Battle Master maneuver): 1d8 + DEX, melee
///     attacks only, self, burns a `PARRY_TAG` charge.
///   - **Interception** (Fighting Style, XGtE): 1d10 + proficiency, any
///     attack, adjacent ally.
///   - **Warding Maneuver** (Cavalier Fighter lv7, XGtE): halve, any
///     attack, self *or* an adjacent ally, burns a
///     `WARDING_MANEUVER_TAG` charge.
///   - **Dampen Elements** (Nature Domain Cleric lv6): halve, but only
///     acid / cold / fire / lightning / thunder damage; self *or* an ally
///     within 30 ft, burns a `DAMPEN_ELEMENTS_TAG` charge. The only row
///     with a damage-type filter.
///   - **Protective Field** (Psi Warrior Fighter lv3, TCE): 1d8 + INT,
///     any attack, self *or* an ally within 30 ft, burns a
///     `PROTECTIVE_FIELD_TAG` charge.
///
/// The two charge-gated rows come last, so a scarce charge is only spent
/// on damage the free clamps above couldn't already absorb. Warding
/// Maneuver precedes Protective Field because it halves what remains
/// while the field subtracts a fixed amount — halving the larger number
/// first leaves less damage than the reverse, and both rows belong to
/// different subclasses, so the ordering only ever matters to a
/// multiclass.
const REACTIVE_DAMAGE_CLAMPS: &[ReactiveDamageClamp] = &[
    ReactiveDamageClamp {
        flag: |a| a.has_uncanny_dodge(),
        tag: None,
        label: "uncanny dodge",
        damage_types: None,
        lane: ClampLane::AnyAttack,
        scope: ClampScope::Holder,
        formula: ClampFormula::Halve,
    },
    ReactiveDamageClamp {
        flag: |a| a.has_deflect_missiles(),
        tag: None,
        label: "deflect missiles",
        damage_types: None,
        lane: ClampLane::RangedWeapon,
        scope: ClampScope::Holder,
        formula: ClampFormula::RollMinus {
            dice: Dice::new(1, 10),
            bonus: |a| a.ability_modifier(AbilityScoreType::Dexterity) + a.level() as i32,
        },
    },
    ReactiveDamageClamp {
        flag: |a| a.has_parry(),
        tag: Some(crate::actions::class_features::PARRY_TAG),
        label: "parry",
        damage_types: None,
        lane: ClampLane::Melee,
        scope: ClampScope::Holder,
        formula: ClampFormula::RollMinus {
            dice: Dice::new(1, 8),
            bonus: |a| a.ability_modifier(AbilityScoreType::Dexterity),
        },
    },
    ReactiveDamageClamp {
        flag: |a| a.has_interception_style(),
        tag: None,
        label: "interception",
        damage_types: None,
        lane: ClampLane::AnyAttack,
        scope: ClampScope::Ally(0),
        formula: ClampFormula::RollMinus {
            dice: Dice::new(1, 10),
            bonus: |a| a.proficiency_bonus(),
        },
    },
    ReactiveDamageClamp {
        flag: |a| a.has_passive_feature(crate::actions::class_features::WARDING_MANEUVER_TAG),
        tag: Some(crate::actions::class_features::WARDING_MANEUVER_TAG),
        label: "warding maneuver",
        damage_types: None,
        lane: ClampLane::AnyAttack,
        scope: ClampScope::HolderOrAlly(0),
        formula: ClampFormula::Halve,
    },
    ReactiveDamageClamp {
        flag: |a| a.has_passive_feature(crate::actions::class_features::DAMPEN_ELEMENTS_TAG),
        tag: Some(crate::actions::class_features::DAMPEN_ELEMENTS_TAG),
        label: "dampen elements",
        // The five RAW elemental damage types. The only row in the cohort
        // that keys off *what* the damage is rather than *how* it
        // arrived, and the reason the filter column exists.
        damage_types: Some(&[
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Lightning,
            DamageType::Thunder,
        ]),
        lane: ClampLane::AnyAttack,
        // 12 tiles = 30 ft on the 2.5 ft grid.
        scope: ClampScope::HolderOrAlly(12),
        formula: ClampFormula::Halve,
    },
    ReactiveDamageClamp {
        flag: |a| a.has_passive_feature(crate::actions::class_features::PROTECTIVE_FIELD_TAG),
        tag: Some(crate::actions::class_features::PROTECTIVE_FIELD_TAG),
        label: "protective field",
        damage_types: None,
        lane: ClampLane::AnyAttack,
        // 12 tiles = 30 ft on the 2.5 ft grid.
        scope: ClampScope::HolderOrAlly(12),
        formula: ClampFormula::RollMinus {
            dice: Dice::new(1, 8),
            bonus: |a| a.ability_modifier(AbilityScoreType::Intelligence),
        },
    },
];

/// Cohort row shape for a passive weapon-hit condition mark: a feature
/// whose holder stamps a condition — plus whatever back-link that
/// condition carries — onto every target their weapon connects with.
///
/// Most rows are *not* once-per-turn. Every connecting swing re-stamps
/// the mark, which is a no-op while one is already up (`add_condition`
/// keeps the longer timer) and refreshes a window that has partly
/// decayed, matching RAW on Eldritch Strike and Unwavering Mark: each
/// fires "when you hit", not "the first time you hit". Ancestral
/// Protectors is worded the other way and says so with its `cadence`.
struct OnHitConditionMark {
    /// Passive-feature tag the mark keys off, read via
    /// `has_passive_feature`. No per-rest charge — every row here is
    /// always-on.
    tag: &'static str,
    /// Condition stamped on the target. If it appears in
    /// `condition_link_side_effect`'s dispatch, the paired `Set*By`
    /// back-link is queued alongside it automatically.
    condition: Condition,
    /// Lifetime of the stamp. Both current rows use `Rounds(2)`: their
    /// RAW windows end "at the end of your next turn", and a
    /// `UntilStartOfNextTurn` timer would decay on the *target's* clock
    /// rather than the marker's.
    timer: ConditionTimer,
    /// `true` if RAW restricts the trigger to melee weapon attacks
    /// (Unwavering Mark's "hit a creature with a melee weapon attack");
    /// `false` if any weapon attack qualifies (Eldritch Strike's "hit a
    /// creature with a weapon attack" — the knight's longbow counts).
    /// Spell attacks never qualify for either row, so the walker gates
    /// on `!is_spell` unconditionally.
    melee_only: bool,
    /// Extra caster-side precondition beyond holding `tag`. `None` for
    /// always-on marks; Ancestral Protectors uses it for RAW's "while
    /// raging" clause, which is a condition rather than a feature and so
    /// can't be folded into the tag check.
    holder_gate: Option<fn(&crate::actors::actor_template::ActorInstance) -> bool>,
    /// How often the mark lands. See `MarkCadence`.
    cadence: MarkCadence,
    /// Full log line for the stamp, minus the leading indent.
    log: &'static str,
}

/// How often a row on `ON_HIT_CONDITION_MARKS` stamps its mark.
///
/// RAW draws this distinction with two different phrasings and the
/// difference is mechanically large, so the row carries it explicitly
/// rather than leaving it to the reader of the trigger site.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkCadence {
    /// "When you hit a creature…" — every connecting swing re-stamps,
    /// and a holder who hits three creatures in a turn has marked all
    /// three. Eldritch Strike, Unwavering Mark.
    EveryHit,
    /// "The first creature you hit on your turn becomes the target…" —
    /// only the turn's first connecting swing stamps, and the mark
    /// *moves*: whoever the holder had marked before loses it, because
    /// RAW makes the mark a relationship with one creature at a time
    /// rather than a debuff that accumulates. Ancestral Protectors.
    ///
    /// Both halves matter. Without the once-per-turn gate a barbarian
    /// with Extra Attack marks two creatures a turn; without the move,
    /// last turn's target keeps the mark until its own timer runs out
    /// and the barbarian ends up guarded against a growing crowd.
    FirstHitOfTurn,
}

/// Cohort of passive weapon-hit condition marks, walked by
/// `push_on_hit_condition_marks` from `resolve_attack_outcome`.
///
/// Entries:
///   - **Eldritch Strike** (Eldritch Knight Fighter lv10): stamps
///     `EldritchStruck`, so the target's next save against a spell the
///     knight casts is at disadvantage. Any weapon attack.
///   - **Unwavering Mark** (Cavalier Fighter lv3): stamps `Dueled`, so
///     the target attacks anyone other than the cavalier at
///     disadvantage. Melee weapon attacks only.
///
/// Both rows lean on a condition that already existed for a spell —
/// `EldritchStruck` has no other source, and `Dueled` is Compelled
/// Duel's. That the Cavalier's headline feature is mechanically a
/// free, at-will, no-concentration Compelled Duel is a fair reading of
/// RAW, and the reason the two share a condition rather than each
/// getting one.
const ON_HIT_CONDITION_MARKS: &[OnHitConditionMark] = &[
    OnHitConditionMark {
        tag: crate::actions::class_features::ELDRITCH_STRIKE_TAG,
        condition: Condition::EldritchStruck,
        timer: ConditionTimer::Rounds(2),
        melee_only: false,
        holder_gate: None,
        cadence: MarkCadence::EveryHit,
        log: "eldritch strike: the blow rattles the target's guard",
    },
    OnHitConditionMark {
        tag: crate::actions::class_features::UNWAVERING_MARK_TAG,
        condition: Condition::Dueled,
        timer: ConditionTimer::Rounds(2),
        melee_only: true,
        holder_gate: None,
        cadence: MarkCadence::EveryHit,
        log: "unwavering mark: the target is locked onto its attacker",
    },
    // 5e Path of the Ancestral Guardian Barbarian **Ancestral
    // Protectors** (subclass level 3). "While you're raging, the first
    // creature you hit with an attack on your turn becomes the target
    // of the warriors."
    OnHitConditionMark {
        tag: crate::actions::class_features::ANCESTRAL_PROTECTORS_TAG,
        condition: Condition::AncestrallyHaunted,
        // RAW's window is "until the start of your next turn", which is
        // the marker's clock, not the target's — the same reason the
        // two rows above use `Rounds(2)` rather than
        // `UntilStartOfNextTurn`.
        timer: ConditionTimer::Rounds(2),
        // RAW is "hit with an attack", not "with a melee weapon
        // attack" — a thrown handaxe marks just as well.
        melee_only: false,
        holder_gate: Some(|a| a.has_condition(Condition::Raging)),
        cadence: MarkCadence::FirstHitOfTurn,
        log: "ancestral protectors: the spirits fix on the barbarian's first mark",
    },
];

/// Walk `ON_HIT_CONDITION_MARKS` and queue every mark the swing earns.
/// Called from `resolve_attack_outcome` once the hit and its damage
/// riders are settled, so a mark lands even if the target dies to the
/// same swing (the condition is then dropped with the actor — harmless,
/// and cheaper than predicting lethality here).
fn push_on_hit_condition_marks(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    p: &AttackParams,
) {
    if p.is_spell {
        return;
    }
    for row in ON_HIT_CONDITION_MARKS {
        if row.melee_only && !p.is_melee {
            continue;
        }
        let holds = encounter.actors.get(&p.caster_id).is_some_and(|a| {
            a.has_passive_feature(row.tag)
                && row.holder_gate.is_none_or(|gate| gate(a))
                && !(row.cadence == MarkCadence::FirstHitOfTurn
                    && a.once_per_turn_used(row.tag))
        });
        if !holds {
            continue;
        }
        if row.cadence == MarkCadence::FirstHitOfTurn {
            // Spend the turn's single stamp, then move the mark off
            // whoever was carrying it. Both halves of `FirstHitOfTurn`
            // — see the variant docs.
            if let Some(a) = encounter.actors.get_mut(&p.caster_id) {
                a.mark_once_per_turn_used(row.tag);
            }
            for stale in previously_marked_by(encounter, row.condition, p.caster_id, p.target_id) {
                effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                    actor_id: stale,
                    condition: row.condition,
                }));
            }
        }
        encounter.log(format!("  {}", row.log));
        effects.push(Box::new(ApplyCondition {
            actor_id: p.target_id,
            condition: row.condition,
            timer: row.timer,
        }));
        // Conditions that carry a back-link to whoever applied them get
        // their `Set*By` install from the central dispatch in
        // side_effects.rs — the same one Compelled Duel and Vow of
        // Enmity use — so a new linked condition needs no change here.
        if let Some(link) = attacker_link_side_effect(row.condition, p.target_id, p.caster_id) {
            effects.push(link);
        }
    }
}

/// Sorted ids of every actor other than `new_target` currently carrying
/// `condition` back-linked to `holder`. Used by `MarkCadence::FirstHitOfTurn`
/// to move an exclusive mark rather than accumulate it.
///
/// Sorted so the generated `RemoveCondition` effects land in a
/// deterministic order; in practice the list is almost always empty or a
/// single id, since the mark is exclusive by construction.
fn previously_marked_by(
    encounter: &EncounterInstance,
    condition: Condition,
    holder: usize,
    new_target: usize,
) -> Vec<usize> {
    let mut ids: Vec<usize> = encounter
        .actors
        .iter()
        .filter(|(id, a)| **id != new_target && a.linked_by(condition) == Some(holder))
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    ids
}

/// Resolve which actor (if any) spends a reaction for `row` against this
/// swing. `&self`-only so the caller can keep reading actor stats before
/// switching to `&mut` for the die roll.
fn pick_clamp_reactor(
    encounter: &EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    row: &ReactiveDamageClamp,
) -> Option<usize> {
    match row.scope {
        ClampScope::Holder => {
            reactive_reducer_eligible(encounter, target_id, attacker_id, row.flag, row.tag)
                .then_some(target_id)
        }
        ClampScope::Ally(max_tiles) => encounter.first_reactive_ally_within(
            attacker_id,
            target_id,
            max_tiles,
            &row.flag,
            row.tag,
        ),
        ClampScope::HolderOrAlly(max_tiles) => {
            if reactive_reducer_eligible(encounter, target_id, attacker_id, row.flag, row.tag) {
                Some(target_id)
            } else {
                encounter.first_reactive_ally_within(
                    attacker_id,
                    target_id,
                    max_tiles,
                    &row.flag,
                    row.tag,
                )
            }
        }
    }
}

/// Roll `row`'s reduction, log it, spend the reactor's reaction (plus
/// any per-rest charge), and return the clamped damage. Assumes the
/// gates in `pick_clamp_reactor` already passed.
fn fire_clamp(
    encounter: &mut EncounterInstance,
    reactor_id: usize,
    target_id: usize,
    damage: u32,
    row: &ReactiveDamageClamp,
) -> u32 {
    let (reduced, detail) = match row.formula {
        ClampFormula::Halve => (damage / 2, "halved".to_string()),
        ClampFormula::RollMinus { dice, bonus } => {
            let flat = encounter.actors.get(&reactor_id).map_or(0, bonus);
            let raw = encounter.roll(&dice) as i32;
            let reduction = (raw + flat).max(0) as u32;
            (
                damage.saturating_sub(reduction),
                format!("{}({}){:+} = -{}", dice, raw, flat, reduction),
            )
        }
    };
    // Name the reactor only on the ally-scoped rows — on a self-clamp
    // the label already identifies them, and the extra name would just
    // repeat the actor whose damage line sits directly above.
    let who = if reactor_id == target_id {
        String::new()
    } else {
        encounter
            .actors
            .get(&reactor_id)
            .map_or_else(String::new, |a| format!("{} ", a.name()))
    };
    encounter.log(format!(
        "  {}: {}{} ({} \u{2192} {})",
        row.label, who, detail, damage, reduced
    ));
    spend_reactive_reducer(encounter, reactor_id, row.tag);
    reduced
}

/// Walk `REACTIVE_DAMAGE_CLAMPS` over a landed swing and return the
/// damage that survives. Called from both attack chokepoints once the
/// hit is confirmed and the base damage line is logged, so every clamp
/// applies uniformly to weapon and spell attacks (subject to each row's
/// `ClampLane`).
///
/// `is_melee` / `is_spell` / `damage_type` describe the swing; `damage`
/// is the pre-resistance total (target-side resistance / immunity is
/// applied later, at `DealDamage::apply`) — matching RAW, where these
/// features reduce the attack's damage before the target's damage types
/// are consulted. `damage_type` is only read by rows that carry a
/// `damage_types` filter.
///
/// Short-circuits as soon as the damage hits 0: a clamp that can't
/// shave anything off shouldn't burn its holder's reaction (or a
/// per-rest charge) for nothing.
/// Damage reductions that depend on who is *swinging* rather than on who
/// is being hit or on anyone spending a reaction.
///
/// The engine had no lane for this. `DealDamage` carries no attacker, so
/// a rule of the form "damage *this creature* deals to *anyone but
/// them* is halved" can't live in the target's resistance pipeline;
/// `REACTIVE_DAMAGE_CLAMPS` is about a defender or ally spending a
/// reaction, which this costs nobody. So it sits between the two, called
/// from both attack chokepoints with both ids in hand.
///
/// One rule today - the Ancestral Guardian's half of Ancestral
/// Protectors. RAW words it as the *victim* gaining resistance, but the
/// gate is entirely on the attacker (are they haunted, and is their
/// target someone other than the barbarian who haunted them), so the
/// attacker-scoped framing is the one that can actually be evaluated.
pub fn attacker_scoped_damage_reduction(
    encounter: &mut EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    damage: u32,
) -> u32 {
    if damage == 0 {
        return 0;
    }
    // 5e Ancestral Protectors: "when the creature hits a creature other
    // than you with an attack, that target has resistance to the damage
    // dealt by the attack."
    let guarded_by = encounter
        .actors
        .get(&attacker_id)
        .and_then(|a| a.linked_by(Condition::AncestrallyHaunted));
    let Some(barbarian) = guarded_by else {
        return damage;
    };
    if barbarian == target_id {
        // The spirits don't protect anyone from a blow aimed at the
        // barbarian themselves - that is the trade the feature offers.
        return damage;
    }
    let reduced = damage / 2;
    let target_name = encounter.actor_name(target_id);
    encounter.log(format!(
        "  ancestral protectors: the spirits blunt the blow against {} ({} -> {})",
        target_name, damage, reduced
    ));
    reduced
}

pub fn apply_reactive_damage_clamps(
    encounter: &mut EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    mut damage: u32,
    is_melee: bool,
    is_spell: bool,
    damage_type: DamageType,
) -> u32 {
    for row in REACTIVE_DAMAGE_CLAMPS {
        if damage == 0 {
            break;
        }
        if !row.lane.admits(is_melee, is_spell) {
            continue;
        }
        if row
            .damage_types
            .is_some_and(|types| !types.contains(&damage_type))
        {
            continue;
        }
        let Some(reactor_id) = pick_clamp_reactor(encounter, attacker_id, target_id, row) else {
            continue;
        };
        damage = fire_clamp(encounter, reactor_id, target_id, damage, row);
    }
    damage
}

/// 5e Fighter Battle Master **Riposte** maneuver — reactive melee
/// counter-attack that fires when a melee attack MISSES the target and
/// the target holds the `has_riposte` flag with an unspent `RIPOSTE_TAG`
/// charge, an available reaction, and a suitable melee weapon action on
/// their action list. Also gated on `viewer_can_see` so an Invisible /
/// Blurred / Displaced attacker (or a Blinded fighter) can't be
/// counter-struck (mirrors the Uncanny Dodge / Deflect Missiles sight
/// gate). No-op on any missing prerequisite. The counter-attack fires
/// via the same "run the underlying attack's side_effects" chokepoint
/// the opportunity-attack dispatcher uses in `EncounterInstance`, so
/// weapon-side riders (Bless, on-hit smite primes, etc.) fold in
/// cleanly without a bespoke roll pipeline here.
///
/// Kept public so `spells.rs` can drive it from `spell_attack_outcome`
/// if we later extend RAW to trigger Riposte on missed spell attacks
/// too — the current RAW clause is melee-only so the caller in
/// `resolve_attack_outcome` gates on `p.is_melee` at the call site.
pub fn try_fire_riposte(
    encounter: &mut EncounterInstance,
    target_id: usize,
    attacker_id: usize,
) {
    use crate::actions::action_template::MELEE_REACH;

    // Shared eligibility gate (passive flag + reaction slot + per-rest
    // charge + sight) — same helper the self-clamp reducers (Uncanny
    // Dodge / Deflect Missiles / Parry) use. Riposte is a counter-attack
    // rather than a damage clamp, but its five-clause gate is identical
    // so both lanes route through the shared chokepoint.
    if !reactive_reducer_eligible(
        encounter,
        target_id,
        attacker_id,
        |a| a.has_riposte(),
        Some(crate::actions::class_features::RIPOSTE_TAG),
    ) {
        return;
    }
    // 5e Charmed: a charmed creature can't attack its charmer. Riposte
    // is where this lane parts company with the three self-clamp
    // reducers that share the eligibility gate above — Uncanny Dodge,
    // Deflect Missiles and Parry only reduce incoming damage, which a
    // charmed creature may do freely, but Riposte swings back.
    if encounter.charm_blocks_hostility(target_id, attacker_id) {
        return;
    }
    // Find the target's first melee weapon action via the shared
    // `first_melee_weapon_action` predicate — same helper that the
    // opportunity-attack dispatcher uses on the reactor side. Filters
    // out touch-range buffs / heals (Cure Wounds is `SingleActor` with
    // reach 1 but harmless), ranged actions, and AoE bursts.
    let Some(attack) = encounter
        .actors
        .get(&target_id)
        .and_then(|target| target.first_melee_weapon_action())
    else {
        return;
    };
    // Confirm the attacker is still in range of the target — RAW: the
    // riposte is a melee weapon attack, so the standard reach check
    // must pass. Uses the encounter helper so multi-tile footprints
    // resolve correctly.
    let reach = attack.reach_tiles().unwrap_or(MELEE_REACH);
    let Some(dist) = encounter.footprint_distance(target_id, attacker_id) else {
        return;
    };
    if dist > reach {
        return;
    }

    let target_name = encounter.actor_name(target_id);
    let attacker_name = encounter.actor_name(attacker_id);
    encounter.log(format!(
        "[reaction] {} ripostes {} after the miss",
        target_name, attacker_name
    ));

    // Fire the underlying attack's side_effects directly (matches the
    // opportunity-attack dispatch shape). Cost consumption is bypassed
    // — Riposte is a reaction, not a regular action, so we spend the
    // reaction + feature charge below instead of the action's normal
    // Action-slot cost.
    let target_vec = vec![attacker_id];
    let effects = attack.side_effects(encounter, target_id, Some(&target_vec), None, None);
    for e in effects {
        e.apply(encounter);
    }
    spend_reactive_reducer(
        encounter,
        target_id,
        Some(crate::actions::class_features::RIPOSTE_TAG),
    );
    encounter.cleanup_dead_actors();
}

/// Resolve a 5e d20 attack roll against a single target's AC. On a hit,
/// returns a `DealDamage` side-effect for the rolled damage; on a miss,
/// returns an empty vec. The d20 result, hit/miss outcome, and damage
/// breakdown are logged in the engine's standard shape.
///
/// Critical hits (natural 20 after advantage/disadvantage) auto-hit and
/// double the damage dice (the modifier is added once). Caster's flat
/// `attack_bonus_buff` (Bless, etc.) is added at roll time so a buff that
/// landed mid-multiattack still picks up later swings correctly.
pub fn resolve_attack(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    resolve_attack_outcome(encounter, p).0
}

/// `resolve_attack` with one action-specific extra clause layered on —
/// see `OnHitRider`. Use this instead of open-coding an attack roll when
/// an action needs a rider the shared cohort tables can't express.
pub fn resolve_attack_with_rider(
    encounter: &mut EncounterInstance,
    p: AttackParams,
    rider: &dyn ActionOnHitRider,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    resolve_attack_outcome_with_rider(encounter, p, rider).0
}

/// Result of an attack roll. `damage_dealt` is the post-crit, pre-target-
/// resistance damage value that will hit the queue — `0` on a miss or when
/// a Mirror Image absorbed the swing. Use this variant when the caller
/// needs to chain off the rolled damage (e.g. a self-heal rider equal to
/// half the damage, or a max-HP drain equal to the damage on a failed
/// save) without re-rolling and double-consuming the RNG.
pub fn resolve_attack_outcome(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    resolve_attack_outcome_with_rider(encounter, p, &NoRider)
}

/// `resolve_attack_outcome` with an action-specific `ActionOnHitRider`
/// layered
/// on. The two public entry points above are this function with the
/// no-op rider; every rule in the pipeline is shared between them, which
/// is the point.
pub fn resolve_attack_outcome_with_rider(
    encounter: &mut EncounterInstance,
    p: AttackParams,
    rider: &dyn ActionOnHitRider,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let Some(target_ac) = encounter
        .actors
        .get(&p.target_id)
        .map(|a| a.armor_class() as i32)
    else {
        return (Vec::new(), 0);
    };
    // 5e Swashbuckler Rogue Fancy Footwork (subclass level 3): every
    // melee attack made by the attacker writes the target id onto
    // their per-turn `melee_attack_targets_this_turn` ledger. Cleared
    // at the attacker's own turn-start reset. The write is
    // unconditional (no `has_fancy_footwork` gate) so the mark stays
    // cheap; the OA-suppression read in `dispatch_opportunity_attacks`
    // is where the flag gates the suppression. RAW's trigger is on
    // "making a melee attack" — the mark goes BEFORE the Sanctuary save
    // check below, so even a swing that bounces off a sanctified target
    // still counts as an attempted attack (mirrors RAW's "if you make a
    // melee attack" wording — the swing was attempted even if it was
    // warded off). Written for weapon melee only via the `p.is_melee`
    // gate — a spell attack routed through this chokepoint (currently
    // only weapon-attack-shaped spells like Booming Blade and Green
    // Flame Blade) picks up the mark too since they're engine-tagged
    // `is_melee: true`, matching the RAW "melee attack roll" trigger.
    //
    // `attack_mode_with_riders` further down writes the same mark, and
    // that duplication is deliberate rather than leftover: the ledger
    // insert is an idempotent `HashSet` add, and the two writers cover
    // different swings. The wrapper never runs for a swing that
    // Sanctuary turns away (the early return below beats it), which is
    // exactly the case this write exists for.
    if p.is_melee
        && let Some(attacker) = encounter.actors.get_mut(&p.caster_id)
    {
        attacker.mark_melee_attacked_this_turn(p.target_id);
    }
    // 5e Cover: intervening combat-active creatures bump the target's
    // effective AC (+2 for half cover, +5 for three-quarters). Adjacent
    // melee swings are exempt (the cover routine returns 0 at gap ≤ 1).
    let cover_bonus = encounter.cover_ac_bonus(p.caster_id, p.target_id);
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics option,
    // lv7): if the target holds `MULTIATTACK_DEFENSE_TAG` AND this
    // attacker has already landed a connecting swing on the target
    // this turn, add +4 to the target's effective AC. The +4 lasts
    // for "the rest of the turn" per RAW — anchored to the attacker's
    // own turn-start reset in `reset_for_new_round`. Shared read
    // between weapon and spell attacks via the encounter helper.
    let multiattack_defense_bonus =
        encounter.multiattack_defense_ac_bonus(p.caster_id, p.target_id);
    let target_ac = target_ac + cover_bonus + multiattack_defense_bonus;

    // 5e Sanctuary: if the target is sanctified, the attacker first makes
    // a WIS save vs the warding caster's DC. On fail, the attack silently
    // misses (no AC roll, no rider consumption). Pass-through behavior
    // matches RAW's "the attacker must choose a new target or lose the
    // attack" clause — we choose the latter to avoid auto-retargeting.
    if encounter.sanctuary_save_blocks(p.caster_id, p.target_id) {
        return (Vec::new(), 0);
    }
    // Casting a harmful action revokes the holder's own Sanctuary buff.
    // We tag the clear after we know the attack is going through (the
    // sanctuary_save_blocks branch returns early on fail).
    encounter.break_sanctuary_on_hostile(p.caster_id);

    // `attack_mode_with_riders` rather than the bare
    // `compute_attack_mode`, because the two disagree about one lane and
    // this is the side that needs it. `compute_attack_mode` reads the
    // `Helped` *condition*; the per-target `HelpGrant` ledger is read
    // only by the wrapper. Weapon swings used to call the inner
    // function and then clear the one-shot rider stack below — which
    // *consumes* the grant — so a grant installed without the
    // accompanying condition was eaten without ever granting anything.
    //
    // Three features install only the grant, and all three exist to
    // make a weapon swing land: the Battle Master's Feinting Attack,
    // Commander's Strike (on the ordered ally), and the Arcane
    // Trickster's Versatile Trickster. The `Help` action was unaffected
    // only because it installs the condition too.
    let mut mode =
        encounter.attack_mode_with_riders(p.caster_id, p.target_id, p.is_melee);
    // Defender-side reactive taxes on the attack roll — Fighting Style:
    // Protection (an adjacent ally spends their reaction) and the
    // `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` per-rest cohort (Warding
    // Flare, Entropic Ward). Both lanes live behind one engine
    // chokepoint so the spell-attack path in `spell_attack_outcome`
    // gets the identical sequence; neither has a weapon-only qualifier
    // in RAW. Ordering-wise both fire before the help / bless one-shot
    // rider consumption below, so a taxed swing STILL burns the
    // attacker's Help / Hidden / Inspired priming — RAW: those primes
    // are consumed on the roll, regardless of disadvantage.
    mode = encounter.apply_reactive_attack_taxes(p.caster_id, p.target_id, mode);
    // 5e long-range disadvantage: ranged weapon attacks beyond normal
    // range but within max range impose disadvantage. The `long_range`
    // threshold (in tiles) is set by the weapon definition — melee
    // weapons and spells leave it `None`.
    if let Some(nr) = p.long_range
        && !p.is_melee
        && let Some(dist) = encounter.footprint_distance(p.caster_id, p.target_id)
        && dist > nr
    {
        mode = mode.combine(crate::engine::dice::RollMode::Disadvantage);
    }
    // Caster-side flat bonuses. `attack_bonus_buff` is the install-side
    // ledger (Bless's AdjustAttackBuff(+2), etc.). `condition_attack_bonus`
    // is the read-side flag table — Sacred Weapon's +CHA modifier and
    // Bardic Inspiration's +3 ride here. Keeping the two lanes separate
    // makes Bless's "install once, drop on concentration" pattern reuse
    // cleanly with the read-only condition lane. Shared with spell
    // attacks via `EncounterInstance::caster_attack_buffs`.
    //
    // Read these BEFORE clearing the one-shot riders so the Inspired
    // condition (and any other condition that contributes to
    // `condition_attack_bonus`) is still active when we sum the bonus.
    // The rider clear below removes Inspired alongside Helped/Hidden,
    // so swapping the order would zero out the +3.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(p.caster_id);
    // 5e Fighting Style: **Archery** — +2 to attack rolls made with ranged
    // weapons. RAW carves out spell attack rolls ("ranged weapon attacks"
    // specifically), so the bonus is gated on `!p.is_melee && !p.is_spell`.
    // Read on the caster side so a ranger's longbow shot picks up the bonus
    // regardless of the target — mirrors how the item / condition attack
    // bonus lanes fold in above.
    let archery_bonus = if !p.is_melee
        && !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_archery_style())
    {
        2
    } else {
        0
    };
    // Bless/Bane: roll an actual 1d4 once per attack and add (Bless) or
    // subtract (Bane) from the total. Both: they cancel and no die is
    // rolled. We log the d4 separately so the player can see why the
    // d20 alone doesn't account for the swing's hit.
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(p.caster_id);
    // Burn through the one-shot rider stack (Helped, Hidden,
    // per-target help grant, Invisibility concentration, Inspired)
    // before the d20 lands so a second swing this turn doesn't double-
    // dip. The advantage / disadvantage flags were already picked up
    // into `mode` by `compute_attack_mode` above; the flat bonuses
    // were just read into `cond_attack_bonus` immediately above. Both
    // are safe to clear here without losing this swing's modifiers.
    encounter.clear_attack_advantage_riders(p.caster_id, p.target_id);
    // 5e Lucky: if the holder rolls a nat-1, they may re-roll once. The
    // helper folds the reroll into the same seedable RNG so determinism
    // by seed holds — and falls back to the raw roll for actors without
    // the trait.
    let mut raw_attack = encounter.roll_d20_lucky(p.caster_id, mode) as i32;
    // 5e Improved Critical: the d20 face that promotes to a crit is
    // template-driven (Champion fighter: 19+; Superior Critical: 18+)
    // and can also be target-scoped (Hexblade's Curse: 19+ against the
    // one cursed creature). The engine-level `crit_threshold_against`
    // accessor combines both and folds in the default of 20 for missing
    // actors / builds with neither.
    let mut nat_crit = raw_attack >= encounter.crit_threshold_against(p.caster_id, p.target_id);
    let mut attack_total =
        raw_attack + p.attack_bonus + buff + cond_attack_bonus + bless_die + archery_bonus;
    let mut is_nat_one = raw_attack == 1;
    let mut hit = !is_nat_one && (nat_crit || attack_total >= target_ac);
    // 5e Tasha's Sorcerer Seeking Spell metamagic: on a missed spell
    // attack, reroll the d20 and use the new face (RAW: "you must use
    // the new roll"). Gated on `is_spell` so weapon swings don't pick
    // up the rider. Mirrors the identical hook on the spell-attack
    // chokepoint in spells.rs — both attack paths go through this same
    // `EncounterInstance::reroll_seeking_spell` helper.
    if !hit && p.is_spell {
        let new_raw = encounter.reroll_seeking_spell(p.caster_id, raw_attack as u32, mode) as i32;
        if new_raw != raw_attack {
            raw_attack = new_raw;
            nat_crit = raw_attack >= encounter.crit_threshold_against(p.caster_id, p.target_id);
            attack_total = raw_attack
                + p.attack_bonus
                + buff
                + cond_attack_bonus
                + bless_die
                + archery_bonus;
            is_nat_one = raw_attack == 1;
            hit = !is_nat_one && (nat_crit || attack_total >= target_ac);
        }
    }
    // 5e Wild Magic Sorcerer **Bend Luck** (lv6 reaction): the target may
    // burn 2 SP + their reaction to subtract a 1d4 from the attacker's
    // total. Only worth firing when the swing would otherwise hit AND
    // isn't a natural crit (the d4 can't undo a 20-face); a nat-1 already
    // misses. The penalty is folded into `attack_total` so the hit check
    // and the log line both reflect the bent total.
    let bend_penalty = if hit && !nat_crit && !is_nat_one {
        encounter.apply_bend_luck_penalty(p.target_id, p.caster_id) as i32
    } else {
        0
    };
    let bend_note = if bend_penalty > 0 {
        attack_total -= bend_penalty;
        hit = attack_total >= target_ac;
        format!(" -d4({})", bend_penalty)
    } else {
        String::new()
    };
    // 5e Paralyzed / Unconscious clause: any hit from within 5ft is a
    // crit. The promotion happens after we've decided the swing connected
    // so a flat miss still misses — the rider only upgrades a regular
    // hit to a crit (mirrors the RAW "any attack that hits the creature
    // is a critical hit" wording).
    let is_crit = nat_crit
        || (hit
            && encounter.target_grants_melee_auto_crit(p.caster_id, p.target_id, p.is_melee));
    let outcome = if is_nat_one {
        "miss (nat 1)"
    } else if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let cover_note = EncounterInstance::cover_log_suffix(cover_bonus);
    encounter.log(format!(
        "  {}: 1d20({}){:+}{}{} = {} vs AC {}{}{} \u{2014} {}",
        p.action_name,
        raw_attack,
        p.attack_bonus + buff + cond_attack_bonus + archery_bonus,
        bless_note,
        bend_note,
        attack_total,
        target_ac,
        cover_note,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        // 5e Fighter Battle Master **Riposte** maneuver: on a melee
        // miss against a fighter with the passive `has_riposte` flag and
        // an unspent `RIPOSTE_TAG` charge, spend the reaction + charge
        // to fire a melee weapon attack against the attacker. RAW: "when
        // a creature misses you with a melee attack, you can use your
        // reaction and expend one superiority die to make a melee weapon
        // attack against the creature." Gated on `is_melee` (RAW: melee
        // miss only), sight (routes through `viewer_can_see` so an
        // Invisible / Blurred / Displaced attacker can't be counter-
        // struck), and the target holding a suitable melee action on
        // their action list. Skipped on nat-1s AND non-melee misses so a
        // whiffed longbow shot or spell attack doesn't burn the charge.
        if p.is_melee {
            try_fire_riposte(encounter, p.target_id, p.caster_id);
        }
        return (Vec::new(), 0);
    }
    // Post-hit interception: a connecting swing may still land on
    // something that isn't the target — a Mirror Image decoy, or the
    // Illusion Wizard's Illusory Self duplicate. Shared with spell
    // attacks via `EncounterInstance::attack_intercepted` so every row
    // of the cohort applies to any attack roll, not just weapon swings
    // (RAW, uniformly: "any attack roll against you").
    if encounter.attack_intercepted(p.target_id, p.caster_id, is_crit) {
        return (Vec::new(), 0);
    }
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics, lv7):
    // record that this attacker has now landed a connecting swing on
    // the target's own body — RAW's trigger is "when a creature hits
    // you" so an intercepted swing (which lands on a Mirror Image
    // decoy or an Illusory Self duplicate, not the target) doesn't
    // count and the mark is written after the interception cohort.
    // Uncanny Dodge and Deflect Missiles run below;
    // both are reactive damage-reducers that don't cancel the hit
    // itself, so the mark IS written even if damage lands as zero.
    // Written even if the target doesn't currently hold the tag; the
    // tag is read on the penalty side (cheap set-insert vs. the
    // passive-feature lookup makes this the simpler ordering).
    if let Some(attacker) = encounter.actors.get_mut(&p.caster_id) {
        attacker.mark_hit_target_this_turn(p.target_id);
    }
    // 5e Fighting Style: **Great Weapon Fighting** — reroll any 1 / 2 on
    // a melee weapon damage die once, taking the new value even if it
    // comes up 1 or 2 again per RAW. Gated on `p.is_melee` so a longbow
    // shot (or a spell attack routed through this chokepoint) doesn't
    // pick up the reroll — the RAW "two-handed melee weapon" gate
    // collapses to "melee weapon attack" since the engine doesn't track
    // weapon-hand-usage (same shape as Dueling's gate collapse). Applied
    // to BOTH the base damage roll AND the crit's doubled dice so the
    // per-die reroll fires uniformly across the swing's dice pool. The
    // helper's non-GWF fast path is a single delegated `roll(&dice)` so
    // the vast majority of swings pay no extra cost.
    let apply_gwf = p.is_melee
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_great_weapon_fighting());
    // Spell attacks that come through this chokepoint (Fire Bolt, Ray
    // of Frost, the two monster spell-attacks) roll their base damage
    // through the caster-aware spell chokepoint instead, so the
    // Sorcerer's Empowered Spell reroll, the Evocation Wizard's
    // Empowered Evocation and the cleric's Potent Spellcasting reach
    // them — RAW names Fire Bolt's own school in Empowered Evocation's
    // text, and a Knowledge Cleric's Sacred Flame collecting +WIS while
    // the same cleric's Thorn Whip didn't was the tell that this lane
    // had been missed. Great Weapon Fighting is deliberately not
    // considered on that branch: RAW gates it to melee weapons, and
    // `apply_gwf` is already false for every spell here.
    let raw_damage = if p.is_spell {
        encounter.roll_empowered_sum(p.caster_id, p.damage_dice.count, p.damage_dice.faces) as i32
    } else {
        encounter.roll_weapon_damage_dice(p.damage_dice, apply_gwf) as i32
    };
    let crit_extra = if is_crit {
        encounter.roll_weapon_damage_dice(p.damage_dice, apply_gwf) as i32
    } else {
        0
    };
    // 5e Brutal Critical (Barbarian level 9 / 13 / 17) + Half-Orc
    // Savage Attacks: both add extra weapon damage dice on a critical
    // melee hit. Spell attacks don't qualify — gated on `is_melee` +
    // `is_crit`. Read from the shared `CRIT_MELEE_EXTRA_DICE_SOURCES`
    // table — each entry is a (label, dice_count_fn) pair; the fn
    // reads the caster's template flag / dice-count field and returns
    // the number of extra weapon-face dice to roll (0 = skip). A
    // level-17 half-orc barbarian rolls 3 (Brutal Critical) + 1
    // (Savage Attacks) = 4 extra dice without touching this site.
    // Each rider logs separately so the source of the extra dice is
    // legible in the combat log. A new crit-extra-dice source (Piercer
    // feat's +1 die, a hypothetical Champion "Superior Critical"
    // bonus die) drops in as a new tuple rather than another
    // if-block copy.
    let brutal_extra = if is_crit && p.is_melee {
        let mut total = 0;
        for (label, count_fn) in CRIT_MELEE_EXTRA_DICE_SOURCES {
            let Some(a) = encounter.actors.get(&p.caster_id) else { break; };
            let count = count_fn(a);
            if count == 0 {
                continue;
            }
            let extra_dice = Dice::new(count, p.damage_dice.faces);
            let rolled = encounter.roll(&extra_dice) as i32;
            encounter.log(format!(
                "  {}: +{}({}) = +{} {:?}",
                label, extra_dice, rolled, rolled, p.damage_type
            ));
            total += rolled;
        }
        total
    } else {
        0
    };
    // Fold in the caster-side flat damage bonuses: item-passive
    // (`+1 Weapon`, Bracers of Archery) and spell-installed
    // (Magic Weapon, Elemental Weapon). Added once per swing — crits
    // already double the dice but the modifier (action's
    // `damage_bonus` + caster damage buff) is added once per RAW.
    // Picked up at this site so weapon AND spell attacks both see
    // the bonus through the same chokepoint.
    //
    // `curse_damage_bonus` rides alongside as the target-scoped half —
    // a hexblade's proficiency bonus against the one creature they
    // cursed. It lives outside `caster_damage_buffs` because that
    // helper takes no target, and this bonus is defined by who is on
    // the receiving end. The spell-attack chokepoint in
    // `spells::spell_attack_outcome` sums the same pair.
    let caster_damage_buff = encounter.caster_damage_buffs(p.caster_id)
        + encounter.curse_damage_bonus(p.caster_id, p.target_id);
    let total_damage_bonus = p.damage_bonus + caster_damage_buff;
    let mut damage = (raw_damage + crit_extra + brutal_extra + total_damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            p.action_name,
            p.damage_dice,
            raw_damage,
            p.damage_dice,
            crit_extra,
            total_damage_bonus,
            damage,
            p.damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            p.action_name, p.damage_dice, raw_damage, total_damage_bonus, damage, p.damage_type,
        ));
    }
    // Caster-side flat melee-only bumps. Read from the
    // `MELEE_CASTER_BUMPS` table — each entry is a (label,
    // amount_fn) pair; the amount_fn reads the caster's
    // features/conditions and returns the flat bonus (0 = skip). A
    // new melee-side bump (Ancestral Guardians retribution, Rage
    // tier-scaling to +3/+4, etc.) drops in as a new tuple rather
    // than another `if p.is_melee && …` block. Order matters only
    // for legibility (all entries stack additively on holders that
    // carry multiple flags).
    if p.is_melee {
        for (label, amount_fn) in MELEE_CASTER_BUMPS {
            let Some(a) = encounter.actors.get(&p.caster_id) else { break; };
            let bump = amount_fn(a);
            if bump == 0 {
                continue;
            }
            damage = damage.saturating_add(bump);
            encounter.log(format!("  {}: +{} melee damage", label, bump));
        }
    }
    // Hunter's Mark rider: attacker concentrating on Hunter's Mark with
    // this target marked deals +1d6 (weapon-typed). Crits double the
    // mark die per RAW — the rider folds into the weapon's damage type.
    if encounter.is_hunters_mark_target(p.caster_id, p.target_id) {
        let hm_total = roll_rider(encounter, Dice::new(1, 6), is_crit);
        damage = damage.saturating_add(hm_total);
        encounter.log(format!(
            "  hunter's mark: +{} extra {:?}",
            hm_total, p.damage_type
        ));
    }
    // Target-side and ally-side reactive damage clamps — Uncanny Dodge
    // (Rogue lv5), Deflect Missiles (Monk lv3), Parry (Battle Master
    // maneuver), Fighting Style: Interception. All four are rows on the
    // shared `REACTIVE_DAMAGE_CLAMPS` cohort; the walker owns the
    // per-row lane gate (weapon vs spell, melee vs ranged), the
    // reactor pick (self vs adjacent ally), the roll, the log, and the
    // reaction / per-rest-charge spend. `is_spell: false` here — this
    // chokepoint only ever resolves weapon swings (including the
    // weapon-shaped attack cantrips, which are engine-tagged as melee
    // weapon attacks), so Deflect Missiles' ranged-weapon lane admits
    // ranged swings that arrive here. The spell-attack chokepoint in
    // `spells::spell_attack_outcome` calls the same walker with
    // `is_spell: true`.
    // Attacker-scoped reductions land before the reactive clamps so a
    // blunted blow is what Uncanny Dodge then halves - the same order
    // RAW resolves resistance and damage-reduction reactions in, and
    // the order that keeps a doubly-protected ally from taking more
    // than a singly-protected one.
    damage = attacker_scoped_damage_reduction(encounter, p.caster_id, p.target_id, damage);
    damage = apply_reactive_damage_clamps(
        encounter,
        p.caster_id,
        p.target_id,
        damage,
        p.is_melee,
        false,
        p.damage_type,
    );
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
        actor_id: p.target_id,
        amount: damage,
        damage_type: p.damage_type,
    })];
    if encounter.is_hex_target(p.caster_id, p.target_id) {
        push_die_rider(
            encounter,
            &mut effects,
            p.target_id,
            Dice::new(1, 6),
            is_crit,
            DamageType::Necrotic,
            "hex",
        );
    }
    // Caster-side per-hit damage riders. Generalized so any condition
    // that grants "+Xdy damage of type T on hit" plugs in here without
    // re-implementing the attack-roll-to-damage glue. Gates:
    //   - `melee_only`: skip on ranged attacks (5e Smite spells / Divine
    //     Smite RAW: melee weapon only).
    //   - `consume_on_trigger`: strip the condition after the rider
    //     lands (one-shot primes like Divine Smite / the four Smite
    //     spells; persistent aura-style riders like Crusader's Mantle
    //     and Crown of Stars leave their condition in place for the
    //     full spell duration).
    //   - `follow_up`: optional secondary clause that fires only on the
    //     swing that *consumed* the rider — used by Blinding Smite (CON
    //     save or Blinded) and Wrathful Smite (WIS save or Frightened).
    for rider in ON_HIT_RIDERS.iter().copied() {
        if rider.melee_only && !p.is_melee {
            continue;
        }
        if rider.ranged_only && p.is_melee {
            continue;
        }
        if !encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_condition(rider.condition))
        {
            continue;
        }
        // Roll the rider damage only if the rider actually has dice —
        // primes whose entire effect is the follow-up (Stunning Strike:
        // no damage, just a stun save) declare 0 dice so the damage
        // line and the DealDamage push are skipped. Capture the rolled
        // total so the follow-up's optional `hp_threshold` gate can
        // predict the target's post-damage HP without re-rolling.
        let rider_total = if rider.dice.count > 0 {
            let total = roll_rider(encounter, rider.dice, is_crit);
            encounter.log(format!(
                "  {}: +{} {:?}",
                rider.label, total, rider.damage_type
            ));
            effects.push(Box::new(DealDamage {
                actor_id: p.target_id,
                amount: total,
                damage_type: rider.damage_type,
            }));
            total
        } else {
            0
        };
        if rider.consume_on_trigger
            && let Some(caster) = encounter.actors.get_mut(&p.caster_id)
        {
            caster.remove_condition(rider.condition);
        }
        if let Some(follow) = rider.follow_up {
            apply_smite_follow_up(
                encounter,
                &mut effects,
                p.caster_id,
                p.target_id,
                follow,
                rider_total + damage,
            );
        }
    }
    // 5e Paladin Improved Divine Smite (level 11+) — passive feature.
    // Every melee weapon hit lays +1d8 radiant on the target, independent
    // of any Smite-prime burn. Lives outside the ON_HIT_RIDERS table
    // because it's keyed off a passive feature flag rather than a
    // transient condition, and we don't want a sentinel "always-on"
    // condition cluttering the conditions enum just to gate this one
    // rider. Crits double the die per `roll_rider` RAW.
    if p.is_melee
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::IMPROVED_DIVINE_SMITE_TAG,
                )
            })
    {
        push_die_rider(
            encounter,
            &mut effects,
            p.target_id,
            Dice::new(1, 8),
            is_crit,
            DamageType::Radiant,
            "improved divine smite",
        );
    }
    // Once-per-turn weapon-hit +die riders on the shared
    // `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort. Every row is a passive-
    // feature tag on the caster whose `has_passive_feature` gate, once-
    // per-turn ledger, per-feature target gate, per-feature damage-type
    // resolver, and log line all fold through the shared
    // `try_fire_once_per_turn_weapon_die_rider` chokepoint — the "check
    // flag → check ledger → check target gate → push_die_rider → mark
    // used" scaffolding lives in one place. Adding a future rider on
    // the "one die + one damage type + one optional target gate" corner
    // (Colossus Slayer / Dreadful Strikes / Psychic Blades / Planar
    // Warrior all sit here today) lands as one row on the cohort table
    // without touching this loop.
    for spec in ONCE_PER_TURN_WEAPON_DIE_RIDERS {
        try_fire_once_per_turn_weapon_die_rider(encounter, &mut effects, &p, is_crit, spec);
    }
    // 5e Ranger **Foe Slayer** (level 20 capstone) — passive once-per-
    // turn rider. On any weapon hit, add the ranger's Wisdom modifier
    // as flat damage of the weapon's damage type. Two gates:
    //   1. Caster has the FOE_SLAYER_TAG passive feature flag.
    //   2. Caster hasn't already fired Foe Slayer this turn
    //      (`foe_slayer_used` — cleared at turn-start by
    //      `reset_for_new_round`).
    // No melee gate (RAW: "an attack you make" — covers longbow shots).
    // The rider is a flat modifier, not a die, so it doesn't double on
    // a crit per RAW (crit-doubling applies to dice, not flat mods).
    // Fires only when the WIS modifier is positive — a WIS-dump ranger
    // (rare) reads no bonus rather than adding a penalty.
    if !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::FOE_SLAYER_TAG,
                ) && !a.foe_slayer_used()
            })
    {
        let wis_mod = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| {
                crate::engine::util::modifier_from_score(
                    a.ability_score(AbilityScoreType::Wisdom),
                )
            })
            .unwrap_or(0);
        if wis_mod > 0 {
            let extra = wis_mod as u32;
            encounter.log(format!(
                "  foe slayer: +{} {:?}",
                extra, p.damage_type
            ));
            effects.push(Box::new(DealDamage {
                actor_id: p.target_id,
                amount: extra,
                damage_type: p.damage_type,
            }));
            if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
                caster.mark_foe_slayer_used();
            }
        }
    }
    // 5e Barbarian Path of the Zealot **Divine Fury** (level 3) — passive
    // once-per-turn rider. The first weapon hit each turn while raging
    // lays +1d6 + half barbarian level (min +1) radiant damage. Three
    // gates:
    //   1. Not a spell attack (RAW: "with a weapon attack").
    //   2. Caster has the DIVINE_FURY_TAG passive feature flag AND is
    //      currently Raging (RAW: "while you're raging").
    //   3. Caster hasn't already fired Divine Fury this turn
    //      (`divine_fury_used` — cleared at turn-start by
    //      `reset_for_new_round`).
    // Crits double the die per `roll_rider` RAW; the +level/2 flat mod
    // doesn't double. Damage is typed Radiant — RAW gives the zealot a
    // choice of radiant or necrotic, we lock to radiant so the "holy
    // warrior" tell stays visible on the log line.
    if !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::DIVINE_FURY_TAG,
                ) && a.has_condition(Condition::Raging)
                    && !a.divine_fury_used()
            })
    {
        let half_level = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| (a.level() / 2).max(1))
            .unwrap_or(1);
        let die = roll_rider(encounter, Dice::new(1, 6), is_crit);
        let total = die + half_level;
        encounter.log(format!(
            "  divine fury: +{} (1d6({})+{}) Radiant",
            total, die, half_level
        ));
        effects.push(Box::new(DealDamage {
            actor_id: p.target_id,
            amount: total,
            damage_type: DamageType::Radiant,
        }));
        if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
            caster.mark_divine_fury_used();
        }
    }
    // Passive weapon-hit condition marks — Eldritch Strike (Eldritch
    // Knight lv10) and Unwavering Mark (Cavalier lv3). Both are rows on
    // the shared `ON_HIT_CONDITION_MARKS` cohort; the walker owns the
    // spell / melee lane gates, the tag check, the log, and the
    // condition-plus-back-link push.
    push_on_hit_condition_marks(encounter, &mut effects, &p);
    // Melee retaliation: any condition the *target* holds that bounces
    // damage back at a melee attacker, plus their creature-intrinsic
    // reflect. Shared with the spell-attack path via
    // `push_melee_reflect_riders` — RAW's "hits you with a melee attack"
    // covers a melee spell attack.
    if p.is_melee {
        push_melee_reflect_riders(encounter, &mut effects, p.caster_id, p.target_id);
    }
    // Retaliation whose RAW trigger is any connecting attack rather than
    // a melee one — Scornful Rebuke and future siblings. No `is_melee`
    // gate: that gate is the only thing separating this lane from the
    // one directly above.
    push_any_attack_reflect_riders(encounter, &mut effects, p.caster_id, p.target_id);
    // The action's own extra clause, last so it reads the finished
    // swing. Its damage folds into the returned figure so callers that
    // chain off `damage_dealt` (a half-damage self-heal, a max-HP drain)
    // see the whole hit.
    damage = damage.saturating_add(rider.apply(encounter, &p, mode, is_crit, &mut effects));
    (effects, damage)
}

/// Queue every melee retaliation payload a landed melee attack earns the
/// *target* against their attacker: first the condition-keyed reflect
/// table (Fire Shield 2d8 fire, Armor of Agathys 5 cold, Investiture of
/// Flame 1d10 fire), then the creature-intrinsic reflect (Black Pudding
/// Corrosive Form, Salamander Heated Body). The two lanes compose
/// additively — a salamander wearing Fire Shield rolls both on the same
/// incoming swing.
///
/// Reflected damage goes through the standard `DealDamage` pipeline, so
/// the attacker's own typed immunity / resistance / vulnerability is
/// honored.
///
/// Shared by both attack chokepoints. Every source here triggers on RAW's
/// "hits you with a melee attack", which a melee *spell* attack
/// (Vampiric Touch, Shocking Grasp, Inflict Wounds) satisfies — so the
/// caller gates only on `is_melee`, never on weapon-vs-spell.
pub fn push_melee_reflect_riders(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    attacker_id: usize,
    target_id: usize,
) {
    for rider in MELEE_REFLECT_RIDERS.iter().copied() {
        if !encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.has_condition(rider.condition))
        {
            continue;
        }
        push_reflect_damage(
            encounter,
            effects,
            attacker_id,
            rider.damage,
            rider.damage_type,
            rider.label,
        );
    }
    if let Some(natural) = encounter
        .actors
        .get(&target_id)
        .and_then(|a| a.natural_melee_reflect())
    {
        push_reflect_damage(
            encounter,
            effects,
            attacker_id,
            natural.damage,
            natural.damage_type,
            natural.label,
        );
    }
}

/// Roll (or read flat) the reflect amount, log the reflection, and queue
/// the `DealDamage` payload against the original attacker. Shared
/// chokepoint for the condition-keyed reflect table and the creature-
/// intrinsic natural reflect lane so the log shape stays uniform across
/// both sources.
fn push_reflect_damage(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    attacker_id: usize,
    damage: ReflectDamage,
    damage_type: DamageType,
    label: &str,
) {
    let amount = match damage {
        ReflectDamage::Flat(n) => {
            encounter.log(format!("  {}: {} {:?} reflected", label, n, damage_type));
            n
        }
        ReflectDamage::Dice(dice) => {
            let rolled = encounter.roll(&dice);
            encounter.log(format!(
                "  {}: {}({}) {:?} reflected",
                label, dice, rolled, damage_type
            ));
            rolled
        }
    };
    effects.push(Box::new(DealDamage {
        actor_id: attacker_id,
        amount,
        damage_type,
    }));
}

/// Target-side retaliation whose trigger is RAW's *"whenever a creature
/// hits you with an attack"* — no melee clause — and whose amount is
/// read off the holder rather than rolled.
///
/// The third reflect lane, and the one the other two couldn't express.
/// `MELEE_REFLECT_RIDERS` keys on a condition and fires only on melee;
/// `MeleeReflect` keys on the creature's body and also fires only on
/// melee. The Conquest Paladin's Scornful Rebuke is neither: it is a
/// permanent class feature (so no condition), it answers arrows and
/// spell attacks as readily as swords (so no melee gate), and its
/// damage is the paladin's Charisma modifier (so no die).
///
/// `amount` returning 0 skips the row silently, which is what makes the
/// "minimum of 1" clauses expressible in the row rather than in the
/// walker.
pub struct AnyAttackReflect {
    /// Does this holder carry the feature? Reads the actor rather than
    /// a tag so a row can gate on a template flag, a passive feature
    /// tag, or a condition — whichever the feature actually stores.
    pub holds: fn(&crate::actors::actor_template::ActorInstance) -> bool,
    /// How much the holder bounces back. Reads the actor because every
    /// feature on this lane scales with one of their ability
    /// modifiers.
    pub amount: fn(&crate::actors::actor_template::ActorInstance) -> u32,
    pub damage_type: DamageType,
    /// Log-friendly tag ("scornful rebuke", ...).
    pub label: &'static str,
}

/// The any-attack reflect table. One row today; the shape exists
/// because the alternative was a bespoke block at each of the two
/// attack chokepoints, and this engine has already learned twice over
/// what that costs (see `MELEE_CASTER_BUMPS`, `REACTIVE_DAMAGE_CLAMPS`).
///
/// Every row is gated on the holder being conscious and not
/// incapacitated — see `push_any_attack_reflect_riders`. RAW writes
/// that clause into Scornful Rebuke explicitly, and it is the right
/// default for the lane: a retaliation the holder is not awake to
/// deliver shouldn't fire.
const ANY_ATTACK_REFLECT_FEATURES: &[AnyAttackReflect] = &[
    // 5e Conquest Paladin **Scornful Rebuke** (subclass level 15).
    // "Whenever a creature hits you with an attack, that creature takes
    // psychic damage equal to your Charisma modifier (minimum of 1) if
    // you're not incapacitated."
    AnyAttackReflect {
        holds: |a| a.has_scornful_rebuke(),
        amount: |a| a.ability_modifier(AbilityScoreType::Charisma).max(1) as u32,
        damage_type: DamageType::Psychic,
        label: "scornful rebuke",
    },
];

/// Queue every `ANY_ATTACK_REFLECT_FEATURES` payload a landed attack
/// earns the target against their attacker, melee or not.
///
/// Sibling to `push_melee_reflect_riders` and called from the same two
/// chokepoints, but unconditionally — the melee gate is exactly the
/// difference between the two lanes.
pub fn push_any_attack_reflect_riders(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    attacker_id: usize,
    target_id: usize,
) {
    for row in ANY_ATTACK_REFLECT_FEATURES {
        let amount = match encounter.actors.get(&target_id) {
            // RAW's "if you're not incapacitated" clause, applied to the
            // whole lane. A downed or stunned holder retaliates with
            // nothing.
            Some(t) if t.is_combat_active() && !t.is_incapacitated() && (row.holds)(t) => {
                (row.amount)(t)
            }
            _ => continue,
        };
        if amount == 0 {
            continue;
        }
        push_reflect_damage(
            encounter,
            effects,
            attacker_id,
            ReflectDamage::Flat(amount),
            row.damage_type,
            row.label,
        );
    }
}

/// Damage payload for a melee retaliation rider. Some shields roll dice
/// (Fire Shield 2d8, Investiture of Flame 1d10); others deal flat damage
/// (Armor of Agathys 5). Captured as an enum so the rider table stays a
/// flat array of plain-data entries.
#[derive(Clone, Copy)]
pub enum ReflectDamage {
    Dice(Dice),
    Flat(u32),
}

/// Plain-data melee reflect descriptor, used by creature-intrinsic
/// retaliation features (Black Pudding Corrosive Form, Salamander
/// Heated Body). Shares the `damage` / `damage_type` / `label` shape
/// with `MeleeReflectRider` but doesn't carry a condition key — the
/// holder's body itself is the trigger, no transient buff needed.
/// Const-constructible so templates can declare it inline.
#[derive(Clone, Copy)]
pub struct MeleeReflect {
    pub damage: ReflectDamage,
    pub damage_type: DamageType,
    /// Log-friendly tag ("corrosive form", "heated body", ...).
    pub label: &'static str,
}

/// A target-side "creature hit me in melee, take this damage back" rider.
/// Mirrors `OnHitRider` in shape but lives on the *target* of the swing
/// rather than the caster: any actor who holds `condition` reflects
/// `damage` of `damage_type` onto every melee attacker that connects.
#[derive(Clone, Copy)]
pub struct MeleeReflectRider {
    pub condition: Condition,
    pub damage: ReflectDamage,
    pub damage_type: DamageType,
    /// Log-friendly tag ("fire shield", "armor of agathys", ...).
    pub label: &'static str,
}

/// Melee retaliation rider table. Symmetric with `ON_HIT_RIDERS` but
/// consumed at the target side of `resolve_attack_outcome`. `Dice::new`
/// is a const fn so the whole table lives as a `const &[..]` — adding
/// a rider doesn't bump a hardcoded length.
const MELEE_REFLECT_RIDERS: &[MeleeReflectRider] = &[
    // 5e Fire Shield — 2d8 fire on every melee contact. Concentration-
    // free, self-only; the warm / cool variant only matters for the
    // resistance lane (we collapse to a single Fire Shield condition).
    MeleeReflectRider {
        condition: Condition::FireShielded,
        damage: ReflectDamage::Dice(Dice::new(2, 8)),
        damage_type: DamageType::Fire,
        label: "fire shield",
    },
    // 5e Armor of Agathys — flat 5 cold on melee contact. We don't
    // scale with slot level (the spell's install site sets the temp
    // HP buffer instead). Mirrors Fire Shield's shape but cheaper.
    MeleeReflectRider {
        condition: Condition::AgathysShielded,
        damage: ReflectDamage::Flat(5),
        damage_type: DamageType::Cold,
        label: "armor of agathys",
    },
    // 5e Investiture of Flame — 1d10 fire on melee contact. Smaller
    // die than Fire Shield (concentration-bound on the caster, paired
    // with the broader fire resistance baked into the install).
    MeleeReflectRider {
        condition: Condition::InvestedInFlame,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Fire,
        label: "investiture of flame",
    },
    // 5e Investiture of Ice — 1d10 cold on melee contact. Symmetric to
    // Investiture of Flame: same die / shape, swapped element. Paired
    // with cold resistance on the caster via the InvestedInIce branch
    // of `effective_damage`.
    MeleeReflectRider {
        condition: Condition::InvestedInIce,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Cold,
        label: "investiture of ice",
    },
    // 5e Investiture of Stone — 1d10 force on melee contact. Sibling to
    // Investiture of Flame / Ice but force-typed (the stone shell
    // crackles with telekinetic recoil rather than burning / freezing
    // the attacker). Paired with broad physical resistance on the
    // caster via the InvestedInStone row of TYPED_RESISTANCE_CONDITIONS.
    MeleeReflectRider {
        condition: Condition::InvestedInStone,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Force,
        label: "investiture of stone",
    },
    // 5e Shadow of Moil — 2d8 necrotic on melee contact. The clinging
    // shadows lash out at anyone who reaches into them. Concentration-
    // bound on the caster (paired with the attacker-disadvantage half
    // via `imposes_disadvantage_to_attackers`). Same shape as Fire
    // Shield, swapped element — necrotic-typed.
    MeleeReflectRider {
        condition: Condition::MoilShrouded,
        damage: ReflectDamage::Dice(Dice::new(2, 8)),
        damage_type: DamageType::Necrotic,
        label: "shadow of moil",
    },
];

/// Roll a single rider die for an on-hit bonus, doubling on crit per
/// 5e RAW. Used by every "per-hit weapon-bonus damage" effect — Hex /
/// Hunter's Mark (1d6), Crusader's Mantle (1d4), Crown of Stars (1d8).
/// Keeps the crit-doubling rule in one place. Public so the spell-attack
/// resolver can share the same crit-doubling rule for Hex.
pub fn roll_rider(encounter: &mut EncounterInstance, dice: Dice, is_crit: bool) -> u32 {
    let base = encounter.roll(&dice);
    let crit_extra = if is_crit { encounter.roll(&dice) } else { 0 };
    base + crit_extra
}

/// Roll a single-die on-hit rider (e.g. Improved Divine Smite, Colossus
/// Slayer, Hex), log it, and queue the resulting `DealDamage` payload
/// against `target_id`. Returns the rolled amount so callers needing the
/// raw value (Hex composes the post-damage total into its `damage`
/// running tally) can read it off the same chokepoint.
///
/// Collapses the recurring `roll_rider → log → push DealDamage` triple
/// that several on-hit features open-coded. Keeps the log shape uniform
/// across rider sources so a future grep / log-scanning test reads from
/// one shape.
pub fn push_die_rider(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    target_id: usize,
    dice: Dice,
    is_crit: bool,
    damage_type: DamageType,
    label: &str,
) -> u32 {
    let extra = roll_rider(encounter, dice, is_crit);
    encounter.log(format!("  {}: +{} {:?}", label, extra, damage_type));
    effects.push(Box::new(DealDamage {
        actor_id: target_id,
        amount: extra,
        damage_type,
    }));
    extra
}

/// Spec for a once-per-turn weapon-hit +XdY typed die rider, consumed by
/// `try_fire_once_per_turn_weapon_die_rider`. Bundles the four per-
/// feature axes (tag / dice / damage type / label) plus the optional
/// target-side gate into a single struct so a callsite drops from the
/// nine-arg raw signature (encounter + effects + params + is_crit +
/// tag + dice + damage_type + label + target_gate) down to a clean
/// five-arg call whose per-feature configuration reads as a labeled
/// struct literal — matching the "spec struct per shared engine
/// chokepoint" shape the sibling `AttackParams`, `MeleeReflectRider`,
/// and `OnHitRider` shapes already use in this module.
pub struct OncePerTurnWeaponRiderSpec {
    /// Passive-feature tag the rider keys off — both the "does the
    /// caster carry this rider" check (`has_passive_feature(tag)`) and
    /// the "has the caster already fired it this turn" ledger key
    /// (`once_per_turn_used(tag)` / `mark_once_per_turn_used(tag)`) —
    /// both read/write through the shared `ONCE_PER_TURN_RIDER_TAGS`
    /// cohort on `ActorInstance`.
    pub tag: &'static str,
    /// Die to roll for the rider (Colossus Slayer 1d8, Dreadful
    /// Strikes 1d4). Crits double the die via the shared
    /// `push_die_rider` chokepoint per 5e RAW.
    pub dice: Dice,
    /// Per-feature damage-type resolver. Colossus Slayer passes
    /// `|p| p.damage_type` (weapon-typed — the rider inherits the
    /// swing's damage type); Dreadful Strikes / Psychic Blades pass
    /// `|_| DamageType::Psychic` — RAW's "whisper dread" / "infuse
    /// with whispers" tell fixes the type regardless of the weapon's
    /// base type. Held as a function-pointer so a future rider that
    /// conditions its damage type on some attack-side axis (a
    /// hypothetical "matches weapon type unless the weapon is Force,
    /// then falls back to Radiant" edge case) lands as another closure
    /// here without widening the helper.
    pub damage_type: fn(&AttackParams) -> DamageType,
    /// Log label — "colossus slayer", "dreadful strikes". Formatted by
    /// `push_die_rider` as `"  {label}: +{extra} {damage_type:?}"` so
    /// the log shape stays uniform across every once-per-turn rider
    /// routed through this helper.
    pub label: &'static str,
    /// Per-feature target-side gate. Colossus Slayer passes
    /// `|t| t.is_wounded()` — RAW gates on the target's current HP
    /// being below max; Dreadful Strikes passes `|_| true` — RAW fires
    /// against any target, wounded or not. A future rider with a
    /// target-condition gate (a hypothetical "extra damage vs Prone /
    /// Frightened targets" subclass rider) lands as another closure
    /// here without widening the helper.
    pub target_gate: fn(&ActorInstance) -> bool,
}

/// Fire a once-per-turn weapon-hit +XdY typed die rider — the shared shape
/// behind Colossus Slayer (Hunter Ranger lv3, +1d8 weapon-typed, wounded-
/// target gate) and Dreadful Strikes (Fey Wanderer Ranger lv3, +1d4
/// Psychic, no target gate). Both features live at the "no flat bonus, no
/// compound log line, one die + one damage type + one optional target
/// gate" corner of the once-per-turn rider design space; folding both
/// through this helper drops the recurring 4-line "check flag → check
/// ledger → check target gate → push_die_rider → mark used" scaffolding
/// per feature down to a single call site.
///
/// Returns `true` if the rider fired (log + `DealDamage` queued + ledger
/// flipped), `false` otherwise. Six short-circuit gates:
///   1. `p.is_spell` — rider is weapon-only (RAW: "with a weapon attack").
///   2. Caster missing → dead / removed attacker, no-op.
///   3. Caster lacks the passive-feature `spec.tag`.
///   4. Caster already fired this `spec.tag` this turn (per the shared
///      `once_per_turn_used(tag)` ledger cleared at `reset_for_new_round`).
///   5. Target missing → wildcard target lookup failed, no-op.
///   6. `spec.target_gate(target)` returns false — the per-rider RAW-side
///      constraint (Colossus Slayer's `is_wounded()`; Dreadful Strikes'
///      always-true).
///
/// Distinct from `push_die_rider` (the standalone log + push chokepoint)
/// — this wrapper adds the two lookup-gate + one ledger-write bookends
/// that every once-per-turn rider shares. Distinct from the Foe Slayer /
/// Divine Fury blocks in `resolve_attack_outcome` — those two riders
/// have flat-bonus / compound-log shapes that don't fit this helper's
/// "die only, uniform log" corner and stay open-coded on their own
/// gates for now.
pub fn try_fire_once_per_turn_weapon_die_rider(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    p: &AttackParams,
    is_crit: bool,
    spec: &OncePerTurnWeaponRiderSpec,
) -> bool {
    if p.is_spell {
        return false;
    }
    let caster_ok = encounter.actors.get(&p.caster_id).is_some_and(|a| {
        a.has_passive_feature(spec.tag) && !a.once_per_turn_used(spec.tag)
    });
    if !caster_ok {
        return false;
    }
    let target_ok = encounter
        .actors
        .get(&p.target_id)
        .is_some_and(spec.target_gate);
    if !target_ok {
        return false;
    }
    push_die_rider(
        encounter,
        effects,
        p.target_id,
        spec.dice,
        is_crit,
        (spec.damage_type)(p),
        spec.label,
    );
    if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
        caster.mark_once_per_turn_used(spec.tag);
    }
    true
}

/// Shared cohort of once-per-turn weapon-hit die-riders — the "one die +
/// one damage type + one optional target gate" corner of the on-hit
/// rider design space. Consumed by `resolve_attack_outcome` right after
/// the Improved Divine Smite block: each row is a passive-feature tag
/// that, if held by the caster and not yet fired this turn, lays its
/// die onto the swing. Every row folds through the shared
/// `try_fire_once_per_turn_weapon_die_rider` helper so the "check flag →
/// check ledger → check target gate → push_die_rider → mark used"
/// scaffolding lives in one place.
///
/// Adding a future subclass rider on the same corner (a hypothetical
/// "extra damage vs Frightened targets" archery-conclave pick, a
/// per-turn magical-strike rider, ...) lands as a fresh
/// `PLANAR_WARRIOR`-shaped `_TAG` const plus one row here — no touching
/// `resolve_attack_outcome`. Distinct from the Foe Slayer / Divine Fury
/// blocks in `resolve_attack_outcome` — those two riders have flat-
/// bonus / compound-log shapes that don't fit this cohort's "die only,
/// uniform log" corner and stay open-coded on their own gates for now.
///
/// Entries (each is one `has_passive_feature(TAG)` gated once-per-turn
/// weapon-hit rider — see the linked tag docstring for the per-feature
/// RAW rationale):
///   - **Colossus Slayer** (Hunter Ranger lv3): +1d8 weapon-typed,
///     wounded-target gate.
///   - **Dreadful Strikes** (Fey Wanderer Ranger lv3, TCE): +1d4
///     Psychic, no target gate.
///   - **Psychic Blades** (Whispers Bard lv3, XGtE): +1d6 Psychic, no
///     target gate.
///   - **Planar Warrior** (Horizon Walker Ranger lv3, XGtE): +1d8
///     Force, no target gate. Force is one of the rarest-resisted
///     damage types in the engine — the Horizon Walker's rider punches
///     through nearly every typed-defense lane cleanly.
///   - **Slayer's Prey** (Monster Slayer Ranger lv3, XGtE): +1d6
///     weapon-typed, no target gate. Sibling in shape to Colossus
///     Slayer (weapon-typed damage) but with the no-target-gate
///     collapse of Planar Warrior / Dreadful Strikes / Psychic Blades
///     — the RAW "mark target with bonus action, hit for +1d6" two-
///     step collapses to a plain once-per-turn +1d6 rider without a
///     per-target-mark ledger.
///   - **Gathered Swarm** (Swarmkeeper Ranger lv3, TCE): +1d6 Piercing,
///     no target gate. RAW's three-option choice (piercing damage /
///     STR-save shove / self-teleport) collapses to the load-bearing
///     damage-rider lane; the two movement alternatives would need a
///     per-hit optional-side-effect surface plus AI heuristics for the
///     trade, so they stay out for now. Sibling in die size to Psychic
///     Blades / Slayer's Prey but Piercing-fixed (physical, distinct
///     from the Psychic Blades / Dreadful Strikes psychic lane and the
///     Planar Warrior force lane).
///   - **Psionic Strike** (Psi Warrior Fighter lv3, TCE): +1d8 Force,
///     no target gate. Mechanically the twin of Planar Warrior — RAW's
///     psionic-die cost and flat +INT are dropped (see the tag
///     docstring); the Psi Warrior's distinguishing feature is the
///     Protective Field row on `REACTIVE_DAMAGE_CLAMPS`, not this
///     rider.
pub const ONCE_PER_TURN_WEAPON_DIE_RIDERS: &[OncePerTurnWeaponRiderSpec] = &[
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::COLOSSUS_SLAYER_TAG,
        dice: Dice::new(1, 8),
        damage_type: |p| p.damage_type,
        label: "colossus slayer",
        target_gate: |t| t.is_wounded(),
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::DREADFUL_STRIKES_TAG,
        dice: Dice::new(1, 4),
        damage_type: |_| DamageType::Psychic,
        label: "dreadful strikes",
        target_gate: |_| true,
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::PSYCHIC_BLADES_TAG,
        dice: Dice::new(1, 6),
        damage_type: |_| DamageType::Psychic,
        label: "psychic blades",
        target_gate: |_| true,
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::PLANAR_WARRIOR_TAG,
        dice: Dice::new(1, 8),
        damage_type: |_| DamageType::Force,
        label: "planar warrior",
        target_gate: |_| true,
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::SLAYERS_PREY_TAG,
        dice: Dice::new(1, 6),
        damage_type: |p| p.damage_type,
        label: "slayer's prey",
        target_gate: |_| true,
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::GATHERED_SWARM_TAG,
        dice: Dice::new(1, 6),
        damage_type: |_| DamageType::Piercing,
        label: "gathered swarm",
        target_gate: |_| true,
    },
    OncePerTurnWeaponRiderSpec {
        tag: crate::actions::class_features::PSIONIC_STRIKE_TAG,
        dice: Dice::new(1, 8),
        damage_type: |_| DamageType::Force,
        label: "psionic strike",
        target_gate: |_| true,
    },
];

/// A "+Xdy damage on hit" rider sourced from one of the caster's active
/// conditions. The rider table is consumed once per weapon hit by
/// `resolve_attack_outcome` — any caster who holds `condition` adds the
/// rolled `dice` of `damage_type` to that swing.
#[derive(Clone, Copy)]
pub struct OnHitRider {
    /// Caster-side flag the rider keys off (Smiting / Crusader's
    /// Mantled / Crown of Stars / one of the four Smite-spell primes).
    pub condition: Condition,
    pub dice: Dice,
    /// Log-friendly name ("divine smite", "searing smite", ...).
    pub label: &'static str,
    pub damage_type: DamageType,
    /// True iff the rider only fires on melee swings (every Paladin
    /// Smite, Divine Smite). Ranged carriers like Crown of Stars or
    /// Crusader's Mantle leave this false so they tag arrow hits too.
    pub melee_only: bool,
    /// True iff the rider only fires on ranged swings (5e Lightning
    /// Arrow RAW: "the next attack you make with a ranged weapon").
    /// Symmetric to `melee_only` — both default to false so the rider
    /// fires on either lane. Defaulting to false keeps every existing
    /// rider entry unchanged.
    pub ranged_only: bool,
    /// True iff the condition is stripped from the caster the moment
    /// the rider lands (one-shot primes). Persistent buffs leave this
    /// false so they stay up until the spell ends.
    pub consume_on_trigger: bool,
    /// Optional save-then-condition follow-up that fires only on the
    /// swing that consumed the rider. Powers Blinding Smite (CON save
    /// or Blinded) and Wrathful Smite (WIS save or Frightened). `None`
    /// for damage-only riders.
    pub follow_up: Option<SmiteFollowUp>,
}

/// Secondary save + effect rider tagged onto a Smite-spell hit. When
/// `save_ability` is `Some(ability)`, the target rolls that save against
/// the caster's spell DC (driven by `dc_ability`); on fail, the rider's
/// `effect` lands. When `save_ability` is `None`, the rider auto-applies
/// on hit with no save (Branding Smite's "brand", Searing Smite's ignite).
/// Stored as a value so the on-hit rider table stays a flat array of
/// plain-data entries.
#[derive(Clone, Copy)]
pub struct SmiteFollowUp {
    /// Save the target rolls (CON for Blinding Smite, WIS for Wrathful
    /// Smite). `None` skips the save entirely — the effect lands
    /// unconditionally on the consuming hit (subject to `hp_threshold`).
    pub save_ability: Option<AbilityScoreType>,
    /// Ability whose mod feeds the caster's spell save DC. Ignored when
    /// `save_ability` is `None` (no save means no DC).
    pub dc_ability: AbilityScoreType,
    /// What lands on a failed save: a condition apply, a push, or both.
    pub effect: FollowUpEffect,
    /// Log-friendly tag ("blinding smite blind", "wrathful smite fear",
    /// "pushing attack push").
    pub label: &'static str,
    /// Optional post-damage HP gate. Banishing Smite RAW: the rider lands
    /// only "if this damage reduces the target to 50 hp or fewer." We
    /// fold this into the smite follow-up site by predicting post-damage
    /// HP as `current_hp - rider_damage` and gating the apply on that.
    /// `None` (the default) skips the gate.
    pub hp_threshold: Option<u32>,
}

/// What a smite follow-up does on a failed save (or auto-trigger).
/// Most riders apply a single condition (`Condition`); the Battle Master
/// Pushing Attack maneuver shoves the target backward instead, and the
/// Sweeping Attack maneuver splashes damage onto an adjacent enemy.
#[derive(Clone, Copy)]
pub enum FollowUpEffect {
    /// Apply a single condition with the given timer. Used by every
    /// Smite-spell rider that produces a debuff (Blinded, Frightened,
    /// Prone, Stunned, Outlined, Burning, ...).
    Condition {
        condition: Condition,
        timer: ConditionTimer,
    },
    /// Push the target `tiles` away from the attacker. Used by the
    /// Battle Master Pushing Attack maneuver. No condition apply —
    /// the displacement IS the effect.
    Push { tiles: u32 },
    /// Splash damage onto one adjacent enemy of the original target.
    /// Used by the Battle Master Sweeping Attack maneuver: the swing's
    /// momentum carries through to a nearby foe for an extra die of
    /// damage. We pick the closest hostile (to the caster) that's
    /// footprint-adjacent to the target and apply the rolled `dice` as
    /// `damage_type`. No save — RAW: the original attack roll is
    /// re-used. If no adjacent enemy exists the effect is a no-op.
    Splash {
        dice: Dice,
        damage_type: DamageType,
    },
}

/// Caster-side on-hit rider table. Every per-hit damage rider that keys
/// off a caster condition (Smite spells, Crown of Stars, Crusader's
/// Mantle, persistent weapon-buff concentration spells, Battle Master
/// maneuvers, etc.) lives here. `Dice::new` is a const fn so the table
/// stays a `const &[..]` — adding a rider doesn't bump a hardcoded
/// length.
const ON_HIT_RIDERS: &[OnHitRider] = &[
        OnHitRider {
            condition: Condition::CrusadersMantled,
            dice: Dice::new(1, 4),
            label: "crusader's mantle",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::CrownOfStars,
            dice: Dice::new(1, 8),
            label: "crown of stars",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Bigby's Hand (level-5 concentration). Persistent +1d10 force
        // rider on every attack the caster lands — not melee-only since
        // the spectral hand can punch at range. Slots cleanly between
        // Crown of Stars (1d8 radiant) and the Smiting one-shot primes.
        OnHitRider {
            condition: Condition::BigbysHanded,
            dice: Dice::new(1, 10),
            label: "bigby's hand",
            damage_type: DamageType::Force,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Spirit Shroud (level-3 concentration). Persistent +1d8 cold
        // rider on every melee swing the holder lands. Mirrors Crown of
        // Stars but cold-typed and melee-only.
        OnHitRider {
            condition: Condition::SpiritShrouded,
            dice: Dice::new(1, 8),
            label: "spirit shroud",
            damage_type: DamageType::Cold,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Circle of Spores Druid **Symbiotic Entity** (subclass
        // level 2). Persistent +1d6 necrotic rider on every melee swing
        // the druid lands while the symbiote is riding them. Same shape
        // as Spirit Shroud — melee-only and non-consumed — but its
        // lifetime is bounded by the temp-HP pool the feature granted
        // rather than by concentration: `ActorInstance::take_damage`
        // strips the condition on the tick the pool empties, which is
        // RAW's "until you lose all these temporary hit points".
        OnHitRider {
            condition: Condition::SymbioticEntity,
            dice: Dice::new(1, 6),
            label: "symbiotic entity",
            damage_type: DamageType::Necrotic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Flame Arrows (XGE level-3 transmutation, concentration).
        // Persistent +1d6 fire rider on every ranged swing the holder
        // lands. Mirrors Spirit Shroud's shape but ranged-only — the
        // `ranged_only` gate folds through the rider dispatch so a melee
        // fallback can't burn the buff. Concentration-bound on the
        // caster; persistent (non-consumed) per-hit rider that drops
        // when the caster ends concentration.
        OnHitRider {
            condition: Condition::FlamingArrowed,
            dice: Dice::new(1, 6),
            label: "flame arrows",
            damage_type: DamageType::Fire,
            melee_only: false,
            ranged_only: true,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Tasha's Otherworldly Guise (TCE level-6 concentration). The
        // celestial-form flavor adds +2d6 radiant per melee weapon hit
        // (RAW: "your weapon attacks deal extra radiant damage equal to
        // your CHA mod"; we collapse to a flat 2d6 for the engine's
        // rider-table envelope, sized between Spirit Shroud's 1d8 and
        // Divine Smite's 2d8). Melee-only — the spell flavors the
        // caster's weapon, not their ranged toolkit. Persistent (non-
        // consumed) per-hit rider, concentration-bound on the caster.
        OnHitRider {
            condition: Condition::OtherworldlyGuised,
            dice: Dice::new(2, 6),
            label: "otherworldly guise",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Elemental Weapon (level-3 transmutation, concentration). The
        // weapon is sheathed in elemental energy: +1d4 fire per melee
        // weapon hit. The matching `+1 attack` half rides the standard
        // `attack_bonus_buff` lane via the Elemental Weapon spell's
        // `with_attack_buffs` ledger — kept off the OnHitRider table since
        // that's a damage-only chokepoint. Melee-only (RAW: "the next time
        // you hit a creature with this weapon," but we honor the spell's
        // melee weapon-imbue flavor by gating ranged swings out — distinct
        // from Flame Arrows which exists for the ranged lane). Persistent
        // (non-consumed); drops when the caster ends concentration.
        OnHitRider {
            condition: Condition::ElementallyWeaponed,
            dice: Dice::new(1, 4),
            label: "elemental weapon",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::Smiting,
            dice: Dice::new(2, 8),
            label: "divine smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Searing Smite — 1st-level paladin evocation, bonus action.
        // +1d6 fire on the primed hit, and the target catches fire
        // (Burning) for 3 rounds. We bake the Burning rider in as a
        // SmiteFollowUp with a permissive save (no save in RAW — the
        // target makes ongoing WIS saves to extinguish; we approximate
        // with a flat 3-round Burning).
        OnHitRider {
            condition: Condition::SearingSmiting,
            dice: Dice::new(1, 6),
            label: "searing smite",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // Auto-apply — RAW Searing Smite ignites the target on
                // hit with no save. `save_ability: None` skips the save
                // roll in `apply_smite_follow_up`.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Burning,
                    timer: ConditionTimer::Rounds(3),
                },
                label: "searing smite ignite",
                hp_threshold: None,
            }),
        },
        // 5e Wrathful Smite — 1st-level. +1d6 psychic on the primed hit;
        // target makes WIS save or is Frightened of the paladin for
        // up to 10 rounds (RAW: 1 minute).
        OnHitRider {
            condition: Condition::WrathfulSmiting,
            dice: Dice::new(1, 6),
            label: "wrathful smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "wrathful smite fear",
                hp_threshold: None,
            }),
        },
        // 5e Branding Smite — 2nd-level. +2d6 radiant; target glows
        // (Outlined for 10 rounds), giving advantage to attackers and
        // ending Invisibility / Hidden status.
        OnHitRider {
            condition: Condition::BrandingSmiting,
            dice: Dice::new(2, 6),
            label: "branding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // RAW Branding Smite is auto-apply on hit (no save).
                // `save_ability: None` skips the save roll.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Outlined,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "branding smite brand",
                hp_threshold: None,
            }),
        },
        // 5e Blinding Smite — 3rd-level. +3d8 radiant; target makes CON
        // save or is Blinded for 10 rounds.
        OnHitRider {
            condition: Condition::BlindingSmiting,
            dice: Dice::new(3, 8),
            label: "blinding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "blinding smite blind",
                hp_threshold: None,
            }),
        },
        // 5e Monk Stunning Strike — bonus action prime; on the next
        // melee hit, the target makes a CON save vs the monk's
        // WIS-based DC or is Stunned for 1 round. Zero rider dice (the
        // stun *is* the effect); the rider loop's `count > 0` guard
        // skips the damage line.
        OnHitRider {
            condition: Condition::StunningStrike,
            dice: Dice::new(0, 1),
            label: "stunning strike",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Wisdom,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "stunning strike stun",
                hp_threshold: None,
            }),
        },
        // 5e Cleric Divine Strike (level 8 class feature, here exposed as
        // a Channel-Divinity-flavored bonus-action prime). +1d8 radiant
        // on the next melee weapon hit; no save. Mirrors the Smiting
        // shape but auto-applies no follow-up condition (the radiant
        // rider IS the effect). Consumed the moment a melee swing lands.
        OnHitRider {
            condition: Condition::DivineStriking,
            dice: Dice::new(1, 8),
            label: "divine strike",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Trickery Domain Cleric Divine Strike (subclass level 8).
        // The poison-typed arm of the row above — RAW varies only the
        // damage type across the eight domains, and the rider table is
        // where that type is written down, so a domain variant is a row
        // here plus a `PrimeStrike` literal and nothing else.
        //
        // Poison is the narrowest typing on the table: every undead and
        // construct in the bestiary is immune and most fiends resist,
        // so this rider lands for nothing more often than any other.
        // Against everything else it is the same 1d8.
        OnHitRider {
            condition: Condition::DivineStrikingPoison,
            dice: Dice::new(1, 8),
            label: "divine strike (poison)",
            damage_type: DamageType::Poison,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Way of the Four Elements Monk **Fangs of the Fire Snake**
        // elemental discipline. The biggest single die on this table
        // (1d10) riding the smallest base weapon die in the engine (the
        // monk's 1d8 unarmed strike), which is the whole point: the
        // discipline buys a doubled swing with a ki point.
        //
        // RAW's other half — +10 ft of reach on the strike — is not
        // modeled; reach is per-action via `Action::reach_tiles` and
        // the only per-swing override lane is wired to the Battle
        // Master's `LungingAttacking`.
        OnHitRider {
            condition: Condition::FangsOfTheFireSnake,
            dice: Dice::new(1, 10),
            label: "fangs of the fire snake",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Trip Attack maneuver. No bonus damage in our
        // model (RAW: +superiority die damage; we skip the die since the
        // existing dice infra doesn't carry a per-class scaling pool);
        // the prone-on-fail-STR-save IS the effect. Mirrors Stunning
        // Strike's "zero-damage rider with a save follow-up" shape.
        OnHitRider {
            condition: Condition::TripAttacking,
            dice: Dice::new(0, 1),
            label: "trip attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "trip attack prone",
                hp_threshold: None,
            }),
        },
        // 5e Staggering Smite — 4th-level paladin enchantment, bonus
        // action prime. +4d6 psychic on the primed hit; target makes a
        // WIS save vs the caster's CHA-based DC or is Stunned until the
        // end of the paladin's next turn (we model as 1 round). One-shot.
        OnHitRider {
            condition: Condition::StaggeringSmiting,
            dice: Dice::new(4, 6),
            label: "staggering smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "staggering smite stun",
                hp_threshold: None,
            }),
        },
        // 5e Banishing Smite — 5th-level paladin abjuration, bonus
        // action prime. +5d10 force on the primed hit; if the target
        // ends the swing at 50 HP or fewer they are banished. We model
        // the banishment via the existing `Mazed` envelope (zero
        // movement + blocked action economy + blocked reactions) for
        // 10 rounds — distinct log line, identical end-state. The HP
        // threshold gate is evaluated at the smite-follow-up site (see
        // `apply_smite_follow_up`'s threshold extension below).
        OnHitRider {
            condition: Condition::BanishingSmiting,
            dice: Dice::new(5, 10),
            label: "banishing smite",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // No save — RAW: the banish is auto-apply if HP ≤ 50.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Mazed,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "banishing smite banish",
                // RAW: only banished "if this damage reduces the target
                // to 50 hp or fewer." We honor the gate by predicting
                // post-damage HP at the smite follow-up site.
                hp_threshold: Some(50),
            }),
        },
        // 5e Shillelagh — druid cantrip prime. The caster's club /
        // staff is wreathed in sylvan magic: the next melee weapon hit
        // deals an extra 1d8 force damage. RAW also lets the swing use
        // WIS instead of STR for the to-hit / damage roll; we skip the
        // stat-swap (the +1d8 rider IS the load-bearing buff). One-shot
        // — the rider table strips this flag the moment a melee swing
        // lands. Concentration-free per RAW (1-minute duration).
        OnHitRider {
            condition: Condition::Shillelaghed,
            dice: Dice::new(1, 8),
            label: "shillelagh",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Thunderous Smite — 1st-level paladin evocation, bonus
        // action prime. +2d6 thunder on the primed hit; target makes a
        // STR save vs the caster's CHA-based DC or is knocked Prone.
        // RAW also pushes 10 ft on the failed save; we collapse the
        // push since the smite follow-up table only carries one
        // condition apply — the prone tag is the load-bearing
        // crowd-control bit and the push helper is reserved for spells
        // whose entire effect is repositioning (Thunderwave).
        OnHitRider {
            condition: Condition::ThunderousSmiting,
            dice: Dice::new(2, 6),
            label: "thunderous smite",
            damage_type: DamageType::Thunder,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "thunderous smite prone",
                hp_threshold: None,
            }),
        },
        // 5e Enlarge / Reduce (Enlarge half) — 2nd-level transmutation,
        // concentration. The holder rolls +1d4 bonus damage on every
        // weapon attack hit (RAW: melee or ranged). Persistent — the
        // rider doesn't consume_on_trigger; it stays up until the
        // caster drops concentration. Damage type matches the weapon
        // (no fixed magical type) — we model with Bludgeoning as a
        // baseline since the engine doesn't carry per-attack weapon
        // typing into the rider table. Slots between Crusader's Mantle
        // (1d4 radiant) and Spirit Shroud (1d8 cold) in the rider table.
        OnHitRider {
            condition: Condition::Enlarged,
            dice: Dice::new(1, 4),
            label: "enlarge",
            damage_type: DamageType::Bludgeoning,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Lightning Arrow — 3rd-level ranger evocation, bonus action,
        // concentration. Primes the ranger's next ranged weapon attack
        // with +4d8 lightning. The 10-ft splash (2d8 lightning on
        // adjacent creatures, DEX save for half) is layered on by the
        // spell's `side_effects` at cast time via a follow-up burst — the
        // rider table just carries the per-hit prime damage, mirroring
        // every other Smite-style one-shot. melee_only=false so a
        // longbow swing tags it; consume_on_trigger=true so the first
        // hit consumes the prime.
        OnHitRider {
            condition: Condition::LightningArrowPrimed,
            dice: Dice::new(4, 8),
            label: "lightning arrow",
            damage_type: DamageType::Lightning,
            melee_only: false,
            ranged_only: true,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Ensnaring Strike — 1st-level ranger conjuration, bonus
        // action, concentration. Primes the ranger's next weapon
        // attack — either melee or ranged (RAW's "the next time you
        // hit a creature with a weapon attack" broad envelope) — with
        // +1d6 piercing and a STR save vs the ranger's WIS-based DC
        // gates Restrained (10 rounds) on fail. Sibling to Wrathful
        // Smite on the lv1 "save-vs-condition follow-up" corner but
        // distinct on damage type (Piercing vs Psychic), condition
        // (Restrained vs Frightened), DC anchor (WIS vs CHA), and
        // weapon lane (either vs melee-only). First entry on the
        // rider table whose weapon-lane composition is
        // `!melee_only && !ranged_only` alongside a save-follow-up —
        // Crusader's Mantle / Crown of Stars / Bigby's Hand / Holy
        // Weapon / Enlarge already ride the either-lane gate but as
        // damage-only riders without a follow-up save.
        OnHitRider {
            condition: Condition::EnsnaringStriking,
            dice: Dice::new(1, 6),
            label: "ensnaring strike",
            damage_type: DamageType::Piercing,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Wisdom,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Restrained,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "ensnaring strike restrain",
                hp_threshold: None,
            }),
        },
        // 5e Zephyr Strike — 1st-level ranger transmutation (XGtE),
        // bonus action, concentration. Primes the ranger's next weapon
        // attack — either melee or ranged (RAW: "the next attack you
        // make on this turn", broad envelope unrestricted by weapon
        // lane) — with +1d8 Force damage. Sibling to Ensnaring Strike
        // on the lv1 either-lane prime corner but the pure-damage
        // trade — no follow-up save, no condition install, just raw
        // Force (one of the rarest-resisted damage types in the
        // engine). melee_only=false and ranged_only=false so the
        // rider fires on either the scimitar swing or the longbow
        // shot; consume_on_trigger=true so the first hit consumes
        // the prime, matching every other one-shot lv1 smite spell.
        // follow_up=None distinguishes it from Ensnaring Strike's
        // STR-save-vs-Restrained rider — Zephyr Strike is the pure
        // damage lane of the lv1 ranger prime pair.
        OnHitRider {
            condition: Condition::ZephyrStriking,
            dice: Dice::new(1, 8),
            label: "zephyr strike",
            damage_type: DamageType::Force,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Holy Weapon — 5th-level paladin evocation, concentration.
        // The caster's weapon glows with radiant light: every weapon
        // attack hit deals an extra 2d8 radiant damage. Persistent (not
        // consumed on trigger) and lane-agnostic — melee or ranged hits
        // both fire the rider. Mirrors the Crusader's Mantle / Spirit
        // Shroud persistent rider shape but tier-5 dice and radiant.
        OnHitRider {
            condition: Condition::HolyWeaponed,
            dice: Dice::new(2, 8),
            label: "holy weapon",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Absorb Elements — 1st-level abjuration, reaction. The caster
        // stores captured elemental energy and releases it on their next
        // melee attack: +1d6 of the absorbed element's damage type. We
        // model the rider as generic Force since the per-element type
        // isn't tracked. One-shot — consumed the moment a melee swing
        // lands. The resistance half is handled by the spell's
        // DamageResistant condition install.
        OnHitRider {
            condition: Condition::AbsorbedElements,
            dice: Dice::new(1, 6),
            label: "absorb elements",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Menacing Attack maneuver. Zero rider damage
        // (RAW: +1 superiority die; we collapse the die since the engine
        // has no per-class scaling pool). On the consuming melee hit the
        // target makes a WIS save vs the fighter's STR-based maneuver DC
        // (8 + prof + STR); on fail, they're Frightened until the end of
        // the fighter's next turn (we model as 1 round). Mirrors Stunning
        // Strike's "no rider damage, save-or-condition" shape.
        OnHitRider {
            condition: Condition::MenacingAttacking,
            dice: Dice::new(0, 1),
            label: "menacing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "menacing attack fear",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Disarming Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Trip). On the consuming melee hit the
        // target makes a STR save vs the fighter's STR-based maneuver DC;
        // on fail, they're Disarmed — attack rolls have disadvantage
        // until the start of their next turn (`UntilStartOfNextTurn`
        // mirrors how Mocked / Dodging clear). One-shot.
        OnHitRider {
            condition: Condition::DisarmingAttacking,
            dice: Dice::new(0, 1),
            label: "disarming attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Disarmed,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "disarming attack disarm",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Pushing Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming). On the consuming melee
        // hit the target makes a STR save vs the fighter's STR-based
        // maneuver DC; on fail, they're shoved 15 ft (4 tiles in our
        // 2.5ft grid — round 15ft/2.5 = 6, but we cap at the engine's
        // PushActor maximum behavior of 4 tiles, matching Thunderwave's
        // 10ft push). The first follow-up to use the `Push` variant of
        // `FollowUpEffect` — no condition apply, just forced movement.
        OnHitRider {
            condition: Condition::PushingAttacking,
            dice: Dice::new(0, 1),
            label: "pushing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Push { tiles: 4 },
                label: "pushing attack shove",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Goading Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming / Pushing). On the
        // consuming melee hit the target makes a WIS save vs the
        // fighter's STR-based maneuver DC; on fail, they're Goaded —
        // attack rolls against anyone *other* than the goading fighter
        // are at disadvantage. The follow-up handler also wires the
        // `Goaded` back-link via the auto-chained SetConditionLink(Goaded) emission in
        // `push_follow_up_effect`. Mirrors Compelled Duel's mechanical
        // envelope, but is per-rest rather than concentration-bound.
        OnHitRider {
            condition: Condition::GoadingAttacking,
            dice: Dice::new(0, 1),
            label: "goading attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Goaded,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "goading attack goad",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Sweeping Attack maneuver. Zero rider damage on
        // the primary target (RAW: damage goes to the secondary creature,
        // not the original); the follow-up's `Splash` variant deals 1d8
        // slashing to one footprint-adjacent enemy of the primary target.
        // `save_ability: None` makes the follow-up auto-apply on hit —
        // RAW: the splash uses the original attack roll, which already
        // hit. If no adjacent enemy exists, the splash is a no-op
        // (logged inside `push_follow_up_effect`).
        OnHitRider {
            condition: Condition::SweepingAttacking,
            dice: Dice::new(0, 1),
            label: "sweeping attack",
            damage_type: DamageType::Slashing,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: None,
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Splash {
                    dice: Dice::new(1, 8),
                    damage_type: DamageType::Slashing,
                },
                label: "sweeping attack splash",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Distracting Strike maneuver. +1d6 bonus damage
        // on the consuming melee hit (RAW: add the superiority die to
        // the damage roll) plus a no-save auto-apply Distracted tag on
        // the target. The Distracted condition itself carries the
        // load-bearing rider — `compute_attack_mode` grants advantage
        // to any attacker *other* than the fighter, gated via the
        // `Distracted` back-link set in the chained `SetConditionLink(Condition::Distracted)`
        // emission inside `push_follow_up_effect`. RAW duration is
        // "until the start of your next turn" — modeled with the
        // `UntilStartOfNextTurn` target-side tick-down envelope shared
        // with Goaded / Mocked / Helped.
        OnHitRider {
            condition: Condition::DistractingAttacking,
            dice: Dice::new(1, 6),
            label: "distracting strike",
            damage_type: DamageType::Slashing,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: None,
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Distracted,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "distracting strike distract",
                hp_threshold: None,
            }),
        },
];

/// Build the `SetXBy` side-effect that records the attacker for a
/// linked condition (`Goaded`/`Goaded` back-link, `Distracted`/`Distracted` back-link
/// etc.). Thin wrapper over `condition_link_side_effect` —
/// the central dispatch lives in side_effects.rs and serves both the
/// weapon on-hit rider chain (here) AND the direct-cast actions
/// (Compelled Duel, Vow of Enmity). Returns `None` for conditions that
/// don't carry a link — the caller just emits the bare `ApplyCondition`.
fn attacker_link_side_effect(
    condition: Condition,
    target_id: usize,
    caster_id: usize,
) -> Option<Box<dyn ApplicableSideEffect>> {
    crate::engine::side_effects::condition_link_side_effect(condition, target_id, caster_id)
}

/// Process the optional secondary save-and-apply step that some Smite
/// spells stack on top of their bonus damage. `save_ability: None`
/// auto-applies the condition on hit (Branding Smite, Searing Smite's
/// ignite); `Some(ability)` rolls that save against the caster's DC.
/// `total_damage` is the swing's combined weapon + rider damage value —
/// used to predict the target's post-damage HP for `hp_threshold` gates
/// (Banishing Smite RAW: banishes "if this damage reduces the target to
/// 50 hp or fewer"). `0` is fine for follow-ups with no threshold set.
fn apply_smite_follow_up(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    follow: SmiteFollowUp,
    total_damage: u32,
) {
    // HP-threshold gate (Banishing Smite). Predict post-damage HP as
    // `current_hp - total_damage` and bail if the target would still be
    // above the threshold. Saturating-sub keeps the math clean when the
    // hit would drop them past zero (the threshold still triggers).
    if let Some(threshold) = follow.hp_threshold {
        let Some(target) = encounter.actors.get(&target_id) else {
            return;
        };
        let predicted = target.hitpoints().saturating_sub(total_damage);
        if predicted > threshold {
            encounter.log(format!(
                "  {}: target stays above {} HP threshold (predicted {} HP)",
                follow.label, threshold, predicted
            ));
            return;
        }
    }
    let Some(save_ability) = follow.save_ability else {
        encounter.log(format!("  {}: auto-apply on hit", follow.label));
        push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
        return;
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return;
    };
    let dc = caster.spell_save_dc(follow.dc_ability);
    let save = encounter.roll_save(target_id, save_ability, dc);
    if save.passed() {
        encounter.log(format!("  {}: target saves", follow.label));
        return;
    }
    encounter.log(format!("  {}: target fails save", follow.label));
    push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
}

/// Materialize a `FollowUpEffect` into the side-effect queue. Splits the
/// "what does the rider actually do?" decision from the save / threshold
/// gates above so a future variant (e.g. forced grapple, dispel) plugs
/// in here without re-walking the gates. `caster_id` is the attacker —
/// used by `Push` to anchor the shove on the attacker's tile and by
/// `Splash` to scope the secondary-target search to enemies of the
/// caster.
fn push_follow_up_effect(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    effect: FollowUpEffect,
) {
    match effect {
        FollowUpEffect::Condition { condition, timer } => {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition,
                timer,
            }));
            // Some conditions carry a back-reference to the attacker
            // (`Goaded` back-link, `Distracted` back-link) — chain the matching
            // `SetXBy` side-effect alongside the apply so the link is
            // never stale relative to the condition flag. Single match
            // chokepoint so a new linked condition lands in one place
            // rather than growing another if-let here. Mirrors how
            // Compelled Duel's spell-site pairing emits ApplyCondition
            // (Dueled) + SetConditionLink(Dueled) together — same shape, lifted
            // behind the rider helper so the on-hit path doesn't
            // re-state the chain at every smite-follow-up site.
            if let Some(link) =
                attacker_link_side_effect(condition, target_id, caster_id)
            {
                effects.push(link);
            }
        }
        FollowUpEffect::Push { tiles } => {
            let Some(caster) = encounter.actors.get(&caster_id) else {
                return;
            };
            effects.push(Box::new(PushActor {
                actor_id: target_id,
                from: caster.location(),
                max_tiles: tiles,
            }));
        }
        FollowUpEffect::Splash { dice, damage_type } => {
            // Pick the closest hostile (to the caster) that's
            // footprint-adjacent to the primary target. Closest = lowest
            // (footprint-distance, id) tuple so the choice is
            // deterministic across runs with the same RNG seed.
            let caster_team = match encounter.actors.get(&caster_id) {
                Some(c) => c.team(),
                None => return,
            };
            let mut best: Option<(isize, usize)> = None;
            for (id, other) in encounter.actors.iter() {
                if *id == caster_id || *id == target_id {
                    continue;
                }
                if other.team() == caster_team || !other.is_combat_active() {
                    continue;
                }
                let Some(dist) = encounter.footprint_distance(*id, target_id) else {
                    continue;
                };
                if dist > 0 {
                    continue;
                }
                let dist_from_caster = encounter
                    .footprint_distance(caster_id, *id)
                    .unwrap_or(isize::MAX);
                let key = (dist_from_caster, *id);
                if best.map(|b| key < b).unwrap_or(true) {
                    best = Some(key);
                }
            }
            let Some((_, splash_id)) = best else {
                encounter.log("  sweeping attack: no adjacent enemy to splash".to_string());
                return;
            };
            let rolled = encounter.roll(&dice);
            encounter.log(format!(
                "  sweeping attack: +{} {:?} splashes to {}",
                rolled,
                damage_type,
                encounter.actor_name(splash_id)
            ));
            effects.push(Box::new(DealDamage {
                actor_id: splash_id,
                amount: rolled,
                damage_type,
            }));
        }
    }
}
