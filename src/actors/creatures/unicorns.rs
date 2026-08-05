use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    UNICORN_HEALING_TOUCH, UNICORN_HOOVES, UNICORN_HORN, UNICORN_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Unicorn — CR 5 large celestial. Iconic horned equine, healer-warrior
/// hybrid. Slots between Couatl (CR 4) and Vampire Spawn (CR 5) on the
/// mid-tier ladder — the only large celestial in the pool until the
/// Solar's apex slot opens at CR 21. Load-bearing per-round threat is
/// the **horn + hooves multi** (heavy mid-tier melee damage) plus the
/// recharge-gated **Healing Touch** that restores a wounded ally on
/// the fly, turning the unicorn into a battlefield medic-paladin
/// hybrid: not the heaviest melee threat at its CR, but the only mid-
/// CR enemy that meaningfully extends combats with team healing.
///
/// Action lanes:
/// - **hooves + horn** (Multiattack) — heterogeneous CompoundAttack:
///   one 2d6 hoof kick + one 1d8 horn gore per Action. Both limbs are
///   STR-based melee.
/// - **unicorn hooves** / **unicorn horn** (standalone) — same as the
///   multi parts, exposed so the AI / player can pick a single swing
///   when bonus-action-tagged or moving in. Pairs with the team-heal
///   bonus-action lane in a future fighter-companion pickup.
/// - **healing touch** — single-target ally heal, 3d8+CHA HP. Gated on
///   the `"healing_touch"` recharge key (recharge 5-6 on a d6 at start
///   of turn) so it can't fire every round. RAW is "3/day" — the engine
///   doesn't track per-day pools, so the recharge envelope is the
///   closest approximation: still usable multiple times per fight but
///   not every round.
///
/// Defensive identity: Magic Resistance (advantage on saves vs spells
/// and magical effects — the standard celestial defense lane shared
/// with Couatl). Charmed + Paralyzed + Poisoned condition immunity
/// matches the MM Unicorn stat block. Poison damage immunity rounds
/// out the celestial immunity envelope (per RAW).
///
/// Stat shape: AC 12 (the unicorn relies on speed + Magic Resistance
/// rather than armor), 67 HP (9d10+18), STR 18, DEX 14, CON 15, INT 11,
/// WIS 17, CHA 16. Speed 50ft — among the fastest in the pool. Senses:
/// Darkvision 60. Languages: Celestial, Elvish, Sylvan. Size Large.
/// CR 5.
///
/// RAW also gives the unicorn Innate Spellcasting (Detect Evil and Good,
/// Druidcraft, Pass without Trace, etc.) plus a passive Magic Weapon
/// rider on its horn (counts as magical for resistance bypass).
/// Omitted per the same convention as Couatl / Androsphinx — innate
/// caster picks don't surface through the engine's action chassis and
/// the magical-weapon clause has no in-engine consumer (we don't track
/// attacker-side magic-weapon typing).
pub static UNICORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*UNICORN_MULTI);
    actions.push(&UNICORN_HOOVES);
    actions.push(&UNICORN_HORN);
    actions.push(&*UNICORN_HEALING_TOUCH);
    CreatureTemplate {
        name: "Unicorn",
        // 'U' (uppercase) — distinct from 'u' (Umber Hulk uses 'U' too,
        // but we collide intentionally — both are CR 5 large creatures
        // and the glyph pool is exhausted at uppercase letters that read
        // as the silhouette). The glyph is a UI hint, not a unique key.
        glyph: 'U',
        ac: 12,
        // 9d10+18 ≈ 67 average per MM (CR 5).
        hitpoints: "9d10+18".parse().unwrap(),
        speed: 50.,
        strength: 18,
        intelligence: 11,
        dexterity: 14,
        wisdom: 17,
        constitution: 15,
        charisma: 16,
        languages: HashSet::from([
            Language::Celestial,
            Language::Elvish,
            Language::Sylvan,
        ]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        // MM Unicorn: immune to poison damage (the celestial purity lane).
        damage_modifiers: std::collections::HashMap::from([
            (DamageType::Poison, crate::engine::types::DamageModifier::Immunity),
        ]),
        // MM Unicorn condition immunities: Charmed, Paralyzed, Poisoned.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Paralyzed,
            Condition::Poisoned,
        ]),
        // 5e Magic Resistance — advantage on saves vs spells / magical
        // effects. Standard celestial defense lane.
        has_magic_resistance: true,
        // Healing Touch recharge — d6 with min_roll 5 means it recharges
        // on 5 or 6 at start of turn. RAW is "3/day" — the engine doesn't
        // track per-day pools, so the recharge envelope is the closest
        // approximation. Initially available (instantiate_creature flags
        // every recharge ability as `available=true`).
        recharge_abilities: vec![("healing_touch", 5)],
        // RAW: when the unicorn closes at least the clause's distance in a
        // straight line and then connects with its horn, the hit carries
        // extra 2d8 piercing and a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::UNICORN_CHARGE),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier};

    #[test]
    fn unicorn_template_shape() {
        let a = ActorInstance::from_creature_template(
            &UNICORN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Large);
        assert!(a.find_action("hooves + horn").is_some());
        assert!(a.find_action("unicorn hooves").is_some());
        assert!(a.find_action("unicorn horn").is_some());
        assert!(a.find_action("healing touch").is_some());
    }

    #[test]
    fn unicorn_has_celestial_immunities() {
        let a = ActorInstance::from_creature_template(
            &UNICORN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Poison damage immunity — celestial purity.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        // Condition immunities: Charmed, Paralyzed, Poisoned.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        // Magic Resistance — advantage on saves vs magical effects.
        assert!(a.has_magic_resistance());
    }

    #[test]
    fn unicorn_healing_touch_starts_recharged() {
        let a = ActorInstance::from_creature_template(
            &UNICORN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Healing touch should be available at instantiation (the
        // recharge chassis sets every entry to `available=true`).
        assert!(a.is_recharge_available("healing_touch"));
    }
}
