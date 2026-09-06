use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    GUARDIAN_NAGA_BITE, GUARDIAN_NAGA_MULTI, GUARDIAN_NAGA_SPITTLE,
};
use crate::actions::spells::{CURE_WOUNDS, FLAME_STRIKE, GEAS, TRUE_SEEING};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Guardian Naga — CR 10 large celestial, and the bestiary's answer to
/// the question of what a *good* monster with 136 hit points looks like.
///
/// It is Lawful Good, it speaks Celestial and Common, and it is on the
/// board because something it was set to guard is behind it. That is
/// the whole design: a guardian naga does not hunt, it does not chase,
/// and it will talk first. What it will not do is move, and everything
/// on the stat block is built for a creature holding a doorway.
///
/// Action lanes:
/// - **double naga bite** — two 2d12+STR piercing bites at reach 10 ft,
///   each with an unconditional 4d10 poison rider. Forty-odd damage a
///   round that asks for no save at all, which is what makes the naga
///   a wall rather than a puzzle.
/// - **poisonous spittle** — CON DC 16 at 60 ft: 7d8 poison, half on a
///   save, Blinded until the naga's next turn on a failure. The reason
///   standing back is not a plan.
/// - **cure wounds** / **flame strike** / **geas** / **true seeing** —
///   RAW's 1/day-each list, minus Clairvoyance, which is a scouting
///   spell with no combat surface. WIS-based, which is both RAW and the
///   engine's single spell ability, so the sheet needs no translation.
///
/// Defensive envelope: AC 18, poison immunity, and immunity to Charmed,
/// Paralyzed, Poisoned and Restrained — the four conditions that would
/// let a party walk past it. A guardian that could be charmed aside is
/// not a guardian, and RAW knows it.
///
/// **Celestial Restoration** — "If the naga dies, it returns to life in
/// 1d6 days and regains all its Hit Points unless Dispel Evil and Good
/// is cast on its remains" — is not modeled and could not be: it
/// resolves days after the encounter ends, and the engine's timeline
/// stops at the end of the fight. It is also the clause that makes the
/// naga worth talking to, since killing it only postpones the problem;
/// that is a table conversation, not a combat rule.
///
/// Stat shape: AC 18, 136 HP (16d10+48), STR 19 / DEX 18 / CON 16 /
/// INT 16 / WIS 19 / CHA 18. Speed 40 (RAW also Climb 40 and Swim 40,
/// neither of which has a lane on a flat board). Skills: Arcana,
/// History, Religion. Darkvision 60. CR 10.
pub static GUARDIAN_NAGA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GUARDIAN_NAGA_MULTI);
    actions.push(&GUARDIAN_NAGA_BITE);
    actions.push(&GUARDIAN_NAGA_SPITTLE);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*FLAME_STRIKE);
    actions.push(&*GEAS);
    actions.push(&*TRUE_SEEING);
    CreatureTemplate {
        name: "Guardian Naga",
        // 'N' — the naga band, shared with the Spirit Naga it is the
        // mirror of. One guards and one corrupts; on a board they are
        // the same long shape in the doorway.
        glyph: 'N',
        ac: 18,
        // 16d10+48 = 136 average per SRD 5.2 (CR 10).
        hitpoints: "16d10+48".parse().unwrap(),
        // RAW speed line: Speed 40 ft., Climb 40 ft., Swim 40 ft. The
        // walking half is the number; a flat board has no use for the
        // other two.
        speed: 40.,
        strength: 19,
        dexterity: 18,
        constitution: 16,
        intelligence: 16,
        wisdom: 19,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 10.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        skills: HashSet::from([Skill::Arcana, Skill::History, Skill::Religion]),
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Immunity)]),
        // The four conditions that would let a party walk past a thing
        // whose entire job is not letting them.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Paralyzed,
            Condition::Poisoned,
            Condition::Restrained,
        ]),
        // RAW: DEX +8, CON +7, INT +7, WIS +8, CHA +8 against a +4
        // proficiency bonus — every save but Strength.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Enough of the top two rungs for RAW's 1/day-each list: Geas
        // and the level-6 versions of Cure Wounds and Flame Strike.
        spell_slots_by_level: vec![0, 0, 0, 0, 2, 2],
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
            &GUARDIAN_NAGA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn guardian_naga_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 10.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert!(a.find_action("double naga bite").is_some());
        assert!(a.find_action("poisonous spittle").is_some());
    }

    /// The four immunities are the stat block's thesis.
    ///
    /// A guardian naga with 136 hit points and two big bites is a wall.
    /// A guardian naga that can be charmed, paralysed, poisoned or
    /// restrained is a wall with a door in it, and the party walks
    /// through. Named individually rather than counted so a future
    /// "celestials get the standard envelope" tidy-up cannot quietly
    /// swap one of them out.
    #[test]
    fn nothing_moves_the_thing_in_the_doorway() {
        let a = make();
        for c in [
            Condition::Charmed,
            Condition::Paralyzed,
            Condition::Poisoned,
            Condition::Restrained,
        ] {
            assert!(
                a.effectively_immune_to_condition(c),
                "a guardian naga that can be {} is not guarding anything",
                c.name()
            );
        }
        assert!(a.is_immune_to(DamageType::Poison));
    }
}
