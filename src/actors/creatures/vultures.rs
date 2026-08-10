use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::VULTURE_BEAK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Vulture — CR 0 medium beast. A carrion bird with `has_pack_tactics`,
/// which on a creature that flies means the advantage arrives from
/// wherever it likes.
///
/// The little sibling of the Giant Vulture (CR 1) already on the
/// roster, and the pairing is the point: RAW gives both the same trait,
/// and the ordinary one is what circles a battlefield in numbers while
/// the giant one is a fight.
///
/// Stat shape per the SRD: AC 10, 5 HP (1d8+1), STR 7 / DEX 10 / CON 13
/// / INT 2 / WIS 12 / CHA 4. Speed 50 (flying). Skills: Perception.
/// CR 0.
pub static VULTURE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&VULTURE_BEAK);
    CreatureTemplate {
        name: "Vulture",
        // 'u' is the Duergar's; 'V' (uppercase) is free below the
        // Vampire / Vrock band's own letters.
        glyph: 'V',
        ac: 10,
        // 1d8+1 ≈ 5 average per the SRD (CR 0).
        hitpoints: "1d8+1".parse().unwrap(),
        speed: 50.,
        strength: 7,
        dexterity: 10,
        constitution: 13,
        intelligence: 2,
        wisdom: 12,
        charisma: 4,
        skills: HashSet::from([Skill::Perception]),
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
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
    fn vulture_template_shape() {
        let a = ActorInstance::from_creature_template(
            &VULTURE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert!(a.find_action("vulture beak").is_some());
        assert!(a.has_pack_tactics());
    }
}
