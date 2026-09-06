use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, MACE, THUG_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Thug — CR ½ humanoid bruiser. The "back-alley enforcer" tier of NPC
/// mooks: heavier than a bandit (more HP, harder hit), softer than a
/// bandit captain. Slots between the Bandit (CR ⅛, one scimitar) and
/// the Bandit Captain (CR 2, three scimitar swings) as the canonical
/// CR-½ humanoid melee pressure entry.
///
/// Action lanes:
/// - **mace** — STR-based 1d6+STR bludgeoning melee via the shared
///   `MACE` static. Bludgeoning rather than slashing so the thug feels
///   different from a bandit even when only the base swing fires.
/// - **double mace** — 2 mace swings per Action via the shared
///   `Multiattack` chassis (`THUG_MULTI`). Two-hit Action lands ~10
///   bludgeoning on a clean pair against a medium-AC target.
/// - **heavy crossbow** — DEX-based 1d10+DEX piercing ranged via the
///   shared `HEAVY_CROSSBOW` static. The 16-tile reach / 10-tile normal
///   range matches the bandit family so a mixed bandit / thug ambush
///   reads as one cohesive raiding party.
///
/// **Pack Tactics** — RAW: "The thug has advantage on attack rolls
/// against a creature if at least one of the thug's allies is within
/// 5 ft of the creature and the ally isn't incapacitated." Routes
/// through the shared `has_pack_tactics: true` template flag which
/// `compute_attack_mode` reads at the attack chokepoint. A pair of
/// thugs locking down one target is the canonical Pack Tactics double-
/// team — same chassis as the Wolf / Kobold pack lane.
///
/// Defensive identity: AC 12 (leather armor, no shield), 32 HP
/// (5d8+10). Vanilla humanoid envelope — no resistances or condition
/// immunities. The threat profile is Pack-Tactics-fueled multiattack
/// at close range; isolated, a thug is just a bag of HP with a club.
///
/// Stat shape: AC 12, ~32 HP (5d8+10), STR 15, DEX 12, CON 14, INT 10,
/// WIS 10, CHA 11. Speed 30. Languages: Common. Size Medium. CR ½.
/// XP: 100 per RAW.
pub static THUG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MACE);
    actions.push(&HEAVY_CROSSBOW);
    actions.push(&*THUG_MULTI);
    CreatureTemplate {
        name: "Thug",
        // 'H' — bandit-tier humanoid. 'B' is the bandit / bandit-captain
        // cohort; 'H' (for "Hoodlum"/"Heavy") keeps the thug distinct
        // on the map while still reading as a humanoid mook silhouette.
        // 'H' is otherwise untaken in the glyph map.
        glyph: 'H',
        ac: 12,
        // 5d8+10 ≈ 32 average per SRD 5.2 (CR ½).
        hitpoints: "5d8+10".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 10,
        dexterity: 12,
        wisdom: 10,
        constitution: 14,
        charisma: 11,
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 5e Pack Tactics — the thug gets advantage when an ally is
        // adjacent to the target. Read at the `compute_attack_mode`
        // chokepoint; no per-attack code needed here.
        has_pack_tactics: true,
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
            &THUG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn thug_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("mace").is_some());
        assert!(a.find_action("heavy crossbow").is_some());
        assert!(a.find_action("double mace").is_some());
    }

    #[test]
    fn thug_carries_pack_tactics() {
        // Pin the load-bearing trait: Pack Tactics is the thug's only
        // mechanical edge over a bandit. A future template-refactor
        // that strips the flag would quietly demote the thug to "just
        // a heavier bandit", flattening its tactical identity.
        let a = make();
        assert!(a.has_pack_tactics());
    }
}
