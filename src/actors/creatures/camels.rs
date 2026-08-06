use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CAMEL_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Camel — CR ⅛ large beast. The "desert pack animal" tier of large
/// herbivore: a tall, slow-moving herd beast with a single nipping bite
/// for incidental damage. Slots beside the Mastiff (CR ⅛ medium, trip-
/// bite trained hound), Giant Crab (CR ⅛ small, pincer ambusher), and
/// Stirge (CR ⅛ tiny, blood-drainer) on the CR-⅛ ambient-creature
/// bench. The camel exists primarily as desert-encounter set dressing
/// — a caravan's pack animal, not a credible combat threat. Pairs
/// naturally with the Bandit / Tribal Warrior / Scout cohort for the
/// canonical "merchant caravan ambushed in the dunes" encounter shape.
///
/// Action lane:
/// - **camel bite** — STR-based flat-1d4 bludgeoning melee via the
///   shared `CAMEL_BITE` static. RAW: "+5 to hit, reach 5 ft, one
///   creature. Hit: 2 (1d4) bludgeoning damage." Note the RAW
///   omission of STR-to-damage even though the camel sports STR 16
///   (+3) — RAW's MM stat block lists 1d4 flat, not 1d4+3. We follow
///   the MM data exactly via the `damage_ability: None` field on the
///   `CAMEL_BITE` static — distinguishing the camel from the standard
///   "STR-to-hit STR-to-damage" envelope of the rest of the beast
///   bench. The flat dice anchor the camel's "incidental nip" flavor.
///
/// Defensive identity: AC 9 (large + low DEX, no natural armor), 15
/// HP (2d10+4). Vanilla beast envelope — no resistances or condition
/// immunities. The camel takes a couple of solid hits to drop; its
/// role is "pack animal," not combatant.
///
/// Stat shape: AC 9, ~15 HP (2d10+4), STR 16, DEX 8, CON 14, INT 2,
/// WIS 8, CHA 5. Speed 50 — RAW's "fast trot" envelope; faster than
/// the Mule / Mastiff (40) but slower than the Riding Horse (60).
/// Size Large. CR ⅛. XP: 25 per RAW.
pub static CAMEL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CAMEL_BITE);
    CreatureTemplate {
        name: "Camel",
        // 'C' (uppercase) — shared with Centaur / Cleric / Couatl /
        // Crocodile cohort. The team color disambiguates on the map and
        // the beast / CR-⅛ context separates the camel from the
        // medium-CR humanoid / monstrosity cohort sharing the glyph.
        // Lowercase 'c' is taken by Cat / Cockatrice; uppercase 'C'
        // reads as "tall four-legged frame" at the small UI scale.
        glyph: 'C',
        ac: 9,
        // 2d10+4 = 15 average per MM (CR ⅛).
        hitpoints: "2d10+4".parse().unwrap(),
        // Speed 50 — the camel's signature trot. Faster than the Mule
        // / Mastiff and slower than the Riding Horse.
        speed: 50.,
        strength: 16,
        intelligence: 2,
        dexterity: 8,
        wisdom: 8,
        constitution: 14,
        charisma: 5,
        cr: 0.125,
        size: Size::Large,
        // 5e Mounted Combat: PHB's mount table — the desert entry.
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
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &CAMEL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn camel_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("camel bite").is_some());
    }
}
