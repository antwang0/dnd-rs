use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::VENOMOUS_SNAKE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Venomous Snake — CR ⅛ tiny beast. The small end of the serpent
/// ladder below the Giant Poisonous Snake (CR ¼), carrying the same
/// 1d6 venom die on a quarter of the frame.
///
/// Action lane:
/// - **venomous snake bite** — DEX-based 1d4+DEX piercing plus a flat
///   1d6 poison rider. On a hit, no save: SRD 5.2 prints the poison as
///   part of the Hit line here rather than as the save-gated clause the
///   giant version carries.
///
/// The venom is what makes a five-hit-point animal worth a party's
/// attention at level 1 — an average bite is about seven damage, which
/// is a fifth of a fresh fighter and a third of a fresh wizard.
///
/// Stat shape: AC 12, ~5 HP (2d4), STR 2, DEX 15, CON 11, INT 1,
/// WIS 10, CHA 3. Speed 30 (walk and swim alike). Senses Blindsight 10.
/// Size Tiny. CR ⅛. XP 25 per RAW.
pub static VENOMOUS_SNAKE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&VENOMOUS_SNAKE_BITE);
    CreatureTemplate {
        name: "Venomous Snake",
        // 'n' — the low serpentine curve on the lowercase shelf; 's'
        // is taken and 'S' belongs to the giant-snake cohort.
        glyph: 'n',
        ac: 12,
        hitpoints: "2d4".parse().unwrap(),
        speed: 30.,
        strength: 2,
        intelligence: 1,
        dexterity: 15,
        wisdom: 10,
        constitution: 11,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(10)]),
        cr: 0.125,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([SWIM_SPEED_TAG]),
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
            &VENOMOUS_SNAKE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn venomous_snake_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("venomous snake bite").is_some());
        assert!(a.has_swim_speed());
    }
}
