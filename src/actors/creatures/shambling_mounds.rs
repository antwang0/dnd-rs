use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    SHAMBLING_MOUND_ENGULF, SHAMBLING_MOUND_MULTI, SHAMBLING_MOUND_SLAM,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Shambling Mound — CR 5 plant. Mass of decaying vegetation animated
/// by druidic magic / vile swamp ooze. Signature lanes:
/// - **2 slams** multiattack (2d8+STR each per swing).
/// - **Engulf** (STR-attack vs AC; on hit deals slam damage and saves
///   gate a 10-round Grappled rider — the mound wraps the victim).
///
/// Defensive shape: lightning immunity (RAW: lightning damage heals
/// the mound by an equal amount — we collapse to immunity since the
/// engine doesn't model damage-to-heal yet; the load-bearing effect is
/// "lightning swings are wasted"). Cold + fire resistance is the
/// standard plant envelope. AC 15, ~80 HP (13d10+39).
pub static SHAMBLING_MOUND_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SHAMBLING_MOUND_SLAM);
    actions.push(&*SHAMBLING_MOUND_MULTI);
    actions.push(&*SHAMBLING_MOUND_ENGULF);
    CreatureTemplate {
        name: "Shambling Mound",
        // 'H' uppercase — distinct from 'h' (hobgoblin) and 'h' lowercase
        // for hippogriff; reads as a hulking plant boss.
        glyph: 'H',
        ac: 15,
        // 13d10+39 ≈ 80 HP — matches MM CR 5 plant envelope.
        hitpoints: "13d10+39".parse().unwrap(),
        speed: 20.,
        strength: 18,
        dexterity: 8,
        constitution: 16,
        intelligence: 5,
        wisdom: 10,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Plant,
        actions,
        // RAW: Lightning Absorption — lightning damage deals nothing and
        // heals the mound for the same number. Cold + fire resistance is
        // the standard plant envelope per MM.
        damage_modifiers: HashMap::from([
            (DamageType::Lightning, DamageModifier::Absorption),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
        ]),
        // SRD 5.2 "Immunities Lightning; Deafened, Exhaustion" — the one
        // Plant in the book on the tireless list, and the reason the
        // rule cannot be derived from creature type: a shambling mound
        // is a heap of rotting vegetation with no muscle in it, where a
        // treant is a tree that walks and gets tired doing it.
        condition_immunities: HashSet::from([
            Condition::Deafened,
            Condition::Exhausted,
        ]),
        ..CreatureTemplate::defaults()
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

    /// Shambling Mound template carries the plant resistance envelope
    /// (lightning immunity, cold + fire resistance) and exposes all
    /// three action lanes (slam, multiattack, engulf).
    #[test]
    fn shambling_mound_template_carries_plant_envelope() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&SHAMBLING_MOUND_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let m = &e.actors[&id];
        assert_eq!(
            m.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Absorption),
            "RAW's Lightning Absorption, not the flat immunity it used to be"
        );
        assert!(m.is_resistant_to(DamageType::Cold));
        assert!(m.is_resistant_to(DamageType::Fire));
        assert!(m.find_action("shambling slam").is_some());
        assert!(m.find_action("shambling mound multiattack").is_some());
        assert!(m.find_action("shambling engulf").is_some());
    }
}
