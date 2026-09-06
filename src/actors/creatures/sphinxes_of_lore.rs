use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    MIND_RENDING_ROAR, SPHINX_OF_LORE_CLAW, SPHINX_OF_LORE_MULTI,
};
use crate::actions::spells::{DISPEL_MAGIC, REMOVE_CURSE};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sphinx of Lore — CR 11 large celestial. The riddler.
///
/// SRD 5.2 splits the old gynosphinx / androsphinx pair into a Sphinx of
/// Lore and a Sphinx of Valor, and the split is along exactly the line
/// the names suggest: the one on this page knows things and the other
/// one hits things. What that means at the table is that the Sphinx of
/// Lore is the boss you are supposed to *lose to socially* — it asks a
/// question, and the fight is what happens when the party gets it
/// wrong.
///
/// Action lanes:
/// - **triple sphinx claw** — three 3d6+STR slashing swings. Forty-two
///   a round on a clean set, which is respectable and is not the reason
///   anybody remembers this stat block.
/// - **mind-rending roar** (Recharge 5–6) — WIS DC 16 against 10d6
///   psychic *and* Incapacitated until the sphinx's next turn, for
///   every enemy that hears it. Thirty-five damage each and then the
///   party loses its Actions, into three claws. This is the fight.
/// - **dispel magic** / **remove curse** — two of RAW's 1/day list that
///   have somewhere to land in a combat round. The rest of it (Detect
///   Magic, Identify, Legend Lore, Locate Object, Plane Shift, Tongues)
///   is the library half of the stat block, and a library has no
///   initiative count.
///
/// Defensive envelope: necrotic and radiant resistance, psychic
/// immunity — which is the joke, since the roar is psychic and the
/// sphinx is the one creature it could never be turned against —
/// immunity to Charmed and Frightened, truesight 120 ft, and three
/// Legendary Resistances. A party's usual answer to a boss is to take
/// its turn away; the sphinx has four separate reasons that will not
/// work.
///
/// Legendary actions (3/round, via `SPHINX_OF_LORE_LEGENDARY`):
/// **Arcane Prowl** — thirty feet and a claw for one point — and
/// **Weight of Years**, a CON DC 16 that hands out a level of
/// exhaustion. The second is the one to watch: exhaustion does not wear
/// off inside a fight, so a sphinx that lands it three rounds running
/// has permanently downgraded somebody.
///
/// **Inscrutable** — "No magic can observe the sphinx remotely or
/// detect its thoughts without its permission" — is not modeled,
/// because nothing in the combat engine reads a creature's thoughts or
/// scries it. Its second half, disadvantage on Insight checks to read
/// the sphinx, is a conversation rule, and the conversation is the part
/// of this encounter that happens before initiative.
///
/// Stat shape: AC 17, 170 HP (20d10+60), STR 18 / DEX 15 / CON 16 /
/// INT 18 / WIS 18 / CHA 18. Speed 40, fly 60. Skills: Arcana, History,
/// Perception, Religion. CR 11.
pub static SPHINX_OF_LORE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPHINX_OF_LORE_MULTI);
    actions.push(&SPHINX_OF_LORE_CLAW);
    actions.push(&MIND_RENDING_ROAR);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*REMOVE_CURSE);
    CreatureTemplate {
        name: "Sphinx of Lore",
        // 'Q' — the sphinx band's 'S' belongs to the Androsphinx and
        // the Sphinx of Wonder, and this is the one that asks the
        // question.
        glyph: 'Q',
        ac: 17,
        // 20d10+60 = 170 average per SRD 5.2 (CR 11).
        hitpoints: "20d10+60".parse().unwrap(),
        speed: 40.,
        fly_speed: 60.,
        strength: 18,
        dexterity: 15,
        constitution: 16,
        intelligence: 18,
        wisdom: 18,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        // RAW languages: Celestial, Common.
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        skills: HashSet::from([
            Skill::Arcana,
            Skill::History,
            Skill::Perception,
            Skill::Religion,
        ]),
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Resistance),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        // RAW prints every save at the bare ability modifier — the
        // sphinx is proficient in none of them, which is unusual for a
        // CR-11 boss and is what the three Legendary Resistances are
        // standing in for.
        legendary_resistances: 3,
        legendary_actions_per_round: 3,
        legendary_actions: crate::engine::legendary_actions::SPHINX_OF_LORE_LEGENDARY,
        // Recharge 5–6 on the roar, through the shared pool every other
        // recharge ability in the engine reads.
        recharge_abilities: vec![("breath_weapon", 5)],
        // Enough of the upper rungs for the two 1/day spells that have
        // a combat surface.
        spell_slots_by_level: vec![0, 0, 2, 0, 0],
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
            &SPHINX_OF_LORE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn sphinx_of_lore_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert!(a.find_action("triple sphinx claw").is_some());
        assert!(a.find_action("mind-rending roar").is_some());
    }

    /// The roar is psychic and the sphinx is immune to psychic, which
    /// is not a coincidence and is the one thing a party might try.
    ///
    /// Worth pinning because the immunity and the roar's damage type
    /// live in two different files, and a future "celestials resist
    /// psychic rather than ignoring it" pass would quietly make the
    /// sphinx vulnerable to its own signature via any effect that turns
    /// an ability back on its user.
    #[test]
    fn the_roar_cannot_be_turned_on_the_thing_that_made_it() {
        use crate::actions::action_template::Action;
        let a = make();
        assert!(a.is_immune_to(DamageType::Psychic));
        assert_eq!(MIND_RENDING_ROAR.damage_types(), vec![DamageType::Psychic]);
    }

    /// Both halves of the roar hang off one save, and the condition
    /// half is what makes it a boss ability rather than a big number.
    #[test]
    fn the_roar_takes_the_turn_as_well_as_the_hit_points() {
        assert_eq!(
            MIND_RENDING_ROAR.condition.map(|(c, _)| c),
            Some(Condition::Incapacitated)
        );
        // RAW scopes the emanation to enemies. A sphinx that stunned
        // its own guardians would be reading its own stat block wrong.
        assert!(MIND_RENDING_ROAR.enemies_only);
    }
}
