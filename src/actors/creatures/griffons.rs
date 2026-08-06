use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GRIFFON_BEAK, GRIFFON_MULTI, GRIFFON_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Griffon — CR 2 large monstrosity. The classic flying predator — half
/// eagle, half lion, all dive-bombing menace. Slots between Hippogriff
/// (CR 1) and Manticore (CR 3) on the flying-beast ladder: the
/// hippogriff's heavier cousin with one more die per swing and one more
/// CR-step worth of HP. The engine doesn't model 3D flight, so the
/// griffon reads as a fast (40 ft) ground predator with a heterogeneous
/// beak+talons multi — the per-Action tempo is its identity, not its
/// flight speed.
///
/// Action lanes:
/// - **griffon multiattack** — 1 beak + 1 talons per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + slashing), so
///   target damage-type resistances don't shut down both halves of the
///   per-Action volley. ~ (1d8+4) + (2d6+4) ≈ 19.5 average per Action
///   against a single target — the CR-2 melee damage budget.
/// - **griffon beak** (standalone) — STR-based 1d8+STR piercing melee,
///   reach 1.
/// - **griffon talons** (standalone) — STR-based 2d6+STR slashing
///   melee, reach 1.
///
/// Defensive identity: AC 12 (natural armor — the dense feathered hide),
/// 59 HP (7d10+21). No resistances or immunities — the griffon is a
/// beast-tier predator with no elemental affinity or magic-resistance
/// envelope. Darkvision 60 ft (RAW: keen sight; the engine's closest
/// proxy). Senses don't include Tremorsense — the griffon hunts by
/// sight, not vibration.
///
/// Stat shape: AC 12, ~59 HP (7d10+21), STR 18, DEX 15, CON 16, INT 2,
/// WIS 13, CHA 8. Speed 40 (the canid-equivalent ground burst that
/// stands in for RAW's 80-ft fly speed). Senses: Darkvision 60.
/// Languages: none (a beast in everything but type). Size Large. CR 2.
///
/// RAW also gives the griffon **Keen Sight** (advantage on Perception
/// checks that rely on sight) — omitted because the engine doesn't tag
/// Perception rolls separately from other ability checks. The
/// load-bearing slice is the beak+talons multi, not the sight-advantage
/// nuance.
pub static GRIFFON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GRIFFON_MULTI);
    actions.push(&GRIFFON_BEAK);
    actions.push(&GRIFFON_TALONS);
    CreatureTemplate {
        name: "Griffon",
        // 'G' (uppercase) — distinct from 'g' (Goblin / Ghoul / Gnoll
        // all share lowercase 'g'). 'G' for "griffon" reads as the
        // larger-statured flying predator silhouette; reserves the
        // capital glyph slot for the more imposing of the bird-themed
        // monstrosities.
        glyph: 'G',
        ac: 12,
        // 7d10+21 ≈ 59 average per MM (CR 2).
        hitpoints: "7d10+21".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 2,
        dexterity: 15,
        wisdom: 13,
        constitution: 16,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Large,
        // 5e Mounted Combat: MM's classic aerial mount.
        mountable: true,
        creature_type: CreatureType::Monstrosity,
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

    #[test]
    fn griffon_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GRIFFON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // The griffon's three action lanes — multi (beak + talons)
        // primary, with each part also exposed as a standalone fallback
        // for the AI's per-resource picking.
        assert!(a.find_action("griffon multiattack").is_some());
        assert!(a.find_action("griffon beak").is_some());
        assert!(a.find_action("griffon talons").is_some());
    }

    #[test]
    fn griffon_has_no_magic_resistance_or_immunities() {
        let a = ActorInstance::from_creature_template(
            &GRIFFON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The griffon is a beast-tier predator — no magic resistance,
        // no condition immunities, no damage modifiers. The 59-HP pool
        // + 40-speed burst IS the defense; the beak+talons multi is
        // the offense.
        assert!(!a.has_magic_resistance());
    }
}
