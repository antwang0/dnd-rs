use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SATYR_RAM, SATYR_SHORTBOW, SATYR_SHORTSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Satyr — CR ½ medium fey. Forty feet of speed, three weapons, and
/// **Magic Resistance** on a body with thirty-one hit points.
///
/// That last combination is the reason the satyr is worth its CR and
/// more. Magic Resistance — advantage on every saving throw against a
/// spell — is a trait the bestiary otherwise reserves for creatures
/// eight CRs higher; on a CR ½ fey it means a party's cheap control
/// spells simply stop working, and the satyr closes forty feet a turn
/// while they try again.
///
/// Action lanes:
/// - **satyr ram** — 2d4+STR bludgeoning. The headbutt.
/// - **satyr shortsword** — 1d6+DEX piercing, light.
/// - **satyr shortbow** — 1d6+DEX piercing at twelve tiles.
///
/// Three weapons on a CR ½ body is unusual and deliberate on RAW's part:
/// the satyr picks whichever of the three the situation wants, which on
/// a forty-foot speed means it is never in the wrong band.
///
/// Stat shape per the SRD: AC 14 (leather armor), 31 HP (7d8), STR 12 /
/// DEX 16 / CON 11 / INT 12 / WIS 10 / CHA 14. Speed 40. Skills:
/// Perception, Performance, Stealth. Languages: Common, Elvish, Sylvan.
/// CR ½.
pub static SATYR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SATYR_RAM);
    actions.push(&SATYR_SHORTSWORD);
    actions.push(&SATYR_SHORTBOW);
    CreatureTemplate {
        name: "Satyr",
        // 'Y' (uppercase) — an unclaimed letter; 'S' and 's' are both
        // deep pools and lowercase 'y' is the Spy's.
        glyph: 'Y',
        ac: 14,
        // 7d8 ≈ 31 average per the SRD (CR ½).
        hitpoints: "7d8".parse().unwrap(),
        speed: 40.,
        strength: 12,
        dexterity: 16,
        constitution: 11,
        intelligence: 12,
        wisdom: 10,
        charisma: 14,
        skills: HashSet::from([Skill::Perception, Skill::Performance, Skill::Stealth]),
        languages: HashSet::from([Language::Common, Language::Elvish, Language::Sylvan]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        has_magic_resistance: true,
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
            &SATYR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn satyr_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.creature_type(), CreatureType::Fey);
        assert!(a.find_action("satyr ram").is_some());
        assert!(a.find_action("satyr shortsword").is_some());
        assert!(a.find_action("satyr shortbow").is_some());
    }

    /// Magic Resistance at CR ½ is the anomaly the stat block is built
    /// around, and the speed is what it buys. Asserted together because
    /// either one alone is an ordinary fey.
    #[test]
    fn the_satyr_shrugs_off_spells_and_closes_the_gap_the_same_turn() {
        let a = make();
        assert!(a.has_magic_resistance());
        assert_eq!(a.speed(), 40.);
    }
}
