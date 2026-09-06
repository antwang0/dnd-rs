use crate::actions::class_features::{NIMBLE_DISENGAGE, NIMBLE_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Goblin Warrior — CR ¼ small goblinoid, and the bestiary's clearest
/// lesson in action economy.
///
/// Action: scimitar, to close in and slash. Bonus: a shortbow ping, or
/// — RAW's **Nimble Escape** — a Disengage or a Hide. Three ways to
/// spend one bonus action is more choice than most creatures at this
/// rung have any of, and the goblin is what it is because of the third:
/// it steps into reach, cuts, and leaves without provoking, which no
/// amount of damage on the scimitar would have produced.
pub static GOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    // RAW **Nimble Escape**: "the goblin takes the Disengage or Hide
    // action" as a Bonus Action. Two entries because RAW's "or" is a
    // choice the creature makes each turn.
    actions.push(&*NIMBLE_DISENGAGE);
    actions.push(&*NIMBLE_HIDE);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Goblin",
        glyph: 'G',
        ac: 15,
        hitpoints: "3d6".parse().unwrap(),
        strength: 8,
        dexterity: 15,
        constitution: 10,
        intelligence: 10,
        wisdom: 8,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});

/// Goblin Minion — CR ⅛ small goblinoid. The goblin the book hands out
/// six at a time.
///
/// SRD 5.2 splits the goblin into two stat blocks and the difference
/// between them is almost entirely armour: the Warrior above wears a
/// shield behind AC 15, and the Minion has a knife and a leather
/// jerkin. Ten hit points against seven, and a d4 against a d6. That is
/// a smaller gap than the CR band suggests, and it is the point — a
/// minion is not a weaker goblin, it is the same goblin without the
/// kit, which is why six of them are a fight and one of them is
/// scenery.
///
/// **Nimble Escape** rides here as it does on the Warrior and the Boss:
/// a bonus-action Disengage or Hide, which on a creature with seven hit
/// points is most of what it has. A minion that stabs and then vanishes
/// is a minion the party has to spend a turn finding.
pub static GOBLIN_MINION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // RAW **Nimble Escape**: "the goblin takes the Disengage or Hide
    // action" as a Bonus Action. Two entries because RAW's "or" is a
    // choice the creature makes each turn.
    actions.push(&*NIMBLE_DISENGAGE);
    actions.push(&*NIMBLE_HIDE);
    CreatureTemplate {
        name: "Goblin Minion",
        // 'g' (lowercase) beside the Warrior's 'G' — the same
        // silhouette one rung down, which is what it is.
        glyph: 'g',
        ac: 12,
        // 2d6 = 7 average per SRD 5.2 (CR ⅛).
        hitpoints: "2d6".parse().unwrap(),
        speed: 30.,
        strength: 8,
        dexterity: 15,
        constitution: 10,
        intelligence: 10,
        wisdom: 8,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.125,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Stealth]),
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

    /// Every goblin on the ladder has both halves of Nimble Escape.
    ///
    /// Both, and not one: RAW's clause is "the Disengage **or** Hide
    /// action", which is a choice the goblin makes each turn, and a
    /// goblin that could only vanish would be a different creature from
    /// one that could only leave. Swept over all three so a fourth
    /// goblin cannot arrive without it.
    #[test]
    fn every_goblin_escapes_nimbly() {
        use crate::actors::creatures::goblin_bosses::GOBLIN_BOSS_TEMPLATE;
        for t in [
            &*GOBLIN_MINION_TEMPLATE,
            &*GOBLIN_TEMPLATE,
            &*GOBLIN_BOSS_TEMPLATE,
        ] {
            let a = make(t);
            assert!(
                a.find_action("nimble disengage").is_some(),
                "{} should be able to leave",
                t.name
            );
            assert!(
                a.find_action("nimble hide").is_some(),
                "{} should be able to vanish",
                t.name
            );
        }
    }

    #[test]
    fn goblin_minion_template_shape() {
        let a = make(&GOBLIN_MINION_TEMPLATE);
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Small);
        assert!(a.find_action("dagger").is_some());
    }

    /// The two goblins are the same creature at two kit levels, and the
    /// gap between them is where it should be.
    ///
    /// Worth pinning because the temptation with a minion tier is to
    /// make it a *worse creature* — drop its Dexterity, take away its
    /// stealth — and RAW does none of that. Every ability score is
    /// identical; the Warrior is simply better equipped.
    #[test]
    fn a_minion_is_a_warrior_without_the_kit() {
        use crate::engine::types::AbilityScoreType::*;
        let minion = make(&GOBLIN_MINION_TEMPLATE);
        let warrior = make(&GOBLIN_TEMPLATE);
        for ability in [Strength, Dexterity, Constitution, Intelligence, Wisdom, Charisma] {
            assert_eq!(
                minion.ability_score(ability),
                warrior.ability_score(ability),
                "{:?} should match",
                ability
            );
        }
        assert!(minion.armor_class() < warrior.armor_class());
        assert!(minion.max_hitpoints() < warrior.max_hitpoints());
    }
}
