use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BASILISK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Basilisk — CR 3 monstrosity. Eight-legged reptile whose bite deals
/// 2d6+3 piercing and opens the petrification ladder on a failed DC 12
/// CON save: Restrained now, stone one failed save later. See
/// `engine::staged_saves`.
///
/// **Delivery diverges from RAW**, which gives the basilisk a
/// Petrifying Gaze as a bonus-action 30-foot cone and leaves the bite
/// plain. The engine hangs the ladder on the bite, which is where it
/// has always been; the ladder itself is the part that was missing.
///
/// AC 15, ~52 HP (8d8+16). Slow (speed 20ft) but durable, with
/// darkvision 60ft.
pub static BASILISK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BASILISK_BITE);
    CreatureTemplate {
        name: "Basilisk",
        glyph: 'ß',
        ac: 15,
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 20.,
        strength: 16,
        intelligence: 2,
        dexterity: 8,
        wisdom: 8,
        constitution: 15,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
