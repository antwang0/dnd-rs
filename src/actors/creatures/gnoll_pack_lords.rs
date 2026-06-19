use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GNOLL_PACK_LORD_MULTI, LONGBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gnoll Pack Lord — CR 2 gnoll warband leader. Tougher and smarter than
/// a baseline gnoll, the pack lord wields a glaive with reach-2 and
/// swings it twice per Action via multiattack. Falls back to a longbow
/// when enemies stay at range. The higher STR (16) and CON (14) make it
/// a credible frontliner that can anchor a pack of CR 1/2 gnolls.
pub static GNOLL_PACK_LORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GNOLL_PACK_LORD_MULTI);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Gnoll Pack Lord",
        // 'L' for pack Lord — 'N' is taken by the base gnoll.
        glyph: 'L',
        ac: 15,
        hitpoints: "3d10+6".parse().unwrap(),
        strength: 16,
        dexterity: 14,
        constitution: 14,
        intelligence: 12,
        wisdom: 11,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
