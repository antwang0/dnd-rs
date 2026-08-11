//! The **summoned spirits** — the bodies the Tasha's `summon <type>`
//! spell family puts on the board.
//!
//! Eight stat blocks in one file rather than eight files, because they
//! are one family in a way that the Wolf and the Ogre are not. None of
//! them is a creature the world contains: nothing generates a Bestial
//! Spirit, nothing rolls one into an encounter, and no template carries
//! one in its action list. Each exists because exactly one spell names
//! it, and the pair — spell and spirit — is what a reader has to hold in
//! their head. Splitting them across eight files would scatter one
//! design decision (how strong is a summon, per slot level) into eight
//! places where nothing could compare them.
//!
//! **The rung they sit on.** Read down the file and the spirits get
//! monotonically better, because the spell levels do:
//!
//! | spell | lvl | spirit | AC | HP | attack |
//! |-------|-----|--------|----|----|--------|
//! | Summon Beast | 2 | Bestial | 13 | ~30 | 2× 1d8 piercing |
//! | Summon Fey | 3 | Fey | 14 | ~33 | 2d6 force + charm |
//! | Summon Undead | 3 | Undead | 14 | ~33 | 2× 2d4 necrotic, ranged |
//! | Summon Aberration | 4 | Aberrant | 15 | ~52 | 2× 1d8 psychic, ranged |
//! | Summon Elemental | 4 | Elemental | 15 | ~52 | 2× 1d10 thunder |
//! | Summon Celestial | 5 | Celestial | 16 | ~75 | 2× 2d6 radiant, ranged |
//! | Summon Draconic Spirit | 5 | Draconic | 14 | ~75 | 2× 1d6 + breath |
//! | Summon Fiend | 6 | Fiendish | 17 | ~90 | 2× 2d6 slashing |
//!
//! That ladder is the feature. A caster choosing between Summon Fey and
//! Summon Undead at level 3 is choosing between a melee controller and a
//! ranged sniper, not between a good spell and a bad one; a caster
//! choosing between level 3 and level 6 is buying roughly twice the body.
//!
//! **What each spirit is not.** RAW's summons each ship an option table
//! — the Bestial Spirit is Land, Sky or Water; the Fiendish Spirit is
//! Demon, Devil or Yugoloth — and RAW scales every line of the block off
//! the slot the spell was cast with. The engine collapses both, for the
//! same reason `SummonSpell::template` documents: `instantiate_creature`
//! takes a template and nothing else, and there is no channel for "this
//! template, but with 20 more hit points". So each spirit here is one
//! branch of its option table, fixed at the spell's base level, chosen
//! for what it adds to the roster rather than for fidelity to a
//! particular column. The picks are called out per block.

use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BreathWeapon, Multiattack, SimpleWeapon, WeaponWithSaveCondition,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::actors::creatures::fire_elementals::{
    elemental_defaults,
};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Every spirit's save DC and to-hit come off its own stat block rather
/// than off the summoner's, which is the one place this whole family
/// departs from RAW — a `SimpleWeapon` has no channel to reach back to
/// whoever called it. The same substitution the Wildfire Spirit makes,
/// and for the same reason.
///
/// The ability scores below are picked so the substitution costs the
/// spells nothing in practice: a spirit summoned by a caster with the
/// spellcasting stat its slot level implies lands within a point of the
/// RAW to-hit either way. This constant is the save-rider half of that
/// bargain — one DC for the whole family, sized to a mid-tier caster,
/// so a Fey Spirit's charm and a future spirit's save rider can't drift
/// apart by accident.
const SPIRIT_SAVE_DC: i32 = 14;

// ---------------------------------------------------------------------
// Bestial Spirit — Summon Beast (level 2 conjuration)
// ---------------------------------------------------------------------

