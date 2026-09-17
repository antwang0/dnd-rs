use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    LONGBOW, MINOTAUR_SKELETON_CHARGE, MINOTAUR_SKELETON_GORE, MINOTAUR_SKELETON_SLAM,
    SKELETON_SHORTSWORD,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Skeleton — lower HP than a zombie, DEX-based, and armed at both
/// ranges. Vulnerable to bludgeoning (brittle bones); immune to poison
/// and exhaustion (undead).
///
/// **It has a sword now.** SRD 5.2's Gear line is *"Shortbow,
/// Shortsword"* and it prints both as Actions; this template shipped
/// with the bow alone, which meant a skeleton with an enemy standing
/// next to it could only shoot — at disadvantage, into melee, for the
/// whole fight. See `SKELETON_SHORTSWORD`.
///
/// **The bow is a longbow, which is the engine's and not the book's.**
/// RAW's skeleton carries a shortbow (`1d6` at 80/320). The longbow
/// hits a die harder and reaches further, and it is what this template
/// has pressure-tested the ranged + line-of-sight path with since it
/// was written — a divergence worth naming rather than quietly
/// correcting, because the whole test suite measures range against it.
pub static SKELETON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGBOW);
    // The melee half of RAW's Gear line, and the half that was
    // missing — see `SKELETON_SHORTSWORD`.
    actions.push(&SKELETON_SHORTSWORD);
    CreatureTemplate {
        name: "Skeleton",
        glyph: 'S',
        ac: 14,
        hitpoints: "2d8+4".parse().unwrap(),
        strength: 10,
        dexterity: 16,
        constitution: 15,
        intelligence: 6,
        wisdom: 8,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // SRD 5.2 "Vulnerabilities Bludgeoning" and "Immunities
        // Poison" — brittle bones, and no body chemistry for a poison
        // to work on.
        //
        // The piercing resistance that used to sit here was neither.
        // It came with a note about arrows slipping between ribs, in
        // the same breath as the two clauses that *are* printed, and
        // the Minotaur Skeleton eighty lines down has been saying so
        // in as many words for as long as both have existed: "SRD 5.2
        // prints only the bludgeoning vulnerability here — no piercing
        // resistance, unlike the archer skeleton above". It halved
        // every arrow and every dagger against the commonest low-CR
        // undead on the roster.
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Vulnerability),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // SRD 5.2 "Immunities Poison; Exhaustion, Poisoned" — a pile
        // of bones has no muscle to tire. Three conditions in the
        // quotation and three in the list; the `Charmed` that used to
        // make a fourth was the 2014 undead convention, which SRD 5.2
        // does not carry and which this line has quoted against itself
        // since the quotation was added.
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Minotaur Skeleton — CR 2 large undead. The horns outlived the bull.
///
/// The bestiary's skeleton is an archer; this is the other kind of
/// skeleton entirely, and the pair is 5.2 showing what the template
/// does to a body rather than to a creature. Forty-five hit points on a
/// forty-foot frame, two swings to choose between, and the same brittle
/// bones underneath — bludgeoning still shatters it.
///
/// Action lanes:
/// - **skeleton slam** — 2d10, the swing it makes standing still and
///   the better of the two on any round it has not run.
/// - **skeleton gore** — 2d6 on its own, and the horn the charge below
///   rides.
///
/// The choice between them is the whole creature. A minotaur skeleton
/// that has twenty feet of open floor in front of it wants the gore:
/// RAW's charge adds 2d8 and a knockdown, which turns the weaker swing
/// into far and away the stronger one. Standing in a doorway it wants
/// the slam. That is a decision the picker makes every round off the
/// distance it just covered, and it is the only creature on the undead
/// bench that has one.
///
/// Stat shape: AC 12, 45 HP (6d10+12), STR 18 / DEX 11 / CON 15 /
/// INT 6 / WIS 8 / CHA 5. Speed 40. Vulnerable to bludgeoning, immune
/// to poison. Darkvision 60. CR 2.
pub static MINOTAUR_SKELETON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MINOTAUR_SKELETON_SLAM);
    actions.push(&MINOTAUR_SKELETON_GORE);
    CreatureTemplate {
        name: "Minotaur Skeleton",
        // 's' (lowercase) beside the archer skeleton's 'S' — the same
        // bones on a bigger frame.
        glyph: 's',
        ac: 12,
        // 6d10+12 = 45 average per SRD 5.2 (CR 2).
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 40.,
        strength: 18,
        dexterity: 11,
        constitution: 15,
        intelligence: 6,
        wisdom: 8,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Undead,
        actions,
        // SRD 5.2 prints only the bludgeoning vulnerability here — no
        // piercing resistance, unlike the archer skeleton above, whose
        // row this template deliberately does not copy.
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Vulnerability),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // SRD 5.2 "Immunities Poison; Exhaustion, Poisoned", the same
        // line the archer skeleton above carries and the same `Charmed`
        // struck off it.
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        charge: Some(MINOTAUR_SKELETON_CHARGE),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(t: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            t,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn minotaur_skeleton_template_shape() {
        let a = make(&MINOTAUR_SKELETON_TEMPLATE);
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert!(a.find_action("skeleton slam").is_some());
        assert!(a.find_action("skeleton gore").is_some());
    }

    /// The charge names the gore and not the slam, which is what makes
    /// the two swings a choice rather than a strictly-worse pair.
    ///
    /// Worth pinning because the clause is a string match on the
    /// attack's display name: a rename of either weapon that missed the
    /// rider would leave a skeleton with a charge that can never fire
    /// and nothing anywhere would say so.
    #[test]
    fn the_charge_rides_the_horn_and_not_the_fist() {
        let a = make(&MINOTAUR_SKELETON_TEMPLATE);
        let charge = a.charge().expect("the skeleton charges");
        assert_eq!(charge.weapon, Some("skeleton gore"));
        assert!(a.find_action(charge.weapon.unwrap()).is_some());
        assert!(charge.knocks_prone);
    }

    /// Both skeletons shatter under a hammer, and neither one shrugs
    /// off an arrow.
    ///
    /// This test used to assert the opposite of its second half, on the
    /// strength of a piercing resistance the archer carried and the
    /// minotaur did not — and the docstring called that pair "5.2's,
    /// not a simplification". It was the simplification. SRD 5.2 prints
    /// *"Vulnerabilities Bludgeoning"* on both and nothing else, which
    /// is what the minotaur's own comment had been saying all along;
    /// the archer's resistance came with a note about arrows slipping
    /// between ribs and no line in the book behind it.
    ///
    /// The half that survives is the half that was ever RAW: a hammer
    /// is what kills a skeleton, and an arrow is just an arrow.
    #[test]
    fn neither_skeleton_shrugs_off_an_arrow() {
        use crate::engine::types::DamageModifier;
        let bull = make(&MINOTAUR_SKELETON_TEMPLATE);
        let archer = make(&SKELETON_TEMPLATE);
        for a in [&bull, &archer] {
            assert_eq!(
                a.damage_modifier(DamageType::Bludgeoning),
                Some(DamageModifier::Vulnerability)
            );
            assert_eq!(a.damage_modifier(DamageType::Piercing), None);
        }
    }
}
