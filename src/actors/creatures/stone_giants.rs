use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    STONE_GIANT_BOULDER, STONE_GIANT_GREATCLUB, STONE_GIANT_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Stone Giant — CR 7 giant. Huge boulder-tossing mountain dweller.
/// Combat envelope:
/// - **Multiattack**: 2 greatclub swings per Action (reach 3).
/// - **Greatclub** (standalone, reach 3): high-damage melee.
/// - **Boulder** (ranged, 24 tiles): the giant's signature ranged
///   threat, paired with the greatclub for melee.
///
/// Stat shape (MM RAW): AC 17, ~126 HP (11d12+55), STR 23, DEX 15,
/// CON 20. Proficient in DEX/CON/WIS saves. Speaks Giant. No
/// damage / condition immunities — they're tough but mortal.
pub static STONE_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*STONE_GIANT_MULTI);
    actions.push(&STONE_GIANT_GREATCLUB);
    actions.push(&STONE_GIANT_BOULDER);
    CreatureTemplate {
        name: "Stone Giant",
        // 'g' (lowercase) — distinct from 'G' (Gnoll). Reads as a
        // smaller-glyph but-still-large stone-skinned humanoid.
        glyph: 'g',
        ac: 17,
        // 11d12+55 ≈ 126 average per MM (CR 7).
        hitpoints: "11d12+55".parse().unwrap(),
        speed: 40.,
        strength: 23,
        intelligence: 10,
        dexterity: 15,
        wisdom: 12,
        constitution: 20,
        charisma: 9,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant]),
        cr: 7.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
