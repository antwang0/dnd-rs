use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RIDING_HORSE_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Riding Horse — CR ¼ large beast. The "civilian mount" tier of
/// equine: a fast (speed 60) large frame with light 2d4 hooves and
/// 13 HP. Slots beneath the Warhorse (CR ½ trained battle mount,
/// 2d6 hooves) and beside the Draft Horse (CR ¼ heavy hauler) on
/// the equine ladder — the canonical "long-distance traveler's
/// mount" beast, faster than the draft horse but lighter on the
/// damage die.
///
/// Action lane:
/// - **horse hooves** — STR-based 2d4+STR bludgeoning melee via the
///   shared `RIDING_HORSE_HOOVES` static. The CR-¼ horse's only
///   defensive swing. 2d4 averages to 5 + STR mod (+3) = 8 per
///   swing — same chassis as the Draft Horse, distinguished only
///   by the STR mod that scales the damage.
///
/// Defensive identity: AC 10 (no armor, no shield, no natural
/// hide), 13 HP (2d10+2). Vanilla beast envelope — no resistances
/// or condition immunities. The riding horse dies to two solid
/// hits; the template exists primarily for travel encounters and
/// as the floor of the equine CR ladder rather than as a combat
/// threat.
///
/// Stat shape: AC 10, ~13 HP (2d10+2), STR 16, DEX 10, CON 12,
/// INT 2, WIS 11, CHA 7. Speed 60 (long-distance travel pace
/// matches the warhorse). Size Large. CR ¼. XP: 50 per RAW.
pub static RIDING_HORSE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RIDING_HORSE_HOOVES);
    CreatureTemplate {
        name: "Riding Horse",
        // 'H' (uppercase) — shared with Warhorse / Hippogriff / Hydra /
        // Hobgoblin cohort. The team color disambiguates on the map;
        // the riding horse will most often spawn beside a civilian
        // Commoner or a Bandit raider, not a Knight, so 'H' reads as
        // "transport mount" in context. The team / size combination
        // separates it from the warhorse on the map without needing
        // a distinct glyph.
        glyph: 'H',
        ac: 10,
        // 2d10+2 = 13 average per MM (CR ¼).
        hitpoints: "2d10+2".parse().unwrap(),
        speed: 60.,
        strength: 16,
        intelligence: 2,
        dexterity: 10,
        wisdom: 11,
        constitution: 12,
        charisma: 7,
        cr: 0.25,
        size: Size::Large,
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
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &RIDING_HORSE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn riding_horse_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("horse hooves").is_some());
    }

    #[test]
    fn riding_horse_is_fast_civilian_mount() {
        // Pin the load-bearing mobility trait: a riding horse's
        // entire identity is "civilian long-distance traveler." Speed
        // 60 matches RAW and outpaces nearly every CR-¼ creature on
        // the bench. A future template-refactor that dropped the
        // speed to the 30 default would erase the mount identity
        // and silently demote the riding horse to "a slow large
        // beast with weak hooves" — same damage profile as a giant
        // rat at a higher CR.
        let a = make();
        assert!(a.speed() >= 60.0);
    }
}
