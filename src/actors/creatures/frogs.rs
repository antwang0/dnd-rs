use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Frog — CR 0 tiny beast. The canonical "ambient amphibian" pond
/// fauna: no attacks, 1 HP, the smallest possible combat threat. Sits
/// beside `RAT_TEMPLATE` / `BAT_TEMPLATE` / `CAT_TEMPLATE` at the very
/// bottom of the CR ladder, distinguished by its **no-attack** action
/// lane — even the rat / bat / hawk cohort has a flat-1 nibble; the
/// regular Frog can only Move / Skip / Hop. Sister to `GIANT_FROG_
/// TEMPLATE` (CR ¼ medium amphibian with tongue grab) on the frog
/// ladder.
///
/// Action lane:
/// - *(no native attacks)* — the frog defaults to `DEFAULT_ACTIONS`
///   only (Move / Dash / Dodge / Hide / Skip). A frog cannot harm a
///   PC; it exists as flavor / ambient atmosphere.
///
/// **Amphibious** (RAW: breathes air and water) is flavor-only — the
/// engine doesn't model drowning today, so the trait collapses to a
/// note. **Standing Leap** (RAW: long jump = 10 ft) is captured by
/// the speed-20 walking baseline; the engine doesn't model separate
/// jump distances. Darkvision 30 surfaces through the standard
/// `SpecialSense::Darkvision` chokepoint.
///
/// Defensive identity: AC 11 (tiny + DEX), 1 HP. The frog dies to
/// any solid hit; its load-bearing tactical value is *zero* —
/// strictly ambient flavor.
///
/// Stat shape: AC 11, ~1 HP (1d4-1 → floored at 1), STR 1, DEX 13,
/// CON 8, INT 1, WIS 8, CHA 3. Speed 20 — RAW: walking 20 ft + swim
/// 20 ft (the engine collapses ground + swim into a single per-
/// creature speed). Size Tiny. CR 0. XP: 10 per RAW.
pub static FROG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        name: "Frog",
        // 'f' (lowercase) — tiny pond fauna silhouette. 'F' (uppercase)
        // is taken by Fire Imp / Flesh Golem / Fire Giant / Frost Giant
        // cohort; lowercase 'f' reads as "tiny amphibian" beside 'c'
        // (Cat), 'r' (Rat), 'b' (Bat) at the bottom of the CR ladder.
        glyph: 'f',
        ac: 11,
        // RAW: 1 (1d4 - 1) — engine floors HP rolls at 1.
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 20.,
        strength: 1,
        intelligence: 1,
        dexterity: 13,
        wisdom: 8,
        constitution: 8,
        charisma: 3,
        // Darkvision 30 — the frog's signature low-light sense for
        // pond / cave ambient flavor. Same chokepoint as the Rat /
        // Lizard cohort at this CR tier.
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
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
            &FROG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn frog_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
    }

    #[test]
    fn frog_has_no_native_attacks() {
        // Pin the load-bearing identity: the frog is the *only* CR-0
        // ambient beast with **no native attack** — Cat / Rat / Bat /
        // Hawk all carry a flat-1 swing; the frog collapses to the
        // default action lane (Move / Skip / Dash / Hide / Dodge).
        // A future template refactor that quietly grafted a frog bite
        // would erase this niche; the canonical bench would silently
        // shift to "every CR-0 beast bites for 1." Pin the absence so
        // the unique identity stays explicit.
        let a = make();
        assert!(a.find_action("frog bite").is_none());
        assert!(a.find_action("bite").is_none());
    }

    #[test]
    fn frog_has_darkvision() {
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Darkvision(30)));
    }
}
