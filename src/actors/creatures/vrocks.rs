use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{VROCK_BEAK, VROCK_MULTI, VROCK_SCREECH, VROCK_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Vrock — CR 6 demon. Vulture-headed flying fiend; the entry-tier
/// demon, sitting below the Glabrezu (CR 9) and the Marilith (CR 16) on
/// the demon ladder. Signature lanes:
/// - **2 talons + 1 beak** multiattack (2d6 each per swing).
/// - **Stunning Screech** (action, NoArgs): every non-demon within 20ft
///   takes 3d6 thunder and a CON-save Stunned-1 follow-up.
///
/// Standard demon envelope: poison immunity, cold/fire/lightning + B/P/S
/// resistance, Charmed/Frightened/Poisoned condition immunities. No
/// Legendary Resistance — that's reserved for CR 11+ demon princes in
/// our pool.
pub static VROCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&VROCK_TALONS);
    actions.push(&VROCK_BEAK);
    actions.push(&*VROCK_MULTI);
    actions.push(&*VROCK_SCREECH);
    CreatureTemplate {
        name: "Vrock",
        // 'V' was free — uppercase letter to mark a CR-6 boss-tier
        // monster (lowercase 'v' is vampire spawn).
        glyph: 'V',
        ac: 15,
        // 14d10+28 = 104 average per MM (CR 6 demon HP envelope).
        hitpoints: "14d10+28".parse().unwrap(),
        speed: 40., // walk + fly 60ft RAW; we use the walking value
        strength: 17,
        intelligence: 8,
        dexterity: 15,
        wisdom: 11,
        constitution: 18,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 6.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Standard demon envelope: immune to poison; resistant to cold +
        // fire + lightning + mundane B/P/S.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Vrock proficient saves: DEX / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::types::Coordinate;

    fn arena() -> EncounterInstance {
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::terrain_gen::TerrainGenParams;
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(1)).unwrap()
    }

    /// Vrock template carries the standard demon envelope and exposes
    /// all four action lanes (talons, beak, multiattack, screech).
    #[test]
    fn vrock_template_carries_demon_envelope() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&VROCK_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let v = &e.actors[&id];
        assert!(v.is_immune_to(DamageType::Poison));
        assert!(v.is_resistant_to(DamageType::Cold));
        assert!(v.is_resistant_to(DamageType::Fire));
        assert!(v.is_resistant_to(DamageType::Lightning));
        assert!(v.is_immune_to_condition(Condition::Poisoned));
        assert!(v.is_immune_to_condition(Condition::Charmed));
        assert!(v.is_immune_to_condition(Condition::Frightened));
        assert_eq!(v.legendary_resistance_max(), 0);
        assert!(v.find_action("vrock talons").is_some());
        assert!(v.find_action("vrock beak").is_some());
        assert!(v.find_action("vrock multiattack").is_some());
        assert!(v.find_action("vrock screech").is_some());
    }

    /// Vrock Stunning Screech: enemy-only thunder burst with a CON-save
    /// Stunned-on-fail rider. The non-demon filter (Poison-immune)
    /// exempts other demons from the screech — verify a goblin gets
    /// damaged (no Poison immunity) and a balor sitting in range is
    /// spared (Poison-immune demon).
    #[test]
    fn vrock_screech_spares_demons() {
        use crate::actions::action_template::Action;
        use crate::actors::creatures::balors::BALOR_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        let mut e = arena();
        let vrock = e
            .instantiate_creature(&VROCK_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        // Place the balor a few tiles farther so its huge footprint
        // doesn't overlap the goblin's. Both sit within the 8-tile burst.
        let balor = e
            .instantiate_creature(&BALOR_TEMPLATE, Coordinate::new(11, 5), 0, 1)
            .unwrap();
        let goblin_hp_before = e.actors[&goblin].hitpoints();
        let balor_hp_before = e.actors[&balor].hitpoints();
        for ef in VROCK_SCREECH.side_effects(&mut e, vrock, None, None, None) {
            ef.apply(&mut e);
        }
        // Goblin (no poison immunity) takes thunder damage.
        assert!(
            e.actors[&goblin].hitpoints() < goblin_hp_before,
            "goblin should take thunder damage from the screech"
        );
        // Balor (poison-immune demon) is exempt — HP unchanged.
        assert_eq!(
            e.actors[&balor].hitpoints(),
            balor_hp_before,
            "balor (poison-immune demon) should be spared by the screech filter"
        );
    }
}
