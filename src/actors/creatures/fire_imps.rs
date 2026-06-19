use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FIRE_BOLT, FRIGHTFUL_PRESENCE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Small fiend whose pressure tools are typed-damage output and the
/// Frightened condition. Action: Fire Bolt (ranged 1d10 fire). Bonus
/// action: Frightful Presence (WIS save vs DC 11 within 6 tiles or
/// frightened for 3 rounds). Immune to fire and poison, vulnerable to
/// cold — gives the player a real reason to think about damage typing
/// when a fire imp is on the board.
pub static FIRE_IMP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*FRIGHTFUL_PRESENCE);
    CreatureTemplate {
        name: "Fire Imp",
        glyph: 'I',
        ac: 13,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 20.,
        strength: 6,
        intelligence: 11,
        dexterity: 17,
        wisdom: 12,
        constitution: 12,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 1.0,
        size: Size::Tiny,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Vulnerability),
        ]),
        has_magic_resistance: true,
        ..CreatureTemplate::defaults()
    }
});
