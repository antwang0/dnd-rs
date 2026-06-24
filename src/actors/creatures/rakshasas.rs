use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{RAKSHASA_CLAW, RAKSHASA_MULTI};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rakshasa — CR 13 fiend boss. The shape-shifting tiger-headed fiend that
/// straddles the demon / devil divide in 5e lore. Slots between Vrock
/// (CR 6) / Glabrezu (CR 9) and Pit Fiend (CR 20) / Balor (CR 19) on the
/// fiend ladder — the mid-to-high tier social-manipulator boss whose
/// signature is **Limited Magic Immunity** (RAW: immune to spells of
/// 6th level or lower; advantage on saves vs all other spells / magical
/// effects). The engine doesn't yet model per-spell-level immunity
/// chokepoints, so the load-bearing slice collapses to flat **Magic
/// Resistance** (advantage on every save vs spells), captured in the
/// existing `has_magic_resistance` lane. The over-tuning vs RAW is on
/// the *party's* side: spells still take rather than auto-fizzling, but
/// the rakshasa has advantage on every save against them. A future
/// `spell_level_immunity` engine hook could land the RAW-strict clause.
///
/// Action lanes:
/// - **Rakshasa Multiattack** — 2 claws per Action. Standard single-
///   sub-attack Multiattack shape; each claw lands its slashing core plus
///   the necrotic rider independently.
/// - **Rakshasa Claw** (standalone) — DEX-based melee, 2d6+DEX slashing
///   core plus a 2d10 necrotic rider on hit. The cursed-touch necrotic
///   rider is the signature life-drain that marks the rakshasa as a fiend
///   rather than a beast.
///
/// Damage envelope: nonmagical B/P/S resistance (the canonical "magic
/// weapons or nothing" fiend defense). RAW also gives the rakshasa a
/// "vulnerability to piercing damage from magic weapons wielded by good
/// creatures" clause — omitted here since the engine doesn't tag wielders
/// by alignment; the load-bearing damage profile is the magic-weapon
/// gate the B/P/S resistance imposes.
///
/// Stat shape: AC 16, ~110 average HP (13d8+52), DEX 17 / CHA 20 — the
/// hyper-charismatic shapechanger statline RAW uses. Languages: Common
/// and Infernal (the rakshasa's native tongue). Senses: Darkvision 60 ft.
/// No legendary resistances or actions — RAW: rakshasas don't have the
/// legendary package, their Limited Magic Immunity is the entire anti-
/// caster defense. CR 13.
///
/// RAW also gives the rakshasa innate spellcasting (Detect Thoughts /
/// Disguise Self / Charm Person / Major Image / Suggestion / Fly /
/// Mirror Image / Plane Shift). Omitted here per the same convention
/// as Cloud Giant / Lich / Mummy Lord — the engine doesn't surface
/// monster spellcasting picks. The load-bearing per-round threat is the
/// double-claw multi plus the Magic Resistance envelope.
pub static RAKSHASA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*RAKSHASA_MULTI);
    actions.push(&RAKSHASA_CLAW);
    CreatureTemplate {
        name: "Rakshasa",
        // 'K' — distinct from 'R' (Adult Red Dragon) and unused in the
        // fiend pool. Mnemonic for "raKshasa" with the K consonant.
        glyph: 'K',
        ac: 16,
        // 13d8+52 ≈ 110 average per MM (CR 13).
        hitpoints: "13d8+52".parse().unwrap(),
        speed: 40.,
        strength: 14,
        intelligence: 13,
        dexterity: 17,
        wisdom: 16,
        constitution: 18,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 13.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // Fiend damage envelope: nonmagical B/P/S resistance. No
        // damage-type resistances beyond the physical triplet — RAW
        // rakshasas don't share the broader fire/cold/lightning resistance
        // envelope of the demon / devil chain.
        damage_modifiers: non_magical_physical_resistances([]),
        // 5e Magic Resistance — the engine's surface for the load-bearing
        // half of RAW's Limited Magic Immunity. See the template doc for
        // the simplification rationale.
        has_magic_resistance: true,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn rakshasa_has_nonmagical_physical_resistance() {
        let a = ActorInstance::from_creature_template(
            &RAKSHASA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // Necrotic / fire / etc. are NOT resisted — distinguishes the
        // rakshasa from the broader demon / devil envelope.
        assert_eq!(a.damage_modifier(DamageType::Fire), None);
        assert_eq!(a.damage_modifier(DamageType::Necrotic), None);
    }

    #[test]
    fn rakshasa_has_magic_resistance() {
        let a = ActorInstance::from_creature_template(
            &RAKSHASA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
    }

    #[test]
    fn rakshasa_has_double_claw_multi() {
        let a = ActorInstance::from_creature_template(
            &RAKSHASA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("rakshasa multiattack").is_some());
        assert!(a.find_action("rakshasa claw").is_some());
        assert_eq!(a.cr(), 13.0);
        assert!(a.has_extra_attack());
    }
}
