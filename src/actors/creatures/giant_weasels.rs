use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_WEASEL_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Weasel — CR ⅛ medium beast. Nine hit points behind AC 13 on a
/// forty-foot speed, with a DEX-based bite: the fastest thing in the
/// low-CR beast pool and the one that is hardest to actually hit.
///
/// The bigger sibling of the Weasel (CR 0) already on the roster. Both
/// are DEX creatures rather than STR ones, which is unusual for a beast
/// and is where the giant weasel's +5 to hit comes from — it is quicker
/// than it is strong, and its damage says so.
///
/// Stat shape per the SRD: AC 13, 9 HP (2d8), STR 11 / DEX 16 / CON 10
/// / INT 4 / WIS 12 / CHA 5. Speed 40. Darkvision 60. Skills:
/// Perception, Stealth. CR ⅛.
pub static GIANT_WEASEL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_WEASEL_BITE);
    CreatureTemplate {
        name: "Giant Weasel",
        // 'i' (lowercase) — free; 'w' is the Winged Kobold's, 'W' the
        // Water Weird's, and the Weasel already holds its own glyph.
        glyph: 'i',
        ac: 13,
        // 2d8 ≈ 9 average per the SRD (CR ⅛).
        hitpoints: "2d8".parse().unwrap(),
        speed: 40.,
        strength: 11,
        dexterity: 16,
        constitution: 10,
        intelligence: 4,
        wisdom: 12,
        charisma: 5,
        senses: HashSet::from([crate::engine::types::SpecialSense::Darkvision(60)]),
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
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
    fn giant_weasel_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GIANT_WEASEL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.125);
        assert!(a.find_action("giant weasel bite").is_some());
        // The bite reads DEX, which is where RAW's +5 comes from on a
        // creature whose Strength is 11.
        assert_eq!(
            GIANT_WEASEL_BITE.attack_ability,
            crate::engine::types::AbilityScoreType::Dexterity
        );
    }
}
