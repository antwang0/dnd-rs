use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    ACID_SPLASH, BESTOW_CURSE, BLINDNESS, BLUR, BURNING_HANDS, CAUSE_FEAR, CHARM_PERSON,
    CHILL_TOUCH, COLOR_SPRAY, CONE_OF_COLD, FIRE_BOLT, FIREBALL, HASTE, HOLD_MONSTER,
    HYPNOTIC_PATTERN, INVISIBILITY, LIGHTNING_BOLT, MAGE_ARMOR, MAGIC_MISSILE, MAGIC_WEAPON,
    MIND_SLIVER, MIRROR_IMAGE, MISTY_STEP, POISON_SPRAY, RAY_OF_FROST, RAY_OF_SICKNESS,
    SCORCHING_RAY, SHATTER, SHIELD, SHOCKING_GRASP, SLEEP, SLOW, STINKING_CLOUD, THUNDERWAVE,
    TOLL_THE_DEAD, TRUE_STRIKE, VAMPIRIC_TOUCH, WEB,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Squishy INT-caster. Fire Bolt as the at-will ranged option, Magic
/// Missile and Burning Hands as level-1 nuke / AoE. Stat shape mirrors
/// the cleric (low HP, medium AC, tunes around ranged spell attacks)
/// but uses INT as the spellcasting ability so a separate save DC and
/// attack mod come into play.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*THUNDERWAVE);
    actions.push(&*BURNING_HANDS);
    actions.push(&*CAUSE_FEAR);
    actions.push(&*WEB);
    actions.push(&*BLINDNESS);
    actions.push(&*MISTY_STEP);
    actions.push(&*SHIELD);
    actions.push(&*ACID_SPLASH);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*POISON_SPRAY);
    actions.push(&*RAY_OF_SICKNESS);
    actions.push(&*SHOCKING_GRASP);
    actions.push(&*TOLL_THE_DEAD);
    actions.push(&*SHATTER);
    actions.push(&*SLEEP);
    actions.push(&*CHARM_PERSON);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*COLOR_SPRAY);
    actions.push(&*FIREBALL);
    actions.push(&*MAGIC_WEAPON);
    actions.push(&*SCORCHING_RAY);
    actions.push(&*LIGHTNING_BOLT);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*HYPNOTIC_PATTERN);
    actions.push(&*BLUR);
    actions.push(&*INVISIBILITY);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*MIND_SLIVER);
    actions.push(&*HOLD_MONSTER);
    actions.push(&*HASTE);
    actions.push(&*SLOW);
    actions.push(&*CONE_OF_COLD);
    actions.push(&*STINKING_CLOUD);
    actions.push(&*TRUE_STRIKE);
    CreatureTemplate {
        name: "Wizard",
        // 'M' (mage) — keeps 'W' free for Wolf, which already claims it.
        glyph: 'M',
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 11,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 4 level-1 / 3 level-2 / 3 level-3 / 1 level-4 / 1 level-5 —
        // typical level-9 wizard loadout. The level-3 slot fuels exactly
        // one Fireball / Haste / Slow / Stinking Cloud; the level-5 slot
        // is reserved for Cone of Cold or Hold Monster.
        spell_slots_by_level: vec![4, 3, 3, 1, 1],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Wizards are proficient in INT and WIS saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
