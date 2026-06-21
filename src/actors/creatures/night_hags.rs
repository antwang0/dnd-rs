use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{NIGHT_HAG_CLAWS, NIGHT_HAG_MULTI};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Night Hag — CR 5 medium fiend. The apex of the hag trio (Sea Hag CR 2,
/// Green Hag CR 3, Night Hag CR 5) — an infernal soul-stealer with Magic
/// Resistance, the standard non-magical-BPS resistance envelope, cold +
/// fire resistance, and Charmed condition immunity. Slots between
/// Vampire Spawn (CR 5) and Unicorn (CR 5) on the mid-tier ladder — the
/// fiend-flavored answer to the celestial Unicorn at the same CR.
///
/// Action lanes:
/// - **night hag claws** — STR-based 2d8+STR slashing melee. The hag's
///   signature rending swing in hag form. Vanilla `SimpleWeapon` — the
///   load-bearing identity is the defensive envelope, not any per-hit
///   rider.
/// - **night hag multiattack** — 2 claw swings per Action. The canonical
///   CR-5 melee burst: ~26 average per turn before crits.
///
/// Defensive identity: AC 17 (natural armor — the hag's gnarled hide),
/// 112 HP (15d8+45). Resistance to cold + fire (the night hag thrives in
/// the infernal cold of the Hells and the smoldering ash of the Lower
/// Planes) plus the standard non-magical-BPS resistance envelope (the
/// hag's fiendish flesh shrugs off mundane weapons RAW — "from
/// nonmagical attacks that aren't silvered"; we collapse to the BPS
/// triple since we don't track silvered-weapon typing). Magic Resistance
/// — advantage on saves vs spells / magical effects, the canonical
/// fiend-tier saver. Charmed condition immunity — the hag's mind is too
/// alien to coerce.
///
/// Stat shape: AC 17, ~112 HP (15d8+45), STR 18, DEX 15, CON 16, INT 16,
/// WIS 14, CHA 16. Speed 30. Senses: Darkvision 120 (the hag sees
/// perfectly in the Underdark and the Lower Planes). Languages: Abyssal,
/// Common, Infernal, Primordial — the full infernal-diplomat envelope.
/// Size Medium. CR 5.
///
/// RAW also gives the night hag:
/// - **Innate Spellcasting** (Detect Magic, Magic Missile, etc.) —
///   omitted per the same convention as Cambion / Couatl / Drider /
///   Unicorn: innate caster picks don't surface through the engine's
///   action chassis.
/// - **Change Shape** (passive shape-shift to a small / medium humanoid)
///   — no in-engine consumer (we don't model identity / disguise
///   checks).
/// - **Etherealness** (passive Ethereal-plane access) — no in-engine
///   consumer (we don't model multi-plane geometry).
/// - **Nightmare Haunting** (dream-haunting from the Ethereal Plane) —
///   requires Etherealness; omitted as a consequence.
/// - **Soul Bag** — the hag's signature trophy item (a captured soul
///   bag worth 1d6 × 1,000 gp) — no in-engine consumer (we don't model
///   the soul economy of the Lower Planes).
pub static NIGHT_HAG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&NIGHT_HAG_CLAWS);
    actions.push(&*NIGHT_HAG_MULTI);
    CreatureTemplate {
        name: "Night Hag",
        // 'N' (uppercase) — distinct from 'n' (Nightmare uses 'N' too;
        // we share the glyph since both are CR-3+ infernal-flavored
        // creatures and the silhouette pool is exhausted at single
        // letters that read as "infernal-coded predator").
        glyph: 'N',
        ac: 17,
        // 15d8+45 ≈ 112 average per MM (CR 5).
        hitpoints: "15d8+45".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 16,
        dexterity: 15,
        wisdom: 14,
        constitution: 16,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([
            Language::Abyssal,
            Language::Common,
            Language::Infernal,
            Language::Primordial,
        ]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // Cold + fire resistance on top of the non-magical-BPS triple —
        // the hag's infernal physiology bites back at the elemental
        // edges. `non_magical_physical_resistances` returns the BPS
        // triple as a fresh map; we layer the two elemental entries
        // on top via the `overlays` argument.
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
        ]),
        // The hag's mind is too alien / hardened to coerce — RAW Charmed
        // condition immunity (we omit the RAW "Frightened immunity"
        // since the night hag stat block doesn't list it).
        condition_immunities: HashSet::from([Condition::Charmed]),
        // Magic Resistance — advantage on saves vs spells / magical
        // effects. Standard mid-tier fiend defensive lane.
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
    fn night_hag_template_shape() {
        let a = ActorInstance::from_creature_template(
            &NIGHT_HAG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        assert!(a.find_action("night hag claws").is_some());
        assert!(a.find_action("night hag multiattack").is_some());
    }

    #[test]
    fn night_hag_has_magic_resistance_and_charm_immunity() {
        let a = ActorInstance::from_creature_template(
            &NIGHT_HAG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Magic Resistance — advantage on saves vs spells / magical
        // effects. The mid-tier fiend defensive lane.
        assert!(a.has_magic_resistance());
        // Charmed condition immunity — the hag's mind is too alien.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
    }

    #[test]
    fn night_hag_resists_cold_fire_and_nonmagical_physical() {
        let a = ActorInstance::from_creature_template(
            &NIGHT_HAG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Cold + fire resistance — the infernal physiology lane.
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Resistance)
        );
        // Non-magical-BPS resistance — RAW "from nonmagical attacks that
        // aren't silvered"; we collapse to the BPS triple since we
        // don't track silvered-weapon typing.
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
    }
}
