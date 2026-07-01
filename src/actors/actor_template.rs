use crate::actions::action_template::Action;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, DiceExpr, Roller};
use crate::engine::side_effects::Resource;
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageModifier, DamageType, Language, Size, Skill,
    SpecialSense,
};
use crate::engine::util::modifier_from_score;
use crate::items::item_template::{Item, ItemBonuses};
use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::actions::class_features::{
    BATTLE_MASTER_MANEUVERS, RELENTLESS_ENDURANCE_TAG, SHORT_REST_FEATURES,
    SORCEROUS_RESTORATION_TAG,
};

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

/// Conditions whose presence grants damage-type immunity. Each row is
/// `(condition, &[damage types zeroed])`. Read by
/// `has_condition_immunity` so adding a new "condition X makes you
/// immune to damage type Y" rider lands as a one-line entry instead of
/// another `if dt == ... && self.has_condition(...)` branch in
/// `effective_damage`. The Mind Blank → psychic and Silenced → thunder
/// immunities both live here.
const TYPED_IMMUNITY_CONDITIONS: &[(Condition, &[DamageType])] = &[
    // 5e Mind Blank: psychic-damage immunity for the duration.
    (Condition::MindBlanked, &[DamageType::Psychic]),
    // 5e Silence: any creature entirely inside the silence sphere is
    // immune to thunder damage (the magical hush absorbs sonic effects).
    (Condition::Silenced, &[DamageType::Thunder]),
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
    (Condition::Petrified, &[DamageType::Poison]),
];

/// Conditions whose resistance only applies to a curated damage-type
/// subset. Each row is `(condition, &[damage types resisted])`. Read by
/// `has_condition_resistance` so a new Investiture-style buff lands as a
/// one-line entry without touching the damage-pipeline code.
const TYPED_RESISTANCE_CONDITIONS: &[(Condition, &[DamageType])] = &[
    (Condition::InvestedInFlame, &[DamageType::Fire]),
    (Condition::InvestedInIce, &[DamageType::Cold]),
    (
        Condition::InvestedInStone,
        &[
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ],
    ),
    (Condition::Purified, &[DamageType::Poison]),
    (
        Condition::Raging,
        &[
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ],
    ),
    // 5e Tasha's Otherworldly Guise (celestial flavor): radiant + poison
    // resistance from the divine-aligned form. Folded into the same lane
    // as the other typed-resistance buffs so the damage pipeline halves
    // both incoming radiant and incoming poison damage cleanly.
    (
        Condition::OtherworldlyGuised,
        &[DamageType::Radiant, DamageType::Poison],
    ),
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
    pub damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// Saving throws this creature is proficient with. Optional;
    /// templates that don't care can leave this empty (default new).
    pub proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions this creature is immune to (e.g. zombies vs Charm,
    /// elementals vs Poisoned).
    pub condition_immunities: HashSet<Condition>,
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
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves for half damage, take 0
    /// on a pass and half on a fail instead of half / full.
    pub has_evasion: bool,
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
    /// 5e Half-Orc Savage Attacks: on a critical melee weapon hit, roll
    /// one additional weapon damage die. Mechanically identical to
    /// `brutal_critical_dice = 1` but exposed as a separate flag so the
    /// half-orc racial doesn't get conflated with the barbarian class
    /// feature in templates that combine both (e.g. a half-orc barbarian
    /// stacks the dice). Read at the same crit-damage site in
    /// `engine::attack` next to `brutal_critical_dice`.
    pub has_savage_attacks: bool,
    /// 5e Dwarven Resilience: advantage on saving throws against poison
    /// AND resistance to poison damage. Read by `compute_save_mode`
    /// (advantage clause) and `effective_damage` (resistance clause).
    /// A single flag drives both halves because RAW: both clauses share
    /// the same trait gate.
    pub has_dwarven_resilience: bool,
    /// 5e Gnome Cunning (Rock / Forest / Deep Gnome racial): advantage on
    /// Intelligence, Wisdom, and Charisma saving throws against magic.
    /// We don't tag saves by "magic vs mundane" in this engine, so we
    /// approximate by granting blanket advantage on INT / WIS / CHA
    /// saves. The false-positive surface is small — most non-magical
    /// effects targeting those abilities (skill checks, social mods)
    /// don't route through `roll_save`. Read by `compute_save_mode`.
    pub has_gnome_cunning: bool,
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
            proficient_saves: HashSet::new(),
            condition_immunities: HashSet::new(),
            features: HashSet::new(),
            regen_per_round: 0,
            regen_suppressors: HashSet::new(),
            legendary_resistances: 0,
            has_evasion: false,
            has_uncanny_dodge: false,
            has_deflect_missiles: false,
            has_displacement: false,
            has_danger_sense: false,
            has_pack_tactics: false,
            has_magic_resistance: false,
            recharge_abilities: Vec::new(),
            legendary_actions_per_round: 0,
            has_extra_attack: false,
            brutal_critical_dice: 0,
            crit_threshold: 20,
            has_lucky: false,
            has_brave: false,
            has_fey_ancestry: false,
            has_aura_of_protection: false,
            has_aura_of_courage: false,
            has_aura_of_devotion: false,
            has_savage_attacks: false,
            has_dwarven_resilience: false,
            has_gnome_cunning: false,
            draconic_ancestry: None,
            sorcery_points: 0,
            death_burst: None,
            natural_melee_reflect: None,
            innate_conditions: Vec::new(),
        }
    }
}