/// **Maul** — the Bestial Spirit's swing. RAW's Land option lists
/// `1d8 + 4` piercing, twice per turn; STR 18 (+4) reproduces the flat
/// term exactly rather than hard-coding it.
pub static SPIRIT_MAUL: SimpleWeapon = SimpleWeapon::melee(
    "maul",
    &["bite", "sm"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Piercing,
);

/// The Bestial Spirit's two swings for one Action. RAW: "The beast makes
/// a number of Maul attacks equal to half this spell's level (rounded
/// down)" — two at the spell's base level of 2… and also at 3, 4 and 5.
/// The fixed pair is the honest reading of the base cast.
pub static SPIRIT_MAUL_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "maul flurry",
    sub_attack: &SPIRIT_MAUL,
    count: 2,
});

/// Bestial Spirit — the Medium beast **Summon Beast** puts beside its
/// caster for a level-2 slot.
///
/// The **Land** branch of RAW's Land/Sky/Water table, picked because it
/// has the highest hit points of the three, which is what a level-2 slot
/// is actually being spent on.
///
/// The other two branches are movement modes, and the grid distinguishes
/// them less than RAW does: a Sky spirit's flight and a Water spirit's
/// swim speed both come out as "moves 30 ft", since the engine models
/// one speed magnitude. They are no longer *identical* to Land, though —
/// flight lifts a creature over difficult terrain and out of the water,
/// and a swimming speed makes a pool free to cross — so a future branch
/// picker would have something to pick between. What it would still not
/// have is a reason to give up the hit points.
///
/// It is the entry rung of the whole family and reads like one: two
/// small bites, thirty hit points, no resistances at all. What a druid
/// buys with it at level 2 is a second body that a goblin has to deal
/// with — and, because Summon Beast holds concentration, a body that
/// costs them Entangle for as long as it stands.
///
/// Glyph 'b' — for **b**east.
pub static BESTIAL_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPIRIT_MAUL);
    actions.push(&*SPIRIT_MAUL_MULTI);
    CreatureTemplate {
        name: "Bestial Spirit",
        glyph: 'b',
        ac: 13,
        // RAW 30 HP flat at the base level.
        hitpoints: "5d8+8".parse().unwrap(),
        speed: 30.,
        strength: 18,
        dexterity: 11,
        constitution: 16,
        intelligence: 4,
        wisdom: 14,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Fey Spirit — Summon Fey (level 3 conjuration)
// ---------------------------------------------------------------------

/// **Fey Blade** — 2d6 force, and on a hit the target rolls a WIS save
/// or is Charmed until the start of its next turn.
///
/// RAW's Mirthful branch charms "until the end of the fey spirit's next
/// turn"; the engine's nearest timer is `UntilStartOfNextTurn`, which
/// clears on the *victim's* turn rather than the spirit's. That is a
/// half-round short of RAW and deliberately so: the alternative,
/// `Rounds(1)`, runs long often enough that a Fey Spirit would routinely
/// lock a target out of two turns for one hit, which is not what a
/// level-3 slot buys.
///
/// The charm is the reason to pick Mirthful over RAW's Fuming (extra
/// damage) and Tricksy (an obscuring cloud): it is the only branch that
/// puts a *condition* on the board, and `Condition::Charmed` carries the
/// engine's enforced can't-attack-the-charmer restriction, so a charmed
/// enemy genuinely stops swinging at the spirit for a beat.
pub static FEY_BLADE: WeaponWithSaveCondition = WeaponWithSaveCondition::melee(
    "fey blade",
    &["blade", "fb"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Force,
    AbilityScoreType::Wisdom,
    SPIRIT_SAVE_DC,
    Condition::Charmed,
    ConditionTimer::UntilStartOfNextTurn,
    "mirthful presence",
);

/// Fey Spirit — the Medium fey **Summon Fey** puts beside its caster for
/// a level-3 slot.
///
/// The **Mirthful** branch of RAW's Fuming/Mirthful/Tricksy table — see
/// `FEY_BLADE` for why the charm rider is the branch worth having.
///
/// Speed 40 rather than 30 is RAW and matters more here than the number
/// suggests: the Fey Spirit is the only summon in the family that can
/// reliably close on a caster's chosen target in the round it arrives,
/// which is what makes a melee spirit worth a slot at all next to the
/// Undead Spirit's ranged bolts on the same rung.
///
/// Glyph 'y' — 'f' is crowded and the fe**y** spirit can have the tail
/// of its own name.
pub static FEY_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FEY_BLADE);
    CreatureTemplate {
        name: "Fey Spirit",
        glyph: 'y',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 40.,
        strength: 13,
        dexterity: 16,
        constitution: 13,
        intelligence: 12,
        wisdom: 12,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Sylvan]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        // RAW's fey are charm-proof; the engine's `has_fey_ancestry`
        // gives the advantage-on-charm-saves half, and the spirit being
        // made of the stuff gives the rest.
        condition_immunities: HashSet::from([Condition::Charmed]),
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Undead Spirit — Summon Undead (level 3 necromancy)
// ---------------------------------------------------------------------

/// **Grave Bolt** — the Undead Spirit's ranged attack, 2d4 necrotic at
/// 60 ft. RAW's range is 150 ft, which on this grid is 60 tiles: further
/// than any arena the generator builds, so it would read as "unlimited"
/// and hide the one interesting thing about a ranged summon, which is
/// where you have to stand it. 24 tiles is the same 60 ft envelope the
/// Wildfire Spirit's Flame Seed uses.
pub static GRAVE_BOLT: SimpleWeapon = SimpleWeapon::ranged(
    "grave bolt",
    &["bolt", "gb"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 4),
    DamageType::Necrotic,
    24,
    24,
);

/// Two Grave Bolts for one Action — RAW's Multiattack on the Skeletal
/// branch.
pub static GRAVE_BOLT_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "grave volley",
    sub_attack: &GRAVE_BOLT,
    count: 2,
});

