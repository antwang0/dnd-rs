use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_FROG_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Frog — CR ¼ medium amphibian beast. The "swamp ambusher"
/// tier: a hopping tongue-grappler that latches onto a target on
/// hit, then drags them back into the muck. Slots beside the Giant
/// Toad (CR 1 Large, swallow-on-grapple) as the *smaller* / *cheaper*
/// auto-grapple amphibian — same flavor envelope, lower CR, lower
/// dice, no swallow. Pairs naturally as a low-tier wetland encounter
/// with stirges, giant rats, and the troglodyte cohort.
///
/// Action lane:
/// - **giant frog bite** — STR-based 1d6+STR piercing melee with an
///   auto-Grappled install on hit (no save) via the new
///   `WeaponWithCondition` chassis. The tongue snaps out, latches on,
///   and the target is grappled — the install fires the moment the
///   swing connects, matching the RAW "the target is grappled (escape
///   DC 11)" semantics (the escape DC is a later Action the grappled
///   actor spends, not a prevention save).
///
/// **Amphibious** (RAW: can breathe air and water) and **Standing
/// Leap** (long jump up to 20 ft, high jump up to 10 ft) are RAW
/// flavor-only — the engine doesn't surface 3D movement or
/// swim/water-terrain mechanics, so both clauses collapse to the
/// per-creature speed (30 walking; the swim half is omitted).
///
/// The **Swallow** follow-up (RAW: bite again with a Grappled Small-
/// or-smaller target → swallow whole, taking acid each turn) is
/// omitted as a deliberate scope cut. The engine doesn't model the
/// "stomach" container state, and adding one for a single CR-¼
/// creature would be over-scope. The auto-grapple bite still carries
/// the frog's tactical identity — pinning a low-HP target while
/// allies close in for the kill.
///
/// Defensive identity: AC 11 (small + DEX-driven), 18 HP (4d8).
/// Vanilla beast envelope — no resistances or condition immunities.
/// **Darkvision 30** lets the frog ambush from the muck in low-light
/// swamp encounters. The frog dies to a single solid hit; its
/// threat lives in the bite-then-Grappled combo that strands a
/// target in melee for the rest of the swamp pack to pile on.
///
/// Stat shape: AC 11, ~18 HP (4d8), STR 12, DEX 13, CON 11, INT 2,
/// WIS 10, CHA 3. Speed 30 (RAW: 30 + swim 30 — we collapse to the
/// walking speed since the engine isn't aquatic-terrain-aware).
/// Senses: Darkvision 30. Size Medium. CR ¼. XP: 50 per RAW.
pub static GIANT_FROG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_FROG_BITE);
    CreatureTemplate {
        name: "Giant Frog",
        // 'f' (lowercase) — small-amphibian silhouette. 'F' is taken
        // by Flameskull / Fire Imp cohort; lowercase 'f' reads as
        // "low-CR beast frog" at the small UI scale, mirroring 'r'
        // (Giant Rat), 't' (Tiger / saber-toothed Tiger), etc.
        glyph: 'f',
        ac: 11,
        // 4d8 = 18 average per MM (CR ¼).
        hitpoints: "4d8".parse().unwrap(),
        speed: 30.,
        strength: 12,
        intelligence: 2,
        dexterity: 13,
        wisdom: 10,
        constitution: 11,
        charisma: 3,
        // Darkvision 30 — the swamp predator's low-light senses.
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
            &GIANT_FROG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_frog_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant frog bite").is_some());
    }

    #[test]
    fn giant_frog_carries_darkvision() {
        // Pin the load-bearing sensory trait: Darkvision 30 — the
        // swamp predator hunts from low-light muck, and a future
        // template-refactor that quietly stripped the Darkvision
        // would erase the "ambushes from the dark / under-water"
        // identity (engine routes Darkvision through the same
        // light-level concealment chokepoint as the rest of the
        // dark-adapted beast cohort).
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Darkvision(30)));
    }
}
