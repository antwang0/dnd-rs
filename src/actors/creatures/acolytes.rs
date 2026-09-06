use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ACOLYTE_CLUB;
use crate::actions::spells::{BLESS, CURE_WOUNDS, SACRED_FLAME, SANCTUARY};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Acolyte — CR ¼ medium humanoid. The junior temple caster, and the
/// bestiary's cheapest source of the two things a party has to plan
/// around rather than out-damage: a heal that undoes a round of work,
/// and a ward that makes one creature un-hittable for a turn.
///
/// Action lanes:
/// - **sacred flame** — the at-will radiant cantrip, DEX-save-for-half.
///   What the acolyte does when nothing needs help.
/// - **cure wounds** — the reason it is on the board. Three level-1
///   slots is three rounds of undoing somebody's hit.
/// - **bless** — +1d4 to allies' attacks and saves. The cheapest force
///   multiplier in the game and the one that makes a pack of cultists
///   dangerous rather than merely numerous.
/// - **sanctuary** — the ward, on an ally the party is about to finish.
/// - **acolyte club** — what is left when the slots are gone.
///
/// Stat shape per the SRD NPC appendix: AC 13, 11 HP (2d8+2), STR 14 / DEX
/// 10 / CON 10 / INT 10 / WIS 14 / CHA 11. Speed 30. Spellcasting is
/// WIS-based, which is both RAW and the engine's single spell ability,
/// so the acolyte is one of the few casters whose sheet needs no
/// translation. Skills: Medicine, Religion. CR ¼.
///
/// The three level-1 slots are the whole budget — no cantrip-only
/// fallback tier and no second rung. An acolyte that has spent them is
/// a commoner with a stick, which is the correct end state for a stat
/// block whose CR is a quarter.
pub static ACOLYTE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*BLESS);
    actions.push(&*SANCTUARY);
    actions.push(&ACOLYTE_CLUB);
    CreatureTemplate {
        name: "Acolyte",
        // 'c' (lowercase) — the temple-tier caster glyph, shared with
        // the Cultist that most often stands opposite it. 'C' is the
        // Cleric / Chimera / Chuul band.
        glyph: 'c',
        ac: 13,
        // 2d8+2 ≈ 11 average per the SRD NPC appendix (CR ¼).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 14,
        dexterity: 10,
        constitution: 12,
        intelligence: 10,
        wisdom: 14,
        charisma: 11,
        skills: HashSet::from([Skill::Medicine, Skill::Religion]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: vec![3],
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
            &ACOLYTE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn acolyte_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("acolyte club").is_some());
    }

    /// The acolyte's whole reason to exist is the support lane, and all
    /// four of its spells have to be reachable for it to be anything
    /// other than a commoner. Named individually rather than counted:
    /// a count passes just as happily when the heal has been swapped
    /// for a second cantrip.
    #[test]
    fn the_acolyte_carries_the_support_kit_it_is_on_the_board_for() {
        let a = make();
        assert!(a.find_action("sacred flame").is_some());
        assert!(a.find_action("cure wounds").is_some());
        assert!(a.find_action("bless").is_some());
        assert!(a.find_action("sanctuary").is_some());
        // Three level-1 slots and nothing above them.
        assert_eq!(a.spell_slot_manager.spell_slots(1).spell_slots, 3);
        assert_eq!(a.spell_slot_manager.spell_slots(2).spell_slots, 0);
    }
}
