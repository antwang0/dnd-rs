use crate::actions::class_features::{DEATHLESS_DASH, DEATHLESS_DISENGAGE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    THROWN_UMBRAL_DAGGER, UMBRAL_DAGGER, VAMPIRE_CHARMING_GAZE, VAMPIRE_FAMILIAR_MULTI,
    VAMPIRE_MULTIATTACK,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::lighting::SunlightFrailty;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Vampire — CR 13 boss undead. The full Vampire (Lord) writeup: stacks
/// the Vampire Spawn's lifesteal bite into a double-tap multiattack and
/// adds a charming gaze to lock down an ally before the bite-train rolls
/// in. Regenerates 20 HP at end of round (suppressed by radiant — our
/// proxy for the "sunlight / running water" weakness). Standard undead
/// immunity envelope (poison / charm) plus vulnerable-to-radiant on top.
///
/// The regen + multiattack + charm package is the marquee boss-tier
/// pattern: chip damage is wasted against the regen, so the party has
/// to bring radiant burst or stall through the charm to actually drop
/// the vampire below zero.
pub static VAMPIRE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&VAMPIRE_CHARMING_GAZE);
    actions.push(&*VAMPIRE_MULTIATTACK);
    CreatureTemplate {
        name: "Vampire",
        // 'v' (lowercase) to avoid clashing with 'V' (Vampire Spawn).
        glyph: 'v',
        ac: 16,
        // RAW speed line: Speed 40 ft., Climb 40 ft. The climb has no
        // lane on a flat board; the walking half is the number.
        speed: 40.,
        // 23d8+92 ≈ 195 average per the MM Vampire stat block.
        hitpoints: "23d8+92".parse().unwrap(),
        strength: 18,
        dexterity: 18,
        constitution: 18,
        intelligence: 17,
        wisdom: 15,
        charisma: 18, // spell DC / charm DC
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common]),
        cr: 13.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e MM Vampire: resistant to necrotic + non-magical BPS,
        // immune to poison. Radiant is the regen-suppressor (proxy
        // for the "sunlight / holy water" RAW downside).
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Vampire saves: prof in DEX / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // Regenerate 20 HP at end of round while combat-active. Radiant
        // damage suppresses for the round (proxy for 5e's "sunlight /
        // holy water" downside — radiant is the carrier for both).
        regen_per_round: 20,
        regen_suppressors: HashSet::from([DamageType::Radiant]),
        has_magic_resistance: true,
        legendary_actions_per_round: 3,
        legendary_actions: crate::engine::legendary_actions::VAMPIRE_LEGENDARY,
        has_extra_attack: true,
        // 5e Vampire **Sunlight Hypersensitivity**: "the vampire takes
        // 20 radiant damage when it starts its turn in sunlight. While
        // in sunlight, it has disadvantage on attack rolls and ability
        // checks." The 20 radiant lands at the top of the turn through
        // `apply_sunlight_hypersensitivity`, and it lands on a creature
        // that is already vulnerable to radiant — 40 a round, which is
        // exactly the point of the trait and the reason a vampire fight
        // happens at night.
        sunlight_frailty: Some(SunlightFrailty::Hypersensitivity),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

