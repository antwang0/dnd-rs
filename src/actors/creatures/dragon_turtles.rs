use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DRAGON_TURTLE_BITE, DRAGON_TURTLE_CLAW, DRAGON_TURTLE_MULTI, DRAGON_TURTLE_STEAM_BREATH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Dragon Turtle — CR 17 gargantuan dragon. The aquatic dragon-kin boss,
/// a turtle the size of a galleon with a scalding steam breath. Slots
/// directly opposite Adult Red Dragon on the dragon ladder (both CR 17 —
/// the dragon-turtle is the "water lane" answer to the Red Dragon's
/// "fire lane"). The load-bearing per-round threat is the mixed bite +
/// double-claw melee burst plus the recharge-gated 12d6 fire breath that
/// keeps even fire-resistant casters honest.
///
/// Action lanes:
/// - **Dragon Turtle Multiattack** — 1 bite + 2 claws per Action. Mixed-
///   limb `CompoundAttack` (bite first to drive the reach-3 envelope so
///   the multi can land on a target a full tile beyond the claw reach).
/// - **Dragon Turtle Bite** (standalone) — STR-based 3d12+STR piercing,
///   reach 3 (15 ft RAW). The dragon turtle's massive snapping jaw;
///   heaviest die-count of any non-Tarrasque melee bite in the engine.
/// - **Dragon Turtle Claw** (standalone) — STR-based 2d8+STR slashing,
///   reach 2 (10 ft RAW). Companion swing paired with the bite in the
///   multi.
/// - **Steam Breath** — burst-3 / range-4 cone, recharge 5-6. 12d6 fire,
///   DC 18 CON, half on save. CON save (inhaled scalding vapor) rather
///   than the DEX save the elemental dragon breaths route through.
///
/// Damage envelope: Fire resistance (the dragon turtle's signature
/// "boiling water doesn't burn the boilermaker" RAW clause). No other
/// damage-type modifiers — RAW gives the dragon turtle resistance only to
/// fire damage.
///
/// Condition immunities: none — RAW gives the dragon turtle no condition
/// immunities (it's a flesh-and-bone aquatic dragon, not undead /
/// construct / fiend). Magic Resistance is also absent RAW — the dragon
/// turtle is a "physical dragon" without the spell-shrug envelope of
/// its surface-dwelling kin.
///
/// Stat shape: AC 20 (heavy shell), ~356 average HP (23d20+115), STR 25,
/// CON 20. Darkvision 120 ft. Languages: Aquan, Draconic. CR 17.
///
/// RAW also gives the dragon turtle proficient DEX/CON/WIS saves — the
/// standard "legendary-class" save profile minus the CHA save. We capture
/// the load-bearing slice via the three-stat proficient_saves set. No
/// legendary actions or resistances RAW — the dragon turtle is a heavy
/// brute, not an anti-caster boss.
pub static DRAGON_TURTLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_TURTLE_MULTI);
    actions.push(&DRAGON_TURTLE_BITE);
    actions.push(&DRAGON_TURTLE_CLAW);
    actions.push(&DRAGON_TURTLE_STEAM_BREATH);
    CreatureTemplate {
        name: "Dragon Turtle",
        // 'U' is taken by Cloud Giant; 'D' is taken by other 'D' glyphs;
        // 't' (lowercase) reads as "turtle" and is unused in the dragon /
        // monstrosity pool.
        glyph: 't',
        ac: 20,
        // 23d20+115 ≈ 356 average per MM (CR 17).
        hitpoints: "23d20+115".parse().unwrap(),
        speed: 20.,
        strength: 25,
        intelligence: 10,
        dexterity: 10,
        wisdom: 12,
        constitution: 20,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Draconic, Language::Primordial]),
        cr: 17.0,
        size: Size::Gargantuan,
        creature_type: CreatureType::Dragon,
        actions,
        // Dragon Turtle proficient saves: DEX, CON, WIS per MM. The CHA
        // save is conspicuously absent (the dragon turtle's wisdom-not-
        // charisma personality contra the surface dragons).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        // Fire resistance — the "boilermaker" lane. The steam breath's
        // own fire typing means a self-centered breath would do half
        // damage to the turtle if friendly fire were possible; the
        // breath chassis already excludes the caster.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Resistance)]),
        // 5e Recharge 5-6 on the steam breath — RAW per MM, shared with
        // the dragon family's `"breath_weapon"` pool key so the start-of-
        // turn refresher uses the same chassis.
        recharge_abilities: vec![("breath_weapon", 5)],
        has_extra_attack: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
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
    fn dragon_turtle_has_fire_resistance() {
        let a = ActorInstance::from_creature_template(
            &DRAGON_TURTLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Resistance)
        );
        // No other damage-type modifiers RAW — the dragon turtle is a
        // "physical dragon" without the magic-resistance envelope.
        assert_eq!(a.damage_modifier(DamageType::Cold), None);
        assert_eq!(a.damage_modifier(DamageType::Slashing), None);
    }

    #[test]
    fn dragon_turtle_has_mixed_multi_and_breath() {
        let a = ActorInstance::from_creature_template(
            &DRAGON_TURTLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("dragon turtle multiattack").is_some());
        assert!(a.find_action("dragon turtle bite").is_some());
        assert!(a.find_action("dragon turtle claw").is_some());
        assert!(a.find_action("steam breath").is_some());
        assert_eq!(a.cr(), 17.0);
        assert_eq!(a.size(), Size::Gargantuan);
        // No legendary resistance — the dragon turtle is a brute, not a
        // legendary anti-caster boss.
        assert_eq!(a.legendary_resistance_remaining(), 0);
        assert!(!a.has_magic_resistance());
    }
}
