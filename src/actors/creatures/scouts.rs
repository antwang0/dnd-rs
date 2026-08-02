use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    LONGBOW, SCOUT_MELEE_MULTI, SCOUT_RANGED_MULTI, SHORTSWORD,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Scout — CR ½ humanoid ranger / tracker. The "wilderness skirmisher"
/// tier of NPC mooks: lighter-armored than a thug but more flexible —
/// a multiattack on either lane (2 shortsword swings OR 2 longbow shots)
/// lets the scout pick its weapon by engagement distance. Slots beside
/// the Thug (CR ½, Pack Tactics + double mace) and the Bandit (CR ⅛,
/// scimitar + heavy crossbow) as a melee-or-ranged switch-hitter — the
/// canonical "tracker" NPC for wilderness encounter design.
///
/// Action lanes:
/// - **shortsword** — DEX-based 1d6+DEX piercing melee via the shared
///   `SHORTSWORD` static (finesse weapon).
/// - **double shortsword** — 2 shortsword swings per Action via the
///   shared `Multiattack` chassis (`SCOUT_MELEE_MULTI`). Two-hit melee
///   Action lands ~7 piercing on a clean pair against medium AC.
/// - **longbow** — DEX-based 1d8+DEX piercing ranged via the shared
///   `LONGBOW` static (20-tile reach, 12 normal range).
/// - **double longbow** — 2 longbow shots per Action via the shared
///   `Multiattack` chassis (`SCOUT_RANGED_MULTI`). The ranged half of
///   RAW's "two melee OR two ranged" multiattack. Two-hit ranged Action
///   lands ~9 piercing on a clean pair at long-bow range.
///
/// RAW also has **Keen Hearing and Sight** (advantage on Perception
/// checks using hearing / sight) and proficiency in Nature / Stealth /
/// Survival. The engine doesn't surface skill checks through combat,
/// so those traits stay flavor-only — the load-bearing combat clauses
/// (the four-lane multiattack split) define the scout's per-round
/// footprint.
///
/// Defensive identity: AC 13 (leather armor + DEX), 16 HP (3d8+3).
/// Vanilla humanoid envelope — no resistances or condition immunities.
/// The scout's edge over the thug is reach: a competent player can
/// kite a scout out of melee, but the longbow lane keeps the scout
/// dangerous at any distance.
///
/// Stat shape: AC 13, ~16 HP (3d8+3), STR 11, DEX 14, CON 12, INT 11,
/// WIS 13, CHA 11. Speed 30. Languages: Common (RAW: "any one language").
/// Size Medium. CR ½. XP: 100 per RAW.
pub static SCOUT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SHORTSWORD);
    actions.push(&LONGBOW);
    actions.push(&*SCOUT_MELEE_MULTI);
    actions.push(&*SCOUT_RANGED_MULTI);
    CreatureTemplate {
        name: "Scout",
        // 'S' — shared with Skeleton / Androsphinx (the 'S' silhouette
        // covers the "stealthy / sharp" humanoid figure). At low CR
        // the scout reads more like a ranger silhouette than a skeleton;
        // the team color disambiguates on the map. Lowercase 's' is
        // taken by Salamander / Stirge cohort.
        glyph: 'S',
        ac: 13,
        // 3d8+3 ≈ 16 average per MM (CR ½).
        hitpoints: "3d8+3".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 11,
        dexterity: 14,
        wisdom: 13,
        constitution: 12,
        charisma: 11,
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
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
            &SCOUT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn scout_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("shortsword").is_some());
        assert!(a.find_action("longbow").is_some());
        assert!(a.find_action("double shortsword").is_some());
        assert!(a.find_action("double longbow").is_some());
    }

    #[test]
    fn scout_lacks_pack_tactics() {
        // Pin the deliberate omission: scouts are skirmishers, NOT
        // pack-tactics swarmers. The thug at the same CR has Pack
        // Tactics; the scout deliberately doesn't, so the two CR-½
        // humanoid mooks feel mechanically distinct rather than
        // overlapping. A future template-refactor that bolts Pack
        // Tactics onto the scout would erase the design contrast.
        let a = make();
        assert!(!a.has_pack_tactics());
    }
}
