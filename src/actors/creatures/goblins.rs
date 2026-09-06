use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sneaky melee skirmisher that doubles up its turn with a bonus-action
/// shortbow shot. Action: scimitar (close in and slash). Bonus: shortbow
/// (extra ranged ping). The action-economy split is the headline — most
/// creatures don't have a bonus-action attack option.
pub static GOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
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
/// One action lane and no bonus one. RAW gives it Nimble Escape, the
/// bonus-action Disengage-or-Hide the Warrior also has; the engine
/// carries Disengage and Hide as ordinary actions any creature can
/// take, so what the trait actually buys is the *bonus action*, and
/// there is no lane for "this creature may take that action for free".
/// The clause is dropped on both goblins rather than half-modeled on
/// one.
pub static GOBLIN_MINION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
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