/// Undead Spirit — the Medium undead **Summon Undead** puts beside its
/// caster for a level-3 slot.
///
/// The **Skeletal** branch of RAW's Ghostly/Putrid/Skeletal table, and
/// the choice is what makes this spell worth casting next to Summon Fey
/// on the same rung: Skeletal is the only branch that shoots. A warlock
/// with a Fey Spirit has bought a bodyguard that has to walk somewhere;
/// a warlock with an Undead Spirit has bought a second archer that can
/// stand behind them and never move. Same slot, opposite shape.
///
/// The undead envelope is the real second half of the purchase — poison
/// immunity, necrotic resistance, and immunity to the three conditions
/// that stop a body from acting at all. A summon that shrugs off the
/// Stinking Cloud its own caster dropped is a summon that can hold a
/// doorway.
///
/// Glyph 'u' — for **u**ndead, and free.
pub static UNDEAD_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GRAVE_BOLT);
    actions.push(&*GRAVE_BOLT_MULTI);
    CreatureTemplate {
        name: "Undead Spirit",
        glyph: 'u',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 30.,
        strength: 12,
        dexterity: 16,
        constitution: 13,
        intelligence: 4,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Necrotic, DamageModifier::Resistance),
        ]),
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Aberrant Spirit — Summon Aberration (level 4 conjuration)
// ---------------------------------------------------------------------

/// **Eye Ray** — 1d8 psychic at range, twice a turn. RAW's Beholderkin
/// branch. Psychic is the point: it is the damage type the fewest
/// creatures in the bestiary resist, which is what a level-4 slot is
/// buying over the level-3 Undead Spirit's necrotic bolts.
pub static EYE_RAY: SimpleWeapon = SimpleWeapon::ranged(
    "eye ray",
    &["ray", "er"],
    AbilityScoreType::Intelligence,
    Dice::new(1, 8),
    DamageType::Psychic,
    24,
    24,
);

/// Two Eye Rays for one Action.
pub static EYE_RAY_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "ray barrage",
    sub_attack: &EYE_RAY,
    count: 2,
});

