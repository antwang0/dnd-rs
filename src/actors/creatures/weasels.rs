use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WEASEL_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Weasel — CR 0 tiny beast. The canonical "snake-killer" tiny
/// mustelid: a single flat-1 bite, AC 13, 1 HP. Slots beside the
/// Cat / Rat / Bat / Hawk cohort at the bottom of the CR ladder.
/// Distinguished from the rat by its DEX-driven envelope (DEX 16 vs
/// the rat's DEX 11) and from the cat by its DEX-keyed bite instead
/// of slashing claws. Sister to `WERETIGER_TEMPLATE` / hybrid
/// mustelid lycanthropes in flavor, but the regular weasel is the
/// "mundane snake-hunter" at the lowest tier.
///
/// Action lane:
/// - **weasel bite** — DEX-based 1-flat piercing melee via the shared
///   `WEASEL_BITE` static. The weasel's only swing. The 1d1-shape
///   die crits cleanly to 2 through the engine's uniform crit
///   chassis — same trick as the Hawk Talons / Bat Bite cohort.
///
/// **Keen Hearing and Smell** (RAW: advantage on Perception checks
/// using hearing or smell) is flavor-only — the engine doesn't
/// surface skill checks through combat.
///
/// Defensive identity: AC 13 (tiny + DEX 16), 1 HP. The weasel dies
/// to any solid hit; the role is "skirmishing nuisance" rather than
/// a credible damage threat.
///
/// Stat shape: AC 13, ~1 HP (1d4-1 → floored at 1), STR 3, DEX 16,
/// CON 8, INT 2, WIS 12, CHA 3. Speed 30. Size Tiny. CR 0. XP: 10
/// per RAW.
pub static WEASEL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WEASEL_BITE);
    CreatureTemplate {
        name: "Weasel",
        // 'w' (lowercase) — tiny mustelid silhouette. 'W' (uppercase)
        // is taken by Warlock / Warhorse / Werewolf / Wight / Wraith
        // / Wyvern cohort; lowercase 'w' reads as "tiny darting
        // nuisance" beside 'c' (Cat), 'f' (Frog), 'l' (Lizard), 'r'
        // (Rat) at the bottom of the CR ladder.
        glyph: 'w',
        ac: 13,
        // RAW: 1 (1d4 - 1) — engine floors HP rolls at 1.
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 30.,
        strength: 3,
        intelligence: 2,
        dexterity: 16,
        wisdom: 12,
        constitution: 8,
        charisma: 3,
        cr: 0.0,
        size: Size::Tiny,
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
            &WEASEL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn weasel_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("weasel bite").is_some());
    }
}
