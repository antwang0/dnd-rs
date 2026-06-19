use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CAMBION_FIRE_RAY, CAMBION_MULTI, CAMBION_SPEAR};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cambion — CR 5 medium fiend (half-devil hybrid). Mobile elite skirmisher
/// with a fire-rider spear (1d6+STR piercing + 2d6 fire) and a 24-tile
/// ranged fire-ray (CHA-based spell attack 4d6 fire). The two-spear multi
/// chains the fire rider twice — ~24 average per Action when both lands —
/// while the fire ray gives the AI a chase option when the target kites
/// out of melee. RAW also has Charm and a fly speed; we collapse fly into
/// the ground-speed of 40 (the engine doesn't model 3D positioning) and
/// skip the once-per-day Charm (the AI gates poorly against once-per-rest
/// CC).
///
/// Templates: AC 19, 82 HP (11d8+33), STR 18 (+4), DEX 18 (+4), CON 16,
/// INT 14, WIS 12, CHA 16. Languages: Abyssal, Common, Infernal.
/// Resists fire, cold, lightning, poison, plus the three physical types
/// vs non-magical attacks. Standard fiend darkvision 60 ft (RAW: 60 ft).
pub static CAMBION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CAMBION_SPEAR);
    actions.push(&*CAMBION_FIRE_RAY);
    actions.push(&*CAMBION_MULTI);
    CreatureTemplate {
        name: "Cambion",
        // 'k' for cambion — distinct from 'C' (Centaur) and 'c'
        // (cloak of resistance / cloaker). Lowercase reads as the
        // medium-tier fiend hybrid.
        glyph: 'k',
        ac: 19,
        hitpoints: "11d8+33".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 14,
        dexterity: 18,
        wisdom: 12,
        constitution: 16,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal, Language::Common, Language::Infernal]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // 5e cambion: resistance to fire / cold / lightning / poison
        // plus the three physical types (RAW: vs non-magical weapons).
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Resistance),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
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
    fn cambion_resists_fire() {
        let a = ActorInstance::from_creature_template(
            &CAMBION_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.effective_damage(10, DamageType::Fire), 5);
        assert_eq!(a.effective_damage(10, DamageType::Poison), 5);
        // Radiant isn't in the resistance set — full damage.
        assert_eq!(a.effective_damage(10, DamageType::Radiant), 10);
    }
}
