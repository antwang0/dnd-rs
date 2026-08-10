use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WARHORSE_SKELETON_HOOVES;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Warhorse Skeleton — CR ½ large undead. A dead cavalry mount that has
/// not noticed, and the bestiary's answer to "what does an undead
/// knight ride".
///
/// Action lane: **warhorse skeleton hooves**, 2d6+STR bludgeoning. One
/// swing, no rider. What makes it a CR ½ rather than a CR ¼ is the
/// sixty-foot speed attached to it: the skeleton crosses the board in
/// one turn and swings on arrival.
///
/// **Mountable**, which is the whole reason the stat block exists.
/// Paired with a Skeleton or a Wight it turns a foot-soldier into
/// cavalry, and the engine's mount lane carries the pairing — the rider
/// moves at the mount's speed and the mount takes the hits.
///
/// The undead envelope is the usual one and reads the way every skeleton
/// does: bludgeoning **vulnerability** (a skeleton is a stack of bones
/// and a mace is what bones are for), poison immunity, and the condition
/// list of a thing with no blood, no breath and no fear.
///
/// Stat shape per the SRD: AC 13 (barding scraps), 22 HP (3d10+6), STR
/// 18 / DEX 12 / CON 15 / INT 2 / WIS 8 / CHA 5. Speed 60. Darkvision
/// 60. CR ½.
pub static WARHORSE_SKELETON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WARHORSE_SKELETON_HOOVES);
    CreatureTemplate {
        name: "Warhorse Skeleton",
        // 's' (lowercase) — the skeleton band, which is what a party
        // should read it as before it notices the size.
        glyph: 's',
        ac: 13,
        // 3d10+6 ≈ 22 average per the SRD (CR ½).
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 60.,
        strength: 18,
        dexterity: 12,
        constitution: 15,
        intelligence: 2,
        wisdom: 8,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
        size: Size::Large,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Bludgeoning, DamageModifier::Vulnerability),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Poisoned,
        ]),
        // The point of the stat block: something for the skeletal
        // knight to sit on.
        mountable: true,
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
            &WARHORSE_SKELETON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn warhorse_skeleton_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert!(a.find_action("warhorse skeleton hooves").is_some());
    }

    /// Mountable and fast, which are the same claim: a mount that moved
    /// at thirty feet would be a worse pair of legs than the rider's
    /// own, and nothing would ever climb on.
    #[test]
    fn the_dead_horse_is_worth_riding() {
        let a = make();
        assert!(a.is_mountable());
        assert_eq!(a.speed(), 60.);
    }

    /// A skeleton is bones, and a mace is what bones are for. The
    /// vulnerability is the one place an undead stat block gives the
    /// party something back, and it is easy to drop by accident when
    /// copying an immunity list from a sibling.
    #[test]
    fn bones_break_under_bludgeoning() {
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Vulnerability)
        );
    }
}
