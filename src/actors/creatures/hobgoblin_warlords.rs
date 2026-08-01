use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HOBGOBLIN_WARLORD_MULTI, LONGBOW, LONGSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hobgoblin Warlord -- CR 6 martial leader. Plate + shield (AC 20).
/// Longsword in melee, longbow for ranged. Extra Attack gives two swings
/// per Action. Proficient INT/WIS/CHA saves.
pub static HOBGOBLIN_WARLORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    actions.push(&LONGBOW);
    // MM's Multiattack: three melee attacks per Action. It was written
    // and then never pushed onto the only template that names it, so
    // the warlord fought with a single swing plus Extra Attack and the
    // "warlord" half of the stat block never showed up. Carried
    // alongside `has_extra_attack` exactly as the Knight does — the two
    // coexist on that chassis and the picker chooses between them.
    actions.push(&*HOBGOBLIN_WARLORD_MULTI);
    CreatureTemplate {
        name: "Hobgoblin Warlord",
        glyph: '!',
        ac: 20,
        hitpoints: "10d8+30".parse().unwrap(),
        strength: 16,
        dexterity: 14,
        constitution: 16,
        intelligence: 14,
        wisdom: 11,
        charisma: 15,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
