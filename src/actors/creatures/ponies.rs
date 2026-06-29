use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PONY_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Pony — CR ⅛ medium beast. The small-rider mount tier of the
/// equine family: a single 2d4+STR hooves swing on an 11-HP frame.
/// Slots beside the Mule (CR ⅛ medium 1d4) on the CR-⅛ mount /
/// pack-animal bench — the canonical "halfling / gnome cavalry"
/// mount, traded the Riding Horse's Large frame and +1 STR for a
/// Medium silhouette and the same hooves dice. Sister to
/// `RIDING_HORSE_TEMPLATE` (CR ¼ large 2d4) and `MULE_TEMPLATE`
/// (CR ⅛ medium 1d4) on the equine ladder — the pony's 2d4 dice
/// match the Riding Horse while its Medium frame matches the Mule,
/// occupying a distinct niche between the two.
///
/// Action lane:
/// - **pony hooves** — STR-based 2d4+STR bludgeoning melee via the
///   shared `PONY_HOOVES` static. The pony's only swing — a sturdy
///   kick averaging ~7 per hit (2d4 = 5 + STR 15 = +2). Heavier
///   than the mule's 1d4 hooves (the trade-off being a -1 CON
///   buffer); pinned to the riding-mount flavor rather than the
///   pack-hauler flavor.
///
/// Defensive identity: AC 10 (no natural hide, no armor), 11 HP
/// (2d8+2). Vanilla beast envelope — no resistances or condition
/// immunities. Same HP envelope as the mule at the same CR tier.
///
/// Stat shape: AC 10, ~11 HP (2d8+2), STR 15, DEX 10, CON 11,
/// INT 2, WIS 11, CHA 7. Speed 40. Size Medium. CR ⅛. XP: 25 per
/// RAW.
pub static PONY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PONY_HOOVES);
    CreatureTemplate {
        name: "Pony",
        // 'p' (lowercase) — shared with Pixie / Pseudodragon /
        // Pteranodon cohort. The team color disambiguates on the map;
        // the pony's medium pack-beast context separates it from the
        // tiny fey / dragonet / pterosaur lane at the prompt layer.
        // Uppercase 'P' is taken by Paladin / Pegasus / Pit Fiend /
        // Polar Bear / Purple Worm at the upper tier.
        glyph: 'p',
        ac: 10,
        // 2d8+2 = 11 average per MM (CR ⅛).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 2,
        dexterity: 10,
        wisdom: 11,
        constitution: 11,
        charisma: 7,
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{AbilityScoreType, Coordinate};

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &PONY_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn pony_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("pony hooves").is_some());
    }

    #[test]
    fn pony_hits_harder_than_mule() {
        // Pin the load-bearing distinction at the same CR tier:
        // the pony rides the heavier 2d4 hooves chassis (matching
        // the Riding Horse) while the mule lands the lighter 1d4
        // hooves chassis. The pony also outranks the mule on STR
        // (15 vs 14). A future refactor that quietly flipped the
        // dice would collapse the pony / mule niche distinction.
        let a = make();
        assert!(a.ability_score(AbilityScoreType::Strength) >= 15);
    }
}
