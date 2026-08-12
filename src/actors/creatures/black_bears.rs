use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BLACK_BEAR_MULTI, BLACK_BEAR_REND};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Black Bear — CR ½ medium beast. The small end of the bear ladder,
/// below the Brown Bear (CR 1) and the Polar Bear (CR 2): same shape,
/// smaller numbers. SRD 5.2 collapses the old claw-and-bite pair into
/// a single **Rend** limb swung twice, which is what the multiattack
/// below does.
///
/// Action lanes:
/// - **black bear multiattack** — two rends per Action (1d6+STR
///   slashing each). The only lane worth taking.
/// - **black bear rend** — the single swing, kept on the list so a
///   bear with a spent Action still has something the prompt parser
///   and the reaction lanes can name.
///
/// **Climb 30 / Swim 30** are not modeled as separate lanes; the board
/// is flat and the bear's walking speed already covers the ground it
/// can reach in a round.
///
/// Defensive identity: AC 11, ~19 HP (3d8+6), darkvision 60. Vanilla
/// beast envelope — no resistances, no condition immunities.
///
/// Stat shape: AC 11, ~19 HP, STR 15, DEX 12, CON 14, INT 2, WIS 12,
/// CHA 7. Speed 30. Skills Perception. Senses Darkvision 60. Size
/// Medium. CR ½. XP 100 per RAW.
pub static BLACK_BEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BLACK_BEAR_REND);
    actions.push(&*BLACK_BEAR_MULTI);
    CreatureTemplate {
        name: "Black Bear",
        // 'B' — the bear cohort's glyph, shared with the brown and
        // polar bears already on the roster.
        glyph: 'B',
        ac: 11,
        hitpoints: "3d8+6".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 2,
        dexterity: 12,
        wisdom: 12,
        constitution: 14,
        charisma: 7,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
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

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &BLACK_BEAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn black_bear_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("black bear rend").is_some());
        assert!(a.find_action("black bear multiattack").is_some());
    }
}
