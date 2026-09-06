use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MERFOLK_SPEAR;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Merfolk — CR ⅛ medium humanoid. The bestiary's cheapest aquatic
/// body, and the one the underwater rules were written about.
///
/// Action lane: **merfolk spear**, 1d6+STR piercing. A spear rather
/// than a trident, and the choice matters exactly once: RAW exempts
/// both from the underwater melee penalty, so a merfolk in its own
/// water swings at no disadvantage — which is the difference between
/// this stat block and any land humanoid dragged into a lake.
///
/// **Amphibious** — RAW: "the merfolk can breathe air and water" — is
/// the trait, and it lands on `SWIM_SPEED_TAG`, which is the engine's
/// name for the whole aquatic package: water costs nothing to cross,
/// and the underwater melee clause lifts. The breathing half has no
/// surface, since the engine has no drowning clock.
///
/// The merrow four CRs above it is the same creature corrupted, and
/// they share the aquatic pool. This is what a party meets in the
/// shallows before that.
///
/// Stat shape per the SRD: AC 11, 11 HP (2d8+2), STR 10 / DEX 13 / CON
/// 12 / INT 11 / WIS 11 / CHA 12. Speed 10 walking, 40 swimming — the
/// engine has one speed and takes the water one, since that is where a
/// merfolk fights. Skills: Perception. Languages: Common, Primordial
/// (Aquan). CR ⅛.
pub static MERFOLK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MERFOLK_SPEAR);
    CreatureTemplate {
        name: "Merfolk",
        // 'e' (lowercase) — free. 'm' is the Merrow's and the Magmin's,
        // and this is neither.
        glyph: 'e',
        ac: 11,
        // 2d8+2 ≈ 11 average per the SRD (CR ⅛).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 10,
        dexterity: 13,
        constitution: 12,
        intelligence: 11,
        wisdom: 11,
        charisma: 12,
        skills: HashSet::from([Skill::Perception]),
        languages: HashSet::from([Language::Common, Language::Primordial]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
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
            &MERFOLK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn merfolk_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("merfolk spear").is_some());
    }

    /// The spear and the swim speed are the same claim twice: RAW's
    /// underwater melee clause is waived either by carrying one of five
    /// named weapons or by having a swimming speed, and the merfolk has
    /// both, which is why it is the one humanoid that fights normally in
    /// a lake.
    #[test]
    fn a_merfolk_swings_at_no_penalty_in_its_own_water() {
        use crate::engine::underwater::melee_keeps_edge;
        let a = make();
        assert!(a.has_swim_speed());
        assert!(melee_keeps_edge("merfolk spear"));
    }
}
