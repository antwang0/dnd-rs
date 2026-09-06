use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CRAB_CLAW;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Crab — CR 0 tiny beast. The shoreline's ambient scenery, and the
/// small end of the crab ladder below the Giant Crab (CR ⅛).
///
/// Action lane:
/// - **crab claw** — flat 1 bludgeoning on a DEX-based roll. RAW's
///   printed +2 is proficiency alone; the crab's Strength 6 would have
///   missed it by two, which is why the roll is Dexterity's.
///
/// **Amphibious** (RAW: breathes air and water) is flavor here — the
/// engine models water as terrain rather than as an atmosphere, so the
/// clause has nothing to be true of. The swim speed is the half that
/// does something: `SWIM_SPEED_TAG` makes `TerrainType::Water` free to
/// cross and lifts the underwater melee penalty.
///
/// Stat shape: AC 11, ~3 HP (1d4+1), STR 6, DEX 11, CON 12, INT 1,
/// WIS 8, CHA 2. Speed 20 (walk and swim alike). Skills Stealth.
/// Senses Blindsight 30. Size Tiny. CR 0. XP 10 per RAW.
pub static CRAB_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CRAB_CLAW);
    CreatureTemplate {
        name: "Crab",
        // 'c' (lowercase) — the small scuttler beside the Giant Crab's
        // own glyph, same size-ladder convention as the sharks.
        glyph: 'c',
        ac: 11,
        hitpoints: "1d4+1".parse().unwrap(),
        speed: 20.,
        strength: 6,
        intelligence: 1,
        dexterity: 11,
        wisdom: 8,
        constitution: 12,
        charisma: 2,
        skills: HashSet::from([Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
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
            &CRAB_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn crab_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("crab claw").is_some());
        assert!(a.has_swim_speed());
    }
}
