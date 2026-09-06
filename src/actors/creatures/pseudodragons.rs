use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{PSEUDODRAGON_BITE, PSEUDODRAGON_STING};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Pseudodragon — CR 1/4 tiny dragon. Iconic "friendly pocket dragon"
/// often paired with arcane casters via Find Familiar / Awakened
/// pet — modeled here as a Tiny dragon with a 1d4 bite + a sleep-
/// poison sting. The sting carries the DC-11 CON save vs the Sleep
/// condition (10-round timer, approximation of RAW's "1 hour or
/// until damage" clause) so a successful sting paralyzes the target
/// for a beat — a frequent low-CR control vector. Magic Resistance
/// on the template: advantage on saves vs spells, matching every
/// other true-dragon family.
pub static PSEUDODRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PSEUDODRAGON_BITE);
    actions.push(&*PSEUDODRAGON_STING);
    CreatureTemplate {
        name: "Pseudodragon",
        // 'p' lowercase for the Tiny dragon — keeps it visually
        // distinct from the bigger 'D' / 'd' dragon family.
        glyph: 'p',
        ac: 14,
        hitpoints: "3d4+3".parse().unwrap(),
        // RAW: 15 ft walk, 60 ft fly. The engine collapses to one
        // ground speed so we land at 30 to reflect the airborne tier
        // without overstating the walk.
        // RAW speed line: Speed 15 ft., fly 60 ft.
        speed: 15.0,
        fly_speed: 60.0,
        strength: 6,
        intelligence: 10,
        dexterity: 15,
        wisdom: 12,
        constitution: 13,
        charisma: 10,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        // RAW: "understands Common and Draconic but can't speak."
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.25,
        size: Size::Tiny,
        creature_type: CreatureType::Dragon,
        actions,
        // Magic Resistance: advantage on every spell save. Standard
        // dragon family trait — slots cleanly into compute_save_mode.
        has_magic_resistance: true,
        ..CreatureTemplate::defaults()
    }
});
