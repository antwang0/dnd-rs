use crate::actions::class_features::{
    AQUATIC_ONLY_TAG, BLOOD_FRENZY_TAG, SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PIRANHA_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Piranha — CR 0 tiny beast. One hit point and one point of damage,
/// and the same **Blood Frenzy** the sahuagin and all three sharks
/// carry: advantage on every bite against a target that isn't at full
/// hit points.
///
/// SRD 5.2 prints the clause inline on the Hit line — *"Melee Attack
/// Roll: +5 (with Advantage if the target doesn't have all its Hit
/// Points)"* — rather than as a named trait, but it is the same rule,
/// and it rides the same `BLOOD_FRENZY_TAG` the rest of the cohort
/// does rather than a second copy spelled differently.
///
/// One piranha is nothing. The stat block exists to be fielded in
/// numbers, and `SWARM_OF_PIRANHAS_TEMPLATE` is what a river actually
/// throws at a party.
///
/// This used to be two stat blocks. The 2014 printing called the same
/// fish a **Quipper**, and the bestiary carried both — same AC, same
/// CR, same ability array to the point, same Blood Frenzy, same swim,
/// differing only in how the one hit point was spelled (`1d1` against
/// RAW's `1d4 − 1`). Two templates for one creature is two things the
/// encounter generator can roll and no way for a reader to tell which
/// is canonical, so the quipper is gone and this is the survivor.
///
/// **Water Breathing** collapses to flavor — water is terrain in this
/// engine, not an atmosphere. The swim speed is the half with teeth.
///
/// Stat shape: AC 13, ~1 HP (1d4−1, floored at 1), STR 2, DEX 16,
/// CON 9, INT 1, WIS 7, CHA 2. Speed 40 (RAW swim 40; the walking 5 is
/// collapsed away). Senses Darkvision 60. Size Tiny. CR 0. XP 10 per
/// RAW.
pub static PIRANHA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PIRANHA_BITE);
    CreatureTemplate {
        name: "Piranha",
        // 'i' — a thin lowercase silhouette for a thin fish; 'q' and
        // 'Q' are the shark ladder and 'p' is taken on the lowercase
        // shelf.
        glyph: 'i',
        ac: 13,
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 40.,
        strength: 2,
        intelligence: 1,
        dexterity: 16,
        wisdom: 7,
        constitution: 9,
        charisma: 2,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([
            SWIM_SPEED_TAG,
            BLOOD_FRENZY_TAG,
            UNDERWATER_BREATHING_TAG,
            // RAW's *only*: a fish out of water.
            AQUATIC_ONLY_TAG,
        ]),
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
            &PIRANHA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn piranha_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("piranha bite").is_some());
        assert!(a.has_swim_speed());
    }

    /// The inline "(with Advantage if the target doesn't have all its
    /// Hit Points)" is Blood Frenzy under another name, and it rides
    /// the shared tag rather than a private copy.
    #[test]
    fn piranha_carries_blood_frenzy() {
        assert!(make().has_passive_feature(BLOOD_FRENZY_TAG));
    }
}
