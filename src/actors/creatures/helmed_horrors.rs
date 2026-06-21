use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HELMED_HORROR_LONGSWORD, HELMED_HORROR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Helmed Horror — CR 4 medium construct. An animated suit of plate +
/// shield piloted by sentient magic. Slots above Animated Armor (CR 1)
/// and below Stone Golem (CR 10) on the construct ladder — the mid-tier
/// "anti-caster" construct whose signature is a fat spell-immunity
/// envelope (RAW: immune to Fireball / Heat Metal / Lightning Bolt
/// outright plus Magic Resistance against everything else). The engine
/// doesn't yet model per-spell immunity chokepoints, so the load-bearing
/// slice collapses to flat Magic Resistance (advantage on every save
/// vs spells) plus damage immunity to the three spell-flavored types
/// the construct never feels: Force, Necrotic, and Poison.
///
/// Action lanes:
/// - **Helmed Horror Multiattack** — 2 longsword swings per Action.
///   Vanilla single-sub-attack Multiattack; each swing rolls its own
///   d20 + STR vs AC for the canonical CR-4 construct double-strike.
/// - **Helmed Horror Longsword** (standalone) — STR-based 2d8+STR
///   slashing melee. Provided so the AI can fall back to a single
///   swing when bonus-action-tagged or moving in.
///
/// Damage envelope: Force / Necrotic / Poison damage immunity (the
/// canonical construct + spell-immunity envelope). No nonmagical B/P/S
/// resistance — the helmed horror is the construct counterpart to the
/// rakshasa: weak to physical, strong against magic. Senses: Blindsight
/// 60 ft (the canonical "blind beyond" construct envelope — the helm
/// has no eyes, just the magical sight that perceives the world).
///
/// Condition immunities: the full construct envelope (Blinded, Charmed,
/// Deafened, Frightened, Paralyzed, Petrified, Poisoned, Stunned). The
/// helm has no eyes / ears / mind / metabolism / joints to control.
///
/// Stat shape: AC 20 (plate + shield), ~60 average HP (8d8+24), STR 18,
/// DEX 13, CON 16. No languages — the helm understands the languages
/// of its creator but cannot speak. CR 4.
///
/// RAW also gives the helmed horror flight (30 ft) — we omit the flight
/// since the engine's 2D grid doesn't model 3D positioning; the
/// load-bearing combat clause is the spell-immunity envelope plus the
/// 2-longsword multi.
pub static HELMED_HORROR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HELMED_HORROR_MULTI);
    actions.push(&HELMED_HORROR_LONGSWORD);
    CreatureTemplate {
        name: "Helmed Horror",
        // 'X' — distinct from 'I' (Animated Armor) and the 'G' / 'g'
        // golem pool. Mnemonic: an empty helm gazing back, the "X" of
        // crossed enchanted blades.
        glyph: 'X',
        ac: 20,
        // 8d8+24 ≈ 60 average per MM (CR 4).
        hitpoints: "8d8+24".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 10,
        dexterity: 13,
        wisdom: 10,
        constitution: 16,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Construct,
        actions,
        // Force / Necrotic / Poison damage immunity — the canonical
        // construct + spell-immunity envelope. No B/P/S resistance: the
        // helmed horror's defense is "spells bounce off, but physical
        // weapons bite normally" — the inverse of the rakshasa's
        // anti-physical / anti-magic split.
        damage_modifiers: HashMap::from([
            (DamageType::Force, DamageModifier::Immunity),
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Standard construct condition immunity envelope — no eyes / ears
        // / mind / metabolism / joints means the holder shrugs off the
        // body-control debuffs.
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Stunned,
            Condition::Exhausted,
        ]),
        // 5e Magic Resistance — the load-bearing slice of RAW's full
        // spell-immunity envelope (advantage on every save vs spells).
        // Captures the "anti-caster" identity that distinguishes the
        // helmed horror from a vanilla animated armor.
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
    use crate::engine::types::Coordinate;

    #[test]
    fn helmed_horror_has_double_longsword_multi() {
        let a = ActorInstance::from_creature_template(
            &HELMED_HORROR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("helmed horror multiattack").is_some());
        assert!(a.find_action("helmed horror longsword").is_some());
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Medium);
        assert!(a.has_extra_attack());
    }

    #[test]
    fn helmed_horror_has_spell_immunity_envelope() {
        let a = ActorInstance::from_creature_template(
            &HELMED_HORROR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The signature spell-flavor damage immunity envelope.
        assert_eq!(
            a.damage_modifier(DamageType::Force),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Necrotic),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        // Physical types are NOT resisted — the helmed horror's defense
        // is anti-magic, not anti-weapon.
        assert_eq!(a.damage_modifier(DamageType::Slashing), None);
        assert_eq!(a.damage_modifier(DamageType::Bludgeoning), None);
        // Magic Resistance for the broader save-advantage envelope.
        assert!(a.has_magic_resistance());
    }

    #[test]
    fn helmed_horror_has_construct_condition_immunities() {
        let a = ActorInstance::from_creature_template(
            &HELMED_HORROR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Body-control debuffs all bounce off the spell-piloted plate.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Stunned));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
