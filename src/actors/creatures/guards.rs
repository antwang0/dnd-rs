use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPEAR;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Guard — CR ⅛ humanoid soldier. The "city watch" tier of NPC mook:
/// trained but unmotivated militia armed with spear and chain shirt +
/// shield. Slots beside the Bandit (CR ⅛, scimitar + heavy crossbow,
/// no shield) as the *lawful* / *defensive* counterpart — heavier AC
/// from the shield + chain shirt makes the guard the canonical
/// "harder to hit but fewer attack options" tradeoff.
///
/// Action lane:
/// - **spear** — STR-based 1d6+STR piercing melee via the shared
///   `SPEAR` static. RAW: versatile / thrown; we collapse to the
///   one-hand 1d6 melee base since the engine doesn't surface per-
///   action grip toggles, and the thrown lane is already covered by
///   `JAVELIN`. Single-swing per Action (no multiattack) — the guard
///   is the per-round-low-damage / high-AC entry on the CR-⅛ bench
///   vs the tribal warrior's per-round-high-damage / low-AC profile.
///
/// Defensive identity: AC 16 (chain shirt + shield — the *highest* AC
/// on the CR-⅛ humanoid bench, three points clear of the bandit's AC
/// 12). 11 HP (2d8+2). Vanilla humanoid envelope — no resistances or
/// condition immunities. The guard's load-bearing trait is the
/// defensive envelope: a swing has to roll AC 16 to land at all,
/// which at low levels means most attacks miss outright.
///
/// Stat shape: AC 16, ~11 HP (2d8+2), STR 13, DEX 12, CON 12, INT 10,
/// WIS 11, CHA 10. Speed 30. Languages: Common (RAW: "any one
/// language"). Size Medium. CR ⅛. XP: 25 per RAW.
pub static GUARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPEAR);
    CreatureTemplate {
        name: "Guard",
        // 'g' (lowercase) — city watchman silhouette. Shared with Gnoll
        // ('g') / Goblin Boss ('g'); the team color disambiguates on the
        // map and the lawful-humanoid context rarely collides with the
        // gnoll-tier mooks in random encounter rolls. 'G' is taken by
        // Galeb Duhr / Gargoyle / Giant cohort.
        glyph: 'g',
        ac: 16,
        // 2d8+2 ≈ 11 average per MM (CR ⅛).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
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
            &GUARD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn guard_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("spear").is_some());
    }

    #[test]
    fn guard_carries_chain_shirt_plus_shield_ac() {
        // Pin the load-bearing defensive trait: AC 16 (chain shirt +
        // shield) is the highest AC on the CR-⅛ humanoid bench,
        // three points clear of the bandit's AC 12 and four points
        // clear of the commoner's AC 10. A future template-refactor
        // that dropped the guard to AC 13 / 14 would silently demote
        // the guard to "a slightly bigger bandit", erasing the
        // "harder to hit, fewer attack options" tradeoff that defines
        // the guard's tactical identity.
        let a = make();
        assert_eq!(a.armor_class(), 16);
    }
}
