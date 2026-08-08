use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DEVA_HEALING_TOUCH, DEVA_MACE, DEVA_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Deva — CR 10 medium celestial. The "lesser angel" tier of the
/// celestial ladder: heavenly messenger, smiter of fiends and undead,
/// stationed between the Couatl (CR 4) / Unicorn (CR 5) low-celestial
/// bench and the Solar (CR 21) apex. Functionally a flying paladin —
/// heavy radiant burst out of every mace swing, a once-per-fight team
/// heal that doubles as a poison/disease cleanse, plus the standard
/// celestial defense envelope (radiant immunity, Magic Resistance,
/// Truesight).
///
/// Action lanes:
/// - **deva mace** — STR-based 1d6+STR bludgeoning melee with a flat
///   4d8 radiant rider. Routes through the shared `WeaponWithRider`
///   chassis so the radiant rider's per-target resistance / immunity
///   applies cleanly — fiends and undead eat the full pile.
/// - **deva multiattack** — 2 mace swings per Action via the shared
///   `Multiattack` chassis. Each swing carries the full radiant rider.
///   Two-hit Action lands ~9 bludgeoning + ~36 radiant against a
///   medium-AC target.
/// - **deva healing touch** — single-target ally heal (4d8 HP). Gated
///   on the `"healing_touch"` recharge key (recharge 4-6 on a d6 at
///   start of turn) so it can't fire every round. The lone in-combat
///   support lane the deva carries — turns it into a battlefield medic
///   when paired with the standard celestial save defenses.
///
/// Defensive identity: AC 17 (the deva's blessed mail), 136 HP
/// (16d8+64). **Magic Resistance** (advantage on saves vs spells /
/// magical effects — standard celestial defense), radiant immunity
/// (the angelic glow absorbs its own damage type) plus the
/// non-magical physical resistance triplet (BPS halved) per RAW. The
/// celestial condition immunity envelope (Charmed / Exhausted /
/// Frightened) round out the save resilience — the deva can't be
/// fear-locked or compelled by mortal magic.
///
/// Stat shape: AC 17, ~136 HP (16d8+64), STR 18, DEX 18, CON 18,
/// INT 17, WIS 20, CHA 20. Speed 30ft walk + 90ft fly (RAW). We
/// collapse to the fly speed since the engine isn't 3D and the
/// stationary terrain doesn't gate flight. Senses: Darkvision 120,
/// Truesight (the deva sees through illusions and invisibility
/// natively — `SpecialSense::Truesight(120)`). Languages: Celestial,
/// Common. Size Medium. CR 10. XP: 5,900 per RAW.
///
/// Innate Spellcasting (Detect Evil and Good at-will, Commune /
/// Raise Dead 1/day, etc.) is omitted per the same convention as
/// Couatl / Unicorn — innate caster picks don't surface through the
/// engine's action chassis and the load-bearing combat clauses (mace
/// + healing touch) already define the deva's per-round footprint.
pub static DEVA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DEVA_MACE);
    actions.push(&*DEVA_MULTI);
    actions.push(&*DEVA_HEALING_TOUCH);
    CreatureTemplate {
        name: "Deva",
        // '✦' (four-pointed star, U+2726) — the angelic-radiance
        // silhouette. Distinct from the existing celestial pool: 'O'
        // (Solar), 'U' (Unicorn). The star glyph reads as the deva's
        // signature halo / aura even at small UI scale. Falls back to
        // 'V' (untaken) if rendering collisions surface.
        glyph: '✦',
        ac: 17,
        // 16d8+64 ≈ 136 average per MM (CR 10).
        hitpoints: "16d8+64".parse().unwrap(),
        speed: 90.,
        strength: 18,
        intelligence: 17,
        dexterity: 18,
        wisdom: 20,
        constitution: 18,
        charisma: 20,
        senses: HashSet::from([
            SpecialSense::Darkvision(120),
            SpecialSense::Truesight(120),
        ]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 10.0,
        size: Size::Medium,
        creature_type: CreatureType::Celestial,
        actions,
        // CR-10 RAW saves: WIS + CHA proficient. The deva's anti-magic
        // posture leans on its Wisdom / Charisma envelope alongside
        // Magic Resistance — most save-or-suck spells dump on WIS / CHA
        // and the deva shrugs them off.
        proficient_saves: HashSet::from([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Deva resistances per MM: radiant immunity, plus non-magical
        // physical resistance (we don't model the magical-vs-mundane
        // split, so we apply flat resistance to BPS — matches the
        // dominant party loadout where most damage is non-magical at
        // CR 10).
        damage_modifiers: HashMap::from([
            (DamageType::Radiant, DamageModifier::Immunity),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Deva condition immunities per MM: Charmed, Exhausted,
        // Frightened. The celestial purity envelope — can't be
        // compelled, fatigued, or fear-locked by mortal magic.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
        ]),
        // 5e Magic Resistance — advantage on saves vs spells / magical
        // effects. Standard celestial defense lane shared with Couatl,
        // Unicorn, and Solar.
        has_magic_resistance: true,
        // Healing Touch recharge — d6 with min_roll 4 means it
        // recharges on 4, 5, or 6 at start of turn. RAW is "1/day"; the
        // recharge envelope is the engine's closest approximation. One
        // notch easier than the unicorn's 5-6 because the deva's CR-10
        // role leans more on sustained healing than the unicorn's
        // burst-heal niche.
        recharge_abilities: vec![("healing_touch", 4)],
        // 5e **Angelic Weapons**: "the deva's weapon attacks are magical."
        features: HashSet::from([crate::actions::class_features::MAGICAL_ATTACKS_TAG]),
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
            &DEVA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn deva_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 10.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert!(a.find_action("deva mace").is_some());
        assert!(a.find_action("deva multiattack").is_some());
        assert!(a.find_action("deva healing touch").is_some());
    }

    #[test]
    fn deva_has_celestial_envelope() {
        let a = make();
        // Radiant immunity — the angelic glow shrugs off its own type.
        assert_eq!(
            a.damage_modifier(DamageType::Radiant),
            Some(DamageModifier::Immunity)
        );
        // Non-magical physical resistance.
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
        // Condition immunities: Charmed, Exhausted, Frightened.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Exhausted));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        // Magic Resistance — advantage on saves vs magical effects.
        assert!(a.has_magic_resistance());
    }

    #[test]
    fn deva_healing_touch_starts_recharged() {
        let a = make();
        // Healing touch should be available at instantiation. The
        // recharge chassis sets every entry to `available=true` at
        // spawn time.
        assert!(a.is_recharge_available("healing_touch"));
    }

    #[test]
    fn deva_template_pins_truesight() {
        // Pin the load-bearing defensive clause: the deva's Truesight
        // is what flavors it as a credible counter to invisible /
        // illusion-wrapped enemies. A future refactor of the sense
        // set shouldn't strip this template field. Routes through the
        // instance-level `senses()` accessor so a template-internal
        // sense refactor that moves the field but preserves the
        // semantic behavior still passes the pin.
        let a = make();
        assert!(
            a.senses().iter().any(|s| matches!(s, SpecialSense::Truesight(120))),
            "deva must carry Truesight 120 — the celestial perception lane",
        );
        // And the load-bearing behavioral clause: `has_truesight`
        // returns true so the compute_attack_mode illusion-suppression
        // gate actually fires for a deva. Without this the template
        // sense would be a flavor field with no in-engine effect.
        assert!(
            a.has_truesight(),
            "deva must report has_truesight() — the gate the engine reads"
        );
    }
}
