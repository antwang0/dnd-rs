use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_CRAB_CLAW;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Crab — CR 1/8 medium beast. The cheapest aquatic-flavored
/// ambient creature in the upper pool, joining the low-end fillers
/// (Stirge, Hyena, Boar) at the bottom of the CR ladder. Single claw
/// pinch attack, no rider — the design intent is "pack them in clusters
/// of 4-6 around the party so the AoE / cleave economy stays relevant".
///
/// Stats roughly track MM Giant Crab at CR ⅛ — high STR for the claw
/// damage, AC 15 from the natural chitin armor, and low HP (13 average).
/// Swim speed is collapsed into the walking lane (the engine has no
/// underwater terrain tag); Blindsight 30 reflects the crab's
/// tremorsense / antennae feeler envelope.
pub static GIANT_CRAB_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_CRAB_CLAW);
    CreatureTemplate {
        name: "Giant Crab",
        // 'c' (lowercase) — distinct from 'C' (Carrion Crawler / Cyclops /
        // Cambion uppercase) and 'B' (Bear). The compact silhouette evokes
        // the crab's low profile and crowded claw legs.
        glyph: 'c',
        ac: 15,
        // 3d8 = 13 average per MM.
        hitpoints: "3d8".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 1,
        dexterity: 15,
        wisdom: 9,
        constitution: 11,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn giant_crab_is_a_cheap_ambient_beast() {
        let a = ActorInstance::from_creature_template(
            &GIANT_CRAB_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.125);
        assert!(a.find_action("crab claw").is_some());
    }
}