/// Vampire Familiar — CR 3 humanoid, and the only creature in this file
/// that is still alive.
///
/// That is the thing worth noticing about it. The Vampire and the
/// Vampire Spawn are Undead with the whole envelope — poison immunity,
/// radiant vulnerability, sunlight to fear. The familiar is a
/// **Humanoid**: a living servant, charmed once and kept, and it dies
/// the ordinary way. What it borrowed from its master is the necrotic
/// resistance and the dagger, and neither of those is enough to make it
/// anything other than a person who made a bad decision.
///
/// Action lanes:
/// - **double umbral dagger** — two swings, each 1d4+DEX piercing plus
///   an unconditional 3d4 necrotic. Fourteen necrotic a round before
///   the piercing, which is why a CR-3 stat block with 65 hit points
///   and no tricks is worth taking seriously.
/// - **umbral dagger** / **thrown umbral dagger** — the swing and the
///   same weapon at 8/24 tiles, rider intact.
/// - **deathless dash** / **deathless disengage** — RAW's Deathless
///   Agility, "the familiar takes the Dash or Disengage action" as a
///   Bonus Action. Nimble Escape's other half: the goblin gets Hide
///   because a goblin wants not to be found, and the familiar gets Dash
///   because a thrall goes where its master needs a body.
///
/// **Charmed immunity, with an exception the engine cannot spell.** RAW
/// reads "Immunities Charmed (except from its vampire master)", and the
/// engine's immunity set is a set of conditions, not a set of
/// conditions-qualified-by-source. The exception ships unmodeled and
/// the immunity ships whole, which is the right way round: the clause
/// exists so a party cannot charm the familiar away from its master,
/// and the master charming its own thrall is a thing that has already
/// happened by the time the fight starts. Modeling it properly would
/// mean a per-condition source allowlist on the immunity check —
/// worth doing the day a second stat block wants one.
///
/// **Vampiric Connection** — the telepathy-and-shared-senses trait — is
/// narrative: it is how the vampire knows what the familiar knows
/// between scenes, and it has no surface in a combat round.
///
/// Stat shape: AC 15, 65 HP (10d8+20), STR 17 / DEX 16 / CON 15 / INT 10
/// / WIS 10 / CHA 14. Speed 30 (RAW also Climb 30, which has no lane on
/// a flat board). Resistances: necrotic. Skills: Perception, Persuasion,
/// Stealth. Darkvision 60. CR 3.
pub static VAMPIRE_FAMILIAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VAMPIRE_FAMILIAR_MULTI);
    actions.push(&UMBRAL_DAGGER);
    actions.push(&THROWN_UMBRAL_DAGGER);
    // RAW **Deathless Agility**: "the familiar takes the Dash or
    // Disengage action" as a Bonus Action. Two entries because RAW's
    // "or" is a choice the creature makes each turn.
    actions.push(&DEATHLESS_DASH);
    actions.push(&DEATHLESS_DISENGAGE);
    CreatureTemplate {
        name: "Vampire Familiar",
        // 'f' — the thrall, one rung under the 'V' band its masters
        // hold, and lowercase because it is the small one in the room.
        glyph: 'f',
        ac: 15,
        // 10d8+20 = 65 average per SRD 5.2 (CR 3).
        hitpoints: "10d8+20".parse().unwrap(),
        // RAW speed line: Speed 30 ft., Climb 30 ft. The climb has no
        // lane on a flat board; the walking half is the number.
        speed: 30.,
        strength: 17,
        dexterity: 16,
        constitution: 15,
        intelligence: 10,
        wisdom: 10,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        // Humanoid, not Undead — see the docstring. This is the field
        // that makes the familiar vulnerable to everything that hunts
        // the living, and it is RAW.
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Perception, Skill::Persuasion, Skill::Stealth]),
        damage_modifiers: damage_modifiers_from([(
            DamageType::Necrotic,
            DamageModifier::Resistance,
        )]),
        // RAW: DEX +5 and WIS +2 against a +2 proficiency bonus.
        proficient_saves: HashSet::from([AbilityScoreType::Dexterity, AbilityScoreType::Wisdom]),
        // RAW's "(except from its vampire master)" has no lane — see
        // the docstring.
        condition_immunities: HashSet::from([Condition::Charmed]),
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
    fn vampire_familiar_template_shape() {
        let a = make(&VAMPIRE_FAMILIAR_TEMPLATE);
        assert_eq!(a.cr(), 3.0);
        assert!(a.find_action("double umbral dagger").is_some());
        assert!(a.find_action("thrown umbral dagger").is_some());
        assert!(a.find_action("deathless dash").is_some());
        assert!(a.find_action("deathless disengage").is_some());
    }

    /// The familiar is a living servant, and every one of the four
    /// things that follow from that is load-bearing.
    ///
    /// It is the only entry in this file that is not Undead, so it can
    /// be charmed away from — no, it cannot, it is immune — but it can
    /// be raised, poisoned, put to sleep, and turned into nothing by a
    /// spell that only touches Humanoids. A refactor that "tidied" the
    /// file by making every vampire Undead would change what the party
    /// is allowed to cast at it, silently.
    #[test]
    fn the_familiar_is_the_one_that_is_still_alive() {
        let a = make(&VAMPIRE_FAMILIAR_TEMPLATE);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert_eq!(
            make(&VAMPIRE_TEMPLATE).creature_type(),
            CreatureType::Undead
        );
        // Borrowed from the master, and the whole of what it borrowed.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(!a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
