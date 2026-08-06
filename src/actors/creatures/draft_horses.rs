use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RIDING_HORSE_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Draft Horse — CR ¼ large beast. The "heavy hauler" tier of
/// equine: a slower (speed 40) but stronger (STR 18) large frame
/// with the same 2d4 hooves chassis as the Riding Horse. Slots
/// beside the Riding Horse (CR ¼, fast civilian mount) and beneath
/// the Warhorse (CR ½, trained battle mount) on the equine ladder
/// — the canonical "plow horse / wagon-puller" beast, traded a
/// 20-ft speed loss vs the Riding Horse for a bigger STR mod that
/// lands a meaningfully harder kick if forced into combat.
///
/// Action lane:
/// - **horse hooves** — STR-based 2d4+STR bludgeoning melee via the
///   shared `RIDING_HORSE_HOOVES` static. Same chassis as the
///   Riding Horse; the +1 STR mod difference (+4 here vs +3 on
///   the riding horse) is the per-template scaling at the damage
///   chokepoint. 2d4 averages to 5 + 4 = 9 per swing — the chunky
///   mod nudges the draft horse's defensive bite into "actually
///   dangerous" territory if cornered.
///
/// Defensive identity: AC 10 (no armor, no shield, no natural
/// hide), 19 HP (3d10+3). Vanilla beast envelope — no resistances
/// or condition immunities. Slightly tougher than the riding
/// horse's 13 HP envelope, matching the "heavy / sturdier" flavor.
/// Still dies to two or three solid hits at its CR; the template
/// exists for cargo / farm encounters and as the second floor of
/// the equine CR ladder rather than as a combat threat.
///
/// Stat shape: AC 10, ~19 HP (3d10+3), STR 18, DEX 10, CON 12,
/// INT 2, WIS 11, CHA 7. Speed 40 (heavier / slower than the
/// riding horse). Size Large. CR ¼. XP: 50 per RAW.
pub static DRAFT_HORSE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RIDING_HORSE_HOOVES);
    CreatureTemplate {
        name: "Draft Horse",
        // 'H' (uppercase) — shared with Riding Horse / Warhorse /
        // Hippogriff / Hydra / Hobgoblin cohort. The team color
        // disambiguates on the map; the draft horse usually spawns
        // beside a Commoner / Bandit caravan, not a Knight, so 'H'
        // reads as "heavy hauler mount" in context. The team / size
        // combination separates it from the warhorse on the map
        // without needing a distinct glyph.
        glyph: 'H',
        ac: 10,
        // 3d10+3 = 19 average per MM (CR ¼).
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 2,
        dexterity: 10,
        wisdom: 11,
        constitution: 12,
        charisma: 7,
        cr: 0.25,
        size: Size::Large,
        // 5e Mounted Combat: PHB's mount table. Bred to pull rather than to run, and rideable either way.
        mountable: true,
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
            &DRAFT_HORSE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn draft_horse_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("horse hooves").is_some());
    }

    #[test]
    fn draft_horse_trades_speed_for_strength() {
        // Pin the load-bearing scaling contrast: draft horse is
        // slower (40) and stronger (STR 18) than the riding horse
        // (60 / STR 16). A future refactor that quietly bumped the
        // speed to 60 would flatten the riding-vs-draft distinction
        // and silently collapse the two horse tiers into one stat
        // block.
        let a = make();
        assert!(a.speed() <= 50.0);
        assert!(a.ability_score(AbilityScoreType::Strength) >= 18);
    }
}