/// Aberrant Spirit — the Large aberration **Summon Aberration** puts
/// beside its caster for a level-4 slot.
///
/// The **Beholderkin** branch of RAW's Beholderkin/Slaad/Star Spawn
/// table. Slaad's regeneration and Star Spawn's psychic aura both want
/// engine hooks the other two branches don't need, and Beholderkin is
/// the one whose signature — a ranged psychic ray — lands entirely
/// inside `SimpleWeapon`. It is also the branch that makes the level-4
/// slot legible next to level 3: same ranged shape as the Undead Spirit,
/// a much harder damage type to resist, and forty per cent more body.
///
/// Large rather than Medium, which is a real cost and not just flavour:
/// `find_adjacent_spawn` has to place a 2×2 footprint, so an Aberrant
/// Spirit summoned in a corridor may simply not fit where a Fey Spirit
/// would. The `search_radius: 4` on the spell is what pays for that.
///
/// Glyph 'a' — for **a**berration.
pub static ABERRANT_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&EYE_RAY);
    actions.push(&*EYE_RAY_MULTI);
    CreatureTemplate {
        name: "Aberrant Spirit",
        glyph: 'a',
        ac: 15,
        hitpoints: "8d10+8".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 10,
        constitution: 15,
        intelligence: 16,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::DeepSpeech]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Elemental Spirit — Summon Elemental (level 4 conjuration)
// ---------------------------------------------------------------------

/// **Slam** — 1d10 thunder, twice a turn. RAW's Air branch, and the
/// damage type is the whole reason to take Air over Earth's bludgeoning:
/// the engine's bestiary is full of things that resist non-magical
/// physical damage and almost nothing resists thunder.
pub static SPIRIT_SLAM: SimpleWeapon = SimpleWeapon::melee(
    "elemental slam",
    &["slam", "es"],
    AbilityScoreType::Strength,
    Dice::new(1, 10),
    DamageType::Thunder,
);

/// Two Slams for one Action.
pub static SPIRIT_SLAM_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "slam flurry",
    sub_attack: &SPIRIT_SLAM,
    count: 2,
});

/// Elemental Spirit — the Large elemental **Summon Elemental** puts
/// beside its caster for a level-4 slot.
///
/// The **Air** branch of RAW's Air/Earth/Fire/Water table, and the
/// choice is made by what is already on the board: Conjure Elemental
/// (level 5) summons a Fire Elemental, so a Fire branch here would be a
/// worse version of a spell the same casters already have. Air's thunder
/// damage is the one elemental note nothing else in the summon family
/// plays.
///
/// Carries the shared elemental defensive envelope — poison immunity,
/// non-magical physical resistance, and the nine-condition
/// `ELEMENTAL_CONDITION_IMMUNITIES` set — plus lightning immunity on
/// top, which is the Air branch's own line. That envelope is worth more
/// than the stat block suggests: an Elemental Spirit is the summon you
/// send *into* your own Sleet Storm.
///
/// Glyph 'e' — for **e**lemental, lowercase against the Fire
/// Elemental's 'E' so the conjured and the real read as a pair without
/// reading as the same thing.
pub static ELEMENTAL_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPIRIT_SLAM);
    actions.push(&*SPIRIT_SLAM_MULTI);
    CreatureTemplate {
        name: "Elemental Spirit",
        glyph: 'e',
        ac: 15,
        hitpoints: "8d10+8".parse().unwrap(),
        speed: 40.,
        strength: 18,
        dexterity: 15,
        constitution: 15,
        intelligence: 4,
        wisdom: 10,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        ..elemental_defaults([(
            DamageType::Lightning,
            DamageModifier::Immunity,
        )])
    }
});

// ---------------------------------------------------------------------
// Celestial Spirit — Summon Celestial (level 5 conjuration)
// ---------------------------------------------------------------------

/// **Radiant Bow** — 2d6 radiant at range, twice a turn. RAW's Avenger
/// branch.
pub static RADIANT_BOW: SimpleWeapon = SimpleWeapon::ranged(
    "radiant bow",
    &["bow", "rb"],
    AbilityScoreType::Dexterity,
    Dice::new(2, 6),
    DamageType::Radiant,
    30,
    30,
);

/// Two Radiant Bow shots for one Action.
pub static RADIANT_BOW_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "radiant volley",
    sub_attack: &RADIANT_BOW,
    count: 2,
});

