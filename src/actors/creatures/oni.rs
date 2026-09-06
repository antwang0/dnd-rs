use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ONI_CLAW, ONI_GLAIVE, ONI_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Oni — CR 7 large giant (Japanese ogre / "ogre mage"). The signature
/// boss-tier giant in the upper-mid pool: heavy glaive multiattack (reach
/// 2 via the polearm, identical envelope to the ogre's greatclub but
/// scaled to a 2d10 die), Magic Resistance, and regeneration of 10 HP at
/// end of round — the regen ticks down the same way the troll's 3/round
/// does, with no fire / radiant suppression (the oni's RAW recipe is "no
/// suppressor, just a flat heal" — we keep parity for simplicity).
///
/// Stats roughly track MM Oni at CR 7 — STR 19 (+4) drives the glaive's
/// to-hit / damage, CON 16 gives a solid HP pool (119 HP at 14d10+42),
/// CHA 15 hints at the RAW spellcasting flavor (the engine doesn't
/// surface the spell list — the oni's identity is the polearm reach).
/// Proficient saves on DEX / CON / WIS / CHA per RAW. Senses include
/// Darkvision 60.
///
/// Damage profile: no template-level resistances or immunities (the RAW
/// oni doesn't have any). The regen + Magic Resistance + heavy glaive
/// dice give the chassis its survival envelope; spellcasting / ranged
/// flavor sits in the "future polish" bucket.
pub static ONI_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ONI_GLAIVE);
    actions.push(&ONI_CLAW);
    actions.push(&*ONI_MULTI);
    CreatureTemplate {
        name: "Oni",
        // 'O' (uppercase) — was free; uppercase 'O' has a horns / ogre-
        // mask silhouette that fits the giant-tier glyph slot. Distinct
        // from Ogre's 'g' (lowercase Goblin/Gargoyle family) and Owlbear's
        // 'o' (lowercase beast slot).
        glyph: 'O',
        ac: 17,
        // 14d10+42 = 119 average per MM.
        hitpoints: "14d10+42".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 30 ft.
        speed: 30.0,
        fly_speed: 30.0,
        strength: 19,
        intelligence: 14,
        dexterity: 11,
        wisdom: 12,
        constitution: 16,
        charisma: 15,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 7.0,
        size: Size::Large,
        creature_type: CreatureType::Giant,
        actions,
        // 5e Oni proficient saves: DEX / CON / WIS / CHA.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // 10 HP per round; no suppressor (RAW oni has no fire / acid
        // suppression like the troll). Mirrors the troll regen lane —
        // `EncounterInstance::round_end` ticks the heal while the oni is
        // combat-active.
        regen_per_round: 10,
        // 5e Magic Resistance — advantage on every save vs spells /
        // magical effects. Read by `compute_save_mode`.
        has_magic_resistance: true,
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
    fn oni_has_magic_resistance_and_regen() {
        let a = ActorInstance::from_creature_template(
            &ONI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
        assert_eq!(a.regen_per_round(), 10);
    }

    #[test]
    fn oni_has_heavy_hp_pool() {
        let a = ActorInstance::from_creature_template(
            &ONI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // 14d10+42 averages 119 HP; the CR-7 chassis should be well above
        // the lower-tier giant baseline.
        assert!(a.max_hitpoints() >= 70);
    }
}
