use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    ACID_SPLASH, CHARM_PERSON, FIRE_BOLT, MAGE_ARMOR, MAGIC_MISSILE, MIRROR_IMAGE, MISTY_STEP,
    RAY_OF_FROST, SHIELD, SLEEP, WEB,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, SpecialSense, Skill,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Rock Gnome Illusionist — Small-size INT-primary wizard chassis.
/// The defining racial trait is **Gnome Cunning**: advantage on
/// Intelligence, Wisdom, and Charisma saving throws against magic.
/// We model the magic qualifier loosely — most saves in this engine
/// originate from spells, so blanket advantage on INT / WIS / CHA
/// saves matches RAW well in practice (read by `compute_save_mode`).
///
/// Stat shape targets a level-3 wizard build: 18 HP (3d6+6), AC 13
/// (mage armor + DEX), INT 16. Speed 25 (RAW Small-race speed).
/// Loadout leans on the wizard's signature lv1 staples — Magic Missile
/// / Shield / Mage Armor — plus a Sleep / Charm Person enchantment
/// lane and a Web for area control. Mirror Image at lv2 covers the
/// gnome's squishy chassis. Cantrips: Fire Bolt / Ray of Frost / Acid
/// Splash.
pub static GNOME_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // Cantrips (at-will).
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*ACID_SPLASH);
    // Level 1
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SHIELD);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    // Level 2
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*WEB);
    actions.push(&*MISTY_STEP);
    CreatureTemplate {
        name: "Rock Gnome Illusionist",
        // 'G' — distinct from 'g' (goblin / grick) and reads as a
        // small, robed INT-caster.
        glyph: 'G',
        ac: 13, // unarmored + DEX (mage armor at floor 13)
        hitpoints: "3d6+6".parse().unwrap(),
        // 5e Rock Gnome: 25 ft speed (Small race).
        speed: 25.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 11,
        // Rock Gnomes are proficient with Arcana per RAW Tinker /
        // Artificer's Lore — Arcana proxies the "double prof on
        // history checks about magic items" RAW clause cleanly.
        skills: HashSet::from([Skill::Arcana]),
        items: Vec::new(),
        // 5e Gnome Darkvision: 60 ft.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Gnomish]),
        cr: 2.0,
        // 5e Gnome: Small size.
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        // Wizard-lite slot table: 3 lv1 / 2 lv2.
        spell_slots_by_level: vec![3, 2],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
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
        // 5e Gnome Cunning: advantage on INT / WIS / CHA saves vs magic.
        has_gnome_cunning: true,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
