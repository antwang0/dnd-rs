use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CYCLOPS_GREATCLUB, CYCLOPS_MULTI, CYCLOPS_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cyclops — CR 6 huge giant. One-eyed brute that swings a 3d8 greatclub
/// twice per Action or hurls a 4d10 boulder out to 60 ft. Sibling to
/// Stone Giant (CR 7) on the giant ladder: same melee dice (3d8) but
/// fewer raw HP and saves than the Stone Giant, and the cyclops trades
/// Stone Giant's Darkvision-and-DEX-saves for cheaper INT / WIS. The
/// Cyclops's notorious depth-perception penalty (RAW: disadvantage on
/// ranged attacks vs distant targets) reads through the normal_range
/// cap on the rock: long-range throws already eat disadvantage.
pub static CYCLOPS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CYCLOPS_GREATCLUB);
    actions.push(&CYCLOPS_ROCK);
    actions.push(&*CYCLOPS_MULTI);
    CreatureTemplate {
        name: "Cyclops",
        // 'Y' (capital) — free in the huge giant slot; 'C' is taken by
        // Centaur and 'G' / 'J' / 'g' / 'L' / 'F' are taken by the
        // other giants. 'Y' for Cyclops as a unique giant-kin glyph.
        glyph: 'Y',
        ac: 14,
        // 18d12+72 ≈ 138 average per MM (CR 6).
        hitpoints: "18d12+72".parse().unwrap(),
        speed: 40.,
        strength: 22,
        intelligence: 8,
        dexterity: 11,
        wisdom: 8,
        constitution: 18,
        charisma: 9,
        languages: HashSet::from([Language::Giant]),
        cr: 6.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
