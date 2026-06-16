use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BOAR_TUSKS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Boar — CR 1/4 medium beast. Stubby tusks (1d6+1 slashing) and a
/// fast 40 ft walk speed. RAW carries Charge (extra 1d6 + DC-11 STR
/// vs Prone after a 20 ft straight dash) and Relentless (drops to 1
/// HP instead of 0 on a damage instance that would otherwise drop
/// it, once per short rest) — both omitted here for engine simplicity;
/// the boar still functions as a fast low-CR melee skirmisher without
/// them. Common low-tier druid Conjure Animals pick and a frequent
/// random encounter filler at low CR.
pub static BOAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BOAR_TUSKS);
    CreatureTemplate {
        name: "Boar",
        glyph: 'b',
        ac: 11,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 13,
        intelligence: 2,
        dexterity: 11,
        wisdom: 9,
        constitution: 12,
        charisma: 5,
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
