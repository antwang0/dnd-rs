use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GOAT_RAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Goat — CR 0 medium beast. The barnyard ungulate floor of the
/// caprid family: a single 1d4+STR headbutt on a 4-HP frame. Slots
/// beside the Camel / Mule / Cat / Frog on the CR-0 ambient-fauna
/// bench — the canonical "farm animal" silhouette. Sister to
/// `GIANT_GOAT_TEMPLATE` (CR ½ large with 2d4 ram) on the caprid
/// ladder, with the regular goat being the smaller-dice / smaller-
/// frame floor tier. Faster than the typical CR-0 ambient (speed 40
/// vs the rat's 20 / cat's 30) — the goat keeps the mountain-
/// dweller's "scampering hill-side" flavor even at the lowest CR
/// tier.
///
/// Action lane:
/// - **goat ram** — STR-based 1d4+STR bludgeoning melee via the
///   shared `GOAT_RAM` static. The goat's only swing. ~3 avg per hit
///   (1d4 = 2.5 + STR 12 = +1), too light to credibly threaten
///   anything but a fellow CR-0 fauna or an already-bloodied PC.
///
/// **Charge** (RAW: 20 ft straight charge → extra 2d4 + DC-10 STR
/// save-or-Prone) and **Sure-Footed** (advantage on STR/DEX saves vs
/// effects that would knock it prone) are both omitted — the same
/// straight-line scope cut that hollows the Boar / Giant Goat /
/// Warhorse charges, and the per-condition save-advantage hook
/// Sure-Footed needs isn't surfaced on `roll_save`.
///
/// Defensive identity: AC 10 (no natural hide, no armor), 4 HP
/// (1d8). Vanilla beast envelope — no resistances or condition
/// immunities. Dies to any solid hit; threat profile is "ambient
/// hillside texture," not credible damage.
///
/// Stat shape: AC 10, ~4 HP (1d8), STR 12, DEX 10, CON 11, INT 2,
/// WIS 10, CHA 5. Speed 40. Size Medium. CR 0. XP: 10 per RAW.
pub static GOAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GOAT_RAM);
    CreatureTemplate {
        name: "Goat",
        // 'g' (lowercase) — shared with Goblin / Giant Rat / Gnome
        // cohort. The team color disambiguates on the map; the goat's
        // Medium frame separates it from the giant goat's uppercase
        // 'G' silhouette without needing a distinct glyph. Lowercase
        // 'g' reads as "small horned ambient" beside 'b' (Boar),
        // 'c' (Cat) on the barnyard bench.
        glyph: 'g',
        ac: 10,
        // 1d8 = 4 average per MM (CR 0).
        hitpoints: "1d8".parse().unwrap(),
        speed: 40.,
        strength: 12,
        intelligence: 2,
        dexterity: 10,
        wisdom: 10,
        constitution: 11,
        charisma: 5,
        cr: 0.0,
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
            &GOAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn goat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("goat ram").is_some());
    }

    #[test]
    fn goat_is_lighter_than_giant_goat() {
        // Pin the load-bearing scaling distinction: the regular goat
        // is the CR-0 / Medium / 1d4-ram floor tier of the caprid
        // ladder, distinct from the Giant Goat's CR-½ / Large /
        // 2d4-ram envelope. A future template refactor that quietly
        // bumped the dice or size would collapse the two-tier ladder
        // into a single redundant entry.
        let a = make();
        assert_eq!(a.size(), Size::Medium);
        assert!(a.hitpoints() <= 10);
    }
}
