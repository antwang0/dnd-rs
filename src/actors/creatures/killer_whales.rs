use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::KILLER_WHALE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Killer Whale — CR 3 huge beast. The "apex orca" tier of marine
/// predator: a Huge frame with a chunky 5d6 single bite and
/// Blindsight 60 (echolocation). Slots beside the Plesiosaurus (CR 2
/// huge marine biter) and the Giant Shark (CR 5 huge blood-frenzy
/// biter) on the marine-megafauna bench — the canonical CR-3
/// echolocating cetacean predator.
///
/// Action lane:
/// - **killer whale bite** — STR-based 5d6+STR piercing melee via
///   the shared `KILLER_WHALE_BITE` static. The CR-3 orca's only
///   swing. The 5d6 dice are the heaviest single-die count on the
///   CR-3 bench (5d6 avg 17.5 + STR mod ≈ 22) — a clean opening
///   hit can drop a soft-AC mid-level target outright.
///
/// **Echolocation** (RAW: the orca can't use blindsight while
/// deafened) and **Hold Breath** (30 min) are flavor-only at the
/// engine scale: the engine doesn't track deafness gating sense
/// availability, and breath rounds don't surface in encounter
/// scope. The Blindsight 60 lands on the template directly as the
/// load-bearing combat sense — anti-stealth in murky water /
/// pitch-black ocean depths where line of sight breaks down.
///
/// Defensive identity: AC 12 (huge + thick blubber), 90 HP
/// (12d12+12). Vanilla beast envelope — no resistances or condition
/// immunities. The HP envelope is heavy (90 HP vs the warhorse's
/// 19) so the orca outlasts most CR-3 frontliners while hitting
/// just as hard as a much higher CR creature on the swing it does
/// land.
///
/// **Keen Hearing** (advantage on hearing Perception) is RAW
/// flavor-only — the engine doesn't surface skill checks through
/// combat.
///
/// Stat shape: AC 12, ~90 HP (12d12+12), STR 19, DEX 14, CON 13,
/// INT 3, WIS 12, CHA 7. Speed 0 walking + swim 60 (magnitude
/// collapsed to 30 since the engine tracks a swimming speed as a flag
/// rather than as a second budget; the orca
/// spends every encounter in or near water and the engine's flat
/// move budget approximates the swim envelope). Senses:
/// Blindsight 60. Size Huge. CR 3. XP: 700 per RAW.
pub static KILLER_WHALE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&KILLER_WHALE_BITE);
    CreatureTemplate {
        name: "Killer Whale",
        // 'K' (uppercase) — shared with Knight / Kobold / Kraken
        // cohort. The team color disambiguates on the map; the
        // orca's beast / marine context separates it from the
        // humanoid / aberration cohort at the prompt layer.
        // Uppercase 'K' reads as "huge marine predator" at the
        // small UI scale.
        glyph: 'K',
        ac: 12,
        // 12d12+12 = 90 average per MM (CR 3).
        hitpoints: "12d12+12".parse().unwrap(),
        // RAW: 0 walking + swim 60. Magnitude collapsed to 30 (the
        // standard baseline) since the engine doesn't surface swim speed
        // separately — most ocean encounters treat tiles as
        // navigable water for the orca's purposes.
        speed: 30.,
        strength: 19,
        intelligence: 3,
        dexterity: 14,
        wisdom: 12,
        constitution: 13,
        charisma: 7,
        // Blindsight 60 — echolocation. The load-bearing combat
        // sense; defines the orca's anti-stealth pressure on murky-
        // water encounters where line of sight breaks down.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 3.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
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
            &KILLER_WHALE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn killer_whale_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("killer whale bite").is_some());
    }

    #[test]
    fn killer_whale_has_echolocation_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60
        // anchors the orca's "echolocating apex predator" identity
        // and breaks invisible-prey concealment at the same range
        // as the Hook Horror / Otyugh / Mammoth cohort. A future
        // template refactor that stripped Blindsight would demote
        // the orca to "a big shark with no senses" — losing the
        // anti-stealth pressure that makes it interesting on
        // murky-water encounters.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }
}
