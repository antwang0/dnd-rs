use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{INCUBUS_MULTI, INCUBUS_NIGHTMARE, INCUBUS_RESTLESS_TOUCH};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Incubus — CR 4 medium fiend, and not the succubus with a different
/// pronoun.
///
/// SRD 5.2 prints them as two stat blocks with the same ability scores,
/// the same AC, five hit points between them, and completely different
/// jobs. (Those five were worth finding: the bestiary's Succubus had
/// been carrying the Incubus's 12d8+12 for as long as the two shared
/// one entry, and separating them put the 13d8+13 back.)
/// The Succubus is a *setup* creature: it charms, then kisses the thing
/// it charmed, and its damage is contingent on its control landing
/// first. The Incubus does not set anything up. It walks in, touches
/// you twice for fifteen psychic each, and when you are nearly down it
/// spends a bonus action putting you to sleep for an hour.
///
/// Action lanes:
/// - **double restless touch** — two 3d6 psychic swings, made with
///   Charisma. RAW's +7 against CHA 20 and a +2 proficiency bonus is
///   the Charisma modifier; the incubus's Strength is 8, so this is a
///   creature that literally hurts you by being charming at you.
/// - **nightmare** (Recharge 6) — a **bonus action**: WIS DC 15 at 60
///   ft, and on a failure, if the target has 20 hit points or fewer, it
///   is Unconscious for an hour. A finisher rather than an opener — an
///   incubus cannot lead with it, and a party that keeps everyone above
///   twenty never sees it.
///
/// The pairing is what makes it dangerous. Thirty psychic a round is
/// most of what gets somebody under twenty hit points, and the bonus
/// action costs the incubus nothing it was going to spend, so the turn
/// that brings you low is the same turn that puts you out.
///
/// Defensive envelope: resistant to cold, fire, poison and psychic —
/// the last of which is the fiend resisting its own damage type, and
/// the reason two incubi are a much worse encounter than one. Fly 60,
/// darkvision 60, telepathy (no combat surface).
///
/// **Succubus Form** — "When the incubus finishes a Long Rest, it can
/// shape-shift into a Succubus, using that stat block instead of this
/// one" — is not modeled. It is a between-adventures clause: the
/// transformation costs a long rest, so within any one encounter the
/// incubus is whichever of the two it walked in as, and the engine
/// already has both stat blocks for a GM to choose between.
///
/// Stat shape: AC 15, 66 HP (12d8+12), STR 8 / DEX 17 / CON 13 /
/// INT 15 / WIS 12 / CHA 20. Speed 30, fly 60. Skills: Deception,
/// Insight, Perception, Persuasion, Stealth. CR 4.
pub static INCUBUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*INCUBUS_MULTI);
    actions.push(&INCUBUS_RESTLESS_TOUCH);
    actions.push(&*INCUBUS_NIGHTMARE);
    CreatureTemplate {
        name: "Incubus",
        // 'σ' is the shadow demon's and 'ς' the succubus's; the incubus
        // takes 'φ' — the same curved fiend band, and a shape that is
        // its own.
        glyph: 'φ',
        ac: 15,
        // 12d8+12 = 66 average per SRD 5.2 (CR 4).
        hitpoints: "12d8+12".parse().unwrap(),
        speed: 30.,
        fly_speed: 60.,
        strength: 8,
        dexterity: 17,
        constitution: 13,
        intelligence: 15,
        wisdom: 12,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal, Language::Common, Language::Infernal]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        skills: HashSet::from([
            Skill::Deception,
            Skill::Insight,
            Skill::Perception,
            Skill::Persuasion,
            Skill::Stealth,
        ]),
        // The fourth of these is the interesting one: an incubus
        // resists the damage type it deals, which is what makes a pair
        // of them so much worse than one of them.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Resistance),
            (DamageType::Psychic, DamageModifier::Resistance),
        ]),
        // Recharge 6 on the nightmare — the stingiest recharge in the
        // engine, which is what a once-a-fight finisher should be.
        recharge_abilities: vec![("nightmare", 6)],
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
            &INCUBUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn incubus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        assert!(a.find_action("double restless touch").is_some());
        assert!(a.find_action("nightmare").is_some());
    }

    /// The incubus hits with Charisma, which is RAW and is the joke.
    ///
    /// Pinned because it looks like a typo. A fiend whose attack rolls
    /// read off the same ability as its save DC is unusual enough that
    /// a future "monster weapons use STR or DEX" tidy-up would fix it
    /// without asking — and would turn a +7 touch into a −1 one, on a
    /// creature whose entire offence is that touch.
    #[test]
    fn the_incubus_hurts_you_by_being_charming_at_you() {
        use crate::engine::types::AbilityScoreType;
        let a = make();
        assert_eq!(
            INCUBUS_RESTLESS_TOUCH.attack_ability,
            AbilityScoreType::Charisma
        );
        assert!(
            a.ability_modifier(AbilityScoreType::Charisma)
                > a.ability_modifier(AbilityScoreType::Strength),
            "an incubus is better at charm than at lifting"
        );
    }

    /// It resists the damage it deals.
    ///
    /// Which is why two incubi are so much worse than one: neither can
    /// be caught in the other's crossfire, and nothing about the pairing
    /// costs either of them anything.
    #[test]
    fn an_incubus_is_not_troubled_by_another_incubus() {
        let a = make();
        assert!(a.is_resistant_to(DamageType::Psychic));
    }
}
