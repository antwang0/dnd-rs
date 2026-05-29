use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MARILITH_LONGSWORD, MARILITH_MULTI, MARILITH_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Marilith — CR 16 demon. Six-armed snake-bodied general of the Abyss;
/// the apex of the multi-swing demon ladder. Signature lane:
/// - **Six longswords** (one per arm) — 2d8 + STR per swing.
/// - **One tail** (reach 2, 10ft) — 2d10 + STR bludgeoning per swing.
/// - **Multiattack** — all seven swings on one Action.
///
/// Slots between the Glabrezu (CR 9 mid-tier demon, 4-swing multi) and
/// the Balor (CR 19 apex). Magic-resistant in RAW but we don't model
/// that yet; the seven-swing volume + standard demon envelope already
/// makes her a meaningful escalation tier. No Legendary Resistance —
/// RAW: marilith doesn't have LR (that's reserved for the Balor / pit
/// fiend / Demon Lord tier in our pool).
pub static MARILITH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MARILITH_LONGSWORD);
    actions.push(&MARILITH_TAIL);
    actions.push(&*MARILITH_MULTI);
    CreatureTemplate {
        name: "Marilith",
        // 'Y' was free — uppercase letter to mark a CR-16 boss; visually
        // suggests the six-armed silhouette branching off the body.
        glyph: 'Y',
        ac: 18,
        // 19d10+85 ≈ 189 average per MM (CR 16 demon HP envelope).
        hitpoints: "19d10+85".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 20,
        dexterity: 20,
        wisdom: 16,
        constitution: 20,
        charisma: 20,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 16.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Standard demon envelope: immune to poison; resistant to cold +
        // fire + lightning + mundane B/P/S. Mirrors the Glabrezu / Balor
        // damage profile so radiant / force land cleanly on her.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Marilith proficient saves: STR / CON / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Demon condition envelope: Poisoned / Charmed / Frightened
        // immunity — same as the rest of the demon pool.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // No LR — marilith RAW lacks Legendary Resistance.
        legendary_resistances: 0,
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

    /// Marilith template carries the demon envelope (poison immunity,
    /// cold/fire/lightning/B/P/S resistance, charmed/frightened/poisoned
    /// condition immunities) and exposes the seven-swing multi.
    #[test]
    fn marilith_template_carries_demon_envelope() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&MARILITH_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let m = &e.actors[&id];
        assert!(m.is_immune_to(DamageType::Poison));
        assert!(m.is_resistant_to(DamageType::Cold));
        assert!(m.is_resistant_to(DamageType::Fire));
        assert!(m.is_resistant_to(DamageType::Lightning));
        assert!(m.is_resistant_to(DamageType::Bludgeoning));
        assert!(m.is_immune_to_condition(Condition::Poisoned));
        assert!(m.is_immune_to_condition(Condition::Charmed));
        assert!(m.is_immune_to_condition(Condition::Frightened));
        // No LR — marilith RAW does not include Legendary Resistance.
        assert_eq!(m.legendary_resistance_max(), 0);
        assert!(m.find_action("marilith longsword").is_some());
        assert!(m.find_action("marilith tail").is_some());
        assert!(m.find_action("marilith multiattack").is_some());
    }
}
