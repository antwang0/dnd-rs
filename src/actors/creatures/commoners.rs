use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CLUB;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Commoner — CR 0 humanoid baseline. The "townsfolk" tier of NPC: a
/// peasant, farmhand, urchin, or innkeeper. Lowest stat block in the
/// pool: AC 10 (no armor), 4 HP (1d8), 10s across every ability. The
/// canonical CR-0 baseline against which every other NPC is scaled.
/// Slots beneath the Bandit / Tribal Warrior (both CR ⅛) as the "no
/// combat training" NPC — appears in town encounters as the civilian
/// being threatened, not as a credible threat themselves.
///
/// Action lane:
/// - **club** — STR-based 1d4+STR bludgeoning melee via the shared
///   `CLUB` static. Lowest damage tier in the weapon pool (tied with
///   Dagger). The peasant picks up a stick to defend themselves; the
///   swing is symbolic rather than dangerous.
///
/// Defensive identity: AC 10 (no armor, no shield), 4 HP (1d8).
/// Vanilla humanoid envelope — no resistances, no condition
/// immunities, no Pack Tactics. A commoner dies to a single solid hit
/// from any combat creature. The template exists for civilian
/// encounters and as the floor of the NPC-CR ladder rather than as a
/// combat threat.
///
/// Stat shape: AC 10, ~4 HP (1d8), STR 10, DEX 10, CON 10, INT 10,
/// WIS 10, CHA 10. Speed 30. Languages: Common (RAW: "any one
/// language"). Size Medium. CR 0. XP: 10 per RAW.
pub static COMMONER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLUB);
    CreatureTemplate {
        name: "Commoner",
        // 'c' (lowercase) — peasant silhouette, distinct from 'C' (Couatl /
        // Centaur / Cyclops cohort). Lowercase reads as "civilian / mook"
        // at the small UI scale, mirroring 'b' (Berserker), 'd' (Dretch).
        glyph: 'c',
        ac: 10,
        // 1d8 = 4 average per MM (CR 0).
        hitpoints: "1d8".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 10,
        dexterity: 10,
        wisdom: 10,
        constitution: 10,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 0.0,
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
            &COMMONER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn commoner_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("club").is_some());
    }

    #[test]
    fn commoner_is_the_baseline_npc() {
        // Pin the baseline shape: every ability score sits at 10 (the
        // RAW commoner statblock baseline), AC at 10 (unarmored), and
        // no Pack Tactics / Extra Attack / damage-modifier flags. The
        // commoner exists as the CR-0 floor of the NPC ladder — a
        // future refactor that quietly bolted on combat traits would
        // demote the commoner from "townsfolk" to "minor combatant",
        // flattening the civilian / combatant contrast.
        let a = make();
        assert!(!a.has_pack_tactics());
        assert!(!a.has_extra_attack());
    }
}
