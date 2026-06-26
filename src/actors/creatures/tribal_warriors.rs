use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SPEAR, TRIBAL_WARRIOR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Tribal Warrior — CR ⅛ humanoid. The "primitive raider" tier of NPC
/// mooks: lower-AC, lower-HP than a bandit but with Pack Tactics and a
/// double-spear multi. Slots beside the Bandit (CR ⅛, scimitar + heavy
/// crossbow, no Pack Tactics) as a melee-focused tribal alternative —
/// dangerous in numbers but trivial alone, mirroring the wolf / kobold
/// design pattern at the humanoid lane.
///
/// Action lanes:
/// - **spear** — STR-based 1d6+STR piercing melee via the shared
///   `SPEAR` static. RAW: versatile / thrown; we collapse to the
///   one-hand 1d6 melee base since the engine doesn't surface per-action
///   grip toggles. The thrown lane is already covered by `JAVELIN`.
/// - **double spear** — 2 spear swings per Action via the shared
///   `Multiattack` chassis (`TRIBAL_WARRIOR_MULTI`). Two-hit Action
///   lands ~7 piercing on a clean pair against a medium-AC target —
///   modest, but Pack Tactics turns each swing into an advantage roll
///   when allies are adjacent.
///
/// **Pack Tactics** — RAW: "The warrior has advantage on an attack roll
/// against a creature if at least one of the warrior's allies is within
/// 5 ft of the creature and the ally isn't incapacitated." Routes
/// through the shared `has_pack_tactics: true` template flag — same
/// chokepoint as the wolf / kobold / thug pack lanes. The combination
/// of Pack Tactics + double spear is the warrior's signature: a pair
/// of tribal warriors closing on one target lands four advantage-rolled
/// spear swings.
///
/// Defensive identity: AC 12 (hide armor), 11 HP (2d8+2). Vanilla
/// humanoid envelope — no resistances or condition immunities. The
/// warrior dies to a single solid hit; the threat lives in the swarm.
///
/// Stat shape: AC 12, ~11 HP (2d8+2), STR 13, DEX 11, CON 12, INT 8,
/// WIS 11, CHA 8. Speed 30. Languages: one language (we pick Common —
/// the engine doesn't track tribal-specific language pools). Size
/// Medium. CR ⅛. XP: 25 per RAW.
pub static TRIBAL_WARRIOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPEAR);
    actions.push(&*TRIBAL_WARRIOR_MULTI);
    CreatureTemplate {
        name: "Tribal Warrior",
        // 'w' — lowercase warrior silhouette, distinct from 'W' (Wolf
        // / Wraith). Lowercase reads as "low-CR humanoid mook" at the
        // small UI scale, mirroring 'b' (Berserker), 'd' (Dretch), etc.
        glyph: 'w',
        ac: 12,
        // 2d8+2 ≈ 11 average per MM (CR ⅛).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 8,
        dexterity: 11,
        wisdom: 11,
        constitution: 12,
        charisma: 8,
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 5e Pack Tactics — the warrior gets advantage when an ally is
        // adjacent to the target. Same chokepoint as Wolf / Kobold /
        // Thug; the combination with the double-spear multi is the
        // warrior's load-bearing offensive lane.
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
            &TRIBAL_WARRIOR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn tribal_warrior_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("spear").is_some());
        assert!(a.find_action("double spear").is_some());
    }

    #[test]
    fn tribal_warrior_carries_pack_tactics() {
        // Pin the load-bearing trait: Pack Tactics is the warrior's
        // only mechanical edge over a bandit's heavy crossbow / scimitar
        // pair. Stripping the flag would silently demote the warrior to
        // "a weaker bandit with a spear", flattening its swarm identity.
        let a = make();
        assert!(a.has_pack_tactics());
    }
}
