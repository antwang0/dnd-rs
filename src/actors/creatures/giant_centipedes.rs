use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_CENTIPEDE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Centipede — CR ¼ small beast. The "venomous-vermin" tier of
/// the low-CR bench: a fragile arachnid-shaped ambusher whose threat
/// profile is entirely in the bite's CON-DC-11 save-or-3d6-poison rider.
/// Slots beside the Giant Wolf Spider (CR ¼ medium, 2d6 poison rider on
/// CON 11) and the Stirge (CR ⅛ tiny, blood-drain) on the venom-flavored
/// low-CR ambusher bench — the centipede's edge is the *heavier* poison
/// rider (3d6 vs the spider's 2d6) at the same DC, balanced against the
/// fragile 9-HP frame.
///
/// Action lane:
/// - **giant centipede bite** — DEX-based 1d4+DEX piercing melee with a
///   DC 11 CON save-or-3d6-poison rider via the shared
///   `WeaponWithSaveDamage` chassis. Same "one save gates both base and
///   rider damage" shape as the Giant Wolf Spider / Spider Bite cohort;
///   only the dice / DC / typing differ. The 3d6 poison dice are
///   load-bearing — a failed save can drop a wounded low-level target
///   in one swing.
///
/// **The RAW "if poison reduces target to 0 HP, the target is stable
/// but poisoned for 1 hour, paralyzed while poisoned" clause** is
/// omitted as a scope cut. The engine's death-save flow handles 0-HP
/// stabilization separately and the conditional Paralyzed-while-Poisoned
/// install is hard to model cleanly through the shared chassis. The
/// pure save-or-damage envelope still captures the load-bearing threat.
///
/// Defensive identity: AC 14 (small + DEX), 9 HP (2d6+2). Vanilla beast
/// envelope — no resistances or condition immunities. The centipede
/// dies to a single solid hit (1 HP avg roll); its threat lives in
/// the bite's venom rider before it goes down. **Blindsight 30** lets
/// the centipede ambush in the dark of a cave / dungeon biome.
///
/// Stat shape: AC 14, ~9 HP (2d6+2), STR 5, DEX 14, CON 12, INT 1,
/// WIS 7, CHA 3. Speed 30 walking + climb 30 (collapsed to walking 30
/// since the engine doesn't track climbing separately). Senses:
/// Blindsight 30. Size Small. CR ¼. XP: 50 per RAW.
pub static GIANT_CENTIPEDE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_CENTIPEDE_BITE);
    CreatureTemplate {
        name: "Giant Centipede",
        // 'C' (uppercase) — shared with Couatl / Centaur / Cyclops /
        // Carrion Crawler cohort; the beast vs other-type context
        // disambiguates in the prompt and the team color separates them
        // on the map. Lowercase 'c' is taken by Commoner. Uppercase 'C'
        // reads as "many-legged crawler" beside the Carrion Crawler at
        // the small UI scale.
        glyph: 'C',
        ac: 14,
        // 2d6+2 = 9 average per MM (CR ¼). Lowest HP tier on the
        // CR-¼ bench — fragile glass-cannon venom-rider profile.
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 5,
        intelligence: 1,
        dexterity: 14,
        wisdom: 7,
        constitution: 12,
        charisma: 3,
        // Blindsight 30 — the centipede hunts by vibration in pitch-
        // black caves and underbrush. Routes through the same
        // concealment-cancelling chokepoint as the rest of the
        // blindsight-bearing pool (Giant Bat, Hook Horror, Otyugh).
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Beast,
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
            &GIANT_CENTIPEDE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_centipede_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant centipede bite").is_some());
    }

    #[test]
    fn giant_centipede_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 30 anchors
        // the centipede's "ambushes in pitch-black caves" identity.
        // A future template refactor that quietly stripped Blindsight
        // would erase the dungeon-vermin niche.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(30)));
    }
}
