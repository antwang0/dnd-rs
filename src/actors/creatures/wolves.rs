use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FRIGHTFUL_HOWL, WOLF_BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Fast melee with a built-in trip rider. Bite always rolls the STR save
/// on hit, so a Wolf naturally knocks targets prone — making subsequent
/// melee attacks (its own next-turn bite, or an ally's swing) hit at
/// advantage. Demonstrates rider-on-hit baked into a creature's
/// canonical action.
pub static WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WOLF_BITE);
    actions.push(&*FRIGHTFUL_HOWL);
    CreatureTemplate {
        name: "Wolf",
        glyph: 'W',
        ac: 12,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 14,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 12,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});

/// Ranger's Companion — the beast a **Beast Master Ranger** binds at
/// level 3, expressed as a template.
///
/// RAW doesn't give the companion its own stat block: it takes a beast
/// of CR 1/4 or lower and *upgrades* it — the ranger's proficiency
/// bonus is added to its AC, attack rolls, damage rolls and saves, and
/// its hit point maximum becomes four times the ranger's level. At
/// level 11 (Bestial Fury) it makes two attacks when the ranger
/// commands it to attack.
///
/// The engine has no "apply a bonus to another creature's stat block"
/// lane, and inventing one for a single feature would be a large
/// mechanism for one caller. Baking the upgrade into a template is the
/// same collapse every other subclass makes, and it keeps the
/// companion's numbers legible in one place:
///
///   - **AC 15** (13 + a level-7 ranger's +3 proficiency).
///   - **`4d8+16`** hit points, averaging 34 — four times a level-8
///     ranger, matching the CR-1.5 chassis the Beast Master ships on.
///   - **Extra Attack**, standing in for Bestial Fury.
///
/// Everything else — the bite, its prone rider, Frightful Howl, Pack
/// Tactics, the 40 ft speed and the darkvision — inherits from
/// `WOLF_TEMPLATE` through the clone tail, which is the point: this is
/// a wolf that has been trained, not a different animal.
///
/// A wolf specifically because the bite's built-in STR-save trip is the
/// rider that makes a companion worth an Action to summon. The
/// companion knocks a target prone, and the ranger's own longbow and
/// every melee ally then swing at advantage — the beast's real
/// contribution is not its damage.
///
/// Glyph 'B' — for the **B**east Master's companion — so it reads
/// distinctly from a wild wolf 'W' on the map.
pub static RANGERS_COMPANION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        name: "Ranger's Companion",
        glyph: 'B',
        // 13 base + the ranger's +3 proficiency bonus (RAW: "add your
        // proficiency bonus to the beast's AC").
        ac: 15,
        // RAW: "its hit point maximum equals four times your ranger
        // level" — 4 × 8 on this chassis, expressed as dice so the
        // companion still rolls a body rather than arriving at a
        // fixed number.
        hitpoints: "4d8+16".parse().unwrap(),
        // Bestial Fury (lv11): the companion makes two attacks when
        // commanded to attack.
        has_extra_attack: true,
        cr: 1.0,
        ..WOLF_TEMPLATE.clone()
    }
});
