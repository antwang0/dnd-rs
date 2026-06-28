use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LIZARD_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Lizard — CR 0 tiny beast. The canonical "ambient reptile" cave
/// fauna: a single flat-1 bite, AC 10, 2 HP. Slots beside the Cat
/// (CR 0 tiny climber), Rat (CR 0 tiny rodent), Bat (CR 0 tiny flier)
/// at the very bottom of the CR ladder. Distinguished from the
/// `GIANT_LIZARD_TEMPLATE` (CR ¼ large reptile with 1d8 bite) by
/// being smaller, lacking ride-mountable utility, and dealing flat 1
/// damage. The threat profile is "ambient noise creature, the cave
/// is full of these but they don't matter individually."
///
/// Action lane:
/// - **lizard bite** — STR-based 1-flat piercing melee via the shared
///   `LIZARD_BITE` static. The lizard's only swing. Same flat-1
///   shape as Rat / Bat at this CR tier.
///
/// **Spider Climb** (RAW: 30 climb on difficult / vertical surfaces)
/// is captured by the speed-20 baseline — the engine collapses climb
/// into the per-creature speed. Darkvision 30 surfaces through the
/// standard `SpecialSense::Darkvision` chokepoint.
///
/// Defensive identity: AC 10 (tiny + DEX 11), 2 HP. Vanilla beast
/// envelope — no resistances or condition immunities. The lizard
/// dies to any solid hit; the role is "scaly ambient texture" rather
/// than a credible damage threat.
///
/// Stat shape: AC 10, ~2 HP (1d4 → average 2), STR 2, DEX 11, CON 10,
/// INT 1, WIS 8, CHA 3. Speed 20 — RAW: walking 20 ft + climb 20 ft
/// (the engine collapses ground + climb into a single per-creature
/// speed). Size Tiny. CR 0. XP: 10 per RAW.
pub static LIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LIZARD_BITE);
    CreatureTemplate {
        name: "Lizard",
        // 'l' (lowercase) — tiny reptile silhouette. 'L' (uppercase) is
        // taken by Lamia / Lich / Lion / Lizardfolk / Lemure cohort;
        // lowercase 'l' reads as "ambient scaly tile-walker" beside 'c'
        // (Cat), 'f' (Frog), 'r' (Rat) at the bottom of the CR ladder.
        glyph: 'l',
        ac: 10,
        // RAW: 2 (1d4) — drifts between 1 and 4 with average 2.5. CON
        // 10 (dead-center), so no +/- modifier; the engine's HP-roll
        // floor of 1 is a safety net.
        hitpoints: "1d4".parse().unwrap(),
        speed: 20.,
        strength: 2,
        intelligence: 1,
        dexterity: 11,
        wisdom: 8,
        constitution: 10,
        charisma: 3,
        // Darkvision 30 — the lizard's signature low-light sense, in
        // the same envelope as the Rat / Frog / Camel cohort at this
        // CR tier.
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 0.0,
        size: Size::Tiny,
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

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &LIZARD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn lizard_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("lizard bite").is_some());
    }
}
