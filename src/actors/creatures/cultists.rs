use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CULTIST_SCIMITAR;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cultist — CR ⅛ medium humanoid. The mob. One scimitar, nine hit
/// points, and a conviction that makes the party's control spells worth
/// rather less than they cost.
///
/// **Dark Devotion** is the whole stat block: "the cultist has advantage
/// on saving throws against being charmed or frightened." Both halves
/// land on flags the engine already keeps for exactly this shape —
/// `has_fey_ancestry` is the charm half and `has_brave` is the fear
/// half, and neither is named after an elf or a bear anywhere the
/// player can see. Which is the point of them being flags rather than
/// racial features: the mechanic is "advantage on this save", and three
/// unrelated stat blocks arrive at it by three unrelated stories.
///
/// It is also the reason a cultist mob is a different fight from a
/// bandit mob at the same CR. Fear is the cheapest way to break a line
/// of low-CR bodies, and this is the line it does not break.
///
/// Stat shape per the SRD NPC appendix: AC 12 (leather), 9 HP (2d8),
/// STR 11 / DEX 12 / CON 10 / INT 10 / WIS 11 / CHA 10. Speed 30.
/// Skills: Deception, Religion. CR ⅛.
pub static CULTIST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CULTIST_SCIMITAR);
    CreatureTemplate {
        name: "Cultist",
        // 'c' (lowercase) — shared with the Acolyte, which is the
        // creature it most often shares a board with and the one whose
        // silhouette it is meant to be confused with at a glance.
        glyph: 'c',
        ac: 12,
        // 2d8 ≈ 9 average per the SRD NPC appendix (CR ⅛).
        hitpoints: "2d8".parse().unwrap(),
        speed: 30.,
        strength: 11,
        dexterity: 12,
        constitution: 10,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        skills: HashSet::from([Skill::Deception, Skill::Religion]),
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Dark Devotion, both halves. See the type docs above for why
        // the charm half rides a flag named after elves.
        has_fey_ancestry: true,
        has_brave: true,
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
            &CULTIST_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn cultist_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("cultist scimitar").is_some());
    }

    /// Dark Devotion is two advantages and not one. Asserting both
    /// matters because the two flags are set independently and a stat
    /// block that kept only the fear half would still read as "has Dark
    /// Devotion" to anybody skimming the literal.
    #[test]
    fn dark_devotion_covers_charm_as_well_as_fear() {
        let a = make();
        assert!(a.has_fey_ancestry());
        assert!(a.has_brave());
    }
}
