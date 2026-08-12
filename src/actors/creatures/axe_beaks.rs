use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::AXE_BEAK_BEAK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Axe Beak — CR ¼ large monstrosity. A flightless bird the size of a
/// horse with a beak that lives up to its name, and a fifty-foot speed
/// that is the reason anybody rides one.
///
/// Action lane:
/// - **axe beak beak** — STR-based 1d8+STR slashing. One swing, no
///   rider, no multiattack. The stat block is a chase with a weapon
///   attached.
///
/// **Mountable**, which is the point of the entry: the axe beak is the
/// cheap fast mount, twenty feet quicker than a riding horse and
/// available at a quarter of the challenge rating. It is a
/// **Monstrosity** rather than a Beast, so it sits outside every
/// beast-scoped effect the roster carries — no Conjure Animals, no Wild
/// Shape, no Beast Bond.
///
/// Stat shape: AC 11, ~19 HP (3d10+3), STR 14, DEX 12, CON 12, INT 2,
/// WIS 10, CHA 5. Speed 50. Size Large. CR ¼. XP 50 per RAW.
pub static AXE_BEAK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&AXE_BEAK_BEAK);
    CreatureTemplate {
        name: "Axe Beak",
        // 'k' — the lowercase bird silhouette shared with the hawk;
        // the axe beak is the one that cannot fly, which the board
        // shows by where it goes rather than by the letter.
        glyph: 'k',
        ac: 11,
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 50.,
        strength: 14,
        intelligence: 2,
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 5,
        cr: 0.25,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // The reason the stat block is in the book: a Large mount with
        // a fifty-foot speed at CR ¼.
        mountable: true,
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
            &AXE_BEAK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn axe_beak_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        assert!(a.find_action("axe beak beak").is_some());
    }

    /// Large, mountable, and faster than every horse on the roster —
    /// the three properties that are the whole reason to field one.
    #[test]
    fn the_axe_beak_is_a_fast_mount() {
        let a = make();
        assert!(a.is_mountable());
        assert!(a.speed() >= 50.0);
    }
}