/// Celestial Spirit — the Large celestial **Summon Celestial** puts
/// beside its caster for a level-5 slot.
///
/// The **Avenger** branch of RAW's Avenger/Defender pair. Defender is
/// the more distinctive block — it wants a healing touch and a
/// damage-redirect reaction — and both halves need channels the summon
/// chassis deliberately doesn't have: a heal needs an ally picker the
/// summon's own AI turn has no way to answer well, and the redirect
/// needs a reaction hook keyed to a creature that may not exist next
/// round. Avenger is the branch that is entirely a stat block.
///
/// It is also the branch that fills a hole. Summon Celestial is the
/// cleric's and the paladin's only summon, and both are melee-shaped
/// classes standing in the front rank; what they lack is a way to
/// threaten something thirty feet away that is busy killing the wizard.
/// A Celestial Spirit with a bow is exactly that, and its radiant
/// damage is the one type the undead the cleric fights most are worst
/// against.
///
/// Glyph 'c' — for **c**elestial.
pub static CELESTIAL_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RADIANT_BOW);
    actions.push(&*RADIANT_BOW_MULTI);
    CreatureTemplate {
        name: "Celestial Spirit",
        glyph: 'c',
        ac: 16,
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 40.,
        strength: 16,
        dexterity: 16,
        constitution: 16,
        intelligence: 10,
        wisdom: 14,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Celestial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Radiant, DamageModifier::Resistance)]),
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Draconic Spirit — Summon Draconic Spirit (level 5 conjuration)
// ---------------------------------------------------------------------

/// **Rend** — 1d6 piercing, twice a turn. Small dice on purpose: the
/// Draconic Spirit's damage lives in its breath, and a spirit whose
/// swings were also good would make the recharge irrelevant.
pub static DRACONIC_REND: SimpleWeapon = SimpleWeapon::melee(
    "rend",
    &["claw", "dr"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// Two Rends for one Action. RAW ties the count to half the slot level;
/// two is the base-level reading, matching the Bestial Spirit's flurry.
pub static DRACONIC_REND_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "rend flurry",
    sub_attack: &DRACONIC_REND,
    count: 2,
});

/// **Draconic Breath** — the reason to cast this spell instead of Summon
/// Celestial on the same rung. 4d6 fire in a burst, DEX save for half,
/// recharge 5–6 off the shared `"breath_weapon"` pool.
///
/// RAW picks the damage type from the caster's chosen ancestry at cast
/// time; the engine has no channel for a cast-time choice (the same
/// reason the three Storm Herald Barbarians are three templates), so
/// fire is fixed here and the ancestry is flavour. Burst 2 / range 4 is
/// the RAW 30 ft cone rendered as the engine's burst envelope, a rung
/// below the real dragon's burst-4 / range-6.
pub static DRACONIC_BREATH: BreathWeapon = BreathWeapon {
    display_name: "draconic breath",
    aliases: &["breath", "db"],
    damage_dice: Dice::new(4, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: SPIRIT_SAVE_DC,
    radius: 2,
    range: 4,
    recharge_key: "breath_weapon",
};

/// Draconic Spirit — the Large dragon **Summon Draconic Spirit** puts
/// beside its caster for a level-5 slot.
///
/// The only summon in the family that brings an *area* attack, and that
/// is what it is for. Every other spirit is a second body doing
/// single-target damage; the Draconic Spirit is a second body that
/// occasionally does what a Fireball does, on a recharge its owner
/// doesn't control. A caster who summons one has bought a die roll at
/// the top of each of its turns.
///
/// The trade is that its swings are deliberately feeble — 1d6 twice, the
/// weakest attack routine in the family and on a level-5 spell. Rounds
/// where the breath doesn't come back are rounds where the spirit is
/// worse than a Bestial Spirit costing three slot levels less. That
/// variance is the spell.
///
/// Glyph 'd' — for **d**raconic, lowercase against the real dragons' 'D'.
pub static DRACONIC_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DRACONIC_REND);
    actions.push(&*DRACONIC_REND_MULTI);
    actions.push(&DRACONIC_BREATH);
    CreatureTemplate {
        name: "Draconic Spirit",
        glyph: 'd',
        ac: 14,
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 40.,
        strength: 19,
        dexterity: 14,
        constitution: 17,
        intelligence: 10,
        wisdom: 14,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60), SpecialSense::Blindsight(30)]),
        languages: HashSet::from([Language::Draconic]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        // Matches the breath it exhales, and RAW: the spirit is immune
        // to the damage type of its chosen ancestry.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        recharge_abilities: vec![("breath_weapon", 5)],
        ..CreatureTemplate::defaults()
    }
});

