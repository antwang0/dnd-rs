use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::COCKATRICE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cockatrice — CR 1/2 monstrosity. Tiny flier with a petrifying bite:
/// 1d4 piercing on hit, plus a CON save (DC 11) or be Petrified for one
/// round. Petrified locks the target out of their action economy and
/// auto-fails STR/DEX saves, so a single bad save can turn an encounter.
/// AC and HP are tuned low (AC 11, ~3d6 HP) so the cockatrice itself
/// goes down quickly — the threat is the rider, not the body.
pub static COCKATRICE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*COCKATRICE_BITE);
    CreatureTemplate {
        name: "Cockatrice",
        // 'k' is currently free (Kobold uses 'K' uppercase, no others
        // claim lowercase k).
        glyph: 'k',
        ac: 11,
        hitpoints: "5d6".parse().unwrap(),
        // RAW speed line: Speed 20 ft., fly 40 ft.
        speed: 20.0,
        fly_speed: 40.0,
        strength: 6,
        intelligence: 2,
        dexterity: 12,
        wisdom: 13,
        constitution: 12,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
