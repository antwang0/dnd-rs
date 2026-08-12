use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BABOON_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Baboon — CR 0 small beast. Three hit points, a bite that averages
/// one damage, and **Pack Tactics**, which is the only line on the stat
/// block that matters. A baboon alone is scenery; six baboons are six
/// attacks a turn rolled at advantage, which at low levels is a real
/// encounter made entirely out of creatures worth ten experience each.
///
/// Action lane:
/// - **baboon bite** — STR-based 1d4+STR piercing. Strength 8 makes
///   the modifier −1, so the printed `1 (1d4 − 1)` falls straight out
///   of the template rather than needing a special flat-damage shape.
///
/// **Climb 30** is not modeled — the board is flat, so the climb speed
/// collapses into the walking speed.
///
/// Defensive identity: AC 12, ~3 HP (1d6). Anything that connects kills
/// a baboon. The cohort's threat is arithmetic, not durability.
///
/// Stat shape: AC 12, ~3 HP, STR 8, DEX 14, CON 11, INT 4, WIS 12,
/// CHA 6. Speed 30. Size Small. CR 0. XP 10 per RAW.
pub static BABOON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BABOON_BITE);
    CreatureTemplate {
        name: "Baboon",
        // 'a' (lowercase) — the small primate beside the ape's 'A',
        // the same size-ladder convention the shark cohort uses.
        glyph: 'a',
        ac: 12,
        hitpoints: "1d6".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 4,
        dexterity: 14,
        wisdom: 12,
        constitution: 11,
        charisma: 6,
        cr: 0.0,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
        // 5e Pack Tactics — advantage when an ally is adjacent to the
        // target. The entire stat block, mechanically speaking.
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
            &BABOON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn baboon_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("baboon bite").is_some());
    }

    /// Pack Tactics is the baboon's whole reason to exist on the
    /// roster. Without it the template is a worse rat.
    #[test]
    fn baboon_carries_pack_tactics() {
        assert!(make().has_pack_tactics());
    }
}
