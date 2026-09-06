use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CROCODILE_BITE, GIANT_CROCODILE_BITE, GIANT_CROCODILE_MULTI, GIANT_CROCODILE_TAIL,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Crocodile — CR ½ large beast. The amphibious ambush predator: a
/// scaly reptilian lurker that strikes from shallows and clamps prey
/// in its jaws. Slots alongside the other low-CR beast fillers (Boar /
/// Wolf / Giant Crab / Lizardfolk) at the bottom of the encounter
/// pool. Sister to the Giant Constrictor Snake in the "huge predator
/// that grapples" lane but at a much lower CR.
///
/// Action lanes:
/// - **crocodile bite** — STR-based 1d10+STR piercing melee. On hit
///   the target picks up the `Grappled` condition for 10 rounds — no
///   save, RAW: the bite latches on automatically. The auto-grapple
///   is the load-bearing rider; the bite IS the lock-down.
///
/// Defensive identity: AC 12 (thick scales), ~19 HP (3d10+3). Standard
/// beast envelope — no special resistances or condition immunities.
/// The crocodile's threat is the grapple lock-down on a target while
/// allies pile on.
///
/// Stat shape: AC 12, ~19 HP (3d10+3), STR 15, DEX 10, CON 13, INT 2,
/// WIS 10, CHA 5. Speed 20 (RAW also swim 30, whose magnitude we don't
/// model — but the tag ships, so a pool is free to cross).
/// Size Large. CR ½.
pub static CROCODILE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CROCODILE_BITE);
    CreatureTemplate {
        name: "Crocodile",
        // 'k' (lowercase) — distinct from 'c' (Couatl / Cambion /
        // Centaur / Carrion Crawler) and 'C' (Cyclops / Cloud Giant /
        // Chimera / Cult Fanatic). 'k' for the reptilian silhouette;
        // free in the glyph map and reads as the low-slung scaled body.
        glyph: 'k',
        ac: 12,
        // 3d10+3 ≈ 19 average per MM (CR ½).
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 20.,
        strength: 15,
        intelligence: 2,
        dexterity: 10,
        wisdom: 10,
        constitution: 13,
        charisma: 5,
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 0.5,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        ..CreatureTemplate::defaults()
    }
});

/// Giant Crocodile — CR 5 huge beast. The apex amphibious predator: a
/// 20-foot scaled monster that can swallow a humanoid in one bite. Slots
/// between the Owlbear (CR 3) and the Werebear (CR 5) on the upper-mid
/// beast bench, and alongside the Giant Constrictor Snake (CR 2) as
/// the canonical "huge ambush predator" pair.
///
/// Action lanes:
/// - **giant crocodile multiattack** — 1 bite + 1 tail per Action via
///   the heterogeneous `CompoundAttack` chassis. The bite carries the
///   auto-grapple rider; the tail is a vanilla heavy slam.
/// - **giant crocodile bite** (standalone) — STR-based 3d10+STR
///   piercing at reach 2 tiles (10 ft). Same auto-grapple rider as the
///   small crocodile but on much heavier dice.
/// - **giant crocodile tail** (standalone) — STR-based 2d8+STR
///   bludgeoning at reach 2 tiles. Vanilla heavy slam — the load-
///   bearing combat clause is the per-Action damage budget paired
///   with the bite's grapple lock.
///
/// Defensive identity: AC 14 (thick scaled hide), ~85 HP (9d12+27).
/// Standard beast envelope — no special resistances or condition
/// immunities. The giant croc's threat is the heavy 2-hit multiattack
/// + grapple lock-down on a huge HP bar.
///
/// Stat shape: AC 14, ~85 HP (9d12+27), STR 21, DEX 9, CON 17, INT 2,
/// WIS 10, CHA 7. Speed 30 (RAW also swim 50, whose magnitude we don't
/// model — but the tag ships, so a pool is free to cross).
/// Size Huge. CR 5.
pub static GIANT_CROCODILE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GIANT_CROCODILE_MULTI);
    actions.push(&*GIANT_CROCODILE_BITE);
    actions.push(&GIANT_CROCODILE_TAIL);
    CreatureTemplate {
        name: "Giant Crocodile",
        // 'K' (uppercase) — sibling glyph to the 'k' crocodile; the
        // huge variant gets the capital for its larger silhouette,
        // matching the wolf/dire-wolf naming style.
        glyph: 'K',
        ac: 14,
        // 9d12+27 ≈ 85 average per MM (CR 5).
        hitpoints: "9d12+27".parse().unwrap(),
        speed: 30.,
        strength: 21,
        intelligence: 2,
        dexterity: 9,
        wisdom: 10,
        constitution: 17,
        charisma: 7,
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 5.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn crocodile_template_shape() {
        let a = ActorInstance::from_creature_template(
            &CROCODILE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("crocodile bite").is_some());
    }

    #[test]
    fn giant_crocodile_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GIANT_CROCODILE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        // Three action lanes — multi primary, bite + tail standalones
        // for AI fallback.
        assert!(a.find_action("giant crocodile multiattack").is_some());
        assert!(a.find_action("giant crocodile bite").is_some());
        assert!(a.find_action("giant crocodile tail").is_some());
    }

    #[test]
    fn crocodile_carries_no_special_immunities() {
        // The crocodile's threat profile lives entirely on the grapple
        // rider — no damage resistances, no condition immunities, just
        // a heavy bite with auto-lockdown. This test pins the "vanilla
        // beast envelope" so a future refactor doesn't accidentally
        // grant the croc magic resistance or a condition immunity.
        let a = ActorInstance::from_creature_template(
            &CROCODILE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(!a.has_magic_resistance());
        assert!(!a.effectively_immune_to_condition(Condition::Grappled));
        assert!(!a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
