use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WINGED_KOBOLD_DAGGER, WINGED_KOBOLD_DROPPED_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Winged Kobold (Urd) — CR ¼ small humanoid. The kobold that got wings,
/// and with them the one thing an ordinary kobold's Pack Tactics could
/// never guarantee: a position.
///
/// Action lanes:
/// - **winged kobold dagger** — 1d4+DEX piercing, light.
/// - **winged kobold dropped rock** — 1d6 bludgeoning at four tiles of
///   clean band. RAW's "one target directly under the kobold" is a
///   vertical clause on a flat board; the short range is what is left
///   of it, and it keeps the urd fighting nearly on top of things.
///
/// **Pack Tactics** is the family trait and the reason kobolds are ever
/// a threat: advantage on an attack roll against any creature an ally
/// is adjacent to. On a flier the clause reads differently from how it
/// reads on the ground — an urd can put itself next to whatever its
/// pack is already fighting without walking through anybody's reach.
///
/// **Sunlight Sensitivity** is the price, at the same `Sensitivity`
/// tier the ground kobold pays: disadvantage on attack rolls while
/// standing in sunlight. It is what turns a swarm of urds from a
/// daytime problem into a night-time one, and it composes with Pack
/// Tactics exactly as RAW intends — an urd in the sun with an ally
/// adjacent to its target rolls straight, having spent its advantage
/// paying off the sun.
///
/// Stat shape per the SRD: AC 13, 7 HP (3d6-3), STR 7 / DEX 16 / CON 9
/// / INT 8 / WIS 7 / CHA 8. Speed 30 walking, 30 flying — the engine
/// has one speed and both are the same number, so nothing is lost.
/// Darkvision 60. Languages: Common, Draconic. CR ¼.
pub static WINGED_KOBOLD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WINGED_KOBOLD_DAGGER);
    actions.push(&WINGED_KOBOLD_DROPPED_ROCK);
    CreatureTemplate {
        name: "Winged Kobold",
        // 'w' (lowercase) — free; 'k' is the Darkmantle's and 'K' the
        // Kobold's own, which the urd is deliberately not confused
        // with: the two fight from different places.
        glyph: 'w',
        ac: 13,
        // 3d6-3 ≈ 7 average per the SRD (CR ¼).
        hitpoints: "3d6-3".parse().unwrap(),
        speed: 30.,
        strength: 7,
        dexterity: 16,
        constitution: 9,
        intelligence: 8,
        wisdom: 7,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        has_pack_tactics: true,
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
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
            &WINGED_KOBOLD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn winged_kobold_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Small);
        assert!(a.find_action("winged kobold dagger").is_some());
        assert!(a.find_action("winged kobold dropped rock").is_some());
    }

    /// Both halves of the kobold bargain. They are set independently
    /// and they cancel each other in sunlight, which is the clause that
    /// makes the creature a night encounter — dropping either one
    /// silently rebalances it.
    #[test]
    fn the_urd_trades_daylight_for_the_pack() {
        let a = make();
        assert!(a.has_pack_tactics());
        assert_eq!(a.sunlight_frailty(), Some(SunlightFrailty::Sensitivity));
    }

    /// The rock is a short-range shot and says so. A ranged weapon with
    /// no declared band is invisible to the long-range clause and to
    /// Underwater Combat's automatic miss.
    #[test]
    fn the_dropped_rock_declares_the_short_band_that_stands_in_for_being_overhead() {
        assert_eq!(WINGED_KOBOLD_DROPPED_ROCK.normal_range, Some(4));
        assert_eq!(WINGED_KOBOLD_DROPPED_ROCK.reach, 8);
    }
}