// ---------------------------------------------------------------------
// Fiendish Spirit — Summon Fiend (level 6 conjuration)
// ---------------------------------------------------------------------

/// **Claws** — 2d6 slashing, twice a turn. The hardest-hitting routine
/// in the family, on the most expensive spell in it.
pub static FIENDISH_CLAWS: SimpleWeapon = SimpleWeapon::melee(
    "fiendish claws",
    &["claws", "fc"],
    AbilityScoreType::Strength,
    Dice::new(2, 6),
    DamageType::Slashing,
);

/// Two Claws for one Action.
pub static FIENDISH_CLAWS_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "claw flurry",
    sub_attack: &FIENDISH_CLAWS,
    count: 2,
});

/// Fiendish Spirit — the Large fiend **Summon Fiend** puts beside its
/// caster for a level-6 slot, and the top of the family's ladder.
///
/// The **Demon** branch of RAW's Demon/Devil/Yugoloth table: the one
/// with the highest hit points and the fewest keywords. Devil's flight
/// and magic resistance and Yugoloth's regeneration are both real
/// features the engine could carry, and both would make the spell's
/// pitch murkier — at level 6 the pitch should be "this is simply a
/// large amount of monster", and it is.
///
/// AC 17 and ninety hit points make it the only summon that survives
/// being focused, which is the actual purchase: a caster who lands one
/// in the enemy's back line has bought several rounds of that back line
/// not shooting at anyone else. Fire, cold and lightning resistance plus
/// poison immunity is the fiendish envelope, and it means the usual
/// answers to a summon — drop a Fireball on it — mostly don't work.
///
/// Glyph 'i' — for f**i**end; 'f' is crowded and 'F' is the fighter.
pub static FIENDISH_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FIENDISH_CLAWS);
    actions.push(&*FIENDISH_CLAWS_MULTI);
    CreatureTemplate {
        name: "Fiendish Spirit",
        glyph: 'i',
        ac: 17,
        hitpoints: "12d10+24".parse().unwrap(),
        speed: 40.,
        strength: 18,
        dexterity: 14,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

/// Every spirit in the family, in ascending spell-level order. The list
/// the sweeps below walk, and the one place a ninth spirit has to be
/// added for every invariant here to cover it.
pub fn summoned_spirit_templates() -> Vec<&'static CreatureTemplate> {
    vec![
        &BESTIAL_SPIRIT_TEMPLATE,
        &FEY_SPIRIT_TEMPLATE,
        &UNDEAD_SPIRIT_TEMPLATE,
        &ABERRANT_SPIRIT_TEMPLATE,
        &ELEMENTAL_SPIRIT_TEMPLATE,
        &CELESTIAL_SPIRIT_TEMPLATE,
        &DRACONIC_SPIRIT_TEMPLATE,
        &FIENDISH_SPIRIT_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn instantiate(template: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap_or_else(|e| panic!("{} failed to instantiate: {}", template.name, e))
    }

    /// Every spirit is a body that can actually fight: it instantiates,
    /// it has hit points, and it carries at least one attack of its own
    /// beyond the default action set.
    ///
    /// The sweep exists because the failure it catches is silent. A
    /// spirit whose attack was declared but never pushed onto `actions`
    /// still summons, still occupies a tile, and still soaks a slot —
    /// it just stands there for the whole encounter, and nothing in the
    /// engine complains.
    #[test]
    fn every_spirit_can_fight() {
        let baseline = DEFAULT_ACTIONS.len();
        for template in summoned_spirit_templates() {
            let actor = instantiate(template);
            assert!(
                actor.max_hitpoints() > 0,
                "{} rolled no hit points",
                template.name
            );
            assert!(
                template.actions.len() > baseline,
                "{} carries nothing but the default actions",
                template.name
            );
        }
    }

    /// The ladder in the module docs is a claim about the data, so the
    /// data has to hold it up: read in declaration order, each spirit is
    /// at least as tough as the one before it.
    ///
    /// Hit points are compared at the template's *average* roll rather
    /// than a sampled one — two adjacent rungs are close enough that a
    /// bad roll on the higher one would fail this for no reason.
    #[test]
    fn the_family_is_a_ladder() {
        let mut previous: Option<&CreatureTemplate> = None;
        for template in summoned_spirit_templates() {
            if let Some(prev) = previous {
                assert!(
                    template.hitpoints.average_roll() >= prev.hitpoints.average_roll(),
                    "{} ({}) is frailer than {} ({}) but costs more",
                    template.name,
                    template.hitpoints.average_roll(),
                    prev.name,
                    prev.hitpoints.average_roll(),
                );
                assert!(
                    template.cr >= prev.cr,
                    "{} (CR {}) rates below {} (CR {}) but costs more",
                    template.name,
                    template.cr,
                    prev.name,
                    prev.cr,
                );
            }
            previous = Some(template);
        }
    }

    /// No two spirits share a glyph. They are the one family in the
    /// bestiary that can plausibly appear on the same board as each
    /// other — a druid holding Summon Beast has a free hand for a wand,
    /// and a two-caster team can field four of these at once — so the
    /// map has to keep them apart.
    #[test]
    fn no_two_spirits_render_alike() {
        let mut seen: HashMap<char, &str> = HashMap::new();
        for template in summoned_spirit_templates() {
            if let Some(other) = seen.insert(template.glyph, template.name) {
                panic!(
                    "{} and {} both render as '{}'",
                    template.name, other, template.glyph
                );
            }
        }
    }

    /// The Draconic Spirit's breath is gated on a recharge, and the gate
    /// only works if the template declares the pool the action reads.
    ///
    /// This is the one cross-field coupling in the file: `DRACONIC_BREATH`
    /// names `"breath_weapon"` and the template has to list the same
    /// string, or the breath is simply never available and the spell's
    /// entire pitch quietly evaporates.
    #[test]
    fn the_draconic_breath_is_wired_to_a_pool_the_template_declares() {
        assert!(
            DRACONIC_SPIRIT_TEMPLATE
                .recharge_abilities
                .iter()
                .any(|(key, _)| *key == DRACONIC_BREATH.recharge_key),
            "the breath reads a pool the dragon doesn't have"
        );
        let actor = instantiate(&DRACONIC_SPIRIT_TEMPLATE);
        assert!(actor.find_action("draconic breath").is_some());
    }

    /// The Fey Spirit's charm rider is the branch we picked it for, so
    /// pin that it is actually on the blade — and that the spirit is
    /// itself immune to what it hands out, which is the fey-ancestry
    /// half RAW gives it.
    #[test]
    fn the_fey_spirit_charms_but_cannot_be_charmed() {
        assert_eq!(FEY_BLADE.condition, Condition::Charmed);
        assert!(instantiate(&FEY_SPIRIT_TEMPLATE).effectively_immune_to_condition(Condition::Charmed));
    }

    /// The Elemental Spirit inherits the shared elemental envelope
    /// rather than a hand-copied one, which is what keeps it in step
    /// with the Fire Elemental and the Wildfire Spirit when that
    /// envelope changes.
    #[test]
    fn the_elemental_spirit_is_made_of_the_element_it_throws() {
        let actor = instantiate(&ELEMENTAL_SPIRIT_TEMPLATE);
        assert_eq!(
            actor.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            actor.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(actor.effectively_immune_to_condition(Condition::Poisoned));
    }
}
