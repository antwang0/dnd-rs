use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{Multiattack, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static NIGHTMARE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "flaming hooves",
    sub_attack: &SLAM,
    count: 2,
});

/// Nightmare — fiendish steed wreathed in flame (CR 3, MM p.235). A
/// Large fiend with fire immunity and cold resistance. Two-hoof
/// multiattack. The hooves deal fire damage RAW but we reuse SLAM
/// (bludgeoning) for simplicity; the fire immunity covers the thematic
/// element.
pub static NIGHTMARE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*NIGHTMARE_MULTI);
    CreatureTemplate {
        name: "Nightmare",
        glyph: 'N',
        ac: 13,
        hitpoints: "8d10+16".parse().unwrap(),
        speed: 60.,
        strength: 18,
        dexterity: 15,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 15,
        languages: HashSet::from([Language::Abyssal, Language::Infernal]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        ..CreatureTemplate::defaults()
    }
});
