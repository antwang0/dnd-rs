use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{AWAKENED_TREE_MULTI, AWAKENED_TREE_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Awakened Tree — CR 2 huge plant. The Awaken spell's signature target:
/// a normal tree given mobility and sentience. Slots between Sea Hag /
/// Ettercap (CR 2) and below the Treant (CR 9) on the plant-tier
/// ladder — the "little cousin of the treant" silhouette with a fraction
/// of the HP pool and only one slam lane per swing.
///
/// Action lanes:
/// - **awakened tree multiattack** — 2 slams per Action via the
///   standard `Multiattack` chassis. RAW: "Multiattack. The tree makes
///   two attacks." Homogeneous twin-swing at reach 10ft — same shape as
///   the zombie multislam, scaled up to 3d6+STR per hit.
/// - **awakened tree slam** (standalone) — STR-based 3d6+STR
///   bludgeoning at reach 10ft. The single-swing fallback for AI scripts
///   that step into adjacency between the two multi swings.
///
/// Defensive identity: AC 13 (natural armor — the bark hide). 59 HP
/// (7d12+14). Vulnerable to fire (the canonical plant weakness — fire
/// rips through living wood). Resistant to bludgeoning and piercing
/// (the bark / dense wood RAW). No condition immunities — the awakened
/// tree is still a "magically-animated tree", not an undead, so it can
/// be frightened / charmed / restrained like any other creature. The
/// MM-RAW "False Appearance" clause (indistinguishable from a normal
/// tree while motionless) doesn't fit the engine's combat-active
/// chokepoint cleanly — every actor surfaces in `combat_active`, so the
/// stealth flavor is dropped.
///
/// Stat shape: AC 13, ~59 HP (7d12+14), STR 19 (+4 mod), DEX 6, CON 15,
/// INT 10, WIS 10, CHA 7. Speed 20 (the lumbering walk — slower than
/// the treant's 30 because the awakened tree's roots are less
/// adapted). Languages: one chosen by the awakener (we default to
/// Common since the engine doesn't model the awakener's language
/// choice). Size Huge. CR 2.
pub static AWAKENED_TREE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*AWAKENED_TREE_MULTI);
    actions.push(&AWAKENED_TREE_SLAM);
    CreatureTemplate {
        name: "Awakened Tree",
        // 'T' (uppercase) — shared with Treant since both are plant-tier
        // tree silhouettes. The CR gap (2 vs 9) and the per-Action damage
        // budget make the two read distinctly in encounter logs.
        glyph: 'T',
        ac: 13,
        // 7d12+14 ≈ 59 average per MM (CR 2).
        hitpoints: "7d12+14".parse().unwrap(),
        speed: 20.,
        strength: 19,
        intelligence: 10,
        dexterity: 6,
        wisdom: 10,
        constitution: 15,
        charisma: 7,
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Huge,
        creature_type: CreatureType::Plant,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
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
    fn awakened_tree_template_shape() {
        let a = ActorInstance::from_creature_template(
            &AWAKENED_TREE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        // Two action lanes — multi (twin slam) primary, single slam
        // standalone for the AI fallback.
        assert!(a.find_action("awakened tree multiattack").is_some());
        assert!(a.find_action("awakened tree slam").is_some());
    }

    #[test]
    fn awakened_tree_is_fire_vulnerable() {
        let a = ActorInstance::from_creature_template(
            &AWAKENED_TREE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fire-vulnerable (the canonical plant weakness) plus
        // BP-resistant from the bark hide.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
    }
}
