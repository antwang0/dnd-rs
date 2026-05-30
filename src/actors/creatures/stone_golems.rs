use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    STONE_GOLEM_MULTI, STONE_GOLEM_SLAM, STONE_GOLEM_SLOW,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Stone Golem — CR 10 boss-tier construct. The signature "magic doesn't
/// work on me" boss: immune to non-magical bludgeoning / piercing /
/// slashing (we collapse to flat physical resistance), immune to poison /
/// psychic damage entirely, and immune to a wide swath of debuff
/// conditions (Charmed / Frightened / Paralyzed / Petrified / Poisoned /
/// Exhausted — it has no metabolism, no mind to terrify, no joints to
/// freeze).
///
/// Action lanes:
/// - **Stone Golem Multiattack** — 2 stone slams per Action. Pure
///   bludgeoning damage; no rider effects.
/// - **Stone Slam** (standalone) — 3d8+STR bludgeoning for when the
///   multi isn't worth the action (e.g. on a low-HP target).
/// - **Stone Golem Slow** — Action; 10ft burst centered on the golem.
///   Every enemy in the radius makes a WIS save vs DC 17 or is `Slowed`
///   for 5 rounds. The golem's only soft-control move.
///
/// **Legendary Resistance (3/Day)** — the construct's signature defense.
/// Failed saves are auto-promoted to passes 3 times per long rest.
/// Counters the party's save-or-suck spells (Hold Monster, Banishment,
/// Slow). Combined with the magic-immunity envelope, the golem is the
/// "anti-caster" boss — the party must whittle it down with magical
/// weapons rather than spells.
///
/// Stat shape: AC 17, ~178 average HP (17d10+85), STR 22, INT 3, no
/// senses beyond darkvision (constructs don't perceive the world like
/// living things). No languages — they understand commands from their
/// creator but don't speak. CR 10.
pub static STONE_GOLEM_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*STONE_GOLEM_MULTI);
    actions.push(&STONE_GOLEM_SLAM);
    actions.push(&*STONE_GOLEM_SLOW);
    CreatureTemplate {
        name: "Stone Golem",
        // 'G' (uppercase) — golem glyph distinct from 'g' (goblin).
        glyph: 'G',
        ac: 17,
        // 17d10+85 ≈ 178 average per MM (CR 10).
        hitpoints: "17d10+85".parse().unwrap(),
        speed: 30.,
        strength: 22,
        intelligence: 3,
        dexterity: 9,
        wisdom: 11,
        constitution: 20,
        charisma: 1,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([crate::engine::types::SpecialSense::Darkvision(120)]),
        languages: HashSet::new(),
        cr: 10.0,
        size: Size::Large,
        creature_type: CreatureType::Construct,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            // Construct immunities — flesh-and-blood damage doesn't
            // bypass enchanted stone.
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
            // Mundane B/P/S resistance per MM (we approximate as flat).
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([
            // Constructs have no mind to charm / frighten and no
            // metabolism to poison or exhaust.
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Exhausted,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day) — the golem's anti-caster
        // signature. Three failed saves per long rest are auto-promoted
        // to passes, neutralizing the party's save-or-suck control spells.
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::{AbilityScoreType, Coordinate};

    fn make_test_encounter() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 12,
            height: 12,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap()
    }

    /// The golem template carries 3 LR charges and the full magic-immunity
    /// envelope (poison + psychic damage immunity, construct condition
    /// immunities). Spot-check the load-bearing fields.
    #[test]
    fn stone_golem_template_shape() {
        let t = &*STONE_GOLEM_TEMPLATE;
        assert_eq!(t.legendary_resistances, 3);
        assert!(matches!(
            t.damage_modifiers.get(&DamageType::Poison),
            Some(DamageModifier::Immunity)
        ));
        assert!(matches!(
            t.damage_modifiers.get(&DamageType::Psychic),
            Some(DamageModifier::Immunity)
        ));
        assert!(t.condition_immunities.contains(&Condition::Charmed));
        assert!(t.condition_immunities.contains(&Condition::Frightened));
        assert!(t.condition_immunities.contains(&Condition::Paralyzed));
        assert!(t.condition_immunities.contains(&Condition::Exhausted));
    }

    /// Failing a save with LR available promotes the fail to a pass and
    /// decrements the charge count. Three failed saves burn the pool;
    /// the fourth fail lands.
    #[test]
    fn stone_golem_burns_legendary_resistance_on_fail() {
        let mut e = make_test_encounter();
        let id = e
            .instantiate_creature(&STONE_GOLEM_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        // Force a fail by using an unreachable DC.
        let initial = e
            .actors
            .get(&id)
            .unwrap()
            .legendary_resistance_remaining();
        assert_eq!(initial, 3);
        for i in 0..3 {
            let save = e.roll_save(id, AbilityScoreType::Charisma, 40);
            assert!(
                save.passed(),
                "LR should auto-promote fail #{} to pass",
                i + 1
            );
            assert_eq!(
                e.actors
                    .get(&id)
                    .unwrap()
                    .legendary_resistance_remaining(),
                3 - (i as u32 + 1)
            );
        }
        // Pool exhausted — next fail lands.
        let save = e.roll_save(id, AbilityScoreType::Charisma, 40);
        assert!(!save.passed(), "exhausted LR pool means the fail sticks");
    }

    /// Long rest refreshes the LR pool back to the template max.
    #[test]
    fn stone_golem_long_rest_restores_legendary_resistance() {
        let mut e = make_test_encounter();
        let id = e
            .instantiate_creature(&STONE_GOLEM_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        // Burn all 3 charges.
        for _ in 0..3 {
            let _ = e.roll_save(id, AbilityScoreType::Charisma, 40);
        }
        assert_eq!(
            e.actors
                .get(&id)
                .unwrap()
                .legendary_resistance_remaining(),
            0
        );
        e.actors.get_mut(&id).unwrap().long_rest();
        assert_eq!(
            e.actors
                .get(&id)
                .unwrap()
                .legendary_resistance_remaining(),
            3
        );
    }
}
