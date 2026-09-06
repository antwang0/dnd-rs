use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BUGBEAR_MORNINGSTAR, BUGBEAR_STALKER_JAVELIN, BUGBEAR_STALKER_MORNINGSTAR,
    BUGBEAR_STALKER_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bugbear — CR 1 goblinoid brute. Morningstar carries a Surprise
/// Attack rider that adds 2d6 extra piercing damage on the first round
/// of combat, capturing the "out of the dark" alpha-strike fantasy
/// without modeling stealth approach. Medium-sized but stronger than
/// a goblin / hobgoblin; lacks ranged options.
pub static BUGBEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BUGBEAR_MORNINGSTAR);
    CreatureTemplate {
        name: "Bugbear",
        // 'B' for bugbear — distinct from 'b' (boots-of-striding glyph).
        glyph: 'B',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 8,
        dexterity: 14,
        wisdom: 11,
        constitution: 13,
        charisma: 9,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});

/// Bugbear Stalker — CR 3 goblinoid hunter. The rung above the bugbear,
/// and a different creature rather than a bigger one.
///
/// Where the CR-1 bugbear is an ambusher — one club, one alpha strike,
/// nothing to do on round two — the stalker is a soldier: sixty-five
/// hit points behind a chain shirt, two morningstar swings a turn at
/// ten feet of reach, and six javelins for the rounds it spends
/// closing. It is the goblinoid ladder's answer to the question the
/// bugbear cannot answer, which is "what happens after the surprise".
///
/// Action lanes:
/// - **bugbear stalker multiattack** — two morningstar swings, reach 2.
/// - **stalker morningstar** — the single swing, for a turn spent
///   moving.
/// - **stalker javelin** — 3d6 thrown, and the reason a stalker at
///   range is still a threat.
///
/// Two RAW clauses are cut, both for want of a lane rather than out of
/// taste. **Quick Grapple** is a bonus-action DEX-save-or-Grappled, and
/// the engine's Grapple is an Action-cost contest; a bonus-action save
/// version would be a second grapple implementation. **Abduct** — "the
/// bugbear needn't spend extra movement to move a creature it is
/// grappling" — waives a cost the engine does not charge, so it is
/// already true. The morningstar's advantage-against-a-grappled-target
/// clause goes with Quick Grapple; see `BUGBEAR_STALKER_MORNINGSTAR`.
///
/// Stat shape: AC 15 (chain shirt), 65 HP (10d8+20), STR 17 / DEX 14 /
/// CON 14 / INT 11 / WIS 12 / CHA 11. Speed 30. Darkvision 60. Skills:
/// Stealth, Survival. CR 3.
pub static BUGBEAR_STALKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BUGBEAR_STALKER_MULTI);
    actions.push(&BUGBEAR_STALKER_MORNINGSTAR);
    actions.push(&BUGBEAR_STALKER_JAVELIN);
    CreatureTemplate {
        name: "Bugbear Stalker",
        // 'K' — 'B' is the bugbear's and 'b' is taken; the stalker gets
        // the hard consonant out of the middle of the word, the same
        // trick the dragon ladder uses when a colour's initial is gone.
        glyph: 'K',
        ac: 15,
        // 10d8+20 = 65 average per SRD 5.2 (CR 3).
        hitpoints: "10d8+20".parse().unwrap(),
        speed: 30.,
        strength: 17,
        dexterity: 14,
        constitution: 14,
        intelligence: 11,
        wisdom: 12,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Stealth, Skill::Survival]),
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
    fn bugbear_stalker_template_shape() {
        let a = make(&BUGBEAR_STALKER_TEMPLATE);
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.size(), Size::Medium);
        assert!(a.find_action("bugbear stalker multiattack").is_some());
        assert!(a.find_action("stalker javelin").is_some());
    }

    /// The stalker's reach is the thing that makes it a soldier rather
    /// than a bigger ambusher: ten feet of morningstar means it
    /// threatens the tile a party would otherwise stand on to hold the
    /// line against the CR-1 bugbear's five.
    #[test]
    fn the_stalker_reaches_a_tile_further_than_the_bugbear_does() {
        let stalker = make(&BUGBEAR_STALKER_TEMPLATE);
        let bugbear = make(&BUGBEAR_TEMPLATE);
        let reach = |a: &ActorInstance, name: &str| {
            a.find_action(name).and_then(|act| act.reach_tiles()).unwrap()
        };
        assert!(
            reach(&stalker, "stalker morningstar") > reach(&bugbear, "morningstar"),
            "the stalker's morningstar is the long one"
        );
    }
}
