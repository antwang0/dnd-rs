use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::COCKATRICE_BITE;
use crate::conditions::Condition;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cockatrice — CR 1/2 monstrosity. Tiny flier with a petrifying bite:
/// 1d4 piercing on hit, plus a DC 11 CON save that opens SRD 5.2's
/// two-stage ladder — Restrained on the first failure, stone on a
/// second at the end of the victim's next turn. See
/// `engine::staged_saves`.
///
/// AC and HP are tuned low (AC 11, ~3d6 HP) so the cockatrice itself
/// goes down quickly, which is the point: the round its victim spends
/// stiffening is a round the party has to kill the bird before the
/// second die is rolled.
pub static COCKATRICE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*COCKATRICE_BITE);
    CreatureTemplate {
        name: "Cockatrice",
        // 'k' is currently free (Kobold uses 'K' uppercase, no others
        // claim lowercase k).
        glyph: 'k',
        ac: 11,
        hitpoints: "5d6+5".parse().unwrap(),
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
        // SRD 5.2 "Immunities Petrified" — the one creature in the book
        // whose own bite is petrification, and the clause that stops two
        // cockatrices in a coop turning each other to stone.
        condition_immunities: HashSet::from([Condition::Petrified]),
        actions,
        ..CreatureTemplate::defaults()
    }
});
