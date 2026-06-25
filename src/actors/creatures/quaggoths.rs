use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{QUAGGOTH_CLAW, QUAGGOTH_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Quaggoth — CR 2 medium humanoid. The bear-thrall race of the
/// Underdark: hulking, fur-covered, near-feral berserker that fights
/// with two claws and a pack-bond mentality. Sits in the CR-2 humanoid
/// slot between the bugbear (CR 1) and the gnoll pack lord (CR 2)
/// brute bench — distinctive for being a two-claw natural-weapon
/// fighter rather than the typical armed humanoid lane.
///
/// Action lanes:
/// - **quaggoth claw** — STR-based 1d6+STR slashing melee. The
///   standalone single-swing exposed for bonus-action-tagged turns or
///   moving in.
/// - **quaggoth multiattack** — 2 claw rakes per Action via the
///   homogeneous `Multiattack` chassis. RAW: "The quaggoth makes two
///   claw attacks." Pure damage output — no rider, no save-or-suck.
///
/// Defensive identity: AC 13 (the matted fur is the only armor a
/// quaggoth wears), 45 HP (6d8+18). Poison resistance is the
/// signature defensive trait (RAW: "The quaggoth has resistance to
/// poison damage" — Underdark-adapted physiology). No other damage
/// modifiers or condition immunities; the quaggoth's identity is
/// pure brute, not arcane holdover.
///
/// **Wounded Fury** (RAW: "While it has 10 hit points or fewer, the
/// quaggoth has advantage on attack rolls and deals an extra 7 (2d6)
/// damage with melee attacks") is omitted as a deliberate scope cut.
/// The engine doesn't yet have a generic "below HP threshold → grant
/// attacker-side advantage + bonus damage" hook on the actor side,
/// and adding one for a single low-CR creature would be over-scope.
/// The two-claw multiattack still carries the quaggoth's per-Action
/// threat envelope (~9 average per swing, ~18 per Action vs medium AC).
///
/// Stat shape: AC 13, ~45 HP (6d8+18), STR 17, DEX 12, CON 16,
/// INT 6, WIS 12, CHA 7. Speed 30ft walk + 30ft climb (RAW — we
/// collapse to the walking speed since the engine isn't 3D and the
/// climb speed only matters on vertical terrain we don't model).
/// Senses: Darkvision 120 (the deep-Underdark adaptation). Languages:
/// Undercommon (collapsed from the RAW Quaggoth language family).
/// Size Medium. CR 2.
pub static QUAGGOTH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&QUAGGOTH_CLAW);
    actions.push(&*QUAGGOTH_MULTI);
    CreatureTemplate {
        name: "Quaggoth",
        // 'q' (lowercase) — distinct mnemonic for Quaggoth. 'Q' is
        // taken by Quasit; lowercase 'q' lets both share the alphabet
        // without glyph collisions. Reads as a hunched, four-limbed
        // silhouette at small UI scale.
        glyph: 'q',
        ac: 13,
        // 6d8+18 ≈ 45 average per MM (CR 2).
        hitpoints: "6d8+18".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 6,
        dexterity: 12,
        wisdom: 12,
        constitution: 16,
        charisma: 7,
        senses: std::collections::HashSet::from([SpecialSense::Darkvision(120)]),
        languages: std::collections::HashSet::new(),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Poison damage resistance — Underdark-adapted physiology
        // shrugs off the toxin lane at half value.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Resistance),
        ]),
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
            &QUAGGOTH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn quaggoth_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("quaggoth claw").is_some());
        assert!(a.find_action("quaggoth multiattack").is_some());
    }

    #[test]
    fn quaggoth_has_poison_resistance() {
        let a = make();
        // Poison resistance — the Underdark-adapted physiology
        // halves toxin damage.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Resistance)
        );
        // No other modifiers — the quaggoth is a pure brute, not an
        // arcane holdover.
        assert_eq!(a.damage_modifier(DamageType::Fire), None);
        assert_eq!(a.damage_modifier(DamageType::Cold), None);
        assert_eq!(a.damage_modifier(DamageType::Radiant), None);
    }
}
