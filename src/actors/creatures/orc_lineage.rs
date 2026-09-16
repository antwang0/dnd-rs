use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, RELENTLESS_ENDURANCE_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actions::species::{ADRENALINE_RUSH, ADRENALINE_RUSH_TAG};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// **Orc Raider** — the ninth and last of SRD 5.2's *species*, and the
/// one the engine was carrying an older edition of.
///
/// In its own module rather than in `creatures::orcs`, which is the
/// **monster**: a CR ½ greataxe with Aggressive, built from the
/// bestiary's stat block. This is the player-facing species built from
/// the Character Species chapter, and the two share a name in the book
/// and nothing else — different traits, different challenge, different
/// list. "Raider" is the engine's disambiguation and not RAW's word.
///
/// > *Creature Type: Humanoid. Size: Medium. Speed: 30 feet.*
///
/// Three traits, and the book changed all three since the printing
/// `creatures::half_orcs` was written from:
///
///   - **Adrenaline Rush.** *"You can take the Dash action as a Bonus
///     Action. When you do so, you gain a number of Temporary Hit Points
///     equal to your Proficiency Bonus."* New, and the species' whole
///     character — see `species::ADRENALINE_RUSH_TAG`.
///   - **Darkvision 120 feet.** Twice the Half-Orc Marauder's sixty, and
///     the longest dark-sight on the lineage bench by a factor of two —
///     only the drow matches it.
///   - **Relentless Endurance.** *"When you are reduced to 0 Hit Points
///     but not killed outright, you can drop to 1 Hit Point instead."*
///     Unchanged, and shared with the half-orc through
///     `RELENTLESS_ENDURANCE_TAG`.
///
/// **Beside the Half-Orc Marauder rather than instead of it.** The two
/// are the same greataxe on the same fighter chassis and they are not
/// the same species: RAW 5.2's Orc trades Savage Attacks — the extra
/// critical die the marauder still carries — for a bonus-action Dash
/// that pays temporary hit points, and swaps sixty feet of darkvision
/// for a hundred and twenty. Replacing one with the other would delete a
/// working build to add a different one; shipping both is the same
/// choice the bench already makes between the dwarf and the goliath.
///
/// What the trade means at the table: the marauder is the better *swing*
/// and this is the better *body*. An orc closes two rounds' worth of
/// ground in one, arrives with temporary hit points on top, and gets up
/// again the first time it goes down.
pub static ORC_LINEAGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&*ADRENALINE_RUSH);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    CreatureTemplate {
        name: "Orc Raider",
        // 'O' — 'H' and 'h' are the half-orc's and the halfling's, and
        // the capital this species actually starts with was free on the
        // lineage bench. The monster orc keeps its lowercase 'o'.
        glyph: 'O',
        ac: 16,
        // 3d10+9 — the half-orc's pool and the human's, which is the
        // point: three plain fighters on one bench, differing in what
        // their species hands them and in nothing else.
        hitpoints: "3d10+9".parse().unwrap(),
        strength: 17,
        dexterity: 12,
        constitution: 16,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        // RAW: "Darkvision. You have Darkvision with a range of 120
        // feet." The species' quietest advantage and the one a dark
        // board makes loudest.
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::Orc]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        features: HashSet::from([
            ADRENALINE_RUSH_TAG,
            RELENTLESS_ENDURANCE_TAG,
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});
