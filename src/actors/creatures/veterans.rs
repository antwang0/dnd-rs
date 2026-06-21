use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, VETERAN_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Veteran — CR 3 humanoid soldier. Plate-armored melee with a 2x
/// longsword multiattack plus a heavy crossbow for ranged finishers.
/// Higher AC than the berserker but lower HP — the trade-off
/// between staying power and stack.
pub static VETERAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VETERAN_MULTI);
    actions.push(&HEAVY_CROSSBOW);
    CreatureTemplate {
        name: "Veteran",
        // 'v' — distinct from existing 'V' (Vampire Spawn).
        glyph: 'v',
        ac: 17,
        // 9d8+18 = 58 average per MM.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 13,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
