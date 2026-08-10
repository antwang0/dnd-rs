use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::JACKAL_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Jackal — CR 0 small beast. The cheapest pack animal in the game: a
/// 1d4 bite on a three-hit-point body, and `has_pack_tactics`, which is
/// the only line that matters.
///
/// A jackal alone is scenery. Six of them are a CR 0 creature rolling
/// with advantage on every swing, which is the whole point of the stat
/// block and the reason it sits below the Hyena (also CR 0, also pack
/// tactics, medium and faster) rather than beside it.
///
/// Stat shape per the SRD: AC 12, 3 HP (1d6), STR 8 / DEX 15 / CON 11 /
/// INT 3 / WIS 12 / CHA 6. Speed 40. Skills: Perception. CR 0.
pub static JACKAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&JACKAL_BITE);
    CreatureTemplate {
        name: "Jackal",
        // 'j' (lowercase) — free, and the letter the word is
        // recognisable by.
        glyph: 'j',
        ac: 12,
        // 1d6 ≈ 3 average per the SRD (CR 0).
        hitpoints: "1d6".parse().unwrap(),
        speed: 40.,
        strength: 8,
        dexterity: 15,
        constitution: 11,
        intelligence: 3,
        wisdom: 12,
        charisma: 6,
        skills: HashSet::from([Skill::Perception]),
        cr: 0.0,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
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

    #[test]
    fn jackal_template_shape() {
        let a = ActorInstance::from_creature_template(
            &JACKAL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Small);
        assert!(a.find_action("jackal bite").is_some());
        // The one line that makes the stat block worth having.
        assert!(a.has_pack_tactics());
    }
}
