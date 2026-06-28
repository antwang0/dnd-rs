use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_GOAT_RAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Goat — CR ½ large beast. The "mountain ram" tier of caprid:
/// a Large frame with chunky 2d4 horns and a 19-HP envelope. Slots
/// beside the Boar (CR ¼ medium tusks) and the Warhorse (CR ½ large
/// hooves) on the low-CR herbivore-megafauna bench — the canonical
/// CR-½ headbutting mountain ungulate.
///
/// Action lane:
/// - **giant goat ram** — STR-based 2d4+STR bludgeoning melee via the
///   shared `GIANT_GOAT_RAM` static. The CR-½ goat's only swing.
///   2d4 averages to 5 + STR mod (+3) = 8 per swing — same single-
///   die damage envelope as the Warhorse's hooves, just bludgeoning
///   from a headbutt rather than a stomp.
///
/// **Charge** (RAW: 20 ft straight charge → extra 2d4 + DC-13 STR
/// save-or-Prone) and **Sure-Footed** (advantage on STR/DEX saves vs
/// effects that would knock it prone) are both omitted as scope cuts.
/// The Charge rider is the same straight-line-movement gap that
/// hollows out the Boar / Warhorse / Mammoth charges; the Sure-Footed
/// trait would need a "what save type and which condition is this
/// protecting against" tag the engine doesn't surface on the
/// `roll_save` chokepoint.
///
/// Defensive identity: AC 11 (large + thick hide, no armor), 19 HP
/// (3d10+3). Vanilla beast envelope — no resistances or condition
/// immunities. The goat dies to two solid hits at its CR; threat
/// lives in the chunky ram die rather than survivability.
///
/// Stat shape: AC 11, ~19 HP (3d10+3), STR 17, DEX 11, CON 12,
/// INT 3, WIS 12, CHA 5. Speed 40. Size Large. CR ½. XP: 100 per RAW.
pub static GIANT_GOAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_GOAT_RAM);
    CreatureTemplate {
        name: "Giant Goat",
        // 'G' (uppercase) — shared with Gargoyle / Ghast / Ghoul /
        // Giant Eagle / Gnoll cohort. The team color disambiguates
        // on the map; the goat's beast / herbivore context separates
        // it from the fiend / undead cohort at the prompt layer.
        // Lowercase 'g' is taken by the goblin glyph. Uppercase 'G'
        // reads as "large hooved animal" at the small UI scale.
        glyph: 'G',
        ac: 11,
        // 3d10+3 = 19 average per MM (CR ½).
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 3,
        dexterity: 11,
        wisdom: 12,
        constitution: 12,
        charisma: 5,
        cr: 0.5,
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
            &GIANT_GOAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_goat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant goat ram").is_some());
    }
}
