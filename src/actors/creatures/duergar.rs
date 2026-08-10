use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DUERGAR_JAVELIN, DUERGAR_WAR_PICK};
use crate::actions::spells::{ENLARGE_REDUCE, INVISIBILITY};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Duergar — CR 1 medium humanoid (gray dwarf). A CR 1 body carrying two
/// spells that belong three CRs higher, and the trade RAW makes for them
/// is that the duergar cannot fight in daylight.
///
/// Action lanes:
/// - **enlarge** — RAW's Enlarge, once per day, on itself. The engine's
///   `Enlarged` condition is the real spell: one size up, advantage on
///   STR checks and saves, and an extra die on every weapon hit. RAW's
///   duergar prints the enlarged damage on each weapon line; the engine
///   gets it from the condition instead, which is why neither weapon
///   below restates it.
/// - **invisibility** — RAW's Invisibility, once per day, on itself.
///   The other half of the duergar's reputation, and the reason a
///   scouting party never sees the first one.
/// - **duergar war pick** — 1d8+STR piercing in contact.
/// - **duergar javelin** — 1d6+STR piercing at twelve tiles.
///
/// **Duergar Resilience** — RAW: "advantage on saving throws against
/// poison, spells, and illusions, as well as to resist being charmed or
/// paralyzed" — lands as `has_magic_resistance` plus poison resistance.
/// The magic-resistance flag is the engine's "advantage on saves against
/// spells", which is the load-bearing two-thirds of the clause; the
/// charm and paralysis halves are inside it whenever the source is a
/// spell, which in this engine is nearly always.
///
/// **Sunlight Sensitivity** is the price: disadvantage on attack rolls
/// while standing in sunlight. Carried at the `Sensitivity` tier — the
/// kobold-and-drow rung, attack rolls only — which is exactly what RAW
/// gives a gray dwarf.
///
/// Stat shape per the SRD: AC 16 (scale mail, shield), 26 HP (4d8+8),
/// STR 14 / DEX 11 / CON 14 / INT 11 / WIS 10 / CHA 9. Speed 25.
/// Darkvision 120 — the deepest in the humanoid band, and the sense that
/// makes the sunlight clause a fair trade. Languages: Dwarvish,
/// Undercommon. CR 1.
pub static DUERGAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ENLARGE_REDUCE);
    actions.push(&*INVISIBILITY);
    actions.push(&DUERGAR_WAR_PICK);
    actions.push(&DUERGAR_JAVELIN);
    CreatureTemplate {
        name: "Duergar",
        // 'u' (lowercase) — an unclaimed letter, and the one the word
        // is recognisable by. 'd' / 'D' are the dragon and drake pools
        // and 'g' is the goblinoid band.
        glyph: 'u',
        ac: 16,
        // 4d8+8 ≈ 26 average per the SRD (CR 1).
        hitpoints: "4d8+8".parse().unwrap(),
        speed: 25.,
        strength: 14,
        dexterity: 11,
        constitution: 14,
        intelligence: 11,
        wisdom: 10,
        charisma: 9,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Dwarvish, Language::Undercommon]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // One slot at each of the two tiers the innate spells sit on —
        // Invisibility and Enlarge are both level 2, and RAW gives the
        // duergar one casting of each per day.
        spell_slots_by_level: vec![0, 2],
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Resistance)]),
        has_magic_resistance: true,
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
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
            &DUERGAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn duergar_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("duergar war pick").is_some());
        assert!(a.find_action("duergar javelin").is_some());
    }

    /// Both innate spells and the slots to pay for them. The pairing is
    /// the assertion: a duergar carrying Enlarge with an empty level-2
    /// tier has a line on its sheet that can never resolve, which is
    /// the exact shape of bug a stat block cannot show you.
    #[test]
    fn the_duergar_can_afford_both_of_its_once_a_day_spells() {
        let a = make();
        assert!(a.find_action("enlarge").is_some());
        assert!(a.find_action("invisibility").is_some());
        assert_eq!(a.spell_slot_manager.spell_slots(2).spell_slots, 2);
        assert_eq!(a.spell_slot_manager.spell_slots(1).spell_slots, 0);
    }

    /// The trade: 120 feet of darkvision against disadvantage in the
    /// sun. Both sides asserted, because a duergar with the frailty and
    /// no darkvision is simply a worse dwarf.
    #[test]
    fn the_duergar_pays_for_its_darkvision_with_daylight() {
        let a = make();
        assert_eq!(a.sunlight_frailty(), Some(SunlightFrailty::Sensitivity));
        assert!(
            a.senses()
                .iter()
                .any(|s| matches!(s, SpecialSense::Darkvision(r) if *r >= 120)),
            "the gray dwarf sees further underground than anything else in its band"
        );
    }
}