/// 5e shorthand: "resistance to bludgeoning, piercing, and slashing damage
/// from non-magical attacks." We don't track magical-vs-mundane weapon
/// distinctions, so the resistance lands on the three physical damage types
/// directly. Returns a fully assembled `damage_modifiers` map seeded with
/// the BPS triplet and extended by `overlays`; mirrors
/// `fire_elementals::elemental_damage_modifiers` in shape but without the
/// elemental's poison-immunity baseline.
///
/// Replaces the hand-copied B/P/S triplet that appeared in dozens of
/// incorporeal-undead / fiend / extraplanar templates (Wraith, Specter,
/// Banshee, Ghost, etc.). Overlays win on collision so a future "promote
/// BPS to Immunity" variant can land cleanly without touching the helper.
pub fn non_magical_physical_resistances(
    overlays: impl IntoIterator<Item = (DamageType, DamageModifier)>,
) -> HashMap<DamageType, DamageModifier> {
    let mut m = HashMap::from([
        (DamageType::Bludgeoning, DamageModifier::Resistance),
        (DamageType::Piercing, DamageModifier::Resistance),
        (DamageType::Slashing, DamageModifier::Resistance),
    ]);
    m.extend(overlays);
    m
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

#[derive(Clone)]
#[allow(dead_code)]
pub struct ActorInstance {
    name: String,
    location: Coordinate,
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
    /// 5e temporary hit points. Damage drains temp HP before regular HP.
    /// Doesn't stack: a new grant replaces existing temp HP only if
    /// larger. Cleared on long rest.
    temp_hp: u32,
    level: u32,
    xp: u32,
    /// Saving-throw proficiencies — adds proficiency bonus to roll_save.
    proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions the actor is wholly immune to.
    condition_immunities: HashSet<Condition>,
    /// Class-feature tags currently available (consumed on use, refreshed
    /// on long rest).
    features_remaining: HashSet<&'static str>,
    features_max: HashSet<&'static str>,
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
    /// Sneak Attack guard — true if the rogue has spent their once-per-turn
    /// sneak this turn. Cleared at turn-start by `reset_for_new_round`.
    sneak_attack_used: bool,
    /// Colossus Slayer guard — symmetric with `sneak_attack_used`. True
    /// if a Hunter ranger has spent their once-per-turn Colossus Slayer
    /// rider this turn. Cleared at turn-start by `reset_for_new_round`.
    colossus_slayer_used: bool,
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
    /// Identity of the actor that has Charmed this actor (if any). 5e
    /// Charmed: the target cannot make attacks against the charmer. We
    /// store the id rather than just the condition flag so the
    /// validation site knows who to block. Cleared when the Charmed
    /// condition is removed.
    charmed_by: Option<usize>,
    /// 5e Fighter Indomitable — one-shot "reroll the next failed save"
    /// marker. Set by the Indomitable action; consumed at the save
    /// site (`EncounterInstance::roll_save`) on a fail. Refreshed by
    /// long rest along with the feature pool.
    indomitable_pending: bool,
    /// Identity of the paladin that has Compelled this actor to a duel.
    /// Paired with the `Dueled` condition: attacks against anyone *other*
    /// than this id are at disadvantage. Cleared when the Dueled
    /// condition lifts.
    dueled_by: Option<usize>,
    /// Identity of the fighter that has goaded this actor (5e Battle
    /// Master Goading Attack). Paired with the `Goaded` condition:
    /// attacks against anyone *other* than this id are at disadvantage.
    /// Cleared when the Goaded condition lifts. Mirrors `dueled_by` —
    /// same mechanical envelope, distinct field so a creature can
    /// simultaneously be dueled by a paladin and goaded by a fighter
    /// without the two getting confused.
    goaded_by: Option<usize>,
    /// Identity of the fighter that has distracted this actor (5e Battle
    /// Master Distracting Strike). Paired with the `Distracted`
    /// condition: attack rolls against this actor by anyone *other* than
    /// this id have advantage. Cleared when the Distracted condition
    /// lifts. Mirrors `goaded_by` in shape but reversed in polarity —
    /// Distracted is a target-side advantage rider rather than an
    /// attacker-side disadvantage one.
    distracted_by: Option<usize>,
    /// Identity of the paladin that has sworn Vow of Enmity against this
    /// actor (5e Vengeance Paladin Channel Divinity, lv3 subclass).
    /// Paired with the `Sworn` condition: attack rolls against this actor
    /// by this paladin (and only this paladin) get advantage. Same flag-
    /// plus-link shape as `dueled_by` / `goaded_by` / `distracted_by`,
    /// but positive-polarity: a *match* on the link grants the swearer
    /// advantage, rather than a *mismatch* imposing disadvantage on
    /// non-counterparties. Cleared when the Sworn condition lifts.
    sworn_by: Option<usize>,
    /// 5e Legendary Resistance — remaining auto-pass charges on failed
    /// saves this long rest. Refreshed to `legendary_resistance_max` on
    /// long rest. See `EncounterInstance::roll_save` for the trigger site.
    legendary_resistance_remaining: u32,
    legendary_resistance_max: u32,
    /// Identity of the caster who has bonded with this actor via Warding
    /// Bond (5e level-2 abjuration). Paired with the `WardingBonded`
    /// condition: when this actor takes damage, the same amount is
    /// mirrored onto the partner via the damage-reflect site in
    /// `DealDamage::apply`. Cleared when the WardingBonded condition is
    /// removed (timer expiry / dispel / either party drops).
    warding_partner: Option<usize>,
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves that deal half on pass,
    /// take 0 on pass and half on fail.
    has_evasion: bool,
    /// 5e Uncanny Dodge (Rogue 5): reaction to halve damage from one
    /// visible attack per round.
    has_uncanny_dodge: bool,
    /// 5e Monk Deflect Missiles (level 3): reaction to reduce ranged
    /// weapon damage by 1d10 + DEX + level.
    has_deflect_missiles: bool,
    has_displacement: bool,
    has_danger_sense: bool,
    has_pack_tactics: bool,
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
    /// 5e Half-Orc Savage Attacks. See `CreatureTemplate` docs.
    has_savage_attacks: bool,
    /// 5e Dwarven Resilience. See `CreatureTemplate` docs.
    has_dwarven_resilience: bool,
    /// 5e Gnome Cunning. See `CreatureTemplate` docs.
    has_gnome_cunning: bool,
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
            temp_hp: 0,
            level: 1,
            xp: 0,
            proficient_saves: ct.proficient_saves.clone(),
            condition_immunities: ct.condition_immunities.clone(),
            features_remaining: ct.features.clone(),
            features_max: ct.features.clone(),
            attack_bonus_buff: 0,
            save_bonus_buff: 0,
            damage_bonus_buff: 0,
            sneak_attack_used: false,
            colossus_slayer_used: false,
            has_taken_turn_in_combat: false,
            relentless_rage_dc: 10,
            help_grants: HashMap::new(),
            regen_per_round: ct.regen_per_round,
            regen_suppressors: ct.regen_suppressors.clone(),
            regen_suppressed: false,
            mirror_images: 0,
            charmed_by: None,
            indomitable_pending: false,
            dueled_by: None,
            goaded_by: None,
            distracted_by: None,
            sworn_by: None,
            legendary_resistance_remaining: ct.legendary_resistances,
            legendary_resistance_max: ct.legendary_resistances,
            warding_partner: None,
            has_evasion: ct.has_evasion,
            has_uncanny_dodge: ct.has_uncanny_dodge,
            has_deflect_missiles: ct.has_deflect_missiles,
            has_displacement: ct.has_displacement,
            has_danger_sense: ct.has_danger_sense,
            has_pack_tactics: ct.has_pack_tactics,
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
            has_savage_attacks: ct.has_savage_attacks,
            has_dwarven_resilience: ct.has_dwarven_resilience,
            has_gnome_cunning: ct.has_gnome_cunning,
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

    /// Who has this actor Charmed (if anyone). Used to gate attack-roll
    /// validation: a Charmed actor can't attack their charmer.
    pub fn charmed_by(&self) -> Option<usize> {
        self.charmed_by
    }

    pub fn set_charmed_by(&mut self, id: Option<usize>) {
        self.charmed_by = id;
    }

    /// Identity of the paladin that has this actor locked in a Compelled
    /// Duel (if any). Read by `compute_attack_mode` to apply the
    /// "disadvantage on attacks vs anyone other than the duelist" rider.
    pub fn dueled_by(&self) -> Option<usize> {
        self.dueled_by
    }

    pub fn set_dueled_by(&mut self, id: Option<usize>) {
        self.dueled_by = id;
    }

    /// Identity of the fighter that has goaded this actor (Goading
    /// Attack maneuver). Read by `compute_attack_mode` to apply the
    /// "disadvantage on attacks vs anyone other than the goader" rider.
    pub fn goaded_by(&self) -> Option<usize> {
        self.goaded_by
    }

    pub fn set_goaded_by(&mut self, id: Option<usize>) {
        self.goaded_by = id;
    }

    /// Identity of the fighter that has distracted this actor
    /// (Distracting Strike maneuver). Read by `compute_attack_mode` to
    /// grant advantage to any attacker *other* than this fighter.
    /// Symmetric to `goaded_by` but target-side advantage rather than
    /// attacker-side disadvantage.
    pub fn distracted_by(&self) -> Option<usize> {
        self.distracted_by
    }

    pub fn set_distracted_by(&mut self, id: Option<usize>) {
        self.distracted_by = id;
    }

    /// Identity of the paladin that has sworn Vow of Enmity on this
    /// actor (Vengeance Paladin Channel Divinity). Read by
    /// `compute_attack_mode` to grant advantage on the swearer's attack
    /// rolls against this target. Positive-polarity sibling of
    /// `distracted_by` (which grants advantage to *every other*
    /// attacker) — Vow of Enmity only buffs the paladin who swore it.
    pub fn sworn_by(&self) -> Option<usize> {
        self.sworn_by
    }

    pub fn set_sworn_by(&mut self, id: Option<usize>) {
        self.sworn_by = id;
    }

    /// Caster id this actor is currently Warding-Bonded to (5e
    /// `WardingBonded` condition). `None` when the bond is inactive.
    /// Read by `DealDamage::apply` to mirror damage onto the partner.
    pub fn warding_partner(&self) -> Option<usize> {
        self.warding_partner
    }

    /// Set / clear the Warding Bond partner. Cleared automatically when
    /// the `WardingBonded` condition is removed via `remove_condition`.
    pub fn set_warding_partner(&mut self, id: Option<usize>) {
        self.warding_partner = id;
    }

    pub fn has_evasion(&self) -> bool {
        self.has_evasion
    }

    pub fn has_uncanny_dodge(&self) -> bool {
        self.has_uncanny_dodge
    }

    pub fn has_deflect_missiles(&self) -> bool {
        self.has_deflect_missiles
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

    /// 5e Half-Orc Savage Attacks — adds one extra weapon damage die on a
    /// critical melee hit. Read at the crit-damage site alongside
    /// `brutal_critical_dice`; the two stack additively on a half-orc
    /// barbarian.
    pub fn has_savage_attacks(&self) -> bool {
        self.has_savage_attacks
    }

    /// 5e Dwarven Resilience — advantage on saves vs poison AND resistance
    /// to poison damage. Read by `compute_save_mode` (advantage clause) and
    /// `effective_damage` (resistance clause).
    pub fn has_dwarven_resilience(&self) -> bool {
        self.has_dwarven_resilience
    }

    /// 5e Gnome Cunning — advantage on INT / WIS / CHA saves vs magic.
    /// Approximated as advantage on every INT / WIS / CHA save (saves
    /// rarely originate from non-magical sources in this engine).
    pub fn has_gnome_cunning(&self) -> bool {
        self.has_gnome_cunning
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
    pub fn crit_threshold(&self) -> u32 {
        self.crit_threshold
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

    /// First action in the actor's list whose `name()` matches `name`.
    pub fn find_action(&self, name: &str) -> Option<&'static (dyn Action + Send + Sync)> {
        self.actions.iter().find(|a| a.name() == name).copied()
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
        self.hitpoints = self.max_hitpoints();
        self.temp_hp = 0;
        self.spell_slot_manager.restore_spell_slots();
        self.conditions.clear();
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

        for tag in SHORT_REST_FEATURES.iter().chain(BATTLE_MASTER_MANEUVERS.iter()) {
            if self.features_max.contains(tag) {
                self.features_remaining.insert(tag);
            }
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
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        if amount > self.temp_hp {
            self.temp_hp = amount;
        }
        self.temp_hp
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
        // Template-level modifier (resistance / immunity / vulnerability).
        let modifier = self.damage_modifiers.get(&dt).copied();
        // Immunity from any source zeroes damage outright.
        if matches!(modifier, Some(DamageModifier::Immunity)) {
            return 0;
        }
        // Condition-driven typed immunity (Mind Blank → Psychic, Silenced
        // → Thunder, future entries). Table-driven via
        // `TYPED_IMMUNITY_CONDITIONS` so adding a new immunity rider is a
        // one-line tuple instead of another `if dt == ... && ...` branch
        // here. Mirrors the `TYPED_RESISTANCE_CONDITIONS` cohort.
        if self.has_condition_immunity(dt) {
            return 0;
        }
        // Item-granted immunity (Periapt of Proof against Poison →
        // poison, Ring of Mind Shielding → psychic). Folded in next to
        // the template / condition immunity sources above — no "one
        // halving" stacking concern since immunity short-circuits the
        // pipeline before any resistance roll fires.
        if self.item_immunity_to_damage(dt) {
            return 0;
        }
        // 5e Monk Purity of Body (level 10). Passive: immune to poison
        // damage AND the Poisoned condition. The condition half lives at
        // `dynamic_immunity_to(Poisoned)`; the damage half folds in here
        // next to the item / condition immunity sources — same "immunity
        // trumps everything" short-circuit. Only fires for the Poison
        // type; the tag has no effect on non-poison damage.
        if dt == DamageType::Poison
            && self.has_passive_feature(
                crate::actions::class_features::PURITY_OF_BODY_TAG,
            )
        {
            return 0;
        }
        // 5e Dwarven Resilience: resistance to poison damage. Folds into
        // the same template-resistance lane below so the 5e "only one
        // halving" rule still holds when a creature has resilience AND
        // a condition-based halver active (e.g. a dwarf barbarian raging
        // wouldn't get double resistance to poison — only one /2).
        let template_resisted = matches!(modifier, Some(DamageModifier::Resistance))
            || (dt == DamageType::Poison && self.has_dwarven_resilience);
        // Start with raw and apply vulnerability / template resistance.
        let mut amt = match modifier {
            Some(DamageModifier::Vulnerability) => raw.saturating_mul(2),
            Some(DamageModifier::Resistance) => raw / 2,
            _ if template_resisted => raw / 2,
            _ => raw,
        };
        // Collect condition-based resistance sources. Per 5e stacking
        // rules, only one halving applies regardless of how many sources
        // grant resistance. If the template (or dwarven resilience) has
        // already halved, condition-based halving is skipped.
        let condition_resistance = !template_resisted && self.has_condition_resistance(dt);
        if condition_resistance {
            amt /= 2;
        }
        // Item-granted resistance (Brooch of Shielding → force, Boots
        // of the Winterlands → cold). Honors the same "one halving"
        // rule — only fires if neither template-level nor condition-
        // based resistance has already halved the amount. Empty
        // `damage_resistances` on every loot-pool item makes this a
        // cheap walk for the common case.
        if !template_resisted && !condition_resistance && self.item_resistance_to(dt) {
            amt /= 2;
        }
        amt
    }

    /// True iff the actor holds a condition that grants outright immunity
    /// to damage of type `dt`. Walks `TYPED_IMMUNITY_CONDITIONS` — each
    /// row pairs a condition with the damage types it zeroes. Currently
    /// covers Mind Blank (psychic) and Silence (thunder); a new immunity
    /// rider adds a one-line tuple entry.
    pub fn has_condition_immunity(&self, dt: DamageType) -> bool {
        TYPED_IMMUNITY_CONDITIONS
            .iter()
            .any(|(c, types)| self.has_condition(*c) && types.contains(&dt))
    }

    /// True iff the actor holds a condition that grants resistance to
    /// damage of type `dt`. Walks two cohorts:
    /// - `BLANKET_RESISTANCE_CONDITIONS`: conditions that resist *every*
    ///   damage type (Stoneskin / Globe of Invulnerability / Warding
    ///   Bond / Petrified).
    /// - `TYPED_RESISTANCE_CONDITIONS`: conditions whose resistance only
    ///   applies to a curated subset of damage types (Investiture of
    ///   Flame → Fire, Raging → physical trio, Purified → Poison).
    ///
    /// Centralizes the per-condition resistance lookup so a new Investiture
    /// spell only needs a one-line entry in the typed cohort, and the
    /// `effective_damage` site stays a single boolean read.
    pub fn has_condition_resistance(&self, dt: DamageType) -> bool {
        if BLANKET_RESISTANCE_CONDITIONS
            .iter()
            .any(|c| self.has_condition(*c))
        {
            return true;
        }
        // 5e Barbarian Path of the Totem Warrior — Bear Totem Spirit
        // (level 3). While raging, resistance to every damage type except
        // psychic. Folded into the condition-resistance lane so the
        // standard "one halving per damage instance" rule still holds
        // (Bear Totem doesn't stack with a separate template resistance,
        // and is short-circuited by the BLANKET cohort above so Stoneskin
        // / Globe / Petrified still win at the gate above).
        if dt != DamageType::Psychic
            && self.has_condition(Condition::Raging)
            && self.has_passive_feature(crate::actions::class_features::BEAR_TOTEM_TAG)
        {
            return true;
        }
        TYPED_RESISTANCE_CONDITIONS.iter().any(|(c, types)| {
            self.has_condition(*c) && types.contains(&dt)
        })
    }

    pub fn damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        self.damage_modifiers.get(&dt).copied()
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
        self.proficient_saves.contains(&ability)
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
        match c {
            Condition::Frightened => {
                self.has_condition(Condition::Heroic)
                    || self.has_condition(Condition::Purified)
                    || self.has_condition(Condition::OtherworldlyGuised)
                    || self.has_brave
            }
            Condition::Charmed => {
                self.has_condition(Condition::MindBlanked)
                    || self.has_condition(Condition::Purified)
                    || self.has_condition(Condition::OtherworldlyGuised)
                    || self.has_fey_ancestry
            }
            Condition::Asleep => self.has_fey_ancestry,
            Condition::Poisoned => {
                self.has_condition(Condition::Purified)
                    || self.has_condition(Condition::OtherworldlyGuised)
                    // 5e Petrified RAW: "The creature is immune to
                    // poison and disease..." The damage-type half lives
                    // on `TYPED_IMMUNITY_CONDITIONS`; the condition
                    // half lives here so an attempt to install a
                    // fresh `Poisoned` condition on a stone creature
                    // no-ops at the `add_condition` chokepoint.
                    || self.has_condition(Condition::Petrified)
                    // 5e Monk Purity of Body (level 10). Passive: immune
                    // to disease and poison. The Poisoned-condition half
                    // lives here alongside Purified / Petrified; the
                    // poison-damage half lives in `effective_damage`
                    // next to the other passive-feature typed-immunity
                    // gates.
                    || self.has_passive_feature(
                        crate::actions::class_features::PURITY_OF_BODY_TAG,
                    )
            }
            // 5e Freedom of Movement: holders are immune to magical
            // movement restraint. Mirrors the Ring of Free Action item
            // immunity (which goes through `item_immunity_to` instead),
            // but condition-driven so concentration / dispel can rip it.
            Condition::Paralyzed | Condition::Restrained | Condition::Grappled => {
                self.has_condition(Condition::Footloose)
            }
            _ => false,
        }
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
        let is_new = !self.conditions.contains_key(&c);
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
        let removed = self.conditions.remove(&c).is_some();
        if removed {
            // Keep tightly-linked auxiliary state in sync with the
            // primary condition flag.
            match c {
                Condition::Charmed => self.charmed_by = None,
                Condition::MirroredImages => self.mirror_images = 0,
                Condition::Dueled => self.dueled_by = None,
                Condition::Goaded => self.goaded_by = None,
                Condition::Distracted => self.distracted_by = None,
                Condition::Sworn => self.sworn_by = None,
                Condition::WardingBonded => self.warding_partner = None,
                _ => {}
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
                    // (charmed_by, mirror_images) clears too.
                    self.remove_condition(c);
                    expired.push(c);
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

    pub fn can_consume_resource(&self, resource: Resource) -> bool {
        let action_blocked = self.is_incapacitated();
        match resource {
            Resource::Movement(amt) => {
                if action_blocked {
                    return false;
                }
                // Conditions that zero out movement entirely.
                if self.conditions.keys().any(|c| c.zeros_movement()) {
                    return false;
                }
                amt <= self.movement
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

    pub fn has_reaction(&self) -> bool {
        self.reaction_slots >= 1 && !self.conditions.keys().any(|c| c.blocks_reactions())
    }

    pub fn consume_resource(&mut self, resource: Resource) -> bool {
        if !self.can_consume_resource(resource) {
            return false;
        }
        match resource {
            Resource::Movement(amt) => self.movement -= amt,
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
        let raw_base = self.base_ac as i32 + self.total_item_bonuses().ac;
        let floor = self.ac_floor();
        (raw_base.max(floor) + self.condition_ac_bonus()).max(0) as u32
    }

    /// Flat AC contribution from active conditions. Shield of Faith
    /// (+2 from the spell), Shielded (+5 from the Shield reaction spell
    /// — RAW value), Mage Armored (sets minimum AC to 13 + DEX, which
    /// we approximate as a flat top-up — see `armor_class`).
    pub fn condition_ac_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::ShieldOfFaith) {
            bonus += 2;
        }
        if self.has_condition(Condition::Shielded) {
            bonus += 5;
        }
        if self.has_condition(Condition::Hasted) {
            bonus += 2;
        }
        if self.has_condition(Condition::Slowed) {
            bonus -= 2;
        }
        // 5e Warding Bond: +1 AC while bonded.
        if self.has_condition(Condition::WardingBonded) {
            bonus += 1;
        }
        // 5e Tasha's Otherworldly Guise: the extraplanar form's shell
        // grants a flat +2 AC bump while the buff is up.
        if self.has_condition(Condition::OtherworldlyGuised) {
            bonus += 2;
        }
        bonus
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
        (self.base_hitpoints as i32 + bonus).max(1) as u32
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
        // 5e Haste doubles speed; Slow halves it. If both happen to be
        // active (e.g. cross-cast), they cancel back to base — applying
        // the factor multiplicatively keeps the math symmetric.
        let mut factor = 1.0_f32;
        if self.has_condition(Condition::Hasted) {
            factor *= 2.0;
        }
        if self.has_condition(Condition::Slowed) {
            factor *= 0.5;
        }
        raw * factor
    }

    /// Sum of all flat speed bonuses contributed by active conditions. One
    /// chokepoint so a new speed-buff condition (Longstrider, Expeditious
    /// Retreat, Fly, Spider Climb, Investiture of Wind, …) lands as a
    /// one-line entry instead of an ad-hoc branch in `speed()`.
    ///
    /// 5e RAW values:
    ///   - Fly / Investiture of Wind: +60 ft (flying speed equal to walking)
    ///   - Spider Climb: +30 ft (climbing speed; we don't model 3D terrain
    ///     so the bonus surfaces as a flat repositioning boost)
    ///   - Longstrider: +10 ft (1-hour transmutation buff)
    ///   - Expeditious Retreat: +30 ft (Dash-as-bonus collapsed to a flat
    ///     speed bump, concentration-bound)
    ///
    /// Returned in feet so it composes with `base_speed` / item bonuses
    /// before the Haste / Slow multiplicative factor in `speed()`.
    pub fn condition_speed_bonus(&self) -> f32 {
        let mut bonus = 0.0_f32;
        // Both the Fly spell and Investiture of Wind grant the holder a
        // 60ft flying speed RAW; the two don't stack — they're separate
        // concentration spells the caster can't both maintain, but the
        // gate honors whichever is up.
        if self.has_condition(Condition::Flying)
            || self.has_condition(Condition::InvestedInWind)
            || self.has_condition(Condition::OtherworldlyGuised)
        {
            bonus += 60.0;
        }
        if self.has_condition(Condition::SpiderClimbing) {
            bonus += 30.0;
        }
        if self.has_condition(Condition::Longstriding) {
            bonus += 10.0;
        }
        if self.has_condition(Condition::ExpeditiouslyRetreating) {
            bonus += 30.0;
        }
        if self.has_condition(Condition::AshardalonStriding) {
            bonus += 20.0;
        }
        // 5e Barbarian Path of the Totem Warrior — Tiger Totem Spirit
        // (2024 PHB Path of the Wild Heart flavor). While raging, the
        // tiger barbarian's speed increases by 10 ft. Lives next to the
        // other condition-keyed speed bonuses so a future RAW-aware
        // refinement (different speeds per movement mode, etc.) lands in
        // one place. The gate combines a condition (Raging) and a passive
        // feature flag (TIGER_TOTEM_TAG) — outside of rage the holder
        // has no extra speed.
        if self.has_condition(Condition::Raging)
            && self.has_passive_feature(crate::actions::class_features::TIGER_TOTEM_TAG)
        {
            bonus += 10.0;
        }
        // 5e Barbarian **Fast Movement** (level 5). Passive +10 ft speed
        // for any barbarian holding the FAST_MOVEMENT_TAG. Distinct from
        // Tiger Totem in that it is *always* on (RAW gates on "not wearing
        // heavy armor" but our engine doesn't model armor tiers so the
        // gate collapses to "always on"). Stacks additively on Tiger for
        // a raging tiger barbarian (+20 total) — RAW allows both to
        // apply since they come from different features.
        if self.has_passive_feature(crate::actions::class_features::FAST_MOVEMENT_TAG) {
            bonus += 10.0;
        }
        bonus
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
    /// Weapon: +CHA, Bardic Inspiration: +3 d6-average).
    pub fn condition_attack_bonus(&self) -> i32 {
        let mut bonus = 0;
        // 5e Channel Divinity: Sacred Weapon — paladin's weapon glows
        // with divine light, adding their CHA modifier to attack rolls.
        // Sourced from the holder's own CHA so monsters who somehow grab
        // the buff still scale off their own stat block (no edge case
        // today, but the symmetry beats hard-coding a +3).
        if self.has_condition(Condition::Sacred) {
            bonus += modifier_from_score(self.charisma);
        }
        // 5e Bardic Inspiration: holder adds a d6 (RAW scales d6→d8→d10→d12
        // by bard level) to the next attack roll. We collapse to the
        // d6-average (+3); the condition is consumed by the next attack
        // via `clear_attack_advantage_riders` so the bonus doesn't
        // double-fire across multiple swings.
        if self.has_condition(Condition::Inspired) {
            bonus += 3;
        }
        // 5e Battle Master Precision Attack maneuver: +1d8 (d8 avg,
        // rounded down to +4) on the primed attack roll. Symmetric with
        // Inspired; consumed by `clear_attack_advantage_riders` so the
        // bonus only fires on the first swing after the prime.
        if self.has_condition(Condition::PrecisionAttacking) {
            bonus += 4;
        }
        bonus
    }

    /// Symmetric save-roll counterpart to `condition_attack_bonus`. Same
    /// rationale for excluding Bless / Bane: their +2 / -2 lives on
    /// `save_bonus_buff` and their d4 die on `bless_bane_attack_die`,
    /// so this lane is condition-only flat bonuses (Bardic
    /// Inspiration: +3 d6-average).
    pub fn condition_save_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Inspired) {
            bonus += 3;
        }
        // 5e Warding Bond: +1 saving throws while bonded.
        if self.has_condition(Condition::WardingBonded) {
            bonus += 1;
        }
        bonus
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
        // 5e RAW: a prone creature crawls at half speed. Every tile of
        // movement costs double while prone, which we approximate by
        // halving the remaining budget so the actor gets half as far.
        if self.has_condition(Condition::Prone) {
            return self.movement * 0.5;
        }
        self.movement
    }

    /// 5e Tasha's Rogue Steady Aim gate. True iff the actor has spent any
    /// movement this turn. Compares the raw movement budget (unfiltered
    /// by Prone / `zeros_movement`) against the actor's current `speed()`
    /// — those filters are aim-irrelevant (a grappled rogue with budget
    /// intact still hasn't moved; a prone rogue still has their full
    /// budget, the halving is a per-step cost). A small float tolerance
    /// absorbs FP drift from the Haste / Slow factor in `speed()`.
    pub fn has_moved_this_turn(&self) -> bool {
        self.movement + 0.01 < self.speed()
    }

    /// Drain the actor's remaining movement budget to zero. Used by
    /// Steady Aim (RAW: "after you use the bonus action, your speed is 0
    /// until the end of the current turn"). Direct setter rather than a
    /// `consume_resource(Resource::Movement(remaining))` chain so the
    /// "zero everything regardless of conditions" semantics is explicit.
    pub fn zero_movement(&mut self) {
        self.movement = 0.0;
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn creature_type(&self) -> CreatureType {
        self.creature_type
    }

    pub fn set_location(&mut self, target: Coordinate) {
        self.location = target;
    }

    pub fn location(&self) -> Coordinate {
        self.location
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

    pub fn roll_initiative(&mut self, roller: &mut impl Roller) {
        let rolled = roller.roll(&Dice::new(1, 20)) as i32;
        self.initiative = Some(rolled + self.initiative_mod());
    }

    /// Top-of-turn refresh: movement and action-economy slots regenerate,
    /// and any condition with `UntilStartOfNextTurn` (e.g. Dodge) expires.
    /// Returns the conditions that were cleared so the engine can log them.
    pub fn reset_for_new_round(&mut self) -> Vec<Condition> {
        self.movement = self.speed();
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
        // Once-per-turn flags reset at start of turn. Sneak Attack:
        // available again. Colossus Slayer: same once-per-turn cadence —
        // the Hunter ranger gets a fresh +1d8 rider window each turn.
        // Help grants from this actor live with the helped actor, so we
        // don't clear them here.
        self.sneak_attack_used = false;
        self.colossus_slayer_used = false;
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
            && self
                .features_max
                .contains(crate::actions::class_features::SURVIVOR_TAG)
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
    pub fn heal(&mut self, amount: u32) -> HealOutcome {
        if amount == 0 {
            return HealOutcome::AlreadyFull;
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
    /// / vulnerability and absorbing through any temp HP first. Returns
    /// `(outcome, final_amount)` where `final_amount` is the actual HP
    /// delta that landed (after all reductions and temp-HP absorption).
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
        // Burn temp HP first; only the leftover hits real HP. Temp HP
        // is only relevant for Active actors — Dying / Stable creatures
        // route damage straight into death-save failures.
        if matches!(self.hp_state, HpState::Active) {
            let absorbed = scaled.min(self.temp_hp);
            self.temp_hp -= absorbed;
            let to_hp = scaled - absorbed;
            if to_hp == 0 {
                return (DamageOutcome::Reduced, 0);
            }
            (self.take_damage(to_hp), to_hp)
        } else {
            (self.take_damage(scaled), scaled)
        }
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
                let after_temp = if self.temp_hp >= amount {
                    self.temp_hp -= amount;
                    return DamageOutcome::Reduced;
                } else {
                    let r = amount - self.temp_hp;
                    self.temp_hp = 0;
                    r
                };
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
                    // 5e Half-Orc Relentless Endurance: when the holder
                    // would drop to 0 HP, they instead drop to 1 HP and
                    // the once-per-rest feature is spent. Identical
                    // mechanical hook to Death Ward but gated on a
                    // feature flag (long-rest refresh) instead of a
                    // condition timer. Death Ward takes priority — it's
                    // an active spell the caster chose to maintain, so
                    // burning the racial first would waste the slot.
                    if self
                        .features_remaining
                        .contains(RELENTLESS_ENDURANCE_TAG)
                    {
                        self.hitpoints = 1;
                        self.features_remaining.remove(RELENTLESS_ENDURANCE_TAG);
                        return DamageOutcome::Reduced;
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


    /// Class-feature gates (Second Wind, Action Surge, etc.).
    pub fn feature_available(&self, tag: &'static str) -> bool {
        self.features_remaining.contains(tag)
    }

    pub fn spend_feature(&mut self, tag: &'static str) -> bool {
        self.features_remaining.remove(tag)
    }

    /// True if this actor was instantiated with `tag` in their template's
    /// feature set. Distinct from `feature_available` — `has_passive_feature`
    /// returns true even after the feature's per-rest charge has been spent.
    /// Used by always-on passives (Wild Magic Surge, Sorcerous Restoration)
    /// whose trigger fires every encounter regardless of any charge pool.
    pub fn has_passive_feature(&self, tag: &'static str) -> bool {
        self.features_max.contains(tag)
    }

    /// Has the rogue used their once-per-turn Sneak Attack already?
    pub fn sneak_attack_used(&self) -> bool {
        self.sneak_attack_used
    }

    pub fn mark_sneak_attack_used(&mut self) {
        self.sneak_attack_used = true;
    }

    /// Has the Hunter ranger spent their once-per-turn Colossus Slayer
    /// rider already this turn? Symmetric with `sneak_attack_used` — set
    /// at the swing site when the rider fires, cleared at the holder's
    /// turn-start by `reset_for_new_round`.
    pub fn colossus_slayer_used(&self) -> bool {
        self.colossus_slayer_used
    }

    pub fn mark_colossus_slayer_used(&mut self) {
        self.colossus_slayer_used = true;
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
    use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
    use crate::actors::creatures::slimes::SLIME_TEMPLATE;
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

    #[test]
    fn non_magical_physical_resistances_seeds_bps_triplet() {
        // Empty overlay → the three physical types resist, nothing else.
        let m = non_magical_physical_resistances([]);
        assert_eq!(m.len(), 3);
        for dt in [
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ] {
            assert_eq!(m.get(&dt).copied(), Some(DamageModifier::Resistance));
        }
    }

    #[test]
    fn non_magical_physical_resistances_overlays_can_promote_bps() {
        // Overlay collides with the base BPS Resistance — overlay wins,
        // matching the `elemental_damage_modifiers` semantics: a future
        // creature built on the chassis can promote one of the three to
        // Immunity without touching the helper.
        let m = non_magical_physical_resistances([
            (DamageType::Bludgeoning, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Resistance),
        ]);
        assert_eq!(
            m.get(&DamageType::Bludgeoning).copied(),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            m.get(&DamageType::Piercing).copied(),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            m.get(&DamageType::Fire).copied(),
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
        // Slime resists piercing / slashing (physical weapons gum up).
        let s = make(&SLIME_TEMPLATE);
        assert_eq!(s.effective_damage(7, DamageType::Piercing), 3);
        assert_eq!(s.effective_damage(0, DamageType::Piercing), 0);
        // Acid is immune (zeroed).
        assert_eq!(s.effective_damage(7, DamageType::Acid), 0);
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
}
