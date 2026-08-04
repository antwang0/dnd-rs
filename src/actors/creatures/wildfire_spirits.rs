use crate::actions::class_features::WILDFIRE_SPIRIT_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SimpleWeapon;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// **Flame Seed** — the wildfire spirit's ranged attack. RAW: `+ spell
/// attack modifier` to hit, range 60 ft, `1d6 + spellcasting ability
/// modifier` fire damage.
///
/// The engine's summons roll their own attacks off their own stat
/// block rather than off the summoner's, which is the one place this
/// departs from RAW: a `SimpleWeapon` has no channel to reach back to
/// whoever called it. DEX 14 (+2) on the spirit's block is chosen to
/// land the to-hit and damage within a point of what a WIS-18 druid's
/// spell attack would have given, so the substitution costs the
/// feature nothing in practice.
///
/// Reach 24 tiles is RAW's 60 ft on the 2.5 ft grid — the same distance
/// Enhanced Bond measures, which is not a coincidence: RAW sizes the
/// spirit's reach and the druid's bond to the same number so that a
/// spirit shooting at something is a spirit close enough to be worth
/// having.
pub static FLAME_SEED: SimpleWeapon = SimpleWeapon::ranged(
    "flame seed",
    &["seed", "flame"],
    AbilityScoreType::Dexterity,
    Dice::new(1, 6),
    DamageType::Fire,
    24,
    24,
);

/// Wildfire Spirit — the Small elemental the Circle of Wildfire Druid
/// summons with their level-2 feature, and the only creature in the
/// bestiary that exists solely to be somebody's subclass.
///
/// It is a poor combatant on purpose. 20-ish hit points, AC 13, one
/// 1d6 attack — a goblin trades with it evenly. What it is actually
/// for is standing in the right place: the druid's Enhanced Bond pays
/// out only while the spirit is within 60 ft, so a Wildfire druid
/// spends the fight steering a second body around the map to keep a
/// die on their own spells. That makes the spirit's position, not its
/// damage, the resource — and it is why the block is deliberately not
/// scaled up. A spirit worth fighting over would be a pet; a spirit
/// worth *standing near* is the subclass.
///
/// `WILDFIRE_SPIRIT_TAG` is the whole reason the template carries a
/// feature set: it is the needle `wildfire_bond_active` searches the
/// board for. Nothing on the spirit reads it.
///
/// Defensive envelope is the shared elemental one — poison immunity
/// and non-magical physical resistance from `elemental_damage_modifiers`,
/// the nine-condition `ELEMENTAL_CONDITION_IMMUNITIES` set, and fire
/// immunity on top, the same overlay the Magmin and Fire Elemental
/// carry. A wildfire spirit standing inside its druid's Wall of Fire
/// is the intended picture.
///
/// RAW's **Fiery Teleportation** (the spirit's own action: teleport
/// itself and one willing creature, then burst for `1d6 + spell mod`
/// fire) is left out. The teleport half needs a destination picker the
/// AI has no channel to answer — the same reason the Eldritch Knight's
/// Arcane Charge and the Transmuter's stone re-attunement are still
/// future work — and shipping the burst half alone would be a worse
/// Flame Seed with extra steps.
///
/// Glyph 'w' (lowercase) — for **w**ildfire, and distinct from the
/// Wildfire Druid's own 'W'. The summoner and the summon read as a
/// pair on the map without reading as the same thing.
pub static WILDFIRE_SPIRIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FLAME_SEED);
    CreatureTemplate {
        name: "Wildfire Spirit",
        glyph: 'w',
        ac: 13,
        // RAW ties the spirit's hit points to the druid's level. A fixed
        // roll is the engine's convention for summons (Conjure Animals'
        // wolves and Conjure Elemental's elemental both use their own
        // stat blocks unchanged), and 5d6 ≈ 17 sits where a level-9
        // druid's spirit would land.
        hitpoints: "5d6".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 14,
        constitution: 10,
        intelligence: 13,
        wisdom: 11,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial, Language::Druidic]),
        cr: 1.0,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        damage_modifiers: elemental_damage_modifiers([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // The marker the druid's Enhanced Bond looks for. Carried by
        // the spirit rather than back-linked from the druid — see
        // `WILDFIRE_SPIRIT_TAG`.
        features: HashSet::from([WILDFIRE_SPIRIT_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn spirit() -> ActorInstance {
        ActorInstance::from_creature_template(
            &WILDFIRE_SPIRIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn the_spirit_carries_the_marker_the_bond_searches_for() {
        assert!(spirit().has_passive_feature(WILDFIRE_SPIRIT_TAG));
    }

    #[test]
    fn the_spirit_shoots_a_flame_seed_the_length_of_the_bond() {
        use crate::actions::action_template::Action;
        let a = spirit();
        assert!(a.find_action("flame seed").is_some());
        // RAW sizes the seed's range and the bond's reach to the same
        // 60 ft. If one moves the other has to.
        assert_eq!(
            FLAME_SEED.reach_tiles(),
            Some(crate::engine::encounter::ENHANCED_BOND_REACH_TILES)
        );
    }

    #[test]
    fn the_spirit_is_made_of_the_element_it_throws() {
        let a = spirit();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert_eq!(a.size(), Size::Small);
    }
}
