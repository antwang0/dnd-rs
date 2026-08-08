use crate::actions::action_template::Action;
use crate::engine::attack::ChargeRider;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, DiceExpr, Roller};
use crate::engine::side_effects::Resource;
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageModifier, DamageType, Language, Size, Skill,
    SpecialSense,
};
use crate::engine::lighting::SunlightFrailty;
use crate::engine::util::{TILE_FEET, modifier_from_score};
use crate::items::item_template::{Item, ItemBonuses};
use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::actions::class_features::{
    BARDIC_INSPIRATION_TAG, FONT_OF_INSPIRATION_TAG, LETHAL_DAMAGE_ABSORBER_FEATURES,
    SHORT_REST_FEATURES, SORCEROUS_RESTORATION_TAG,
};

/// Conditions whose RAW duration clause is bounded by the temporary hit
/// points the same feature handed out — the "...or until you lose all
/// these temporary hit points" wording, of which the Circle of Spores
/// Druid's Symbiotic Entity is the engine's example.
///
/// Read by `drain_temp_hp`, the single chokepoint that decrements the
/// pool. A feature whose buff should die with its shield adds one row
/// here and nothing else — the alternative is a bespoke check at each
/// of the two damage entry points, which is exactly the kind of
/// duplicated rule that goes stale the first time a third entry point
/// appears.
const TEMP_HP_BOUND_CONDITIONS: &[Condition] = &[Condition::SymbioticEntity];

/// Conditions whose resistance covers every damage type — a blanket
/// "halve all incoming damage" buff. Read by `has_condition_resistance`
/// so a new generic damage-resistant condition (future Stoneskin /
/// Globe-style buff) only needs an entry here.
const BLANKET_RESISTANCE_CONDITIONS: &[Condition] = &[
    Condition::DamageResistant,
    Condition::Globed,
    Condition::WardingBonded,
    Condition::Petrified,
];

/// One row in the `TYPED_IMMUNITY_CONDITIONS` cohort — a single held
/// condition whose presence grants outright immunity to every damage
/// type in `types`. Sibling to `PassiveTypedImmunity { flag, types }`
/// on the passive-feature-flag lane — same "{ source, affected slice }"
/// shape, different source axis (`source: Condition` here vs.
/// `flag: fn(&ActorInstance) -> bool` on the passive-feature sibling).
/// A slice of damage types (rather than a single one) lets a hypothetical
/// multi-type immunity condition (a future Overwhelmed-with-Silence
/// thunder + force lane, an Elemental-Absorbing sorcerer buff) land as
/// a single row without duplicating the source condition per damage
/// type. Matches the shape of `FlagDrivenImmunity { flag, suppressed }`
/// on the condition-immunity lane — same "{ source, slice of the
/// affected axis }" declarative-table pattern.
struct ConditionDrivenTypedImmunity {
    source: Condition,
    types: &'static [DamageType],
}

/// Conditions whose presence grants damage-type immunity. Read by
/// `has_condition_immunity` so adding a new "condition X makes you
/// immune to damage type Y" rider lands as a one-line entry instead of
/// another `if dt == ... && self.has_condition(...)` branch in
/// `effective_damage`. The Mind Blank → psychic and Silenced → thunder
/// immunities both live here.
///
/// Sibling to `PASSIVE_TYPED_IMMUNITIES` on the passive-feature-flag
/// lane (that cohort keys off always-on template flags, this cohort
/// keys off held conditions like Mind Blank / Silenced / Petrified).
/// The two immunity lanes are OR'd via `is_immune_to_damage_type` — any
/// hit is sufficient to zero the damage instance.
const TYPED_IMMUNITY_CONDITIONS: &[ConditionDrivenTypedImmunity] = &[
    // 5e Mind Blank: psychic-damage immunity for the duration.
    ConditionDrivenTypedImmunity {
        source: Condition::MindBlanked,
        types: &[DamageType::Psychic],
    },
    // 5e Silence: any creature entirely inside the silence sphere is
    // immune to thunder damage (the magical hush absorbs sonic effects).
    ConditionDrivenTypedImmunity {
        source: Condition::Silenced,
        types: &[DamageType::Thunder],
    },
    // 5e Petrified: "The creature is immune to poison and disease,
    // although a poison or disease already in its system is suspended,
    // not neutralized." The Petrified condition is on
    // `BLANKET_RESISTANCE_CONDITIONS` for the blanket "resistance to
    // all damage" half of the RAW envelope; the typed-immunity row
    // here promotes the poison lane from resistance → immunity, which
    // the damage pipeline checks first (immunity short-circuits before
    // any halving). Companion to the dynamic-condition-immunity entry
    // in `dynamic_immunity_to(Poisoned)` so a creature turned to
    // stone is also immune to a fresh `Poisoned` condition install
    // RAW.
    ConditionDrivenTypedImmunity {
        source: Condition::Petrified,
        types: &[DamageType::Poison],
    },
];

/// One row in the `TYPED_RESISTANCE_CONDITIONS` cohort — a single held
/// condition whose presence grants resistance (half damage) to every
/// damage type in `types`. Sibling to `PassiveTypedResistance
/// { flag, types }` on the passive-feature-flag lane — same
/// "{ source, affected slice }" shape, different source axis
/// (`source: Condition` here vs. `flag: fn(&ActorInstance) -> bool`
/// on the passive-feature sibling). A slice of damage types (rather
/// than a single one) folds a multi-type resistance condition
/// (Investiture of Stone → physical trio, Otherworldly Guise → radiant
/// + poison) through one row rather than duplicating the source
///   condition per damage type.
struct ConditionDrivenTypedResistance {
    source: Condition,
    types: &'static [DamageType],
}

/// Conditions whose resistance only applies to a curated damage-type
/// subset. Read by `has_condition_resistance` so a new Investiture-
/// style buff lands as a one-line entry without touching the damage-
/// pipeline code.
///
/// Sibling to `PASSIVE_TYPED_RESISTANCES` on the passive-feature-flag
/// lane (that cohort keys off always-on template flags like Dwarven /
/// Fiendish / Draconic / Heart-of-the-Storm / Psychic Defenses /
/// Radiant Soul, this cohort keys off held conditions like
/// Investiture of Flame / Purified / Raging / Otherworldly Guise).
const TYPED_RESISTANCE_CONDITIONS: &[ConditionDrivenTypedResistance] = &[
    ConditionDrivenTypedResistance {
        source: Condition::InvestedInFlame,
        types: &[DamageType::Fire],
    },
    ConditionDrivenTypedResistance {
        source: Condition::InvestedInIce,
        types: &[DamageType::Cold],
    },
    ConditionDrivenTypedResistance {
        source: Condition::InvestedInStone,
        types: &[
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ],
    },
    ConditionDrivenTypedResistance {
        source: Condition::Purified,
        types: &[DamageType::Poison],
    },
    ConditionDrivenTypedResistance {
        source: Condition::Raging,
        types: &[
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ],
    },
    // 5e Tasha's Otherworldly Guise (celestial flavor): radiant + poison
    // resistance from the divine-aligned form. Folded into the same lane
    // as the other typed-resistance buffs so the damage pipeline halves
    // both incoming radiant and incoming poison damage cleanly.
    ConditionDrivenTypedResistance {
        source: Condition::OtherworldlyGuised,
        types: &[DamageType::Radiant, DamageType::Poison],
    },
    // 5e Intellect Fortress: "the target has resistance to psychic
    // damage" — the damage half of the spell. Its save half rides
    // `MENTAL_SAVE_MODE_CONDITIONS` over in the encounter module; both
    // halves are one row each, which is the whole reason the spell
    // needed no code of its own.
    ConditionDrivenTypedResistance {
        source: Condition::IntellectFortified,
        types: &[DamageType::Psychic],
    },
];

/// One row in the `TYPED_VULNERABILITY_CONDITIONS` cohort — a single
/// held condition whose presence makes the holder take *double* damage
/// of every type in `types`.
///
/// The mirror of `ConditionDrivenTypedImmunity` /
/// `ConditionDrivenTypedResistance` on the other side of the ledger,
/// and the last of the three to get a lane. Vulnerability used to be
/// readable only off a creature's own `damage_modifiers` map, which
/// meant every 5e effect of the form "the target is vulnerable to X
/// until Y" had nowhere to land — the reason the Grave Domain Cleric's
/// Path to the Grave shipped as an attack-advantage grant with its own
/// docstring flagging the missing half.
struct ConditionDrivenTypedVulnerability {
    source: Condition,
    types: &'static [DamageType],
}

/// Conditions whose presence makes the holder vulnerable. Read by
/// `has_condition_vulnerability`, which `effective_damage` ORs with the
/// template's own `damage_modifiers` entry — so a curse-granted
/// vulnerability and a template-granted resistance to the same type
/// still cancel, per PHB p.197.
///
/// One row today:
///
///   - **Path to the Grave** (Grave Domain Cleric Channel Divinity,
///     lv2): "the next time you or an ally of yours hits the cursed
///     creature with an attack, the creature has vulnerability to all
///     of that attack's damage, and then the curse ends." *All* of the
///     attack's damage, hence `DamageType::ALL` rather than a curated
///     slice — the curse doesn't care what the blow was made of.
///
/// The "and then the curse ends" clause is not here: this cohort
/// answers what the condition *does*, and the attack chokepoints answer
/// when it goes, by queueing a `RemoveCondition` behind the last of the
/// swing's damage effects. That placement is what makes "all of that
/// attack's damage" true for a swing that lands piercing and radiant
/// and necrotic in three separate instances.
const TYPED_VULNERABILITY_CONDITIONS: &[ConditionDrivenTypedVulnerability] = &[
    ConditionDrivenTypedVulnerability {
        source: Condition::MarkedForGrave,
        types: &DamageType::ALL,
    },
];

/// Every condition on `TYPED_VULNERABILITY_CONDITIONS` that RAW says is
/// spent by the attack it doubles.
///
/// Read by the weapon- and spell-attack chokepoints, which append a
/// removal for each of these behind the swing's damage. Kept as its own
/// list rather than a `consumed: bool` on the row because a
/// vulnerability that simply runs out on its timer (a hypothetical
/// Hex-style "vulnerable to cold for a minute") needs no attack-side
/// machinery at all, and pairing the two in one struct would put a
/// field on every future row that most of them would set to `false`.
pub const VULNERABILITIES_SPENT_BY_THE_ATTACK: &[Condition] = &[Condition::MarkedForGrave];

/// One row in the `RAGE_GATED_BROAD_RESISTANCES` cohort — a single
/// passive-feature tag whose presence, combined with an active `Raging`
/// condition, grants resistance to every damage type EXCEPT those in
/// `types_except`. Sibling to `ConditionDrivenTypedResistance
/// { source, types }` and `PassiveTypedResistance { flag, types }` on
/// the "cohort row carries a slice of the affected axis" declarative-
/// table pattern — same declarative shape, but the row's slice inverts
/// the semantics (all-types-except vs. only-these-types). The compound
/// gate (Raging condition + passive tag) folds the two-clause check
/// into the row rather than a per-row flag closure, keeping the row
/// literal declarative — same shape the `FailedSaveAddDieSource` /
/// `ReactiveDisadvantageSource` cohorts use for their per-rest
/// per-tag gating parameters.
struct RageGatedBroadResistance {
    /// Passive-feature tag whose presence gates the row. Read via
    /// `has_passive_feature(tag)` on the checked actor. Distinct from
    /// the `flag` closure shape on `PassiveTypedResistance` because
    /// every row here shares the same additional Rage gate — folding
    /// the closure to a bare tag drops the per-row `|a| a.has_condition
    /// (Raging) && a.has_passive_feature(TAG)` boilerplate that would
    /// duplicate on every future rage-gated broad-resistance entry.
    tag: &'static str,
    /// Damage types EXCLUDED from the row's resistance grant — a hit
    /// on any type in this slice passes through unchanged even while
    /// the row's flags are all live. Inverse of the `types` slice on
    /// `ConditionDrivenTypedResistance` / `PassiveTypedResistance`,
    /// which lists ONLY the types resisted. Bear Totem RAW resists
    /// every damage type except psychic, so `&[DamageType::Psychic]`
    /// is the natural literal.
    types_except: &'static [DamageType],
}

/// Passive-tag-driven "broad resistance while raging" cohort read by
/// `has_condition_resistance`. Every row pairs a passive-feature tag
/// with an exclusion slice — an incoming damage instance of type `dt`
/// is halved iff the actor holds the tag AND holds the `Raging`
/// condition AND `dt` is NOT in the row's `types_except` slice. The
/// row grants resistance under the same "one halving per damage
/// instance" rule the other resistance lanes honor.
///
/// Pre-cleanup this cohort's single entry (Bear Totem Spirit) rode as
/// an ad-hoc `if dt != DamageType::Psychic && self.has_condition
/// (Raging) && self.has_passive_feature(BEAR_TOTEM_TAG) { return
/// true; }` branch inline in `has_condition_resistance` — sibling to
/// the ad-hoc branches the earlier cleanup passes promoted into
/// `PASSIVE_TYPED_RESISTANCES` / `FLAG_DRIVEN_IMMUNITIES` /
/// `PASSIVE_FEATURE_SPEED_BONUSES` / etc. The promotion drops the
/// special-case branch and lets future rage-gated broad-resistance
/// features (a hypothetical Storm Herald Tundra Storm Aura at higher
/// levels that resists most damage types while raging, a future
/// Ancestral Guardian broad-resistance rider, etc.) land as one-line
/// entries in the declarative table rather than another inline
/// if-branch chain in `has_condition_resistance`.
///
/// Entries:
///   - **Bear Totem Spirit (Barbarian Path of the Totem Warrior lv3)**:
///     resistance to every damage type except **psychic** while
///     raging. Sibling row to the other rage-gated Totem Spirits on
///     the `PASSIVE_FEATURE_SPEED_BONUSES` cohort (Tiger / Elk /
///     Wolverine / Panther) — same "compound Raging + tag gate"
///     pattern, different affected axis (broad resistance here vs.
///     speed bonus there).
///
/// A new rage-gated broad-resistance feature drops in as a fresh row
/// with its own `(tag, types_except)` pair; a feature that resists a
/// SPECIFIC damage type instead (e.g. a hypothetical rage-gated
/// single-type resistance) is a better fit for the sibling
/// `PASSIVE_TYPED_RESISTANCES` cohort with a compound closure that
/// includes the Rage gate (same shape the Tiger / Elk / Wolverine /
/// Panther speed-bonus rows already use).
const RAGE_GATED_BROAD_RESISTANCES: &[RageGatedBroadResistance] = &[
    RageGatedBroadResistance {
        tag: crate::actions::class_features::BEAR_TOTEM_TAG,
        types_except: &[DamageType::Psychic],
    },
];

/// One row in the `PASSIVE_TYPED_RESISTANCES` cohort — a single passive
/// flag / tag that grants resistance to every damage type in `types`.
/// The `flag` closure reads a racial / subclass passive-feature accessor
/// on `ActorInstance` (struct-field flag like `has_dwarven_resilience`
/// or a tag check like `has_passive_feature(HEART_OF_THE_STORM_TAG)`);
/// when it returns true the actor takes half damage of every entry in
/// `types` per the same "one halving per damage instance" rule that
/// template `damage_modifiers` and the `TYPED_RESISTANCE_CONDITIONS`
/// cohort honor.
///
/// A slice of damage types (rather than a single one) lets a feature
/// that resists multiple types (Storm Sorcerer Heart of the Storm ->
/// Lightning + Thunder) land as a single row without duplicating the
/// flag closure per damage type. Matches the shape of
/// `FlagDrivenImmunity { flag, suppressed: &'static [Condition] }` on
/// the sibling passive-immunity lane.
struct PassiveTypedResistance {
    flag: fn(&ActorInstance) -> bool,
    types: &'static [DamageType],
}

/// Passive-feature-driven typed resistance table read by
/// `effective_damage` and `has_own_typed_reduction`. Each row is a
/// `PassiveTypedResistance { flag, types }`; the flag returning true
/// grants half damage of every listed type per the "one halving per
/// damage instance" rule that template `damage_modifiers` and the
/// `TYPED_RESISTANCE_CONDITIONS` cohort honor.
///
/// Sibling to `TYPED_RESISTANCE_CONDITIONS` (condition-driven typed
/// resistance) but keyed off always-on template flags rather than held
/// conditions — these fire regardless of any timer-based source.
///
/// Entries:
///   - **Dwarven Resilience (Dwarf racial)**: poison resistance. The
///     save-advantage clause lives on `compute_save_mode` — this row
///     covers only the damage-halving half.
///   - **Fiendish Resilience (Warlock Fiend Patron lv10)**: fire
///     resistance. RAW: "Choose one damage type when you finish a short
///     or long rest. You have resistance to that damage type until you
///     choose a different one." We collapse the choice to a fixed Fire
///     lock so the flag is a single template pick — thematic for the
///     Fiend patron's fire-anchored flavor (Burning Hands / Fireball /
///     Wall of Fire on the expanded spell list, Dark One's Blessing as
///     the kill-triggered temp-HP well) and it drops the need for a
///     "picked type" mutable slot on `ActorInstance`. Adding a "choose
///     any of the ten damage types on short rest" surface later is a
///     `Option<DamageType>` swap at this row and a matching short-rest
///     hook.
///   - **Draconic Resilience (Sorcerer Draconic Bloodline lv6)**: fire
///     resistance. RAW: bloodline-picked damage type matching the
///     draconic ancestor. Collapsed to a fixed Fire lock on the same
///     shape as Fiendish Resilience — thematic for the Red / Gold /
///     Brass ancestor picks that lean into the sorcerer's fire-heavy
///     spell list (Burning Hands / Scorching Ray / Fireball as natural
///     pickups). Sibling row to Fiendish Resilience on the same damage
///     type — distinct source (Sorcerer bloodline vs. Warlock patron)
///     but identical damage-pipeline surface; the standard "one
///     halving per damage instance" rule caps a hypothetical multi-
///     class carrier at a single /2 per Fire hit.
///   - **Heart of the Storm (Sorcerer Storm Sorcery lv6)**: lightning
///     AND thunder resistance. RAW gives both types on a single feature
///     — the multi-type slice on this row folds through as one entry
///     (rather than two duplicated flag closures) since the row shape
///     accepts a slice of damage types. The eruption-on-cast half of
///     RAW (a 10ft ally-agnostic burst when the sorcerer casts a
///     lightning / thunder spell of lv1+) rides on
///     `EncounterInstance::trigger_heart_of_the_storm_eruption` — the
///     resistance clause is the passive half; the eruption clause is
///     the active half.
///   - **Psychic Defenses (Sorcerer Aberrant Mind lv14, TCE)**: psychic
///     resistance. RAW pairs the resistance with advantage on saves
///     vs. Charmed / Frightened — the advantage-on-save half collapses
///     to install-immunity and lives on `FLAG_DRIVEN_IMMUNITIES` (the
///     same "advantage-on-save collapses to install-immunity" pattern
///     Halfling Brave uses on that cohort).
///   - **Radiant Soul (Warlock Celestial Patron lv6, XGtE)**: radiant
///     resistance. RAW also grants a +CHA-mod damage rider on radiant /
///     fire spells; that clause is a per-cast damage-boost hook not yet
///     wired, so this row ships only the load-bearing defensive half.
///   - **Elemental Gift (Warlock Genie Marid Patron lv6, TCE)**: cold
///     resistance. RAW's Elemental Gift picks a damage type based on
///     the warlock's chosen genie kind; the Marid variant covers Cold.
///     Sibling to the Dao variant below on the "typed resistance from
///     an Otherworldly Patron (Genie kind)" lane.
///   - **Elemental Gift (Warlock Genie Dao Patron lv6, TCE)**:
///     bludgeoning resistance. First user of the Bludgeoning slot on
///     the passive typed-resistance lane — the physical damage trio
///     (Bludgeoning / Piercing / Slashing) was uncovered by any
///     passive typed-resistance cohort row until this entry landed.
///     Sibling to the Marid variant above on the "typed resistance
///     from an Otherworldly Patron (Genie kind)" lane; the two never
///     legally co-occur on a single build (RAW: one genie kind per
///     warlock).
///   - **Elemental Gift (Warlock Genie Djinni Patron lv6, TCE)**:
///     thunder resistance. Third of the four Genie kinds to land on
///     this cohort — completes the physical-element trio (Cold /
///     Bludgeoning / Thunder) alongside Marid and Dao. Overlaps the
///     Thunder axis with Heart of the Storm on the Storm Sorcerer
///     chassis (a different class); a hypothetical multiclass carrier
///     caps at a single /2 per Thunder hit via the "one halving per
///     damage instance" rule. Sibling to the Marid / Dao variants on
///     the "typed resistance from an Otherworldly Patron (Genie kind)"
///     lane; the three never legally co-occur on a single build (RAW:
///     one genie kind per warlock).
///   - **Inured to Undeath (Wizard School of Necromancy lv10, PHB)**:
///     necrotic resistance. First user of the **Necrotic** slot on this
///     cohort — Cold / Fire / Lightning / Thunder / Psychic / Radiant /
///     Bludgeoning were already covered by the sibling patron / bloodline
///     rows, and Necrotic was the last commonly-encountered spell-lane
///     damage type left uncovered. First Wizard-chassis row on the
///     cohort (every prior row came off Warlock / Sorcerer subclasses);
///     matches the "Wizard finally lands its first subclass template"
///     milestone the `NECROMANCY_WIZARD_TEMPLATE` ships alongside.
///   - **Storm Soul (Sea) (Barbarian Path of the Storm Herald lv6,
///     XGtE)**: lightning resistance. First **Barbarian**-chassis row
///     on this cohort — every prior row came off a racial trait or a
///     Warlock / Sorcerer / Wizard subclass. Overlaps the Lightning
///     axis with Heart of the Storm (Storm Sorcerer lv6, Lightning +
///     Thunder) on a different chassis; the two never legally co-
///     occur on a single build (Barbarian vs. Sorcerer subclass), and
///     a hypothetical multiclass carrier caps at a single /2 per
///     Lightning hit via the "one halving per damage instance" rule.
///     Distinct from the barbarian chassis's rage-gated broad
///     resistance (Bear Totem, `RAGE_GATED_BROAD_RESISTANCES`) on
///     both axis (typed, not broad) and gate (always-on, not rage-
///     gated).
///
/// A new passive typed resistance (Circle of the Moon Wild Shape
/// per-form types, Bladeling's Painful Quills necrotic resistance,
/// etc.) drops in here as a one-line entry rather than another
/// hand-rolled `if dt == ... && self.has_...` branch in
/// `effective_damage`.
const PASSIVE_TYPED_RESISTANCES: &[PassiveTypedResistance] = &[
    PassiveTypedResistance {
        flag: |a| a.has_dwarven_resilience,
        types: &[DamageType::Poison],
    },
    PassiveTypedResistance {
        flag: |a| a.has_fiendish_resilience,
        types: &[DamageType::Fire],
    },
    PassiveTypedResistance {
        flag: |a| a.has_draconic_resilience,
        types: &[DamageType::Fire],
    },
    // 5e Sorcerer Storm Sorcery **Heart of the Storm** (level 6). RAW
    // grants resistance to lightning AND thunder damage on the same
    // subclass feature — the slice-of-types shape folds both types
    // through a single row. Ships on `STORM_SORCERER_TEMPLATE` above
    // its strict RAW lv6 gate for the same reason Draconic Resilience
    // (RAW lv6) ships on `DRACONIC_SORCERER_TEMPLATE` — class templates
    // target a balanced playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::HEART_OF_THE_STORM_TAG,
        ),
        types: &[DamageType::Lightning, DamageType::Thunder],
    },
    // 5e Sorcerer Aberrant Mind **Psychic Defenses** (level 14, TCE).
    // RAW grants resistance to psychic damage AND advantage on saves
    // vs. Charmed / Frightened. The resistance clause lands here; the
    // Charmed / Frightened install-immunity clause (RAW's advantage-
    // on-save collapsed to immunity for the same reason Halfling Brave
    // does) lands as a companion row on `FLAG_DRIVEN_IMMUNITIES` below.
    // Ships on `ABERRANT_MIND_SORCERER_TEMPLATE` above its strict RAW
    // lv14 gate for the same reason Heart of the Storm (RAW lv6) ships
    // on `STORM_SORCERER_TEMPLATE` — class templates target a balanced
    // playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::PSYCHIC_DEFENSES_TAG,
        ),
        types: &[DamageType::Psychic],
    },
    // 5e Warlock Celestial Patron **Radiant Soul** (level 6, XGtE). RAW
    // grants resistance to radiant damage on the Otherworldly Patron:
    // The Celestial subclass. Sibling row to Fiendish Resilience (Fire)
    // and Draconic Resilience (Fire) on the "typed resistance from a
    // patron / bloodline" lane — same halving rule, different damage
    // axis (Radiant vs. Fire), and to Heart of the Storm (Lightning +
    // Thunder) / Psychic Defenses (Psychic) on the "one feature tag
    // drives one cohort row" pattern. The +CHA-mod-to-radiant-or-fire
    // damage rider half of RAW is left as future work; the resistance
    // clause is the load-bearing defensive half and rides here alone.
    // Ships on `CELESTIAL_WARLOCK_TEMPLATE` above its strict RAW lv6
    // gate for the same reason `ARCHFEY_WARLOCK_TEMPLATE` ships
    // Beguiling Defenses (RAW lv10) — class templates target a
    // balanced playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::RADIANT_SOUL_TAG,
        ),
        types: &[DamageType::Radiant],
    },
    // 5e Warlock Genie (Marid) Patron **Elemental Gift** (level 6, TCE).
    // RAW grants a per-genie-kind damage-type resistance; the Marid
    // variant covers **cold** — the marid's water- and ice-flavored
    // patron pact. First user of the Cold slot on the passive typed-
    // resistance lane (Fiendish / Draconic own Fire, Heart of the Storm
    // owns Lightning + Thunder, Psychic Defenses owns Psychic, Radiant
    // Soul owns Radiant). Sibling row to Radiant Soul on the "typed
    // resistance from an Otherworldly Patron" lane — same halving rule,
    // different patron flavor and different damage axis. Ships on
    // `MARID_WARLOCK_TEMPLATE` above its strict RAW lv6 gate for the
    // same reason Radiant Soul (RAW lv6) ships on
    // `CELESTIAL_WARLOCK_TEMPLATE` — class templates target a balanced
    // playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::MARID_ELEMENTAL_GIFT_TAG,
        ),
        types: &[DamageType::Cold],
    },
    // 5e Warlock Genie (Dao) Patron **Elemental Gift** (level 6, TCE).
    // RAW grants a per-genie-kind damage-type resistance; the Dao
    // variant covers **bludgeoning** — the dao's earth-flavored patron
    // hardens the warlock against blunt-force hits. First user of the
    // Bludgeoning slot on the passive typed-resistance lane (Fiendish /
    // Draconic own Fire, Heart of the Storm owns Lightning + Thunder,
    // Psychic Defenses owns Psychic, Radiant Soul owns Radiant, and
    // Marid's Elemental Gift owns Cold; Bludgeoning was uncovered on
    // the passive-typed-resistance cohort until this row landed).
    // Sibling row to the Marid variant on the "typed resistance from an
    // Otherworldly Patron (Genie kind)" lane — same halving rule,
    // different patron flavor and different damage axis. Ships on
    // `DAO_WARLOCK_TEMPLATE` above its strict RAW lv6 gate for the
    // same reason `MARID_WARLOCK_TEMPLATE` ships Elemental Gift (RAW
    // lv6) — class templates target a balanced playable level, not
    // lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::DAO_ELEMENTAL_GIFT_TAG,
        ),
        types: &[DamageType::Bludgeoning],
    },
    // 5e Warlock Genie (Djinni) Patron **Elemental Gift** (level 6, TCE).
    // RAW grants a per-genie-kind damage-type resistance; the Djinni
    // variant covers **thunder** — the djinni's storm- and sky-flavored
    // patron pact hardens the warlock against thundercracks, Shatter
    // bursts, and Thunderwave shoves. Overlaps the Thunder axis with
    // Heart of the Storm (Storm Sorcerer lv6) on the "one halving per
    // damage instance" rule — a hypothetical multiclass carrier caps
    // at a single /2 per Thunder hit rather than double-halving. Third
    // of the four Genie kinds to land here; the Efreeti (Fire) variant
    // is a semantic duplicate of the Fiendish / Draconic Resilience
    // Fire rows and is left as future work in favor of the three
    // physical-element genie kinds whose damage axes are otherwise
    // uncovered on the passive-resistance lane. Ships on
    // `DJINNI_WARLOCK_TEMPLATE` above its strict RAW lv6 gate for the
    // same reason `MARID_WARLOCK_TEMPLATE` and `DAO_WARLOCK_TEMPLATE`
    // ship Elemental Gift (RAW lv6) — class templates target a
    // balanced playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::DJINNI_ELEMENTAL_GIFT_TAG,
        ),
        types: &[DamageType::Thunder],
    },
    // 5e Warlock Genie (Efreeti) Patron **Elemental Gift** (level 6, TCE).
    // RAW grants a per-genie-kind damage-type resistance; the Efreeti
    // variant covers **fire** — the efreeti's flame-flavored patron pact
    // hardens the warlock against the blaze. Fourth (and final) of the
    // four RAW Genie kinds to land on the passive typed-resistance cohort
    // — completes the four-quadrant Genie patron coverage grid alongside
    // Marid (Cold), Dao (Bludgeoning), and Djinni (Thunder). The four
    // never legally co-occur on a single build (RAW: one genie kind per
    // warlock), so the split-tag shape is a template-drift lock rather
    // than a stacking concern.
    //
    // Semantic duplicate on the Fire axis of Fiendish Resilience (Warlock
    // Fiend Patron lv10) and Draconic Resilience (Sorcerer Draconic
    // Bloodline lv6). The duplication is a **taxonomic completeness**
    // grant, not a mechanical-coverage grant — the Efreeti variant lands
    // so the four-genie family reads as a full quadrant even though the
    // Fire axis is already covered by two other rows on distinct chassis.
    // The three Fire-resistance rows (Fiendish / Draconic / Efreeti) never
    // legally co-occur on a single build (Warlock Fiend vs. Warlock
    // Efreeti vs. Sorcerer Draconic subclass), and a hypothetical
    // multiclass carrier caps at a single /2 per Fire hit under the "one
    // halving per damage instance" rule. Also overlaps the Fire axis with
    // Storm Soul (Desert) on the Barbarian chassis — same "one halving
    // per damage instance" cap applies. Ships on `EFREETI_WARLOCK_TEMPLATE`
    // above its strict RAW lv6 gate for the same reason
    // `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
    // `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6) — class
    // templates target a balanced playable level, not lockstep PHB
    // progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::EFREETI_ELEMENTAL_GIFT_TAG,
        ),
        types: &[DamageType::Fire],
    },
    // 5e Wizard Arcane Tradition **School of Necromancy** — **Inured to
    // Undeath** (level 10, PHB). RAW grants resistance to necrotic damage
    // (paired with a max-HP-can't-be-reduced clause that has no combat
    // surface on today's engine and is left as future work). First user
    // of the **Necrotic** slot on the passive typed-resistance lane —
    // Cold is owned by Marid's Elemental Gift, Fire by Fiendish /
    // Draconic Resilience, Lightning + Thunder by Heart of the Storm,
    // Psychic by Psychic Defenses, Radiant by Radiant Soul, Bludgeoning
    // by Dao's Elemental Gift, and Thunder by Djinni's Elemental Gift;
    // Necrotic was uncovered on the passive-typed-resistance cohort
    // until this row lands. Necrotic is a signature damage type of the
    // wizard's own undead spell list (Chill Touch / Ray of Enfeeblement
    // / Vampiric Touch / Blight / Circle of Death / Finger of Death /
    // Negative Energy Flood) — the necromancer's own kit stops
    // trickling back onto its own chassis on a friendly-fire miscast
    // under the halving rule. Ships on `NECROMANCY_WIZARD_TEMPLATE`
    // above its strict RAW lv10 gate for the same reason
    // `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
    // `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6) —
    // class templates target a balanced playable level, not lockstep
    // PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::INURED_TO_UNDEATH_TAG,
        ),
        types: &[DamageType::Necrotic],
    },
    // 5e Barbarian Primal Path **Path of the Storm Herald (Sea)** —
    // **Storm Soul (Sea)** (level 6, XGtE). RAW grants resistance to
    // lightning damage (paired with a swim-speed + water-breathing
    // clause that has no combat surface on today's engine and is left
    // as future work). First Barbarian-chassis row on the passive
    // typed-resistance lane — every prior row came off a racial trait
    // or a Warlock / Sorcerer / Wizard subclass. Overlaps the Lightning
    // axis with Heart of the Storm (Storm Sorcerer lv6) on a different
    // chassis; the two never legally co-occur on a single build
    // (Barbarian vs. Sorcerer subclass) and a hypothetical multiclass
    // carrier caps at a single /2 per Lightning hit under the "one
    // halving per damage instance" rule. Distinct from the barbarian
    // chassis's rage-gated broad resistance (Bear Totem,
    // `RAGE_GATED_BROAD_RESISTANCES`) on both axis (typed, not broad)
    // and gate (always-on, not rage-gated). Ships on
    // `SEA_STORM_HERALD_BARBARIAN_TEMPLATE` at (or above) its strict
    // RAW lv6 gate for the same reason `MARID_WARLOCK_TEMPLATE` /
    // `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship
    // Elemental Gift (RAW lv6) — class templates target a balanced
    // playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::STORM_SOUL_SEA_TAG,
        ),
        types: &[DamageType::Lightning],
    },
    // 5e Barbarian Primal Path **Path of the Storm Herald (Desert)** —
    // **Storm Soul (Desert)** (level 6, XGtE). RAW grants resistance to
    // fire damage (paired with an "immune to extreme heat" exhaustion
    // rider and an "ignite an unattended flammable object" ribbon —
    // neither has a combat surface today, both left as future work).
    // Second Barbarian-chassis row on the passive typed-resistance lane
    // — `STORM_SOUL_SEA_TAG` blazed the trail with the Lightning row.
    // Overlaps the Fire axis with Fiendish Resilience (Warlock Fiend
    // Patron lv10) and Draconic Resilience (Sorcerer Draconic Bloodline
    // lv6) on different chassis; the three never legally co-occur on a
    // single build (Barbarian vs. Warlock vs. Sorcerer subclass) and a
    // hypothetical multiclass carrier caps at a single /2 per Fire hit
    // under the "one halving per damage instance" rule. Distinct from
    // the barbarian chassis's rage-gated broad resistance (Bear Totem,
    // `RAGE_GATED_BROAD_RESISTANCES`) on both axis (typed, not broad)
    // and gate (always-on, not rage-gated). Ships on
    // `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE` at (or above) its strict
    // RAW lv6 gate for the same reason `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
    // ships Storm Soul (Sea) (RAW lv6) and `MARID_WARLOCK_TEMPLATE` /
    // `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
    // Gift (RAW lv6) — class templates target a balanced playable
    // level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::STORM_SOUL_DESERT_TAG,
        ),
        types: &[DamageType::Fire],
    },
    // 5e Barbarian Primal Path **Path of the Storm Herald (Tundra)** —
    // **Storm Soul (Tundra)** (level 6, XGtE). RAW grants resistance to
    // cold damage (paired with an "immune to extreme cold" exhaustion
    // rider and a "freeze water within 5 ft" ribbon — neither has a
    // combat surface today, both left as future work). Third Barbarian-
    // chassis row on the passive typed-resistance lane — `STORM_SOUL_SEA_TAG`
    // blazed the trail with the Lightning row and `STORM_SOUL_DESERT_TAG`
    // followed with the Fire row; this row completes the three-flavor
    // Storm Herald elemental trio (Sea Lightning / Desert Fire / Tundra
    // Cold) on the barbarian chassis. Overlaps the Cold axis with
    // Marid's Elemental Gift (Warlock Genie Marid Patron lv6) on a
    // different chassis; the two never legally co-occur on a single
    // build (Barbarian vs. Warlock subclass) and a hypothetical
    // multiclass carrier caps at a single /2 per Cold hit under the
    // "one halving per damage instance" rule. Distinct from the
    // barbarian chassis's rage-gated broad resistance (Bear Totem,
    // `RAGE_GATED_BROAD_RESISTANCES`) on both axis (typed, not broad)
    // and gate (always-on, not rage-gated). Ships on
    // `TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE` at (or above) its strict
    // RAW lv6 gate for the same reason `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
    // ships Storm Soul (Sea) (RAW lv6), `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE`
    // ships Storm Soul (Desert) (RAW lv6), and `MARID_WARLOCK_TEMPLATE` /
    // `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
    // Gift (RAW lv6) — class templates target a balanced playable
    // level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::STORM_SOUL_TUNDRA_TAG,
        ),
        types: &[DamageType::Cold],
    },
    // 5e Cleric Divine Domain **Forge Domain** — **Soul of the Forge**
    // (level 6, XGtE). RAW grants resistance to fire damage (paired with
    // a "+1 AC while wearing heavy armor" clause that has no first-class
    // combat surface today, left as future work). First Cleric-chassis
    // row on the passive typed-resistance lane — every prior row came
    // off a racial trait or a Warlock / Sorcerer / Wizard / Barbarian
    // subclass. Overlaps the Fire axis with four existing rows: Fiendish
    // Resilience (Warlock Fiend Patron lv10), Draconic Resilience
    // (Sorcerer Draconic Bloodline lv6), Efreeti Elemental Gift (Warlock
    // Genie Efreeti lv6), and Storm Soul (Desert) (Barbarian Storm
    // Herald Desert lv6). The five Fire-resistance rows never legally
    // co-occur on a single build (Warlock Fiend vs. Warlock Efreeti vs.
    // Sorcerer Draconic vs. Barbarian Storm Herald Desert vs. Cleric
    // Forge Domain are five distinct class-subclass slots), and a
    // hypothetical multiclass carrier caps at a single /2 per Fire hit
    // under the "one halving per damage instance" rule. Ships on
    // `FORGE_CLERIC_TEMPLATE` at (or above) its strict RAW lv6 gate for
    // the same reason `SEA_STORM_HERALD_BARBARIAN_TEMPLATE` /
    // `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE` /
    // `TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE` ship Storm Soul (RAW
    // lv6), `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
    // `DJINNI_WARLOCK_TEMPLATE` / `EFREETI_WARLOCK_TEMPLATE` ship
    // Elemental Gift (RAW lv6) — class templates target a balanced
    // playable level, not lockstep PHB progression.
    PassiveTypedResistance {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::SOUL_OF_THE_FORGE_TAG,
        ),
        types: &[DamageType::Fire],
    },
];

/// One row in the `PASSIVE_TYPED_IMMUNITIES` cohort — a single passive
/// flag / tag that grants **immunity** (not resistance) to every damage
/// type in `types`. Immunity short-circuits the damage pipeline before
/// any halving; the flag closure reads a racial / subclass passive-
/// feature accessor on `ActorInstance` the same way `PassiveTypedResistance`
/// does on the sibling resistance lane.
///
/// A slice of damage types (rather than a single one) lets a hypothetical
/// multi-type immunity feature (a future Bladeling's Painful Quills
/// necrotic + slashing immunity, an Elemental Adept multi-type immunity)
/// land as a single row without duplicating the flag closure per
/// damage type. Matches the shape of `PassiveTypedResistance { flag,
/// types }` on the sibling passive-resistance lane and
/// `FlagDrivenImmunity { flag, suppressed }` on the passive-condition-
/// immunity lane — same "flag closure + slice of the affected axis"
/// declarative-table pattern.
struct PassiveTypedImmunity {
    flag: fn(&ActorInstance) -> bool,
    types: &'static [DamageType],
}

/// Passive-feature-driven typed **immunity** table read by
/// `is_immune_to_damage_type`. Each row is a `PassiveTypedImmunity
/// { flag, types }`; the flag returning true short-circuits the damage
/// pipeline to zero on any damage instance of a listed type.
///
/// Sibling to `PASSIVE_TYPED_RESISTANCES` on the passive-feature-flag
/// lane (that cohort halves damage, this cohort zeroes it); sibling to
/// `TYPED_IMMUNITY_CONDITIONS` on the "damage-type immunity" surface
/// (that cohort keys off held conditions like Mind Blank / Silence /
/// Petrified, this cohort keys off always-on template flags). The two
/// immunity lanes are OR'd — any hit is sufficient to zero the damage
/// instance.
///
/// Entries:
///   - **Purity of Body (Monk lv10)**: poison-damage immunity. Sibling
///     to the Petrified condition's poison-immunity row on
///     `TYPED_IMMUNITY_CONDITIONS` — same suppressed type, different
///     source (Monk passive vs. condition-held). The Poisoned-condition
///     half lives on `FLAG_DRIVEN_IMMUNITIES` next to Petrified's
///     Poisoned bounce.
///
/// A new passive typed immunity (Bladeling's Painful Quills necrotic
/// immunity, a hypothetical Iron Body monk necrotic immunity, etc.)
/// drops in here as a one-line entry rather than another hand-rolled
/// `dt == ... && self.has_passive_feature(...)` branch in
/// `is_immune_to_damage_type`.
const PASSIVE_TYPED_IMMUNITIES: &[PassiveTypedImmunity] = &[
    PassiveTypedImmunity {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::PURITY_OF_BODY_TAG,
        ),
        types: &[DamageType::Poison],
    },
];

/// One row in the `CONDITION_DRIVEN_IMMUNITIES` cohort — a single held
/// condition whose presence grants immunity to every condition in
/// `suppressed`. Sibling to `FlagDrivenImmunity { flag, suppressed }`
/// on the passive-feature-flag lane — same "{ source, suppressed
/// slice }" shape, different source axis (`source: Condition` here vs.
/// `flag: fn(&ActorInstance) -> bool` on the passive-feature sibling).
/// A slice of suppressed conditions (rather than a single one) folds
/// a multi-condition immunity buff (Purified / Otherworldly Guise
/// → Frightened + Charmed + Poisoned, Footloose → Paralyzed +
/// Restrained + Grappled) through one row rather than duplicating the
/// source condition per suppressed condition.
struct ConditionDrivenConditionImmunity {
    source: Condition,
    suppressed: &'static [Condition],
}

/// Broad condition-driven immunity table read by `dynamic_immunity_to`.
/// Each row pairs a source condition with the set of downstream condition
/// installs it suppresses. Purified / Otherworldly Guise both grant the
/// same Frightened + Charmed + Poisoned trio; Heroic, MindBlanked,
/// Petrified, and Footloose each map to a narrower cohort. Adding a
/// future broad-immunity buff (a Purified-of-Fear cantrip, a Sanctuary-
/// tier ward, etc.) lands as one row instead of an inlined `||` clause
/// duplicated across every affected condition's arm.
///
/// Sibling to `FLAG_DRIVEN_IMMUNITIES` on the passive-feature-flag
/// lane (that cohort keys off always-on template flags like Fey
/// Ancestry / Nature's Ward / Beguiling Defenses, this cohort keys
/// off held conditions like Heroism / Mind Blank / Petrified /
/// Purified / Otherworldly Guise / Footloose). The two immunity
/// lanes are OR'd via `dynamic_immunity_to` — any hit is sufficient
/// to bounce the condition install.
const CONDITION_DRIVEN_IMMUNITIES: &[ConditionDrivenConditionImmunity] = &[
    // 5e Heroism: immune to Frightened for the duration.
    ConditionDrivenConditionImmunity {
        source: Condition::Heroic,
        suppressed: &[Condition::Frightened],
    },
    // 5e Mind Blank: immune to Charmed.
    ConditionDrivenConditionImmunity {
        source: Condition::MindBlanked,
        suppressed: &[Condition::Charmed],
    },
    // 5e Undead Warlock **Form of Dread**: "you are immune to the
    // frightened condition" while transformed. The tidiest half of the
    // feature and the one that makes its offensive half safe to lean
    // on — a warlock spreading Frightened around cannot have it turned
    // back on them for as long as the form holds.
    ConditionDrivenConditionImmunity {
        source: Condition::FormOfDread,
        suppressed: &[Condition::Frightened],
    },
    // 5e Petrified: immune to poison / disease — the Poisoned-condition
    // half. Damage-type half lives on `TYPED_IMMUNITY_CONDITIONS`.
    ConditionDrivenConditionImmunity {
        source: Condition::Petrified,
        suppressed: &[Condition::Poisoned],
    },
    // 5e Aura of Purity install rider: immune to Charmed, Frightened,
    // and Poisoned for the duration.
    ConditionDrivenConditionImmunity {
        source: Condition::Purified,
        suppressed: &[
            Condition::Frightened,
            Condition::Charmed,
            Condition::Poisoned,
        ],
    },
    // 5e Tasha's Otherworldly Guise (celestial flavor): immune to the
    // same trio as Purified for the duration.
    ConditionDrivenConditionImmunity {
        source: Condition::OtherworldlyGuised,
        suppressed: &[
            Condition::Frightened,
            Condition::Charmed,
            Condition::Poisoned,
        ],
    },
    // 5e Freedom of Movement (Footloose install): immune to magical
    // movement restraint — Paralyzed / Restrained / Grappled.
    ConditionDrivenConditionImmunity {
        source: Condition::Footloose,
        suppressed: &[
            Condition::Paralyzed,
            Condition::Restrained,
            Condition::Grappled,
        ],
    },
];

/// Row of `FLAG_DRIVEN_IMMUNITIES`. `flag` reads a racial / subclass
/// passive-feature accessor on `ActorInstance` (Halfling Brave, Fey
/// Ancestry, Nature's Ward, Purity of Body); when it returns true the
/// actor is treated as immune to every condition in `suppressed`. Held
/// as a function-pointer rather than an enum tag so a new row can point
/// to any predicate on the actor without expanding a central enum.
struct FlagDrivenImmunity {
    flag: fn(&ActorInstance) -> bool,
    suppressed: &'static [Condition],
}

/// Broad flag-driven immunity table read by `dynamic_immunity_to`.
/// Sibling to `CONDITION_DRIVEN_IMMUNITIES` but keyed off passive-feature
/// flags rather than held conditions — these fire regardless of any
/// timer-based source. Multiple rows can cover the same condition
/// (Fey Ancestry, Nature's Ward, Aura of Devotion, and MindBlanked all
/// suppress Charmed installs); any hit is sufficient. Adding a future
/// racial / subclass capstone lands as one row here without touching
/// the `dynamic_immunity_to` matcher body.
const FLAG_DRIVEN_IMMUNITIES: &[FlagDrivenImmunity] = &[
    // 5e Halfling Brave racial: RAW "advantage on saves vs Frightened"
    // — approximated as immunity because the engine doesn't tag saves
    // by what condition they defend against.
    FlagDrivenImmunity {
        flag: |a| a.has_brave,
        suppressed: &[Condition::Frightened],
    },
    // 5e Elf / Half-Elf / Drow Fey Ancestry: advantage on Charmed saves
    // (approximated as immunity, same reason as Brave), and RAW
    // "magic can't put you to sleep" — since our engine only installs
    // Asleep from magical sources, that half is RAW-exact.
    FlagDrivenImmunity {
        flag: |a| a.has_fey_ancestry,
        suppressed: &[Condition::Charmed, Condition::Asleep],
    },
    // 5e Warlock Undying Patron **Aspect of the Moon** eldritch
    // invocation (SCAG): the warlock no longer needs to sleep and can't
    // be forced to sleep by any means. Collapses to a single
    // `Asleep`-install bounce here — natural sleep sits outside the
    // combat loop. Sibling row to Fey Ancestry on the `Asleep`-immunity
    // lane: either flag alone suffices, both together are redundant
    // (an Elven Undying Warlock stacks the two rows cleanly under the
    // OR-of-cohort-hits semantics `dynamic_immunity_to` already honors).
    FlagDrivenImmunity {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::ASPECT_OF_THE_MOON_TAG,
        ),
        suppressed: &[Condition::Asleep],
    },
    // 5e Paladin Oath of the Ancients Nature's Ward (lv15 capstone):
    // immunity to Charmed AND Frightened installs. The two-condition
    // grant folds through one row; a single flag drives both bounces.
    FlagDrivenImmunity {
        flag: |a| a.has_natures_ward,
        suppressed: &[Condition::Charmed, Condition::Frightened],
    },
    // 5e Monk Purity of Body (lv10 passive feature): immunity to poison
    // (Poisoned-condition half). Poison-damage half lives in
    // `effective_damage` next to the other passive-feature typed-
    // immunity gates.
    FlagDrivenImmunity {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::PURITY_OF_BODY_TAG,
        ),
        suppressed: &[Condition::Poisoned],
    },
    // 5e Barbarian Path of the Berserker Mindless Rage (lv6): while
    // raging, the barbarian can't be charmed or frightened. Compound
    // gate — the flag closure checks BOTH the passive-feature tag AND
    // the `Raging` condition, so the immunity flips off the moment
    // rage drops. RAW also suspends any pre-existing Charmed /
    // Frightened for the duration; our engine reads immunity via
    // `dynamic_immunity_to` at every effect chokepoint so a mid-rage
    // read short-circuits identically for pre-installed conditions.
    FlagDrivenImmunity {
        flag: |a| {
            a.has_passive_feature(crate::actions::class_features::MINDLESS_RAGE_TAG)
                && a.has_condition(Condition::Raging)
        },
        suppressed: &[Condition::Charmed, Condition::Frightened],
    },
    // 5e Sorcerer Aberrant Mind **Psychic Defenses** (lv14, TCE):
    // immunity to Charmed AND Frightened installs. RAW grants advantage
    // on saves vs. those two conditions; collapsed to install-immunity
    // for the same reason Halfling Brave (advantage vs. Frightened) and
    // Fey Ancestry (advantage vs. Charmed) collapse — the engine
    // doesn't tag saves by what condition they defend against. Sibling
    // to Nature's Ward (Ancients Paladin lv15) on the same Charmed +
    // Frightened suppression lane — same suppressed set, different
    // chassis and different source flag; the two rows are OR'd so a
    // hypothetical Ancients Paladin / Aberrant Mind Sorcerer multi-
    // class stacks them cleanly under the OR-of-cohort-hits semantics.
    // The damage-resistance half (psychic) lives on the sibling
    // `PASSIVE_TYPED_RESISTANCES` row above.
    FlagDrivenImmunity {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::PSYCHIC_DEFENSES_TAG,
        ),
        suppressed: &[Condition::Charmed, Condition::Frightened],
    },
    // 5e Warlock Archfey Patron **Beguiling Defenses** (lv10, PHB):
    // immunity to Charmed installs. Sibling to Fey Ancestry on the
    // exact same Charmed-suppression lane — same suppressed set (a
    // single-element slice), different source (racial Fey Ancestry vs.
    // Archfey-patron subclass feature); the two rows are OR'd so a
    // Fey-Ancestry Elven Archfey Warlock carries redundant Charmed
    // immunity, and either flag alone is sufficient. Distinct from
    // Psychic Defenses (Aberrant Mind Sorcerer lv14) on the condition
    // axis — Beguiling Defenses covers only Charmed (RAW-exact, since
    // the reaction charm-back clause presumes the warlock stays lucid),
    // while Psychic Defenses covers both Charmed AND Frightened. Ships
    // on `ARCHFEY_WARLOCK_TEMPLATE` above its strict RAW lv10 gate for
    // the same reason Entropic Ward (RAW lv6) ships on
    // `GREAT_OLD_ONE_WARLOCK_TEMPLATE` — class templates target a
    // balanced playable level, not lockstep PHB progression.
    FlagDrivenImmunity {
        flag: |a| a.has_passive_feature(
            crate::actions::class_features::BEGUILING_DEFENSES_TAG,
        ),
        suppressed: &[Condition::Charmed],
    },
    // 5e Eldritch Knight Fighter **Weapon Bond** (subclass lv3, PHB):
    // "you can't be disarmed of that weapon unless you are
    // incapacitated." The engine's only disarm source today is the
    // Battle Master's Disarming Attack maneuver, whose STR-save failure
    // installs `Disarmed`; the bond bounces that install.
    //
    // The compound gate is what keeps this RAW rather than a strictly
    // better `condition_immunities` entry: the closure checks the tag
    // AND `!is_incapacitated()`, so a Stunned / Paralyzed / Unconscious
    // knight — every condition that inherits Incapacitated — loses the
    // bond and can be disarmed normally. Same two-clause closure shape
    // as the Mindless Rage row above (tag AND live condition state),
    // which is the reason this lane exists at all rather than the flat
    // template set.
    FlagDrivenImmunity {
        flag: |a| {
            a.has_passive_feature(crate::actions::class_features::WEAPON_BOND_TAG)
                && !a.is_incapacitated()
        },
        suppressed: &[Condition::Disarmed],
    },
];

/// 5e Transmutation Wizard **Transmuter's Stone** (subclass level 6) —
/// which of the stone's benefits its holder currently carries.
///
/// RAW offers four: darkvision 60 ft, +10 ft walking speed,
/// proficiency in Constitution saving throws, or resistance to one of
/// acid / cold / fire / lightning / thunder. Three are modelled here.
/// Darkvision is dropped for the same reason the Diviner's Third Eye
/// drops its own darkvision option — the engine has no light level for
/// it to act on, so it would be a variant that does nothing.
///
/// Each surviving variant is read by the cohort that already owns its
/// effect, rather than by a Transmuter's-Stone-specific branch:
/// `Swiftness` by `PASSIVE_FEATURE_SPEED_BONUSES`, `Resilience` by
/// `FLAG_DRIVEN_SAVE_PROFICIENCIES`, `Warding` by
/// `PASSIVE_TYPED_RESISTANCES`. One feature, one field, three
/// pre-existing tables — which is why the stone costs three one-line
/// rows rather than three new engine surfaces.
///
/// `Warding` carries its damage type rather than fixing one, because
/// RAW's choice is per-attunement and the five options are not
/// interchangeable in play: fire resistance is worth far more against
/// a red dragon than thunder resistance is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransmutersStoneBenefit {
    /// +10 ft walking speed.
    Swiftness,
    /// Proficiency in Constitution saving throws — on a d6-hit-die
    /// INT-caster this is mostly a concentration-holding buff, and
    /// secondarily the poison / paralysis lane.
    Resilience,
    /// Resistance to one damage type (RAW: acid, cold, fire, lightning
    /// or thunder).
    Warding(DamageType),
}

/// One row in the `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort — a single
/// passive-feature flag that grants proficiency in a specific saving
/// throw ability. Sibling to `FlagDrivenImmunity` on the passive-
/// feature-flag-driven engine surface. Held as a function-pointer
/// rather than a field name so a new row can point to any predicate
/// on the actor (e.g. a future compound gate like Mindless Rage's
/// `feature-tag && condition-active` shape). All-ability grants
/// (Monk Diamond Soul, RAW lv14) still short-circuit before this
/// cohort in `is_save_proficient` since they don't key off a single
/// ability.
struct FlagDrivenSaveProficiency {
    flag: fn(&ActorInstance) -> bool,
    ability: AbilityScoreType,
}

/// Flag-driven save-proficiency cohort read by
/// `ActorInstance::is_save_proficient`. Sibling to
/// `FLAG_DRIVEN_IMMUNITIES` on the passive-feature-flag-driven engine
/// surface — each row promotes a single (flag, ability) pair to
/// "proficient", stacking on top of the actor's explicit
/// `proficient_saves` set. Adding a future single-ability save
/// proficiency feature (Warlock Aspect of the Moon, Bard Countercharm,
/// etc.) lands as one row here without touching the
/// `is_save_proficient` matcher body.
///
/// Distinct from Diamond Soul (Monk lv14, ALL saves) — that grant
/// short-circuits before this cohort since it doesn't key off a
/// single ability. Rows in this table are OR'd together with the
/// per-ability check; any row hit is sufficient.
const FLAG_DRIVEN_SAVE_PROFICIENCIES: &[FlagDrivenSaveProficiency] = &[
    // 5e Rogue Slippery Mind (lv15): proficiency in Wisdom saves. The
    // rogue's anti-Hold Person / anti-Dominate Person capstone.
    FlagDrivenSaveProficiency {
        flag: |a| a.has_slippery_mind,
        ability: AbilityScoreType::Wisdom,
    },
    // 5e Zealot Barbarian Iron Mind (subclass lv7): proficiency in
    // Wisdom saves. Same mechanical grant as Slippery Mind — kept as
    // a distinct flag so a rogue / zealot-barbarian multiclass would
    // carry both without conflation.
    FlagDrivenSaveProficiency {
        flag: |a| a.has_iron_mind,
        ability: AbilityScoreType::Wisdom,
    },
    // 5e Samurai Fighter Elegant Courtier (subclass lv7, XGtE):
    // proficiency in Wisdom saves. Third row on the WIS-save-
    // proficiency axis alongside Slippery Mind / Iron Mind — same
    // mechanical grant, distinct source (subclass tag closure via
    // `has_passive_feature(ELEGANT_COURTIER_TAG)` rather than a
    // dedicated struct-field flag, matching the "tag-only cross-class
    // helper" pattern the wizard `with_subclass_tag` chassis uses for
    // `NECROMANCY_WIZARD_TEMPLATE` / `WAR_MAGIC_WIZARD_TEMPLATE`). The
    // three never legally co-occur on a single build (Rogue vs. Ranger
    // vs. Barbarian vs. Fighter subclass slots), and a hypothetical
    // multiclass carrier picks up the proficiency via any single row
    // under the "any row hit is sufficient" OR semantic.
    FlagDrivenSaveProficiency {
        flag: |a| a.has_passive_feature(crate::actions::class_features::ELEGANT_COURTIER_TAG),
        ability: AbilityScoreType::Wisdom,
    },
    // 5e Transmutation Wizard **Transmuter's Stone** (subclass level 6),
    // attuned to Resilience: proficiency in Constitution saving throws.
    // The only row in this cohort keyed off a struct field carrying a
    // *choice* rather than a boolean — the transmuter always has the
    // stone, and this row fires only while it is set to Resilience.
    //
    // On a d6-hit-die INT-caster the grant is mostly a concentration
    // buff: the damage-driven CON save is the one an unproficient
    // wizard fails most, and it is the one that costs them the Web or
    // the Hold Monster they spent the turn on.
    FlagDrivenSaveProficiency {
        flag: |a| a.transmuters_stone == Some(TransmutersStoneBenefit::Resilience),
        ability: AbilityScoreType::Constitution,
    },
];

/// One row in the `RESIZING_CONDITIONS` cohort: a condition that moves
/// its holder along the size ladder while it is up. `steps` is +1 for a
/// growth effect and -1 for a shrink, and is summed then clamped by
/// `ActorInstance::desired_size` so two growth effects stacked on one
/// creature still only buy one category — RAW's "this spell has no
/// effect on a creature already enlarged" without needing each effect to
/// know about the others.
struct ResizingCondition {
    condition: Condition,
    steps: i32,
    /// Extra holder-side precondition beyond carrying `condition`, or
    /// `None` for the rows that resize whoever holds it.
    ///
    /// The Path of the Giant Barbarian's Giant's Havoc is the first row
    /// with one, and it is there because the condition it keys off —
    /// `Raging` — is carried by every barbarian on the roster. RAW
    /// grows the giant-path barbarian automatically while raging rather
    /// than through a press of its own, so the alternative to a gate
    /// here was a second condition installed by a bonus action that RAW
    /// does not charge.
    ///
    /// Named and shaped to match `OnHitConditionMark::holder_gate` and
    /// `OncePerTurnWeaponRiderSpec::caster_gate`, which grew the same
    /// column for the same reason.
    holder_gate: Option<fn(&ActorInstance) -> bool>,
}

/// Every condition that changes its holder's size category, and by how
/// much. Read only by `ActorInstance::desired_size`; the actual board
/// move is `EncounterInstance::reconcile_footprints`, which is the sole
/// writer of `ActorInstance::size`.
///
/// Entries:
///   - **Enlarged** (Enlarge / Reduce, the growth half; also the two
///     growth potions in `item_actions`): +1 category.
///   - **Reduced** (Enlarge / Reduce, the shrink half): -1 category.
///   - **GiantsMight** (Rune Knight Fighter, subclass level 3): +1
///     category.
///   - **Raging**, gated on `GIANT_STATURE_TAG` (Path of the Giant
///     Barbarian, subclass level 3): +1 category, and the only row that
///     carries a gate — see `ResizingCondition::holder_gate`.
///
/// A new growth or shrink effect lands as one row here and inherits the
/// room check, the retry-when-space-appears behavior, and the restore-on-
/// expiry for free.
const RESIZING_CONDITIONS: &[ResizingCondition] = &[
    ResizingCondition {
        condition: Condition::Enlarged,
        steps: 1,
        holder_gate: None,
    },
    ResizingCondition {
        condition: Condition::Reduced,
        steps: -1,
        holder_gate: None,
    },
    ResizingCondition {
        condition: Condition::GiantsMight,
        steps: 1,
        holder_gate: None,
    },
    // 5e Path of the Giant Barbarian **Giant's Havoc**, the Giant
    // Stature half (subclass level 3): "your size becomes Large, if
    // there is enough room."
    //
    // Keyed off `Raging` rather than off a condition of its own,
    // because RAW hands it out automatically the moment the rage starts
    // — there is no press, and inventing one would have charged the
    // barbarian a bonus action the rules do not. The gate is what keeps
    // the other sixteen barbarian builds their own size.
    ResizingCondition {
        condition: Condition::Raging,
        steps: 1,
        holder_gate: Some(|a| {
            a.has_passive_feature(crate::actions::class_features::GIANT_STATURE_TAG)
        }),
    },
];

/// One row in the `FLAG_DRIVEN_SAVE_ADVANTAGES` cohort — a single
/// passive-feature-driven advantage on saving throws. `flag` is a
/// closure over `(&ActorInstance, AbilityScoreType)` so a row can gate
/// on the specific ability being saved (Dwarven Resilience → CON only,
/// Gnome Cunning → INT/WIS/CHA only), on a blanket grant (Magic
/// Resistance → all abilities), or on a compound gate involving other
/// conditions (Barbarian Danger Sense → DEX only AND not
/// Blinded/Deafened/Incapacitated/Stunned). Held as a function-pointer
/// rather than a `(field_name, ability_filter)` struct so a new row
/// can compose an arbitrary predicate without expanding a central
/// filter enum — same shape the `FlagDrivenImmunity` /
/// `FlagDrivenSaveProficiency` / `PassiveTypedResistance` cohorts
/// already use for their respective engine surfaces.
///
/// Sibling to `FLAG_DRIVEN_SAVE_PROFICIENCIES` on the passive-feature-
/// flag-driven engine surface — that cohort promotes an ability to
/// "proficient" (adds prof bonus to the roll total), this cohort
/// promotes the roll SHAPE to advantage (2d20 kept high). The two
/// stack cleanly: a hypothetical Wisdom-save cast against an Iron-Mind
/// / Magic-Resistant chassis rolls 2d20 keep-high AND adds proficiency
/// bonus — both cohorts fire independently on their respective ability
/// gates.
struct FlagDrivenSaveAdvantage {
    flag: fn(&ActorInstance, AbilityScoreType) -> bool,
}

/// Flag-driven save-advantage cohort read by
/// `EncounterInstance::compute_save_mode`. Every row's flag is called
/// once per save with the actor + rolled-ability pair; any hit combines
/// `RollMode::Advantage` into the save mode. Sibling to
/// `FLAG_DRIVEN_SAVE_PROFICIENCIES` on the "passive-feature-flag-driven
/// save modifier" pattern — same closure shape, different roll axis
/// (roll mode vs. proficiency-bonus contribution).
///
/// Entries (in order):
///   - **Magic Resistance** (Balor / Pit Fiend / Lich / Rakshasa /
///     Solar racial): advantage on every save. RAW's "against spells
///     and magical effects" qualifier collapses to blanket advantage
///     since the engine doesn't tag save sources as spell / mundane —
///     the false-positive surface is small (nearly every combat save
///     originates from a spell effect).
///   - **Dwarven Resilience** (Dwarf racial): advantage on CON saves.
///     RAW's poison-save qualifier collapses to blanket CON-save
///     advantage since the poison-save trigger is CON-based and the
///     false-positive surface (CON saves vs non-poison effects) is
///     small.
///   - **Gnome Cunning** (Rock / Forest / Deep Gnome racial): advantage
///     on INT/WIS/CHA saves. RAW's "against magic" qualifier collapses
///     to blanket advantage on the three abilities — most saves in
///     this engine originate from spells.
///   - **Barbarian Danger Sense** (lv2): advantage on DEX saves against
///     effects the barbarian can see, while not blinded / deafened /
///     incapacitated / stunned. The compound sensory gate rides
///     inside the closure so the cohort row is a single entry rather
///     than a pre-cohort if-branch with three inline `!has_condition`
///     tails.
///
/// Adding a future passive save-advantage feature (a hypothetical
/// Aspect of the Sun on the Undying Warlock lane, a Ranger Land's
/// Stride save-advantage rider, etc.) lands as a one-line entry here
/// rather than another `if actor.has_...` branch scattered through
/// `compute_save_mode`.
///
/// Danger Sense's compound gate on Condition::Blinded / Deafened /
/// Incapacitated / Stunned lives inside the row's closure so the
/// cohort table stays declarative and the `compute_save_mode` body
/// collapses to a single filter-any pass over the cohort. All-ability
/// blanket rows (Magic Resistance) still ride the same closure shape
/// with a `_` ability wildcard.
const FLAG_DRIVEN_SAVE_ADVANTAGES: &[FlagDrivenSaveAdvantage] = &[
    // 5e Magic Resistance: advantage on all saving throws. Carried by
    // fiends (Balor, Pit Fiend), undead bosses (Lich), and other
    // magically-attuned creatures. We grant blanket advantage on every
    // save — see the cohort docstring for the RAW-qualifier collapse
    // rationale.
    FlagDrivenSaveAdvantage {
        flag: |a, _| a.has_magic_resistance(),
    },
    // 5e Dwarven Resilience: advantage on CON saves (RAW: vs poison
    // specifically; collapses to blanket CON-save advantage — see the
    // cohort docstring). Pairs with the poison-resistance half in
    // `effective_damage` via the `PASSIVE_TYPED_RESISTANCES` cohort.
    FlagDrivenSaveAdvantage {
        flag: |a, ability| {
            matches!(ability, AbilityScoreType::Constitution) && a.has_dwarven_resilience()
        },
    },
    // 5e Gnome Cunning: advantage on INT / WIS / CHA saves (RAW: vs
    // magic specifically; collapses to blanket three-ability advantage
    // — see the cohort docstring).
    FlagDrivenSaveAdvantage {
        flag: |a, ability| {
            matches!(
                ability,
                AbilityScoreType::Intelligence
                    | AbilityScoreType::Wisdom
                    | AbilityScoreType::Charisma
            ) && a.has_gnome_cunning()
        },
    },
    // 5e Barbarian Danger Sense (level 2): advantage on DEX saves
    // against effects you can see, while not blinded, deafened, or
    // incapacitated. The compound gate (four sensory conditions
    // suppress the advantage) rides inside this closure so the cohort
    // row stays a single declarative entry — Stunned is included
    // alongside Incapacitated per RAW since Stunned implicitly
    // Incapacitates.
    FlagDrivenSaveAdvantage {
        flag: |a, ability| {
            matches!(ability, AbilityScoreType::Dexterity)
                && a.has_danger_sense()
                && !a.has_condition(Condition::Blinded)
                && !a.has_condition(Condition::Deafened)
                && !a.has_condition(Condition::Incapacitated)
                && !a.has_condition(Condition::Stunned)
        },
    },
];

/// One row in the `PASSIVE_FEATURE_SPEED_BONUSES` cohort — a single
/// passive-feature-driven speed bump. `flag` is any predicate on the
/// actor (currently: `has_passive_feature(TAG)` for always-on passives,
/// `has_passive_feature(TAG) && has_condition(Raging)` for rage-gated
/// totems); when it returns true the row's `bonus_ft` is added to the
/// actor's walking speed. Held as a function-pointer rather than an
/// enum tag so a new row can point to any predicate without expanding
/// a central enum — same shape the `FlagDrivenImmunity` /
/// `FlagDrivenSaveProficiency` / `PassiveTypedResistance` cohorts
/// already use for their respective engine surfaces.
struct PassiveFeatureSpeedBonus {
    flag: fn(&ActorInstance) -> bool,
    bonus_ft: f32,
}

/// Passive-feature-driven speed-bonus cohort read by
/// `ActorInstance::passive_feature_speed_bonus`. Every row is summed
/// (with an OR-of-flags predicate); adding a fresh always-on /
/// rage-gated speed passive lands as a one-line entry here rather than
/// another `if actor.has_passive_feature(...) { bonus += N; }` branch
/// in `passive_feature_speed_bonus` or `condition_speed_bonus`.
///
/// Sibling to `FLAG_DRIVEN_IMMUNITIES` (condition-immunity),
/// `FLAG_DRIVEN_SAVE_PROFICIENCIES` (save-proficiency), and
/// `PASSIVE_TYPED_RESISTANCES` (damage-type resistance) on the
/// "passive-feature-flag-driven engine surface" pattern — same shape,
/// different lane.
///
/// Entries (in order):
///   - **Tiger Totem Spirit** (Barbarian Path of the Wild Heart, RAW
///     lv3): +10 ft **while raging** — the flag composes a condition
///     check (Raging) with the passive tag so an unraged tiger has no
///     bonus. Stacks additively with Fast Movement on a raging
///     barbarian.
///   - **Elk Totem Spirit** (Barbarian Path of the Totem Warrior, RAW
///     XGtE lv3): +15 ft **while raging** — same compound gate as
///     Tiger but a bigger sprint magnitude. Distinct from Tiger only
///     in the magnitude; the two flags never legally co-occur on a
///     single subclass build.
///   - **Fast Movement** (Barbarian lv5): +10 ft always-on (RAW's
///     "not wearing heavy armor" clause collapses to "always" since
///     the engine doesn't model armor tiers).
///   - **Unarmored Movement** (Monk lv2): +10 ft always-on (RAW's
///     "not wearing armor and no shield" clause collapses to "always"
///     since the engine doesn't model armor tiers). Read via
///     `UNARMORED_MOVEMENT_SPEED_BONUS` so the magnitude stays
///     declarative next to the tag definition.
///   - **Roving** (Ranger 2024 lv6 optional class feature): +5 ft
///     always-on. The +5 is smaller than Fast Movement / Unarmored
///     Movement — rangers kite half a step further, not sprint like
///     a raging barbarian or a monk.
///   - **Superior Mobility** (Scout Rogue lv9, XGtE): +10 ft
///     always-on. Same magnitude as Fast Movement / Unarmored Movement
///     — three unrelated class chassis (Barbarian / Monk / Rogue-Scout)
///     converge on the same +10 lane. Stacks additively with Roving
///     under the multiclass rule via the same cohort. RAW also grants
///     matching climbing / swimming speeds; both fold into the walking
///     `speed()` accessor since the engine has no 3D terrain to
///     differentiate.
///   - **Wolverine Totem Spirit** (Barbarian Path of the Wild Heart
///     2024 lv3): +10 ft **while raging** — same compound gate as
///     Tiger / Elk and same +10 magnitude as Tiger, but a distinct
///     tag so a Tiger-vs-Wolverine encounter renders unambiguously
///     and the two flags never legally co-occur on a single PC.
///   - **Panther Totem Spirit** (Barbarian Path of the Totem Warrior
///     XGtE lv3): +5 ft **while raging** — smallest magnitude in the
///     rage-gated totem lane (Panther +5, Tiger +10, Wolverine +10,
///     Elk +15). RAW's "climbing speed equal to walking" clause folds
///     into a slim flat walking-speed bump since the engine has no 3D
///     terrain to differentiate the climb axis.
const PASSIVE_FEATURE_SPEED_BONUSES: &[PassiveFeatureSpeedBonus] = &[
    PassiveFeatureSpeedBonus {
        flag: |a| {
            a.has_condition(Condition::Raging)
                && a.has_passive_feature(crate::actions::class_features::TIGER_TOTEM_TAG)
        },
        bonus_ft: crate::actions::class_features::TIGER_TOTEM_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| {
            a.has_condition(Condition::Raging)
                && a.has_passive_feature(crate::actions::class_features::ELK_TOTEM_TAG)
        },
        bonus_ft: crate::actions::class_features::ELK_TOTEM_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| {
            a.has_condition(Condition::Raging)
                && a.has_passive_feature(crate::actions::class_features::WOLVERINE_TOTEM_TAG)
        },
        bonus_ft: crate::actions::class_features::WOLVERINE_TOTEM_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| {
            a.has_condition(Condition::Raging)
                && a.has_passive_feature(crate::actions::class_features::PANTHER_TOTEM_TAG)
        },
        bonus_ft: crate::actions::class_features::PANTHER_TOTEM_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::FAST_MOVEMENT_TAG),
        bonus_ft: crate::actions::class_features::FAST_MOVEMENT_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::UNARMORED_MOVEMENT_TAG),
        bonus_ft: crate::actions::class_features::UNARMORED_MOVEMENT_SPEED_BONUS,
    },
    PassiveFeatureSpeedBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::ROVING_TAG),
        bonus_ft: crate::actions::class_features::ROVING_SPEED_BONUS,
    },
    // 5e Scout Rogue **Superior Mobility** (subclass level 9, XGtE) —
    // passive +10 ft walking-speed bump on the scout rogue chassis. RAW
    // also grants matching climbing / swimming speeds; both fold into
    // the walking `speed()` accessor since the engine has no 3D terrain
    // to differentiate. Same magnitude as Fast Movement (Barbarian +10)
    // and Unarmored Movement (Monk +10) — three unrelated class chassis
    // converge on the same +10 always-on lane; stacks additively with
    // Roving (+5) under the multiclass rule via the same cohort. Ships
    // on `SCOUT_ROGUE_TEMPLATE` above its strict RAW lv9 gate for the
    // same reason `ASSASSIN_ROGUE_TEMPLATE` / `SWASHBUCKLER_ROGUE_TEMPLATE`
    // ship their lv3 subclass features on the CR-1 baseline chassis —
    // class templates target a balanced playable level, not lockstep
    // PHB progression.
    PassiveFeatureSpeedBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::SUPERIOR_MOBILITY_TAG),
        bonus_ft: crate::actions::class_features::SUPERIOR_MOBILITY_SPEED_BONUS,
    },
    // 5e Glory Paladin **Aura of Alacrity** (subclass level 7, TCE) —
    // passive +10 ft walking-speed bump on the paladin chassis. The
    // RAW aura-extends-to-adjacent-allies half needs a per-turn-start
    // aura scan surface this engine doesn't expose as a first-class
    // hook today; the self-side +10 ft is the load-bearing tactical
    // piece and folds here through the same table every other
    // passive-speed-bump row already uses. Same magnitude as Fast
    // Movement (Barbarian +10) / Unarmored Movement (Monk +10) /
    // Superior Mobility (Scout Rogue +10) — a fifth class chassis
    // converges on the same +10 always-on lane; stacks additively
    // with the +5 Roving row under the multiclass rule via the same
    // cohort. Ships on `GLORY_PALADIN_TEMPLATE` at (or above) its
    // strict RAW lv7 gate for the same reason `SCOUT_ROGUE_TEMPLATE`
    // ships Superior Mobility (RAW lv9) — class templates target a
    // balanced playable level, not lockstep PHB progression.
    PassiveFeatureSpeedBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::AURA_OF_ALACRITY_TAG),
        bonus_ft: crate::actions::class_features::AURA_OF_ALACRITY_SPEED_BONUS,
    },
    // 5e Transmutation Wizard **Transmuter's Stone** (subclass level 6),
    // attuned to Swiftness: +10 ft walking speed. Keys off the stone
    // field rather than a feature tag — the tag marks that the wizard
    // *has* a stone, the field says which benefit it is set to, and
    // only this row's benefit grants speed. Same +10 magnitude as Fast
    // Movement / Unarmored Movement / Superior Mobility / Aura of
    // Alacrity, on a sixth chassis; stacks additively through this
    // cohort like every other row.
    PassiveFeatureSpeedBonus {
        flag: |a| a.transmuters_stone == Some(TransmutersStoneBenefit::Swiftness),
        bonus_ft: TRANSMUTERS_STONE_SPEED_BONUS,
    },
];

/// The Transmuter's Stone Swiftness attunement's walking-speed bump, in
/// feet. RAW: "+10 feet to the holder's walking speed."
pub const TRANSMUTERS_STONE_SPEED_BONUS: f32 = 10.0;

/// One row in the `CONDITION_SPEED_BONUSES` cohort — a single
/// condition-driven speed bump. `flag` is any predicate on the actor
/// (usually `has_condition(C)`, occasionally a triple-OR compound like
/// the Flying / InvestedInWind / OtherworldlyGuised "any-flavor-of-
/// magic-flight" cohort); when it returns true the row's `bonus_ft`
/// is added to the actor's walking speed. Sibling to
/// `PassiveFeatureSpeedBonus` on the "cohort of `(flag_closure,
/// bonus_ft)` rows" pattern — same shape, different lane
/// (condition-driven vs. always-on / rage-gated passive-feature-tag-
/// driven).
struct ConditionSpeedBonus {
    flag: fn(&ActorInstance) -> bool,
    bonus_ft: f32,
}

/// Every condition that puts an actor in the air under its own power.
///
/// Read by `ActorInstance::has_magical_flight` to ask "is this actor
/// flying?", and by the **Earthbind** spell to make one stop — RAW's
/// "the target's flying speed (if any) becomes 0 feet", which only
/// means anything if the spell knows every way a flying speed can have
/// been granted.
///
/// That second consumer is why this is a list rather than three `||`s
/// inside the predicate. Earthbind used to name `Flying` and
/// `InvestedInWind` and stop, with a comment calling them "both flight
/// sources" — so a warlock aloft on **Otherworldly Guise** simply
/// ignored the spell. The two lanes could disagree because there was
/// nothing for them to disagree *with*. Now a fourth flight source is
/// one row here and both lanes pick it up.
///
/// `Lifted` (Telekinesis) is deliberately absent: it is someone else's
/// concentration holding the target up, not a flying speed of their
/// own, so Earthbind has nothing to strip. It joins this cohort only at
/// `is_grounded`, which asks the different question of whether the
/// actor's feet are on the floor.
pub const MAGICAL_FLIGHT_CONDITIONS: &[Condition] = &[
    Condition::Flying,
    Condition::InvestedInWind,
    Condition::OtherworldlyGuised,
];

/// Condition-driven speed-bonus cohort read by
/// `ActorInstance::condition_speed_bonus`. Every row is summed (with
/// an OR-of-flags predicate); adding a fresh condition-driven speed
/// buff (a hypothetical Wind Walk transmutation, a Boots of Elvenkind
/// speed rider, a Warding Wind reverse-slow, etc.) lands as a
/// one-line entry here rather than another
/// `if self.has_condition(...) { bonus += N; }` branch in
/// `condition_speed_bonus`.
///
/// Sibling to `PASSIVE_FEATURE_SPEED_BONUSES` on the "cohort of
/// `(flag_closure, bonus_ft)` rows" pattern — the two tables split by
/// source-of-the-flag: passive-feature tag (there) vs. held condition
/// (here).
///
/// Entries (in order):
///   - **Fly / Investiture of Wind / Otherworldly Guise**: +60 ft
///     flying speed. Any one is sufficient (RAW: the three effects
///     don't stack — they're separate concentration spells the caster
///     can't both maintain), so the row reads the shared
///     `has_magical_flight` predicate rather than three separate rows
///     with matching magnitudes.
///   - **Spider Climb**: +30 ft. RAW grants a climbing speed equal to
///     walking speed; the engine doesn't model 3D terrain, so the
///     bonus surfaces as a flat repositioning boost.
///   - **Longstrider**: +10 ft (1-hour transmutation buff).
///   - **Expeditious Retreat**: +30 ft (Dash-as-bonus collapsed to a
///     flat speed bump, concentration-bound).
///   - **Ashardalon's Stride** (Fizban's transmutation): +20 ft.
const CONDITION_SPEED_BONUSES: &[ConditionSpeedBonus] = &[
    ConditionSpeedBonus {
        flag: ActorInstance::has_magical_flight,
        bonus_ft: 60.0,
    },
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::SpiderClimbing),
        bonus_ft: 30.0,
    },
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::Longstriding),
        bonus_ft: 10.0,
    },
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::ExpeditiouslyRetreating),
        bonus_ft: 30.0,
    },
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::AshardalonStriding),
        bonus_ft: 20.0,
    },
    // 5e Fathomless Warlock **Tentacle of the Deep**: "its speed is
    // reduced by 10 feet until the start of your next turn". The first
    // negative row on the cohort, and the reason `bonus_ft` is signed —
    // a slow and a buff on the same creature sum rather than fighting
    // over which one the code checked first.
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::Coiled),
        bonus_ft: -10.0,
    },
    // 5e Bladesinging Wizard **Bladesong** (subclass level 2): "your
    // walking speed increases by 10 feet." One of the three clauses the
    // trance grants; the AC bump rides
    // `ABILITY_SCALED_AC_BONUSES` and the concentration-save bump lives
    // at `roll_concentration_save`, because neither is a flat number
    // and neither applies to every roll of its kind.
    ConditionSpeedBonus {
        flag: |a| a.has_condition(Condition::Bladesinging),
        bonus_ft: 10.0,
    },
];

/// One row in a **boolean cohort** — a single source of some yes/no
/// property an actor can have, expressed as a predicate over the actor.
/// `flag` is any such predicate (a held condition, a passive-feature
/// tag, or an OR of several); one matching row is enough, so every
/// cohort built from these is read with `.any()` rather than summed.
///
/// Shape sibling to `PassiveFeatureSpeedBonus` / `ConditionSpeedBonus`
/// minus the magnitude — the property is a boolean, so the row is the
/// flag alone. Same reason those two hold a function pointer instead of
/// an enum tag: a new row points at any predicate without expanding a
/// central enum.
///
/// Deliberately named for its *shape* rather than for a cohort, because
/// three of them share it and no cohort's name would be right for the
/// other two: `DIFFICULT_TERRAIN_IMMUNITIES`, `WATER_SURCHARGE_IMMUNITIES`
/// and `SWIM_SPEED_SOURCES`. It carried the first cohort's name while
/// it had only one, and the second and third arriving is what made that
/// a lie rather than a shorthand.
struct ActorFlagRow {
    flag: fn(&ActorInstance) -> bool,
}

/// Difficult-terrain-immunity cohort read by
/// `ActorInstance::ignores_difficult_terrain`, which the pathfinder
/// consults once per candidate step to decide whether
/// `TerrainType::movement_cost` applies at all.
///
/// Before this cohort existed, `dijkstra_path` multiplied every step
/// onto a `DifficultTerrain` tile by 2.0 with no escape hatch, which
/// made two shipped effects lie about what they do: `Condition::Footloose`
/// (Freedom of Movement) documents "the target ignores difficult
/// terrain" and did not, and a creature flying under `Fly` /
/// `Investiture of Wind` / `Otherworldly Guise` was charged for rubble
/// it was sixty feet above. Both are rows here now, and the lane is
/// open for the content that wants it.
///
/// Entries (in order):
///   - **Freedom of Movement** (`Footloose`, lv4 abjuration): RAW's
///     "the target's movement is unaffected by difficult terrain" — the
///     clause the condition's own docstring already promised. The
///     condition's other half (dynamic immunity to Paralyzed /
///     Restrained / Grappled) sits on `dynamic_immunity_to`; this is
///     the movement half.
///   - **Magical flight** (`Flying` / `InvestedInWind` /
///     `OtherworldlyGuised`): a creature in the air doesn't wade
///     through the mud under it. Deliberately the same
///     `has_magical_flight` predicate the `CONDITION_SPEED_BONUSES`
///     flight row uses, so the two lanes can never disagree about what
///     counts as flying — an actor getting the +60 ft flying-speed
///     bump is exactly an actor that skips the terrain tax.
///   - **Land's Stride** (`LANDS_STRIDE_TAG`, Ranger lv8 / Land Druid
///     lv6): the class-feature row, and the first one that is a build
///     choice rather than a spell effect. See the tag's docstring for
///     which RAW clauses ship.
const DIFFICULT_TERRAIN_IMMUNITIES: &[ActorFlagRow] = &[
    ActorFlagRow {
        flag: |a| a.has_condition(Condition::Footloose),
    },
    ActorFlagRow {
        flag: ActorInstance::has_magical_flight,
    },
    ActorFlagRow {
        flag: |a| a.has_passive_feature(crate::actions::class_features::LANDS_STRIDE_TAG),
    },
];

/// Sources of "this actor pays no movement surcharge for
/// `TerrainType::Water`" — read by `ActorInstance::swims_freely`, the
/// water-side twin of `ignores_difficult_terrain`.
///
/// Two cohorts rather than one because 5e prices swimming and difficult
/// terrain identically and waives them differently, and every row below
/// is a row that is *not* on `DIFFICULT_TERRAIN_IMMUNITIES` or is there
/// for a different reason:
///
///   - **A swimming speed** (`SWIM_SPEED_TAG`) is the whole rule — "if
///     you have a swimming speed, you can use it to swim without
///     spending extra movement" — and it does nothing whatsoever for
///     rubble. The shark is not nimble on land.
///   - **Land's Stride** is the mirror image and is deliberately absent
///     here: RAW scopes it to "nonmagical difficult terrain", which a
///     lake is not. A ranger crossing a river swims like anybody else.
///   - **Freedom of Movement** is on both, and it is the only row that
///     is, because RAW puts it on both: "the target's movement is
///     unaffected by difficult terrain… being underwater imposes no
///     penalties on the target's movement or attacks."
///   - **Magical flight** is on both for a reason that is not a rule at
///     all — a creature sixty feet up is not swimming, in the same
///     sense and for the same reason it is not wading through the mud.
///     It is the one row here that answers "is this actor in the water"
///     rather than "does the water charge this actor", which is why
///     `EncounterInstance::is_immersed` reads the same predicate: a
///     flying creature that paid nothing to cross a lake must also not
///     be swinging at disadvantage over it.
const WATER_SURCHARGE_IMMUNITIES: &[ActorFlagRow] = &[
    ActorFlagRow {
        flag: ActorInstance::has_swim_speed,
    },
    ActorFlagRow {
        flag: |a| a.has_condition(Condition::Footloose),
    },
    ActorFlagRow {
        flag: ActorInstance::has_magical_flight,
    },
    // 5e **Water Walk**: "move across any liquid surface as if it were
    // harmless solid ground." The surcharge half of the spell. Its
    // other half is at `EncounterInstance::is_immersed` — a
    // water-walker is on the surface rather than in it, which is the
    // one thing separating this row from the swimming speed above it.
    ActorFlagRow {
        flag: |a| a.has_condition(Condition::WaterWalking),
    },
];

/// Sources of a swimming speed, read by `ActorInstance::has_swim_speed`.
///
/// Split out from `WATER_SURCHARGE_IMMUNITIES` rather than folded into
/// it because the two answer different questions and only one of them
/// is "a swimming speed". 5e's melee Underwater Combat clause is
/// specifically "a creature that doesn't have a swimming speed", and a
/// wizard hovering on *Fly* does not have one — they are simply not in
/// the water, which is a different exemption arriving down a different
/// road. Keeping the narrower predicate separate is what lets
/// `UnderwaterVerdict::for_attack` take `swims` and `waived` as two
/// arguments and give the bow and the trident different answers.
const SWIM_SPEED_SOURCES: &[ActorFlagRow] = &[
    ActorFlagRow {
        flag: |a| a.has_passive_feature(crate::actions::class_features::SWIM_SPEED_TAG),
    },
    // 5e Scout Rogue **Superior Mobility** (subclass lv9, XGtE): "you
    // also gain a climbing speed and a swimming speed equal to your
    // walking speed." The tag's own docstring used to note that the
    // swimming half had no combat surface in this engine because there
    // was no water to swim in; there is now, and this row is that
    // sentence finally meaning something.
    ActorFlagRow {
        flag: |a| a.has_passive_feature(crate::actions::class_features::SUPERIOR_MOBILITY_TAG),
    },
    // 5e Ranger **Roving** (2024 PHB lv6): "you also have a Climb Speed
    // and a Swim Speed equal to your Speed." The sibling clause to the
    // Scout's, on the other of the two chassis that ship one — and the
    // one place Roving and Land's Stride, which the ranger also carries,
    // stop overlapping: Land's Stride is scoped to nonmagical difficult
    // terrain and a lake is not that.
    ActorFlagRow {
        flag: |a| a.has_passive_feature(crate::actions::class_features::ROVING_TAG),
    },
];

/// The 5e exhaustion ladder, one constant per rung, named for what the
/// rung does rather than for its number.
///
/// Each tier's effect is cumulative with every tier below it, so the
/// gates are all `>=`. They are constants rather than literals for the
/// usual reason — six sites in four modules read them, and a bare `3`
/// at a save site says nothing about why — but also because the ladder
/// is the one part of exhaustion a table is likely to house-rule, and a
/// house rule that moves a rung should be one edit.
///
/// Tier 1 — disadvantage on ability checks. Read by
/// `EncounterInstance::compute_check_mode`.
pub const EXHAUSTION_CHECK_DISADVANTAGE_TIER: u32 = 1;
/// Tier 2 — speed halved. Rides `CONDITION_SPEED_MULTIPLIERS` as a
/// ×0.5 factor, so it composes with Haste and Slow the way every other
/// speed multiplier does.
pub const EXHAUSTION_HALF_SPEED_TIER: u32 = 2;
/// Tier 3 — disadvantage on attack rolls *and* saving throws. The rung
/// the old single-flag model collapsed the whole ladder onto: before
/// the tiers existed, one application of exhaustion handed out this
/// penalty plus tier 1's immediately.
pub const EXHAUSTION_ROLL_PENALTY_TIER: u32 = 3;
/// Tier 4 — hit point maximum halved. Read by `max_hitpoints`; current
/// HP is clipped to the new ceiling as the tier lands.
pub const EXHAUSTION_HALF_HP_TIER: u32 = 4;
/// Tier 5 — speed 0. Read by `remaining_movement` alongside the
/// `zeros_movement` condition cohort, rather than as another speed
/// multiplier, because RAW's "speed 0" is not a number a Dash can add
/// to.
pub const EXHAUSTION_ZERO_SPEED_TIER: u32 = 5;
/// Tier 6 — death. Also the cap: a creature cannot hold more
/// exhaustion than the amount that kills it.
pub const EXHAUSTION_DEATH_TIER: u32 = 6;

/// One row in the `CONDITION_SPEED_MULTIPLIERS` cohort — a single
/// condition whose presence applies a multiplicative factor to the
/// holder's final walking speed. Sibling to `ConditionSpeedBonus` on
/// the "cohort of `(flag_closure, magnitude)` rows" pattern — same
/// shape, different composition axis: the additive bumps
/// (Longstrider / Fly / Spider Climb / ...) fold in first through
/// `condition_speed_bonus`, then the multiplicative factors here stack
/// on top via `.product()`. Adding a new speed multiplier lands as a
/// one-line table entry rather than another `if self.has_condition(...)
/// { factor *= N; }` branch in `speed()`.
struct ConditionSpeedMultiplier {
    flag: fn(&ActorInstance) -> bool,
    factor: f32,
}

/// Multiplicative walking-speed factor cohort read by `speed()` after
/// additive bonuses fold in. Every matching row's `factor` composes
/// via `.product()` so multiple simultaneous multipliers compound
/// correctly (Haste ×2 + Slow ×½ ⇒ ×1 = base; Slow ×½ + Power Word
/// Pain ×½ ⇒ ×¼ if both landed on the same target through separate
/// concentration chains).
///
/// Sibling to `CONDITION_SPEED_BONUSES` (additive flat bumps) — the
/// two tables split by composition axis (add-then-multiply):
/// additive bumps land first, then multiplicative factors apply.
///
/// Entries (in order):
///   - **Hasted** (5e Haste spell, concentration): ×2 walking speed.
///     Paired with the +2 AC / advantage-on-DEX-saves / extra-attack
///     riders on the sibling cohorts.
///   - **Slowed** (5e Slow spell, concentration): ×½ walking speed.
///     Paired with the -2 AC / -2 DEX-save riders on the sibling
///     cohorts — the compound debuff shape distinguishes Slowed from
///     Power Word: Pain's pure-speed-and-attack-disadvantage lane.
///   - **PowerWordPained** (5e Power Word: Pain, XGtE level-7
///     necromancy, concentration): ×½ walking speed. Distinct from
///     Slowed on the "no AC / DEX-save penalty" axis — the pain
///     rides the pure attack-disadvantage + half-speed lane so DEX-
///     anchored bursts still save at full bonus and the target's AC
///     stays intact. RAW's per-turn CON-save-to-break loop rides the
///     shared `ROUND_END_SAVES` table next to Hold Person / Hideous
///     Laughter / Flesh to Stone.
const CONDITION_SPEED_MULTIPLIERS: &[ConditionSpeedMultiplier] = &[
    ConditionSpeedMultiplier {
        flag: |a| a.has_condition(Condition::Hasted),
        factor: 2.0,
    },
    ConditionSpeedMultiplier {
        flag: |a| a.has_condition(Condition::Slowed),
        factor: 0.5,
    },
    ConditionSpeedMultiplier {
        flag: |a| a.has_condition(Condition::PowerWordPained),
        factor: 0.5,
    },
    // 5e exhaustion tier 2: "speed halved". The first row on this
    // cohort gated on something other than a bare condition flag, which
    // is what the `flag` closure was for — the ladder's tiers are a
    // number, not six conditions. Tier 5's "speed 0" deliberately does
    // *not* ride here: see `remaining_movement`.
    ConditionSpeedMultiplier {
        flag: |a| a.exhaustion_level() >= EXHAUSTION_HALF_SPEED_TIER,
        factor: 0.5,
    },
];

/// One row in the ability-scaled condition-bonus cohorts — a condition
/// whose bonus is not a compile-time number but the holder's own
/// modifier in some ability, read at the moment the bonus is asked for.
///
/// The flat cohorts (`CONDITION_AC_BONUSES`,
/// `CONDITION_ATTACK_BONUSES`, `CONDITION_SAVE_BONUSES`) each carry an
/// `i32` per row, which is right for Shield's +5 and Bless's +2 and
/// wrong for anything that scales with the holder. `condition_attack_bonus`
/// carried the Paladin's Sacred Weapon (+CHA) as a hand-written branch
/// underneath its cohort walk for exactly that reason, with a comment
/// noting that a second variable-magnitude rider would justify a
/// closure-based sibling table. The Bladesinger's +INT to AC is that
/// second rider, so this is that table.
struct AbilityScaledConditionBonus {
    /// Source condition whose presence gates the row.
    source: Condition,
    /// Ability whose modifier the row contributes while `source` is
    /// held. Read off the holder rather than the installer, so a
    /// monster who somehow picks the buff up scales off its own stat
    /// block.
    ability: AbilityScoreType,
    /// Floor applied to the modifier. RAW writes "(minimum of +1)" into
    /// some of these features and not others; `0` means "no floor
    /// beyond the natural one", which is what Sacred Weapon wants.
    floor: i32,
}

/// Ability-scaled attack-roll bonuses. Read by
/// `condition_attack_bonus` after the flat `CONDITION_ATTACK_BONUSES`
/// walk; the two sums add.
const ABILITY_SCALED_ATTACK_BONUSES: &[AbilityScaledConditionBonus] = &[
    // 5e Channel Divinity: Sacred Weapon — the paladin's weapon glows
    // with divine light, adding their Charisma modifier to attack
    // rolls. No RAW minimum.
    AbilityScaledConditionBonus {
        source: Condition::Sacred,
        ability: AbilityScoreType::Charisma,
        floor: 0,
    },
];

/// Ability-scaled AC bonuses. Read by `condition_ac_bonus` after the
/// flat `CONDITION_AC_BONUSES` walk; the two sums add, so a
/// bladesinging wizard under Shield of Faith gets both.
const ABILITY_SCALED_AC_BONUSES: &[AbilityScaledConditionBonus] = &[
    // 5e Bladesinging Wizard **Bladesong** (subclass level 2): "you gain
    // a bonus to your AC equal to your Intelligence modifier (minimum
    // of +1)." The minimum is RAW and is why `floor` exists on the row
    // shape at all.
    AbilityScaledConditionBonus {
        source: Condition::Bladesinging,
        ability: AbilityScoreType::Intelligence,
        floor: 1,
    },
];

/// One row in the `CONDITION_AC_BONUSES` cohort — a single condition
/// whose presence contributes a flat AC delta to the holder. Sibling to
/// `ConditionSpeedBonus { flag, bonus_ft }` on the "cohort of `(source
/// closure / condition, magnitude)` rows" pattern — same declarative
/// shape, different lane (AC delta vs. speed delta) and different unit
/// (integer AC points vs. feet). The delta is signed so a debuff-side
/// entry (Slowed → -2 AC) sits on the same table as the buff-side
/// entries without a separate cohort.
struct ConditionAcBonus {
    /// Source condition whose presence gates the row. Every AC-bump
    /// buff currently uses a plain single-condition gate, so this is
    /// a bare `Condition` rather than a closure — the shape stays
    /// symmetric with `ConditionDrivenTypedResistance { source, types
    /// }` and `ConditionDrivenConditionImmunity { source, suppressed
    /// }` on the "single-condition source" cohort lane. A future
    /// compound-gate buff (a hypothetical "+2 AC while Raging and
    /// Concentrating") would either widen this row to a `flag`
    /// closure (matching `ConditionSpeedBonus`) or land as a new
    /// sibling cohort with the compound predicate — the shape is
    /// deliberately narrower here than on the speed cohort to keep
    /// the common single-condition case a bare row.
    source: Condition,
    /// Signed AC delta contributed while `source` is held. Positive
    /// for a buff (Shield of Faith +2, Shielded +5, Hasted +2,
    /// WardingBonded +1, OtherworldlyGuised +2); negative for a
    /// debuff (Slowed -2). Read by `condition_ac_bonus` as a plain
    /// integer sum so vertical stacking (Shield of Faith + Shielded +
    /// Hasted → +9 AC) folds through the same walk as horizontal
    /// stacking (Hasted + Slowed → 0 AC — the two riders cancel).
    bonus: i32,
}

/// Condition-driven AC-bonus cohort read by
/// `ActorInstance::condition_ac_bonus`. Every row's `bonus` is summed
/// (signed) when the row's `source` condition is held; the resulting
/// delta is added on top of `armor_class`'s post-floor base. Adding a
/// fresh condition-driven AC bump (a hypothetical Sanctuary +4 AC on
/// the caster, a Blur-adjacent visual-obscurement AC bump, etc.)
/// lands as a one-line entry here rather than another
/// `if self.has_condition(...) { bonus += N; }` branch in
/// `condition_ac_bonus`.
///
/// Sibling to `CONDITION_SPEED_BONUSES` on the "cohort of `(source,
/// magnitude)` rows" pattern — the two tables split by affected axis:
/// AC delta here vs. speed delta there. Sibling to
/// `TYPED_RESISTANCE_CONDITIONS` / `CONDITION_DRIVEN_IMMUNITIES` on
/// the "single-condition source" cohort lane — same `source:
/// Condition` row shape, different affected axis (AC points vs.
/// damage-type resistance vs. condition-install immunity).
///
/// Entries (order doesn't affect the summed total; grouped by
/// buff-vs-debuff flavor for readability):
///   - **Shield of Faith** (`ShieldOfFaith`, +2 AC): the lv1 Cleric /
///     Paladin abjuration; concentration-bound. Sibling on the "spell-
///     installed AC buff" lane to Shield / Mage Armor / Barkskin but
///     the latter three flow through `ac_floor` / a template flag rather
///     than a flat bonus.
///   - **Shield** (`Shielded`, +5 AC): the Wizard / Sorcerer / Warlock
///     lv1 reaction spell; installs the `Shielded` condition until
///     start of the caster's next turn. RAW value.
///   - **Haste** (`Hasted`, +2 AC): the Wizard / Sorcerer lv3
///     transmutation; concentration-bound. Pairs with the +30 ft speed
///     bump handled elsewhere (Haste's speed clause folds into the
///     `Flying` / `InvestedInWind` / speed-doubling accessors rather
///     than this cohort).
///   - **Slow** (`Slowed`, -2 AC): the Wizard / Sorcerer lv3
///     transmutation debuff — RAW imposes -2 AC on the target for the
///     duration. The only debuff-side row on this cohort; the signed
///     delta pattern lets it ride the same table as the +2 / +5 buff
///     rows.
///   - **Warding Bond** (`WardingBonded`, +1 AC): the Cleric /
///     Paladin lv2 abjuration; installs the `WardingBonded` condition
///     on the bonded ally. RAW grants +1 AC AND +1 saves AND resistance
///     to all damage AND mirror damage — the +1 AC clause lives here,
///     the +1 save clause on the sibling `condition_save_bonus` /
///     future `CONDITION_SAVE_BONUSES` cohort, the resistance and
///     mirror-damage clauses on their own lanes.
///   - **Otherworldly Guise** (`OtherworldlyGuised`, +2 AC): the
///     Warlock lv6 self-buff (Tasha's Otherworldly Guise) — the
///     extraplanar shell's +2 AC clause. Sibling to Haste on the
///     concentration-bound +2 AC lane; distinguished by the concurrent
///     +60 ft fly speed (on `CONDITION_SPEED_BONUSES`) and typed
///     resistances (radiant + poison, on the resistance lane).
const CONDITION_AC_BONUSES: &[ConditionAcBonus] = &[
    ConditionAcBonus {
        source: Condition::ShieldOfFaith,
        bonus: 2,
    },
    ConditionAcBonus {
        source: Condition::Shielded,
        bonus: 5,
    },
    ConditionAcBonus {
        source: Condition::Hasted,
        bonus: 2,
    },
    ConditionAcBonus {
        source: Condition::Slowed,
        bonus: -2,
    },
    ConditionAcBonus {
        source: Condition::WardingBonded,
        bonus: 1,
    },
    ConditionAcBonus {
        source: Condition::OtherworldlyGuised,
        bonus: 2,
    },
];

/// One row in the `CONDITION_SAVE_BONUSES` / `CONDITION_CHECK_BONUSES`
/// cohorts — a single condition whose presence contributes a flat d20-
/// roll delta to the holder. Sibling to `ConditionAcBonus { source,
/// bonus }` on the "single-condition source + signed magnitude" cohort
/// lane — same declarative row shape, different affected axis (a rolled
/// d20 total here vs. a static AC there).
///
/// The row shape is deliberately not save-specific — it was named for
/// its first cohort, and the check lane needs exactly the same two
/// fields, so the two cohorts share one row type rather than declaring
/// a second identical struct. The delta is signed for the same reason
/// `ConditionAcBonus.bonus` is signed: a save-debuff row (Unsettling
/// Words' −4) sits on the same table as the +1 / +3 buff rows without
/// a separate cohort. Bless / Bane deliberately don't ride this cohort
/// — their d4 die lives on `bless_bane_attack_die` and their +N / -N
/// flat lives on `save_bonus_buff`, so including them here would
/// double-count.
struct ConditionRollBonus {
    /// Source condition whose presence gates the row. Every save-bump
    /// buff currently uses a plain single-condition gate, so this is a
    /// bare `Condition` matching the `ConditionAcBonus.source` shape.
    /// A future compound-gate save buff (a hypothetical "+2 saves while
    /// Concentrating on a specific spell") would either widen this row
    /// to a `flag` closure or land as a sibling cohort with the
    /// compound predicate — the shape is deliberately narrower here
    /// than a closure-based cohort to keep the common single-condition
    /// case a bare row.
    source: Condition,
    /// Signed save delta contributed while `source` is held. Positive
    /// for a buff (Inspired +3, WardingBonded +1); a future negative
    /// entry for a save-debuff would sit here as a signed delta.
    /// Summed by `condition_save_bonus` so vertical stacking (Inspired
    /// + WardingBonded → +4 saves) folds through the same walk as
    ///   horizontal stacking on the sibling `CONDITION_AC_BONUSES`
    ///   cohort.
    bonus: i32,
}

/// Condition-driven save-bonus cohort read by
/// `ActorInstance::condition_save_bonus`. Every row's `bonus` is summed
/// (signed) when the row's `source` condition is held; the resulting
/// delta is added to the actor's save total via the shared save-mode
/// composition path. Adding a fresh condition-driven save bump (a
/// hypothetical "Guided → +CHA on saves" future entry, a "Heroic → +1
/// on saves" future entry, etc.) lands as a one-line entry here rather
/// than another `if self.has_condition(...) { bonus += N; }` branch in
/// `condition_save_bonus`.
///
/// Sibling to `CONDITION_AC_BONUSES` on the "single-condition source +
/// signed magnitude" cohort pattern — the two tables split by affected
/// axis: save delta here vs. AC delta there. Same filter-map-sum walk
/// shape, same signed-delta convention, same "one row per source
/// condition" ordering discipline.
///
/// Entries (order doesn't affect the summed total; grouped by
/// buff-vs-debuff flavor for readability):
///   - **Bardic Inspiration** (`Inspired`, +3 saves): the bard's
///     signature Inspired die (RAW scales d6→d8→d10→d12 by bard
///     level); collapses to the d6-average (+3) matching the twin
///     `Inspired → +3 attacks` row on the sibling
///     `condition_attack_bonus` path. Consumed by
///     `clear_attack_advantage_riders` so the bonus doesn't double-
///     fire across multiple checks.
///   - **Warding Bond** (`WardingBonded`, +1 saves): the Cleric /
///     Paladin lv2 abjuration; installs the `WardingBonded` condition
///     on the bonded ally. RAW grants +1 AC AND +1 saves AND resistance
///     to all damage AND mirror damage — the +1 save clause lives
///     here, the +1 AC clause on the sibling `CONDITION_AC_BONUSES`
///     cohort's WardingBonded row, the resistance and mirror-damage
///     clauses on their own lanes.
///
/// A new condition-driven save bump (a future Sanctuary +N save on
/// the caster, a hypothetical Bard Song of Rest save aura, etc.) lands
/// as a fresh one-line row here rather than another inline if-branch
/// in `condition_save_bonus`.
const CONDITION_SAVE_BONUSES: &[ConditionRollBonus] = &[
    ConditionRollBonus {
        source: Condition::Inspired,
        bonus: 3,
    },
    ConditionRollBonus {
        source: Condition::WardingBonded,
        bonus: 1,
    },
    // 5e College of Eloquence Bard **Unsettling Words** (`Unsettled`,
    // −4 saves): the first row on this cohort to carry a negative
    // magnitude, and the reason the field was declared signed. RAW
    // subtracts a Bardic Inspiration die from the target's next save;
    // −4 is the d8 average floored, the same collapse Precision
    // Attack's +4 already makes on the sibling attack cohort. Spent by
    // `CONSUMED_ON_SAVE` on the first save the holder rolls, so the
    // penalty is one save deep rather than a standing debuff for the
    // life of the timer.
    ConditionRollBonus {
        source: Condition::Unsettled,
        bonus: -4,
    },
];

/// Flat ability-check bonuses contributed by active conditions — the
/// check-lane sibling of `CONDITION_SAVE_BONUSES`, read by
/// `condition_check_bonus` and spent by the engine's `CONSUMED_ON_CHECK`
/// cohort.
///
/// One row, and it is the whole reason two 5e effects were inert. RAW
/// spends a Bardic Inspiration die on "one ability check, attack roll,
/// or saving throw" — the attack and save lanes each had a cohort for
/// it and the check lane had none, so a die was worth two-thirds of
/// what the rules say. Guidance fared worse: it installs the same
/// `Inspired` condition and its RAW effect is *only* on ability checks,
/// so before this cohort existed a cast of it could not change a single
/// number in the game.
const CONDITION_CHECK_BONUSES: &[ConditionRollBonus] = &[
    ConditionRollBonus {
        source: Condition::Inspired,
        bonus: 3,
    },
];

/// One row in the `CONDITION_ATTACK_BONUSES` cohort — a single condition
/// whose presence contributes a flat attack-roll delta to the holder.
/// Sibling to `ConditionRollBonus { source, bonus }` and
/// `ConditionAcBonus { source, bonus }` on the "single-condition source +
/// signed magnitude" cohort lane — same declarative row shape, different
/// affected axis (attack roll here vs. save roll / AC there) and same
/// unit (integer to-hit points, same scale as save / AC integers). The
/// delta is signed for the same reason `ConditionRollBonus.bonus` and
/// `ConditionAcBonus.bonus` are signed: a hypothetical to-hit-debuff
/// row (a future "Blessed-by-the-Enemy → -1 attacks" entry) would sit
/// on the same table as the +3 / +4 / +10 buff rows without a separate
/// cohort.
///
/// Bless / Bane deliberately don't ride this cohort — their d4 die
/// lives on `bless_bane_attack_die` and their +N / -N flat lives on
/// `attack_bonus_buff`, so including them here would double-count on
/// the attack pipeline. Sacred (Sacred Weapon: +CHA modifier) also
/// stays inline in `condition_attack_bonus` rather than riding this
/// cohort — its magnitude is variable (per-actor CHA), while every row
/// on this cohort carries a compile-time flat integer under the
/// declarative-table pattern shared with `CONDITION_SAVE_BONUSES` and
/// `CONDITION_AC_BONUSES`. A future closure-based sibling cohort could
/// absorb variable-magnitude to-hit riders (a hypothetical Guided +CHA
/// on attacks, etc.) if a second entry appears — a single Sacred outlier
/// doesn't justify widening the row shape today.
struct ConditionAttackBonus {
    /// Source condition whose presence gates the row. Every attack-bump
    /// buff on this cohort currently uses a plain single-condition
    /// gate, matching the `ConditionRollBonus.source` /
    /// `ConditionAcBonus.source` shape. A future compound-gate to-hit
    /// buff (a hypothetical "+2 attacks while Raging and Reckless") would
    /// either widen this row to a `flag` closure or land as a sibling
    /// cohort with the compound predicate — the shape is deliberately
    /// narrower here than a closure-based cohort to keep the common
    /// single-condition case a bare row.
    source: Condition,
    /// Signed to-hit delta contributed while `source` is held. Positive
    /// for the current buff rows (Inspired +3 d6-average, PrecisionAttacking
    /// +4 d8-average, GuidedStriking +10 flat); a future negative entry
    /// for an attack-debuff would sit here as a signed delta. Summed by
    /// `condition_attack_bonus` so vertical stacking (Inspired +
    /// PrecisionAttacking → +7 attacks) folds through the same
    /// filter-map-sum walk as horizontal stacking on the sibling
    /// `CONDITION_SAVE_BONUSES` / `CONDITION_AC_BONUSES` cohorts.
    bonus: i32,
}

/// Condition-driven attack-bonus cohort read by
/// `ActorInstance::condition_attack_bonus`. Every row's `bonus` is
/// summed (signed) when the row's `source` condition is held; the
/// resulting delta is added to the attacker's to-hit roll via the shared
/// attack-composition path. Adding a fresh condition-driven attack bump
/// (a hypothetical Sanctuary-broken +N attack rider, a future Mark of
/// the Hunter +N attack aura, etc.) lands as a one-line entry here
/// rather than another `if self.has_condition(...) { bonus += N; }`
/// branch in `condition_attack_bonus`.
///
/// Sibling to `CONDITION_SAVE_BONUSES` (save-roll delta) and
/// `CONDITION_AC_BONUSES` (AC delta) on the "single-condition source +
/// signed magnitude" cohort pattern — the three tables split by affected
/// axis: attack roll here vs. save roll vs. AC. Same filter-map-sum
/// walk shape, same signed-delta convention, same "one row per source
/// condition" ordering discipline.
///
/// Entries (order doesn't affect the summed total; grouped by
/// buff-vs-debuff flavor for readability):
///   - **Bardic Inspiration** (`Inspired`, +3 attacks): the bard's
///     signature Inspired die (RAW scales d6→d8→d10→d12 by bard level);
///     collapses to the d6-average (+3) matching the twin `Inspired
///     → +3 saves` row on the sibling `CONDITION_SAVE_BONUSES` cohort.
///     Consumed by `clear_attack_advantage_riders` so the bonus doesn't
///     double-fire across multiple swings.
///   - **Precision Attack** (`PrecisionAttacking`, +4 attacks): the
///     Battle Master Fighter maneuver — RAW +1d8 (d8-average, rounded
///     down to +4) on the primed attack roll. Symmetric with Inspired
///     on the "one-shot self-prime" lane; consumed by
///     `clear_attack_advantage_riders` via `CONSUMED_ON_ATTACK` so the
///     bonus only fires on the first swing after the prime installs.
///   - **Guided Strike** (`GuidedStriking`, +10 attacks): the War Domain
///     Cleric Channel Divinity — RAW flat +10 to the next attack roll
///     (the largest single-swing accuracy buff in the game). Sibling
///     to PrecisionAttacking on the "one-shot self-prime" lane; consumed
///     by `clear_attack_advantage_riders` via `CONSUMED_ON_ATTACK` so
///     the bonus only fires on the first swing after the prime installs.
///
/// Sacred Weapon's `Sacred` condition (Paladin Oath of Devotion Channel
/// Divinity: +CHA modifier on attacks) deliberately doesn't ride this
/// cohort — its magnitude reads the holder's CHA mod at attack time,
/// while every row here carries a compile-time flat integer. The
/// variable-magnitude case lives in `condition_attack_bonus` as an
/// inline outlier, matching how Bless / Bane are intentionally excluded
/// from the cohort due to their double-count risk with `attack_bonus_buff`
/// / `bless_bane_attack_die` — the exclusion pattern here is "shape
/// mismatch" rather than "lane mismatch". A future closure-based
/// sibling cohort (or a widened `bonus: BonusValue` enum row shape)
/// could absorb variable-magnitude to-hit riders if a second entry
/// appears — a single Sacred outlier doesn't justify widening the row
/// shape today.
const CONDITION_ATTACK_BONUSES: &[ConditionAttackBonus] = &[
    ConditionAttackBonus {
        source: Condition::Inspired,
        bonus: 3,
    },
    ConditionAttackBonus {
        source: Condition::PrecisionAttacking,
        bonus: 4,
    },
    ConditionAttackBonus {
        source: Condition::GuidedStriking,
        bonus: 10,
    },
];

/// One row in the `ABILITY_MOD_INITIATIVE_BONUSES` cohort — a single
/// passive-feature flag that adds the holder's modifier for a specific
/// ability to the initiative-roll total. Held as a `(flag_fn, ability)`
/// pair so a new row can point to any predicate on the actor
/// (`|a| a.has_some_flag`) and target any of the six abilities without
/// widening a central enum. Sibling to `FlagDrivenSaveProficiency` on
/// the "passive-feature flag → single-ability engine surface" pattern —
/// same closure shape, different roll axis (initiative-total scalar
/// here vs. save-throw proficiency-bonus contribution there).
struct AbilityModInitiativeBonus {
    flag: fn(&ActorInstance) -> bool,
    ability: AbilityScoreType,
}

/// Flag-driven ability-mod initiative-bump cohort read by
/// `ActorInstance::initiative_flat_bonus`. Every row is summed with the
/// sibling `PROFICIENCY_INITIATIVE_BONUSES` cohort (Remarkable Athlete's
/// half-prof-rounded-up, Aura of the Sentinel's full prof); any row
/// whose flag fires here adds the target ability's modifier to the
/// initiative total. Adding a future ability-mod initiative bump (a
/// hypothetical Alert / Chef / Watcher class feature that keys off a
/// distinct ability) lands as a one-line entry here rather than another
/// `if self.has_XXX { bonus += self.ability_modifier(...); }` branch in
/// `initiative_flat_bonus`.
///
/// Sibling to `FLAG_DRIVEN_SAVE_PROFICIENCIES` on the "one flag,
/// one ability" declarative cohort pattern — same shape, different
/// engine surface (initiative-total scalar vs. save proficiency-bonus
/// contribution).
///
/// Entries (in order):
///   - **Rakish Audacity** (Swashbuckler Rogue, XGtE lv3): +CHA-mod.
///   - **Dread Ambusher** (Gloom Stalker Ranger, XGtE lv3): +WIS-mod.
///   - **Tactical Wit** (War Magic Wizard, XGtE lv2): +INT-mod.
///     Subclass-tag lookup on `WAR_MAGIC_WIZARD_TEMPLATE` via
///     `has_passive_feature(TACTICAL_WIT_TAG)` — same declarative row
///     shape as the struct-field-flag siblings above, but the closure
///     reads a passive-feature tag rather than a dedicated
///     `has_tactical_wit` field, matching the "tag-only cross-class
///     helper" pattern (`with_subclass_tag`) the wizard chassis already
///     uses for `NECROMANCY_WIZARD_TEMPLATE`.
const ABILITY_MOD_INITIATIVE_BONUSES: &[AbilityModInitiativeBonus] = &[
    AbilityModInitiativeBonus {
        flag: |a| a.has_rakish_audacity,
        ability: AbilityScoreType::Charisma,
    },
    AbilityModInitiativeBonus {
        flag: |a| a.has_dread_ambusher,
        ability: AbilityScoreType::Wisdom,
    },
    AbilityModInitiativeBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::TACTICAL_WIT_TAG),
        ability: AbilityScoreType::Intelligence,
    },
];

/// Flag-driven initiative-advantage cohort read by
/// `ActorInstance::rolls_initiative_with_advantage`. Every row is a
/// predicate on the actor; any row that fires flips the initiative
/// d20-roll shape from flat to advantage (two d20s, higher kept).
/// Adding a future initiative-advantage source (Alert feat's pre-2024
/// variant, a hypothetical Guardian Armor set bonus, another class
/// subclass tell) lands here as a one-line entry rather than another
/// `|| new_flag` join in `rolls_initiative_with_advantage`.
///
/// Sibling to `ABILITY_MOD_INITIATIVE_BONUSES` on the "passive
/// initiative augment" lane — that cohort stacks a flat number on the
/// result, this one drops the advantage die. Both cohorts fire on the
/// same initiative-roll chokepoint (`roll_initiative`), so a carrier of
/// entries on both lanes rolls with advantage AND picks up whichever
/// flat bumps apply.
///
/// Entries (in order):
///   - **Feral Instinct** (Barbarian lv7): struct-field flag on the
///     baseline `BARBARIAN_TEMPLATE` chassis (and every barbarian
///     subclass that inherits it — Totem / Storm Herald / Berserker /
///     Zealot).
///   - **Vigilant Blessing** (Twilight Cleric lv1, TCE): subclass-tag
///     lookup on `TWILIGHT_CLERIC_TEMPLATE` — the same collapse RAW's
///     "advantage on the next initiative" one-shot ribbon into a
///     passive template flag that every other subclass template on
///     this cohort uses (Feral Instinct is RAW-strictly always-on).
const INITIATIVE_ADVANTAGE_SOURCES: &[fn(&ActorInstance) -> bool] = &[
    |a| a.has_feral_instinct,
    |a| a.has_passive_feature(crate::actions::class_features::VIGILANT_BLESSING_TAG),
];

/// Proficiency-bonus fraction applied to the initiative-roll total by a
/// row in the `PROFICIENCY_INITIATIVE_BONUSES` cohort. Held as a small
/// enum rather than a `(numerator, denominator)` pair (or a raw closure)
/// so the two RAW callouts read at the row site: **Full** is the
/// prof-bonus-as-is Watchers Paladin lane; **HalfRoundUp** is the
/// `ceil(prof / 2)` Champion Fighter lane.
///
/// A future half-rounded-DOWN prof lane, third-prof lane, or any other
/// fixed RAW fraction lands as a fresh variant here rather than a
/// bespoke inline formula on the cohort row.
#[derive(Clone, Copy)]
enum PbFraction {
    /// Full proficiency bonus. Watchers Paladin Aura of the Sentinel
    /// lane — RAW: "a bonus to initiative equal to your proficiency
    /// bonus".
    Full,
    /// Half proficiency bonus rounded up. Champion Fighter Remarkable
    /// Athlete lane — RAW: "add half your proficiency bonus (rounded
    /// up) to any Strength, Dexterity, or Constitution check". The
    /// `(prof + 1) / 2` integer formula matches the PHB table:
    /// prof 2→+1, prof 3→+2, prof 4→+2, prof 5→+3, prof 6→+3.
    HalfRoundUp,
}

impl PbFraction {
    fn apply(self, prof_bonus: i32) -> i32 {
        match self {
            PbFraction::Full => prof_bonus,
            PbFraction::HalfRoundUp => (prof_bonus + 1) / 2,
        }
    }
}

/// One row in the `PROFICIENCY_INITIATIVE_BONUSES` cohort — a single
/// passive-feature flag that adds a fraction of the holder's proficiency
/// bonus to the initiative-roll total. Held as a `(flag_fn, fraction)`
/// pair so a row can point to any predicate on the actor (struct-field
/// flag for Remarkable Athlete, subclass-tag lookup for Aura of the
/// Sentinel) and pick between full / half prof without widening the row
/// shape.
///
/// Sibling to `AbilityModInitiativeBonus` on the "flag → single
/// initiative bump" lane — same closure shape, different scalar source
/// (proficiency-bonus fraction here vs. ability modifier there). The
/// split by scalar source keeps each cohort's row shape tight — mixing
/// prof-bonus rows into the ability-mod cohort would need a discriminant
/// enum on the scalar side that half the rows would ignore.
struct ProficiencyInitiativeBonus {
    flag: fn(&ActorInstance) -> bool,
    fraction: PbFraction,
}

/// Flag-driven proficiency-bonus initiative-bump cohort read by
/// `ActorInstance::initiative_flat_bonus`. Every row is summed with the
/// sibling `ABILITY_MOD_INITIATIVE_BONUSES` cohort on the same accessor;
/// any row whose flag fires adds `fraction.apply(prof_bonus)` to the
/// initiative total. Adding a future prof-bonus-based initiative bump
/// (a hypothetical Alert-style feat, a new subclass with a third-prof
/// or half-prof-rounded-down bump, etc.) lands as a one-line entry
/// here rather than another `if self.has_XXX { bonus += prof / N; }`
/// branch in `initiative_flat_bonus`.
///
/// Sibling to `ABILITY_MOD_INITIATIVE_BONUSES` on the "one flag, one
/// initiative bump" declarative cohort pattern — same shape, different
/// scalar source (proficiency-bonus fraction here vs. ability modifier
/// there). Both cohorts fire on the same initiative-roll chokepoint;
/// carriers of rows on both lanes stack additively.
///
/// Entries (in order):
///   - **Remarkable Athlete** (Champion Fighter, RAW lv7): half prof
///     bonus rounded up. Struct-field-flag closure via
///     `has_remarkable_athlete` — the CHAMPION_TEMPLATE's dedicated
///     boolean flag rather than a subclass tag. RAW clause is "STR /
///     DEX / CON check that doesn't already use proficiency"; the
///     initiative roll (a DEX check) is the only combat surface that
///     hits the check-not-prof gate, so the cohort row collapses to
///     the initiative-total lane cleanly.
///   - **Aura of the Sentinel** (Watchers Paladin, RAW lv7, TCE): full
///     prof bonus. Subclass-tag closure via
///     `has_passive_feature(AURA_OF_THE_SENTINEL_TAG)` — the same
///     "one feature tag drives one initiative-cohort row" declarative
///     shape the sibling `TACTICAL_WIT_TAG` row on the ability-mod
///     cohort uses.
const PROFICIENCY_INITIATIVE_BONUSES: &[ProficiencyInitiativeBonus] = &[
    ProficiencyInitiativeBonus {
        flag: |a| a.has_remarkable_athlete,
        fraction: PbFraction::HalfRoundUp,
    },
    ProficiencyInitiativeBonus {
        flag: |a| a.has_passive_feature(crate::actions::class_features::AURA_OF_THE_SENTINEL_TAG),
        fraction: PbFraction::Full,
    },
];

/// Lifecycle state of an actor's hit points. Replaces the previous
/// `dying: bool` + `stable: bool` pair so the four meaningful states are
/// type-checked, and the death-save counters are scoped to the only
/// variant that uses them. `Dead` exists transiently between failure-3
/// and removal from `EncounterInstance.actors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpState {
    Active,
    Dying { successes: u32, failures: u32 },
    Stable,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathSaveOutcome {
    NotDying,
    Continuing,
    Stabilized,
    Dead,
    Revived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    Reduced,
    Downed,
    Killed,
    DyingFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealOutcome {
    Healed,
    Revived,
    AlreadyFull,
    NoOp,
}

/// State of an actor that's concentrating on a spell. Tracks what they
/// applied so dropping concentration can clean up automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationData {
    pub spell_name: String,
    /// Conditions this concentration applied. On drop, each is removed
    /// from its target. `(target_id, condition)`.
    pub conditions: Vec<(usize, Condition)>,
    /// Attack-roll buff deltas to roll back on drop.
    pub attack_buffs: Vec<(usize, i32)>,
    pub save_buffs: Vec<(usize, i32)>,
    /// Damage-roll buff deltas to roll back on drop. Mirrors `attack_buffs`
    /// for the damage lane (Magic Weapon's `+1` damage, Elemental Weapon's
    /// `+1/+2/+3` flame, etc.).
    pub damage_buffs: Vec<(usize, i32)>,
    /// 5e: making an attack ends Invisibility but not Greater Invisibility.
    /// Set true for concentration data whose effect ends when the caster
    /// makes any attack roll (clear_attack_advantage_riders consumes it).
    pub breaks_on_attack: bool,
}

/// 5e Help grant — a snapshot of "actor X has helped actor Y get
/// advantage against enemy Z." Stored on the recipient actor; consumed
/// by their next attack against `against`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpGrant {
    pub helper_id: usize,
    pub against: usize,
}

impl ConcentrationData {
    /// Bare concentration mark with no associated conditions to prune on
    /// drop — used by spells whose entire effect is the concentration
    /// marker itself (Crusader's Mantle, Crown of Stars, Mordenkainen's
    /// Sword: a passive aura / persistent presence on the caster, no
    /// per-target tag to remove).
    pub fn new(spell_name: impl Into<String>) -> Self {
        Self::with_conditions(spell_name, Vec::new())
    }

    pub fn with_conditions(
        spell_name: impl Into<String>,
        conditions: Vec<(usize, Condition)>,
    ) -> Self {
        Self {
            spell_name: spell_name.into(),
            conditions,
            attack_buffs: Vec::new(),
            save_buffs: Vec::new(),
            damage_buffs: Vec::new(),
            breaks_on_attack: false,
        }
    }

    /// Mark this concentration as ending when the caster makes any attack
    /// roll. Used by Invisibility (vanilla) but not Greater Invisibility.
    pub fn breaking_on_attack(mut self) -> Self {
        self.breaks_on_attack = true;
        self
    }

    /// Chainable builder setter for `attack_buffs`. Replaces the field in
    /// place; pair with `with_conditions(...)` / `new(...)` so a single
    /// fluent expression builds the full payload. Bless is the canonical
    /// triple-lane case: conditions + attack buffs + save buffs.
    pub fn with_attack_buffs(mut self, attack_buffs: Vec<(usize, i32)>) -> Self {
        self.attack_buffs = attack_buffs;
        self
    }

    /// Chainable builder setter for `save_buffs`. Mirrors
    /// `with_attack_buffs` on the save-roll lane (Bless, Enhance Ability).
    pub fn with_save_buffs(mut self, save_buffs: Vec<(usize, i32)>) -> Self {
        self.save_buffs = save_buffs;
        self
    }

    /// Chainable builder setter for `damage_buffs`. Mirrors
    /// `with_attack_buffs` on the damage-roll lane (Magic Weapon /
    /// Elemental Weapon-style installs).
    pub fn with_damage_buffs(mut self, damage_buffs: Vec<(usize, i32)>) -> Self {
        self.damage_buffs = damage_buffs;
        self
    }
}

#[derive(Clone)]
pub struct CreatureTemplate {
    pub name: &'static str,
    pub glyph: char,
    pub ac: u32,
    pub hitpoints: DiceExpr,
    pub speed: f32,
    pub strength: u32,
    pub intelligence: u32,
    pub dexterity: u32,
    pub wisdom: u32,
    pub constitution: u32,
    pub charisma: u32,
    pub skills: HashSet<Skill>,
    pub items: Vec<&'static Item>,
    pub senses: HashSet<SpecialSense>,
    pub languages: HashSet<Language>,
    pub cr: f32,
    pub size: Size,
    pub creature_type: CreatureType,
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
    pub spell_slots_by_level: Vec<u32>,
    pub rolls_death_saves: bool,
    /// Per-damage-type modifiers (resistance / immunity / vulnerability).
    /// Looked up by `damage_modifier` on the instance.
    ///
    /// **Unqualified.** Every row here applies to damage of its type
    /// from any source whatsoever. A balor's fire immunity is this: it
    /// does not care whether the fire came off a torch or off a Meteor
    /// Swarm. The rows that *do* care live in
    /// `nonmagical_damage_modifiers` next door.
    pub damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// Per-damage-type modifiers that apply **only to damage from a
    /// nonmagical attack** — 5e's single most common defensive clause,
    /// written on the stat block as "resistance to bludgeoning,
    /// piercing, and slashing damage from nonmagical attacks".
    ///
    /// A second map rather than a qualifier on the first because the
    /// two answer different questions and a creature can carry both:
    /// a mummy is unqualifiedly immune to poison, unqualifiedly
    /// vulnerable to fire, *and* resistant to mundane steel. Folding
    /// them into one table would need a per-row qualifier that 200
    /// templates would have to spell out on every row, almost all of
    /// them writing "unqualified".
    ///
    /// Read at the two attack chokepoints
    /// (`engine::attack::resolve_attack_outcome_with_rider` and
    /// `spells::spell_attack_outcome`) rather than in
    /// `DealDamage::apply`, because RAW's qualifier is *"from a
    /// nonmagical **attack**"* — not "from a nonmagical source". Damage
    /// that never came from an attack roll (a failed Dexterity save
    /// against a collapsing ceiling, a spike growth's needles) is not
    /// damage from an attack at all, and so is never reduced by one of
    /// these rows. Applying them at the damage sink instead would have
    /// been the easier plumbing and the wrong rule.
    ///
    /// Set by `CreatureTemplate::resistant_to_nonmagical_physical`,
    /// which is the only thing in the engine that writes the B/P/S
    /// triplet — see there.
    pub nonmagical_damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// Saving throws this creature is proficient with. Optional;
    /// templates that don't care can leave this empty (default new).
    pub proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions this creature is immune to (e.g. zombies vs Charm,
    /// elementals vs Poisoned).
    pub condition_immunities: HashSet<Condition>,
    /// 5e **Charge** / **Pounce** / **Trampling Charge**, or `None` for
    /// the overwhelming majority of creatures that just walk up and
    /// swing.
    ///
    /// A creature-level trait rather than a weapon-level one because
    /// that is how the rules phrase it, and because weapon names are not
    /// unique across the bestiary — see `ChargeRider`.
    pub charge: Option<ChargeRider>,
    /// Whether this creature's anatomy is one a rider can sit on — 5e's
    /// "a willing creature that is at least one size larger than you and
    /// that has an appropriate anatomy may serve as a mount" (PHB
    /// p.198), minus the size clause, which
    /// `EncounterInstance::can_mount` derives from the two creatures at
    /// the moment of mounting.
    ///
    /// A declared flag rather than a derived one because *anatomy* is
    /// the half of that sentence nothing on the sheet answers. Size and
    /// willingness are both already on the board — size is a field, and
    /// willingness is team membership — but "has a back, and is broken
    /// to the saddle" is not recoverable from `creature_type` plus
    /// `size`. Beast-and-Large would take the giant shark, the gorilla
    /// and the swarm of bats along with the warhorse; Beast alone misses
    /// the pegasus (Celestial), the nightmare (Fiend) and the
    /// hippogriff (Monstrosity), which are the three most famous mounts
    /// in the game. There is no predicate here, only a list, so the
    /// engine keeps the list.
    ///
    /// `false` for the overwhelming majority of the bestiary, including
    /// every player class — a fighter is not something you ride.
    pub mountable: bool,
    /// Class-feature tags available to this creature (Second Wind,
    /// Action Surge, etc.). Empty for ordinary monsters.
    pub features: HashSet<&'static str>,
    /// HP to regenerate at end-of-round while combat-active. 0 (the
    /// default for ordinary monsters) disables the heal. Trolls set this
    /// to 3; future regenerators (e.g. vampires) plug in here.
    pub regen_per_round: u32,
    /// Damage types that suppress this creature's regeneration for one
    /// round (5e troll: fire / acid). When damage of one of these types
    /// lands, `regen_suppressed` flips on the instance; `round_end`
    /// clears it after skipping that round's heal.
    pub regen_suppressors: HashSet<DamageType>,
    /// 5e Legendary Resistance — number of times per long rest the creature
    /// can choose to succeed on a save it just failed. Read by
    /// `EncounterInstance::roll_save`: when a failed save would land and
    /// the actor's `legendary_resistance_remaining` counter is non-zero,
    /// the save is promoted to a pass and the counter is decremented.
    /// Long rest restores to this template max. 0 = no legendary
    /// resistance (the default for ordinary creatures).
    pub legendary_resistances: u32,
    /// 5e Abjuration Wizard **Arcane Ward** (subclass level 2), expressed
    /// as the RAW "twice your wizard level" term of the ward's maximum.
    /// 0 (the default, and every non-abjurer) disables the feature. The
    /// holder's Intelligence modifier is added on top when the ward
    /// forms, so a template shipping `arcane_ward_base: 18` on a build
    /// with INT 20 weaves a 23-point ward.
    ///
    /// Carried as a template constant rather than derived from the
    /// actor's `level` because `level` tracks in-run XP progression from
    /// 1, not the build level a class template targets — same reasoning
    /// that ships `crit_threshold: 19` flat on the Champion rather than
    /// gating it behind a level check.
    pub arcane_ward_base: u32,
    /// 5e Divination Wizard **Portent** (subclass level 2) — how many
    /// foretold d20 faces the holder banks per long rest. 0 (the
    /// default, and every non-diviner) disables the feature. RAW ships
    /// 2 at subclass level 2 and 3 at level 14 (**Greater Portent**),
    /// so the level-14 upgrade is expressible as a one-field bump on
    /// this term rather than a second flag.
    ///
    /// Carried as a template constant rather than derived from the
    /// actor's `level` for the same reason `arcane_ward_base` is:
    /// `level` tracks in-run XP progression from 1, not the build level
    /// a class template targets.
    pub portent_dice: u32,
    /// 5e Transmutation Wizard **Transmuter's Stone** (subclass level
    /// 6) — which of the stone's benefits the holder currently carries.
    /// `None` (the default, and every non-transmuter) means no stone.
    ///
    /// A template axis rather than a runtime choice, for the same
    /// reason `portent_dice` is a scalar and the three Storm Herald
    /// Barbarians are three templates: RAW's pick-one-of-four is a
    /// build decision, and the engine's action layer has no channel
    /// for "cast this, but with option C". A Swiftness or Warding
    /// transmuter is this template with one field changed, exactly the
    /// way `portent_dice: 2` gives the subclass-level-2 Diviner.
    ///
    /// RAW's re-attunement clause ("you can change the effect when you
    /// cast a transmutation spell of 1st level or higher") is what this
    /// collapses. Modelling it would need either a free action that
    /// cycles blindly through the options or a prompt channel the AI
    /// can't answer, and both are worse models of the feature than a
    /// stone the wizard has already settled on.
    pub transmuters_stone: Option<TransmutersStoneBenefit>,
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves for half damage, take 0
    /// on a pass and half on a fail instead of half / full.
    pub has_evasion: bool,
    /// 5e **Mounted Combatant** feat (PHB p.168) — the feat that turns
    /// `engine::mounts` from a way to travel into a way to fight.
    ///
    /// All three of RAW's clauses ship, and each lands on a lane that
    /// already existed, which is the argument for one flag rather than
    /// three:
    ///
    ///   - "You have advantage on melee attack rolls against an
    ///     unmounted creature that is smaller than your mount" —
    ///     `compute_attack_mode`, beside every other reason a swing
    ///     rolls twice.
    ///   - "You can force an attack that targets your mount to target
    ///     you instead" — the damage-redirect lane the Crown Paladin's
    ///     Divine Allegiance already owns, at
    ///     `EncounterInstance::claim_rider_interposition`.
    ///   - "If your mount is subjected to an effect that allows it to
    ///     make a Dexterity saving throw to take only half damage, it
    ///     instead takes no damage if it succeeds and only half damage
    ///     if it fails" — which is Evasion, granted to somebody else,
    ///     so it lands in `save_mitigation_for` next to the real one.
    ///
    /// `false` for everybody who has not taken the feat, which on this
    /// roster is everybody but the Cavalier and the Knight — the two
    /// builds whose stat block is written around a horse.
    pub has_mounted_combatant: bool,
    /// 5e Uncanny Dodge (Rogue 5): use reaction to halve damage from one
    /// attack you can see. Modeled as a passive flag checked in the
    /// attack resolution pipeline.
    pub has_uncanny_dodge: bool,
    /// 5e Monk Deflect Missiles (level 3): when hit by a ranged weapon
    /// attack, the monk can spend their reaction to reduce the damage by
    /// `1d10 + DEX modifier + monk level`. Modeled as a passive flag read
    /// in the attack resolution pipeline next to `has_uncanny_dodge`:
    /// fires only when the swing is ranged (gated on `is_melee == false`)
    /// and the monk has a reaction available. The damage reduction lands
    /// after Uncanny Dodge / damage modifiers so a fully-stacked
    /// rogue/monk multiclass still gets both layers cleanly. RAW also
    /// gates on the swing being a "weapon attack" — spell attacks don't
    /// qualify and the rider doesn't fire on them (we read the `is_melee`
    /// flag and the attack-rider chokepoint that the rest of the
    /// reaction-based features use).
    pub has_deflect_missiles: bool,
    /// 5e Fighter Battle Master **Parry** maneuver (feature-gated
    /// reaction): when hit by a melee attack, spend a reaction plus one
    /// superiority-die charge (`PARRY_TAG`) to reduce the damage by
    /// `1d8 + DEX modifier`. Same "auto-fire when available" shape as
    /// Uncanny Dodge / Deflect Missiles: the flag gates the reaction, the
    /// per-rest charge caps the number of uses. Read at
    /// `resolve_attack_outcome` alongside the sibling damage-reducers.
    pub has_parry: bool,
    /// 5e Fighter Battle Master **Riposte** maneuver (feature-gated
    /// reaction): when a creature misses you with a melee attack, spend
    /// a reaction plus one superiority-die charge (`RIPOSTE_TAG`) to make
    /// a melee weapon attack against the attacker. Read at
    /// `resolve_attack_outcome` after the miss branch — the target's
    /// first melee action fires against the attacker via the same
    /// side-effect chokepoint the opportunity-attack dispatcher uses.
    pub has_riposte: bool,
    /// 5e Displacer Beast trait: the creature projects a displaced image.
    /// Attacks against it have disadvantage. Breaks on damage; restores
    /// at the start of the creature's next turn.
    pub has_displacement: bool,
    /// 5e Barbarian Danger Sense (level 2): advantage on DEX saves against
    /// effects you can see while not blinded, deafened, or incapacitated.
    pub has_danger_sense: bool,
    /// 5e Pack Tactics (Wolf, Dire Wolf, Kobold): advantage on attack rolls
    /// when an ally is adjacent to the target. Read by `compute_attack_mode`.
    pub has_pack_tactics: bool,
    /// 5e **Sunlight Sensitivity** / **Sunlight Weakness** / **Sunlight
    /// Hypersensitivity** — the kobold-and-drow, shadow, and vampire
    /// tiers of "this creature does not belong outdoors". `None` for
    /// almost everything; see `SunlightFrailty` for what each tier
    /// costs and why they are one field rather than three flags.
    ///
    /// Inert unless the encounter's ambient light is
    /// `AmbientLight::Daylight`. A torchlit hall is bright and is not
    /// sunlight, which is exactly the distinction the ambient enum
    /// exists to carry.
    pub sunlight_frailty: Option<SunlightFrailty>,
    /// 5e **Swarm** (Swarm of Bats, Swarm of Rats, Swarm of Insects,
    /// Swarm of Poisonous Snakes, Swarm of Quippers): this "creature" is
    /// a cloud of Tiny ones sharing one HP pool and one initiative slot.
    ///
    /// Most of the statblock's swarm identity is already expressible
    /// with fields that exist — `damage_modifiers` carries the
    /// bludgeoning / piercing / slashing resistance (a sword swing
    /// scatters bats rather than cutting them), and
    /// `condition_immunities` carries the eight conditions a cloud can't
    /// be put in (you cannot knock a swarm prone). Those need no flag.
    ///
    /// This flag is for the two clauses that have nowhere else to live:
    ///
    ///   - **The swarm thins as it dies.** RAW writes it into the attack
    ///     line — "21 (6d4+6) piercing damage, or 10 (3d4+3) piercing
    ///     damage if the swarm has half of its hit points or fewer" —
    ///     because there are fewer mouths left to bite with. Read at
    ///     `attack::attacker_scoped_damage_reduction`, which is the lane
    ///     for damage that depends on who is swinging.
    ///   - **"The swarm can't regain hit points or gain temporary hit
    ///     points."** Dead bats don't come back, so the cleric's Cure
    ///     Wounds and the bard's inspiration-shaped temp HP both bounce.
    ///     Read at `heal` and `gain_temp_hp`, the two chokepoints every
    ///     source of either funnels through.
    ///
    /// Not modelled: "the swarm can occupy another creature's space".
    /// `actor_map` is one id per tile, and multi-occupancy is a change
    /// to the board rather than to a creature.
    pub is_swarm: bool,
    /// 5e Magic Resistance (Balor, Lich, Pit Fiend, etc.): advantage on
    /// saving throws against spells and other magical effects. Read by
    /// `compute_save_mode` — applies to every save the creature rolls
    /// (we don't yet distinguish spell vs non-spell save sources, so we
    /// conservatively grant advantage on all saves, matching the most
    /// common interpretation for combat engines).
    pub has_magic_resistance: bool,
    /// 5e Recharge ability: some creature abilities recharge on a d6 roll
    /// at the start of each turn (e.g. "Recharge 5-6" means the ability
    /// recharges if the d6 shows 5 or 6). Each entry is (action_name,
    /// min_roll) — the action becomes available again when the d6 >=
    /// min_roll. Empty for creatures without recharge abilities.
    pub recharge_abilities: Vec<(&'static str, u32)>,
    /// 5e Legendary Actions — number of legendary action points refreshed
    /// at the start of each of the creature's turns. Dragons get 3,
    /// liches 3, beholders 3, etc. 0 = no legendary actions (the default
    /// for ordinary creatures). The encounter loop grants this many
    /// LegendaryAction resource tokens at the start of the creature's
    /// turn and the AI spends them between other actors' turns.
    pub legendary_actions_per_round: u32,
    /// 5e **Lair Actions** — the effects the *place* takes, on its own
    /// initiative, while this creature is alive inside it. Empty for
    /// every creature that doesn't have a lair, which is almost all of
    /// them.
    ///
    /// A list of effects rather than a list of `Action`s, and that is
    /// the whole design. Everything else in the engine that does
    /// something to the board is an `Action`, because an actor chose it
    /// and chose what to aim it at — the trait is built around a caster,
    /// a cost, a targeting schema, and a validation pass over the
    /// arguments somebody supplied. A lair action has none of that.
    /// Nobody spends anything for it, nobody aims it, and it fires
    /// whether or not the creature whose lair it is could act; it is the
    /// cave that acts. So each entry is a name and a function, and it
    /// picks its own targets off the board.
    ///
    /// See `engine::lair_actions` for the entries and
    /// `EncounterInstance::dispatch_lair_actions` for when they fire.
    pub lair_actions: &'static [crate::engine::lair_actions::LairAction],
    /// 5e Extra Attack — when this creature takes the Attack action, it
    /// can make two attacks instead of one. True for Fighters, Paladins,
    /// Rangers, Barbarians, Monks (level 5+), and monsters with
    /// Multiattack. Unlike class features, this is permanent and never
    /// consumed.
    pub has_extra_attack: bool,
    /// 5e Brutal Critical (Barbarian level 9+): on a critical hit with a
    /// melee weapon, roll one additional damage die of the weapon's type.
    /// Scales to 2 extra at level 13 and 3 at level 17. Read at the crit
    /// damage site in `engine::attack` — 0 disables the rider entirely
    /// (the default for non-barbarians).
    pub brutal_critical_dice: u32,
    /// 5e Improved Critical (Champion Fighter level 3): critical hits
    /// trigger on a d20 result of 19 or 20 instead of just 20. Superior
    /// Critical (level 15) drops the threshold to 18. Stored as the
    /// minimum d20 face that crits — `20` (the default) matches RAW for
    /// every other build. Read at the attack-resolution site so weapon
    /// AND spell-attack swings honor the lower threshold.
    pub crit_threshold: u32,
    /// 5e Lucky trait (Halfling racial) / Lucky feat: when the holder
    /// rolls a natural 1 on an attack roll, ability check, or saving
    /// throw, they can reroll the die and must use the new roll. We
    /// model the attack-roll and save-roll halves at the d20 sites in
    /// `resolve_attack_outcome` / `roll_save`. Ability checks share the
    /// same roll path so they pick up the reroll automatically.
    pub has_lucky: bool,
    /// 5e Halfling Brave racial trait: advantage on saving throws against
    /// being Frightened. The engine doesn't tag saves by what condition
    /// they protect against, so we approximate by treating Brave as full
    /// immunity to Frightened — checked dynamically at
    /// `ActorInstance::add_condition` alongside the Heroism / MindBlank
    /// immunity gates (see `dynamic_immunity_to`). The over-tuning
    /// (advantage → immunity) is small in practice: every fear effect in
    /// the engine still has to roll the underlying save, and Brave only
    /// kicks in if that save fails AND the source resolves to
    /// Frightened. The Paladin's Aura of Courage covers the in-aura ally
    /// case at a different chokepoint (`ApplyCondition::apply`) because
    /// that gate needs encounter geometry to find the aura emitter.
    pub has_brave: bool,
    /// 5e Elf / Half-Elf / Drow Fey Ancestry racial trait: advantage on
    /// saving throws against being Charmed, and magic can't put the
    /// holder to sleep. Approximated as full immunity to both Charmed
    /// and Asleep at the `dynamic_immunity_to` chokepoint — same shape
    /// as the Halfling Brave gate. The "magic can't put you to sleep"
    /// clause RAW only blocks magical sleep (e.g. the Sleep spell);
    /// natural unconsciousness (HP 0) still applies, and the engine
    /// keeps `Unconscious` separate from `Asleep` so the half-elf still
    /// drops normally when their HP runs out. The Charmed-advantage
    /// over-tuning matches Brave's; the Sleep block is RAW since Asleep
    /// is only ever installed by magical sources in this engine.
    pub has_fey_ancestry: bool,
    /// 5e Paladin Aura of Protection (level 6+): the paladin and every
    /// ally within 10 ft (4 tile gap in this 2.5ft grid) adds the
    /// paladin's CHA modifier (minimum +1) to all saving throws.
    /// Stored as a flag here; the aura radius and bonus formula live
    /// in `EncounterInstance::aura_of_protection_bonus`, the chokepoint
    /// `roll_save` reads. Stacks additively if multiple paladins are in
    /// range — we keep the simple "best bonus wins" rule (the largest
    /// CHA mod of any aura-bearer in range) to avoid degenerate stacks
    /// where two CHA-20 paladins double-buff every save.
    pub has_aura_of_protection: bool,
    /// 5e Paladin Aura of Courage (level 10+): the paladin and every
    /// ally within 10 ft is immune to the Frightened condition. Companion
    /// to Aura of Protection; both auras share the 10ft radius. Engine
    /// reads via `EncounterInstance::is_in_aura_of_courage` which the
    /// `Frightened` apply path consults to suppress installs.
    pub has_aura_of_courage: bool,
    /// 5e Paladin **Oath of Devotion Aura of Devotion** (Devotion
    /// subclass level 7): the paladin and every ally within 10 ft is
    /// immune to the Charmed condition. Third sibling of the paladin
    /// aura family alongside Aura of Protection (saves bonus) and Aura
    /// of Courage (Frightened immunity) — same 10ft radius, same
    /// paladin-emitter model. Engine reads via
    /// `EncounterInstance::is_in_aura_of_devotion` which the `Charmed`
    /// apply path consults to suppress installs. RAW-gated to the
    /// Oath of Devotion subclass, so a plain paladin doesn't get it
    /// even at high levels.
    pub has_aura_of_devotion: bool,
    /// 5e Rogue **Elusive** (level 18 capstone): no attack roll has
    /// advantage against the holder while they aren't Incapacitated. The
    /// rogue's ultimate defensive tell — even Assassinate / Hidden /
    /// Pack-Tactics / Vow of Enmity swings against the elusive rogue
    /// resolve at Normal (or Disadvantage, if the attacker also has a
    /// disadvantage source). Post-processing clause in
    /// `compute_attack_mode`: after every attacker- and target-side
    /// source is combined, if the target has this flag and isn't
    /// Incapacitated / Stunned / Paralyzed / Unconscious, downgrade
    /// Advantage to Normal (Disadvantage passes through untouched).
    pub has_elusive: bool,
    /// 5e Paladin **Oath of the Ancients — Nature's Ward** (Ancients
    /// subclass level 15): passive immunity to being Charmed AND
    /// Frightened. RAW also grants immunity to disease and no aging;
    /// disease has no mechanical surface in the combat engine and
    /// aging is a non-combat concept, so both halves are RAW no-ops
    /// we don't wire up. Read by `dynamic_immunity_to` next to the
    /// Halfling Brave (Frightened) and Fey Ancestry (Charmed + Asleep)
    /// gates — the flag drives both installs off in a single line.
    ///
    /// Distinct from Aura of Courage / Aura of Devotion in three ways:
    ///   1. **Self-only** — the paladin doesn't project this to allies
    ///      (unlike the paladin auras that grant the same immunity in
    ///      a 10ft bubble); Nature's Ward is a personal capstone.
    ///   2. **Always-on** — no combat-active / incapacitated gate; a
    ///      down-and-dying Ancients paladin is still immune to both
    ///      installs (RAW: "immune to being charmed" is unconditional).
    ///   3. **Both conditions from one flag** — the paladin's other
    ///      auras cover a single condition each; Nature's Ward is the
    ///      only "two-condition immunity from one flag" template lane.
    pub has_natures_ward: bool,
    /// 5e Barbarian **Feral Instinct** (level 7 passive): advantage on
    /// initiative rolls. Read by `ActorInstance::roll_initiative` — the
    /// d20 is rolled twice and the higher result is kept. The classic
    /// "barbarian goes first" tell: the raging bruiser opens the round
    /// before the fireball lands. Read on top of the DEX modifier so a
    /// barbarian with average DEX still opens the round competitively
    /// against a rogue's DEX-primary initiative.
    pub has_feral_instinct: bool,
    /// 5e Champion Fighter **Remarkable Athlete** (level 7 subclass
    /// passive): add half of the holder's proficiency bonus (rounded up)
    /// to every STR, DEX, or CON check that doesn't already include the
    /// proficiency bonus. In our engine the only STR/DEX/CON check with
    /// combat surface is the initiative roll (DEX check RAW), so the
    /// grant collapses to "+ceil(prof / 2) on initiative rolls" — read
    /// at the shared `PROFICIENCY_INITIATIVE_BONUSES` cohort in
    /// `initiative_flat_bonus`, next to Watchers Paladin's Aura of the
    /// Sentinel (full prof). Distinct from `has_feral_instinct`
    /// (advantage on the initiative roll shape) — the two composers
    /// stack cleanly: a hypothetical Barbarian-multiclass Champion
    /// would roll with advantage AND pick up the flat bump on the
    /// higher of the two rolls. Ships on the CR-3 Champion template
    /// above its strict RAW level gate for the same reason Survivor
    /// (lv18) ships there — class templates target a balanced playable
    /// level, not lockstep PHB progression. Sibling to `has_feral_instinct`
    /// on the passive-initiative-augment lane.
    pub has_remarkable_athlete: bool,
    /// 5e Champion Fighter **Superior Critical** (subclass level 15):
    /// critical hits trigger on a d20 result of 18, 19, or 20 instead
    /// of the Improved Critical 19-20 window. Passive; overrides
    /// `crit_threshold` to `min(field_value, 18)` when set so a
    /// Champion who already has Improved Critical (threshold 19) drops
    /// cleanly to 18. Exposed as a separate flag so the two Champion
    /// features stay independently readable in template diffs and the
    /// mechanical intent ("this template layers Superior Critical on
    /// top of whatever the base crit threshold is") is explicit rather
    /// than a magic-number crit_threshold: 18 that reads as "why 18?"
    /// three years later.
    ///
    /// Read by `crit_threshold()` — the accessor caps the returned
    /// value at 18 when this flag is set so every attack-roll site
    /// (weapon + spell, `resolve_attack_outcome` /
    /// `spell_attack_outcome`) picks up the drop. Composes cleanly
    /// with Improved Critical (baseline Champion): setting `crit_threshold:
    /// 19` on the template AND flipping this flag lands on 18 (the
    /// lower of the two). Setting the flag on a template with the
    /// default `crit_threshold: 20` also lands on 18 — a hypothetical
    /// Barbarian who somehow picks up Superior Critical still
    /// benefits.
    pub has_superior_critical: bool,
    /// 5e Monk **Diamond Soul** (level 14 passive): proficiency in all
    /// saving throws. The monk's late-game defensive envelope: pairs
    /// with Evasion and Deflect Missiles to make the monk one of the
    /// hardest chassis to lock down. Read by
    /// `ActorInstance::is_save_proficient` — if the flag is set, every
    /// ability returns proficient regardless of the explicit
    /// `proficient_saves` set. Composes cleanly with the paladin's Aura
    /// of Protection — the CHA-bonus save layer stacks on top of the
    /// diamond-soul proficiency floor.
    pub has_diamond_soul: bool,
    /// 5e Rogue **Slippery Mind** (level 15 passive): proficiency in
    /// Wisdom saving throws. Read by `ActorInstance::is_save_proficient`
    /// next to `has_diamond_soul` — if the flag is set, WIS saves return
    /// proficient regardless of the explicit `proficient_saves` set. The
    /// classic anti-Hold Person / anti-Dominate Person defensive tell —
    /// the level-15 rogue can no longer be reliably WIS-locked by casters,
    /// mirroring the RAW envelope where Slippery Mind moves the rogue's
    /// bad save into their strong-save cluster. Distinct from Diamond Soul
    /// (all six saves) — Slippery Mind is a narrower, single-ability
    /// grant; the rogue's DEX save proficiency comes from the base class,
    /// and Evasion / Uncanny Dodge already cover the DEX-save damage lane.
    pub has_slippery_mind: bool,
    /// 5e Zealot Barbarian **Iron Mind** (level 7 subclass passive):
    /// proficiency in Wisdom saving throws. Mechanically identical to
    /// Slippery Mind — both flip the same is_save_proficient(WIS)
    /// gate — but exposed as a distinct template flag so the two
    /// class features don't get conflated (a hypothetical rogue /
    /// zealot-barbarian multiclass legally carries both by RAW, and
    /// the two would need to be independently swappable in feature-
    /// gating tests). Read alongside `has_slippery_mind` via the
    /// shared `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort — any hit is
    /// sufficient. Ships on `ZEALOT_BARBARIAN_TEMPLATE` above its
    /// strict RAW lv7 gate for the same reason Divine Fury (lv3) does
    /// on the same chassis — class templates target a balanced
    /// playable level, not lockstep PHB progression. Composes cleanly
    /// with the paladin's Aura of Protection (adjacent CHA-bonus on
    /// every save) and with Danger Sense (DEX-save advantage) — the
    /// zealot's WIS save is now a good save, so the pincer that used
    /// to lock a raging barbarian (Hold Person / Command / Dominate)
    /// no longer reliably lands.
    pub has_iron_mind: bool,
    /// 5e Half-Orc Savage Attacks: on a critical melee weapon hit, roll
    /// one additional weapon damage die. Mechanically identical to
    /// `brutal_critical_dice = 1` but exposed as a separate flag so the
    /// half-orc racial doesn't get conflated with the barbarian class
    /// feature in templates that combine both (e.g. a half-orc barbarian
    /// stacks the dice). Read at the same crit-damage site in
    /// `engine::attack` next to `brutal_critical_dice`.
    pub has_savage_attacks: bool,
    /// 5e Fighting Style: **Archery** (Fighter / Ranger / Paladin lv1
    /// pick): +2 to attack rolls made with ranged weapons. Read at
    /// `resolve_attack` — gated on `!is_melee && !is_spell` so a spell
    /// attack roll (Fire Bolt, Eldritch Blast) doesn't pick up the bonus
    /// per RAW ("ranged weapon attacks" specifically). Ships on the
    /// baseline RANGER_TEMPLATE + HUNTER_RANGER_TEMPLATE since both
    /// wield the longbow as their workhorse ranged weapon.
    pub has_archery_style: bool,
    /// 5e Fighting Style: **Defense** (Fighter / Paladin / Ranger lv1
    /// pick): +1 AC while wearing armor. Read at
    /// `ActorInstance::armor_class` — we don't model armor tiers so the
    /// gate collapses to "always on" for any holder of the flag, same
    /// shape as Fast Movement's "always on" collapse. Ships on the
    /// baseline FIGHTER + CHAMPION + PALADIN (and every paladin
    /// subclass) since the fighting-style pick is a lv1 class feature
    /// that composes cleanly with the smite / maneuver kits.
    pub has_defense_style: bool,
    /// 5e Fighting Style: **Dueling** (Fighter / Paladin / Ranger lv1
    /// pick): +2 to damage rolls on melee weapon attacks while wielding
    /// a one-handed weapon and no other weapon. Read at the damage-roll
    /// chokepoint in `engine::attack` — we don't track weapon-hand-usage
    /// so the gate collapses to "melee weapon attacks" (RAW pre-req
    /// on wielding one-handed is approximated). Ships on the baseline
    /// FIGHTER template (scimitar is one-handed slashing) so the class
    /// feature is engine-visible without stepping on the Champion /
    /// Paladin (both use two-handed weapons where Dueling doesn't apply
    /// RAW).
    pub has_dueling_style: bool,
    /// 5e Fighting Style: **Great Weapon Fighting** (Fighter / Paladin
    /// / Ranger lv1 pick): when the holder rolls a 1 or 2 on a damage
    /// die for a melee weapon attack made while wielding a two-handed
    /// or versatile-two-handed weapon, they may reroll the die once and
    /// must use the new roll (even if it comes up 1 or 2 again). Read
    /// at the damage-roll chokepoint in `engine::attack` — per-die
    /// reroll routed through `EncounterInstance::roll_weapon_damage_dice`
    /// so both the base swing and a crit's doubled dice pick up the
    /// reroll. We don't track weapon-hand-usage so the RAW "two-handed /
    /// versatile-two-handed" gate collapses to "melee weapon attack" —
    /// same shape as Dueling's gate collapse. Templates that ship
    /// two-handed workhorses (greatsword, greataxe, greatclub) carry
    /// this flag; the Fighter chassis (scimitar 1H) does NOT so
    /// Dueling and GWF stay mutually exclusive on the baseline lanes.
    pub has_great_weapon_fighting: bool,
    /// 5e Fighting Style: **Two-Weapon Fighting** (Fighter / Ranger lv1
    /// pick): "when you engage in two-weapon fighting, you can add your
    /// ability modifier to the damage of the second attack."
    ///
    /// Read by `actions::two_weapon::offhand_damage_ability`, and by
    /// nothing else. The style's entire content is that one clause, and
    /// the "second attack" it names is the bonus-action off-hand swing
    /// — so the flag turns `OffHandAttack`'s damage roll from dice-only
    /// into dice-plus-modifier and has no other effect anywhere.
    ///
    /// It used to be a blanket `+STR mod` on every melee swing the
    /// holder made, on the reasoning that the engine could not tell an
    /// off-hand swing from a main-hand one. It can now, and the
    /// approximation was generous in both directions that mattered: a
    /// holder with Extra Attack collected it two or three times a turn,
    /// and a holder who never dual-wielded collected it for free.
    ///
    /// Distinct from Dueling on more than the number: Dueling's flat +2
    /// really does ride every qualifying swing, so it stays a
    /// `MELEE_CASTER_BUMPS` entry. The two are also mutually exclusive
    /// in RAW — one hand or two — and no template ships both.
    pub has_two_weapon_fighting_style: bool,
    /// 5e Fighting Style: **Protection** (Fighter / Paladin lv1 pick):
    /// when a creature the holder can see attacks a target OTHER than
    /// the holder within 5 ft, the holder can use their reaction to
    /// impose disadvantage on the attack roll. RAW gates on "wielding a
    /// shield" — we don't model shield-wielding as a template-visible
    /// item slot, so the gate collapses to "flag holder, ally-adjacent,
    /// reaction available". Read at `compute_attack_mode` — the ally
    /// adjacency sweep is symmetric to Pack Tactics / Wolf Totem, but
    /// gated on the protector holding the flag and paying a reaction
    /// per swing. Not shipped on any current template by default; the
    /// flag exists so templates that carry a shield (a future Battle
    /// Master / Paladin sub-build) can opt in without inventing a new
    /// item slot.
    pub has_protection_style: bool,
    /// 5e Fighting Style: **Interception** (Fighter / Paladin lv1 pick,
    /// XGtE). When a creature the holder can see hits a target OTHER
    /// than the holder with a weapon or spell attack within 5 ft of the
    /// holder, they can use their reaction to reduce the damage the
    /// target takes by `1d10 + proficiency bonus` (to a minimum of 0).
    /// RAW gates on "wielding a shield or a simple / martial weapon" —
    /// we don't model shield / weapon-slot wielding as a template-
    /// visible flag, so the gate collapses to "flag holder, ally-
    /// adjacent, reaction available". Read at `resolve_attack_outcome`
    /// AFTER the damage is computed but BEFORE it's applied — the
    /// helper `apply_interception_reduction` returns the reduction,
    /// which the attack site clamps against the pending damage. Not
    /// shipped on any current template by default; the flag exists so
    /// templates that carry a shield can opt in.
    pub has_interception_style: bool,
    /// 5e Ranger **Feral Senses** (level 18 class capstone). Passive
    /// concealment-piercer: an unseen attacker doesn't gain advantage
    /// on attack rolls against the holder, and the holder doesn't
    /// suffer disadvantage on attack rolls against an unseen target.
    /// Read at `compute_attack_mode` next to `has_truesight` — the
    /// piercing lookup joins the truesight cohort so an Invisible /
    /// Blurred / Displaced attacker (or target) has their concealment
    /// tax suppressed against the flag holder. Distinct from Truesight
    /// (sense) which is available intrinsically or via the True Seeing
    /// spell — Feral Senses is a flat "ranger's animal instincts"
    /// template flag, unrangeable and unconditional. Ships on the
    /// baseline RANGER_TEMPLATE (inherited by HUNTER_RANGER_TEMPLATE)
    /// above its strict RAW level gate for the same reason Foe Slayer
    /// (lv20 capstone) does — class templates target a balanced
    /// playable level, not lockstep PHB progression.
    pub has_feral_senses: bool,
    /// 5e Rogue **Blindsense** (level 14 class feature). Passive
    /// concealment-piercer with a 10-ft (4-tile footprint Chebyshev)
    /// range gate: while able to hear, the rogue is aware of the
    /// location of any hidden or invisible creature within 10 ft. Read
    /// at `compute_attack_mode` next to `has_truesight` / `has_feral_senses`
    /// — same illusion-suppression cohort, but only fires when the
    /// subject is within the 10-ft envelope. RAW "while able to hear"
    /// clause collapses to "!Deafened" on the holder. Ships on the
    /// baseline ROGUE_TEMPLATE (inherited by ASSASSIN_ROGUE_TEMPLATE)
    /// above its strict RAW level gate for the same reason Elusive /
    /// Slippery Mind do. The 10-ft envelope keeps the flag from
    /// out-classing Feral Senses (unbounded) — the rogue leans in
    /// close to leverage it, the ranger benefits at any range.
    pub has_blindsense: bool,
    /// 5e Oathbreaker Paladin (DMG) level-7 subclass feature — **Aura
    /// of Hate**. Passive template flag: when the holder swings in
    /// melee, they add their Charisma modifier (minimum +1) to the
    /// weapon damage roll. Approximates RAW's "the paladin, as well as
    /// any fiends and undead within 10 feet" aura shape by collapsing
    /// to a self-only bonus at the caster-side melee bumps table in
    /// `engine::attack::resolve_attack_outcome` — the ally-side fiend /
    /// undead half of the aura is dropped since we don't tag those as
    /// an aura-eligible cohort at the template level.
    ///
    /// Dropping the ally half is an aura-shape decision rather than an
    /// omission: the other paladin auras (Protection, Courage,
    /// Devotion) read the EMITTER's flag from the ally's side, and a
    /// fiend-or-undead cohort would need a fresh footprint-Chebyshev
    /// pass at the attack site to match. Self-only keeps the read local
    /// and the bump-table plumbing uniform.
    ///
    /// Read in the shared `MELEE_CASTER_BUMPS` table alongside Rage
    /// (+2), Dueling (+2), and Two-Weapon Fighting (+STR mod). Ships
    /// on `OATHBREAKER_PALADIN_TEMPLATE` — no default-on template
    /// otherwise carries it. The minimum +1 folds in via `max(1)` on
    /// the CHA lookup, matching RAW.
    ///
    /// Distinct from Vow of Enmity (Vengeance paladin), which is a
    /// once-per-rest advantage prime rather than a passive damage
    /// bump; and from Improved Divine Smite, which is a die rather
    /// than a flat mod and rides as a separate rider payload. Aura of
    /// Hate folds into the base weapon damage roll, so a resistance on
    /// the weapon's damage type halves the bonus too — the same way
    /// Rage, Dueling and Two-Weapon Fighting behave.
    pub has_aura_of_hate: bool,
    /// 5e Conquest Paladin (XGtE) level-7 subclass feature — **Aura of
    /// Conquest**. Passive template flag with two clauses, both scoped
    /// to Frightened enemies whose footprint sits within 10 ft of the
    /// paladin: their speed drops to 0, and they take psychic damage
    /// equal to half the paladin's level at the start of each of their
    /// turns.
    ///
    /// The only *hostile* aura on the paladin chassis. Protection,
    /// Courage, Devotion, Warding and Alacrity all project onto allies
    /// and are read through `paladin_aura_emitters`, which filters to
    /// the subject's own team; Conquest projects onto enemies, so it
    /// reads through the enemy-scoped
    /// `EncounterInstance::in_hostile_aura_of_conquest` instead.
    ///
    /// Both clauses land at the victim's turn start
    /// (`apply_aura_of_conquest`), which is where RAW puts the damage
    /// and where zeroing the movement budget is equivalent to RAW's
    /// speed-0 clause: the engine hands out the turn's movement in
    /// `reset_for_new_round`, so draining it there is the same thing as
    /// never having had it.
    pub has_aura_of_conquest: bool,
    /// 5e Conquest Paladin (XGtE) level-15 subclass feature —
    /// **Scornful Rebuke**. Passive template flag: any creature that
    /// hits the paladin with an attack takes psychic damage equal to
    /// the paladin's Charisma modifier (minimum 1), unless the paladin
    /// is incapacitated.
    ///
    /// Read as a row on `engine::attack::ANY_ATTACK_REFLECT_FEATURES`,
    /// the reflect lane that fires on *any* connecting attack. The two
    /// older reflect lanes are both melee-gated, which is exactly the
    /// distinction: an archer who shoots a Conquest Paladin from across
    /// the room still takes the rebuke.
    pub has_scornful_rebuke: bool,
    /// 5e Ancients Paladin (PHB) level-7 subclass feature — **Aura of
    /// Warding**. Passive template flag: the paladin and any friendly
    /// creatures whose footprint sits within 10 ft (4 tiles) of the
    /// paladin have resistance to damage from spells. RAW keys off
    /// "damage from spells"; we approximate with the closed set of
    /// spell-typical damage types (acid / cold / fire / lightning /
    /// thunder / necrotic / radiant / force / psychic — every element
    /// that shows up on the standard spell blast lane). Weapon-only
    /// types (bludgeoning / piercing / slashing / poison) are excluded
    /// so a paladin's aura doesn't accidentally halve a nearby swing's
    /// melee damage.
    ///
    /// Read at the `DealDamage::apply` chokepoint via
    /// `EncounterInstance::is_in_aura_of_warding` — the raw amount is
    /// halved BEFORE `effective_damage` runs so the standard 5e
    /// "one halving per damage instance" rule still holds (the aura
    /// no-ops when the target already has a same-type resistance /
    /// immunity from their template / condition / item lanes).
    /// Emitter must be combat-active and not incapacitated — RAW: the
    /// aura requires the paladin to be conscious, mirroring the
    /// Aura of Protection / Aura of Courage / Aura of Devotion gate.
    ///
    /// Ships on `ANCIENTS_PALADIN_TEMPLATE`. Distinct from Nature's
    /// Ward (self-immunity to Charmed / Frightened, self-only) and
    /// Undying Sentinel (once-per-long-rest drop-to-1-HP cheat-death):
    /// the aura extends the Ancients paladin's anti-magic identity to
    /// every 10ft-adjacent ally, forming a bubble that shrugs off the
    /// classic caster's Fireball / Cone of Cold / Lightning Bolt burst
    /// on the whole huddled party.
    pub has_aura_of_warding: bool,
    /// 5e Barbarian **Persistent Rage** (level 15 class feature). Passive
    /// template flag: the barbarian's Rage lasts longer. RAW says the
    /// rage no longer ends prematurely if the barbarian doesn't attack
    /// or take damage. Our engine's Rage doesn't end early to begin with
    /// — it holds for a fixed `Rounds(10)` timer regardless of activity
    /// — so the RAW "no premature end" clause is a no-op. We repurpose
    /// the flag as a duration bump: a Persistent-Rage barbarian's Rage
    /// installs for `Rounds(20)` instead of the baseline `Rounds(10)`,
    /// matching the RAW "1 minute → practically-encounter-length"
    /// intent by doubling the timer.
    ///
    /// Read by the `RAGE` action's `side_effects` — the rage-installer
    /// swaps the timer based on the caster's `has_persistent_rage()`
    /// flag. Ships on the CR-4 (level-9) Barbarian family templates
    /// above their strict RAW level gate for the same reason
    /// Relentless Rage / Feral Instinct / Brutal Critical do — class
    /// templates target a balanced playable level, not lockstep PHB
    /// progression. The extended timer composes cleanly with the
    /// Berserker's Frenzy (more raging turns = more Frenzy strikes),
    /// the Zealot's Divine Fury (more Rage rounds mean more turns
    /// where the once-per-turn rider fires), and each totem spirit
    /// (Bear's blanket resistance, Wolf's ally-adjacency aura, Eagle's
    /// bonus-action Dash, Tiger's +10 ft speed) — every effect that
    /// gates on Raging benefits from the doubled window.
    pub has_persistent_rage: bool,
    /// 5e Fighting Style: **Blind Fighting** (Tasha's Cauldron of
    /// Everything, Fighter / Ranger / Paladin lv1 pick). Passive
    /// concealment-piercer with a 10-ft (4-tile footprint Chebyshev)
    /// range gate — the holder has blindsight out to 10 ft. Sits in
    /// the same `pierces_illusion_of` cohort as `has_truesight` /
    /// `has_feral_senses` / `has_blindsense`, with the Blindsense-
    /// shape 10-ft envelope for a range-gated close-quarters piercer.
    ///
    /// Two distinctions from the neighboring flags:
    ///   1. **No hearing gate** — RAW: "you can see any creature that
    ///      isn't behind total cover in a 10-foot radius, even if
    ///      you're blinded or in darkness." No "while able to hear"
    ///      clause, so Deafened doesn't suppress the flag (unlike
    ///      Blindsense which lapses when the holder can't hear).
    ///   2. **Fighting-style pick** — mutually exclusive with the
    ///      other Fighting Style options at the RAW-level pick, but
    ///      cohabits engine-side with Dueling / Defense / Archery on
    ///      the fighter chassis (same "class templates target a
    ///      playable level, not lockstep PHB progression" reasoning
    ///      that lets the Champion carry Defense + Dueling).
    ///
    /// Not shipped on any current template by default — the flag
    /// exists so a future Battle Master / Paladin sub-build (a chassis
    /// that leans into close-quarters melee against invisible or
    /// concealed opponents) can opt in as a single-line template
    /// switch. Same "flag exists, not shipped by default" pattern the
    /// engine uses for `has_two_weapon_fighting_style`,
    /// `has_protection_style`, and `has_interception_style`. Ranger
    /// (Feral Senses at unbounded range) and Rogue (Blindsense at 10 ft)
    /// already carry redundant piercers so no ship-by-default there.
    pub has_blind_fighting_style: bool,
    /// 5e Dwarven Resilience: advantage on saving throws against poison
    /// AND resistance to poison damage. Read by `compute_save_mode`
    /// (advantage clause) and `effective_damage` (resistance clause).
    /// A single flag drives both halves because RAW: both clauses share
    /// the same trait gate.
    pub has_dwarven_resilience: bool,
    /// 5e Warlock Fiend Patron **Fiendish Resilience** (level 10).
    /// Passive template flag: the warlock has resistance to fire damage.
    /// RAW gives the warlock a rest-cycle choice ("choose one damage
    /// type when you finish a short or long rest; you have resistance
    /// to that damage type until you choose a different one") — we
    /// collapse the choice to a fixed Fire lock so the flag is a single
    /// template pick. Thematic for the Fiend patron's fire-anchored
    /// flavor (Burning Hands / Fireball / Wall of Fire on the expanded
    /// spell list, Dark One's Blessing as the kill-triggered temp-HP
    /// well) and it drops the need for a "picked type" mutable slot on
    /// `ActorInstance`.
    ///
    /// Read at the `effective_damage` chokepoint via the shared
    /// `PASSIVE_TYPED_RESISTANCES` cohort — same lane as Dwarven
    /// Resilience's poison-halving half. Ships on
    /// `FIEND_WARLOCK_TEMPLATE` above the strict RAW lv10 gate for the
    /// same reason Dark One's Own Luck (RAW lv6) and Dark One's
    /// Blessing (RAW lv1) do — class templates target a balanced
    /// playable level, not lockstep PHB progression. Adding a "choose
    /// any of the ten damage types on short rest" surface later is an
    /// `Option<DamageType>` swap at the cohort row plus a short-rest
    /// hook — no signature changes upstream.
    pub has_fiendish_resilience: bool,
    /// 5e Sorcerer Draconic Bloodline **Draconic Resilience** (level 6).
    /// Passive template flag: the sorcerer has resistance to the damage
    /// type associated with their draconic ancestry. RAW gives the
    /// sorcerer a bloodline-picked damage type — we collapse the choice
    /// to a fixed Fire lock (Red / Gold / Brass ancestor flavor, the
    /// most iconic sorcerer bloodline picks) so the flag is a single
    /// template pick without a mutable "picked type" slot on
    /// `ActorInstance`. Thematic for the Draconic Bloodline's fire-
    /// heavy identity (Burning Hands / Scorching Ray / Fireball as
    /// natural pickups on the sorcerer chassis) and it drops the need
    /// for a per-ancestor branch on `PASSIVE_TYPED_RESISTANCES`.
    ///
    /// Read at the `effective_damage` chokepoint via the shared
    /// `PASSIVE_TYPED_RESISTANCES` cohort — same lane as Dwarven
    /// Resilience's poison-halving half and Fiendish Resilience's fire-
    /// halving half. Ships on `DRACONIC_SORCERER_TEMPLATE` above the
    /// strict RAW lv6 gate for the same reason Fiendish Resilience
    /// (RAW lv10) rides on `FIEND_WARLOCK_TEMPLATE` above its strict
    /// gate — class templates target a balanced playable level, not
    /// lockstep PHB progression. Distinct from `has_fiendish_resilience`
    /// on the "fire resistance" lane — same magnitude, same damage
    /// type, different class chassis, so a hypothetical Fiend Warlock /
    /// Draconic Sorcerer multiclass carries both flags cleanly but the
    /// standard "one halving per damage instance" rule caps the total
    /// at a single /2 per Fire hit (the resistance folder never double-
    /// halves the same damage roll).
    ///
    /// Adding a "choose any of the ten damage types on bloodline pick"
    /// surface later is a `has_draconic_resilience: bool` →
    /// `draconic_resilience_type: Option<DamageType>` swap at this row,
    /// the cohort entry, and the accessor; no signature changes upstream
    /// on `effective_damage`.
    pub has_draconic_resilience: bool,
    /// 5e Gnome Cunning (Rock / Forest / Deep Gnome racial): advantage on
    /// Intelligence, Wisdom, and Charisma saving throws against magic.
    /// We don't tag saves by "magic vs mundane" in this engine, so we
    /// approximate by granting blanket advantage on INT / WIS / CHA
    /// saves. The false-positive surface is small — most non-magical
    /// effects targeting those abilities (skill checks, social mods)
    /// don't route through `roll_save`. Read by `compute_save_mode`.
    pub has_gnome_cunning: bool,
    /// 5e Swashbuckler Rogue (XGtE) level-3 subclass feature —
    /// **Rakish Audacity**. Two-part passive: (a) add the holder's
    /// Charisma modifier to initiative rolls, and (b) Sneak Attack
    /// qualifies without an ally adjacent to the target so long as no
    /// creature other than the target is within 5 ft of the target
    /// (RAW: "you don't need advantage on the attack roll to use your
    /// Sneak Attack against a creature if you are within 5 feet of it,
    /// no other creatures are within 5 feet of you, and you don't have
    /// disadvantage on the attack roll"). The advantage-clause survives
    /// intact via the existing Sneak Attack eligibility ladder — this
    /// flag only opens the second, no-ally path.
    ///
    /// Read at two chokepoints:
    ///   - `initiative_flat_bonus` — the CHA-mod bump rides the
    ///     shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort alongside
    ///     Dread Ambusher's WIS-mod bump. Composes cleanly with
    ///     `rolls_initiative_with_advantage` (Feral Instinct) so a
    ///     hypothetical Barbarian / Swashbuckler multiclass rolls with
    ///     advantage AND stacks CHA-mod on top.
    ///   - `class_attacks::sneak_attack_eligible` — the "solo duelist"
    ///     path fires when no OTHER hostile creature is within 5 ft of
    ///     the target (footprint-adjacent), the target is footprint-
    ///     adjacent to the swashbuckler, and the swash has not swung
    ///     with disadvantage this turn. Distinct from the ally-adjacency
    ///     path (an ally within 5 ft of the target) — the swash covers
    ///     the "no one else is here" gap.
    ///
    /// Ships on `SWASHBUCKLER_ROGUE_TEMPLATE`; no other current template
    /// carries the flag. Sibling on the "template flag opens a new
    /// Sneak Attack qualification path" lane to Steady Aim (bonus-action
    /// self-advantage, already on the baseline rogue chassis) — Rakish
    /// Audacity is the passive version.
    pub has_rakish_audacity: bool,
    /// 5e Swashbuckler Rogue (XGtE) level-3 subclass feature —
    /// **Fancy Footwork**. Passive template flag: on the swashbuckler's
    /// turn, any creature they make a melee attack against can't make
    /// opportunity attacks against them for the rest of the turn. RAW:
    /// "During your turn, if you make a melee attack against a
    /// creature, that creature can't make opportunity attacks against
    /// you for the rest of your turn."
    ///
    /// Read at `EncounterInstance::dispatch_opportunity_attacks`
    /// alongside the `is_disengaging()` gate: when the swashbuckler
    /// moves out of a reactor's reach, each candidate reactor id is
    /// checked against the swash's per-turn "melee-attacked" ledger
    /// (`melee_attack_targets_this_turn`). If the reactor was on the
    /// swash's melee-attack list this turn AND the swash holds this
    /// flag, that reactor's OA is silently suppressed. Other reactors
    /// (an untouched flanker the swash never swung at) still fire OAs
    /// normally.
    ///
    /// The ledger is marked at the top of `resolve_attack` for
    /// `is_melee` swings — every melee action routing through the
    /// shared attack chokepoint (shortsword, dagger, monk unarmed,
    /// smite riders, opportunity attacks themselves) writes the
    /// target id into the swash's ledger. Cleared at the swash's
    /// turn-start reset alongside the other per-turn HashSets.
    ///
    /// Ships on `SWASHBUCKLER_ROGUE_TEMPLATE`; no other current template
    /// carries the flag. Sibling to Disengage — Disengage suppresses
    /// ALL OAs for the turn, Fancy Footwork surgically suppresses only
    /// the swash's chosen melee targets, freeing the swash's bonus
    /// action for Cunning Strike primes rather than Cunning Disengage.
    pub has_fancy_footwork: bool,
    /// 5e Gloom Stalker Ranger (XGtE) level-3 subclass feature —
    /// **Dread Ambusher**. The load-bearing half we model is the
    /// initiative bump: RAW "You have a bonus to your initiative rolls
    /// equal to your Wisdom modifier." (The RAW first-turn extra attack
    /// + bonus damage half is left as future work; the initiative bump
    ///   is the tell that anchors the Gloom Stalker's "always strikes
    ///   first" identity.) Read by `initiative_flat_bonus` through the
    ///   shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort alongside Rakish
    ///   Audacity's CHA-mod bump; Remarkable Athlete's `+ceil(prof / 2)`
    ///   stays as its own if-branch on the same helper. Ships on
    ///   `GLOOM_STALKER_RANGER_TEMPLATE`; no other current template
    ///   carries the flag. Sibling on the "template flag → one-ability-
    ///   mod initiative-bump" cohort lane — the cohort now covers two
    ///   distinct ability modifiers (Rakish Audacity → CHA, Dread
    ///   Ambusher → WIS), each additive so a hypothetical Gloom Stalker
    ///   Ranger / Swashbuckler Rogue multiclass carries both bumps
    ///   cleanly.
    pub has_dread_ambusher: bool,
    /// 5e Dragonborn Draconic Ancestry: damage type matching the chosen
    /// ancestor (Red / Gold = Fire, Blue / Bronze = Lightning, etc.).
    /// Read by `BreathWeapon` to type its 5-tile cone and consumed by
    /// `damage_modifiers` to give the dragonborn matching resistance.
    /// `None` for non-dragonborn templates (the default).
    pub draconic_ancestry: Option<DamageType>,
    /// 5e Sorcerer Sorcery Points: the resource pool spent on Metamagic
    /// (Empowered Spell, Quickened Spell, Twinned Spell, etc.). The
    /// sorcerer's pool refreshes on a long rest. RAW: 2 + level points
    /// at L2, scaling to 20 by L20. We expose the cap directly so
    /// templates can pin the value to a level-appropriate count
    /// (e.g. 4 for a CR-4 sorcerer ≈ level 4). 0 = no sorcery points
    /// (the default for non-sorcerer creatures).
    pub sorcery_points: u32,
    /// 5e **Death Burst** trigger — a final burst this creature fires
    /// automatically when reduced to 0 HP (mephit cohort, magmin,
    /// future ash-zombie style entries). `None` = no on-death burst
    /// (the default for everything else). Resolved by
    /// `EncounterInstance::cleanup_dead_actors` *before* the dying actor
    /// is removed from the map, so the burst centers on the corpse's
    /// own tile. The struct lives in `actions::monster_attacks` next to
    /// `BreathWeapon` since it shares the same save-burst chassis.
    pub death_burst: Option<&'static crate::actions::monster_attacks::DeathBurst>,
    /// 5e **natural melee reflect** — a creature-intrinsic version of the
    /// Fire Shield / Armor of Agathys retaliation rider, but keyed off
    /// the creature's body rather than a transient condition. Black
    /// Pudding's Corrosive Form (1d8 acid on every melee contact) and
    /// Salamander's Heated Body (1d6 fire) are the canonical entries.
    /// `None` for the vast majority of creatures. The damage feeds
    /// through the standard damage pipeline so the attacker's typed
    /// resistance / immunity / vulnerability is honored. The reflect
    /// fires on every melee swing connecting with the holder — distinct
    /// from `death_burst` which fires once on 0-HP. Composes additively
    /// with condition-keyed `MELEE_REFLECT_RIDERS`: a salamander wearing
    /// Fire Shield rolls *both* reflects on the same incoming hit.
    pub natural_melee_reflect: Option<crate::engine::attack::MeleeReflect>,
    /// Conditions to install on this creature the moment it enters the
    /// encounter — the "creature is born already X" lane. Each entry is
    /// applied once via `add_condition` at instantiation. The canonical
    /// case is the **Invisible Stalker** (RAW: "The stalker is invisible.")
    /// installing `(Invisible, Permanent)`; new templates whose flavor
    /// includes a passive always-on body-state buff (a future Flesh-Golem
    /// `DamageResistant`, a permanent self-haste, etc.) add an entry
    /// here. Empty for the vast majority of creatures.
    ///
    /// Distinct from `has_displacement`, which also re-installs the
    /// `Displaced` condition at start-of-turn after damage strips it —
    /// the displacement-restore lane is a separate mechanic. Innate
    /// conditions listed here are installed exactly once and not
    /// auto-restored if dispelled / consumed mid-fight.
    pub innate_conditions: Vec<(Condition, ConditionTimer)>,
}

impl CreatureTemplate {
    /// Returns a `CreatureTemplate` populated with sensible defaults —
    /// empty item / sense / skill / spell-slot / language / damage-modifier
    /// collections, 10 across every ability, Medium beast with a 1d8 hit-die
    /// pool, AC 10, speed 30 ft, every passive feature off, the standard
    /// DEFAULT_ACTIONS pool pre-populated, and the natural-20 crit floor.
    /// Designed for use with struct update syntax so creature definitions
    /// only need to list fields that differ from the baseline:
    ///
    /// ```ignore
    /// let mut actions = DEFAULT_ACTIONS.clone();
    /// actions.push(&BROWN_BEAR_BITE);
    /// CreatureTemplate {
    ///     name: "Brown Bear",
    ///     glyph: 'B',
    ///     ac: 11,
    ///     hitpoints: "4d10+12".parse().unwrap(),
    ///     speed: 40.,
    ///     strength: 19,
    ///     // ... only the fields that differ from defaults ...
    ///     actions,
    ///     ..CreatureTemplate::defaults()
    /// }
    /// ```
    ///
    /// Cuts the ~50-line "default tail" each creature file had to spell out
    /// by hand, and lets new template fields (added in future work) land
    /// without an N-file mechanical edit — old templates pick up the new
    /// field's default automatically via `..defaults()`.
    /// Clone this template as a subclass build: override `name` + `glyph`
    /// (the two per-subclass identity axes) and insert a single passive
    /// feature `subclass_tag` into the shared feature set. The rest of the
    /// template — actions, stats, spell slots, save profs, struct-field
    /// flags — carries through the `..self.clone()` tail unchanged.
    ///
    /// The **cross-class shared helper** for the "clone base + insert one
    /// tag" declarative-template pattern that every tag-only subclass
    /// build on every class chassis has been repeating by hand.
    /// Consolidates the per-class ad-hoc bodies (a five-line `let mut
    /// features = BASE.features.clone(); features.insert(TAG);
    /// CreatureTemplate { name, glyph, features, ..BASE.clone() }` block
    /// repeated once per subclass) into a single method call per
    /// subclass template.
    ///
    /// Users (23+ callsites across eight class chassis today):
    ///   - **Warlock** (8 tag-only Otherworldly Patron subclass templates
    ///     via the class-scoped `subclass_warlock_template` helper —
    ///     Undying / Great Old One / Archfey / Celestial / Marid / Dao /
    ///     Djinni / Efreeti).
    ///   - **Sorcerer** (3 tag-only Sorcerous Origin subclass templates —
    ///     Aberrant Mind, Divine Soul, Shadow Magic).
    ///   - **Wizard** (3 tag-only Arcane Tradition subclass templates —
    ///     Necromancy, War Magic, Illusion).
    ///   - **Cleric** (3 tag-only Divine Domain subclass templates —
    ///     Life, Forge, Twilight).
    ///   - **Monk** (1 tag-only Monastic Tradition subclass template —
    ///     Way of the Long Death).
    ///   - **Paladin** (2 tag-only Sacred Oath subclass templates —
    ///     Oath of Glory, Oath of the Watchers).
    ///   - **Fighter** (1 tag-only Martial Archetype subclass template —
    ///     Samurai).
    ///   - **Ranger** (4 tag-only Conclave subclass templates — Fey
    ///     Wanderer, Horizon Walker, Monster Slayer, Swarmkeeper).
    ///
    /// The **Bard** chassis has its own class-scoped
    /// `subclass_bard_template` helper (see `bards.rs`) with an optional
    /// `subclass_tag` axis that layers a single tag into the baseline
    /// features set the same way this helper does — sibling shape,
    /// specialized for bards to also cover the extra_attack /
    /// dueling_style flag flips and per-subclass action-layer picks the
    /// Valor / Swords / Lore / Whispers family carries.
    ///
    /// Distinct from `subclass_barbarian_template` (barbarian family
    /// helper): that helper builds the shared level-9 envelope from
    /// scratch rather than cloning off a base template, so it doesn't
    /// route through this method. Also distinct from tag-plus-actions
    /// / tag-plus-struct-field-flag subclass templates (Fiend Warlock,
    /// War / Light / Tempest Cleric, Berserker / Zealot Barbarian, Storm
    /// Sorcerer, the four paladin oaths) which layer more than a single
    /// feature tag on top of the baseline — those subclasses stay on the
    /// per-file explicit clone-and-insert body since the helper's
    /// tag-only interface can't express the additional actions / flags /
    /// languages / tags they need.
    ///
    /// A new tag-only subclass on any class chassis lands as a one-line
    /// entry: `BASE_TEMPLATE.with_subclass_tag(name, glyph, tag)`.
    pub fn with_subclass_tag(
        &self,
        name: &'static str,
        glyph: char,
        subclass_tag: &'static str,
    ) -> Self {
        let mut features = self.features.clone();
        features.insert(subclass_tag);
        Self {
            name,
            glyph,
            features,
            ..self.clone()
        }
    }

    pub fn defaults() -> Self {
        use crate::actions::default_actions::DEFAULT_ACTIONS;
        Self {
            name: "",
            glyph: '?',
            ac: 10,
            hitpoints: "1d8".parse().unwrap(),
            speed: 30.0,
            strength: 10,
            intelligence: 10,
            dexterity: 10,
            wisdom: 10,
            constitution: 10,
            charisma: 10,
            skills: HashSet::new(),
            items: Vec::new(),
            senses: HashSet::new(),
            languages: HashSet::new(),
            cr: 0.0,
            size: Size::Medium,
            creature_type: CreatureType::Beast,
            actions: DEFAULT_ACTIONS.clone(),
            spell_slots_by_level: Vec::new(),
            rolls_death_saves: false,
            damage_modifiers: HashMap::new(),
            nonmagical_damage_modifiers: HashMap::new(),
            proficient_saves: HashSet::new(),
            condition_immunities: HashSet::new(),
            charge: None,
            mountable: false,
            features: HashSet::new(),
            regen_per_round: 0,
            regen_suppressors: HashSet::new(),
            legendary_resistances: 0,
            arcane_ward_base: 0,
            portent_dice: 0,
            transmuters_stone: None,
            has_evasion: false,
            has_mounted_combatant: false,
            has_uncanny_dodge: false,
            has_deflect_missiles: false,
            has_parry: false,
            has_riposte: false,
            has_displacement: false,
            has_danger_sense: false,
            has_pack_tactics: false,
            sunlight_frailty: None,
            is_swarm: false,
            has_magic_resistance: false,
            recharge_abilities: Vec::new(),
            legendary_actions_per_round: 0,
            lair_actions: &[],
            has_extra_attack: false,
            brutal_critical_dice: 0,
            crit_threshold: 20,
            has_lucky: false,
            has_brave: false,
            has_fey_ancestry: false,
            has_aura_of_protection: false,
            has_aura_of_courage: false,
            has_aura_of_devotion: false,
            has_elusive: false,
            has_natures_ward: false,
            has_feral_instinct: false,
            has_remarkable_athlete: false,
            has_superior_critical: false,
            has_diamond_soul: false,
            has_slippery_mind: false,
            has_iron_mind: false,
            has_savage_attacks: false,
            has_archery_style: false,
            has_defense_style: false,
            has_dueling_style: false,
            has_great_weapon_fighting: false,
            has_two_weapon_fighting_style: false,
            has_protection_style: false,
            has_interception_style: false,
            has_feral_senses: false,
            has_blindsense: false,
            has_blind_fighting_style: false,
            has_aura_of_hate: false,
            has_aura_of_conquest: false,
            has_scornful_rebuke: false,
            has_aura_of_warding: false,
            has_persistent_rage: false,
            has_dwarven_resilience: false,
            has_fiendish_resilience: false,
            has_draconic_resilience: false,
            has_gnome_cunning: false,
            has_rakish_audacity: false,
            has_fancy_footwork: false,
            has_dread_ambusher: false,
            draconic_ancestry: None,
            sorcery_points: 0,
            death_burst: None,
            natural_melee_reflect: None,
            innate_conditions: Vec::new(),
        }
    }
}

/// Collect an explicit list of unqualified per-type damage modifiers
/// into the map `CreatureTemplate::damage_modifiers` wants.
///
/// Exists so a template can write `damage_modifiers_from([])` — the
/// empty case `HashMap::from([])` cannot infer — and so the forty
/// templates that pair an unqualified overlay list with
/// `resistant_to_nonmagical_physical` do not each have to import
/// `HashMap` to say "no overlays".
pub fn damage_modifiers_from(
    entries: impl IntoIterator<Item = (DamageType, DamageModifier)>,
) -> HashMap<DamageType, DamageModifier> {
    entries.into_iter().collect()
}

impl CreatureTemplate {
    /// The three physical damage types, resisted, qualified to nonmagical
    /// attacks — 5e's "resistance to bludgeoning, piercing, and slashing
    /// damage from nonmagical attacks", and the only thing in the engine
    /// that writes that triplet.
    ///
    /// Returned as a whole `CreatureTemplate` for the `..` tail of a
    /// template literal rather than as a bare map for the
    /// `nonmagical_damage_modifiers` field, and that is the point of it.
    /// The clause is a *pairing* — a qualified resistance is only
    /// correct alongside the absence of an unqualified one — and the
    /// tail position is the one place a template cannot set the field
    /// twice or set it with the wrong qualifier. Forty stat blocks carry
    /// this clause; every one of them now says so by writing
    ///
    /// ```ignore
    /// CreatureTemplate {
    ///     damage_modifiers: damage_modifiers_from([
    ///         (DamageType::Poison, DamageModifier::Immunity),
    ///     ]),
    ///     ..CreatureTemplate::resistant_to_nonmagical_physical()
    /// }
    /// ```
    ///
    /// and the unqualified overlays stay visibly unqualified.
    ///
    /// The predecessor of this constructor dropped the triplet straight
    /// into `damage_modifiers` with a docstring conceding "we don't
    /// track magical-vs-mundane weapon distinctions". The engine now
    /// does — see `crate::engine::magic` — which turns a +1 longsword
    /// and a paladin's Divine Smite into the answer to a wraith rather
    /// than one more halved swing.
    pub fn resistant_to_nonmagical_physical() -> CreatureTemplate {
        CreatureTemplate {
            nonmagical_damage_modifiers: HashMap::from([
                (DamageType::Bludgeoning, DamageModifier::Resistance),
                (DamageType::Piercing, DamageModifier::Resistance),
                (DamageType::Slashing, DamageModifier::Resistance),
            ]),
            ..CreatureTemplate::defaults()
        }
    }
}

/// The 5e "incorporeal undead" envelope shared by Ghost / Wraith /
/// Specter / Shadow: immune to Necrotic + Poison damage and to a long
/// menu of body-control conditions (Charmed, Exhausted, Frightened,
/// Grappled, Paralyzed, Petrified, Poisoned, Prone, Restrained,
/// Unconscious). Each individual template still overlays its own damage-
/// type resistances (cold for the wraith, radiant vulnerability for the
/// shadow demon, etc.) and may opt out of an immunity by re-inserting it
/// into a smaller set, but the common base lives here so a new
/// incorporeal undead lands as a one-line `.clone()` instead of a 10-line
/// literal.
pub static INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES: std::sync::LazyLock<HashSet<Condition>> =
    std::sync::LazyLock::new(|| {
        HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
            Condition::Unconscious,
        ])
    });

#[derive(Clone, PartialEq)]
pub struct SpellSlotInfo {
    pub max_spell_slots: u32,
    pub spell_slots: u32,
}

#[derive(Clone, PartialEq)]
pub struct SpellSlotManager {
    ssi_by_lvl: Vec<SpellSlotInfo>,
}

impl SpellSlotManager {
    fn idx(lvl: u32) -> Option<usize> {
        lvl.checked_sub(1).map(|n| n as usize)
    }

    pub fn spell_slots(&self, lvl: u32) -> SpellSlotInfo {
        Self::idx(lvl)
            .and_then(|i| self.ssi_by_lvl.get(i).cloned())
            .unwrap_or(SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            })
    }

    pub fn consume_spell_slot(&mut self, lvl: u32) -> bool {
        let Some(i) = Self::idx(lvl) else {
            return false;
        };
        let Some(ssi) = self.ssi_by_lvl.get_mut(i) else {
            return false;
        };
        if ssi.spell_slots == 0 {
            return false;
        }
        ssi.spell_slots -= 1;
        true
    }

    pub fn restore_spell_slot(&mut self, lvl: u32, qty: u32) -> bool {
        let Some(i) = Self::idx(lvl) else {
            return false;
        };
        let Some(ssi) = self.ssi_by_lvl.get_mut(i) else {
            return false;
        };
        if ssi.spell_slots + qty > ssi.max_spell_slots {
            return false;
        }
        ssi.spell_slots += qty;
        true
    }

    pub fn restore_spell_slots(&mut self) {
        for ssi in self.ssi_by_lvl.iter_mut() {
            ssi.spell_slots = ssi.max_spell_slots;
        }
    }

    /// Restore one expended slot at the highest level that is `<= cap`
    /// and currently below its maximum, returning that level. `None`
    /// when no slot in the `1..=cap` band is expended (or `cap == 0`).
    ///
    /// Highest-first rather than lowest-first because every caller is a
    /// "you regain one expended spell slot of a level lower than X"
    /// feature (Divination Wizard's Expert Divination today), and the
    /// most valuable slot in the eligible band is always the one the
    /// holder would pick. Distinct from `restore_spell_slot`, which
    /// targets one named level and fails if that level is already full.
    pub fn restore_highest_expended_slot_up_to(&mut self, cap: u32) -> Option<u32> {
        (1..=cap).rev().find(|&lvl| self.restore_spell_slot(lvl, 1))
    }

    pub fn increase_max_spell_slot(&mut self, lvl: u32, qty: u32) {
        let Some(i_usize) = Self::idx(lvl) else {
            return;
        };
        for _ in self.ssi_by_lvl.len()..=i_usize {
            self.ssi_by_lvl.push(SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            });
        }
        self.ssi_by_lvl[i_usize].max_spell_slots += qty;
        self.ssi_by_lvl[i_usize].spell_slots += qty;
    }
}

/// Expand a template's flat set of feature tags into the charge map an
/// `ActorInstance` carries, asking `class_features::feature_charges`
/// how deep each pool runs.
///
/// The template side stays a `HashSet<&'static str>` on purpose: a
/// template says *which* features a creature has, and the size of a
/// feature's pool is a property of the feature, not of whoever carries
/// it. Writing the count on the template would mean every chassis that
/// picks up Arcane Shot has to remember the number, and one that forgot
/// would silently ship a weaker subclass.
fn feature_charge_map(features: &HashSet<&'static str>) -> HashMap<&'static str, u32> {
    features
        .iter()
        .map(|&tag| (tag, crate::actions::class_features::feature_charges(tag)))
        .collect()
}

#[derive(Clone)]
pub struct ActorInstance {
    name: String,
    location: Coordinate,
    /// Feet of movement this actor has actually spent on its current
    /// turn, written by `consume_resource` and cleared by
    /// `reset_for_new_round`.
    ///
    /// Exists because "has this creature moved yet" cannot be recovered
    /// from the remaining budget, which is what it used to be inferred
    /// from. `movement < speed()` is wrong three ways: a Dash refills
    /// the budget, so a rogue who dashed and then walked its whole speed
    /// reads as standing still; a Haste that lands mid-turn raises
    /// `speed()` out from under a budget that was filled at the old
    /// value, so an actor that has not moved reads as having moved; and
    /// a Slow does the same in the other direction. A counter that only
    /// ever goes up when feet are spent has none of those failure modes.
    movement_spent_this_turn: f32,
    /// Where the actor's current straight run began, and the unit
    /// direction it is running in — `None` between runs.
    ///
    /// The board only ever knows where a creature *is*. A whole family
    /// of 5e riders keys off where it came from: "if the creature moves
    /// at least 20 feet straight toward a target and then hits it" is
    /// the Boar's Charge, the Triceratops's Trampling Charge, the
    /// Centaur's, the Unicorn's, and both big cats' Pounce, and every
    /// one of those stat blocks shipped here with the clause dropped and
    /// a comment saying the engine could not see the movement.
    ///
    /// A *run*, not the turn's net displacement. RAW asks for twenty
    /// straight feet before the blow, not for a whole turn spent in one
    /// direction — a boar that sidesteps a rock and then puts its head
    /// down has charged. So the pair is maintained step by step by
    /// `note_walked_step`: a step that continues the run leaves the
    /// origin where it is, and one that turns re-anchors it. Anything
    /// that is not the creature walking — a shove, a teleport, the top
    /// of a new turn — calls `break_run` and the count starts over.
    run_origin: Coordinate,
    run_step: Option<Coordinate>,
    team_id: usize,
    base_ac: u32,
    base_hitpoints: u32,
    base_speed: f32,
    base_size: Size,
    initiative: Option<i32>,
    strength: u32,
    intelligence: u32,
    dexterity: u32,
    wisdom: u32,
    constitution: u32,
    charisma: u32,
    skills: HashSet<Skill>,
    items: Vec<&'static Item>,
    senses: HashSet<SpecialSense>,
    /// Languages the creature speaks, copied from its template.
    ///
    /// The one genuinely unread field on this struct, and kept
    /// deliberately: it is part of what a creature *is*, the templates
    /// all fill it in, and combat simply has no surface that asks. The
    /// allow is scoped to this field rather than to the struct — a
    /// blanket one used to sit on `ActorInstance` and it is what let
    /// `base_size` be written and never read for as long as it was.
    #[allow(dead_code)]
    languages: HashSet<Language>,
    cr: f32,
    hitpoints: u32,
    movement: f32,
    action_slots: u32,
    bonus_action_slots: u32,
    reaction_slots: u32,
    legendary_action_slots: u32,
    size: Size,
    creature_type: CreatureType,
    pub spell_slot_manager: SpellSlotManager,
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
    glyph: char,
    hp_state: HpState,
    conditions: HashMap<Condition, ConditionTimer>,
    concentration: Option<ConcentrationData>,
    rolls_death_saves: bool,
    /// Per-damage-type modifier table copied from the creature template.
    damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// The source-qualified half of the table, copied from
    /// `CreatureTemplate::nonmagical_damage_modifiers` — the rows that
    /// only fire against damage from a nonmagical attack.
    nonmagical_damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// 5e temporary hit points. Damage drains temp HP before regular HP.
    /// Doesn't stack: a new grant replaces existing temp HP only if
    /// larger. Cleared on long rest.
    temp_hp: u32,
    /// 5e Abjuration Wizard **Arcane Ward** (subclass level 2) — the
    /// ward's current hit points. A separate pool from `temp_hp`: it is
    /// drained *first* (RAW "the ward takes the damage instead of you",
    /// which resolves before the "damage to you" that temp HP absorbs),
    /// it persists at 0 rather than vanishing (RAW "while the ward has
    /// 0 hit points it can't absorb damage, but its magic remains"),
    /// and it refills off abjuration casts rather than off a fresh
    /// grant-the-larger-value rule. Only meaningful while
    /// `arcane_ward_formed` is set.
    arcane_ward: u32,
    /// Whether the Arcane Ward has been woven yet this encounter. The
    /// ward doesn't exist until its holder casts their first abjuration
    /// spell of 1st level or higher (RAW); before that, the pool is
    /// dormant and absorbs nothing. Distinguishes "ward at 0 HP, still
    /// rechargeable" from "ward never formed", which the recharge hook
    /// needs to tell apart: the first abjuration cast fills the ward to
    /// its maximum, every later one restores only twice the slot level.
    arcane_ward_formed: bool,
    /// RAW "twice your wizard level" term of the Arcane Ward maximum,
    /// pre-doubled into a flat pool size by the template. 0 disables the
    /// feature entirely — the overwhelming majority of actors. Carried
    /// as a template constant rather than derived from `level` because
    /// `level` tracks in-run XP progression from 1, not the build level
    /// the class template targets; the same reason the Champion ships
    /// `crit_threshold: 19` rather than deriving it from a level gate.
    /// The holder's Intelligence modifier is added on top at
    /// ward-formation time, so the full RAW maximum is
    /// `arcane_ward_base + INT mod`.
    arcane_ward_base: u32,
    /// 5e Divination Wizard **Portent** (subclass level 2) — the
    /// foretold d20 faces still unspent, one entry per banked die.
    /// Empty means either "no feature" or "all dice spent"; the two are
    /// told apart by `portent_dice_max` and `portent_forecast` rather
    /// than by this vec's length.
    ///
    /// Order is not significant — the spend path picks by *value*
    /// (highest face for a roll the diviner wants to succeed, lowest
    /// for one they want to fail), so the pool behaves as a multiset.
    portent_pool: Vec<u32>,
    /// Whether the foretold dice have been rolled since the last long
    /// rest. Distinguishes "not forecast yet" from "forecast and fully
    /// spent", which the lazy fill needs to tell apart: an un-forecast
    /// holder rolls a fresh `portent_dice_max` faces on first use, a
    /// spent-out one rolls nothing until the next long rest.
    ///
    /// The fill is lazy rather than done inside `long_rest` because
    /// `long_rest` takes no roller — the dice are rolled off the
    /// encounter's seeded roller at the first substitution opportunity
    /// instead, which keeps the values reproducible by seed without
    /// threading a roller through every rest call site. Nothing can
    /// observe the difference: the pool is opaque until it is read.
    portent_forecast: bool,
    /// RAW count of foretold dice the holder banks per long rest (2 at
    /// Portent, 3 at Greater Portent). 0 disables the feature entirely
    /// — the overwhelming majority of actors. Template constant for the
    /// same reason `arcane_ward_base` is one.
    portent_dice_max: u32,
    /// 5e Transmutation Wizard **Transmuter's Stone** — the benefit the
    /// holder's stone is attuned to, copied from the template at
    /// instantiation. `None` for every actor without the subclass.
    /// Read by three separate cohorts (speed, save proficiency, typed
    /// resistance) which each match on the variant they serve.
    transmuters_stone: Option<TransmutersStoneBenefit>,
    /// 5e Evocation Wizard **Overchannel** (subclass level 14) — how
    /// many times the holder has maximized a spell since their last long
    /// rest. Drives the escalating backlash: the first use is free, the
    /// second costs 2d12 necrotic per spell level, and each later use
    /// adds another d12 per level. Counting uses (rather than storing the
    /// next die count) keeps the RAW formula readable at the one site
    /// that applies it.
    overchannel_uses: u32,
    /// Set at the damage-roll site when Overchannel actually fires,
    /// cleared by the post-cast trigger that charges the backlash.
    ///
    /// The two steps can't be one: RAW maximizes the damage *during* the
    /// cast and charges the caster "immediately after you cast it", and
    /// those are different phases of `Action::execute` — the roll happens
    /// inside `side_effects`, the backlash has to land in the post-cast
    /// trigger dispatch so it resolves as a normal `DealDamage` (with
    /// concentration checks and death handling) rather than as a raw HP
    /// poke from inside a dice helper.
    overchannel_backlash_pending: bool,
    level: u32,
    xp: u32,
    /// Saving-throw proficiencies — adds proficiency bonus to roll_save.
    proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions the actor is wholly immune to.
    condition_immunities: HashSet<Condition>,
    /// This creature's charge clause, copied from its template. See
    /// `CreatureTemplate::charge`.
    charge: Option<ChargeRider>,
    /// Whether this creature can be ridden at all, copied from its
    /// template. See `CreatureTemplate::mountable`.
    mountable: bool,
    /// The creature this actor is currently sitting on, or `None` for
    /// everybody on their own feet.
    ///
    /// Half of a two-sided link the engine keeps in step:
    /// `a.mounted_on == Some(b)` iff `b.ridden_by == Some(a)`, and
    /// `EncounterInstance::{mount, dismount}` are the only two writers
    /// of either side. Nothing outside that pair sets these — a
    /// one-sided write would leave a rider glued to a horse that has
    /// forgotten them, and the board would keep rendering both.
    ///
    /// A mounted rider is *off the occupancy grid*: `actor_map` holds
    /// one id per tile and the pair shares a space, so while the link is
    /// up the mount owns the tiles and the rider's `location` is kept
    /// mirrored to the mount's. That mirroring is what lets every
    /// distance, aura, line-of-sight and burst query keep working on the
    /// rider unchanged — they all read `location()` and `size()`, not
    /// the grid.
    mounted_on: Option<usize>,
    /// The creature currently sitting on this actor. The other side of
    /// the `mounted_on` link; see there.
    ridden_by: Option<usize>,
    /// Class-feature charges currently unspent, keyed by feature tag
    /// (decremented on use, refilled to `features_max` on long rest and
    /// — for the short-rest cohorts — on short rest).
    ///
    /// A count rather than a set membership: most features hold exactly
    /// one charge, but a feature whose RAW resource is a pool declares
    /// its size in `FEATURE_CHARGES` and gets that many. A tag present
    /// with a count of zero is a *spent* feature the holder still
    /// carries — which is why `has_passive_feature` reads `features_max`
    /// and `feature_available` reads this map's value, not its keys.
    features_remaining: HashMap<&'static str, u32>,
    features_max: HashMap<&'static str, u32>,
    /// Bless / Resistance flat to-hit and save bonuses. Independent of the
    /// `Blessed` condition flag for stacking flexibility.
    attack_bonus_buff: i32,
    save_bonus_buff: i32,
    /// Spell-installed flat damage-roll bonus (Magic Weapon, Elemental
    /// Weapon). Symmetric with `attack_bonus_buff` on the to-hit lane —
    /// concentration installs delta via `AdjustDamageBuff` and rolls it
    /// back on drop. Independent of `ItemBonuses.damage_bonus` (which is
    /// the passive carried-item lane); both sources sum at the damage-
    /// roll site via `caster_damage_buffs`.
    damage_bonus_buff: i32,
    /// Shared once-per-turn rider ledger — a set of feature tags
    /// whose "already fired this turn" state is tracked in one place
    /// instead of a bool field per feature. Marked at the swing site
    /// via `mark_once_per_turn_used(tag)`; cleared wholesale at
    /// turn-start by `reset_for_new_round`. Read via
    /// `once_per_turn_used(tag)` — the four legacy thin-wrappers
    /// (`sneak_attack_used` / `colossus_slayer_used` /
    /// `foe_slayer_used` / `divine_fury_used`) delegate to it so
    /// pre-refactor callsites keep the same one-liner shape; newer
    /// tags (Dreadful Strikes, Psychic Blades, Planar Warrior,
    /// Slayer's Prey, Gathered Swarm) route through the generic API
    /// directly. The tag list is documented in
    /// `class_features::ONCE_PER_TURN_RIDER_TAGS`.
    once_per_turn_marks: HashSet<&'static str>,
    /// 5e Hunter Ranger **Multiattack Defense** (Defensive Tactics
    /// option, lv7) ledger. Every target this actor lands a connecting
    /// attack on this turn is inserted here, keyed by target id; a
    /// target with `MULTIATTACK_DEFENSE_TAG` reads this set to know
    /// which of its attackers have already hit it (and thus eat the +4
    /// AC penalty for the rest of the attacker's turn). Cleared at
    /// turn-start by `reset_for_new_round`, so the +4 envelope resets
    /// cleanly at the top of each attacker's next turn.
    ///
    /// Kept on the attacker side (rather than a per-target
    /// "attackers-that-hit-me" set) because RAW's "rest of the turn"
    /// clause is anchored to the attacker's turn — turn-start reset on
    /// the attacker is the natural chokepoint, whereas a target-side
    /// set would need per-attacker turn-tracking to know when to clear.
    hit_targets_this_turn: HashSet<usize>,
    /// 5e Rogue Assassin **Assassinate** (level 3) tracker. Flipped to
    /// `true` the first time this actor begins a turn in the encounter
    /// — set by the engine's `start_turn_for` hook. Read at
    /// `compute_attack_mode` next to the Pack Tactics / Wolf Totem
    /// branches: an Assassin rogue rolls with advantage against any
    /// target whose `has_taken_turn_in_combat` is still false. Latches
    /// once-only and is never cleared mid-encounter (RAW: "any creature
    /// that hasn't taken a turn in the combat yet").
    has_taken_turn_in_combat: bool,
    /// 5e Barbarian Relentless Rage DC. Starts at 10, climbs by 5 each
    /// time the feature successfully pins the holder at 1 HP, resets to
    /// 10 on short / long rest. Stored alongside the feature flag rather
    /// than a separate per-rest counter so the DC progression keeps
    /// matching the once-per-rest pattern the other features use.
    relentless_rage_dc: u32,
    /// Help-action grants. Map of helper_id → target_id where the helper
    /// is providing advantage on the helped actor's next attack vs the
    /// listed target. Consumed when the helped actor attacks the target.
    help_grants: HashMap<usize, usize>,
    /// 5e regenerator state: how much HP to recover each round-end while
    /// combat-active, and which damage types disable that heal for one
    /// round. `regen_suppressed` is set by `DealDamage` whenever damage
    /// of a suppressor type lands and cleared by `round_end` after the
    /// heal is skipped.
    regen_per_round: u32,
    regen_suppressors: HashSet<DamageType>,
    regen_suppressed: bool,
    /// Remaining 5e Mirror Image decoys. Each incoming attack rolls
    /// against the decoy pool first; a hit pops one decoy and misses the
    /// caster. Cleared when concentration drops or the pool hits zero
    /// (which also strips the MirroredImages condition).
    mirror_images: u32,
    /// Back-links for the conditions that need to remember *who* put them
    /// there, keyed by the condition itself.
    ///
    /// Seven conditions carry one, and they all wanted the same thing —
    /// Charmed needs the charmer so the victim can't swing back at them,
    /// Dueled and Goaded need the marker so attacks on anyone else take
    /// disadvantage, Distracted needs it to exclude the marker from the
    /// advantage it hands everyone else, Sworn and EldritchStruck need it
    /// so only the marker collects, and WardingBonded needs the partner
    /// to mirror damage onto. Each used to be its own `Option<usize>`
    /// field with its own pair of accessors and its own arm in
    /// `remove_condition`'s teardown match.
    ///
    /// Keying the table by condition is what makes the teardown
    /// structural: `remove_condition` drops `condition_links[&c]` for
    /// whatever `c` it removed, so a link *cannot* outlive its condition
    /// and a new linked condition can't forget to add itself to the
    /// teardown — there is nothing left to forget. The read accessor
    /// (`linked_by`) enforces the other direction by returning `None`
    /// unless the condition is actually held, which turns the
    /// `has_condition(X) && x_by() == Some(id)` idiom that every consumer
    /// wrote by hand into a single comparison that can't be half-written.
    ///
    /// `LINKED_CONDITIONS` in `engine::side_effects` is the install-side
    /// counterpart: the list of conditions whose install emits a
    /// `SetConditionLink` alongside the `ApplyCondition`.
    condition_links: HashMap<Condition, usize>,
    /// The lair's repertoire, copied off the template. Empty for
    /// everything that isn't the resident of somewhere.
    lair_actions: &'static [crate::engine::lair_actions::LairAction],
    /// Index into `lair_actions` of whatever the lair did last round, so
    /// the next round can avoid it — RAW: "the [creature] can't use the
    /// same lair action two rounds in a row." `None` before the first
    /// one fires.
    last_lair_action: Option<usize>,
    /// 5e Exhaustion, as its six cumulative tiers rather than a flag.
    ///
    /// The number and `Condition::Exhausted` are two views of one
    /// state, held in step by `add_condition` / `remove_condition`: the
    /// flag is present exactly when this is non-zero. That pairing is
    /// what let the tiers arrive without touching a single caller —
    /// every existing source already says `add_condition(Exhausted)`,
    /// which is now "gain a level" (RAW's own phrasing), and every
    /// existing cleanse already says `remove_condition(Exhausted)`,
    /// which is now "reduce by one level" (also RAW's own phrasing,
    /// and the thing Greater Restoration and a long rest both actually
    /// do). Immunity is unchanged: `add_condition` bounces first, so a
    /// creature immune to exhaustion never picks up a tier.
    ///
    /// Read through `exhaustion_level`. The tiers land at:
    /// `EXHAUSTION_CHECK_DISADVANTAGE_TIER` (checks),
    /// `EXHAUSTION_HALF_SPEED_TIER` (speed), `EXHAUSTION_ROLL_PENALTY_TIER`
    /// (attacks and saves), `EXHAUSTION_HALF_HP_TIER` (hit point
    /// maximum), `EXHAUSTION_ZERO_SPEED_TIER` (speed again), and
    /// `EXHAUSTION_DEATH_TIER`.
    exhaustion: u32,
    /// 5e Fighter Indomitable — one-shot "reroll the next failed save"
    /// marker. Set by the Indomitable action; consumed at the save
    /// site (`EncounterInstance::roll_save`) on a fail. Refreshed by
    /// long rest along with the feature pool.
    indomitable_pending: bool,
    /// 5e Legendary Resistance — remaining auto-pass charges on failed
    /// saves this long rest. Refreshed to `legendary_resistance_max` on
    /// long rest. See `EncounterInstance::roll_save` for the trigger site.
    legendary_resistance_remaining: u32,
    legendary_resistance_max: u32,
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves that deal half on pass,
    /// take 0 on pass and half on fail.
    has_evasion: bool,
    /// Whether this creature has the Mounted Combatant feat, copied from
    /// its template. See `CreatureTemplate::has_mounted_combatant`.
    has_mounted_combatant: bool,
    /// 5e Uncanny Dodge (Rogue 5): reaction to halve damage from one
    /// visible attack per round.
    has_uncanny_dodge: bool,
    /// 5e Monk Deflect Missiles (level 3): reaction to reduce ranged
    /// weapon damage by 1d10 + DEX + level.
    has_deflect_missiles: bool,
    /// 5e Fighter Battle Master Parry maneuver: feature-gated reaction to
    /// reduce melee damage by 1d8 + DEX modifier.
    has_parry: bool,
    /// 5e Fighter Battle Master Riposte maneuver: feature-gated reaction
    /// to make a melee weapon attack against an attacker who missed you.
    has_riposte: bool,
    has_displacement: bool,
    has_danger_sense: bool,
    has_pack_tactics: bool,
    sunlight_frailty: Option<SunlightFrailty>,
    is_swarm: bool,
    has_magic_resistance: bool,
    /// Recharge tracking: maps action name → (min_roll, is_available).
    /// At start-of-turn the engine rolls a d6 for each exhausted ability;
    /// if the roll >= min_roll the ability becomes available again.
    recharge_abilities: Vec<(&'static str, u32, bool)>,
    /// 5e Legendary Actions per round. See `CreatureTemplate` docs.
    legendary_actions_per_round: u32,
    /// 5e Extra Attack. See `CreatureTemplate` docs.
    has_extra_attack: bool,
    /// 5e Brutal Critical. See `CreatureTemplate` docs.
    brutal_critical_dice: u32,
    /// 5e Improved Critical: minimum d20 face that crits. See
    /// `CreatureTemplate` docs.
    crit_threshold: u32,
    /// 5e Lucky trait / feat. See `CreatureTemplate` docs.
    has_lucky: bool,
    /// 5e Halfling Brave trait. See `CreatureTemplate` docs.
    has_brave: bool,
    /// 5e Fey Ancestry trait (Elf / Half-Elf / Drow). See
    /// `CreatureTemplate` docs.
    has_fey_ancestry: bool,
    /// 5e Paladin Aura of Protection. See `CreatureTemplate` docs.
    has_aura_of_protection: bool,
    /// 5e Paladin Aura of Courage. See `CreatureTemplate` docs.
    has_aura_of_courage: bool,
    /// 5e Devotion Paladin Aura of Devotion. See `CreatureTemplate` docs.
    has_aura_of_devotion: bool,
    /// 5e Rogue Elusive (level 18). See `CreatureTemplate` docs.
    has_elusive: bool,
    /// 5e Paladin Nature's Ward (Ancients subclass level 15). See
    /// `CreatureTemplate` docs.
    has_natures_ward: bool,
    /// 5e Barbarian Feral Instinct (level 7). See `CreatureTemplate` docs.
    has_feral_instinct: bool,
    /// 5e Champion Fighter Remarkable Athlete (level 7). See
    /// `CreatureTemplate` docs.
    has_remarkable_athlete: bool,
    /// 5e Champion Fighter Superior Critical (subclass level 15). Caps
    /// `crit_threshold()` at 18 when set. See `CreatureTemplate` docs.
    has_superior_critical: bool,
    /// 5e Monk Diamond Soul (level 14). See `CreatureTemplate` docs.
    has_diamond_soul: bool,
    /// 5e Rogue Slippery Mind (level 15). See `CreatureTemplate` docs.
    has_slippery_mind: bool,
    /// 5e Zealot Barbarian Iron Mind (subclass level 7). See
    /// `CreatureTemplate` docs.
    has_iron_mind: bool,
    /// 5e Half-Orc Savage Attacks. See `CreatureTemplate` docs.
    has_savage_attacks: bool,
    /// 5e Fighting Style: Archery (+2 ranged weapon attack rolls). See
    /// `CreatureTemplate` docs.
    has_archery_style: bool,
    /// 5e Fighting Style: Defense (+1 AC). See `CreatureTemplate` docs.
    has_defense_style: bool,
    /// 5e Fighting Style: Dueling (+2 melee weapon damage). See
    /// `CreatureTemplate` docs.
    has_dueling_style: bool,
    /// 5e Fighting Style: Great Weapon Fighting (reroll 1s / 2s on melee
    /// weapon damage dice). See `CreatureTemplate` docs.
    has_great_weapon_fighting: bool,
    /// 5e Fighting Style: Two-Weapon Fighting (+STR mod to melee weapon
    /// damage). See `CreatureTemplate` docs.
    has_two_weapon_fighting_style: bool,
    /// 5e Fighting Style: Protection (reaction: impose disadvantage on
    /// attack against ally). See `CreatureTemplate` docs.
    has_protection_style: bool,
    /// 5e Fighting Style: Interception (reaction: reduce damage to
    /// adjacent ally by 1d10 + prof). See `CreatureTemplate` docs.
    has_interception_style: bool,
    /// 5e Ranger Feral Senses (level 18). Passive concealment-piercer
    /// with no range gate. See `CreatureTemplate` docs.
    has_feral_senses: bool,
    /// 5e Rogue Blindsense (level 14). Passive concealment-piercer
    /// with a 10-ft range gate. See `CreatureTemplate` docs.
    has_blindsense: bool,
    /// 5e Fighting Style: Blind Fighting (Tasha). Passive
    /// concealment-piercer with a 10-ft range gate, no hearing gate.
    /// See `CreatureTemplate` docs.
    has_blind_fighting_style: bool,
    /// 5e Oathbreaker Paladin Aura of Hate (level 7). Self-side +CHA
    /// mod (min +1) to melee weapon damage. See `CreatureTemplate`
    /// docs.
    has_aura_of_hate: bool,
    /// 5e Conquest Paladin Aura of Conquest (level 7). Hostile 10ft
    /// aura: Frightened enemies inside it are speed-0 and take psychic
    /// damage at the start of their turns. See `CreatureTemplate` docs.
    has_aura_of_conquest: bool,
    /// 5e Conquest Paladin Scornful Rebuke (level 15). Any attacker who
    /// connects takes CHA-mod psychic. See `CreatureTemplate` docs.
    has_scornful_rebuke: bool,
    /// 5e Ancients Paladin Aura of Warding (level 7). Passive 10ft
    /// aura granting spell-typical damage resistance to nearby allies.
    /// See `CreatureTemplate` docs.
    has_aura_of_warding: bool,
    /// 5e Barbarian Persistent Rage (level 15). Doubles the Rage
    /// condition timer. See `CreatureTemplate` docs.
    has_persistent_rage: bool,
    /// 5e Dwarven Resilience. See `CreatureTemplate` docs.
    has_dwarven_resilience: bool,
    /// 5e Warlock Fiend Patron Fiendish Resilience (level 10). Passive
    /// fire-damage resistance. See `CreatureTemplate` docs.
    has_fiendish_resilience: bool,
    /// 5e Sorcerer Draconic Bloodline Draconic Resilience (level 6).
    /// Passive fire-damage resistance (bloodline choice collapsed to
    /// Fire). See `CreatureTemplate` docs.
    has_draconic_resilience: bool,
    /// 5e Gnome Cunning. See `CreatureTemplate` docs.
    has_gnome_cunning: bool,
    /// 5e Swashbuckler Rogue Rakish Audacity (level 3). See
    /// `CreatureTemplate` docs.
    has_rakish_audacity: bool,
    /// 5e Swashbuckler Rogue Fancy Footwork (level 3). See
    /// `CreatureTemplate` docs.
    has_fancy_footwork: bool,
    /// 5e Gloom Stalker Ranger Dread Ambusher (level 3). See
    /// `CreatureTemplate` docs — passive +WIS-mod initiative bump on
    /// the shared `initiative_flat_bonus` lane.
    has_dread_ambusher: bool,
    /// 5e Swashbuckler Rogue Fancy Footwork ledger: targets this actor
    /// has made a melee attack against during their current turn. Read
    /// in `EncounterInstance::dispatch_opportunity_attacks` — a target
    /// on this list can't OA the swashbuckler as they move away.
    /// Cleared alongside the other per-turn HashSets in
    /// `reset_for_new_round`, so the "rest of your turn" clause snaps
    /// off the moment initiative advances past the swash.
    ///
    /// Written unconditionally at the top of every melee attack chokepoint
    /// (`engine::attack::resolve_attack`, gated on `p.is_melee`) so the
    /// ledger stays populated regardless of whether the holder actually
    /// carries the Fancy Footwork flag — the read-side check gates on the
    /// flag, keeping the write-side a single unconditional insert. A
    /// non-swashbuckler attacker's ledger just goes unread.
    melee_attack_targets_this_turn: HashSet<usize>,
    /// 5e two-weapon fighting ledger: has this actor swung a **light
    /// melee weapon** at Action cost during their current turn?
    ///
    /// RAW's clause is "when you take the Attack action and attack with
    /// a light melee weapon that you're holding in one hand", and this
    /// flag is that whole clause: the Action-cost half and the
    /// light-weapon half both have to be true for it to be set. Read by
    /// `OffHandAttack`'s gate, which is the only reader.
    ///
    /// A flag rather than a target set, unlike the ledger above it: the
    /// off-hand swing may go at anybody in reach, not only at whoever
    /// the main hand hit. Written at the stack's execution chokepoint
    /// in `process_stack` rather than inside the swing, because the
    /// fact being recorded is about the *action* — its cost and its
    /// weapon — and the attack resolver sees neither.
    ///
    /// Cleared at turn-start alongside the other per-turn ledgers, so a
    /// dual-wielder who saves their bonus action cannot spend it on
    /// last turn's main-hand swing.
    light_weapon_swing_this_turn: bool,
    /// 5e Dragonborn Draconic Ancestry damage type, if any. Drives the
    /// breath weapon's typing and the matching damage resistance.
    draconic_ancestry: Option<DamageType>,
    /// Remaining 5e Sorcery Points for Metamagic. Decremented when the
    /// caster spends a point on a metamagic prime; refreshed to
    /// `sorcery_points_max` on long rest.
    sorcery_points: u32,
    /// Long-rest cap on the sorcery-points pool. Copied from the template
    /// at instantiation; never mutated thereafter.
    sorcery_points_max: u32,
    /// 5e Death Burst — passive on-death trigger copied from the template.
    /// `None` for everything that doesn't explode (the default). Read at
    /// the death-cleanup chokepoint in `EncounterInstance` so the burst
    /// fires before the corpse is removed from the map.
    death_burst: Option<&'static crate::actions::monster_attacks::DeathBurst>,
    /// 5e natural melee reflect — passive "your touch hurts" trigger
    /// copied from the template (Black Pudding Corrosive Form, Salamander
    /// Heated Body). `None` for everything else. Read at the
    /// `resolve_attack_outcome` melee-reflect chokepoint so the rider
    /// fires alongside the condition-keyed reflect table.
    natural_melee_reflect: Option<crate::engine::attack::MeleeReflect>,
}

impl ActorInstance {
    pub fn from_creature_template(
        ct: &'static CreatureTemplate,
        location: Coordinate,
        team_id: usize,
        roller: &mut impl Roller,
        instance_n: usize,
    ) -> Result<ActorInstance, Box<dyn Error>> {
        // Floor the HP roll at 1 — a fresh spawn must be alive. The
        // hit-die expression on tiny CR-0 creatures (e.g. the Hawk's
        // RAW `1d4 - 1`) can roll to 0 on an unlucky d4, which would
        // park the actor in `is_combat_active == false` at
        // instantiation (HpState::Active && hitpoints > 0 fails on
        // hitpoints == 0). The floor matches `max_hitpoints()`'s
        // existing `.max(1)` guard, so the round-trip through
        // `max_hitpoints()` stays self-consistent — a creature whose
        // template's hit expression evaluates to 0 still spawns with
        // 1 HP, the way RAW intends ("a hawk's hit point maximum
        // can't be less than 1").
        let hp_roll_val: u32 = ct.hitpoints.eval(roller).max(1) as u32;
        let name: String = format!("{} {}", ct.name, instance_n);
        Ok(ActorInstance {
            name,
            location,
            movement_spent_this_turn: 0.0,
            // A creature that has not yet moved is not running.
            run_origin: location,
            run_step: None,
            team_id,
            base_ac: ct.ac,
            base_hitpoints: hp_roll_val,
            base_speed: ct.speed,
            base_size: ct.size,
            initiative: None,
            strength: ct.strength,
            intelligence: ct.intelligence,
            dexterity: ct.dexterity,
            wisdom: ct.wisdom,
            constitution: ct.constitution,
            charisma: ct.charisma,
            skills: ct.skills.clone(),
            items: ct.items.clone(),
            senses: ct.senses.clone(),
            languages: ct.languages.clone(),
            cr: ct.cr,
            hitpoints: hp_roll_val,
            movement: 0.0,
            action_slots: 0,
            bonus_action_slots: 0,
            reaction_slots: 0,
            legendary_action_slots: 0,
            size: ct.size,
            creature_type: ct.creature_type,
            spell_slot_manager: SpellSlotManager {
                ssi_by_lvl: ct
                    .spell_slots_by_level
                    .iter()
                    .map(|&n| SpellSlotInfo {
                        max_spell_slots: n,
                        spell_slots: n,
                    })
                    .collect(),
            },
            actions: ct.actions.clone(),
            glyph: ct.glyph,
            hp_state: HpState::Active,
            conditions: HashMap::new(),
            concentration: None,
            rolls_death_saves: ct.rolls_death_saves,
            damage_modifiers: ct.damage_modifiers.clone(),
            nonmagical_damage_modifiers: ct.nonmagical_damage_modifiers.clone(),
            temp_hp: 0,
            arcane_ward: 0,
            arcane_ward_formed: false,
            arcane_ward_base: ct.arcane_ward_base,
            portent_pool: Vec::new(),
            portent_forecast: false,
            portent_dice_max: ct.portent_dice,
            transmuters_stone: ct.transmuters_stone,
            overchannel_uses: 0,
            overchannel_backlash_pending: false,
            level: 1,
            xp: 0,
            proficient_saves: ct.proficient_saves.clone(),
            condition_immunities: ct.condition_immunities.clone(),
            charge: ct.charge,
            mountable: ct.mountable,
            mounted_on: None,
            ridden_by: None,
            features_remaining: feature_charge_map(&ct.features),
            features_max: feature_charge_map(&ct.features),
            attack_bonus_buff: 0,
            save_bonus_buff: 0,
            damage_bonus_buff: 0,
            once_per_turn_marks: HashSet::new(),
            hit_targets_this_turn: HashSet::new(),
            has_taken_turn_in_combat: false,
            relentless_rage_dc: 10,
            help_grants: HashMap::new(),
            regen_per_round: ct.regen_per_round,
            regen_suppressors: ct.regen_suppressors.clone(),
            regen_suppressed: false,
            mirror_images: 0,
            condition_links: HashMap::new(),
            lair_actions: ct.lair_actions,
            last_lair_action: None,
            exhaustion: 0,
            indomitable_pending: false,
            legendary_resistance_remaining: ct.legendary_resistances,
            legendary_resistance_max: ct.legendary_resistances,
            has_evasion: ct.has_evasion,
            has_mounted_combatant: ct.has_mounted_combatant,
            has_uncanny_dodge: ct.has_uncanny_dodge,
            has_deflect_missiles: ct.has_deflect_missiles,
            has_parry: ct.has_parry,
            has_riposte: ct.has_riposte,
            has_displacement: ct.has_displacement,
            has_danger_sense: ct.has_danger_sense,
            has_pack_tactics: ct.has_pack_tactics,
            sunlight_frailty: ct.sunlight_frailty,
            is_swarm: ct.is_swarm,
            has_magic_resistance: ct.has_magic_resistance,
            recharge_abilities: ct
                .recharge_abilities
                .iter()
                .map(|&(name, min_roll)| (name, min_roll, true))
                .collect(),
            legendary_actions_per_round: ct.legendary_actions_per_round,
            has_extra_attack: ct.has_extra_attack,
            brutal_critical_dice: ct.brutal_critical_dice,
            crit_threshold: ct.crit_threshold.max(1),
            has_lucky: ct.has_lucky,
            has_brave: ct.has_brave,
            has_fey_ancestry: ct.has_fey_ancestry,
            has_aura_of_protection: ct.has_aura_of_protection,
            has_aura_of_courage: ct.has_aura_of_courage,
            has_aura_of_devotion: ct.has_aura_of_devotion,
            has_elusive: ct.has_elusive,
            has_natures_ward: ct.has_natures_ward,
            has_feral_instinct: ct.has_feral_instinct,
            has_remarkable_athlete: ct.has_remarkable_athlete,
            has_superior_critical: ct.has_superior_critical,
            has_diamond_soul: ct.has_diamond_soul,
            has_slippery_mind: ct.has_slippery_mind,
            has_iron_mind: ct.has_iron_mind,
            has_savage_attacks: ct.has_savage_attacks,
            has_archery_style: ct.has_archery_style,
            has_defense_style: ct.has_defense_style,
            has_dueling_style: ct.has_dueling_style,
            has_great_weapon_fighting: ct.has_great_weapon_fighting,
            has_two_weapon_fighting_style: ct.has_two_weapon_fighting_style,
            has_protection_style: ct.has_protection_style,
            has_interception_style: ct.has_interception_style,
            has_feral_senses: ct.has_feral_senses,
            has_blindsense: ct.has_blindsense,
            has_blind_fighting_style: ct.has_blind_fighting_style,
            has_aura_of_hate: ct.has_aura_of_hate,
            has_aura_of_conquest: ct.has_aura_of_conquest,
            has_scornful_rebuke: ct.has_scornful_rebuke,
            has_aura_of_warding: ct.has_aura_of_warding,
            has_persistent_rage: ct.has_persistent_rage,
            has_dwarven_resilience: ct.has_dwarven_resilience,
            has_fiendish_resilience: ct.has_fiendish_resilience,
            has_draconic_resilience: ct.has_draconic_resilience,
            has_gnome_cunning: ct.has_gnome_cunning,
            has_rakish_audacity: ct.has_rakish_audacity,
            has_fancy_footwork: ct.has_fancy_footwork,
            has_dread_ambusher: ct.has_dread_ambusher,
            melee_attack_targets_this_turn: HashSet::new(),
            light_weapon_swing_this_turn: false,
            draconic_ancestry: ct.draconic_ancestry,
            sorcery_points: ct.sorcery_points,
            sorcery_points_max: ct.sorcery_points,
            death_burst: ct.death_burst,
            natural_melee_reflect: ct.natural_melee_reflect,
        })
    }

    /// On-death burst this actor fires when reduced to 0 HP, if any. `None`
    /// for the vast majority of creatures; mephits / magmins / similar
    /// templates set this to a `DeathBurst` static. Read by
    /// `EncounterInstance::cleanup_dead_actors` at the death chokepoint.
    pub fn death_burst(
        &self,
    ) -> Option<&'static crate::actions::monster_attacks::DeathBurst> {
        self.death_burst
    }

    /// Natural melee reflect (Black Pudding Corrosive Form, Salamander
    /// Heated Body, etc.) — a creature-intrinsic retaliation against
    /// any melee swing that connects. Returns the rider description by
    /// value (it's `Copy`); `None` for the vast majority of creatures.
    /// Read at `resolve_attack_outcome` next to the condition-keyed
    /// `MELEE_REFLECT_RIDERS` table.
    pub fn natural_melee_reflect(&self) -> Option<crate::engine::attack::MeleeReflect> {
        self.natural_melee_reflect
    }

    /// Remaining Mirror Image decoys (5e spell). Zero = no decoys; the
    /// MirroredImages condition should be off in that state.
    pub fn mirror_images(&self) -> u32 {
        self.mirror_images
    }

    /// Grant `n` Mirror Image decoys. Overwrites any prior pool (5e: re-
    /// casting the spell creates a fresh set). Caller is responsible for
    /// applying the MirroredImages condition.
    pub fn set_mirror_images(&mut self, n: u32) {
        self.mirror_images = n;
    }

    /// Pop one Mirror Image decoy. Returns true if a decoy was consumed
    /// (caller treats the attack as a miss). When the pool hits zero the
    /// MirroredImages condition is cleared so the holder loses the
    /// disadvantage-on-attacks rider.
    pub fn pop_mirror_image(&mut self) -> bool {
        if self.mirror_images == 0 {
            return false;
        }
        self.mirror_images -= 1;
        if self.mirror_images == 0 {
            self.conditions.remove(&Condition::MirroredImages);
        }
        true
    }

    /// The actor that applied `c` to this actor, or `None` if `c` isn't
    /// currently held or carries no back-link.
    ///
    /// This is the only read path onto `condition_links`, and the
    /// `has_condition` guard is why. Every consumer of a back-link wants
    /// "is this creature X-ed *by that actor*" — a bare link read would
    /// answer "yes" for a stale id whose condition had already lifted,
    /// which is a bug the caller has no way to see. Folding the flag
    /// check in means `linked_by(Sworn) == Some(paladin)` is the whole
    /// question, and the seven consumers that used to spell out
    /// `has_condition(Sworn) && sworn_by() == Some(paladin)` can no
    /// longer write half of it.
    ///
    /// The pairing also keeps the two halves honest in the other
    /// direction: `remove_condition` drops the entry, so a link can
    /// never outlive its condition even if a future caller forgets to
    /// clear it explicitly.
    pub fn linked_by(&self, c: Condition) -> Option<usize> {
        if !self.has_condition(c) {
            return None;
        }
        self.condition_links.get(&c).copied()
    }

    /// Point `c`'s back-link at `source` (or clear it with `None`).
    ///
    /// Callers normally reach this through the `SetConditionLink` side
    /// effect rather than directly, so that the link install travels
    /// with the `ApplyCondition` that grants the flag — see
    /// `engine::side_effects::install_condition_with_link`.
    pub fn set_condition_link(&mut self, c: Condition, source: Option<usize>) {
        match source {
            Some(id) => {
                self.condition_links.insert(c, id);
            }
            None => {
                self.condition_links.remove(&c);
            }
        }
    }

    pub fn has_evasion(&self) -> bool {
        self.has_evasion
    }

    /// Whether this creature fights from the saddle the way the feat
    /// describes. See `CreatureTemplate::has_mounted_combatant`.
    pub fn has_mounted_combatant(&self) -> bool {
        self.has_mounted_combatant
    }

    pub fn has_uncanny_dodge(&self) -> bool {
        self.has_uncanny_dodge
    }

    pub fn has_deflect_missiles(&self) -> bool {
        self.has_deflect_missiles
    }

    /// 5e Fighter Battle Master Parry maneuver: passive flag that
    /// combined with an available `PARRY_TAG` feature charge and the
    /// target's reaction fires a `1d8 + DEX modifier` damage reducer on
    /// a melee hit. Read at `resolve_attack_outcome` alongside
    /// Uncanny Dodge / Deflect Missiles.
    pub fn has_parry(&self) -> bool {
        self.has_parry
    }

    /// 5e Fighter Battle Master Riposte maneuver: passive flag that
    /// combined with an available `RIPOSTE_TAG` feature charge and the
    /// target's reaction fires a follow-up melee weapon attack against
    /// the missed attacker. Read at `resolve_attack_outcome` in the miss
    /// branch, right after the miss log line lands.
    pub fn has_riposte(&self) -> bool {
        self.has_riposte
    }

    pub fn has_displacement(&self) -> bool {
        self.has_displacement
    }

    pub fn has_danger_sense(&self) -> bool {
        self.has_danger_sense
    }

    pub fn has_pack_tactics(&self) -> bool {
        self.has_pack_tactics
    }

    /// True if this actor is a 5e **swarm** — see the template field for
    /// the two clauses the flag carries and the ones it doesn't.
    pub fn is_swarm(&self) -> bool {
        self.is_swarm
    }

    /// True if a heal aimed at this actor would do anything at all.
    ///
    /// The single gate `heal` consults, so every source of healing in
    /// the engine — Cure Wounds, a potion, a Life Cleric's aura, a
    /// Paladin's Lay on Hands, Aura of Vitality's per-round tick —
    /// respects a no-heal rule without any of them knowing it exists.
    /// Also read by the AI's support rung, which will not offer a heal
    /// to an ally that cannot take one.
    ///
    /// Two sources say no:
    ///
    ///   - **Swarm** — "the swarm can't regain hit points or gain
    ///     temporary hit points". Permanent, and it also covers temp HP
    ///     (see `gain_temp_hp`).
    ///   - **Chill Touch** (`ChillTouched`) — "the target can't regain
    ///     hit points until the start of your next turn". A round long,
    ///     and hit points only.
    ///
    /// Deliberately *not* a bar on being revived from Dying: RAW's
    /// no-heal clauses stop the HP going up, and the `Dead` /
    /// `HpState` arms in `heal` already own the question of who can be
    /// brought back at all.
    pub fn can_regain_hitpoints(&self) -> bool {
        !self.is_swarm && !self.has_condition(Condition::ChillTouched)
    }

    /// True while a swarm has been thinned to half its hit points or
    /// fewer — the gate on the halved-bite clause every swarm statblock
    /// writes into its attack line.
    ///
    /// False for anything that isn't a swarm, at any HP: this is the
    /// swarm's own "there are fewer of me now" rule and not a general
    /// bloodied threshold, so a wounded ogre hits exactly as hard as a
    /// fresh one.
    ///
    /// Reads current HP against max, so a swarm healed back over the
    /// line would bite in full again — which no swarm can be, since
    /// `heal` refuses them, but the predicate stays honest about what it
    /// measures rather than latching.
    pub fn is_thinned_swarm(&self) -> bool {
        self.is_swarm && self.hitpoints * 2 <= self.max_hitpoints()
    }

    pub fn has_magic_resistance(&self) -> bool {
        self.has_magic_resistance
    }

    /// True if this actor emits the Paladin's Aura of Protection
    /// (level 6+). Read by `EncounterInstance::aura_of_protection_bonus`
    /// to fold the aura's CHA bonus into every nearby ally's save total.
    pub fn has_aura_of_protection(&self) -> bool {
        self.has_aura_of_protection
    }

    /// 5e Halfling Brave — advantage on saves vs Frightened, approximated
    /// as full immunity to the Frightened condition install. Read by
    /// `dynamic_immunity_to` so the chokepoint in `add_condition` catches
    /// it alongside Heroism / MindBlank.
    pub fn has_brave(&self) -> bool {
        self.has_brave
    }

    /// 5e Fey Ancestry (Elf / Half-Elf / Drow) — advantage on saves vs
    /// Charmed and immune to magical Sleep. Read by `dynamic_immunity_to`
    /// for both the Charmed (over-tuned approximation) and Asleep (RAW
    /// match — Asleep is only installed by magical sources here) install
    /// chokepoints.
    pub fn has_fey_ancestry(&self) -> bool {
        self.has_fey_ancestry
    }

    /// True if this actor emits the Paladin's Aura of Courage (level 10+).
    /// Read by `EncounterInstance::is_in_aura_of_courage` so the Frightened
    /// apply path can suppress installs on allies inside the bubble.
    pub fn has_aura_of_courage(&self) -> bool {
        self.has_aura_of_courage
    }

    /// True if this actor emits the Devotion Paladin's Aura of Devotion
    /// (Devotion subclass level 7+). Read by
    /// `EncounterInstance::is_in_aura_of_devotion` so the Charmed apply
    /// path can suppress installs on allies inside the bubble.
    pub fn has_aura_of_devotion(&self) -> bool {
        self.has_aura_of_devotion
    }

    /// 5e Rogue Elusive (level 18): no attack roll has advantage against
    /// the holder while they aren't Incapacitated. Read by the
    /// post-processing clause in `EncounterInstance::compute_attack_mode`.
    pub fn has_elusive(&self) -> bool {
        self.has_elusive
    }

    /// 5e Paladin Nature's Ward (Ancients subclass lv15): immunity to
    /// Charmed AND Frightened installs. Read by `dynamic_immunity_to`
    /// via the `FLAG_DRIVEN_IMMUNITIES` table.
    pub fn has_natures_ward(&self) -> bool {
        self.has_natures_ward
    }

    /// 5e Barbarian Feral Instinct (level 7): advantage on initiative
    /// rolls. Read by `roll_initiative` — the d20 is rolled twice and
    /// the higher result is kept.
    pub fn has_feral_instinct(&self) -> bool {
        self.has_feral_instinct
    }

    /// 5e Champion Fighter Remarkable Athlete (level 7): flat
    /// `ceil(proficiency_bonus / 2)` bump on initiative rolls (the sole
    /// STR/DEX/CON check with a combat surface in this engine). Read
    /// by `roll_initiative` next to `has_feral_instinct` — the two
    /// composers land on the same roll (feral instinct's advantage
    /// picks the higher d20, remarkable athlete's flat bump is added
    /// on top of whichever d20 wins).
    pub fn has_remarkable_athlete(&self) -> bool {
        self.has_remarkable_athlete
    }

    /// 5e Monk Diamond Soul (level 14): proficiency in every saving
    /// throw. Read by `is_save_proficient` to short-circuit the explicit
    /// `proficient_saves` lookup.
    pub fn has_diamond_soul(&self) -> bool {
        self.has_diamond_soul
    }

    /// 5e Rogue Slippery Mind (level 15): proficiency in Wisdom saves.
    /// Read by `is_save_proficient` — a narrower version of Diamond Soul
    /// (single ability, not all six).
    pub fn has_slippery_mind(&self) -> bool {
        self.has_slippery_mind
    }

    /// 5e Zealot Barbarian Iron Mind (subclass level 7): proficiency in
    /// Wisdom saves. Read by `is_save_proficient` via the shared
    /// `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort alongside `has_slippery_mind`
    /// — mechanically identical single-ability proficiency grant, kept as
    /// a distinct flag so the class origin isn't conflated.
    pub fn has_iron_mind(&self) -> bool {
        self.has_iron_mind
    }

    /// 5e Half-Orc Savage Attacks — adds one extra weapon damage die on a
    /// critical melee hit. Read at the crit-damage site alongside
    /// `brutal_critical_dice`; the two stack additively on a half-orc
    /// barbarian.
    pub fn has_savage_attacks(&self) -> bool {
        self.has_savage_attacks
    }

    /// 5e Fighting Style: Archery — +2 to attack rolls with ranged
    /// weapons. Read at `resolve_attack` gated on `!is_melee && !is_spell`.
    pub fn has_archery_style(&self) -> bool {
        self.has_archery_style
    }

    /// 5e Fighting Style: Defense — +1 AC. Read at
    /// `ActorInstance::armor_class`.
    pub fn has_defense_style(&self) -> bool {
        self.has_defense_style
    }

    /// 5e Fighting Style: Dueling — +2 to melee weapon damage rolls
    /// (RAW gated on wielding a one-handed weapon; the engine's
    /// weapon-hand-usage collapse turns the gate into "melee weapon
    /// attack only"). Read at the damage-roll site in `engine::attack`.
    pub fn has_dueling_style(&self) -> bool {
        self.has_dueling_style
    }

    /// Test-only setter for the Dueling Fighting Style flag. Lets tests
    /// dial the flag on or off on any chassis so the +2 damage rider
    /// can be exercised in isolation. Mirrors `set_savage_attacks` /
    /// `set_dwarven_resilience` etc. on the racial-flag lane.
    #[cfg(test)]
    pub fn set_dueling_style(&mut self, value: bool) {
        self.has_dueling_style = value;
    }

    /// 5e Fighting Style: Great Weapon Fighting — reroll 1 / 2 on a
    /// melee weapon damage die once. Read at the damage-roll site in
    /// `engine::attack` via `roll_weapon_damage_dice`.
    pub fn has_great_weapon_fighting(&self) -> bool {
        self.has_great_weapon_fighting
    }

    /// Test-only setter for the Great Weapon Fighting flag. Mirrors
    /// `set_dueling_style` so the per-die reroll rider can be exercised
    /// in isolation on any chassis.
    #[cfg(test)]
    pub fn set_great_weapon_fighting(&mut self, value: bool) {
        self.has_great_weapon_fighting = value;
    }

    /// 5e Fighting Style: Two-Weapon Fighting — +STR mod (min 0) to
    /// melee weapon damage rolls. Approximates the RAW "add ability
    /// modifier to the off-hand attack" clause by folding it into
    /// every melee swing (the engine doesn't distinguish off-hand
    /// swings at the action-list level). Read at the damage-roll site
    /// in `engine::attack`, gated on `p.is_melee`.
    pub fn has_two_weapon_fighting_style(&self) -> bool {
        self.has_two_weapon_fighting_style
    }

    /// Test-only setter for the Two-Weapon Fighting flag.
    #[cfg(test)]
    pub fn set_two_weapon_fighting_style(&mut self, value: bool) {
        self.has_two_weapon_fighting_style = value;
    }

    /// 5e Fighting Style: Protection — reaction: impose disadvantage
    /// on an attack roll against an ally adjacent to the holder. Read
    /// at `compute_attack_mode`.
    pub fn has_protection_style(&self) -> bool {
        self.has_protection_style
    }

    /// Test-only setter for the Protection Fighting Style flag.
    #[cfg(test)]
    pub fn set_protection_style(&mut self, value: bool) {
        self.has_protection_style = value;
    }

    /// 5e Fighting Style: Interception — reaction: reduce the damage of
    /// an incoming attack against an adjacent ally by 1d10 + proficiency
    /// bonus. Read at `resolve_attack_outcome` / `spell_attack_outcome`
    /// AFTER the damage is computed but BEFORE it's applied.
    pub fn has_interception_style(&self) -> bool {
        self.has_interception_style
    }

    /// Test-only setter for the Interception Fighting Style flag.
    #[cfg(test)]
    pub fn set_interception_style(&mut self, value: bool) {
        self.has_interception_style = value;
    }

    /// 5e Ranger **Feral Senses** (lv18 capstone): the holder pierces
    /// illusion-style concealment at any range — sibling to Truesight
    /// on the concealment-suppression cohort. Wired through
    /// `EncounterInstance::pierces_illusion_of` at the
    /// `compute_attack_mode` chokepoint.
    pub fn has_feral_senses(&self) -> bool {
        self.has_feral_senses
    }

    /// Test-only setter for the Feral Senses flag. Mirrors
    /// `set_interception_style` so tests can dial it on without
    /// needing the RANGER_TEMPLATE chassis.
    #[cfg(test)]
    pub fn set_feral_senses(&mut self, value: bool) {
        self.has_feral_senses = value;
    }

    /// 5e Rogue **Blindsense** (lv14): the holder pierces illusion-style
    /// concealment against subjects within 10 ft, provided the holder
    /// isn't Deafened. Wired through
    /// `EncounterInstance::pierces_illusion_of` — the 10-ft envelope
    /// and the deafened gate both live in the encounter helper so the
    /// accessor stays a flat boolean.
    pub fn has_blindsense(&self) -> bool {
        self.has_blindsense
    }

    /// Test-only setter for the Blindsense flag. Mirrors
    /// `set_feral_senses` for the same reason.
    #[cfg(test)]
    pub fn set_blindsense(&mut self, value: bool) {
        self.has_blindsense = value;
    }

    /// 5e Fighting Style: **Blind Fighting** (Tasha): the holder
    /// pierces illusion-style concealment against subjects within 10
    /// ft. Unlike Blindsense, no hearing gate — Deafened doesn't
    /// suppress the flag (RAW: "even if you're blinded or in
    /// darkness"). Wired through
    /// `EncounterInstance::pierces_illusion_of` — the 10-ft envelope
    /// lives in the encounter helper so the accessor stays a flat
    /// boolean.
    pub fn has_blind_fighting_style(&self) -> bool {
        self.has_blind_fighting_style
    }

    /// Test-only setter for the Blind Fighting Fighting Style flag.
    /// Mirrors `set_blindsense` for the same reason.
    #[cfg(test)]
    pub fn set_blind_fighting_style(&mut self, value: bool) {
        self.has_blind_fighting_style = value;
    }

    /// 5e Oathbreaker Paladin **Aura of Hate** (level 7): +CHA mod
    /// (min +1) to melee weapon damage. Read at the caster-side melee
    /// bumps table in `engine::attack::resolve_attack_outcome` next
    /// to Rage / Dueling / Two-Weapon Fighting.
    pub fn has_aura_of_hate(&self) -> bool {
        self.has_aura_of_hate
    }

    /// Test-only setter for the Aura of Hate flag. Mirrors
    /// `set_dueling_style` — lets tests dial the flag on any chassis
    /// so the +CHA melee bump rider can be exercised in isolation.
    #[cfg(test)]
    pub fn set_aura_of_hate(&mut self, value: bool) {
        self.has_aura_of_hate = value;
    }

    /// 5e Conquest Paladin **Aura of Conquest** (level 7): the emitter
    /// side of the paladin's one hostile aura. Read by
    /// `EncounterInstance::in_hostile_aura_of_conquest`, which walks
    /// the emitters from a victim's point of view.
    pub fn has_aura_of_conquest(&self) -> bool {
        self.has_aura_of_conquest
    }

    /// 5e Conquest Paladin **Scornful Rebuke** (level 15): CHA-mod
    /// psychic back at anything that hits the holder. Read as a row on
    /// the `ANY_ATTACK_REFLECT_FEATURES` lane in `engine::attack`.
    pub fn has_scornful_rebuke(&self) -> bool {
        self.has_scornful_rebuke
    }

    /// 5e Ancients Paladin **Aura of Warding** (level 7): the paladin
    /// emits a 10-ft aura that grants resistance to spell-typical
    /// damage to nearby allies. Read by
    /// `EncounterInstance::is_in_aura_of_warding` — this accessor
    /// exposes the emitter side; the ally-side lookup walks all
    /// emitters and picks the first one whose footprint sits in
    /// range.
    pub fn has_aura_of_warding(&self) -> bool {
        self.has_aura_of_warding
    }

    /// Test-only setter for the Aura of Warding flag. Mirrors
    /// `set_aura_of_hate` — lets tests dial the aura onto any chassis
    /// so the spell-damage halving envelope can be exercised in
    /// isolation.
    #[cfg(test)]
    pub fn set_aura_of_warding(&mut self, value: bool) {
        self.has_aura_of_warding = value;
    }

    /// 5e Barbarian **Persistent Rage** (level 15): passive that
    /// keeps the Rage installed longer. Read by `RAGE`'s
    /// `side_effects` — swaps the install timer from `Rounds(10)` to
    /// `Rounds(20)` when the flag is set.
    pub fn has_persistent_rage(&self) -> bool {
        self.has_persistent_rage
    }

    /// Test-only setter for the Persistent Rage flag. Mirrors
    /// `set_aura_of_hate` on the racial / class flag lane.
    #[cfg(test)]
    pub fn set_persistent_rage(&mut self, value: bool) {
        self.has_persistent_rage = value;
    }

    /// Test-only helper: install a class-feature tag on both
    /// `features_max` (so long-rest refills work) and
    /// `features_remaining` (so it fires immediately). Used by
    /// multiclass-shape fixtures that need to layer a feature on top
    /// of a template that doesn't natively carry it — e.g. dialing
    /// Half-Orc Relentless Endurance onto an Ancients Paladin to
    /// verify the `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort's
    /// order-of-consumption. Non-test callers should always route
    /// through the template's `features` HashSet at creation time.
    #[cfg(test)]
    pub fn grant_feature_for_test(&mut self, tag: &'static str) {
        let charges = crate::actions::class_features::feature_charges(tag);
        self.features_max.insert(tag, charges);
        self.features_remaining.insert(tag, charges);
    }

    /// 5e Dwarven Resilience — advantage on saves vs poison AND resistance
    /// to poison damage. Read by `compute_save_mode` (advantage clause) and
    /// `effective_damage` (resistance clause).
    pub fn has_dwarven_resilience(&self) -> bool {
        self.has_dwarven_resilience
    }

    /// 5e Warlock Fiend Patron **Fiendish Resilience** (level 10) —
    /// passive fire-damage resistance. Read by `effective_damage`
    /// (resistance clause via `PASSIVE_TYPED_RESISTANCES`). RAW's rest-
    /// cycle choice of damage type is collapsed to a fixed Fire lock
    /// on this flag — see `CreatureTemplate::has_fiendish_resilience`
    /// for the rationale.
    pub fn has_fiendish_resilience(&self) -> bool {
        self.has_fiendish_resilience
    }

    /// 5e Sorcerer Draconic Bloodline **Draconic Resilience** (level 6)
    /// — passive fire-damage resistance. Read by `effective_damage`
    /// (resistance clause via `PASSIVE_TYPED_RESISTANCES`). RAW's
    /// bloodline choice of damage type is collapsed to a fixed Fire
    /// lock on this flag — see `CreatureTemplate::has_draconic_resilience`
    /// for the rationale. Sibling accessor to
    /// `has_fiendish_resilience` on the passive-fire-resistance lane;
    /// each flag is a distinct class-source pick so a hypothetical
    /// Fiend Warlock / Draconic Sorcerer multiclass carries both flags
    /// cleanly.
    pub fn has_draconic_resilience(&self) -> bool {
        self.has_draconic_resilience
    }

    /// 5e Gnome Cunning — advantage on INT / WIS / CHA saves vs magic.
    /// Approximated as advantage on every INT / WIS / CHA save (saves
    /// rarely originate from non-magical sources in this engine).
    pub fn has_gnome_cunning(&self) -> bool {
        self.has_gnome_cunning
    }

    /// 5e Swashbuckler Rogue Rakish Audacity (level 3) — passive
    /// two-part class feature: (a) +CHA-mod to initiative, and (b) a
    /// second Sneak Attack qualification path that fires when the swash
    /// stands alone with the target. Read by `initiative_flat_bonus`
    /// and `class_attacks::sneak_attack_eligible`.
    pub fn has_rakish_audacity(&self) -> bool {
        self.has_rakish_audacity
    }

    /// 5e Swashbuckler Rogue Fancy Footwork (level 3) — passive class
    /// feature: melee attack targets can't OA the swashbuckler for the
    /// rest of the turn. Read by
    /// `EncounterInstance::dispatch_opportunity_attacks`.
    pub fn has_fancy_footwork(&self) -> bool {
        self.has_fancy_footwork
    }

    /// 5e Gloom Stalker Ranger Dread Ambusher (level 3) — passive
    /// class feature: +WIS-mod to initiative rolls. Read by
    /// `initiative_flat_bonus` alongside Rakish Audacity's CHA-mod
    /// bump and Remarkable Athlete's `+ceil(prof / 2)`.
    pub fn has_dread_ambusher(&self) -> bool {
        self.has_dread_ambusher
    }

    /// Test-only setter for the Dread Ambusher flag. Mirrors
    /// `set_blindsense` / `set_blind_fighting_style` — enables the
    /// `ability_mod_initiative_bonuses_cohort_sums_multiple_hits` test
    /// to toggle the flag on a Swashbuckler baseline (which already
    /// ships Rakish Audacity) to prove the cohort sums both bumps
    /// rather than short-circuiting. No non-test callsite; a real
    /// runtime setup would install the flag at template-instantiation
    /// time via `has_dread_ambusher: true` on the template.
    #[cfg(test)]
    pub fn set_dread_ambusher(&mut self, value: bool) {
        self.has_dread_ambusher = value;
    }

    /// 5e Swashbuckler Rogue Fancy Footwork ledger read: has this actor
    /// made a melee attack against `target_id` during their current
    /// turn? Called from `dispatch_opportunity_attacks` when the mover
    /// holds `has_fancy_footwork`; a `true` result skips the reactor's
    /// OA silently.
    pub fn has_melee_attacked_this_turn(&self, target_id: usize) -> bool {
        self.melee_attack_targets_this_turn.contains(&target_id)
    }

    /// Mark `target_id` on the melee-attack ledger. Called from the top
    /// of `engine::attack::resolve_attack` on every melee swing
    /// (`p.is_melee == true`), unconditionally — the Fancy Footwork
    /// read-side gates on the flag, so a non-swashbuckler attacker's
    /// ledger just goes unread. Keeping the write unconditional avoids
    /// a per-swing flag lookup on the attack hot path.
    pub fn mark_melee_attacked_this_turn(&mut self, target_id: usize) {
        self.melee_attack_targets_this_turn.insert(target_id);
    }

    /// Snapshot the melee-attack ledger as an owned `HashSet` so the
    /// caller can drop the immutable borrow on `&self` before the
    /// engine loop mutates the actor map. Used by
    /// `dispatch_opportunity_attacks` — the OA candidate-scan loop
    /// mutates `self.actors`, so any borrow into the mover has to
    /// materialize into a plain-data snapshot up-front. Callers that
    /// only need to check one id at a time should prefer
    /// `has_melee_attacked_this_turn` instead.
    pub fn melee_attack_targets_this_turn_snapshot(&self) -> HashSet<usize> {
        self.melee_attack_targets_this_turn.clone()
    }

    /// 5e two-weapon fighting ledger read: has this actor spent their
    /// Action on a light melee weapon this turn? `OffHandAttack`'s
    /// gate, and the only reader of the flag.
    pub fn has_swung_light_weapon_this_turn(&self) -> bool {
        self.light_weapon_swing_this_turn
    }

    /// Stamp the two-weapon fighting ledger. Called from
    /// `EncounterInstance::process_stack` for every resolved action
    /// that answers `true` to both `Action::is_light_melee_weapon` and
    /// an Action-cost check — see `light_weapon_swing_this_turn` for
    /// why the write lives there rather than inside the swing.
    pub fn mark_light_weapon_swing_this_turn(&mut self) {
        self.light_weapon_swing_this_turn = true;
    }

    /// 5e Dragonborn Draconic Ancestry — damage type of the breath weapon
    /// and the matching template-resistance lane. `None` for non-dragonborn.
    pub fn draconic_ancestry(&self) -> Option<DamageType> {
        self.draconic_ancestry
    }

    /// Test-only setter for the Savage Attacks flag. Lets tests dial it
    /// on without needing a Half-Orc template — mirrors
    /// `set_brutal_critical_dice` so the crit-damage lane can be
    /// exercised on any chassis.
    #[cfg(test)]
    pub fn set_savage_attacks(&mut self, value: bool) {
        self.has_savage_attacks = value;
    }

    /// Test-only setter for the base armor class. The item / condition
    /// / style lanes still stack on top through `armor_class`, so this
    /// moves the floor rather than pinning the total.
    ///
    /// For tests that need a swing to certainly hit or certainly miss
    /// regardless of the die — the bestiary tops out around AC 22, which
    /// is not far enough above a competent attacker's bonus to make
    /// "misses by more than a d8 could cover" a fact rather than a
    /// tendency.
    #[cfg(test)]
    pub fn set_base_ac(&mut self, value: u32) {
        self.base_ac = value;
    }

    /// Test-only setter for the Dwarven Resilience flag.
    #[cfg(test)]
    pub fn set_dwarven_resilience(&mut self, value: bool) {
        self.has_dwarven_resilience = value;
    }

    /// Test-only setter for the Gnome Cunning flag.
    #[cfg(test)]
    pub fn set_gnome_cunning(&mut self, value: bool) {
        self.has_gnome_cunning = value;
    }

    /// 5e Sorcery Points remaining (Sorcerer Metamagic pool). 0 for
    /// non-sorcerers. Read by metamagic action validators to gate
    /// activation; spent via `spend_sorcery_point`.
    pub fn sorcery_points(&self) -> u32 {
        self.sorcery_points
    }

    /// Long-rest cap on the sorcery-points pool. Surfaced for UI /
    /// debugging — gameplay reads `sorcery_points` for affordability
    /// checks and `restore_sorcery_points` for long-rest refill.
    pub fn sorcery_points_max(&self) -> u32 {
        self.sorcery_points_max
    }

    /// Spend one sorcery point. Returns true on success, false if the
    /// pool is empty. Used by Metamagic prime actions (Empowered Spell,
    /// future Quickened Spell, Twinned Spell, etc.).
    pub fn spend_sorcery_point(&mut self) -> bool {
        if self.sorcery_points == 0 {
            return false;
        }
        self.sorcery_points -= 1;
        true
    }

    /// Spend `n` sorcery points atomically — either all `n` points come
    /// out of the pool or none do. Returns true on success. Used by
    /// multi-point metamagic (Quickened: 2, Heightened: 3, future
    /// Twinned: slot-level) so the cost lives in one debit rather than
    /// a loop at every call site that could be interrupted mid-spend.
    pub fn spend_sorcery_points(&mut self, n: u32) -> bool {
        if self.sorcery_points < n {
            return false;
        }
        self.sorcery_points -= n;
        true
    }

    /// Restore the sorcery-points pool to the long-rest cap. Called from
    /// `long_rest` alongside spell slot / feature refresh.
    pub fn restore_sorcery_points(&mut self) {
        self.sorcery_points = self.sorcery_points_max;
    }

    /// Grant `n` sorcery points to the pool, saturating at the long-rest
    /// cap. Used by Font of Magic's "convert spell slot to SP" lane:
    /// RAW "the slot value is added to your sorcery points, up to your
    /// maximum" — surplus is silently dropped. Returns the actual delta
    /// applied (useful for tests / logs that want the consumed amount).
    pub fn give_sorcery_points(&mut self, n: u32) -> u32 {
        let cap = self.sorcery_points_max;
        let prev = self.sorcery_points;
        self.sorcery_points = (prev + n).min(cap);
        self.sorcery_points - prev
    }

    /// True if any Sorcerer metamagic prime is currently up on this actor.
    /// Reads `Condition::is_metamagic_prime` so the cohort lives in one
    /// place and AI gates (and the Font of Magic "don't shuffle resources
    /// mid-prime" check) don't have to list each prime by name.
    pub fn has_any_metamagic_prime(&self) -> bool {
        self.conditions.keys().any(|c| c.is_metamagic_prime())
    }

    /// True if any 2024 Rogue Cunning Strike prime (`Poison` / `Trip` /
    /// `Withdraw` / `Daze`) is currently up on this actor. Mirrors
    /// `has_any_metamagic_prime`'s shape — reads
    /// `Condition::is_cunning_strike_prime` so the cohort lives in one
    /// place. Used by the bonus-action validators, the shortsword
    /// consume site, and the AI's "don't double-prime" gate.
    pub fn has_any_cunning_strike_prime(&self) -> bool {
        self.conditions.keys().any(|c| c.is_cunning_strike_prime())
    }

    pub fn legendary_actions_per_round(&self) -> u32 {
        self.legendary_actions_per_round
    }

    /// The lair's repertoire. Empty for a creature with no lair, which
    /// is how `dispatch_lair_actions` tells the two apart.
    pub fn lair_actions(&self) -> &'static [crate::engine::lair_actions::LairAction] {
        self.lair_actions
    }

    /// Index of the lair action taken last round, if any. Read by the
    /// dispatcher to honor RAW's "not the same one two rounds running".
    pub fn last_lair_action(&self) -> Option<usize> {
        self.last_lair_action
    }

    /// Record which lair action just fired.
    pub fn set_last_lair_action(&mut self, index: usize) {
        self.last_lair_action = Some(index);
    }

    pub fn has_extra_attack(&self) -> bool {
        self.has_extra_attack
    }

    /// Number of bonus damage dice the actor adds to a critical melee
    /// hit (5e Barbarian Brutal Critical). 0 = no rider.
    pub fn brutal_critical_dice(&self) -> u32 {
        self.brutal_critical_dice
    }

    /// Minimum d20 face that promotes the swing to a critical hit
    /// (5e Champion Improved / Superior Critical: 19 or 18). Defaults to
    /// 20 for every other build. Read at every attack-roll site.
    ///
    /// `has_superior_critical` (Champion subclass level 15) caps the
    /// returned value at 18 so a template with Improved Critical
    /// baseline (`crit_threshold: 19`) drops cleanly to 18 the moment
    /// the flag flips, and a hypothetical Barbarian who somehow picks
    /// up Superior Critical (default `crit_threshold: 20`) also lands
    /// on 18. `min` rather than a hardcoded 18 assignment so a future
    /// even-wider threshold (a Fighter feat granting 17-20 crits) can
    /// stack cleanly by lowering `crit_threshold` further without this
    /// accessor overriding the finer value.
    pub fn crit_threshold(&self) -> u32 {
        if self.has_superior_critical {
            self.crit_threshold.min(18)
        } else {
            self.crit_threshold
        }
    }

    /// 5e Champion Fighter Superior Critical (subclass level 15):
    /// crit threshold drops to 18. Read by `crit_threshold()`.
    pub fn has_superior_critical(&self) -> bool {
        self.has_superior_critical
    }

    /// Test-only setter for the Superior Critical flag. Mirrors
    /// `set_persistent_rage` / `set_aura_of_hate` on the racial /
    /// class flag lane — lets tests dial the passive onto any chassis
    /// to verify the `crit_threshold()` cap folds correctly across
    /// templates that don't natively ship the flag.
    #[cfg(test)]
    pub fn set_superior_critical(&mut self, value: bool) {
        self.has_superior_critical = value;
    }

    /// True if the actor has the Lucky trait / feat. The d20 reroll
    /// fires on a natural 1 at the attack-roll / save-roll site.
    pub fn has_lucky(&self) -> bool {
        self.has_lucky
    }

    /// Test-only setter for brutal critical dice. Lets tests dial the
    /// rider on without needing a dedicated level-13 template.
    #[cfg(test)]
    pub fn set_brutal_critical_dice(&mut self, dice: u32) {
        self.brutal_critical_dice = dice;
    }

    /// Check if a recharge ability is currently available.
    pub fn is_recharge_available(&self, action_name: &str) -> bool {
        self.recharge_abilities
            .iter()
            .any(|(name, _, avail)| *name == action_name && *avail)
    }

    /// Mark a recharge ability as spent (unavailable until recharged).
    pub fn spend_recharge(&mut self, action_name: &str) {
        for entry in &mut self.recharge_abilities {
            if entry.0 == action_name {
                entry.2 = false;
            }
        }
    }

    /// Raw recharge entries for inspection by the encounter engine.
    pub fn recharge_entries(&self) -> &[(&'static str, u32, bool)] {
        &self.recharge_abilities
    }

    /// Set a recharge ability's availability state.
    pub fn set_recharge_available(&mut self, action_name: &str, available: bool) {
        for entry in &mut self.recharge_abilities {
            if entry.0 == action_name {
                entry.2 = available;
            }
        }
    }

    /// HP regenerated each round-end while combat-active. 0 disables the
    /// heal; non-zero means `EncounterInstance::round_end` will heal the
    /// actor unless `regen_suppressed` is set.
    pub fn regen_per_round(&self) -> u32 {
        self.regen_per_round
    }

    pub fn regen_suppressed(&self) -> bool {
        self.regen_suppressed
    }

    pub fn clear_regen_suppression(&mut self) {
        self.regen_suppressed = false;
    }

    /// Flag the actor's regeneration as suppressed for this round if `dt`
    /// is one of the configured suppressor types. No-op for non-regen
    /// actors (whose `regen_suppressors` set is empty).
    pub fn note_regen_damage(&mut self, dt: DamageType) {
        if self.regen_suppressors.contains(&dt) {
            self.regen_suppressed = true;
        }
    }

    pub fn rolls_death_saves(&self) -> bool {
        self.rolls_death_saves
    }

    /// First action in the actor's list whose `name()` matches `name`,
    /// compared case-insensitively.
    ///
    /// Case-insensitive because the engine carries a spell's display
    /// name in two casings and neither is wrong. `Action::name()` is
    /// lowercase throughout ("web", "stinking cloud") since it doubles
    /// as the command-line token; `ConcentrationData::spell_name` is
    /// Title Case ("Web", "Stinking Cloud") since it is written for the
    /// log and the UI. `EncounterInstance::concentrating_on_school`
    /// resolves the second back to the first to read the spell's
    /// school, and an exact comparison would silently never match —
    /// failing closed in a way that looks exactly like "this spell has
    /// no school tag".
    ///
    /// **Searches the template list only.** An action that arrives on a
    /// carried consumable — everything with an `Item::on_use` — is not
    /// in `self.actions` and is not found here. That is the right
    /// default for the thirty-odd callers that are asking "does this
    /// creature's stat block have X", and a trap for anyone asking
    /// "can this creature do X right now": the lookup returns `None`
    /// and reads exactly like the creature not having the action,
    /// rather than like the search having been aimed at the wrong list.
    /// `available_actions` is the list that includes them, and
    /// `ai::simple::try_self_action_inc_items` is the AI-side lookup
    /// built on it.
    pub fn find_action(&self, name: &str) -> Option<&'static (dyn Action + Send + Sync)> {
        self.actions
            .iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
            .copied()
    }

    /// First melee weapon action on this actor's action list — the
    /// shared predicate used by both the opportunity-attack dispatcher
    /// (`EncounterInstance::dispatch_opportunity_attacks`) and the
    /// Battle Master Riposte reaction (`engine::attack::try_fire_riposte`).
    /// Filters on the same three clauses:
    ///
    /// 1. `is_harmful()` — excludes touch-range buffs / heals (Cure
    ///    Wounds is `SingleActor` with reach 1 but harmless — an ally
    ///    shouldn't opportunity-heal a fleeing target).
    /// 2. `deals_damage()` — excludes Shove and Grapple, which are
    ///    harmful, `SingleActor` and reach 1, and which every
    ///    reach-weapon monster in the bestiary was therefore
    ///    opportunity-attacking with. An opportunity attack that shoves
    ///    is not an opportunity attack; RAW's trigger text says "melee
    ///    attack", and a contest is not one.
    /// 3. `SingleActor` schema — excludes AoE / burst / point-target
    ///    spells; an opportunity attack / riposte hits one creature.
    /// 4. `is_melee_attack()` — a swing rather than a shot, measured
    ///    against the whole melee band rather than against an ordinary
    ///    weapon's reach.
    /// 5. `!chains_multiple_attacks()` — one swing, not a monster's
    ///    whole Attack routine. RAW grants "one melee attack"; without
    ///    this a tarrasque answered a provoking step with bite, two
    ///    claws and a tail, for free, for each creature that walked
    ///    past. See `Action::chains_multiple_attacks`.
    /// 6. `school().is_none()` — not a spell. RAW's trigger grants "one
    ///    melee attack", and casting is not one without the War Caster
    ///    feat, which the engine does not model. The clause is also the
    ///    structural fix for the exposure
    ///    `an_opportunity_attack_is_never_something_the_reactor_would_pay_for`
    ///    was written to watch: both reaction dispatchers run
    ///    `side_effects` directly and charge only the reaction, so a
    ///    slot spell reaching this list would be cast free, once per
    ///    provoking step, for the rest of the fight. The old
    ///    `<= MELEE_REACH` gate held that back by accident — it happened
    ///    to exclude the Cleric's reach-2 Spiritual Weapon — and
    ///    widening the band to fix the ogre would have opened it.
    ///
    /// Clauses 2 and 4 are both fixes to the same class of bug, and the
    /// ogre shows both at once. Its greatclub reaches 2 tiles, so the
    /// old `<= MELEE_REACH` gate skipped it; the next harmful
    /// `SingleActor` reach-1 action on the list is Shove, so every ogre,
    /// hill giant, wyvern, treant and dragon in the game answered a
    /// creature leaving its reach by trying to push it over. See
    /// `MELEE_BAND_REACH`.
    ///
    /// Returns `None` when the actor has no eligible melee swing. Both
    /// call sites previously open-coded this find-and-filter chain; the
    /// helper centralizes it so a fix like the two above lands in one
    /// place instead of two.
    pub fn first_melee_weapon_action(
        &self,
    ) -> Option<&'static (dyn Action + Send + Sync)> {
        use crate::actions::action_template::TargetingSchema;
        self.actions
            .iter()
            .find(|act| {
                act.is_harmful()
                    && act.deals_damage()
                    && !act.chains_multiple_attacks()
                    && matches!(act.targeting_schema(), TargetingSchema::SingleActor)
                    && act.school().is_none()
                    && act.is_melee_attack()
            })
            .copied()
    }

    /// Sum every carried item's `ItemBonuses` into one struct.
    pub fn total_item_bonuses(&self) -> ItemBonuses {
        self.items
            .iter()
            .fold(ItemBonuses::ZERO, |acc, it| acc + it.bonuses)
    }

    pub fn items(&self) -> &[&'static Item] {
        &self.items
    }

    pub fn pickup_item(&mut self, item: &'static Item) {
        self.items.push(item);
        // Install passive-condition trinket buffs (Slippers of Spider
        // Climbing, Winged Boots, etc.). Each entry is installed with
        // `Permanent` timer; the install gate honors immunities (so a
        // Cloak of Displacement on a creature with template Displacement-
        // immunity silently no-ops). Duplicates are deduped by
        // `add_condition` (it keeps the longer / Permanent timer).
        for &c in item.passive_conditions {
            self.add_condition(c, ConditionTimer::Permanent);
        }
    }

    pub fn has_item_named(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name)
    }

    /// True while the actor carries any item whose
    /// `grants_magical_attacks` flag is set — the `+1` / `+2` weapon
    /// tier. Read by the `MAGICAL_ATTACK_SOURCES` cohort in
    /// `crate::engine::magic`; see there for the other six ways a swing
    /// can be magical.
    pub fn wields_enchanted_weapon(&self) -> bool {
        self.items.iter().any(|i| i.grants_magical_attacks)
    }

    pub fn remove_item_by_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.name == name) {
            let removed = self.items.remove(pos);
            // Strip passive conditions the dropped item granted, unless
            // another carried item still grants the same condition (e.g.
            // two Winged Boots paired) — keeps the install lane idempotent
            // across multi-item stacks.
            for &c in removed.passive_conditions {
                let still_granted = self
                    .items
                    .iter()
                    .any(|it| it.passive_conditions.contains(&c));
                if !still_granted {
                    self.remove_condition(c);
                }
            }
            true
        } else {
            false
        }
    }

    /// Re-install every passive condition granted by a currently-carried
    /// item. Used after `long_rest` clears the condition map so trinkets
    /// like Slippers of Spider Climbing keep their always-on buff across
    /// rest cycles. Idempotent — running it on an actor whose passive
    /// conditions are already up is a no-op (the install gate dedupes via
    /// `add_condition`'s timer-extension logic).
    fn reinstall_item_passive_conditions(&mut self) {
        // Snapshot the (item, condition) pairs first so the borrow on
        // `self.items` doesn't fight the `add_condition` mutation.
        let to_install: Vec<Condition> = self
            .items
            .iter()
            .flat_map(|it| it.passive_conditions.iter().copied())
            .collect();
        for c in to_install {
            self.add_condition(c, ConditionTimer::Permanent);
        }
    }

    /// Base actions plus one entry per unique consumable item the actor
    /// is carrying (deduped by item name).
    pub fn available_actions(&self) -> Vec<&'static (dyn Action + Send + Sync)> {
        let mut out = self.actions.clone();
        let mut seen: HashSet<&'static str> = HashSet::new();
        for item in &self.items {
            if let Some(action) = item.on_use
                && seen.insert(item.name)
            {
                out.push(action);
            }
        }
        out
    }

    /// Restore full HP, all spell slots, clear non-permanent conditions,
    /// concentration and any temp HP. 5e long rest semantics.
    pub fn long_rest(&mut self) {
        self.hp_state = HpState::Active;
        // 5e: "finishing a long rest reduces a creature's exhaustion
        // level by 1." One rung, not the whole ladder — a creature that
        // marched itself to tier 4 wakes up at tier 3, and the halved
        // hit point maximum below is computed *after* the reduction so a
        // rest that clears tier 4 also restores the full pool.
        //
        // Explicit rather than riding the `conditions.clear()` below,
        // which bypasses `remove_condition` and would otherwise strand
        // the tier count with no flag beside it.
        self.reduce_exhaustion(1);
        self.hitpoints = self.max_hitpoints();
        self.temp_hp = 0;
        // 5e Arcane Ward RAW: "once you create the ward, you can't create
        // it again until you finish a long rest." Dropping both the pool
        // and the woven latch here is what makes the next encounter's
        // first abjuration cast re-weave a full-strength ward instead of
        // trickling twice-the-slot-level onto a stale one.
        self.arcane_ward = 0;
        self.arcane_ward_formed = false;
        // 5e Portent RAW: "When you finish a long rest, roll two d20s
        // and record the numbers rolled." Dropping the pool *and* the
        // forecast latch is what re-arms the lazy fill — the next
        // substitution opportunity rolls a fresh set off the encounter
        // roller. Clearing only the pool would leave the latch set and
        // strand the diviner without a forecast for the rest of the run.
        self.portent_pool.clear();
        self.portent_forecast = false;
        // RAW: the Overchannel backlash escalates "if you use this
        // feature again before you finish a long rest", so the rest
        // resets the escalation to its free first use.
        self.overchannel_uses = 0;
        self.overchannel_backlash_pending = false;
        self.spell_slot_manager.restore_spell_slots();
        self.conditions.clear();
        // Exhaustion is the one condition a long rest does not lift
        // outright, so the blanket clear above has to be walked back
        // whenever a tier survived the reduction. Re-installed from the
        // number, which is the authority — the flag is its shadow.
        if self.exhaustion > 0 {
            self.conditions
                .insert(Condition::Exhausted, ConditionTimer::Permanent);
        }
        self.concentration = None;
        self.attack_bonus_buff = 0;
        self.save_bonus_buff = 0;
        self.damage_bonus_buff = 0;
        self.features_remaining = self.features_max.clone();
        self.indomitable_pending = false;
        // 5e Rogue Assassin **Assassinate** is a per-combat latch ("any
        // creature that hasn't taken a turn in the combat yet"). A long
        // rest separates encounters in the multi-encounter loop — clear
        // the latch here so an Assassin in a fresh combat still gets the
        // alpha-strike window against targets whose latch latched in the
        // previous fight.
        self.has_taken_turn_in_combat = false;
        // 5e Relentless Rage RAW: "When you finish a short or long rest,
        // the DC resets to 10." Long-rest path also calls this reset; the
        // short-rest path below tops up the same field.
        self.relentless_rage_dc = 10;
        self.legendary_resistance_remaining = self.legendary_resistance_max;
        for entry in &mut self.recharge_abilities {
            entry.2 = true;
        }
        self.legendary_action_slots = self.legendary_actions_per_round;
        self.sorcery_points = self.sorcery_points_max;
        // Restore passive-trinket conditions cleared by `conditions.clear()`
        // above so the wearer wakes up still spider-climbing / flying /
        // whatever the carried trinkets grant.
        self.reinstall_item_passive_conditions();
    }

    /// 5e Short Rest — 1 hour of downtime. Restores: Hit Dice-based
    /// healing (we approximate with CON-mod * level HP), fighter features
    /// (Second Wind, Action Surge), and warlock Pact Magic slots (lv1-5).
    /// Does NOT restore full HP, clear conditions, or reset concentration.
    pub fn short_rest(&mut self, roller: &mut impl Roller) {
        if !matches!(self.hp_state, HpState::Active) {
            return;
        }
        let con_mod = modifier_from_score(self.constitution);
        let dice_count = (self.level / 2).max(1);
        let roll = roller.roll(&Dice::new(dice_count, 8)) as i32;
        let heal = (roll + con_mod * dice_count as i32).max(0) as u32;
        self.heal(heal);

        // The Battle Master maneuvers used to be chained on here as a
        // second registry. They aren't any more: they spend from
        // `SUPERIORITY_DICE_TAG`, which is a row on `SHORT_REST_FEATURES`
        // like any other feature, so the one refill below restores the
        // whole suite.
        for &tag in SHORT_REST_FEATURES.iter() {
            // Read through the `has_passive_feature` accessor rather than
            // the private `features_max` set directly — same lane the
            // Sorcerous Restoration / Tiger Totem / Fast Movement sites
            // already went through in a prior nudge. Keeps the passive-
            // feature read shape uniform across the class-feature lane.
            if !self.has_passive_feature(tag) {
                continue;
            }
            // Bardic Inspiration's short-rest refresh is a lv5-gated
            // Bard feature (Font of Inspiration). Without the Font tag,
            // Bardic Inspiration refreshes only on long rest per RAW; a
            // lv1-4 bard would otherwise get free short-rest refills.
            // Sibling to Sorcerous Restoration below on the "class-feature
            // refresh gated by a distinct passive tag" lane — both live
            // outside `SHORT_REST_FEATURES` proper as their conditional
            // logic bites at the read site.
            if tag == BARDIC_INSPIRATION_TAG
                && !self.has_passive_feature(FONT_OF_INSPIRATION_TAG)
            {
                continue;
            }
            // Refill to the pool's own size rather than to one, so a
            // multi-charge feature comes back off a short rest with
            // everything RAW says it has.
            let max = self.features_max.get(tag).copied().unwrap_or(1);
            self.features_remaining.insert(tag, max);
        }

        // 5e Relentless Rage RAW: DC resets to 10 on short / long rest.
        // Same reset as the long-rest path above; the field is the only
        // bit of per-rest Relentless Rage state.
        self.relentless_rage_dc = 10;

        // 5e Sorcerer **Sorcerous Restoration** (lv20 capstone): regain 4
        // expended sorcery points on short rest. We collapse the RAW
        // "after using metamagic" gate to "always, if the feature is on"
        // — short rests are rare enough that the partial refill rarely
        // arrives at full pool, and the heuristic keeps the trigger
        // testable. Capped at `sorcery_points_max` via `give_sorcery_points`.
        if self.has_passive_feature(SORCEROUS_RESTORATION_TAG) {
            self.give_sorcery_points(4);
        }
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// 5e: a new application replaces the existing pool only if it's
    /// larger. Returns the resulting pool size — callers that want a
    /// "did it change?" boolean can diff against `temp_hp()` from before
    /// the call, or compare against `amount` (a no-op leaves the prior
    /// pool, which is `>= amount`).
    ///
    /// A **swarm** takes none: RAW's "the swarm can't regain hit points
    /// or gain temporary hit points". Refused here rather than at each
    /// of the dozen sources so a new temp-HP grant inherits the rule for
    /// free — the same reason `heal` carries the other half.
    ///
    /// Gated on `is_swarm` directly rather than on
    /// `can_regain_hitpoints`, and the difference is deliberate: that
    /// predicate also answers for **Chill Touch**, whose RAW clause
    /// names hit points and stops there. Temporary hit points are not
    /// hit points, so a chilled fighter can still be handed a False Life
    /// pool while a swarm cannot. Two rules that overlap on one lane and
    /// diverge on the other, which is exactly the case a shared
    /// predicate would have papered over.
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        if self.is_swarm {
            return self.temp_hp;
        }
        if amount > self.temp_hp {
            self.temp_hp = amount;
        }
        self.temp_hp
    }

    /// Total damage this actor can absorb before dropping: real HP plus
    /// both pools that stand in front of it — temporary hit points and
    /// the Abjuration Wizard's Arcane Ward.
    ///
    /// This is the number "how close is that target to falling?" actually
    /// wants. Raw `hitpoints()` answers a different question, and the gap
    /// between them is not cosmetic: a 9 HP abjurer behind a full 9-point
    /// ward reads as the squishiest thing on the board while needing
    /// double the damage of anyone at the same HP, and an ally who just
    /// ate a Heroism / False Life / Armor of Agathys grant reads the
    /// same way. A focus-fire heuristic keyed on `hitpoints()` walks into
    /// both.
    ///
    /// Deliberately *not* what healing decisions should read — a heal
    /// restores real HP only, so an ally sitting at 2 HP behind 20 temp
    /// HP is still the one worth a Cure Wounds, and the support pipeline
    /// keeps using `hitpoints()`.
    pub fn effective_hitpoints(&self) -> u32 {
        self.hitpoints
            .saturating_add(self.temp_hp)
            .saturating_add(self.arcane_ward)
    }

    /// Record an Overchannel use and report how many d12 of necrotic
    /// backlash *per spell level* the caster owes for it.
    ///
    /// RAW: "The first time you do so, you suffer no adverse effect. If
    /// you use this feature again before you finish a long rest, you
    /// take 2d12 necrotic damage for each level of the spell. Each time
    /// you use this feature again before finishing a long rest, the
    /// necrotic damage per level increases by 1d12." So use #1 owes 0,
    /// use #2 owes 2, use #3 owes 3, and so on — the die count and the
    /// use count coincide from the second use onward.
    pub fn note_overchannel_use(&mut self) -> u32 {
        self.overchannel_uses += 1;
        if self.overchannel_uses <= 1 {
            0
        } else {
            self.overchannel_uses
        }
    }

    /// Latch that Overchannel fired on the cast currently resolving, so
    /// the post-cast trigger knows to charge the backlash.
    pub fn set_overchannel_backlash_pending(&mut self) {
        self.overchannel_backlash_pending = true;
    }

    /// Read and clear the Overchannel backlash latch. Returns whether it
    /// was set — the post-cast trigger's single gate.
    pub fn take_overchannel_backlash_pending(&mut self) -> bool {
        std::mem::take(&mut self.overchannel_backlash_pending)
    }

    /// How many times Overchannel has fired since the last long rest.
    /// Exposed for the UI's resource panel and for tests that assert the
    /// escalation ramp.
    pub fn overchannel_uses(&self) -> u32 {
        self.overchannel_uses
    }

    /// True when this actor holds the Abjuration Wizard's Arcane Ward
    /// feature at all (`arcane_ward_base > 0` on their template) —
    /// independent of whether the ward has been woven yet. The single
    /// gate every ward consumer opens with.
    pub fn has_arcane_ward(&self) -> bool {
        self.arcane_ward_base > 0
    }

    /// Current Arcane Ward hit points. 0 for actors without the feature,
    /// for holders who haven't cast an abjuration spell yet, and for
    /// holders whose ward has been fully drained (RAW: the magic remains
    /// and can still be recharged — see `arcane_ward_formed`).
    pub fn arcane_ward(&self) -> u32 {
        self.arcane_ward
    }

    /// Whether the Arcane Ward has already been woven since the last
    /// long rest. Distinct from `arcane_ward() > 0`: a fully-drained
    /// ward is still formed (RAW keeps its magic alive and rechargeable)
    /// and must not re-weave at full strength on the next abjuration
    /// cast. Read by the post-cast hook to pick the log line and by the
    /// UI to decide whether to render the ward gauge at all.
    pub fn arcane_ward_formed(&self) -> bool {
        self.arcane_ward_formed
    }

    /// Full RAW Arcane Ward maximum: twice the holder's wizard level
    /// (carried as the flat `arcane_ward_base` template term) plus their
    /// Intelligence modifier. Floors at `arcane_ward_base` so a hypothetical
    /// negative-INT abjurer can't end up with a ward smaller than the
    /// level term alone. Returns 0 for actors without the feature.
    pub fn arcane_ward_max(&self) -> u32 {
        if !self.has_arcane_ward() {
            return 0;
        }
        let int_mod = self.ability_modifier(AbilityScoreType::Intelligence);
        (self.arcane_ward_base as i32 + int_mod).max(self.arcane_ward_base as i32) as u32
    }

    /// 5e Abjuration Wizard Arcane Ward form-or-recharge step, run when
    /// the holder casts an abjuration spell of 1st level or higher.
    ///
    /// - First such cast **weaves** the ward: it appears at its full
    ///   `arcane_ward_max`, independent of the slot level spent.
    /// - Every later cast **recharges** it by twice the slot level,
    ///   capped at the maximum. A ward sitting at 0 recharges normally —
    ///   RAW keeps the magic alive even when the pool is spent.
    ///
    /// Returns `Some((gained, now))` when the pool actually moved, and
    /// `None` for non-holders, cantrips (`spell_level == 0`), and
    /// already-full wards — so the caller logs only on a real change.
    pub fn weave_or_recharge_arcane_ward(&mut self, spell_level: u32) -> Option<(u32, u32)> {
        if !self.has_arcane_ward() || spell_level == 0 {
            return None;
        }
        let max = self.arcane_ward_max();
        let target = if self.arcane_ward_formed {
            (self.arcane_ward + 2 * spell_level).min(max)
        } else {
            self.arcane_ward_formed = true;
            max
        };
        let gained = target.saturating_sub(self.arcane_ward);
        if gained == 0 {
            return None;
        }
        self.arcane_ward = target;
        Some((gained, self.arcane_ward))
    }

    /// True if this actor carries the 5e Divination Wizard **Portent**
    /// feature at all (`portent_dice > 0` on their template) — not
    /// whether any foretold die is currently unspent. Cheapest gate on
    /// the substitution path, which runs on *every* d20 the engine
    /// rolls, so it stays a single scalar compare.
    pub fn has_portent(&self) -> bool {
        self.portent_dice_max > 0
    }

    /// How many foretold dice this actor banks per long rest (2 at
    /// Portent, 3 at Greater Portent; 0 for non-diviners).
    pub fn portent_dice_max(&self) -> u32 {
        self.portent_dice_max
    }

    /// The foretold faces still unspent, in bank order. Read by the UI
    /// gauge and by tests; the spend path uses `peek_portent_die` /
    /// `take_portent_die` instead so the "highest or lowest" policy
    /// lives in one place.
    pub fn portent_pool(&self) -> &[u32] {
        &self.portent_pool
    }

    /// Whether the foretold dice have been rolled since the last long
    /// rest. False means the lazy fill still owes this actor a
    /// forecast; true with an empty `portent_pool` means every die has
    /// been spent and none come back before the next long rest.
    pub fn portent_forecast(&self) -> bool {
        self.portent_forecast
    }

    /// Bank a fresh set of foretold faces. Called by the encounter-side
    /// lazy fill with `portent_dice_max` d20 rolls off the seeded
    /// roller. Idempotent guard: a second call while the forecast latch
    /// is already set is a no-op, so a mid-encounter re-entry can't
    /// silently refill a spent pool.
    pub fn set_portent_pool(&mut self, faces: Vec<u32>) {
        if self.portent_forecast || !self.has_portent() {
            return;
        }
        self.portent_forecast = true;
        self.portent_pool = faces;
    }

    /// The face this actor would spend on a roll they want to go
    /// `want_high ? well : badly` — the highest banked face when the
    /// diviner wants the roll to succeed, the lowest when they want it
    /// to fail. `None` when the pool is empty.
    ///
    /// Split from `take_portent_die` so the caller can apply its
    /// "is this die decisive enough to be worth burning?" threshold
    /// before committing to the spend.
    pub fn peek_portent_die(&self, want_high: bool) -> Option<u32> {
        if want_high {
            self.portent_pool.iter().copied().max()
        } else {
            self.portent_pool.iter().copied().min()
        }
    }

    /// Spend the face `peek_portent_die` would have returned, removing
    /// it from the pool. `None` (and no mutation) when the pool is
    /// empty. Removes exactly one entry even when the pool holds
    /// duplicates of the chosen face.
    pub fn take_portent_die(&mut self, want_high: bool) -> Option<u32> {
        let face = self.peek_portent_die(want_high)?;
        let idx = self.portent_pool.iter().position(|&f| f == face)?;
        Some(self.portent_pool.remove(idx))
    }

    /// Returns the post-modifier damage value (immunity → 0, resistance
    /// → halve, vulnerability → double, none → unchanged). Doesn't touch
    /// temp HP — that's `take_typed_damage`'s job.
    ///
    /// 5e stacking rule (PHB p.197): "Multiple instances of resistance or
    /// vulnerability that affect the same damage type count as only one
    /// instance." We track whether any resistance source has applied via
    /// `resisted` and skip further halving once it's set. Immunity still
    /// trumps everything and zeros the amount immediately.
    pub fn effective_damage(&self, raw: u32, dt: DamageType) -> u32 {
        // 5e's three stacking rules for typed damage, in the order they
        // resolve:
        //
        //   1. Immunity beats everything. Four lanes can grant it
        //      (template modifier, condition, item, passive feature);
        //      `is_immune_to_damage_type` walks all four.
        //   2. Resistance and vulnerability to the same type cancel.
        //   3. Multiple resistances still only halve once, however many
        //      sources agree.
        //
        // Collapsing the four resistance lanes into one boolean before
        // the match is what makes rules 2 and 3 hold by construction
        // rather than by the order the lanes happen to be tested in.
        // The previous shape applied vulnerability first and then
        // consulted each resistance lane in turn, each gated on the
        // earlier ones having missed — which got rule 3 right, got
        // rule 2 right for the condition and item lanes, and quietly
        // dropped it for the passive-feature lane, so a creature with a
        // feature-granted resistance and a template vulnerability to the
        // same type took double rather than full.
        if self.is_immune_to_damage_type(dt) {
            return 0;
        }
        let template_modifier = self.damage_modifiers.get(&dt).copied();
        let vulnerable = matches!(template_modifier, Some(DamageModifier::Vulnerability))
            || self.has_condition_vulnerability(dt);
        let resistant = matches!(template_modifier, Some(DamageModifier::Resistance))
            || self.has_passive_typed_resistance(dt)
            || self.has_condition_resistance(dt)
            || self.item_resistance_to(dt);
        match (vulnerable, resistant) {
            (true, true) => raw,
            (true, false) => raw.saturating_mul(2),
            (false, true) => raw / 2,
            (false, false) => raw,
        }
    }

    /// True iff the actor holds a condition that grants outright immunity
    /// to damage of type `dt`. Walks `TYPED_IMMUNITY_CONDITIONS` — each
    /// row is `ConditionDrivenTypedImmunity { source, types }` and
    /// pairs a source condition with the damage types it zeroes.
    /// Currently covers Mind Blank (psychic), Silence (thunder), and
    /// Petrified (poison); a new immunity rider adds a one-line
    /// struct-literal entry.
    pub fn has_condition_immunity(&self, dt: DamageType) -> bool {
        TYPED_IMMUNITY_CONDITIONS
            .iter()
            .any(|row| self.has_condition(row.source) && row.types.contains(&dt))
    }

    /// True iff the actor holds a condition that makes it vulnerable to
    /// damage of type `dt`. Walks `TYPED_VULNERABILITY_CONDITIONS`, the
    /// mirror of the immunity and resistance cohorts above.
    ///
    /// OR'd with the template's own `Vulnerability` entry rather than
    /// checked after it, so `effective_damage`'s resistance-cancels-
    /// vulnerability rule holds however the vulnerability arrived: a
    /// fire-resistant creature cursed by Path to the Grave takes full
    /// fire damage, not double and not half.
    pub fn has_condition_vulnerability(&self, dt: DamageType) -> bool {
        TYPED_VULNERABILITY_CONDITIONS
            .iter()
            .any(|row| self.has_condition(row.source) && row.types.contains(&dt))
    }

    /// True iff the actor is immune to damage of type `dt` from ANY
    /// source. Walks the four immunity lanes in one call:
    ///   1. Template `damage_modifiers` marked `Immunity`.
    ///   2. Condition-driven typed immunity (`TYPED_IMMUNITY_CONDITIONS`
    ///      — Mind Blank / Silence / Purified etc.).
    ///   3. Item-granted immunity (Periapt of Proof against Poison,
    ///      Ring of Mind Shielding, etc.).
    ///   4. Passive-feature-driven typed immunity
    ///      (`PASSIVE_TYPED_IMMUNITIES` — Monk Purity of Body pins
    ///      Poison damage to zero; future racial / subclass
    ///      typed-immunity features drop in as a one-line cohort
    ///      entry).
    ///
    /// Shared read chokepoint for `effective_damage` (which uses it as
    /// its top-of-pipeline immunity short-circuit) and any external
    /// rider that needs a single "is this actor zero-taking this type?"
    /// check. Sibling to `has_own_typed_reduction` (broader — includes
    /// resistance / vulnerability) but narrower — immunity only.
    pub fn is_immune_to_damage_type(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Immunity)
        ) || self.has_condition_immunity(dt)
            || self.item_immunity_to_damage(dt)
            || self.has_passive_typed_immunity(dt)
    }

    /// True iff the actor holds any passive-feature-driven typed
    /// immunity to damage of type `dt`. Walks the shared
    /// `PASSIVE_TYPED_IMMUNITIES` cohort — each row is
    /// `PassiveTypedImmunity { flag, types }`. Read by
    /// `is_immune_to_damage_type` (folds into the immunity short-
    /// circuit at the top of `effective_damage`). A new passive typed
    /// immunity drops in as a one-line cohort entry rather than
    /// another hand-rolled `dt == ... && self.has_passive_feature(...)`
    /// branch here. Sibling to `has_passive_typed_resistance` on the
    /// resistance lane — same walk shape, different cohort table.
    fn has_passive_typed_immunity(&self, dt: DamageType) -> bool {
        PASSIVE_TYPED_IMMUNITIES
            .iter()
            .any(|entry| entry.types.contains(&dt) && (entry.flag)(self))
    }

    /// True iff the actor holds a condition that grants resistance to
    /// damage of type `dt`. Walks three cohorts:
    /// - `BLANKET_RESISTANCE_CONDITIONS`: conditions that resist *every*
    ///   damage type (Stoneskin / Globe of Invulnerability / Warding
    ///   Bond / Petrified).
    /// - `RAGE_GATED_BROAD_RESISTANCES`: passive-tag-driven broad
    ///   resistance rows gated by Raging — resistance to every damage
    ///   type EXCEPT those in the row's `types_except` slice (Bear Totem
    ///   Spirit → all-except-psychic while raging).
    /// - `TYPED_RESISTANCE_CONDITIONS`: conditions whose resistance only
    ///   applies to a curated subset of damage types (Investiture of
    ///   Flame → Fire, Raging → physical trio, Purified → Poison).
    ///
    /// Centralizes the per-condition resistance lookup so a new Investiture
    /// spell only needs a one-line entry in the typed cohort, and the
    /// `effective_damage` site stays a single boolean read. Pre-cleanup
    /// the Bear Totem row rode as an ad-hoc `if dt != Psychic && Raging
    /// && has_passive_feature(BEAR_TOTEM_TAG)` inline branch here; the
    /// promoted cohort drops the special case and lets future rage-gated
    /// broad-resistance features land as one-line entries in
    /// `RAGE_GATED_BROAD_RESISTANCES` rather than another inline
    /// if-branch chain in this body.
    pub fn has_condition_resistance(&self, dt: DamageType) -> bool {
        if BLANKET_RESISTANCE_CONDITIONS
            .iter()
            .any(|c| self.has_condition(*c))
        {
            return true;
        }
        // 5e "rage-gated broad resistance" cohort — currently one row
        // (Bear Totem Spirit: all-except-psychic while raging). The
        // Raging gate is shared across every row so it's checked once
        // here; per-row differences live in the row's `tag` +
        // `types_except` slice. Ordering vs. the TYPED cohort below
        // doesn't matter — both grant the same "one halving" flag; the
        // BLANKET short-circuit above still wins first for Stoneskin /
        // Globe / Petrified so the "one halving per damage instance"
        // rule still holds.
        if self.has_condition(Condition::Raging)
            && RAGE_GATED_BROAD_RESISTANCES.iter().any(|row| {
                !row.types_except.contains(&dt) && self.has_passive_feature(row.tag)
            })
        {
            return true;
        }
        TYPED_RESISTANCE_CONDITIONS.iter().any(|row| {
            self.has_condition(row.source) && row.types.contains(&dt)
        })
    }

    pub fn damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        self.damage_modifiers.get(&dt).copied()
    }

    /// The modifier this creature applies to `dt` damage **from a
    /// nonmagical attack specifically**, or `None` if a nonmagical
    /// attack is treated no differently from any other source.
    ///
    /// Returns `None` whenever the unqualified table already carries a
    /// row for `dt`, and that early-out is the 5e "multiple instances
    /// of resistance count as only one" rule holding by construction:
    /// the unqualified row is applied by `DealDamage::apply` on every
    /// damage instance, so a qualified row for the same type would be a
    /// second halving of the same blow rather than a different rule. No
    /// stat block on the roster carries both for one type — this is
    /// what makes that stay true rather than a coincidence the next
    /// template can break.
    pub fn nonmagical_damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        if self.damage_modifiers.contains_key(&dt) {
            return None;
        }
        self.nonmagical_damage_modifiers.get(&dt).copied()
    }

    /// True iff this actor already has some form of typed damage
    /// modification (resistance / immunity / vulnerability) for `dt`
    /// coming from their OWN sources — template damage modifiers,
    /// blanket + typed condition resistances / immunities, or item-
    /// granted resistance / immunity. Used by external "extra halving"
    /// riders (Ancients Paladin **Aura of Warding**) to enforce the
    /// standard 5e "one halving per damage instance" rule — if the
    /// actor already scales the damage themselves, the rider no-ops
    /// and the built-in scaling in `effective_damage` fires unchanged.
    ///
    /// Vulnerability counts too even though it *doubles* damage — the
    /// point of the check is "does the actor already handle this type
    /// specially?", and layering a halving on top of a doubling would
    /// unpredictably wash out to no change (0.5 * 2 = 1).
    pub fn has_own_typed_reduction(&self, dt: DamageType) -> bool {
        // Template `damage_modifiers` — any entry (resistance / immunity
        // / vulnerability) means the actor already has a typed handler.
        self.damage_modifiers.contains_key(&dt)
            || self.has_condition_resistance(dt)
            || self.item_resistance_to(dt)
            || self.has_passive_typed_resistance(dt)
            // Immunity (any source) short-circuits at the shared helper.
            || self.is_immune_to_damage_type(dt)
    }

    /// True iff the actor holds any passive-feature-driven typed
    /// resistance to damage of type `dt`. Walks the shared
    /// `PASSIVE_TYPED_RESISTANCES` cohort — each row is a
    /// `PassiveTypedResistance { flag, types }`; the flag returning
    /// true grants half damage of every listed type. Read by
    /// `effective_damage` (folds into the template-resistance lane)
    /// and `has_own_typed_reduction` (the "does this actor already
    /// scale this type?" gate for Aura of Warding stacking). Sibling
    /// walk shape to `has_passive_typed_immunity` on the
    /// `PASSIVE_TYPED_IMMUNITIES` cohort — same `entry.types.contains
    /// (&dt) && (entry.flag)(self)` any-row predicate, different lane
    /// (halving vs. zero). A new passive typed resistance drops in as
    /// a one-line cohort entry rather than another hand-rolled
    /// `if dt == ... && self.has_...` branch here.
    fn has_passive_typed_resistance(&self, dt: DamageType) -> bool {
        PASSIVE_TYPED_RESISTANCES
            .iter()
            .any(|entry| entry.types.contains(&dt) && (entry.flag)(self))
            || self.transmuters_stone_resists(dt)
    }

    /// 5e Transmutation Wizard **Transmuter's Stone** (subclass level
    /// 6), attuned to Warding: resistance to the one damage type the
    /// stone is set to.
    ///
    /// The stone's other two attunements ride cohort rows
    /// (`PASSIVE_FEATURE_SPEED_BONUSES`, `FLAG_DRIVEN_SAVE_PROFICIENCIES`);
    /// this one can't. `PassiveTypedResistance::types` is a
    /// `&'static [DamageType]` — a compile-time list — and Warding's
    /// type is a runtime value carried in the variant, chosen per
    /// attunement. Enumerating five rows (one per RAW-legal type)
    /// would express it, but at the cost of five near-identical
    /// entries whose only difference is the type they compare against,
    /// which is the same shape the cohort exists to collapse. One OR
    /// against the field is the smaller change.
    fn transmuters_stone_resists(&self, dt: DamageType) -> bool {
        self.transmuters_stone == Some(TransmutersStoneBenefit::Warding(dt))
    }

    /// Test-only setter for an actor's per-type damage modifier. Lets tests
    /// patch resistance / immunity / vulnerability onto an existing actor
    /// (e.g. to verify Transmuted Spell remaps onto a resisted type)
    /// without needing a dedicated template per resistance profile.
    #[cfg(test)]
    pub fn set_damage_modifier(&mut self, dt: DamageType, modifier: DamageModifier) {
        self.damage_modifiers.insert(dt, modifier);
    }

    pub fn is_resistant_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Resistance)
        )
    }

    /// True when this creature halves `dt` damage from a **nonmagical
    /// attack** specifically — the qualified sibling of
    /// `is_resistant_to`, and the shape the forty stat blocks built on
    /// `CreatureTemplate::resistant_to_nonmagical_physical` answer to.
    ///
    /// Deliberately not folded into `is_resistant_to`: a caller that
    /// wants to know "will this blow be halved" has to know whose blow
    /// it is, and every caller that cannot answer that should get
    /// `false` from the unqualified predicate rather than a guess.
    pub fn resists_nonmagical(&self, dt: DamageType) -> bool {
        self.nonmagical_damage_modifier(dt) == Some(DamageModifier::Resistance)
    }

    pub fn is_vulnerable_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Vulnerability)
        )
    }

    pub fn is_immune_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Immunity)
        )
    }

    pub fn cr(&self) -> f32 {
        self.cr
    }

    /// 5e proficiency bonus: +2 at L1-4, +3 at L5-8, +4 at L9-12, etc.
    /// Both PCs (driven by `level`) and monsters (whose CR is roughly
    /// equivalent to a player level) read from the same scale via
    /// `proficiency_bonus_for_level` so the curve lives in one place.
    pub fn proficiency_bonus(&self) -> i32 {
        let effective_level = self.level.max(self.cr.floor().max(1.0) as u32);
        crate::engine::util::proficiency_bonus_for_level(effective_level)
    }

    /// Linear XP value: CR × 200. Linear is good enough for the dungeon
    /// loop and keeps the ramp legible.
    pub fn xp_value(&self) -> u32 {
        (self.cr * 200.0).round().max(0.0) as u32
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn is_save_proficient(&self, ability: AbilityScoreType) -> bool {
        // 5e Monk Diamond Soul (level 14): proficient in every saving
        // throw. Short-circuits the explicit `proficient_saves` set so
        // the monk's late-game envelope doesn't have to enumerate all
        // six abilities in the template. Kept out of the single-
        // ability cohort below since it applies to all six.
        if self.has_diamond_soul {
            return true;
        }
        // Single-ability flag-driven save-proficiency grants (Rogue
        // Slippery Mind lv15 WIS, Zealot Barbarian Iron Mind lv7 WIS,
        // and any future single-ability class / racial pickup). Each
        // row keys off a passive-feature closure and a target ability;
        // any hit is sufficient. Sibling to `FLAG_DRIVEN_IMMUNITIES`
        // — adding a future single-ability save-proficiency feature
        // lands as one row in the cohort table above rather than a
        // new if-branch here. Same `iter().any()` shape as the sibling
        // `has_flag_driven_save_advantage` cohort walk so the two
        // save-side cohort readers stay uniform.
        FLAG_DRIVEN_SAVE_PROFICIENCIES
            .iter()
            .any(|entry| entry.ability == ability && (entry.flag)(self))
            || self.proficient_saves.contains(&ability)
    }

    /// True if any passive-feature-driven save-advantage row in the
    /// shared `FLAG_DRIVEN_SAVE_ADVANTAGES` cohort fires for the given
    /// ability on this actor. Walked by `compute_save_mode` as a single
    /// filter-any pass over the cohort so the caller composes the
    /// advantage bit with the other save-mode flags (condition-driven
    /// disadvantage, Dodge, Haste / Slow, Feeblemind, Rage-on-STR)
    /// without an if-branch chain in the encounter body.
    ///
    /// Sibling to `is_save_proficient` on the "single accessor that
    /// walks a class-feature cohort" pattern — that one folds
    /// `FLAG_DRIVEN_SAVE_PROFICIENCIES`, this one folds
    /// `FLAG_DRIVEN_SAVE_ADVANTAGES`. Both are OR-of-rows filter passes
    /// so adding a fresh feature is a one-line cohort entry, not a
    /// touch on this method or the encounter body.
    pub fn has_flag_driven_save_advantage(&self, ability: AbilityScoreType) -> bool {
        FLAG_DRIVEN_SAVE_ADVANTAGES
            .iter()
            .any(|entry| (entry.flag)(self, ability))
    }

    /// True if this actor is proficient in the given skill (i.e. adds
    /// their proficiency bonus to checks made with it). 5e: the skill
    /// proficiency is tracked separately from the ability score it
    /// modifies — a creature can be proficient in Stealth (DEX-based)
    /// without being proficient in DEX-based saves.
    pub fn has_skill(&self, skill: Skill) -> bool {
        self.skills.contains(&skill)
    }

    /// Passive Perception (5e PHB p.175): 10 + WIS modifier + proficiency
    /// bonus if proficient in Perception. This is the score other actors
    /// compare against when sneaking (Stealth roll vs passive Perception)
    /// and when noticing hidden threats. Centralized here so callers
    /// (Hide / future stealth mechanics) don't open-code the
    /// `10 + ability_modifier(Wisdom)` and silently miss the proficiency
    /// bump for skilled scouts.
    pub fn passive_perception(&self) -> i32 {
        let mut score = 10 + self.ability_modifier(AbilityScoreType::Wisdom);
        if self.has_skill(Skill::Perception) {
            score += self.proficiency_bonus();
        }
        score
    }

    pub fn xp(&self) -> u32 {
        self.xp
    }

    /// XP needed to reach the *next* level from the current level.
    /// Linear curve `level * 300`.
    pub fn xp_threshold_for_next_level(&self) -> u32 {
        self.level * 300
    }

    pub fn award_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
    }

    /// Promote a PC to the next level if they've crossed the threshold.
    pub fn try_level_up(&mut self, roller: &mut impl Roller) -> Option<u32> {
        if self.xp < self.xp_threshold_for_next_level() {
            return None;
        }
        self.level += 1;
        let con_mod = modifier_from_score(self.constitution);
        let roll = roller.roll(&Dice::new(1, 10)) as i32;
        let gain = (roll + con_mod).max(1) as u32;
        self.base_hitpoints = self.base_hitpoints.saturating_add(gain);
        self.hitpoints = self
            .hitpoints
            .saturating_add(gain)
            .min(self.max_hitpoints());
        Some(self.level)
    }

    pub fn is_concentrating(&self) -> bool {
        self.concentration.is_some()
    }

    pub fn concentration(&self) -> Option<&ConcentrationData> {
        self.concentration.as_ref()
    }

    pub fn start_concentration(&mut self, data: ConcentrationData) -> Option<ConcentrationData> {
        self.concentration.replace(data)
    }

    pub fn end_concentration(&mut self) -> Option<ConcentrationData> {
        self.concentration.take()
    }

    pub fn has_condition(&self, c: Condition) -> bool {
        self.conditions.contains_key(&c)
    }

    /// True if the actor is dynamically immune to condition `c` from a
    /// non-template source — currently:
    ///   - Heroism (`Heroic`) → immune to Frightened
    ///   - Mind Blank (`MindBlanked`) → immune to Charmed
    ///   - Halfling Brave racial → immune to Frightened (approximated
    ///     from RAW's "advantage on saves vs Frightened")
    ///   - Fey Ancestry racial → immune to Charmed (approximation) AND
    ///     Asleep (RAW: "magic can't put you to sleep"; the only Asleep
    ///     installer in this engine is the Sleep spell, so this matches
    ///     RAW exactly)
    ///
    /// Distinct from `condition_immunities` (template-level immunities
    /// pinned at creature creation): this lane reads live state so an
    /// effect that drops can drop its rider immunity along with it. Read
    /// by `add_condition` as part of the install gate.
    pub fn dynamic_immunity_to(&self, c: Condition) -> bool {
        // Condition-driven cohort: any held source condition on the
        // `CONDITION_DRIVEN_IMMUNITIES` table whose suppression set
        // covers `c` grants immunity. Adding a new broad-immunity buff
        // (a future Sanctuary-tier ward, etc.) lands as one row in the
        // table without touching this method. Each row is
        // `ConditionDrivenConditionImmunity { source, suppressed }` —
        // the struct-row shape mirrors the sibling `FlagDrivenImmunity
        // { flag, suppressed }` cohort walked immediately below, so
        // both immunity lanes share the same "{ source, suppressed
        // slice }" declarative-table pattern end-to-end.
        if CONDITION_DRIVEN_IMMUNITIES.iter().any(|row| {
            row.suppressed.contains(&c) && self.has_condition(row.source)
        }) {
            return true;
        }
        // Flag / passive-feature cohort: race + subclass immunities that
        // fire regardless of any held condition. Table-driven for the
        // same reason CONDITION_DRIVEN_IMMUNITIES is — adding a future
        // racial / class capstone (e.g. Firbolg Hidden Step's Invisible
        // grant, a Circle of the Land Nature's Sanctuary) lands as one
        // row per flag. Each row lists the conditions the flag grants
        // immunity to; multiple rows can cover the same condition (both
        // Fey Ancestry AND Nature's Ward suppress Charmed).
        FLAG_DRIVEN_IMMUNITIES
            .iter()
            .any(|entry| (entry.flag)(self) && entry.suppressed.contains(&c))
    }

    /// Add a condition with the given timer. If the actor is immune to
    /// the condition (template-level via `condition_immunities`, or
    /// dynamic via `dynamic_immunity_to`), no-op and return false.
    ///
    /// 5e: re-applying a condition with a *longer* timer extends the
    /// effect; a shorter timer is ignored. Permanent beats any rounds
    /// timer; `UntilStartOfNextTurn` is treated as the shortest possible
    /// duration. Returns true if the condition was newly added.
    pub fn add_condition(&mut self, c: Condition, timer: ConditionTimer) -> bool {
        if self.effectively_immune_to_condition(c) {
            return false;
        }
        // 5e exhaustion is gained a level at a time, and every source in
        // the game says "gains 1 level of exhaustion" rather than
        // "becomes exhausted". Routing the install through the ladder is
        // what makes that true of every existing caller at once — the
        // flag they set is still set, it just now means "at least one
        // level", and a second application stacks instead of being
        // swallowed by the `contains_key` check below.
        let is_new = !self.conditions.contains_key(&c);
        if c == Condition::Exhausted {
            self.gain_exhaustion(1);
            return is_new;
        }
        let new_timer = match (self.conditions.get(&c).copied(), timer) {
            (None, t) => t,
            (Some(ConditionTimer::Permanent), _) => ConditionTimer::Permanent,
            (_, ConditionTimer::Permanent) => ConditionTimer::Permanent,
            (Some(ConditionTimer::Rounds(a)), ConditionTimer::Rounds(b)) => {
                ConditionTimer::Rounds(a.max(b))
            }
            (Some(ConditionTimer::Rounds(a)), ConditionTimer::UntilStartOfNextTurn) => {
                ConditionTimer::Rounds(a)
            }
            (Some(ConditionTimer::UntilStartOfNextTurn), ConditionTimer::Rounds(b)) => {
                ConditionTimer::Rounds(b)
            }
            (Some(ConditionTimer::UntilStartOfNextTurn), ConditionTimer::UntilStartOfNextTurn) => {
                ConditionTimer::UntilStartOfNextTurn
            }
        };
        self.conditions.insert(c, new_timer);
        is_new
    }

    /// Current exhaustion tier, 0 (none) through
    /// `EXHAUSTION_DEATH_TIER`. `has_condition(Exhausted)` is exactly
    /// `exhaustion_level() > 0`; the two are held in step by
    /// `gain_exhaustion` / `reduce_exhaustion`, which are the only
    /// writers.
    pub fn exhaustion_level(&self) -> u32 {
        self.exhaustion
    }

    /// Climb `levels` rungs of the exhaustion ladder, returning the new
    /// tier. Saturates at `EXHAUSTION_DEATH_TIER`, and reaching that
    /// rung kills outright — RAW's tier 6 is "death", with no save and
    /// no dying state to roll out of.
    ///
    /// Reached by every caller through `add_condition(Exhausted, _)`;
    /// public for the sources that hand out more than one level at a
    /// time and for tests that want to start partway up.
    pub fn gain_exhaustion(&mut self, levels: u32) -> u32 {
        if levels == 0 {
            return self.exhaustion;
        }
        self.exhaustion = self
            .exhaustion
            .saturating_add(levels)
            .min(EXHAUSTION_DEATH_TIER);
        self.conditions
            .insert(Condition::Exhausted, ConditionTimer::Permanent);
        if self.exhaustion >= EXHAUSTION_DEATH_TIER {
            self.hitpoints = 0;
            self.temp_hp = 0;
            self.hp_state = HpState::Dead;
            return self.exhaustion;
        }
        // Tier 4 halves the hit point maximum, and a creature sitting
        // above the new ceiling has to come down to it. Clipped here
        // rather than inside `max_hitpoints` because that accessor is
        // read on every damage and heal and has no business mutating.
        let cap = self.max_hitpoints();
        self.hitpoints = self.hitpoints.min(cap);
        self.exhaustion
    }

    /// Walk `levels` rungs back down, returning true if any tier was
    /// actually shed. Dropping to 0 clears `Condition::Exhausted`; any
    /// other landing keeps it, because the creature is still exhausted,
    /// just less so.
    ///
    /// Reached by every cleanse through `remove_condition(Exhausted)`.
    pub fn reduce_exhaustion(&mut self, levels: u32) -> bool {
        if self.exhaustion == 0 {
            // Keep the flag and the number honest even if something
            // desynced them — a bare `conditions.remove` elsewhere would
            // otherwise leave a level-0 creature flagged as exhausted.
            return self.conditions.remove(&Condition::Exhausted).is_some();
        }
        self.exhaustion = self.exhaustion.saturating_sub(levels);
        if self.exhaustion == 0 {
            self.conditions.remove(&Condition::Exhausted);
        }
        true
    }

    pub fn is_immune_to_condition(&self, c: Condition) -> bool {
        self.condition_immunities.contains(&c)
    }

    /// True if any item the actor is carrying grants immunity to `c`.
    /// Item-granted immunities (Necklace of Adaptation → Poisoned, Ring
    /// of Free Action → Paralyzed / Restrained / Grappled) fold in here
    /// so the install gate doesn't have to know about specific item
    /// names. Read by `add_condition` and `effectively_immune_to_condition`
    /// alongside the template / dynamic immunity lanes.
    pub fn item_immunity_to(&self, c: Condition) -> bool {
        self.items
            .iter()
            .any(|i| i.condition_immunities.contains(&c))
    }

    /// True if any item the actor is carrying grants resistance to
    /// damage of type `dt`. Walks the `damage_resistances` slice on each
    /// carried item — trinkets like the Brooch of Shielding (force) and
    /// Boots of the Winterlands (cold) fall out without code changes at
    /// the damage site. Read by `effective_damage` alongside template
    /// and condition-based resistance, honoring the 5e "one halving"
    /// stacking rule (item resistance is skipped when another source
    /// has already halved).
    pub fn item_resistance_to(&self, dt: DamageType) -> bool {
        self.items
            .iter()
            .any(|i| i.damage_resistances.contains(&dt))
    }

    /// True if any item the actor is carrying grants outright immunity
    /// to damage of type `dt`. Walks the `damage_immunities` slice on
    /// each carried item — trinkets like the Periapt of Proof against
    /// Poison (poison) and Ring of Mind Shielding (psychic) zero
    /// incoming damage through `effective_damage` without code changes
    /// at the damage site. Immunity wins over everything (no "one
    /// halving" stacking concern), so this fires before resistance
    /// rolls.
    pub fn item_immunity_to_damage(&self, dt: DamageType) -> bool {
        self.items
            .iter()
            .any(|i| i.damage_immunities.contains(&dt))
    }

    /// Combines template-level (`is_immune_to_condition`), dynamic
    /// (`dynamic_immunity_to`), and item-granted (`item_immunity_to`)
    /// immunity gates. Mirrors the install-side gate in `add_condition`
    /// — if all three bail on installing the condition, this helper
    /// returns true. Use this from any "should I bother targeting them?"
    /// prune (AI heuristics, spell validators, AoE early-pruning) so
    /// dynamic immunities (Halfling Brave's Frightened, Fey Ancestry's
    /// Charmed / Asleep, Heroic's Frightened, MindBlanked's Charmed) and
    /// trinket immunities (Necklace of Adaptation's Poisoned, Ring of
    /// Free Action's Paralyzed / Restrained / Grappled) are honored
    /// alongside the static template immunities.
    pub fn effectively_immune_to_condition(&self, c: Condition) -> bool {
        self.condition_immunities.contains(&c)
            || self.dynamic_immunity_to(c)
            || self.item_immunity_to(c)
    }

    pub fn remove_condition(&mut self, c: Condition) -> bool {
        // The mirror of the install: RAW's cleanses for exhaustion —
        // Greater Restoration, a long rest — each say "reduce the
        // target's exhaustion level by 1", not "end the condition". A
        // creature dragged to tier 4 and then given Greater Restoration
        // walks away at tier 3, still slowed and still rolling badly,
        // which is the whole texture of the mechanic.
        if c == Condition::Exhausted {
            return self.reduce_exhaustion(1);
        }
        let removed = self.conditions.remove(&c).is_some();
        if removed {
            // Keep tightly-linked auxiliary state in sync with the
            // primary condition flag. The back-link is keyed by the
            // condition, so dropping it needs no per-condition arm —
            // the seven linked conditions (Charmed, Dueled, Goaded,
            // Distracted, Sworn, EldritchStruck, WardingBonded) and any
            // future eighth are torn down by this one line, and there
            // is no longer a match to forget to extend. Mirror Image's
            // decoy count is the one piece of auxiliary state that
            // isn't an actor id, so it keeps its own arm.
            self.condition_links.remove(&c);
            if c == Condition::MirroredImages {
                self.mirror_images = 0;
            }
        }
        removed
    }

    pub fn conditions(&self) -> &HashMap<Condition, ConditionTimer> {
        &self.conditions
    }

    /// Decrement every `Rounds(n)` timer by 1 and report which conditions
    /// expired. `Permanent` and `UntilStartOfNextTurn` are untouched.
    pub fn tick_condition_timers(&mut self) -> Vec<Condition> {
        let mut expired = Vec::new();
        let snapshot: Vec<(Condition, ConditionTimer)> = self
            .conditions
            .iter()
            .map(|(c, t)| (*c, *t))
            .collect();
        for (c, timer) in snapshot {
            match timer {
                ConditionTimer::Permanent | ConditionTimer::UntilStartOfNextTurn => {}
                ConditionTimer::Rounds(0) | ConditionTimer::Rounds(1) => {
                    // Route through remove_condition so auxiliary state
                    // (Charmed back-link, mirror_images) clears too.
                    self.remove_condition(c);
                    // Report the expiry only if the flag actually left.
                    // Exhaustion is the one condition whose removal is a
                    // *decrement* — a timed application that lands on a
                    // creature already two rungs up leaves it exhausted,
                    // and announcing "no longer exhausted" would be a
                    // lie the caller has no way to check.
                    if !self.conditions.contains_key(&c) {
                        expired.push(c);
                    }
                }
                ConditionTimer::Rounds(n) => {
                    self.conditions.insert(c, ConditionTimer::Rounds(n - 1));
                }
            }
        }
        expired
    }

    /// Clear every condition with the `UntilStartOfNextTurn` timer.
    pub fn clear_until_next_turn_conditions(&mut self) -> Vec<Condition> {
        let mut expired = Vec::new();
        let to_remove: Vec<Condition> = self
            .conditions
            .iter()
            .filter_map(|(c, t)| match t {
                ConditionTimer::UntilStartOfNextTurn => Some(*c),
                _ => None,
            })
            .collect();
        for c in to_remove {
            self.remove_condition(c);
            expired.push(c);
        }
        expired
    }

    pub fn glyph(&self) -> char {
        self.glyph
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The actor's special-senses pool (Darkvision / Blindsight /
    /// Tremorsense / Truesight). Copied from the creature template at
    /// instantiation; doesn't change over the encounter's lifetime.
    /// Read by `has_truesight` for the truesight-suppress-illusion
    /// gate, by future blindsight / tremorsense gates, and by tests
    /// pinning per-template sense pools (so a future sense-set
    /// refactor surfaces breakage at the instance level rather than
    /// requiring the template literal to be re-read).
    pub fn senses(&self) -> &HashSet<SpecialSense> {
        &self.senses
    }

    /// True if the actor sees with Truesight — either intrinsically via
    /// a template `SpecialSense::Truesight(_)` (Deva, Solar, Pit Fiend,
    /// Marilith, Nalfeshnee, Lich, Kraken, Couatl, Glabrezu, Erinyes,
    /// Androsphinx, Nothic) OR via a transient `Condition::TrueSighted`
    /// (the True Seeing spell, the Eyes of Truth magic item). Read by
    /// `compute_attack_mode` at the illusion-concealment suppression
    /// gate so the intrinsic-senses cohort actually counters Invisible
    /// / Blurred / Displaced — matching the RAW intent. Without this
    /// any-of accessor the engine quietly let a Pit Fiend miss an
    /// invisible mage at disadvantage even though RAW the fiend should
    /// see right through the spell.
    pub fn has_truesight(&self) -> bool {
        self.senses
            .iter()
            .any(|s| matches!(s, SpecialSense::Truesight(_)))
            || self.has_condition(Condition::TrueSighted)
    }

    /// The widest envelope of `sense` this creature has, converted from
    /// the template's feet to grid tiles. `0` for a creature without
    /// the sense.
    ///
    /// Rounded down, so a 10-ft envelope is 4 tiles on the 2.5-ft grid
    /// rather than 4.5 of one. Takes a *matcher* rather than a
    /// `SpecialSense` value because the radius rides inside the enum
    /// variant — `SpecialSense::Blindsight(60)` and
    /// `SpecialSense::Blindsight(30)` are different values of the same
    /// sense, so equality is the wrong question and "which variant, and
    /// what radius" is the right one.
    fn sense_tiles(&self, extract: fn(&SpecialSense) -> Option<u32>) -> isize {
        self.senses
            .iter()
            .filter_map(|s| extract(s).map(|feet| (feet as f32 / TILE_FEET) as isize))
            .max()
            .unwrap_or(0)
    }

    /// The widest **blindsight** envelope this creature has, in tiles.
    ///
    /// 5e: "A creature with blindsight can perceive its surroundings
    /// without relying on sight, within a specific radius." The engine
    /// carried `SpecialSense::Blindsight(_)` on thirty-odd templates —
    /// bats, oozes, dragons, animated armor — and read it nowhere, so
    /// the sense that defines a bat was decoration.
    ///
    /// It now answers both questions in the engine that sight can fail
    /// on its own terms: whether heavy obscurement stops you seeing
    /// (`obscurement_blinds`), and whether an illusion does
    /// (`concealment_piercing_of`). The second is the RAW half that was
    /// missing — "without relying on sight" is exactly the clause that
    /// makes Invisibility useless against a bat, and the engine used to
    /// let an invisible mage walk past one.
    pub fn blindsight_tiles(&self) -> isize {
        self.sense_tiles(|s| match s {
            SpecialSense::Blindsight(feet) => Some(*feet),
            _ => None,
        })
    }

    /// The widest **darkvision** envelope this creature has, in tiles.
    ///
    /// 5e: "you can see in dim light within the radius as if it were
    /// bright light, and in darkness as if it were dim light. You can't
    /// discern colour in darkness, only shades of grey."
    ///
    /// The colour clause has no surface here. The rest is read by
    /// `EncounterInstance::perceived_light`, which is the one place the
    /// upgrade happens and therefore the one place that has to know the
    /// radius.
    ///
    /// This accessor is the reason the lighting layer exists at all.
    /// `SpecialSense::Darkvision(_)` was declared on some two hundred
    /// templates — every goblin, every dwarf, every devil — and read by
    /// nothing, so the sense that separates a kobold from a commoner in
    /// an unlit corridor was decoration. It is the same gap
    /// `blindsight_tiles` closed for blindsight, and it went unnoticed
    /// for the opposite reason: blindsight was unread because the
    /// engine had no invisibility to counter, and darkvision was unread
    /// because the engine had no darkness to see through.
    /// The **Darkvision** spell raises this to 60 feet and never lowers
    /// it — a `max` rather than an assignment, because RAW grants "out
    /// to a range of 60 feet" and a drow's own 120 is not something a
    /// 2nd-level slot can take away. It is also the only condition in
    /// the engine that grants a sense, which is why the fold happens
    /// here rather than by writing into `self.senses`: the sense map is
    /// copied from the template at instantiation and is not a place
    /// timers can safely be written.
    pub fn darkvision_tiles(&self) -> isize {
        let innate = self.sense_tiles(|s| match s {
            SpecialSense::Darkvision(feet) => Some(*feet),
            _ => None,
        });
        if self.has_condition(Condition::Darkvisioned) {
            return innate.max(crate::actions::spells::DARKVISION_SPELL_TILES);
        }
        innate
    }

    /// True if this creature sees normally in *magical* darkness — the
    /// 5e warlock invocation **Devil's Sight**, and the trait of the
    /// same name that every devil in the bestiary carries.
    ///
    /// The one counter RAW provides to the Darkness spell's "a creature
    /// with darkvision can't see through this darkness", and the reason
    /// the spell is a warlock signature rather than a coin flip: a
    /// warlock with the invocation drops a sphere on the melee and
    /// keeps shooting out of it while nothing inside can see them.
    ///
    /// Carried as a passive-feature tag rather than a `SpecialSense`
    /// because it has no radius that matters here — the invocation's
    /// 120 ft exceeds anything the engine's boards can put between two
    /// creatures — and because the warlock's other invocations already
    /// live in the same pool.
    pub fn has_devils_sight(&self) -> bool {
        self.has_passive_feature(crate::actions::class_features::DEVILS_SIGHT_TAG)
    }

    /// How badly this creature reacts to sunlight, if at all — see
    /// `crate::engine::lighting::SunlightFrailty`. `None` for the
    /// overwhelming majority of the bestiary.
    pub fn sunlight_frailty(&self) -> Option<SunlightFrailty> {
        self.sunlight_frailty
    }

    /// The widest **tremorsense** envelope this creature has, in tiles.
    ///
    /// 5e: "A creature with tremorsense can detect and pinpoint the
    /// origin of vibrations within a specific radius, provided that the
    /// creature and the source of the vibrations are in contact with
    /// the same ground."
    ///
    /// The trailing clause is the whole character of the sense and the
    /// reason it isn't just a second blindsight: it reads the floor, so
    /// anything off the floor is invisible to it. Callers pair this
    /// radius with `is_grounded` on the *subject* — a purple worm's
    /// 60-ft tremorsense pinpoints the invisible rogue standing on the
    /// sand and loses the wizard who cast Fly, which is the tactical
    /// answer RAW gives and the one the board should reward.
    ///
    /// Carried by the burrowers and the buried ambushers — Purple Worm,
    /// Tarrasque, Ankheg, Umber Hulk, Xorn, Chuul, Galeb Duhr — where
    /// it was declared and read nowhere until this accessor.
    pub fn tremorsense_tiles(&self) -> isize {
        self.sense_tiles(|s| match s {
            SpecialSense::Tremorsense(feet) => Some(*feet),
            _ => None,
        })
    }

    /// True while this actor is held aloft by magic — the `Fly` /
    /// `Investiture of Wind` / `Otherworldly Guise` cohort.
    ///
    /// The engine models no natural flight (a creature template's fly
    /// speed is folded into its single walking speed), so magical
    /// flight is the only way an actor leaves the floor under its own
    /// power, and this predicate is the whole of "airborne" on that
    /// axis. Any one source is sufficient: RAW the three are separate
    /// concentration spells one caster can't stack, so they're an OR
    /// rather than a sum.
    ///
    /// Named rather than inlined because four separate lanes ask it
    /// and must never disagree about the answer — the +60 ft speed row
    /// in `CONDITION_SPEED_BONUSES`, the difficult-terrain waiver in
    /// `DIFFICULT_TERRAIN_IMMUNITIES`, the ground-contact gate on
    /// tremorsense (`is_grounded`), and the Earthbind spell, which has
    /// to strip every one of them.
    pub fn has_magical_flight(&self) -> bool {
        MAGICAL_FLIGHT_CONDITIONS
            .iter()
            .any(|&c| self.has_condition(c))
    }

    /// True while this actor is in contact with the ground — the
    /// subject-side gate on tremorsense.
    ///
    /// Two ways off the floor, and they are the two the engine models:
    ///
    ///   - **Magical flight** (`has_magical_flight`) — the actor is
    ///     flying under its own concentration.
    ///   - **`Lifted`** (Telekinesis) — the actor is suspended in the
    ///     air by someone else's. RAW the spell "moves the creature up
    ///     to 30 feet in any direction, including upward", and the
    ///     condition's own docstring says "suspended in the air", so a
    ///     telekinetically-held target is exactly as unreadable to a
    ///     tremorsensing burrower as a flying one.
    ///
    /// Deliberately *not* gated on being Prone, Unconscious, or
    /// Restrained: all three leave the creature very much in contact
    /// with the ground, and a prone target is if anything easier to
    /// feel.
    pub fn is_grounded(&self) -> bool {
        !self.has_magical_flight() && !self.has_condition(Condition::Lifted)
    }

    pub fn team(&self) -> usize {
        self.team_id
    }

    pub fn ability_score(&self, ast: AbilityScoreType) -> u32 {
        match ast {
            AbilityScoreType::Strength => self.strength,
            AbilityScoreType::Intelligence => self.intelligence,
            AbilityScoreType::Dexterity => self.dexterity,
            AbilityScoreType::Wisdom => self.wisdom,
            AbilityScoreType::Constitution => self.constitution,
            AbilityScoreType::Charisma => self.charisma,
        }
    }

    /// True if any active condition's `blocks_action_economy` clause
    /// (Stunned / Incapacitated / Paralyzed / Unconscious) is set.
    pub fn is_incapacitated(&self) -> bool {
        self.conditions.keys().any(|c| c.blocks_action_economy())
    }

    /// Bonus tile-gap reach added by active conditions to the action's
    /// declared `reach_tiles()`. 5e Battle Master Lunging Attack is the
    /// canonical case (+5ft / +1 tile to the next melee swing). Read by
    /// `Action::validate_input` after the base reach lookup. The
    /// `base_reach` argument lets the helper gate the bonus to melee
    /// envelopes (<= 2 tile gap) so a ranged spell-attack from a primed
    /// fighter doesn't inherit the reach extension. Returns 0 when no
    /// rider is active.
    pub fn extra_melee_reach(&self, base_reach: isize) -> isize {
        // Only melee / touch / polearm-reach actions benefit. Ranged
        // spell-attacks have base_reach >> 2 (e.g. Fire Bolt = 48 tiles)
        // so the gate cuts them out cleanly.
        if base_reach > 2 {
            return 0;
        }
        let mut bonus = 0;
        if self.has_condition(Condition::LungingAttacking) {
            bonus += 1;
        }
        bonus
    }

    /// Bonus tile-gap reach added by active conditions to the action's
    /// declared `reach_tiles()` for *ranged* actions (5e Sorcerer Distant
    /// Spell metamagic). RAW: "When you cast a spell that has a range of
    /// 5 feet or greater, you can spend 1 sorcery point to double the
    /// range of the spell." We model this by returning `base_reach` as
    /// the bonus — adding the base to itself doubles it. Gated to ranged
    /// envelopes (base_reach > 2) so a melee weapon swing or polearm
    /// reach attack can't burn the prime — the LungingAttacking branch
    /// in `extra_melee_reach` covers those.
    pub fn extra_spell_reach(&self, base_reach: isize) -> isize {
        if !self.has_condition(Condition::DistantSpelling) {
            return 0;
        }
        if base_reach <= 2 {
            return 0;
        }
        base_reach
    }

    /// How far this actor can actually reach with an action that
    /// declares `base_reach` — the declared reach plus whatever the
    /// active reach-extending primes add.
    ///
    /// The two contributing lanes are disjoint by construction:
    /// `extra_melee_reach` refuses anything past a 2-tile envelope and
    /// `extra_spell_reach` refuses anything inside one, so the sum is
    /// always exactly one of them or zero. Summing rather than
    /// branching keeps the caller from having to know which lane an
    /// action belongs to, which is the whole reason this exists.
    ///
    /// It exists because the formula had two homes and they drifted.
    /// `Action::validate_input` added both bonuses; the AI's attack
    /// picker compared `dist > reach` against the *declared* reach and
    /// dropped every candidate the primes had just made legal. The
    /// visible consequence was that a Battle Master's Lunging Attack
    /// could never be cashed: the AI's own picker primes it when an
    /// enemy stands at exactly the gap the lunge opens, and then
    /// refused to consider a single weapon against that enemy — so the
    /// prime was spent, every time, on a swing that never happened.
    /// Distant Spell had the same shape on the ranged lane.
    ///
    /// One formula, two callers, and a new reach-extending prime lands
    /// as a row inside one of the two helpers rather than as an edit in
    /// both places that have to agree.
    pub fn extra_reach(&self, base_reach: isize) -> isize {
        self.extra_melee_reach(base_reach) + self.extra_spell_reach(base_reach)
    }

    pub fn can_consume_resource(&self, resource: Resource) -> bool {
        let action_blocked = self.is_incapacitated();
        match resource {
            Resource::Movement(amt) => {
                if action_blocked {
                    return false;
                }
                // `remaining_movement`, not the raw `movement` field —
                // the two are the same number for the overwhelming
                // majority of actors and deliberately differ for the
                // ones RAW says cannot move at all. Reading the field
                // meant this gate saw only the `zeros_movement`
                // conditions and missed the sixth rung of exhaustion,
                // whose whole sentence is "speed reduced to 0": a
                // creature that far gone could still buy a stand-up, a
                // mount or a dismount, because those price themselves
                // in feet and ask here rather than asking the
                // pathfinder.
                amt <= self.remaining_movement()
            }
            Resource::SpellSlot(spell_lvl) => {
                if action_blocked {
                    return false;
                }
                // 5e Silence: holders inside the magical-silence sphere
                // can't cast spells with verbal components (RAW). We
                // approximate by blocking *all* leveled spells, since
                // every leveled SRD spell has a V component by default
                // and the few S-only outliers are non-combat utility.
                // Cantrips are unaffected (no SpellSlot cost).
                if self.conditions.keys().any(|c| c.blocks_spell_slots()) {
                    return false;
                }
                self.spell_slot_manager.spell_slots(spell_lvl).spell_slots >= 1
            }
            Resource::Action => !action_blocked && self.action_slots >= 1,
            Resource::BonusAction => !action_blocked && self.bonus_action_slots >= 1,
            // 5e Shocking Grasp & similar lockout effects: any condition
            // whose `blocks_reactions` clause is true (NoReaction,
            // Confused) silences the reaction lane. Stacks with the
            // Incapacitated family which already zeroes them.
            Resource::Reaction => {
                !action_blocked
                    && !self.conditions.keys().any(|c| c.blocks_reactions())
                    && self.reaction_slots >= 1
            }
            Resource::LegendaryAction => !action_blocked && self.legendary_action_slots >= 1,
        }
    }

    /// True iff this actor could actually SPEND a reaction right now.
    /// Delegates to `can_consume_resource(Resource::Reaction)` so the
    /// availability check and the actual consume path share ONE source
    /// of truth — pre-refactor this shortcut skipped the
    /// `blocks_action_economy` cohort (Stunned / Paralyzed / Incapacitated
    /// / Unconscious / Asleep / Petrified / Mazed / Sphered), meaning a
    /// stunned rogue's Uncanny Dodge would halve incoming damage while
    /// the follow-up `consume_resource` silently no-op'd (reaction slot
    /// wasn't taxed). Read at every reactive-defense gate:
    /// Uncanny Dodge, Deflect Missiles, Protection style, etc.
    pub fn has_reaction(&self) -> bool {
        self.can_consume_resource(Resource::Reaction)
    }

    pub fn consume_resource(&mut self, resource: Resource) -> bool {
        if !self.can_consume_resource(resource) {
            return false;
        }
        match resource {
            Resource::Movement(amt) => {
                self.movement -= amt;
                self.movement_spent_this_turn += amt;
            }
            Resource::SpellSlot(lvl) => {
                self.spell_slot_manager.consume_spell_slot(lvl);
            }
            Resource::Action => self.action_slots -= 1,
            Resource::BonusAction => self.bonus_action_slots -= 1,
            Resource::Reaction => self.reaction_slots -= 1,
            Resource::LegendaryAction => self.legendary_action_slots -= 1,
        }
        true
    }

    pub fn give_resource(&mut self, resource: Resource) {
        match resource {
            Resource::Movement(amt) => self.movement += amt,
            Resource::SpellSlot(lvl) => {
                self.spell_slot_manager.restore_spell_slot(lvl, 1);
            }
            Resource::Action => self.action_slots += 1,
            Resource::BonusAction => self.bonus_action_slots += 1,
            Resource::Reaction => self.reaction_slots += 1,
            Resource::LegendaryAction => self.legendary_action_slots += 1,
        }
    }

    pub fn armor_class(&self) -> u32 {
        // AC-floor conditions (Mage Armor → 13 + DEX, Barkskin → 16) act
        // as a minimum AC: the caster gets the better of their raw base
        // and the floor. They don't stack with worn armor RAW, but a
        // floor lets the caster benefit when their base AC is lower.
        // The condition AC bonus (Shield, Shield of Faith, Hasted, etc.)
        // stacks on top of whichever number wins.
        //
        // 5e Fighting Style: Defense (+1 AC while wearing armor) reads
        // here as a flat +1 on the base AC lane — we don't model armor
        // tiers so the RAW "while wearing armor" gate collapses to
        // "always on" for any holder of the flag. Applied to the raw
        // base *before* the floor comparison so an AC-floor buff
        // (Mage Armor / Barkskin) still overrides a lower armored AC.
        let defense_bonus = if self.has_defense_style { 1 } else { 0 };
        let raw_base = self.base_ac as i32 + self.total_item_bonuses().ac + defense_bonus;
        let floor = self.ac_floor();
        (raw_base.max(floor) + self.condition_ac_bonus()).max(0) as u32
    }

    /// Flat AC contribution from active conditions — Shield of Faith
    /// (+2 from the spell), Shield (+5 from the reaction spell, RAW
    /// value), Haste (+2), Slow (-2), Warding Bond (+1), and Tasha's
    /// Otherworldly Guise (+2). Walks the shared `CONDITION_AC_BONUSES`
    /// cohort — each row is a `ConditionAcBonus { source, bonus }`;
    /// every held source contributes its signed `bonus` to the sum, so
    /// vertical stacking (Shield of Faith + Shielded + Hasted → +9 AC)
    /// folds through the same walk as horizontal stacking (Hasted +
    /// Slowed → 0 AC — the two riders cancel). Adding a future
    /// condition-driven AC bump (a hypothetical Sanctuary +4 AC on the
    /// caster, a Blur-adjacent visual-obscurement AC bump, etc.) lands
    /// as a one-line entry in the cohort rather than another
    /// `if self.has_condition(...) { bonus += N; }` branch here.
    ///
    /// Sibling to `condition_speed_bonus_from_table` on the
    /// condition-driven cohort-walk lane — same filter-map-sum shape
    /// (walk the cohort, sum every held row's magnitude), different
    /// affected axis (AC points here vs. feet there). Mage Armored
    /// (AC-floor lane, not a flat bump) is handled by `ac_floor`, not
    /// this cohort — the two lanes are compositional (floor wins over
    /// base, then this bonus stacks on top).
    pub fn condition_ac_bonus(&self) -> i32 {
        let flat: i32 = CONDITION_AC_BONUSES
            .iter()
            .filter(|row| self.has_condition(row.source))
            .map(|row| row.bonus)
            .sum();
        flat + self.ability_scaled_bonus(ABILITY_SCALED_AC_BONUSES)
    }

    /// Sum of every held row in an ability-scaled condition-bonus
    /// cohort. Shared by `condition_ac_bonus` (Bladesong's +INT) and
    /// `condition_attack_bonus` (Sacred Weapon's +CHA) — the two lanes
    /// differ only in which table they hand in.
    fn ability_scaled_bonus(&self, rows: &[AbilityScaledConditionBonus]) -> i32 {
        rows.iter()
            .filter(|row| self.has_condition(row.source))
            .map(|row| self.ability_modifier(row.ability).max(row.floor))
            .sum()
    }

    /// Effective AC floor from active AC-setting conditions. Mage Armor
    /// floors at `13 + DEX`; Barkskin floors at 16. The maximum across
    /// every active floor wins so the holder takes the highest qualifying
    /// minimum — RAW: "Barkskin / Mage Armor don't stack with each other
    /// or with worn armor; pick the best." Returns 0 when no floor is
    /// active so `armor_class` falls back to base AC unmodified.
    pub fn ac_floor(&self) -> i32 {
        let mut floor = 0;
        if self.has_condition(Condition::MageArmored) {
            floor = floor.max(13 + modifier_from_score(self.dexterity));
        }
        if self.has_condition(Condition::Barkskinned) {
            floor = floor.max(16);
        }
        floor
    }

    pub fn hitpoints(&self) -> u32 {
        self.hitpoints
    }

    pub fn max_hitpoints(&self) -> u32 {
        let bonus = self.total_item_bonuses().max_hp;
        let raw = (self.base_hitpoints as i32 + bonus).max(1) as u32;
        // 5e exhaustion tier 4: "hit point maximum halved". Applied
        // after the item bonuses fold in, so a Ring of Regeneration's
        // +HP is halved along with everything else — RAW halves the
        // maximum, whatever built it. Floors at 1 so a halved maximum
        // can never itself be the thing that kills; tier 6 is the rung
        // that does that.
        if self.exhaustion >= EXHAUSTION_HALF_HP_TIER {
            return (raw / 2).max(1);
        }
        raw
    }

    /// True if the actor has taken any damage relative to their full HP
    /// pool. Centralizes the recurring `hitpoints() < max_hitpoints()`
    /// check so wounded-creature riders (Sahuagin Blood Frenzy advantage,
    /// future "bloodied" predicates) read from one chokepoint and a
    /// future redefinition of "wounded" (e.g. half-HP threshold) lands
    /// in one place instead of being scattered across call sites.
    pub fn is_wounded(&self) -> bool {
        self.hitpoints() < self.max_hitpoints()
    }

    /// Permanently bump the actor's max HP by `delta`. Current HP rises
    /// by the same amount so the boost is immediately useful (matches
    /// 5e's Aid spell semantics: "their hit point maximum and current
    /// hit points increase by 5"). Use a negative delta to apply a
    /// max-HP penalty (e.g. exhaustion); the floor is 1 max HP.
    pub fn bump_max_hp(&mut self, delta: i32) {
        let new_base = (self.base_hitpoints as i32 + delta).max(1) as u32;
        let added = new_base.saturating_sub(self.base_hitpoints);
        self.base_hitpoints = new_base;
        if added > 0 {
            let cap = self.max_hitpoints();
            self.hitpoints = self.hitpoints.saturating_add(added).min(cap);
        } else {
            // On a downward bump, never exceed the new cap.
            self.hitpoints = self.hitpoints.min(self.max_hitpoints());
        }
    }

    pub fn speed(&self) -> f32 {
        let bonus = self.total_item_bonuses().speed as f32;
        let raw = (self.base_speed + bonus + self.condition_speed_bonus()).max(0.0);
        // Multiplicative speed factors (Haste ×2, Slow ×½, Power Word
        // Pain ×½, …) compose via the shared `CONDITION_SPEED_MULTIPLIERS`
        // table so a new speed multiplier lands as a one-line entry.
        // `.product()` on an empty iterator returns 1.0 (identity for
        // multiplication), so a target with no matching row keeps its
        // additive-only speed unchanged. Cross-cast composition stays
        // symmetric: Haste + Slow ⇒ ×2 · ×½ = ×1 back to base, Slow +
        // Power Word Pain ⇒ ×½ · ×½ = ×¼ if both land on the same
        // target through separate concentration chains.
        let factor: f32 = CONDITION_SPEED_MULTIPLIERS
            .iter()
            .filter(|row| (row.flag)(self))
            .map(|row| row.factor)
            .product();
        raw * factor
    }

    /// Sum of all flat speed bonuses contributed by active conditions
    /// AND passive-feature tags. One chokepoint so a new speed-buff
    /// source (Longstrider, Expeditious Retreat, Fly, Spider Climb,
    /// Investiture of Wind, Tiger Totem, Fast Movement, …) lands as a
    /// one-line entry in the appropriate cohort table
    /// (`CONDITION_SPEED_BONUSES` for condition-driven,
    /// `PASSIVE_FEATURE_SPEED_BONUSES` for tag-driven) instead of an
    /// ad-hoc branch in `speed()`.
    ///
    /// Body is a two-line compose of the two cohort helpers. Each
    /// cohort is summed independently (a matching row contributes its
    /// `bonus_ft`; misses contribute 0), then the two totals are added
    /// so a raging Tiger totem barbarian under Longstrider picks up
    /// +10 (Tiger) + +10 (Longstrider) + Fast Movement's +10 =
    /// +30 ft over base.
    ///
    /// Returned in feet so it composes with `base_speed` / item bonuses
    /// before the Haste / Slow multiplicative factor in `speed()`.
    pub fn condition_speed_bonus(&self) -> f32 {
        // Condition-driven speed bumps (Fly / Investiture / Otherworldly
        // Guise, Spider Climb, Longstrider, Expeditious Retreat,
        // Ashardalon's Stride) fold through the shared
        // `CONDITION_SPEED_BONUSES` table so adding a new
        // condition-driven speed buff lands as a one-line entry
        // instead of another `if self.has_condition(...) { bonus += N; }`
        // here.
        //
        // Passive-feature-driven speed bumps (Tiger / Elk / Wolverine /
        // Panther Totem, Fast Movement, Unarmored Movement, Roving)
        // fold through the sibling `PASSIVE_FEATURE_SPEED_BONUSES`
        // table for the same reason.
        self.condition_speed_bonus_from_table() + self.passive_feature_speed_bonus()
    }

    /// Sum of flat speed bonuses granted by held conditions. Walks the
    /// `CONDITION_SPEED_BONUSES` cohort — every row's flag is
    /// evaluated against `self`, and matching rows contribute their
    /// `bonus_ft` to the total. Adding a fresh condition-driven speed
    /// buff lands as a one-line entry in that table rather than a new
    /// `if self.has_condition(...) { bonus += N; }` branch here.
    ///
    /// Sibling to `passive_feature_speed_bonus` — both are
    /// filter-map-sum walks over their respective cohort tables and
    /// both are composed by `condition_speed_bonus` before the Haste /
    /// Slow multiplicative factor lands in `speed()`.
    fn condition_speed_bonus_from_table(&self) -> f32 {
        CONDITION_SPEED_BONUSES
            .iter()
            .filter(|row| (row.flag)(self))
            .map(|row| row.bonus_ft)
            .sum()
    }

    /// Sum of flat speed bonuses granted by passive-feature tags. Walks
    /// the `PASSIVE_FEATURE_SPEED_BONUSES` cohort — every row's flag is
    /// evaluated against `self`, and matching rows contribute their
    /// `bonus_ft` to the total. Adding a fresh always-on / rage-gated
    /// speed passive lands as a one-line entry in that table rather
    /// than a new `if actor.has_passive_feature(...) { bonus += N; }`
    /// branch here.
    ///
    /// Returned in feet so the caller (`condition_speed_bonus`)
    /// composes it with the condition-keyed bumps before the
    /// Haste / Slow multiplicative factor lands in `speed()`.
    fn passive_feature_speed_bonus(&self) -> f32 {
        PASSIVE_FEATURE_SPEED_BONUSES
            .iter()
            .filter(|row| (row.flag)(self))
            .map(|row| row.bonus_ft)
            .sum()
    }

    /// True if this actor pays no movement surcharge for the terrain it
    /// crosses — `TerrainType::DifficultTerrain`, and the clamber over a
    /// `TerrainType::LowWall`, which costs the same and is waived by the
    /// same things. Read once per path by the pathfinder
    /// (`EncounterInstance::dijkstra_path`), which skips the terrain
    /// multiplier entirely when it holds.
    ///
    /// Any one matching row in `DIFFICULT_TERRAIN_IMMUNITIES` is enough
    /// — the surcharge is either charged or it isn't, so unlike the two
    /// speed-bonus cohorts (which sum) this one short-circuits on the
    /// first hit. Adding a fresh source (a Boots of Striding rider, the
    /// 2024 Circle of the Land's terrain grant, a monster's burrow
    /// speed) lands as a one-line entry in that table rather than a new
    /// branch here or — worse — in the pathfinder's inner loop.
    pub fn ignores_difficult_terrain(&self) -> bool {
        self.matches_any(DIFFICULT_TERRAIN_IMMUNITIES)
    }

    /// True if any row in `cohort` holds for this actor — the shared
    /// read for every `ActorFlagRow` table.
    ///
    /// One line of body, and worth naming anyway: it is the sentence
    /// "one matching source is enough", which is the rule that makes a
    /// boolean cohort a cohort rather than a list. The three readers
    /// that call it had three identical `.iter().any(|row| (row.flag)(self))`
    /// bodies, which is exactly the number at which the next one gets
    /// written slightly differently.
    fn matches_any(&self, cohort: &[ActorFlagRow]) -> bool {
        cohort.iter().any(|row| (row.flag)(self))
    }

    /// True if this actor has a swimming speed — natural (the tag on an
    /// aquatic template) or granted (Scout Rogue's Superior Mobility).
    ///
    /// The narrow question, and the one 5e's melee Underwater Combat
    /// clause asks by name: "a creature that doesn't have a swimming
    /// speed (either natural or granted by magic) has disadvantage on
    /// the attack roll." Deliberately *not* satisfied by flight or by
    /// Freedom of Movement — both of those exempt an attacker from the
    /// water's penalties, and neither is a swimming speed. See
    /// `SWIM_SPEED_SOURCES`.
    pub fn has_swim_speed(&self) -> bool {
        self.matches_any(SWIM_SPEED_SOURCES)
    }

    /// True if this actor crosses `TerrainType::Water` at no movement
    /// surcharge — the water-side twin of `ignores_difficult_terrain`,
    /// read once per path by `EncounterInstance::dijkstra_path`.
    ///
    /// Wider than `has_swim_speed`: a creature does not have to be
    /// swimming to cross a lake for free, it merely has to not be
    /// paying for it. Flight and Freedom of Movement both qualify. See
    /// `WATER_SURCHARGE_IMMUNITIES` for why the two cohorts are not one.
    pub fn swims_freely(&self) -> bool {
        self.matches_any(WATER_SURCHARGE_IMMUNITIES)
    }

    /// True if 5e's Underwater Combat penalties are waived for this
    /// actor outright, whatever weapon they are holding and whichever
    /// half of the rule is being asked about.
    ///
    /// Exactly one thing does that, and it does it in so many words:
    /// **Freedom of Movement** — "being underwater imposes no penalties
    /// on the target's movement or attacks." A swimming speed is not
    /// enough (it saves the swing and not the shot) and neither is
    /// flight (a flying creature is exempt because it is not in the
    /// water at all, which `is_immersed` answers a step earlier).
    ///
    /// A named predicate on a cohort of one, because the alternative is
    /// a bare `has_condition(Footloose)` at the attack site with a
    /// comment explaining which of Freedom of Movement's four clauses
    /// is being read — and because the next effect that grants this
    /// (a Cap of Water Breathing rider, a Fathomless invocation) should
    /// land as a row rather than as a second `||` somewhere in
    /// `engine::attack`.
    pub fn underwater_penalties_waived(&self) -> bool {
        self.has_condition(Condition::Footloose)
    }

    pub fn item_save_bonus(&self) -> i32 {
        self.total_item_bonuses().save
    }

    /// Sum of every carried item's `attack_bonus` field. Folded into the
    /// caster-side attack-roll buff lane via
    /// `EncounterInstance::caster_attack_buffs` so weapon swings AND spell
    /// attacks both pick up the passive without the call sites re-summing
    /// the inventory. Symmetric with `item_save_bonus` on the save lane.
    pub fn item_attack_bonus(&self) -> i32 {
        self.total_item_bonuses().attack_bonus
    }

    /// Sum of every carried item's `damage_bonus` field. Folded into the
    /// damage-roll site in `engine::attack` / spell-attack chokepoint so
    /// `+N weapon`-style items pick up their +N damage half once per swing.
    pub fn item_damage_bonus(&self) -> i32 {
        self.total_item_bonuses().damage_bonus
    }

    /// Flat to-hit bonus contributed only by *conditions* whose dice
    /// aren't already represented elsewhere. Bless / Bane install a
    /// separate `attack_bonus_buff` delta on top of the d4 die roll
    /// (`bless_bane_attack_die`), so they're intentionally excluded
    /// from this lane — including them here would double-count. This
    /// lane is reserved for condition-only flat bonuses (Sacred
    /// Weapon: +CHA, Bardic Inspiration: +3 d6-average, Precision
    /// Attack: +4 d8-average, Guided Strike: +10).
    ///
    /// Walks the shared `CONDITION_ATTACK_BONUSES` cohort — each row is
    /// a `ConditionAttackBonus { source, bonus }`; every held source
    /// contributes its signed `bonus` to the sum, matching the
    /// filter-map-sum shape used by `condition_save_bonus` /
    /// `condition_ac_bonus` on their respective sibling cohorts.
    /// Adding a future flat condition-driven attack bump (a hypothetical
    /// Sanctuary-broken +N attack rider, a future Mark of the Hunter +N
    /// attack aura, etc.) lands as a one-line entry in the cohort
    /// rather than another `if self.has_condition(...) { bonus += N; }`
    /// branch here.
    ///
    /// Sacred Weapon (`Condition::Sacred`, +CHA modifier) stays inline
    /// below the cohort walk because its magnitude reads the holder's
    /// CHA mod at attack time, while every cohort row carries a
    /// compile-time flat integer. The exclusion mirrors how Bless /
    /// Bane are intentionally left off the cohort — the pattern here is
    /// "shape mismatch" (variable vs. flat magnitude) rather than "lane
    /// mismatch" (`attack_bonus_buff` double-count). A future
    /// closure-based sibling cohort could absorb variable-magnitude
    /// riders if a second entry appears — a single Sacred outlier
    /// doesn't justify widening the row shape today.
    ///
    /// Sibling to `condition_save_bonus` / `condition_ac_bonus` on the
    /// condition-driven cohort-walk lane — same filter-map-sum shape
    /// (walk the cohort, sum every held row's magnitude), different
    /// affected axis (attack roll here vs. save / AC there).
    pub fn condition_attack_bonus(&self) -> i32 {
        let mut bonus: i32 = CONDITION_ATTACK_BONUSES
            .iter()
            .filter(|row| self.has_condition(row.source))
            .map(|row| row.bonus)
            .sum();
        // Ability-scaled riders — today just Channel Divinity: Sacred
        // Weapon, whose +CHA is read off the holder's own stat block so
        // a monster who somehow grabs the buff scales off theirs. Used
        // to be a hand-written branch here; it now shares
        // `ABILITY_SCALED_ATTACK_BONUSES` with the Bladesinger's +INT
        // AC row on the sibling cohort, which is what the old comment
        // here said should happen once a second such rider existed.
        bonus += self.ability_scaled_bonus(ABILITY_SCALED_ATTACK_BONUSES);
        bonus
    }

    /// Symmetric save-roll counterpart to `condition_attack_bonus`. Same
    /// rationale for excluding Bless / Bane: their +2 / -2 lives on
    /// `save_bonus_buff` and their d4 die on `bless_bane_attack_die`,
    /// so this lane is condition-only flat bonuses (Bardic
    /// Inspiration: +3 d6-average; Warding Bond: +1 to saves while
    /// bonded). Walks the shared `CONDITION_SAVE_BONUSES` cohort —
    /// each row is a `ConditionRollBonus { source, bonus }`; every
    /// held source contributes its signed `bonus` to the sum, so
    /// vertical stacking (Inspired + WardingBonded → +4 saves) folds
    /// through the same walk as horizontal stacking on the sibling
    /// `CONDITION_AC_BONUSES` cohort. Adding a future condition-driven
    /// save bump (a hypothetical Sanctuary +N save on the caster, a
    /// Bard Song of Rest save aura, etc.) lands as a one-line entry in
    /// the cohort rather than another `if self.has_condition(...)
    /// { bonus += N; }` branch here.
    ///
    /// Sibling to `condition_ac_bonus` on the condition-driven
    /// cohort-walk lane — same filter-map-sum shape (walk the cohort,
    /// sum every held row's magnitude), different affected axis (save
    /// points here vs. AC points there).
    pub fn condition_save_bonus(&self) -> i32 {
        CONDITION_SAVE_BONUSES
            .iter()
            .filter(|row| self.has_condition(row.source))
            .map(|row| row.bonus)
            .sum()
    }

    /// Flat ability-check delta from active conditions — the check-lane
    /// sibling of `condition_save_bonus`, walking
    /// `CONDITION_CHECK_BONUSES` with the identical shape. Read by
    /// `EncounterInstance::roll_ability_check`, which then burns the
    /// one-shot rows through `CONSUMED_ON_CHECK`.
    pub fn condition_check_bonus(&self) -> i32 {
        CONDITION_CHECK_BONUSES
            .iter()
            .filter(|row| self.has_condition(row.source))
            .map(|row| row.bonus)
            .sum()
    }

    pub fn attack_bonus_buff(&self) -> i32 {
        self.attack_bonus_buff
    }

    pub fn save_bonus_buff(&self) -> i32 {
        self.save_bonus_buff
    }

    /// Spell-installed flat damage-roll buff (Magic Weapon, Elemental
    /// Weapon). Folded into every damage roll via `caster_damage_buffs`
    /// alongside the item-side `damage_bonus` lane.
    pub fn damage_bonus_buff(&self) -> i32 {
        self.damage_bonus_buff
    }

    pub fn add_attack_bonus_buff(&mut self, delta: i32) {
        self.attack_bonus_buff += delta;
    }

    pub fn add_save_bonus_buff(&mut self, delta: i32) {
        self.save_bonus_buff += delta;
    }

    pub fn add_damage_bonus_buff(&mut self, delta: i32) {
        self.damage_bonus_buff += delta;
    }

    pub fn remaining_movement(&self) -> f32 {
        if self.conditions.keys().any(|c| c.zeros_movement()) {
            return 0.0;
        }
        // 5e exhaustion tier 5: "speed reduced to 0". Read here rather
        // than as another `CONDITION_SPEED_MULTIPLIERS` row with a
        // factor of 0, because those factors scale the *speed* that
        // fills the budget at turn start, and a Dash pours a second
        // helping straight back in. A creature this exhausted does not
        // get to sprint; it does not get to move.
        if self.exhaustion >= EXHAUSTION_ZERO_SPEED_TIER {
            return 0.0;
        }
        // Prone is deliberately *not* here, and it used to be. 5e's
        // crawl is "every foot of movement costs 1 extra foot", which
        // the pathfinder charges per tile through its `prone_factor` —
        // and halving the budget here as well charged it twice, so a
        // prone creature with 30 ft of speed crawled 7.5 feet instead
        // of 15. The sibling test has said the budget stays intact
        // since the pathfinder learned the rule; this is the half that
        // never got taken back out.
        //
        // Which is also the RAW-correct reading of standing up. RAW
        // prices that at "half your speed" out of the *full* budget: a
        // 30-ft creature pays 15 to stand and walks the other 15. With
        // the budget pre-halved it paid 15 out of 15 and stood up with
        // nothing left.
        self.movement
    }

    /// 5e Tasha's Rogue Steady Aim gate. True iff the actor has spent any
    /// movement this turn.
    ///
    /// Reads the spend counter rather than comparing the remaining
    /// budget against `speed()`, which is what it used to do and which
    /// disagreed with itself the moment anything touched either side of
    /// that comparison mid-turn — see `movement_spent_this_turn`.
    ///
    /// "Spent", not "displaced": a creature shoved across the board by
    /// Thunderwave has not moved for this purpose, and RAW agrees —
    /// forced movement is something that happens to you.
    pub fn has_moved_this_turn(&self) -> bool {
        self.movement_spent_this_turn > 0.0
    }

    /// Feet of movement spent so far this turn. Public for the charge
    /// gate, which needs "did it run" separately from "where did it end
    /// up".
    pub fn movement_spent_this_turn(&self) -> f32 {
        self.movement_spent_this_turn
    }

    /// Drain the actor's remaining movement budget to zero. Used by
    /// Steady Aim (RAW: "after you use the bonus action, your speed is 0
    /// until the end of the current turn"). Direct setter rather than a
    /// `consume_resource(Resource::Movement(remaining))` chain so the
    /// "zero everything regardless of conditions" semantics is explicit.
    pub fn zero_movement(&mut self) {
        // Booked as spent rather than simply discarded. The budget is
        // gone either way, and every gate that asks `has_moved_this_turn`
        // is really asking "can this actor still be somewhere else by the
        // end of the turn" — for which a drained budget and a walked one
        // are the same answer. Steady Aim, the one caller, relies on it:
        // RAW locks the rogue in place for the rest of the turn.
        self.movement_spent_this_turn += self.movement.max(0.0);
        self.movement = 0.0;
    }

    /// Replace this turn's movement budget outright, without booking the
    /// difference as spent.
    ///
    /// The distinction from `zero_movement` — and the reason this isn't
    /// that plus a `give_resource` — is `movement_spent_this_turn`. That
    /// counter answers "has this creature moved yet", which a rider who
    /// has just been handed their mount's speed has not. Draining and
    /// refilling would tell every once-per-turn gate that reads it (the
    /// rogue's Steady Aim, the charge clauses) that the turn's walking
    /// had already happened.
    ///
    /// Sole caller is `EncounterInstance::grant_mounted_movement`, which
    /// is the one place in the engine where the legs a creature walks on
    /// are not its own.
    pub(crate) fn set_movement_budget(&mut self, feet: f32) {
        self.movement = feet;
    }

    /// The actor's size *as the board currently sees it*. Every footprint
    /// calculation in the engine — reach, LOS, spawn room, the actor map
    /// stamp — reads this one value, so it is deliberately a plain field
    /// rather than a derivation: a size the geometry hasn't been told
    /// about is worse than no size change at all.
    ///
    /// Growth and shrink effects therefore do not write here. They install
    /// a condition, `desired_size` reports what that condition asks for,
    /// and `EncounterInstance::reconcile_footprints` is the one place
    /// allowed to move the field — because it is the only place that can
    /// check whether the tiles are free and restamp the map in the same
    /// breath.
    pub fn size(&self) -> Size {
        self.size
    }

    /// The size this actor's template shipped with, before any growth or
    /// shrink effect. `size()` returns to this the moment the last
    /// resizing condition drops.
    pub fn base_size(&self) -> Size {
        self.base_size
    }

    /// The size the actor's current conditions ask for: one category up
    /// from `base_size` per growth effect held, one down per shrink
    /// effect. Growth and shrink cancel — RAW says the two spells "have
    /// no effect on a creature already under the other's influence", and
    /// netting the two ladders is the same answer with no ordering
    /// question.
    ///
    /// Only ever a *request*. The board may not have room, in which case
    /// `size()` stays where it is and the reconciler retries on the next
    /// pump — a creature hemmed in by a wall grows the moment the wall
    /// stops being the problem, which is what RAW's "if there is enough
    /// room" clause means over a whole combat rather than at one instant.
    pub fn desired_size(&self) -> Size {
        let step = RESIZING_CONDITIONS
            .iter()
            .filter(|entry| {
                self.has_condition(entry.condition)
                    && entry.holder_gate.is_none_or(|gate| gate(self))
            })
            .map(|entry| entry.steps)
            .sum::<i32>();
        if step == 0 {
            return self.base_size;
        }
        Size::from_ordinal(self.base_size.ordinal() + step.clamp(-1, 1))
    }

    /// Move the actor's effective size. Engine-only: the caller is
    /// responsible for having cleared the old footprint from the actor map
    /// and for stamping the new one, which is why nothing outside
    /// `EncounterInstance::reconcile_footprints` calls this.
    pub fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    pub fn creature_type(&self) -> CreatureType {
        self.creature_type
    }

    /// Raw location write. Ends any straight run: a creature that is
    /// simply *put* somewhere has not walked there, and the charge
    /// clauses that read the run all say "if the creature moves".
    ///
    /// Failing closed here rather than at the handful of shove / pull /
    /// teleport sites is the whole reason the break lives on the setter:
    /// the cost of a site that forgets is a charge fired off movement
    /// the creature never made, and the cost of one break too many is a
    /// charge that doesn't fire.
    pub fn set_location(&mut self, target: Coordinate) {
        self.location = target;
        self.break_run();
    }

    /// One tile travelled under the creature's own power, extending the
    /// straight run rather than ending it. The sibling of
    /// `set_location`, and the narrower of the two: `EncounterInstance`
    /// routes only `MoveActor`'s per-tile walk here.
    pub fn walk_to(&mut self, target: Coordinate) {
        let from = self.location;
        self.location = target;
        self.note_walked_step(from, target);
    }

    pub fn location(&self) -> Coordinate {
        self.location
    }

    /// Where the actor's current straight run began. Equal to its
    /// location whenever it isn't running.
    pub fn run_origin(&self) -> Coordinate {
        self.run_origin
    }

    /// End any run in progress. Called at the top of every turn and by
    /// every location change that isn't the creature walking — a shove,
    /// a pull, a teleport, a summon's placement.
    ///
    /// Failing *closed* is the point: the cost of forgetting to call
    /// this somewhere is a charge that fires off movement the creature
    /// didn't make, and the cost of calling it once too often is a
    /// charge that doesn't fire. Between those, the second.
    pub fn break_run(&mut self) {
        self.run_origin = self.location;
        self.run_step = None;
    }

    /// Record one walked tile, extending the current straight run or
    /// starting a new one. `from` and `to` are adjacent on the one path
    /// that calls this; anything else re-anchors rather than trying to
    /// interpret a jump.
    fn note_walked_step(&mut self, from: Coordinate, to: Coordinate) {
        let delta = to - from;
        let unit = Coordinate::new(delta.x.signum(), delta.y.signum());
        if delta == unit && self.run_step == Some(unit) {
            return;
        }
        // A turn, or a jump: this step is the first of a new run.
        self.run_origin = from;
        self.run_step = if delta == unit { Some(unit) } else { None };
    }

    /// This creature's 5e Charge / Pounce clause, if it has one.
    pub fn charge(&self) -> Option<ChargeRider> {
        self.charge
    }

    /// Whether this creature's anatomy admits a rider at all. See
    /// `CreatureTemplate::mountable` — the size and willingness halves
    /// of RAW's gate are `EncounterInstance::can_mount`'s.
    pub fn is_mountable(&self) -> bool {
        self.mountable
    }

    /// The creature this actor is riding, if any.
    pub fn mounted_on(&self) -> Option<usize> {
        self.mounted_on
    }

    /// The creature riding this actor, if any.
    pub fn ridden_by(&self) -> Option<usize> {
        self.ridden_by
    }

    /// Forget whatever rider/mount link this actor is holding, without
    /// touching the board.
    ///
    /// The one legitimate use for a one-sided write, and it is not a
    /// rule: it is for an actor being lifted out of one encounter and
    /// dropped into another. Ids are per-encounter, so a link carried
    /// across names a creature that no longer exists — or, worse, one
    /// that now does and isn't a horse. `EncounterInstance::with_pcs`
    /// is the sole caller and it re-stamps every survivor onto the new
    /// grid immediately after, which is what makes cutting the link
    /// without landing anybody safe here and nowhere else.
    ///
    /// Ending a ride *inside* a live encounter is `dismount`.
    pub fn clear_ride_links(&mut self) {
        self.mounted_on = None;
        self.ridden_by = None;
    }

    /// Write one side of the rider/mount link. Crate-visible rather than
    /// public because the invariant is that the two sides agree, and
    /// only `EncounterInstance::{mount, dismount}` can see both actors
    /// at once to keep them that way.
    pub(crate) fn set_mounted_on(&mut self, mount_id: Option<usize>) {
        self.mounted_on = mount_id;
    }

    /// Write the other side of the link. See `set_mounted_on`.
    pub(crate) fn set_ridden_by(&mut self, rider_id: Option<usize>) {
        self.ridden_by = rider_id;
    }

    /// 5e "you can mount or dismount a creature… the cost is movement
    /// equal to half your speed" (PHB p.198), in feet.
    ///
    /// Measured against `speed()` rather than `base_speed` so a Longstrider
    /// or a Slow moves the toll with it, which is what "half your speed"
    /// says. Rounded to the engine's 2.5-ft tile so the toll is always a
    /// whole number of tiles and a 30-ft creature pays exactly 15.
    ///
    /// A creature whose speed has been reduced to nothing pays nothing
    /// and still cannot mount: `Resource::Movement` refuses outright for
    /// anyone under a `zeros_movement` condition, whatever the amount,
    /// so the Rooted and the Restrained stay where they are without this
    /// having to say so.
    pub fn mount_movement_cost(&self) -> f32 {
        (self.speed() / 2.0 / TILE_FEET).floor() * TILE_FEET
    }

    /// How many tiles the actor has walked in an unbroken straight line,
    /// or `None` if it isn't running.
    ///
    /// 5e's charge clauses all read "if the creature moves at least 20
    /// feet straight toward a target and then hits it". Two of those
    /// three words are answered here; "toward a target" is the caller's,
    /// because only the caller knows who got hit.
    ///
    /// The count is the run's own length, so a boar that steps around a
    /// boulder and then puts its head down for twenty feet is charging —
    /// the sidestep ends one run and begins another rather than
    /// disqualifying the turn. Difficult ground doesn't disqualify it
    /// either: the run is counted in tiles crossed, not in feet spent,
    /// so mud slows the charge without cancelling it.
    ///
    /// Returns tiles rather than feet so the caller compares against the
    /// board's own units; `CHARGE_RUN_TILES` is the 20-foot threshold in
    /// those units.
    pub fn straight_run_tiles(&self) -> Option<isize> {
        self.run_step?;
        let delta = self.location - self.run_origin;
        let tiles = delta.x.abs().max(delta.y.abs());
        (tiles > 0).then_some(tiles)
    }

    /// Footprint-Chebyshev gap (in tiles) to another actor, accounting
    /// for both creatures' size categories. 0 means touching/adjacent.
    /// Free-standing analogue of `EncounterInstance::footprint_distance`
    /// for callers (AI heuristics, condition aura sweeps) that already
    /// hold both actor references and want to skip the id-lookup round-
    /// trip. Mirrors the same gap formula via the shared
    /// `engine::util::footprint_chebyshev` helper.
    pub fn footprint_gap_to(&self, other: &ActorInstance) -> isize {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        footprint_chebyshev(
            self.location,
            get_tiles_from_size(self.size),
            other.location,
            get_tiles_from_size(other.size),
        )
    }

    pub fn initiative(&self) -> Option<i32> {
        self.initiative
    }

    pub fn initiative_mod(&self) -> i32 {
        modifier_from_score(self.dexterity)
    }

    /// True if the actor rolls the initiative d20 with advantage. Read by
    /// `roll_initiative`. Walks the shared `INITIATIVE_ADVANTAGE_SOURCES`
    /// cohort — any row whose predicate fires flips the roll shape to
    /// advantage. Adding a future advantage source (Alert feat's
    /// pre-2024 variant, Guardian Armor set bonus, etc.) lands as a
    /// one-line entry in that cohort rather than another `|| new_flag`
    /// join here. Sibling to `initiative_flat_bonus` on the "passive
    /// initiative augment" lane — that helper stacks a flat number on
    /// the result, this one drops the advantage die.
    pub fn rolls_initiative_with_advantage(&self) -> bool {
        INITIATIVE_ADVANTAGE_SOURCES.iter().any(|f| f(self))
    }

    /// Flat bonus added to the initiative result *after* the d20 roll and
    /// DEX modifier. Read by `roll_initiative` alongside
    /// `rolls_initiative_with_advantage`. Composes two cohort lanes,
    /// split by scalar source:
    ///
    /// - **Proficiency-bonus cohort** — rows in
    ///   `PROFICIENCY_INITIATIVE_BONUSES` (Remarkable Athlete → half
    ///   prof rounded up, Aura of the Sentinel → full prof). Each row
    ///   is a `(flag_fn, fraction)` pair; rows whose flag fires stack
    ///   additively. Adding a new prof-bonus-based initiative bump
    ///   (a hypothetical Alert-style feat, a new subclass with a
    ///   third-prof or half-prof-rounded-down bump, etc.) lands as one
    ///   row in that cohort rather than another if-branch here.
    /// - **Ability-mod cohort** — rows in `ABILITY_MOD_INITIATIVE_BONUSES`
    ///   (Rakish Audacity → CHA-mod, Dread Ambusher → WIS-mod, Tactical
    ///   Wit → INT-mod). Each row is a `(flag_fn, ability)` pair; rows
    ///   whose flag fires stack additively. Adding a new ability-mod
    ///   initiative bump (Alert feat's pre-2024 CON-mod variant, etc.)
    ///   lands as one row in that cohort rather than another if-branch
    ///   here.
    ///
    /// Sibling to `rolls_initiative_with_advantage` on the "passive
    /// initiative augment" lane — that helper flips the roll shape
    /// (advantage vs. normal), this one stacks a scalar on the total.
    pub fn initiative_flat_bonus(&self) -> i32 {
        // 5e proficiency-bonus-based initiative bumps (Remarkable
        // Athlete's half-prof rounded up on the Champion Fighter,
        // Aura of the Sentinel's full prof on the Watchers Paladin,
        // any future prof-bonus bump). Each cohort row is a
        // `(flag_fn, fraction)` pair; rows whose flag fires stack
        // additively so a hypothetical Champion-Fighter / Watchers-
        // Paladin multiclass carries both bumps at once. Same
        // filter-map-sum shape as the sibling ability-mod cohort walk
        // below so the two initiative-cohort readers stay uniform.
        let prof_bonus = self.proficiency_bonus();
        let prof_bonus_sum: i32 = PROFICIENCY_INITIATIVE_BONUSES
            .iter()
            .filter(|entry| (entry.flag)(self))
            .map(|entry| entry.fraction.apply(prof_bonus))
            .sum();
        // 5e ability-mod-based initiative bumps (Rakish Audacity's
        // CHA-mod, Dread Ambusher's WIS-mod, Tactical Wit's INT-mod,
        // any future single-ability bump). Each cohort row is a
        // `(flag_fn, ability)` pair; rows whose flag fires stack
        // additively so a hypothetical Gloom Stalker Ranger /
        // Swashbuckler Rogue / War Magic Wizard multiclass would carry
        // all three bumps at once. Adding a new ability-mod initiative
        // bump lands as one row in the cohort table above rather than
        // another if-branch here. Same filter-map-sum shape as the
        // sibling proficiency-bonus cohort walk above so the two
        // initiative-cohort readers stay uniform with each other.
        let ability_mod_sum: i32 = ABILITY_MOD_INITIATIVE_BONUSES
            .iter()
            .filter(|entry| (entry.flag)(self))
            .map(|entry| self.ability_modifier(entry.ability))
            .sum();
        prof_bonus_sum + ability_mod_sum
    }

    pub fn roll_initiative(&mut self, roller: &mut impl Roller) {
        // Two passive-initiative-augment sources compose on the same
        // roll: `rolls_initiative_with_advantage` picks the higher of
        // two d20s (Feral Instinct — Barbarian lv7 struct-field flag,
        // Vigilant Blessing — Twilight Cleric lv1 subclass-tag lookup;
        // any row on `INITIATIVE_ADVANTAGE_SOURCES` suffices); then
        // `initiative_flat_bonus` stacks a scalar bump on top —
        // proficiency-bonus cohort (Remarkable Athlete — Champion
        // Fighter lv7: half-prof rounded up; Aura of the Sentinel —
        // Watchers Paladin lv7: full prof; every row sums additively
        // via `PROFICIENCY_INITIATIVE_BONUSES`) plus ability-mod cohort
        // (Rakish Audacity — Swashbuckler Rogue lv3: +CHA-mod; Dread
        // Ambusher — Gloom Stalker Ranger lv3: +WIS-mod; Tactical Wit
        // — War Magic Wizard lv2: +INT-mod; every ability-mod row
        // sums additively via `ABILITY_MOD_INITIATIVE_BONUSES`).
        // Adding a new source of any shape lands in the matching
        // helper as a one-line entry without touching this body.
        let rolled = if self.rolls_initiative_with_advantage() {
            let a = roller.roll_d20() as i32;
            let b = roller.roll_d20() as i32;
            a.max(b)
        } else {
            roller.roll_d20() as i32
        };
        self.initiative = Some(rolled + self.initiative_mod() + self.initiative_flat_bonus());
    }

    /// Top-of-turn refresh: movement and action-economy slots regenerate,
    /// and any condition with `UntilStartOfNextTurn` (e.g. Dodge) expires.
    /// Returns the conditions that were cleared so the engine can log them.
    pub fn reset_for_new_round(&mut self) -> Vec<Condition> {
        self.movement = self.speed();
        // A run does not survive the turn boundary: RAW's charge clauses
        // all end in "on the same turn".
        self.break_run();
        self.movement_spent_this_turn = 0.0;
        self.action_slots = 1;
        self.bonus_action_slots = 1;
        self.reaction_slots = 1;
        self.legendary_action_slots = self.legendary_actions_per_round;
        // 5e Tasha's Mind Whip: on the holder's next turn, they lose one
        // of action / bonus action / reaction. We zero the action slot
        // (most-impactful pick) and burn the condition the moment it
        // gates the next turn. The NoReaction rider was applied
        // separately on the cast for the reaction-loss half; the
        // start-of-turn cleanup is the action-loss half.
        if self.conditions.remove(&Condition::MindWhipped).is_some() {
            self.action_slots = 0;
        }
        // Once-per-turn attack-rider ledger — every entry in
        // `ONCE_PER_TURN_RIDER_TAGS` (Sneak Attack, Colossus Slayer,
        // Foe Slayer, Divine Fury, Dreadful Strikes, Psychic Blades,
        // Planar Warrior, Slayer's Prey, Gathered Swarm) shares a
        // single `HashSet<&'static str>` cleared here in one line.
        // Adding a future once-per-turn rider (a subclass equivalent,
        // a new Battle Master maneuver's per-turn window) needs no
        // touch to this reset site — the tag just plugs into the
        // shared ledger via `mark_once_per_turn_used(TAG)` and lands
        // as one new row on the sibling `ONCE_PER_TURN_WEAPON_DIE_RIDERS`
        // cohort (the cross-module invariant is pinned by
        // `every_weapon_die_rider_tag_is_registered_in_the_ledger_list`).
        // Help grants from this actor live with the helped actor, so we
        // don't clear them here.
        self.once_per_turn_marks.clear();
        // 5e Hunter Ranger Multiattack Defense (Defensive Tactics, lv7):
        // per-turn ledger of targets this actor has landed a connecting
        // hit on. Cleared at turn-start so the +4 AC penalty against
        // repeat-attackers only spans the attacker's own turn — the
        // "rest of the turn" clause in RAW.
        self.hit_targets_this_turn.clear();
        // 5e Swashbuckler Rogue Fancy Footwork (subclass level 3):
        // per-turn ledger of targets this actor has made a melee attack
        // against. Cleared at turn-start so the OA-suppression window
        // only spans the swash's own turn — the "rest of your turn"
        // clause in RAW. Written unconditionally on every melee attack
        // (see `mark_melee_attacked_this_turn`); read-side gated on
        // `has_fancy_footwork` in the OA dispatcher, so a non-swash
        // attacker's ledger populates and clears the same as the
        // swash's without any read-side effect.
        self.melee_attack_targets_this_turn.clear();
        // 5e two-weapon fighting: the main-hand light swing that opens
        // the off-hand bonus attack is good for this turn only. Cleared
        // here so a dual-wielder who holds their bonus action can't
        // cash it against last turn's swing.
        self.light_weapon_swing_this_turn = false;
        // 5e Fighter Champion — Survivor (level 18): passive at-start-of-
        // turn regen. While combat-active AND at or below half max HP,
        // the holder regains `5 + CON modifier` HP (floor 1, so a -2 CON
        // Champion still ticks up 3). Routes through `heal` so the
        // standard at-max ceiling clips the regen — Survivor doesn't
        // bump the cap. Gate on `is_combat_active` so a downed Champion
        // doesn't auto-resurrect; Survivor is stabilization, not revival.
        if matches!(self.hp_state, HpState::Active)
            && self.hitpoints > 0
            && self.hitpoints * 2 <= self.max_hitpoints()
            && self.has_passive_feature(crate::actions::class_features::SURVIVOR_TAG)
        {
            let con_mod = modifier_from_score(self.constitution);
            let amount = (5 + con_mod).max(1) as u32;
            self.heal(amount);
        }
        let mut expired = self.clear_until_next_turn_conditions();
        // 5e: Dodge / Disengage / Helped end at the start of the holder's
        // next turn regardless of whatever timer was used to install
        // them. Force-clear those here so a Permanent-timer Dodge from
        // a test or alternate code path still drops on the right tick.
        for c in [
            Condition::Dodging,
            Condition::Disengaging,
            Condition::Helped,
        ] {
            if self.remove_condition(c) {
                expired.push(c);
            }
        }
        expired
    }

    pub fn is_dodging(&self) -> bool {
        self.has_condition(Condition::Dodging)
            && !self.has_condition(Condition::Incapacitated)
            && !self.has_condition(Condition::Stunned)
            && !self.has_condition(Condition::Restrained)
            // A Sphered creature is fully encased and can't reactively
            // dodge incoming attacks — the sphere holds them in place.
            && !self.has_condition(Condition::Sphered)
    }

    pub fn set_dodging(&mut self, on: bool) {
        if on {
            // Dodge ends at the start of the actor's next turn (5e).
            self.add_condition(Condition::Dodging, ConditionTimer::UntilStartOfNextTurn);
        } else {
            self.remove_condition(Condition::Dodging);
        }
    }

    pub fn is_disengaging(&self) -> bool {
        self.has_condition(Condition::Disengaging)
    }

    pub fn set_disengaging(&mut self, on: bool) {
        if on {
            self.add_condition(Condition::Disengaging, ConditionTimer::UntilStartOfNextTurn);
        } else {
            self.remove_condition(Condition::Disengaging);
        }
    }

    /// Identity of the helper who granted advantage to this actor, if
    /// any. Returns the first helper id we find — `set_help_grant`
    /// keeps the map at most one entry, so this is unambiguous.
    pub fn helped_by(&self) -> Option<usize> {
        self.help_grants.keys().next().copied()
    }

    /// Flat to-hit / save bonus contributed by Bless. Returns +2 (the
    /// d4 average) when the actor is Blessed; otherwise 0.
    pub fn bless_bonus(&self) -> i32 {
        if self.is_blessed() { 2 } else { 0 }
    }

    pub fn action_slots(&self) -> u32 {
        self.action_slots
    }

    pub fn bonus_action_slots(&self) -> u32 {
        self.bonus_action_slots
    }

    /// Heal HP. A Dying or Stable actor with `amount > 0` snaps back to
    /// Active at exactly `amount` HP (5e: regaining HP from 0 sets you
    /// to the new value). Active actors heal up to their max.
    ///
    /// A target that `can_regain_hitpoints` says no to regains nothing.
    /// Reported as `NoOp` rather than `AlreadyFull` so a caller that
    /// spends a resource on the heal can tell "this did nothing" from
    /// "this target was topped off", which is the same distinction the
    /// `Dead` arm above draws — and which the AI's heal rung reads to
    /// avoid offering a heal that would be thrown away.
    pub fn heal(&mut self, amount: u32) -> HealOutcome {
        if amount == 0 {
            return HealOutcome::AlreadyFull;
        }
        if !self.can_regain_hitpoints() {
            return HealOutcome::NoOp;
        }
        let cap = self.max_hitpoints();
        match self.hp_state {
            HpState::Dead => HealOutcome::NoOp,
            HpState::Dying { .. } | HpState::Stable => {
                self.hp_state = HpState::Active;
                self.hitpoints = amount.min(cap);
                self.remove_condition(Condition::Unconscious);
                HealOutcome::Revived
            }
            HpState::Active => {
                let new_hp = self.hitpoints.saturating_add(amount).min(cap);
                if new_hp == self.hitpoints {
                    HealOutcome::AlreadyFull
                } else {
                    self.hitpoints = new_hp;
                    HealOutcome::Healed
                }
            }
        }
    }

    /// Convenience for `modifier_from_score(self.ability_score(ability))` —
    /// the most-repeated read of an actor's ability modifier. Replaces ~40
    /// sites of the explicit `modifier_from_score(caster.ability_score(...))`
    /// dance across the spells / class-features layer with a one-liner.
    pub fn ability_modifier(&self, ability: AbilityScoreType) -> i32 {
        modifier_from_score(self.ability_score(ability))
    }

    pub fn spell_save_dc(&self, ability: AbilityScoreType) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    /// Everything this actor adds to a saving throw of `ability` from
    /// its own sheet: the ability modifier, the proficiency bonus if it
    /// is proficient, carried-item bonuses, the spell-installed save
    /// buff (Bless's `AdjustSaveBuff`), and the condition-only flat lane
    /// (a Bardic Inspiration die already handed over).
    ///
    /// Deliberately *not* the whole save total.
    /// `roll_save_with_extra_mode_and_bonus` adds three more things that
    /// no actor can answer alone — the Bless / Bane d4, a nearby
    /// paladin's Aura of Protection, and whatever call-site bonus the
    /// feature rolling the save brought with it — and it applies
    /// advantage and disadvantage, which are not a number at all. The
    /// save site sums this and then those.
    ///
    /// Its other caller is the AI, which uses it to ask "which of this
    /// creature's saves is the weak one" before choosing what to throw
    /// at it. That is a heuristic and the omissions above are fine for
    /// it: an aura or a Bless lifts every save by the same amount and
    /// so does not change which one is lowest.
    pub fn save_modifier(&self, ability: AbilityScoreType) -> i32 {
        let prof = if self.is_save_proficient(ability) {
            self.proficiency_bonus()
        } else {
            0
        };
        self.ability_modifier(ability)
            + prof
            + self.item_save_bonus()
            + self.save_bonus_buff()
            + self.condition_save_bonus()
    }

    /// Standard d20 attack-roll modifier — proficiency bonus + the
    /// ability mod. Named `spell_attack_modifier` for the historical
    /// caster-cantrip call sites, but the math is identical for any
    /// proficient attack (RAW: monsters are universally proficient
    /// with their natural weapons). The Horned Devil / Efreeti hurled
    /// flame attacks reuse this helper for their CHA-based spell-
    /// attack shots; any future weapon impl that needs the raw
    /// `prof + ability` sum should call this rather than re-inlining
    /// the addition.
    pub fn spell_attack_modifier(&self, ability: AbilityScoreType) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }

    /// For spells available to multiple classes (Bard / Sorcerer / Wizard /
    /// Warlock — Eyebite, Otto's, Fire Storm), pick the ability whose raw
    /// score is highest from a candidate set and return the resulting
    /// `spell_save_dc`. Ties break by the order in `candidates`. Falls back
    /// to the first ability if every candidate score is identical.
    /// Centralized so spell impls don't each open-code the
    /// "max(INT, CHA, WIS)" pattern.
    pub fn best_spell_save_dc<I>(&self, candidates: I) -> i32
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        self.spell_save_dc(self.best_spellcasting_ability(candidates))
    }

    /// Spell-attack-modifier analogue of `best_spell_save_dc`. Picks the
    /// ability whose raw score is highest from `candidates` and returns
    /// `spell_attack_modifier` for that ability. Ties break by the order
    /// in `candidates`. Used by spells that resolve as a ranged spell
    /// attack but are available to multiple casting classes (Chromatic
    /// Orb on INT/CHA, future Witch-Bolt-style pickups, etc.) so the
    /// caller doesn't have to open-code the max-of-scores pattern.
    pub fn best_spell_attack_modifier<I>(&self, candidates: I) -> i32
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        self.spell_attack_modifier(self.best_spellcasting_ability(candidates))
    }

    /// The three abilities 5e ever uses for spellcasting, in the order
    /// `best_spellcasting_ability` breaks ties: Intelligence (wizard,
    /// artificer, Eldritch Knight, Arcane Trickster), Charisma (bard,
    /// sorcerer, warlock, paladin), Wisdom (cleric, druid, ranger,
    /// Four Elements monk).
    ///
    /// The order is the historical default rather than a claim about
    /// importance — it is the anchor most hardcoded call sites used
    /// before they were promoted, so a template whose top two mental
    /// scores tie keeps the DC it had.
    pub const SPELLCASTING_ABILITIES: [AbilityScoreType; 3] = [
        AbilityScoreType::Intelligence,
        AbilityScoreType::Charisma,
        AbilityScoreType::Wisdom,
    ];

    /// This caster's spell save DC, off whichever of the three
    /// spellcasting abilities they score highest in.
    ///
    /// The engine's model of RAW's "8 + proficiency + your spellcasting
    /// ability modifier". 5e keys that phrase to the caster's *class*,
    /// which a `CreatureTemplate` doesn't carry — a template is a
    /// stat block, not a character sheet, and the same spell static is
    /// shared by every class list that gets it. Reading the highest
    /// mental score instead is the standing approximation, and it is a
    /// good one precisely because a stat block invests in exactly the
    /// ability its class casts off: a wizard's INT, a warlock's CHA, a
    /// druid's WIS.
    ///
    /// Prefer this over `spell_save_dc(SomeFixedAbility)` for any spell
    /// on more than one class list. A fixed anchor is only correct when
    /// exactly one kind of creature ever casts the thing — and when it
    /// is wrong it is invisible, because the spell still fires, still
    /// logs, and still rolls a save. It just rolls against a DC built
    /// from a stat the caster never invested in.
    pub fn spellcasting_save_dc(&self) -> i32 {
        self.best_spell_save_dc(Self::SPELLCASTING_ABILITIES)
    }

    /// Attack-roll sibling of `spellcasting_save_dc`, for spells that
    /// resolve as a spell attack rather than a save.
    pub fn spellcasting_attack_modifier(&self) -> i32 {
        self.best_spell_attack_modifier(Self::SPELLCASTING_ABILITIES)
    }

    /// Pick the highest-scoring spellcasting ability from `candidates`. Ties
    /// break by the order in `candidates`. Falls back to Intelligence on an
    /// empty iterator (no caller currently passes empty — the fallback is a
    /// belt-and-suspenders so the helper is total). Shared body for the
    /// best-DC / best-attack-modifier pair so the picker logic lives at one
    /// chokepoint.
    pub fn best_spellcasting_ability<I>(&self, candidates: I) -> AbilityScoreType
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        candidates
            .into_iter()
            .max_by_key(|a| self.ability_score(*a))
            .unwrap_or(AbilityScoreType::Intelligence)
    }

    /// Apply `raw` damage of type `dt`, factoring in immunity / resistance
    /// / vulnerability and absorbing through the Arcane Ward and then any
    /// temp HP first. Returns `(outcome, final_amount)` where
    /// `final_amount` is the actual HP delta that landed (after all
    /// reductions and both absorption pools).
    pub fn take_typed_damage(&mut self, raw: u32, dt: DamageType) -> (DamageOutcome, u32) {
        let scaled = self.effective_damage(raw, dt);
        if scaled == 0 {
            return (
                match self.hp_state {
                    HpState::Dying { .. } | HpState::Stable | HpState::Dead => {
                        DamageOutcome::DyingFailure
                    }
                    HpState::Active => DamageOutcome::Reduced,
                },
                0,
            );
        }
        // 5e Abjuration Wizard Arcane Ward: "whenever you take damage,
        // the ward takes the damage instead." Drains ahead of temp HP —
        // the ward intercepts the damage before it is damage *to you*,
        // which is the layer temp HP absorbs. Gated to Active actors for
        // the same reason temp HP is: a Dying / Stable actor's incoming
        // damage is bookkept as death-save failures, not an HP delta the
        // ward could stand in front of.
        let scaled = if matches!(self.hp_state, HpState::Active) {
            let warded = scaled.min(self.arcane_ward);
            self.arcane_ward -= warded;
            scaled - warded
        } else {
            scaled
        };
        if scaled == 0 {
            return (DamageOutcome::Reduced, 0);
        }
        // Burn temp HP first; only the leftover hits real HP. Temp HP
        // is only relevant for Active actors — Dying / Stable creatures
        // route damage straight into death-save failures.
        if matches!(self.hp_state, HpState::Active) {
            let to_hp = scaled - self.drain_temp_hp(scaled);
            if to_hp == 0 {
                return (DamageOutcome::Reduced, 0);
            }
            (self.take_damage(to_hp), to_hp)
        } else {
            (self.take_damage(scaled), scaled)
        }
    }

    /// Spend up to `amount` damage against the temporary-hit-point pool
    /// and report what the pool actually absorbed. The caller subtracts
    /// the return value from the incoming damage; whatever is left is
    /// what reaches real hit points.
    ///
    /// This is the one place temp HP is decremented, which is what lets
    /// it also enforce the RAW duration clause shared by every feature
    /// worded "...until you lose all these temporary hit points" — see
    /// `TEMP_HP_BOUND_CONDITIONS`. Both damage entry points route
    /// through it (`effective_damage`'s post-resistance path and
    /// `take_damage`'s direct path), so a symbiote can't survive its own
    /// shield running out down one lane and not the other.
    ///
    /// The condition is dropped silently, without a log line, for the
    /// same reason Death Ward's burn-off is silent at this layer:
    /// `ActorInstance` has no handle on the encounter log. The
    /// disappearance is visible where it matters — the next melee swing
    /// simply stops printing its rider.
    fn drain_temp_hp(&mut self, amount: u32) -> u32 {
        let absorbed = amount.min(self.temp_hp);
        if absorbed == 0 {
            return 0;
        }
        self.temp_hp -= absorbed;
        if self.temp_hp == 0 {
            for &c in TEMP_HP_BOUND_CONDITIONS {
                self.remove_condition(c);
            }
        }
        absorbed
    }

    pub fn take_damage(&mut self, amount: u32) -> DamageOutcome {
        match self.hp_state {
            HpState::Stable => {
                self.hp_state = HpState::Dying {
                    successes: 0,
                    failures: 1,
                };
                DamageOutcome::DyingFailure
            }
            HpState::Dying {
                successes,
                failures,
            } => {
                self.hp_state = HpState::Dying {
                    successes,
                    failures: failures + 1,
                };
                DamageOutcome::DyingFailure
            }
            HpState::Dead => DamageOutcome::DyingFailure,
            HpState::Active => {
                let after_temp = amount - self.drain_temp_hp(amount);
                if after_temp == 0 {
                    return DamageOutcome::Reduced;
                }
                let hp_before = self.hitpoints;
                self.hitpoints = self.hitpoints.saturating_sub(after_temp);
                if self.hitpoints == 0 {
                    // 5e Massive Damage (PHB p.197): if remaining damage
                    // after hitting 0 HP equals or exceeds the creature's
                    // max HP, it dies instantly — no death saves.
                    let overflow = after_temp.saturating_sub(hp_before);
                    if overflow >= self.max_hitpoints() {
                        self.hp_state = HpState::Dead;
                        return DamageOutcome::Killed;
                    }
                    // 5e Death Ward: when the holder would drop to 0 HP,
                    // they instead drop to 1 HP and the ward burns off.
                    if self.conditions.contains_key(&Condition::DeathWarded) {
                        self.hitpoints = 1;
                        self.conditions.remove(&Condition::DeathWarded);
                        return DamageOutcome::Reduced;
                    }
                    // 5e "drop to 1 HP instead" cohort — Half-Orc
                    // Relentless Endurance, Paladin Ancients Undying
                    // Sentinel, and any future sibling — routed through
                    // one ordered list so the first-available charge
                    // fires. Death Ward is checked *above* this loop —
                    // RAW: the spell is an active resource the caster
                    // chose to maintain, so burning the racial / class
                    // feature before Death Ward would waste the slot.
                    // Massive Damage (overflow ≥ max HP) also short-
                    // circuits before this cohort (RAW).
                    for &tag in LETHAL_DAMAGE_ABSORBER_FEATURES {
                        if self.spend_feature(tag) {
                            self.hitpoints = 1;
                            return DamageOutcome::Reduced;
                        }
                    }
                    if self.rolls_death_saves {
                        self.hp_state = HpState::Dying {
                            successes: 0,
                            failures: 0,
                        };
                        self.add_condition(Condition::Unconscious, ConditionTimer::Permanent);
                        self.add_condition(Condition::Prone, ConditionTimer::Permanent);
                        DamageOutcome::Downed
                    } else {
                        self.hp_state = HpState::Dead;
                        DamageOutcome::Killed
                    }
                } else {
                    DamageOutcome::Reduced
                }
            }
        }
    }

    pub fn hp_state(&self) -> HpState {
        self.hp_state
    }

    pub fn is_dying(&self) -> bool {
        matches!(self.hp_state, HpState::Dying { .. })
    }

    pub fn is_stable(&self) -> bool {
        matches!(self.hp_state, HpState::Stable)
    }

    /// Promote a Dying actor to Stable without restoring any HP (5e
    /// Spare the Dying / Medicine check stabilize semantics: they stop
    /// rolling death saves but stay at 0 HP and Unconscious). No-op for
    /// non-Dying actors. Returns true if the actor's state changed.
    pub fn stabilize(&mut self) -> bool {
        if matches!(self.hp_state, HpState::Dying { .. }) {
            self.hp_state = HpState::Stable;
            true
        } else {
            false
        }
    }

    pub fn is_combat_active(&self) -> bool {
        matches!(self.hp_state, HpState::Active) && self.hitpoints > 0
    }

    pub fn death_save_record(&self) -> (u32, u32) {
        match self.hp_state {
            HpState::Dying {
                successes,
                failures,
            } => (successes, failures),
            _ => (0, 0),
        }
    }

    pub fn apply_death_save(&mut self, raw_d20: u32) -> DeathSaveOutcome {
        debug_assert!(
            (1..=20).contains(&raw_d20),
            "death-save d20 out of range: {}",
            raw_d20
        );
        let HpState::Dying {
            successes,
            failures,
        } = self.hp_state
        else {
            return DeathSaveOutcome::NotDying;
        };
        if raw_d20 == 20 {
            self.hp_state = HpState::Active;
            self.hitpoints = 1;
            self.remove_condition(Condition::Unconscious);
            return DeathSaveOutcome::Revived;
        }
        let (succ, fail) = if raw_d20 == 1 {
            (successes, failures.saturating_add(2))
        } else if raw_d20 >= 10 {
            (successes.saturating_add(1), failures)
        } else {
            (successes, failures.saturating_add(1))
        };
        if fail >= 3 {
            self.hp_state = HpState::Dead;
            DeathSaveOutcome::Dead
        } else if succ >= 3 {
            self.hp_state = HpState::Stable;
            DeathSaveOutcome::Stabilized
        } else {
            self.hp_state = HpState::Dying {
                successes: succ,
                failures: fail,
            };
            DeathSaveOutcome::Continuing
        }
    }


    /// Class-feature gates (Second Wind, Action Surge, etc.). True while
    /// the holder still has at least one unspent charge of `tag`.
    pub fn feature_available(&self, tag: &'static str) -> bool {
        self.feature_charges_remaining(tag) > 0
    }

    /// Which counter in `features_remaining` / `features_max` actually
    /// backs `tag` for *this* actor.
    ///
    /// Almost always `tag` itself. The exception is a member of a shared
    /// pool (`class_features::SHARED_FEATURE_POOLS` — the Battle Master
    /// maneuvers and their superiority dice today): if this actor also
    /// carries the pool tag, every read and write for the member is
    /// redirected onto the pool, so fourteen maneuvers spend from one
    /// count of four rather than from fourteen counts of one.
    ///
    /// The `features_max.contains_key(pool)` guard is what makes the
    /// redirect opt-in per actor rather than per tag. A chassis that
    /// picks up a maneuver without the pool — a monster given Trip
    /// Attack, a test fixture grafting one on — keeps the plain per-tag
    /// charge it would have had before pools existed, instead of
    /// reading a counter it does not own and finding the feature
    /// permanently unusable. Shipping a maneuver without its pool is
    /// still a mistake, just a loud one: `every_pool_member_ships_with_its_pool`
    /// fails on it.
    fn charge_counter_for(&self, tag: &'static str) -> &'static str {
        match crate::actions::class_features::shared_pool_for(tag) {
            Some(pool) if self.features_max.contains_key(pool) => pool,
            _ => tag,
        }
    }

    /// How many charges of `tag` are left. `0` covers both "spent" and
    /// "never had it", which is what every caller wants — the two are
    /// distinguished by `has_passive_feature`.
    ///
    /// For a shared-pool member this reports the *pool's* remaining
    /// count, so a fighter who has spent three superiority dice reads 1
    /// through every maneuver they know rather than 1 through the one
    /// they last used. The `features_max` gate keeps "never had it"
    /// answering 0: knowing the pool is not knowing the maneuver.
    ///
    /// Public because the multi-charge features are exactly the ones
    /// whose tests need to see the pool draining a charge at a time
    /// rather than flipping a bit.
    pub fn feature_charges_remaining(&self, tag: &'static str) -> u32 {
        if !self.features_max.contains_key(tag) {
            return 0;
        }
        self.features_remaining
            .get(self.charge_counter_for(tag))
            .copied()
            .unwrap_or(0)
    }

    /// Spend one charge of `tag`. Returns `true` if a charge was
    /// actually there to spend — every caller that gates on
    /// `feature_available` first can ignore the result, and the ones
    /// that don't (the lethal-damage absorber cohort) use it as the
    /// gate itself.
    ///
    /// Spending a shared-pool member debits the pool, which is the whole
    /// point: a Trip Attack and a Riposte cost the same one die out of
    /// the same four.
    pub fn spend_feature(&mut self, tag: &'static str) -> bool {
        if !self.features_max.contains_key(tag) {
            return false;
        }
        let counter = self.charge_counter_for(tag);
        match self.features_remaining.get_mut(counter) {
            Some(remaining) if *remaining > 0 => {
                *remaining -= 1;
                true
            }
            _ => false,
        }
    }

    /// Put a spent per-use charge back without waiting for a rest.
    /// Returns `true` if the charge was actually restored (the pool had
    /// room), `false` if it was already full or the actor doesn't carry
    /// the feature at all.
    ///
    /// The counterpart to `spend_feature` for features whose RAW
    /// recharge condition is an in-encounter event rather than a rest —
    /// today the Conjuration Wizard's Benign Transposition, which comes
    /// back whenever its holder casts a conjuration spell of 1st level
    /// or higher. Distinct from `grant_feature_for_test`, which also
    /// writes `features_max` and so *adds* a feature the template never
    /// had: this only refills a charge for a feature the actor already
    /// carries, and is a no-op for one they don't.
    ///
    /// Refills one charge, not the pool: a feature whose recharge
    /// trigger fires twice hands back two charges, which is the RAW
    /// reading of every event-driven recharge in the book.
    ///
    /// Handing a charge back to a shared-pool member credits the pool,
    /// symmetric with `spend_feature` debiting it — otherwise a
    /// hypothetical "regain the die you spent" trigger would top up a
    /// counter nothing reads and leave the pool empty.
    pub fn restore_feature_charge(&mut self, tag: &'static str) -> bool {
        if !self.features_max.contains_key(tag) {
            return false;
        }
        let counter = self.charge_counter_for(tag);
        let Some(&max) = self.features_max.get(counter) else {
            return false;
        };
        let remaining = self.features_remaining.entry(counter).or_insert(0);
        if *remaining >= max {
            return false;
        }
        *remaining += 1;
        true
    }

    /// True if this actor was instantiated with `tag` in their template's
    /// feature set. Distinct from `feature_available` — `has_passive_feature`
    /// returns true even after the feature's per-rest charge has been spent.
    /// Used by always-on passives (Wild Magic Surge, Sorcerous Restoration)
    /// whose trigger fires every encounter regardless of any charge pool.
    pub fn has_passive_feature(&self, tag: &'static str) -> bool {
        self.features_max.contains_key(tag)
    }

    /// Has the rogue used their once-per-turn Sneak Attack already?
    /// Shared once-per-turn ledger read. Returns true when `tag` has
    /// been marked spent this turn (via `mark_once_per_turn_used`).
    /// Cleared wholesale at turn-start by `reset_for_new_round`. The
    /// registered tags are documented in
    /// `class_features::ONCE_PER_TURN_RIDER_TAGS`, though any static
    /// str can be used — the ledger doesn't consult the registry at
    /// runtime.
    pub fn once_per_turn_used(&self, tag: &'static str) -> bool {
        self.once_per_turn_marks.contains(tag)
    }

    /// Shared once-per-turn ledger write. Marks `tag` as spent for
    /// the remainder of this actor's turn; a subsequent
    /// `once_per_turn_used(tag)` returns true until
    /// `reset_for_new_round` clears the whole ledger.
    pub fn mark_once_per_turn_used(&mut self, tag: &'static str) {
        self.once_per_turn_marks.insert(tag);
    }

    /// Rogue Sneak Attack once-per-turn ledger read. Thin wrapper on
    /// the shared `once_per_turn_used(SNEAK_ATTACK_TAG)` — kept for
    /// callsite ergonomics on the rogue's attack path.
    pub fn sneak_attack_used(&self) -> bool {
        self.once_per_turn_used(crate::actions::class_features::SNEAK_ATTACK_TAG)
    }

    pub fn mark_sneak_attack_used(&mut self) {
        self.mark_once_per_turn_used(crate::actions::class_features::SNEAK_ATTACK_TAG)
    }

    /// Hunter Ranger Colossus Slayer once-per-turn ledger read. Thin
    /// wrapper on the shared `once_per_turn_used(COLOSSUS_SLAYER_TAG)`
    /// — kept for callsite ergonomics on the swing rider path.
    pub fn colossus_slayer_used(&self) -> bool {
        self.once_per_turn_used(crate::actions::class_features::COLOSSUS_SLAYER_TAG)
    }

    pub fn mark_colossus_slayer_used(&mut self) {
        self.mark_once_per_turn_used(crate::actions::class_features::COLOSSUS_SLAYER_TAG)
    }

    /// Ranger Foe Slayer once-per-turn ledger read. Thin wrapper on
    /// the shared `once_per_turn_used(FOE_SLAYER_TAG)` — kept for
    /// callsite ergonomics on the swing rider path.
    pub fn foe_slayer_used(&self) -> bool {
        self.once_per_turn_used(crate::actions::class_features::FOE_SLAYER_TAG)
    }

    pub fn mark_foe_slayer_used(&mut self) {
        self.mark_once_per_turn_used(crate::actions::class_features::FOE_SLAYER_TAG)
    }

    /// Zealot Barbarian Divine Fury once-per-turn ledger read. Thin
    /// wrapper on the shared `once_per_turn_used(DIVINE_FURY_TAG)` —
    /// kept for callsite ergonomics on the swing rider path.
    pub fn divine_fury_used(&self) -> bool {
        self.once_per_turn_used(crate::actions::class_features::DIVINE_FURY_TAG)
    }

    pub fn mark_divine_fury_used(&mut self) {
        self.mark_once_per_turn_used(crate::actions::class_features::DIVINE_FURY_TAG)
    }

    /// 5e Hunter Ranger **Multiattack Defense** (Defensive Tactics
    /// option, lv7): has this actor already landed a connecting attack
    /// on `target_id` this turn? Read by the attack chokepoints so a
    /// Multiattack-Defense target adds +4 to their AC against the
    /// same attacker's subsequent swings this turn. Cleared at
    /// turn-start by `reset_for_new_round`.
    pub fn has_hit_target_this_turn(&self, target_id: usize) -> bool {
        self.hit_targets_this_turn.contains(&target_id)
    }

    /// Mark `target_id` as having been hit by this actor this turn.
    /// Called by the attack-resolution chokepoints after a successful
    /// AC-beating swing; the Multiattack Defense +4 AC read below fires
    /// on the *next* connecting attempt against the same target from
    /// this attacker.
    pub fn mark_hit_target_this_turn(&mut self, target_id: usize) {
        self.hit_targets_this_turn.insert(target_id);
    }

    /// Has this actor begun a turn since combat started? Latched once-only
    /// by the engine's `start_turn_for` hook the first time the actor's
    /// turn comes up. Read by the Assassinate gate in `compute_attack_mode`
    /// — Assassin rogues roll with advantage against targets whose flag is
    /// still false.
    pub fn has_taken_turn_in_combat(&self) -> bool {
        self.has_taken_turn_in_combat
    }

    pub fn mark_taken_turn_in_combat(&mut self) {
        self.has_taken_turn_in_combat = true;
    }

    /// 5e Barbarian Relentless Rage — current DC for the CON save that
    /// pins the barbarian at 1 HP when a killing blow would otherwise
    /// drop them. Starts at 10, climbs by 5 each successful save, resets
    /// to 10 on short / long rest. Read by the take-damage intercept
    /// in `DealDamage::apply`.
    pub fn relentless_rage_dc(&self) -> u32 {
        self.relentless_rage_dc
    }

    /// Bump the Relentless Rage DC by 5 (RAW: "Each time you use this
    /// feature after the first, the DC increases by 5"). Called by the
    /// encounter-side intercept after a save succeeds.
    pub fn bump_relentless_rage_dc(&mut self) {
        self.relentless_rage_dc = self.relentless_rage_dc.saturating_add(5);
    }

    /// Snap the actor back to 1 HP from a downed state — used by the
    /// Relentless Rage save-intercept after a successful CON roll.
    /// Clears the Unconscious / Prone install that `take_damage` queued
    /// and flips the HP-state machine back to Active so subsequent
    /// damage in the same round routes through the normal pipeline. The
    /// dying-tick lane (death saves, stabilize) sits below this guard,
    /// so a failed Relentless Rage roll falls through to the standard
    /// PC-down chain naturally.
    pub fn revive_at_one_hp(&mut self) {
        self.hitpoints = 1;
        self.hp_state = HpState::Active;
        self.remove_condition(Condition::Unconscious);
        self.remove_condition(Condition::Prone);
    }

    /// True if this actor has an Indomitable reroll pending — set by
    /// the Indomitable action, consumed at the next failed save.
    pub fn indomitable_pending(&self) -> bool {
        self.indomitable_pending
    }

    pub fn mark_indomitable_pending(&mut self) {
        self.indomitable_pending = true;
    }

    pub fn consume_indomitable(&mut self) -> bool {
        let pending = self.indomitable_pending;
        self.indomitable_pending = false;
        pending
    }

    /// 5e Legendary Resistance — remaining auto-pass charges this long rest.
    /// Read by `EncounterInstance::roll_save` to promote a failed save when
    /// the counter is non-zero. Zero for ordinary creatures.
    pub fn legendary_resistance_remaining(&self) -> u32 {
        self.legendary_resistance_remaining
    }

    /// Per-rest cap on Legendary Resistance charges (the template max).
    /// Surfaced for UI / AI heuristics that need to know if a creature
    /// has the trait at all without caring about the current pool.
    pub fn legendary_resistance_max(&self) -> u32 {
        self.legendary_resistance_max
    }

    /// Spend one Legendary Resistance charge. Returns true if a charge
    /// was actually consumed (counter was > 0), false otherwise. Caller
    /// is expected to check `legendary_resistance_remaining > 0` first
    /// and decide whether burning a charge is worth it.
    pub fn consume_legendary_resistance(&mut self) -> bool {
        if self.legendary_resistance_remaining == 0 {
            return false;
        }
        self.legendary_resistance_remaining -= 1;
        true
    }

    /// Convenience: check Bless condition without callers having to
    /// import the Condition enum just for this single test.
    pub fn is_blessed(&self) -> bool {
        self.has_condition(Condition::Blessed)
    }

    pub fn is_baned(&self) -> bool {
        self.has_condition(Condition::Baned)
    }

    pub fn is_heroic(&self) -> bool {
        self.has_condition(Condition::Heroic)
    }

    /// True iff the actor is currently `Petrified` — turned to stone.
    /// Convenience accessor used by the AI / UI to surface the state
    /// without each call site re-importing `Condition`.
    pub fn is_petrified(&self) -> bool {
        self.has_condition(Condition::Petrified)
    }

    /// True iff the actor holds a Death Ward — the next killing blow
    /// will be absorbed by `take_damage`. Surfaced for AI heuristics
    /// (skip dispelling targets without the buff) and UI tagging.
    pub fn has_death_ward(&self) -> bool {
        self.has_condition(Condition::DeathWarded)
    }

    /// Record that `helper_id` Helped this actor against `target_id`.
    /// The helped actor's next attack against `target_id` benefits from
    /// advantage; the grant is consumed (cleared) by `consume_help_for`.
    pub fn help_grant(&self, target_id: usize) -> bool {
        self.help_grants.values().any(|t| *t == target_id)
    }

    /// True iff *any* Help grant is standing on this actor, whoever it
    /// names. Read by the AI pickers that install a self-grant
    /// (Versatile Trickster, and any future sibling on the Feinting
    /// Attack lane) so they don't spend a bonus action overwriting a
    /// grant that is already going to fire — `set_help_grant` replaces
    /// rather than accumulates, so a second designation is strictly a
    /// loss.
    pub fn help_grant_any(&self) -> bool {
        !self.help_grants.is_empty()
    }

    /// Lowest spell-slot level this actor still has a slot for, or
    /// `None` when every tier is spent. Reads the raw pool rather than
    /// `can_consume_resource`, so a caster whose spell lane is blocked
    /// (`Silenced`, `WildShaped`) still reports what they *hold*.
    ///
    /// That distinction is the whole reason the helper exists: Combat
    /// Wild Shape's slot-to-hit-points conversion is expressly not
    /// casting, so it has to see slots the can't-cast gate would hide.
    /// A future "burn a slot for something that isn't a spell" feature
    /// (a Divine Smite variant priced off the pool, an arcane-recovery
    /// sibling) reads the same way.
    /// True iff some held condition bars this actor from casting at all
    /// (`Condition::blocks_spellcasting` — Silence, Wild Shape). Read by
    /// `Action::validate_input` against actions that declare a
    /// `school()`, which is the engine's marker for "is a spell".
    ///
    /// Distinct from the `SpellSlot` lane of `can_consume_resource`,
    /// which bounces slot-costing spells and therefore can't see
    /// cantrips at all.
    pub fn blocked_from_casting(&self) -> bool {
        self.conditions.keys().any(|c| c.blocks_spellcasting())
    }

    pub fn lowest_available_spell_slot(&self) -> Option<u32> {
        (1..=9).find(|lvl| self.spell_slot_manager.spell_slots(*lvl).spell_slots > 0)
    }

    /// Set or clear a Help grant on this actor.
    /// `Some(g)` overwrites any prior grant; `None` clears all grants.
    pub fn set_help_grant(&mut self, grant: Option<HelpGrant>) {
        self.help_grants.clear();
        if let Some(g) = grant {
            self.help_grants.insert(g.helper_id, g.against);
        }
    }

    /// Consume one Help grant against `target_id` (if any). Returns true
    /// if a grant was consumed — caller folds that into advantage logic.
    pub fn consume_help_for(&mut self, target_id: usize) -> bool {
        let helper = self
            .help_grants
            .iter()
            .find(|(_, t)| **t == target_id)
            .map(|(h, _)| *h);
        match helper {
            Some(h) => {
                self.help_grants.remove(&h);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::shadow_demons::SHADOW_DEMON_TEMPLATE;
    use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
    use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
    use crate::engine::dice::FastRandRoller;

    fn make(ct: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            ct,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap()
    }

    /// The paired constructor puts the B/P/S triplet in the qualified
    /// table and leaves the unqualified one for the template's own
    /// overlays. Pinned because the split *is* the feature: a triplet
    /// that landed in `damage_modifiers` instead would halve a +2
    /// longsword exactly as the pre-`engine::magic` engine did, and
    /// every one of the forty stat blocks built on this constructor
    /// would silently revert.
    #[test]
    fn the_nonmagical_physical_constructor_qualifies_the_triplet() {
        let t = CreatureTemplate::resistant_to_nonmagical_physical();
        assert!(t.damage_modifiers.is_empty());
        assert_eq!(t.nonmagical_damage_modifiers.len(), 3);
        for dt in [
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ] {
            assert_eq!(
                t.nonmagical_damage_modifiers.get(&dt).copied(),
                Some(DamageModifier::Resistance)
            );
        }
    }

    /// An unqualified row shadows the qualified one of the same type,
    /// so the blow is halved once rather than twice. The 5e "multiple
    /// instances of resistance count as only one" rule, enforced at the
    /// accessor rather than left to whichever lane happened to run
    /// first.
    #[test]
    fn an_unqualified_row_shadows_the_qualified_one() {
        static PROMOTED: std::sync::LazyLock<CreatureTemplate> =
            std::sync::LazyLock::new(|| CreatureTemplate {
                damage_modifiers: damage_modifiers_from([(
                    DamageType::Bludgeoning,
                    DamageModifier::Immunity,
                )]),
                ..CreatureTemplate::resistant_to_nonmagical_physical()
            });
        let a = make(&PROMOTED);
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(a.nonmagical_damage_modifier(DamageType::Bludgeoning), None);
        // The types the overlay left alone still answer through the
        // qualified lane.
        assert_eq!(a.damage_modifier(DamageType::Slashing), None);
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn poison_immunity_zeroes_damage() {
        let z = make(&ZOMBIE_TEMPLATE);
        assert_eq!(z.effective_damage(10, DamageType::Poison), 0);
        assert_eq!(z.effective_damage(10, DamageType::Slashing), 10);
    }

    #[test]
    fn skeleton_doubles_bludgeoning() {
        let s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.effective_damage(7, DamageType::Bludgeoning), 14);
        assert_eq!(s.effective_damage(99, DamageType::Poison), 0);
        assert_eq!(s.effective_damage(7, DamageType::Slashing), 7);
    }

    #[test]
    fn resistance_halves_round_down() {
        // The skeleton's piercing resistance is unqualified, so the
        // damage sink applies it without needing to know who swung.
        let s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.effective_damage(7, DamageType::Piercing), 3);
        assert_eq!(s.effective_damage(0, DamageType::Piercing), 0);
        // Poison is immune (zeroed).
        assert_eq!(s.effective_damage(7, DamageType::Poison), 0);
    }

    #[test]
    fn temp_hp_does_not_stack() {
        let mut s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.gain_temp_hp(5), 5);
        // Smaller grant is ignored: pool stays at 5.
        assert_eq!(s.gain_temp_hp(3), 5);
        // Larger grant replaces.
        assert_eq!(s.gain_temp_hp(8), 8);
    }

    #[test]
    fn mind_blank_zeroes_psychic_damage() {
        let mut s = make(&SKELETON_TEMPLATE);
        // Sanity check: no buff = baseline psychic damage lands.
        assert_eq!(s.effective_damage(15, DamageType::Psychic), 15);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        // With the buff up, psychic drops to zero — mirrors the Immunity
        // damage modifier semantics.
        assert_eq!(s.effective_damage(15, DamageType::Psychic), 0);
        // Other damage types still flow through normally.
        assert_eq!(s.effective_damage(7, DamageType::Bludgeoning), 14);
    }

    #[test]
    fn mind_blank_blocks_charm() {
        let mut s = make(&SKELETON_TEMPLATE);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        // Charm application is blocked by the dynamic immunity hook even
        // though Charmed isn't on the template's `condition_immunities`.
        let added = s.add_condition(Condition::Charmed, ConditionTimer::Rounds(10));
        assert!(!added, "charm should fizzle against mind blank");
        assert!(!s.has_condition(Condition::Charmed));
    }

    #[test]
    fn mind_blank_drop_restores_psychic_lane() {
        let mut s = make(&SKELETON_TEMPLATE);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        assert_eq!(s.effective_damage(20, DamageType::Psychic), 0);
        s.remove_condition(Condition::MindBlanked);
        // After dispel / expire, psychic damage flows through normally.
        assert_eq!(s.effective_damage(20, DamageType::Psychic), 20);
    }

    #[test]
    fn petrified_grants_damage_resistance() {
        let mut s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.effective_damage(20, DamageType::Fire), 20);
        s.add_condition(Condition::Petrified, ConditionTimer::Permanent);
        assert_eq!(
            s.effective_damage(20, DamageType::Fire),
            10,
            "petrified creature should take half fire damage"
        );
        assert_eq!(
            s.effective_damage(20, DamageType::Slashing),
            10,
            "petrified creature should take half slashing damage"
        );
    }

    #[test]
    fn petrified_is_immune_to_poison_damage() {
        // 5e RAW: "The creature is immune to poison and disease..."
        // The blanket-resistance row only halves poison; the typed
        // immunity row in TYPED_IMMUNITY_CONDITIONS zeros it. Pin the
        // immunity-wins-over-resistance precedence so a future refactor
        // that quietly demotes Petrified back to "all-damage resistance
        // only" surfaces as a failure here.
        //
        // Uses the Bandit template — a vanilla humanoid with no
        // template-level poison modifier, so a baseline hit lands at
        // full damage and the Petrified install is the load-bearing
        // change. Skeleton / Zombie already have template-level poison
        // immunity so couldn't tell the two paths apart.
        use crate::actors::creatures::bandits::BANDIT_TEMPLATE;
        let mut s = make(&BANDIT_TEMPLATE);
        assert_eq!(s.effective_damage(20, DamageType::Poison), 20);
        s.add_condition(Condition::Petrified, ConditionTimer::Permanent);
        assert_eq!(
            s.effective_damage(20, DamageType::Poison),
            0,
            "petrified creature should be immune to poison damage RAW"
        );
    }

    #[test]
    fn petrified_blocks_poisoned_condition_install() {
        // 5e RAW companion to the poison-damage immunity: a Petrified
        // creature is also immune to the Poisoned condition. Routes
        // through `dynamic_immunity_to` so the `add_condition` chokepoint
        // no-ops the install. Pin the gate so a future refactor that
        // quietly drops Petrified from the dynamic immunity table
        // surfaces here.
        //
        // Uses the Bandit template — a vanilla humanoid with no
        // template-level Poisoned-condition immunity, so the Petrified
        // gate is the only thing that can block the install.
        use crate::actors::creatures::bandits::BANDIT_TEMPLATE;
        let mut s = make(&BANDIT_TEMPLATE);
        s.add_condition(Condition::Petrified, ConditionTimer::Permanent);
        assert!(
            s.effectively_immune_to_condition(Condition::Poisoned),
            "petrified creature should be immune to Poisoned RAW"
        );
        let installed = s.add_condition(Condition::Poisoned, ConditionTimer::Rounds(5));
        assert!(
            !installed,
            "Poisoned install should be blocked by Petrified immunity"
        );
        assert!(
            !s.has_condition(Condition::Poisoned),
            "Poisoned should not have landed on a Petrified target"
        );
    }

    #[test]
    fn resistance_does_not_stack_per_5e_rules() {
        let mut s = make(&SKELETON_TEMPLATE);
        // Skeleton is vulnerable to bludgeoning (doubles), so test with
        // a creature that has no template-level modifier for fire.
        assert_eq!(s.effective_damage(20, DamageType::Fire), 20);
        // Add DamageResistant (Stoneskin).
        s.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        assert_eq!(s.effective_damage(20, DamageType::Fire), 10);
        // Add WardingBonded on top — 5e says resistance doesn't stack.
        s.add_condition(Condition::WardingBonded, ConditionTimer::Rounds(10));
        assert_eq!(
            s.effective_damage(20, DamageType::Fire),
            10,
            "two resistance sources should halve only once (5e stacking rule)"
        );
    }

    #[test]
    fn necklace_of_adaptation_blocks_poisoned() {
        // Fighter has no template-level poison immunity — a clean
        // baseline for the necklace's install-gate contribution.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert!(
            f.add_condition(Condition::Poisoned, ConditionTimer::Rounds(10)),
            "fighter has no template-level poison immunity"
        );
        f.remove_condition(Condition::Poisoned);
        // Necklace of Adaptation blocks the install.
        f.pickup_item(&crate::items::item_template::NECKLACE_OF_ADAPTATION);
        assert!(
            !f.add_condition(Condition::Poisoned, ConditionTimer::Rounds(10)),
            "necklace of adaptation should block poison install"
        );
        assert!(!f.has_condition(Condition::Poisoned));
        // Sanity check: the AoE-prune helper agrees.
        assert!(f.effectively_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn ring_of_free_action_blocks_paralysis_and_restraint() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::RING_OF_FREE_ACTION);
        assert!(
            !f.add_condition(Condition::Paralyzed, ConditionTimer::Permanent),
            "ring should block paralysis"
        );
        assert!(
            !f.add_condition(Condition::Restrained, ConditionTimer::Permanent),
            "ring should block restraint"
        );
        assert!(
            !f.add_condition(Condition::Grappled, ConditionTimer::Permanent),
            "ring should block grapple"
        );
        // Removing the ring restores normal install behavior.
        f.remove_item_by_name("Ring of Free Action");
        assert!(
            f.add_condition(Condition::Paralyzed, ConditionTimer::Permanent),
            "without the ring, paralysis installs normally"
        );
    }

    #[test]
    fn stone_of_good_luck_grants_save_and_ac_bonus() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let base_save = f.total_item_bonuses().save;
        let base_ac = f.total_item_bonuses().ac;
        f.pickup_item(&crate::items::item_template::STONE_OF_GOOD_LUCK);
        assert_eq!(f.total_item_bonuses().save, base_save + 1);
        assert_eq!(f.total_item_bonuses().ac, base_ac + 1);
    }

    #[test]
    fn brooch_of_shielding_halves_force_damage() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        // Baseline force damage.
        assert_eq!(f.effective_damage(20, DamageType::Force), 20);
        f.pickup_item(&crate::items::item_template::BROOCH_OF_SHIELDING);
        // Brooch halves force damage.
        assert_eq!(f.effective_damage(20, DamageType::Force), 10);
        // Other damage types still flow at full.
        assert_eq!(f.effective_damage(20, DamageType::Slashing), 20);
    }

    #[test]
    fn boots_of_the_winterlands_halve_cold_damage() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert_eq!(f.effective_damage(20, DamageType::Cold), 20);
        f.pickup_item(&crate::items::item_template::BOOTS_OF_THE_WINTERLANDS);
        assert_eq!(f.effective_damage(20, DamageType::Cold), 10);
        // Force damage is unaffected.
        assert_eq!(f.effective_damage(20, DamageType::Force), 20);
    }

    #[test]
    fn periapt_of_proof_against_poison_zeros_damage_and_blocks_condition() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        // Baseline: fighter has no template-level poison immunity.
        assert_eq!(f.effective_damage(20, DamageType::Poison), 20);
        assert!(
            f.add_condition(Condition::Poisoned, ConditionTimer::Rounds(10)),
            "fighter has no template-level poison immunity"
        );
        f.remove_condition(Condition::Poisoned);
        // Periapt zeroes poison damage AND blocks the Poisoned install
        // in one trinket — both lanes wired through the new fields.
        f.pickup_item(&crate::items::item_template::PERIAPT_OF_PROOF_AGAINST_POISON);
        assert_eq!(
            f.effective_damage(20, DamageType::Poison),
            0,
            "periapt should zero poison damage"
        );
        assert!(
            !f.add_condition(Condition::Poisoned, ConditionTimer::Rounds(10)),
            "periapt should block the Poisoned install"
        );
        // Other damage types still flow at full.
        assert_eq!(f.effective_damage(20, DamageType::Slashing), 20);
        assert!(f.effectively_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn ring_of_mind_shielding_zeros_psychic_and_blocks_charm() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert_eq!(f.effective_damage(20, DamageType::Psychic), 20);
        f.pickup_item(&crate::items::item_template::RING_OF_MIND_SHIELDING);
        assert_eq!(
            f.effective_damage(20, DamageType::Psychic),
            0,
            "ring of mind shielding should zero psychic damage"
        );
        assert!(
            !f.add_condition(Condition::Charmed, ConditionTimer::Rounds(10)),
            "ring of mind shielding should block the Charmed install"
        );
        // Frightened still installs — the ring guards the Charmed lane
        // only, not the broader "mental" cohort.
        assert!(f.add_condition(Condition::Frightened, ConditionTimer::Rounds(10)));
    }

    #[test]
    fn item_immunity_short_circuits_resistance_lane() {
        // 5e: immunity zeroes damage outright. Even with a condition-
        // based resistance source active, the immunity lane wins and
        // the resistance halving never runs (no compound /2/0 path).
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        f.pickup_item(&crate::items::item_template::PERIAPT_OF_PROOF_AGAINST_POISON);
        assert_eq!(
            f.effective_damage(20, DamageType::Poison),
            0,
            "item immunity should short-circuit before resistance halving"
        );
    }

    #[test]
    fn robe_of_the_archmagi_grants_ac_and_save_bonus() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let base_save = f.total_item_bonuses().save;
        let base_ac = f.total_item_bonuses().ac;
        f.pickup_item(&crate::items::item_template::ROBE_OF_THE_ARCHMAGI);
        assert_eq!(f.total_item_bonuses().save, base_save + 2);
        assert_eq!(f.total_item_bonuses().ac, base_ac + 2);
    }

    #[test]
    fn item_resistance_respects_one_halving_rule() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        // Install a condition-based blanket resistance (Stoneskin).
        f.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        assert_eq!(f.effective_damage(20, DamageType::Force), 10);
        // Adding the brooch on top must not double-halve — 5e stacking
        // rule: only one halving applies per damage instance.
        f.pickup_item(&crate::items::item_template::BROOCH_OF_SHIELDING);
        assert_eq!(
            f.effective_damage(20, DamageType::Force),
            10,
            "item resistance must not stack with condition resistance"
        );
    }

    #[test]
    fn elemental_resistance_rings_halve_their_damage_type() {
        // The four-element resistance ring family (fire / cold / acid /
        // lightning) all ride the same `damage_resistances` lane. One
        // table-driven test sanity-checks each entry's typed halving
        // and confirms non-matching types still flow at full.
        use crate::items::item_template::{
            RING_OF_ACID_RESISTANCE, RING_OF_COLD_RESISTANCE, RING_OF_FIRE_RESISTANCE,
            RING_OF_LIGHTNING_RESISTANCE,
        };
        let cases: &[(&crate::items::item_template::Item, DamageType)] = &[
            (&RING_OF_FIRE_RESISTANCE, DamageType::Fire),
            (&RING_OF_COLD_RESISTANCE, DamageType::Cold),
            (&RING_OF_ACID_RESISTANCE, DamageType::Acid),
            (&RING_OF_LIGHTNING_RESISTANCE, DamageType::Lightning),
        ];
        for (item, dt) in cases {
            let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
            assert_eq!(
                f.effective_damage(20, *dt),
                20,
                "{} baseline should not have resistance",
                item.name
            );
            f.pickup_item(item);
            assert_eq!(
                f.effective_damage(20, *dt),
                10,
                "{} should halve {} damage",
                item.name,
                dt
            );
            // A non-matching elemental type still lands at full — the
            // resistance is single-type.
            let unrelated = match dt {
                DamageType::Fire => DamageType::Cold,
                _ => DamageType::Fire,
            };
            assert_eq!(
                f.effective_damage(20, unrelated),
                20,
                "{} should not halve {} damage",
                item.name,
                unrelated
            );
        }
    }

    #[test]
    fn elemental_resistance_rings_respect_one_halving_rule() {
        // Stacking with a condition-based resistance source must still
        // halve only once. Mirrors `item_resistance_respects_one_halving_rule`
        // for the elemental-ring family.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        f.pickup_item(&crate::items::item_template::RING_OF_FIRE_RESISTANCE);
        assert_eq!(
            f.effective_damage(20, DamageType::Fire),
            10,
            "ring + condition resistance must not stack into /4"
        );
    }

    #[test]
    fn weapon_plus_one_grants_attack_and_damage_bonus() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let base_attack = f.item_attack_bonus();
        let base_damage = f.item_damage_bonus();
        f.pickup_item(&crate::items::item_template::WEAPON_PLUS_ONE);
        assert_eq!(f.item_attack_bonus(), base_attack + 1);
        assert_eq!(f.item_damage_bonus(), base_damage + 1);
    }

    #[test]
    fn weapon_plus_two_grants_two_attack_and_two_damage() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::WEAPON_PLUS_TWO);
        assert_eq!(f.item_attack_bonus(), 2);
        assert_eq!(f.item_damage_bonus(), 2);
    }

    #[test]
    fn bracers_of_archery_grants_damage_only() {
        // RAW: bracers grant +2 damage on bow attacks. Engine collapses
        // the gate to "all damage rolls" but the attack lane stays 0.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::BRACERS_OF_ARCHERY);
        assert_eq!(f.item_attack_bonus(), 0, "bracers should not bump to-hit");
        assert_eq!(f.item_damage_bonus(), 2, "bracers should bump damage by 2");
    }

    #[test]
    fn ioun_stone_of_mastery_grants_attack_and_save_bonus() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let base_attack = f.item_attack_bonus();
        let base_save = f.total_item_bonuses().save;
        f.pickup_item(&crate::items::item_template::IOUN_STONE_OF_MASTERY);
        assert_eq!(f.item_attack_bonus(), base_attack + 1);
        assert_eq!(f.total_item_bonuses().save, base_save + 1);
    }

    #[test]
    fn sentinel_shield_grants_ac_and_save_bonus() {
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let base_ac = f.total_item_bonuses().ac;
        let base_save = f.total_item_bonuses().save;
        f.pickup_item(&crate::items::item_template::SENTINEL_SHIELD);
        assert_eq!(f.total_item_bonuses().ac, base_ac + 1);
        assert_eq!(f.total_item_bonuses().save, base_save + 1);
    }

    #[test]
    fn weapon_bonus_items_stack_linearly() {
        // Two `+1 Weapon`s sum to +2/+2 — sanity-check that the per-item
        // sum in `total_item_bonuses` honors the attack/damage lanes
        // alongside the existing AC/save/speed lanes.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::WEAPON_PLUS_ONE);
        f.pickup_item(&crate::items::item_template::WEAPON_PLUS_ONE);
        assert_eq!(f.item_attack_bonus(), 2);
        assert_eq!(f.item_damage_bonus(), 2);
    }

    #[test]
    fn slippers_of_spider_climbing_grants_spider_climb_buff() {
        // Slippers should install the SpiderClimbing condition on
        // pickup so the +30 ft speed bump flows through
        // `condition_speed_bonus` without an explicit cast.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let baseline_speed = f.speed();
        assert!(!f.has_condition(Condition::SpiderClimbing));
        f.pickup_item(&crate::items::item_template::SLIPPERS_OF_SPIDER_CLIMBING);
        assert!(
            f.has_condition(Condition::SpiderClimbing),
            "slippers should install SpiderClimbing on pickup"
        );
        // +30 ft (= 6 tiles * 5 ft) over the baseline.
        assert!(
            f.speed() > baseline_speed,
            "slippers should boost speed via SpiderClimbing"
        );
    }

    #[test]
    fn winged_boots_grants_flying_buff() {
        // Winged Boots should install Flying on pickup so the +60 ft
        // speed bump and ranged-attacker disadvantage flow through the
        // same condition the Fly spell installs.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        let baseline_speed = f.speed();
        assert!(!f.has_condition(Condition::Flying));
        f.pickup_item(&crate::items::item_template::WINGED_BOOTS);
        assert!(
            f.has_condition(Condition::Flying),
            "winged boots should install Flying on pickup"
        );
        // +60 ft over the baseline.
        assert!(
            f.speed() > baseline_speed + 30.0,
            "winged boots should boost speed by Flying's +60 ft"
        );
    }

    #[test]
    fn passive_item_condition_strips_on_drop_when_unique() {
        // Removing the slippers strips the SpiderClimbing condition when
        // no other carried item still grants it — keeps the install lane
        // idempotent across multi-item stacks.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::SLIPPERS_OF_SPIDER_CLIMBING);
        assert!(f.has_condition(Condition::SpiderClimbing));
        assert!(f.remove_item_by_name("Slippers of Spider Climbing"));
        assert!(
            !f.has_condition(Condition::SpiderClimbing),
            "dropping the only slipper should strip the SpiderClimbing buff"
        );
    }

    #[test]
    fn passive_item_condition_persists_when_a_second_grantor_remains() {
        // If a second item also grants the same passive condition,
        // dropping one should NOT strip the buff — the remaining grantor
        // keeps it pinned. We use two Winged Boots (Flying) — a contrived
        // case but the right shape for the "multiple grantors" path.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::WINGED_BOOTS);
        f.pickup_item(&crate::items::item_template::WINGED_BOOTS);
        assert!(f.has_condition(Condition::Flying));
        // Drop one pair — the other still grants Flying.
        f.remove_item_by_name("Winged Boots");
        assert!(
            f.has_condition(Condition::Flying),
            "Flying should remain while a second Winged Boots still grants it"
        );
        // Drop the second — now the buff strips.
        f.remove_item_by_name("Winged Boots");
        assert!(
            !f.has_condition(Condition::Flying),
            "Flying should strip when the last grantor is dropped"
        );
    }

    #[test]
    fn passive_item_condition_reinstalls_on_long_rest() {
        // Long rest clears the condition map; the reinstall hook should
        // bring back item-granted passive conditions so the wearer wakes
        // up still flying / spider-climbing / etc.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        f.pickup_item(&crate::items::item_template::WINGED_BOOTS);
        assert!(f.has_condition(Condition::Flying));
        // Simulate a mid-encounter dispel that strips Flying.
        f.remove_condition(Condition::Flying);
        assert!(!f.has_condition(Condition::Flying));
        // Long rest reinstalls the passive item buff.
        f.long_rest();
        assert!(
            f.has_condition(Condition::Flying),
            "long_rest should re-install Flying from Winged Boots"
        );
    }

    #[test]
    fn cloak_of_etherealness_grants_blanket_damage_resistance() {
        // Cloak installs `DamageResistant` so every incoming damage type
        // is halved through the existing condition lane.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert_eq!(f.effective_damage(20, DamageType::Force), 20);
        f.pickup_item(&crate::items::item_template::CLOAK_OF_ETHEREALNESS);
        assert!(f.has_condition(Condition::DamageResistant));
        assert_eq!(f.effective_damage(20, DamageType::Force), 10);
        assert_eq!(f.effective_damage(20, DamageType::Slashing), 10);
    }

    #[test]
    fn cloak_of_displacement_grants_displaced_buff() {
        // Cloak installs `Displaced` on pickup so attackers eat
        // disadvantage through the existing condition lane.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert!(!f.has_condition(Condition::Displaced));
        f.pickup_item(&crate::items::item_template::CLOAK_OF_DISPLACEMENT);
        assert!(
            f.has_condition(Condition::Displaced),
            "cloak should install Displaced on pickup"
        );
    }

    #[test]
    fn scarab_of_protection_grants_dual_condition_immunity() {
        // Scarab folds Charmed AND Frightened immunity through the
        // condition-immunity lane and stacks a +1 save bump.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert!(!f.effectively_immune_to_condition(Condition::Charmed));
        assert!(!f.effectively_immune_to_condition(Condition::Frightened));
        let base_save = f.total_item_bonuses().save;
        f.pickup_item(&crate::items::item_template::SCARAB_OF_PROTECTION);
        assert!(
            f.effectively_immune_to_condition(Condition::Charmed),
            "scarab should grant Charmed immunity"
        );
        assert!(
            f.effectively_immune_to_condition(Condition::Frightened),
            "scarab should grant Frightened immunity"
        );
        assert_eq!(
            f.total_item_bonuses().save,
            base_save + 1,
            "scarab should add a +1 save bonus"
        );
    }

    #[test]
    fn hp_roll_floors_at_one_so_fresh_spawns_are_alive() {
        // Regression: a template whose hit-die expression evaluates
        // to 0 (or below) must still spawn the actor at 1 HP, so it
        // enters combat in the `HpState::Active` lane with `hitpoints
        // > 0`. The canonical case is the Hawk's RAW `1d4 - 1` (and
        // any future CR-0 tiny beast with a similarly minimal hit
        // pool) — an unlucky d4 = 1 would otherwise floor the roll
        // to 0 and silently park the spawn outside `is_combat_active`.
        use crate::actions::default_actions::DEFAULT_ACTIONS;
        let ct = CreatureTemplate {
            // `0` evaluates to a flat 0 via `DiceExpr::constant(0)`;
            // mirrors the worst-case d4 = 1 → `1 - 1 = 0` for the
            // hawk's hit expression but pins the input deterministically
            // so the test doesn't ride on RNG quirks.
            hitpoints: "0".parse().unwrap(),
            actions: DEFAULT_ACTIONS.clone(),
            ..CreatureTemplate::defaults()
        };
        // Leak a `'static` borrow so `from_creature_template`'s
        // `&'static CreatureTemplate` bound is satisfied. The test
        // is single-shot — the leak is bounded to one allocation.
        let leaked: &'static CreatureTemplate = Box::leak(Box::new(ct));
        let actor = ActorInstance::from_creature_template(
            leaked,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            actor.hitpoints(),
            1,
            "HP roll must floor at 1 so the spawn is combat-active"
        );
        assert!(
            actor.is_combat_active(),
            "actor with hit expression evaluating to 0 must still be combat-active"
        );
        assert_eq!(
            actor.max_hitpoints(),
            1,
            "max_hitpoints must reflect the floored base, not the raw 0"
        );
    }

    #[test]
    fn ring_of_heroism_installs_heroic_buff() {
        // Ring installs the Heroic condition on pickup; the engine's
        // existing `Heroic` lane covers the Frightened-immunity rider.
        let mut f = make(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE);
        assert!(!f.has_condition(Condition::Heroic));
        f.pickup_item(&crate::items::item_template::RING_OF_HEROISM);
        assert!(
            f.has_condition(Condition::Heroic),
            "ring should install Heroic on pickup"
        );
        assert!(
            f.effectively_immune_to_condition(Condition::Frightened),
            "Heroic should fold into Frightened immunity"
        );
    }

    /// Resistance and vulnerability to the same damage type cancel,
    /// whichever lane the resistance came from.
    ///
    /// The condition and item lanes always got this right, because the
    /// old pipeline doubled first and then let those two lanes halve.
    /// The passive-feature lane did not: it shared its gate with the
    /// template's own resistance, so on a vulnerable creature the whole
    /// gate went dark and the feature's halving vanished. A creature
    /// with a feature-granted resistance and a template vulnerability to
    /// the same type took double instead of full — the exact opposite of
    /// the direction the extra feature should push.
    #[test]
    fn a_resistance_and_a_vulnerability_to_the_same_type_cancel() {
        // The skeleton is the engine's canonical vulnerable creature:
        // vulnerable to bludgeoning, and to nothing else.
        let mut s = make(&SKELETON_TEMPLATE);
        assert_eq!(
            s.effective_damage(10, DamageType::Bludgeoning),
            20,
            "vulnerability alone doubles"
        );

        // Condition lane: Rage's blanket physical resistance.
        s.add_condition(Condition::Raging, ConditionTimer::Rounds(10));
        assert_eq!(
            s.effective_damage(10, DamageType::Bludgeoning),
            10,
            "a condition-granted resistance cancels the vulnerability"
        );
        s.remove_condition(Condition::Raging);

        assert_eq!(
            s.effective_damage(10, DamageType::Bludgeoning),
            20,
            "and the vulnerability comes back when the rage drops"
        );

        // Passive-feature lane — the one that was wrong. The Shadow
        // Demon is vulnerable to radiant; the Celestial Warlock's
        // Radiant Soul grants resistance to it. Nothing on the roster
        // pairs the two today, which is why the bug went unnoticed and
        // why the pairing has to be built by hand here.
        let mut d = make(&SHADOW_DEMON_TEMPLATE);
        assert_eq!(
            d.effective_damage(10, DamageType::Radiant),
            20,
            "the shadow demon is vulnerable to radiant"
        );
        d.grant_feature_for_test(crate::actions::class_features::RADIANT_SOUL_TAG);
        assert_eq!(
            d.effective_damage(10, DamageType::Radiant),
            10,
            "a feature-granted resistance cancels it like any other"
        );
    }

    /// However many sources agree that a creature is resistant, the
    /// damage is halved once.
    #[test]
    fn stacked_resistances_still_only_halve_once() {
        let mut z = make(&ZOMBIE_TEMPLATE);
        // Two independent condition-driven blanket resistances at once.
        z.add_condition(Condition::Raging, ConditionTimer::Rounds(10));
        z.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        assert_eq!(
            z.effective_damage(20, DamageType::Slashing),
            10,
            "two resistances are still one halving"
        );
    }
}
