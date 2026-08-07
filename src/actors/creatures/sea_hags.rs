use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SEA_HAG_CLAWS, SEA_HAG_DEATH_GLARE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sea Hag — CR 2 medium fey. Brackish-water sister to the Green Hag, with
/// a soul-rending stare in place of the green hag's illusion kit. Slots
/// next to Dryad (CR 1) and Green Hag (CR 3) on the low-tier fey bench
/// — fills the CR 2 fey slot between them.
///
/// Action lanes:
/// - **sea hag claws** — STR-based 1d4+STR slashing melee. The steady
///   damage tap; vanilla `SimpleWeapon`.
/// - **death glare** — single-target WIS save (DC 11) at 30ft (12 tiles).
///   On fail the target takes 6d6 psychic damage. RAW reduces a Frightened
///   target to 0 HP outright; we approximate via a heavy psychic hit
///   since the engine doesn't yet expose a "would-be-killed-by" gate at
///   the side-effect layer. The 6d6 max maps cleanly to the hag's CR-2
///   power curve — heavy single-target burst but not auto-kill.
///
/// Defensive identity: AC 14 (natural armor — the sea hag's barnacle-
/// crusted hide). No magic resistance, no broad damage modifiers — the
/// sea hag is a glass cannon controller. Her load-bearing tactical
/// clause is the Death Glare burst; in melee she's a CR 2 stat block,
/// vulnerable to focused fire.
///
/// Stat shape: AC 14, ~52 HP (7d8+21), STR 16, DEX 13, CON 16, INT 12,
/// WIS 12, CHA 13. Speed 30, Swim 40. Senses: Darkvision 60. Languages:
/// Aquan, Common, Giant. Size Medium. CR 2.
///
/// RAW also gives the sea hag:
/// - **Amphibious** (breathes air + water) — no in-engine consumer (the
///   engine doesn't model breathing / drowning).
/// - **Horrific Appearance** (each enemy that starts its turn within
///   30ft + can see the hag rolls WIS save or takes 1d6 psychic +
///   Frightened until end of turn) — would need a start-of-turn aura
///   hook. The engine has `round_end` and `reset_for_new_round` hooks
///   for the actor itself, but no "any actor adjacent to me at start
///   of their turn → roll save → take damage" chain. Omitted; the
///   Death Glare lane covers the load-bearing fear-mortality clause.
/// - **Death Glare** (RAW: reduces Frightened target to 0 HP) — modeled
///   as a heavy psychic burst (see action notes above).
/// - **Illusory Appearance** (passive shape-shift) — no in-engine
///   consumer; the engine doesn't model identity / disguise checks.
/// - **Innate Spellcasting** — omitted per the same convention as Dryad /
///   Couatl: innate caster picks don't surface through the action
///   chassis.
pub static SEA_HAG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SEA_HAG_CLAWS);
    actions.push(&*SEA_HAG_DEATH_GLARE);
    CreatureTemplate {
        name: "Sea Hag",
        // 'h' (lowercase) — distinct from 'H' (used for the Helmed Horror /
        // Harpy / Hippogriff / Hobgoblin family). Same letter as the
        // Green Hag's lowercase 'h' since both are hags; the species is
        // disambiguated by name in the UI / target picker.
        glyph: 'h',
        ac: 14,
        // 7d8+21 ≈ 52 average per MM (CR 2).
        hitpoints: "7d8+21".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 12,
        dexterity: 13,
        wisdom: 12,
        constitution: 16,
        charisma: 13,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // RAW languages: Aquan, Common, Giant. The engine doesn't yet
        // enumerate Aquan (subsumed under Primordial); we surface Common
        // + Giant + Primordial as the closest analog.
        languages: HashSet::from([
            Language::Common,
            Language::Giant,
            Language::Primordial,
        ]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
    fn sea_hag_template_shape() {
        let a = ActorInstance::from_creature_template(
            &SEA_HAG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Medium);
        assert!(a.find_action("sea hag claws").is_some());
        assert!(a.find_action("death glare").is_some());
    }

    #[test]
    fn sea_hag_has_basic_stats() {
        let a = ActorInstance::from_creature_template(
            &SEA_HAG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // No Magic Resistance — sea hag is a CR-2 glass-cannon controller.
        assert!(!a.has_magic_resistance());
        // Fey creature type.
        assert_eq!(a.creature_type(), CreatureType::Fey);
    }
}
