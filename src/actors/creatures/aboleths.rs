use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ABOLETH_MULTI, ABOLETH_TENTACLE};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
#[cfg(test)]
use crate::engine::types::DamageType;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Aboleth — CR 10 aquatic aberration. Ancient psychic horror with a
/// tentacle burst as its workhorse and a slow regenerating HP pool.
/// Stats target MM aboleth: 135 HP, AC 17, three tentacles per
/// Action (2d6 bludgeoning each), reach 10ft, immune to mundane
/// fear-style mind effects (Charmed / Frightened).
///
/// Regenerator: heals 10 HP at end-of-round while combat-active. No
/// suppressor type — fire would extinguish an actual aboleth, but
/// without an aquatic-substrate model the regen is just a slow heal.
pub static ABOLETH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ABOLETH_TENTACLE);
    actions.push(&*ABOLETH_MULTI);
    CreatureTemplate {
        name: "Aboleth",
        glyph: 'A',
        ac: 17,
        hitpoints: "18d10+36".parse().unwrap(),
        speed: 30.,
        strength: 21,
        intelligence: 18,
        dexterity: 9,
        wisdom: 15,
        constitution: 15,
        charisma: 18,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::DeepSpeech]),
        cr: 10.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Aboleths don't have notable damage modifiers in MM, but they're
        // implicitly immune to the slow-drowning aquatic effects we don't
        // model. Leave the table empty.
        damage_modifiers: HashMap::new(),
        // Aboleth proficient saves: CON, INT, WIS.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // The ancient horror is immune to Charmed (its mind is too alien)
        // and Frightened (it has watched stars die).
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        features: HashSet::new(),
        regen_per_round: 10,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});

/// Sanity check: vulnerability handling is implicit (no entries means
/// no double damage), so plate this template against the well-known
/// MM tables.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn aboleth_is_charm_immune() {
        let a = ActorInstance::from_creature_template(
            &ABOLETH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Charmed));
        assert!(a.is_immune_to_condition(Condition::Frightened));
    }

    #[test]
    fn aboleth_normal_damage_to_acid() {
        let a = ActorInstance::from_creature_template(
            &ABOLETH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.effective_damage(10, DamageType::Acid), 10);
    }
}
