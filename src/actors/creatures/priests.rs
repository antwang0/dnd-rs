use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PRIEST_MACE;
use crate::actions::spells::{BLESS, CURE_WOUNDS, GUIDING_BOLT, SACRED_FLAME, SPIRITUAL_WEAPON};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Priest — CR 2 medium humanoid. The acolyte's superior, and the rung
/// where a temple stops being a speed bump. Two slot tiers instead of
/// one, a second-level Spiritual Weapon that keeps swinging after the
/// priest has moved on to healing, and Guiding Bolt to hand the melee
/// line its advantage.
///
/// Action lanes:
/// - **sacred flame** — the at-will radiant cantrip.
/// - **guiding bolt** — 4d6 radiant and advantage on the next attack
///   against the target. The priest's contribution to somebody else's
///   damage, which at CR 2 is worth more than its own.
/// - **cure wounds** — the undo button.
/// - **bless** — the party-wide +1d4.
/// - **spiritual weapon** — a level-2 conjured blade that attacks on
///   the priest's bonus action for the rest of the fight. The single
///   most efficient thing on the sheet: cast once, paid for once, swung
///   every round after.
/// - **priest mace** — the fallback.
///
/// **Divine Eminence** — RAW's "as a bonus action, the priest can expend
/// a spell slot to cause its melee weapon attacks to magically deal an
/// extra 10 (3d6) radiant damage" — is deliberately absent, and
/// Spiritual Weapon is why. Both spend a slot on a bonus action to add
/// damage; the engine already carries one of them exactly, and the one
/// it carries is the one a priest with a mace and STR 16 should
/// actually be casting.
///
/// Stat shape per the SRD NPC appendix: AC 13 (chain shirt), 38 HP
/// (7d8+7), STR 16 / DEX 10 / CON 12 / INT 13 / WIS 16 / CHA 13. Speed
/// 30 — SRD 5.2 stopped charging the priest for its mail. Skills: Medicine,
/// Persuasion, Religion. CR 2. Slots: 4 × level 1, 3 × level 2.
pub static PRIEST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*GUIDING_BOLT);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*BLESS);
    actions.push(&*SPIRITUAL_WEAPON);
    actions.push(&PRIEST_MACE);
    CreatureTemplate {
        name: "Priest",
        // 'p' (lowercase) — free in the humanoid band. 'P' is the Pit
        // Fiend / Pegasus / Phase Spider pool.
        glyph: 'p',
        ac: 13,
        // 7d8+7 ≈ 38 average per the SRD NPC appendix (CR 2).
        hitpoints: "7d8+7".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 10,
        constitution: 12,
        intelligence: 13,
        wisdom: 16,
        charisma: 13,
        skills: HashSet::from([Skill::Medicine, Skill::Persuasion, Skill::Religion]),
        languages: HashSet::from([Language::Common, Language::Celestial]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: vec![4, 3],
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
            &PRIEST_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn priest_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("priest mace").is_some());
    }

    /// A second slot tier is what separates the priest from the acolyte,
    /// and Spiritual Weapon is the spell that tier is for. Asserting
    /// both together is the claim: a priest with the spell and no
    /// level-2 slot could never cast it, and the sheet would still read
    /// as if it could.
    #[test]
    fn the_priest_can_actually_afford_its_level_two_spell() {
        let a = make();
        assert!(a.find_action("spiritual weapon").is_some());
        assert_eq!(a.spell_slot_manager.spell_slots(1).spell_slots, 4);
        assert_eq!(a.spell_slot_manager.spell_slots(2).spell_slots, 3);
        assert_eq!(a.spell_slot_manager.spell_slots(3).spell_slots, 0);
    }
}
