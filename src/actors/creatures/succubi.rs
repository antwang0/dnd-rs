use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SUCCUBUS_CHARM, SUCCUBUS_CLAWS, SUCCUBUS_DRAINING_KISS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Succubus / Incubus — CR 4 medium fiend (neutral-evil). The seduction-
/// specialist demon: a Charm gate at range, a draining Kiss that auto-
/// connects once the target is locked in, and slashing claws as the
/// fallback when the social kit doesn't apply. RAW: shapechanger, fly
/// speed, and a "telepathic bond" out-of-combat charm — we collapse the
/// flavor and surface only the load-bearing combat slice:
///
/// - `SUCCUBUS_CHARM` (single-target, 30-ft range, WIS save) — the
///   setup move that anchors the kiss target.
/// - `SUCCUBUS_DRAINING_KISS` (melee, requires Charmed target) — the
///   load-bearing damage + max-HP drain combo.
/// - `SUCCUBUS_CLAWS` (melee, vanilla slashing) — fallback when the
///   target isn't yet charmed.
///
/// Damage envelope: resistant to cold / fire / lightning / poison
/// (demon physiology) and to bludgeoning / piercing / slashing from non-
/// magical weapons — we approximate the magic-weapon clause with a flat
/// physical resistance on B/P/S.
///
/// Stats track MM Succubus at CR 4 — high DEX and CHA (CHA primary for
/// the charm DC), modest HP, melee + control profile. Magic Resistance
/// flag joins the standard caster-counter lane.
pub static SUCCUBUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SUCCUBUS_CLAWS);
    actions.push(&*SUCCUBUS_DRAINING_KISS);
    actions.push(&*SUCCUBUS_CHARM);
    CreatureTemplate {
        name: "Succubus",
        // 'ς' (Greek final sigma) — distinct from 'σ' (Shadow Demon)
        // and other curved fiend glyphs. Reads as the seductive
        // hooked silhouette of a winged fey-fiend.
        glyph: 'ς',
        ac: 15,
        hitpoints: "10d8+20".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 15,
        dexterity: 17,
        wisdom: 12,
        constitution: 13,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([
            Language::Abyssal,
            Language::Common,
            Language::Infernal,
        ]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Resistance),
            // Non-magical physical resistance approximated as flat B/P/S
            // resistance — same shape as the quasit's envelope.
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
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

    #[test]
    fn succubus_has_high_charisma() {
        let a = ActorInstance::from_creature_template(
            &SUCCUBUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // CHA 20 → +5 modifier → save DC 8 + 2 prof + 5 = 15.
        use crate::engine::types::AbilityScoreType;
        assert_eq!(
            a.spell_save_dc(AbilityScoreType::Charisma),
            15
        );
    }

    #[test]
    fn succubus_resists_physical() {
        let a = ActorInstance::from_creature_template(
            &SUCCUBUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }
}
