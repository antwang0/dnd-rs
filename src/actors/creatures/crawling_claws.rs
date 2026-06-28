use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CRAWLING_CLAW_SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Crawling Claw — CR 0 tiny undead. The "severed hand" minion tier:
/// a fragile tiny frame with a single 1d4 slashing swing and
/// Turn-Undead immunity (modeled implicitly — the engine doesn't
/// surface Turn Undead as a discrete combat action, so the trait is
/// flavor-only for now). Slots beside the Commoner (CR 0 humanoid
/// baseline) on the lowest CR tier of the bench — the "summoner's
/// minor cantrip" undead useful as wave-spawn filler for a
/// necromancer encounter.
///
/// Action lane:
/// - **crawling claw** — STR-based 1d4+STR slashing melee via the
///   shared `CRAWLING_CLAW_SLAM` static. The CR-0 claw's only swing.
///   1d4+1 averages to 3 — barely a scratch on most targets, but
///   adequate to chip away at unarmored / sleeping civilians (the
///   thematic flavor of the necromantic minion). RAW's "choose
///   bludgeoning / piercing / slashing per swing" is a minor flavor
///   option the engine doesn't surface; we pin to slashing as the
///   default.
///
/// **Turn Immunity** (RAW: "The claw is immune to features that
/// turn undead") is RAW-true but flavor-only at this engine scale —
/// the engine doesn't yet expose a discrete Turn Undead action a
/// cleric could fire at the claw, so the immunity has no current
/// behavior to gate. Listed in the docstring for parity with the
/// canonical statblock so a future Turn Undead implementation
/// landing on the cleric chassis can read the flag here.
///
/// Defensive identity: AC 12 (tiny + agile), 2 HP (1d4). Vanilla
/// undead envelope — Poison damage / Poisoned condition immunity
/// via the standard undead profile. The claw dies to a single
/// scratch from any combat creature; it exists for the swarm-
/// spawn flavor rather than as a credible solo threat.
///
/// **Languages** — RAW: "understands Common but can't speak."
/// Language sets in this engine encode "can be addressed in this
/// tongue" (for Suggestion / Command targeting), so we list
/// Common — the speech-vs-comprehension wrinkle doesn't matter at
/// the combat layer.
///
/// Stat shape: AC 12, ~2 HP (1d4), STR 13, DEX 14, CON 11, INT 5,
/// WIS 10, CHA 4. Speed 20 walking (collapsed to 30, the default
/// floor — the claw scuttles at a slow walking pace, but the engine
/// rounds short speeds up to the baseline tile-per-round move).
/// Senses: Blindsight 30 (the eyeless hand "feels" the environment
/// through vibration / necromantic sensitivity). Size Tiny. CR 0.
/// XP: 10 per RAW.
pub static CRAWLING_CLAW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CRAWLING_CLAW_SLAM);
    CreatureTemplate {
        name: "Crawling Claw",
        // 'x' (lowercase) — Tiny scuttling appendage silhouette.
        // Distinct from 'X' (Spider). Lowercase 'x' is otherwise
        // unused in the glyph map and reads as "small skittering
        // thing" at the small UI scale, mirroring the spider
        // hierarchy ('X' = giant, 'x' = severed hand).
        glyph: 'x',
        ac: 12,
        // 1d4 = 2 average per MM (CR 0).
        hitpoints: "1d4".parse().unwrap(),
        speed: 20.,
        strength: 13,
        intelligence: 5,
        dexterity: 14,
        wisdom: 10,
        constitution: 11,
        charisma: 4,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Undead,
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
            &CRAWLING_CLAW_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn crawling_claw_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert!(a.find_action("crawling claw").is_some());
    }
}
