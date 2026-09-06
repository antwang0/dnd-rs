use crate::actions::class_features::{BLOOD_FRENZY_TAG, SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    SAHUAGIN_BITE, SAHUAGIN_CLAWS, SAHUAGIN_MULTI, SPEAR, THROWN_SPEAR,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sahuagin — CR 1/2 shark-tooth raider. The signature trait is Blood
/// Frenzy: melee attacks against any wounded target (current HP < max HP)
/// roll with advantage. Wired into `compute_attack_mode` via the passive
/// feature tag, so the first sahuagin hit doesn't get the bonus but every
/// follow-up swing in the same encounter — once any sahuagin has scratched
/// the target — does.
pub static SAHUAGIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SAHUAGIN_BITE);
    actions.push(&SAHUAGIN_CLAWS);
    actions.push(&*SAHUAGIN_MULTI);
    // RAW's stat block gives the sahuagin a spear at "reach 5 ft. or
    // range 20/60 ft." — a single line that is two attacks, and the
    // half of it the template used to be missing entirely. The
    // multiattack still opens with tooth and claw; the spear is what a
    // raider does about a target on the far bank.
    //
    // The throw is the live case for 5e's underwater ranged exemption.
    // `UNDERWATER_RANGED_WEAPONS` has named the spear since it was
    // written, and until now no creature in the engine could make a
    // ranged attack with one — so the row could never match, and the
    // rule it encodes had nothing to apply itself to. A sahuagin is
    // exactly the creature it was written for: it swims, it fights in
    // water, and its spear carries there where a bow does not.
    actions.push(&SPEAR);
    actions.push(&THROWN_SPEAR);
    CreatureTemplate {
        name: "Sahuagin",
        // 'S' — uppercase since lowercase 's' is Stirge; 'S' was free in
        // the medium-humanoid lane.
        glyph: 'S',
        ac: 12,
        hitpoints: "4d8+4".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 12,
        dexterity: 11,
        wisdom: 13,
        constitution: 12,
        charisma: 9,
        // 120 ft darkvision — sahuagin are abyssal-deep-water hunters
        // in 5e and the darkvision range matches their MM stat block.
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Blood Frenzy passive: advantage on melee attacks vs wounded
        // targets. Read by `compute_attack_mode`'s gate.
        features: HashSet::from([SWIM_SPEED_TAG, BLOOD_FRENZY_TAG, UNDERWATER_BREATHING_TAG]),
        skills: HashSet::from([Skill::Perception]),
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
    fn sahuagin_has_blood_frenzy_feature() {
        let a = ActorInstance::from_creature_template(
            &SAHUAGIN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_passive_feature(BLOOD_FRENZY_TAG));
    }
}
