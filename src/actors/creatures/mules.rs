use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MULE_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Mule — CR ⅛ medium beast. The patient pack animal: a single
/// 1d4+STR defensive kick on an 11-HP frame. Slots beside the
/// Camel (CR ⅛ large pack beast), Mastiff (CR ⅛ trained guard
/// dog), Guard (CR ⅛ city watch) on the CR-⅛ docile / mundane
/// bench — the canonical "merchant caravan beast of burden."
/// Distinguished from the Pony (also CR ⅛ medium beast) by its
/// lighter 1d4 dice (the pony lands a heavier 2d4 hooves) and its
/// flavor as the cargo hauler rather than the rideable mount.
/// Sister to `RIDING_HORSE_TEMPLATE` (CR ¼ large 2d4) and
/// `PONY_TEMPLATE` (CR ⅛ medium 2d4) on the equine / asinine
/// pack-animal ladder.
///
/// Action lane:
/// - **mule hooves** — STR-based 1d4+STR bludgeoning melee via the
///   shared `MULE_HOOVES` static. The mule's only swing — a stubborn
///   kick when cornered, dealing about 4.5 average per hit (1d4 = 2.5
///   + STR 14 = +2). The mule is a beast of burden, not a combatant.
///
/// **Beast of Burden** (RAW: counts as Large for determining carry
/// capacity) and **Sure-Footed** (advantage on STR/DEX saves vs
/// effects that would knock it prone) are both out-of-scope. The
/// engine doesn't model carry weight or per-condition save-advantage
/// hooks; both traits are flavor-only at the combat layer.
///
/// Defensive identity: AC 10 (no natural hide, no armor), 11 HP
/// (2d8+2). Vanilla beast envelope — no resistances or condition
/// immunities. Slightly tougher than the goat / camel at the same
/// tier (the CON-13 buffer is the mule's load-bearing defense),
/// matching the "stubborn pack mule" flavor.
///
/// Stat shape: AC 10, ~11 HP (2d8+2), STR 14, DEX 10, CON 13,
/// INT 2, WIS 10, CHA 5. Speed 40. Size Medium. CR ⅛. XP: 25 per
/// RAW.
pub static MULE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MULE_HOOVES);
    CreatureTemplate {
        name: "Mule",
        // 'm' (lowercase) — shared with Mastiff / Magmin / Mimic
        // cohort. The team color disambiguates on the map; the mule's
        // medium-pack-beast context separates it from the mastiff /
        // mimic at the prompt layer. Uppercase 'M' is taken by the
        // Manticore / Medusa / Mephit / Merrow / Minotaur / Mummy
        // bench at the upper tier.
        glyph: 'm',
        ac: 10,
        // 2d8+2 = 11 average per MM (CR ⅛).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 14,
        intelligence: 2,
        dexterity: 10,
        wisdom: 10,
        constitution: 13,
        charisma: 5,
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
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &MULE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn mule_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("mule hooves").is_some());
    }
}
