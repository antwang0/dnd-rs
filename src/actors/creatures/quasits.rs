use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{QUASIT_CLAWS, QUASIT_SCARE};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Quasit — CR 1 tiny fiend (chaotic-evil demon). The Imp's
/// disorganized-evil mirror: same Tiny silhouette, same poisoned natural
/// attack, but with a Scare ability in place of the Imp's Fire Bolt
/// ranged option. RAW: poisoned claws, fear gaze, and a magic resistance
/// flag. Shrugs off poison entirely (demon physiology) and resists the
/// usual physical damage types from non-magical sources — we approximate
/// the "non-magical weapons" clause with a flat physical resistance
/// envelope across bludgeoning / piercing / slashing.
///
/// Stats roughly track MM Quasit at CR 1 — high DEX, modest HP, melee +
/// fear lockdown. DEX-primary so the claws hit consistently and the
/// scare-then-retreat tactic threads cleanly through cramped dungeon
/// terrain.
pub static QUASIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&QUASIT_CLAWS);
    actions.push(&*QUASIT_SCARE);
    CreatureTemplate {
        name: "Quasit",
        // 'q' — lowercase tiny-fiend glyph. Distinct from 'I' (Imp, the
        // devilish mirror) and from the uppercase humanoid pool. The
        // descender hints at the quasit's hooked tail.
        glyph: 'q',
        ac: 13,
        hitpoints: "7d4".parse().unwrap(),
        speed: 40.,
        strength: 5,
        intelligence: 7,
        dexterity: 17,
        wisdom: 10,
        constitution: 10,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal, Language::Common]),
        cr: 1.0,
        size: Size::Tiny,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: non_magical_physical_resistances([(
            DamageType::Poison,
            DamageModifier::Immunity,
        )]),
        // Magic Resistance: advantage on saves vs spells and other
        // magical effects. Slots into the standard caster-counter lane.
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
    fn quasit_resists_physical_and_is_poison_immune() {
        let a = ActorInstance::from_creature_template(
            &QUASIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn quasit_has_magic_resistance() {
        let a = ActorInstance::from_creature_template(
            &QUASIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
    }
}
