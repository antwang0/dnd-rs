use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{Multiattack, NIGHTMARE_HOOVES};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static NIGHTMARE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "flaming hooves",
    sub_attack: &NIGHTMARE_HOOVES,
    count: 2,
});

/// Nightmare — fiendish steed wreathed in flame (CR 3, MM p.235). A
/// Large fiend with fire immunity and cold resistance. Two-hoof
/// multiattack, and the hooves burn: SRD 5.2 prints *"13 (2d8 + 4)
/// Bludgeoning damage plus 10 (3d6) Fire damage"*, and both halves are
/// modeled now. This note used to say the fire was dropped "for
/// simplicity" and that the steed's own fire immunity covered the
/// theme — which left ten average points a hoof, twice a round, on a
/// CR 3 creature, unrolled. See `NIGHTMARE_HOOVES`.
pub static NIGHTMARE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*NIGHTMARE_MULTI);
    CreatureTemplate {
        name: "Nightmare",
        glyph: 'N',
        ac: 13,
        hitpoints: "8d10+24".parse().unwrap(),
        // RAW speed line: Speed 60 ft., fly 90 ft.
        speed: 60.0,
        fly_speed: 90.0,
        // SRD 5.2: *"Fly 90 ft. (hover)"*.
        hovers: true,
        strength: 18,
        dexterity: 15,
        constitution: 16,
        intelligence: 10,
        wisdom: 13,
        charisma: 15,
        languages: HashSet::from([Language::Abyssal, Language::Infernal]),
        cr: 3.0,
        size: Size::Large,
        // 5e Mounted Combat: MM: "a nightmare... serves as a fiendish steed". The Fiend half of the
        // same argument the pegasus makes.
        mountable: true,
        creature_type: CreatureType::Fiend,
        actions,
        // SRD 5.2: *"Immunities Fire"*, and nothing else. The Cold
        // resistance beside it was a 2014 reading.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        ..CreatureTemplate::defaults()
    }
});
