use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WORG_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Worg — CR 1/2 goblinoid mount. Stats slot between Wolf (CR 1/4) and
/// Dire Wolf (CR 1): higher STR, more HP, 2d6 bite + Trip (STR save
/// vs prone) without the dire wolf's pack-tactics rider.
///
/// Worgs typically appear ridden by goblins or alongside hobgoblin
/// patrols; we leave the mount mechanic abstract — the worg is its
/// own actor on the map.
pub static WORG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WORG_BITE);
    CreatureTemplate {
        name: "Worg",
        glyph: 'w', // lowercase w to distinguish from W (Wolf).
        ac: 13,
        hitpoints: "4d10+4".parse().unwrap(),
        speed: 50.,
        strength: 16,
        intelligence: 7,
        dexterity: 13,
        wisdom: 11,
        constitution: 13,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Goblin]),
        cr: 0.5,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
