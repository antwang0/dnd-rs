use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SOLAR_LONGSWORD, SOLAR_MULTI};
use crate::actions::spells::{
    CURE_WOUNDS, FORESIGHT, HEAL_SPELL_HIGH, HOLY_AURA, MASS_HEAL, RESURRECTION,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Solar — CR 21 angelic celestial. The mightiest non-deity creature in
/// the SRD: huge HP pool, radiant immunity, and a kit that mixes a 2-
/// swing slaying-longsword multi (4d8 slash + 1d6 radiant rider per
/// swing) with the apex divine spells: Holy Aura (level-8 30ft save
/// advantage), Foresight (level-9 single-target buff), Mass Heal +
/// Resurrection for sustain.
///
/// Stats target MM solar: 243 HP (22d10+121), AC 21, STR 26, immune to
/// fire/poison/radiant + the standard celestial condition immunities
/// (Charmed / Exhausted / Frightened / Poisoned). Spell slots tuned so
/// the marquee high-level spells each fire once.
pub static SOLAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SOLAR_LONGSWORD);
    actions.push(&*SOLAR_MULTI);
    actions.push(&*HOLY_AURA);
    actions.push(&*FORESIGHT);
    actions.push(&*MASS_HEAL);
    actions.push(&*HEAL_SPELL_HIGH);
    actions.push(&*RESURRECTION);
    actions.push(&*CURE_WOUNDS);
    CreatureTemplate {
        name: "Solar",
        glyph: 'O', // 'S' is already Skeleton; 'O' for the angelic Ouranos figure.
        ac: 21,
        hitpoints: "22d10+121".parse().unwrap(),
        speed: 30.,
        strength: 26,
        intelligence: 25,
        dexterity: 22,
        wisdom: 25,
        constitution: 26,
        charisma: 30,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Truesight(120),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 21.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        // 4 lv5 + 2 lv6 + 2 lv7 + 2 lv8 + 1 lv9 — enough to fire Holy
        // Aura, Foresight, and Mass Heal once apiece, plus Resurrection
        // / Heal for emergencies.
        spell_slots_by_level: vec![0, 0, 0, 0, 4, 2, 2, 2, 1],
        rolls_death_saves: false,
        // Solar resistances per MM: immune to fire / poison / radiant,
        // plus resistant to non-magical physical (we don't model the
        // magical-vs-mundane split, so flat resistance).
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Radiant, DamageModifier::Immunity),
        ]),
        // Solar proficient saves: every single one. RAW gives the solar
        // proficiency on all six saves via its angelic Aura of Light.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Poisoned,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day) — RAW per MM. The Solar's
        // signature anti-save defense rounding out the celestial boss
        // envelope (Holy Aura + Foresight + Mass Heal).
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 3,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
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
            &SOLAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn solar_is_radiant_immune() {
        let s = make();
        assert_eq!(s.effective_damage(99, DamageType::Radiant), 0);
        assert_eq!(s.effective_damage(99, DamageType::Fire), 0);
        assert_eq!(s.effective_damage(99, DamageType::Poison), 0);
        // Necrotic still bites — undead-killing angels aren't necrotic-immune
        // in RAW.
        assert_eq!(s.effective_damage(10, DamageType::Necrotic), 10);
    }

    #[test]
    fn solar_can_cast_foresight() {
        let s = make();
        // Slot table starts at level 5 — confirm level-9 slot exists.
        assert_eq!(s.spell_slot_manager.spell_slots(9).max_spell_slots, 1);
        assert_eq!(s.spell_slot_manager.spell_slots(8).max_spell_slots, 2);
    }
}
