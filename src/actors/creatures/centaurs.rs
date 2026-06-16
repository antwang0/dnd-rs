use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CENTAUR_HOOVES, CENTAUR_MULTI, CENTAUR_PIKE, LONGBOW,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Centaur — CR 2 large monstrosity. Sturdy hybrid kit: a reach-2 pike
/// for opening jabs, a 2d6 hoof kick for the second swing, and a longbow
/// for ranged fallback when out of melee. Multi pairs pike + hooves so a
/// committed melee turn brings 1d10 + 2d6 down on a single target.
/// Fast 50ft walking speed reflects the equine half — kites comfortably
/// between bow shots.
pub static CENTAUR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CENTAUR_PIKE);
    actions.push(&CENTAUR_HOOVES);
    actions.push(&LONGBOW);
    actions.push(&*CENTAUR_MULTI);
    CreatureTemplate {
        name: "Centaur",
        // 'C' was free in the medium / large monstrosity slot — capital
        // because Large.
        glyph: 'C',
        ac: 12,
        hitpoints: "5d10+10".parse().unwrap(),
        speed: 50.,
        strength: 18,
        intelligence: 9,
        dexterity: 14,
        wisdom: 13,
        constitution: 14,
        charisma: 11,
        languages: HashSet::from([Language::Elvish, Language::Sylvan]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
