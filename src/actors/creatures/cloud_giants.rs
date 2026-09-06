use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CLOUD_GIANT_MORNINGSTAR, CLOUD_GIANT_MULTI, CLOUD_GIANT_ROCK,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cloud Giant — CR 9 huge giant. Slots between Fire Giant (CR 9) and
/// Storm Giant (CR 13) on the giant ladder: same 3d8 melee die and 4d10
/// thrown rock as the Fire Giant, but with the heavier STR-based dice
/// and a wider proficient-save spread (CON / INT / WIS / CHA per MM).
/// The Cloud Giant's RAW innate spellcasting (fog cloud, gust of wind,
/// telekinesis at higher tiers) is omitted — the engine doesn't yet
/// surface monster spellcasting picks, so the morningstar + rock + Multi
/// chassis carries the threat profile.
///
/// Cloud Giants are smarter and more charismatic than their lower-CR
/// kin — INT 12 / WIS 16 / CHA 16 reads as the boss-tier giant statline.
/// Keen Smell (advantage on smell-based Perception) collapses to a
/// higher base WIS rather than a dedicated trait flag. Saves: CON,
/// INT, WIS, CHA per MM.
pub static CLOUD_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLOUD_GIANT_MORNINGSTAR);
    actions.push(&CLOUD_GIANT_ROCK);
    actions.push(&*CLOUD_GIANT_MULTI);
    CreatureTemplate {
        name: "Cloud Giant",
        // 'U' — unused in the giant family pool ('G' frost, 'J' hill,
        // 'g' stone, 'L' storm, 'F' fire, 'Y' cyclops, 'O' oni). 'U' for
        // the misty cloud-tier giant kin; mnemonic for "Upper-tier".
        glyph: 'U',
        ac: 14,
        // 16d12+96 ≈ 200 average per MM (CR 9).
        hitpoints: "16d12+96".parse().unwrap(),
        speed: 40.,
        // RAW speed line: Speed 40 ft., Fly 20 ft. (hover). Half the
        // walk, and the reason a cloud giant fights from a ledge.
        fly_speed: 20.,
        hovers: true,
        strength: 27,
        intelligence: 12,
        dexterity: 10,
        wisdom: 16,
        constitution: 22,
        charisma: 16,
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 9.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        // Cloud Giant proficient saves: CON, INT, WIS, CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_extra_attack: true,
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
    fn cloud_giant_has_heavy_hp_pool() {
        let a = ActorInstance::from_creature_template(
            &CLOUD_GIANT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // 16d12+96 averages 200 HP; the CR-9 chassis should be well above
        // the lower-tier giants.
        assert!(a.max_hitpoints() >= 130);
        assert_eq!(a.cr(), 9.0);
    }

    #[test]
    fn cloud_giant_has_morningstar_and_rock() {
        let a = ActorInstance::from_creature_template(
            &CLOUD_GIANT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("cloud morningstar").is_some());
        assert!(a.find_action("cloud rock").is_some());
        assert!(a.find_action("cloud giant multiattack").is_some());
    }

    #[test]
    fn cloud_giant_has_extra_attack() {
        let a = ActorInstance::from_creature_template(
            &CLOUD_GIANT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_extra_attack());
    }
}
