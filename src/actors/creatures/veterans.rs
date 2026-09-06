use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, SPEAR, THROWN_SPEAR, VETERAN_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Veteran — CR 3 humanoid soldier. Plate-armored melee with a 2x
/// longsword multiattack plus a heavy crossbow for ranged finishers.
/// Higher AC than the berserker but lower HP — the trade-off
/// between staying power and stack.
pub static VETERAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VETERAN_MULTI);
    actions.push(&HEAVY_CROSSBOW);
    CreatureTemplate {
        name: "Veteran",
        // 'v' — distinct from existing 'V' (Vampire Spawn).
        glyph: 'v',
        ac: 17,
        // 10d8+20 = 65 average per SRD 5.2 (CR 3).
        hitpoints: "10d8+20".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 13,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        has_extra_attack: true,
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        ..CreatureTemplate::defaults()
    }
});

/// Warrior Infantry — CR ⅛ humanoid. The soldier the book hands out by
/// the dozen, and the bottom rung of the same ladder the Veteran tops.
///
/// SRD 5.2 files three "Warrior" stat blocks — Infantry, Veteran, and
/// the Warrior Veteran the bestiary already carries under its older
/// name — and the Infantry is the one with nothing on it: a chain
/// shirt, a spear, nine hit points, and **Pack Tactics**.
///
/// The Pack Tactics is the whole design and the reason this is not just
/// a Guard with a different name. A guard is a person doing a job; a
/// squad of infantry is a *formation*, and RAW prices it that way — six
/// of them adjacent to two targets all swing at advantage, which turns
/// a CR-⅛ mook into something a mid-level party has to actually break
/// up rather than walk through.
///
/// Action lanes: **spear** and **thrown spear**, the shared statics,
/// which is exactly RAW's "Melee or Ranged Attack Roll: +3, reach 5 ft.
/// or range 20/60 ft."
///
/// Stat shape: AC 13, 9 HP (2d8), STR 13 / DEX 11 / CON 11 / INT 8 /
/// WIS 11 / CHA 8. Speed 30. CR ⅛.
pub static WARRIOR_INFANTRY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPEAR);
    actions.push(&THROWN_SPEAR);
    CreatureTemplate {
        name: "Warrior Infantry",
        // 'i' — the infantry mark, lowercase against the Veteran's 'v'
        // for the same reason the goblin minion is lowercase beside the
        // goblin: the same silhouette, one rung down.
        glyph: 'i',
        ac: 13,
        // 2d8 = 9 average per SRD 5.2 (CR ⅛).
        hitpoints: "2d8".parse().unwrap(),
        speed: 30.,
        strength: 13,
        dexterity: 11,
        constitution: 11,
        intelligence: 8,
        wisdom: 11,
        charisma: 8,
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // RAW: "The warrior has Advantage on an attack roll against a
        // creature if at least one of the warrior's allies is within 5
        // feet of the creature and the ally doesn't have the
        // Incapacitated condition." The shared flag, read at the
        // `compute_attack_mode` chokepoint.
        has_pack_tactics: true,
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
    fn warrior_infantry_template_shape() {
        let a = make(&WARRIOR_INFANTRY_TEMPLATE);
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("spear").is_some());
        assert!(a.find_action("thrown spear").is_some());
    }

    /// The formation is the stat block.
    ///
    /// Nine hit points and a d6 is a creature nobody would print twice.
    /// Pack Tactics is what makes six of them a fight — and it is the
    /// single flag that separates the infantry from the guard, so a
    /// refactor that dropped it would leave two identical mooks and no
    /// reason for either.
    #[test]
    fn infantry_fight_in_formation_and_that_is_the_whole_of_it() {
        assert!(make(&WARRIOR_INFANTRY_TEMPLATE).has_pack_tactics());
    }
}
