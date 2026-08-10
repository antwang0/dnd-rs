use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::AZER_WARHAMMER;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::attack::{MeleeReflect, ReflectDamage};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Azer **Heated Body** — RAW: "A creature that touches the azer or hits
/// it with a melee attack while within 5 feet of it takes 5 (1d10) fire
/// damage."
///
/// The same clause the salamander carries and the remorhaz carries, at
/// the die RAW gives each of them — 1d10 here against the salamander's
/// 1d6+3, because an azer is a smaller furnace. Routed through the
/// natural-melee-reflect lane, which composes additively with the
/// condition-keyed reflects: an azer under Fire Shield rolls both on the
/// same incoming swing.
pub static AZER_HEATED_BODY: MeleeReflect = MeleeReflect {
    damage: ReflectDamage::Dice(Dice::new(1, 10)),
    damage_type: DamageType::Fire,
    label: "heated body",
};

/// Azer — CR 2 medium elemental. A dwarf-shaped being of living bronze
/// and fire from the Elemental Plane of Fire, and the cheapest creature
/// in the bestiary that punishes the party for choosing to melee it.
///
/// Three of the azer's four traits are the same fact seen from
/// different angles — it is made of fire — and the engine expresses
/// each of them in a different lane, which is what makes the stat block
/// worth reading:
///
///   - **Heated Body** is the melee reflect above: 1d10 fire back at
///     everything that connects with it in contact.
///   - **Heated Weapon** is the 1d6 fire rider on the warhammer, which
///     lives on the weapon rather than on the body because it applies
///     to what the azer hits rather than to what hits the azer.
///   - **Fire immunity** and **poison immunity** are the damage-modifier
///     map, and they are why a party that opened with fire has wasted
///     its opener.
///
/// The fourth is **Illumination**: RAW's azer sheds bright light in a
/// 10-foot radius. Not modeled — the lighting layer takes light sources
/// from carried torches and cast spells, and a creature that is itself
/// a light source has no hook there. The tactical half of the clause
/// (you cannot sneak up on an azer in the dark, and the azer cannot
/// sneak up on you) is lost; the damage half, which is the whole rest of
/// the stat block, is not.
///
/// Stat shape per the SRD: AC 17 (natural armor, shield), 39 HP
/// (6d8+12), STR 17 / DEX 12 / CON 15 / INT 12 / WIS 13 / CHA 10. Speed
/// 30. Proficient CON saves. Languages: Primordial (Ignan). CR 2.
pub static AZER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&AZER_WARHAMMER);
    CreatureTemplate {
        name: "Azer",
        // 'z' (lowercase) — the tail of the word rather than its head,
        // because 'a' is the Salamander's and 'A' the Animated Armor /
        // Assassin band, and 'z' is otherwise unclaimed.
        glyph: 'z',
        ac: 17,
        // 6d8+12 ≈ 39 average per the SRD (CR 2).
        hitpoints: "6d8+12".parse().unwrap(),
        speed: 30.,
        strength: 17,
        dexterity: 12,
        constitution: 15,
        intelligence: 12,
        wisdom: 13,
        charisma: 10,
        languages: HashSet::from([Language::Primordial]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Elemental,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        proficient_saves: HashSet::from([AbilityScoreType::Constitution]),
        natural_melee_reflect: Some(AZER_HEATED_BODY),
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
            &AZER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn azer_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert!(a.find_action("azer warhammer").is_some());
    }

    /// All three faces of "made of fire", each in the lane that carries
    /// it. Asserted together because the failure this guards against is
    /// a stat block that keeps one of them and reads as if it kept all
    /// three.
    #[test]
    fn the_azer_burns_in_every_direction_at_once() {
        let a = make();
        // Immune to what it is made of…
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        // …burns whoever reaches it…
        let reflect = a.natural_melee_reflect().expect("heated body");
        assert_eq!(reflect.damage_type, DamageType::Fire);
        // …and burns whoever it reaches. The rider is on the weapon, so
        // the hammer has to declare fire among its damage types.
        let hammer = a.find_action("azer warhammer").expect("warhammer");
        assert!(hammer.damage_types().contains(&DamageType::Fire));
    }
}
