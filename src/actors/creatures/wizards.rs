use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    ACID_SPLASH, BANISHMENT, BESTOW_CURSE, BLINDNESS, BLUR, BOOMING_BLADE, BURNING_HANDS,
    CAUSE_FEAR, CHAIN_LIGHTNING, CHARM_PERSON, CHILL_TOUCH, CLOUDKILL, CLOUD_OF_DAGGERS,
    COLOR_SPRAY, CONE_OF_COLD, COUNTERSPELL, CROWN_OF_MADNESS, CROWN_OF_STARS, DIMENSION_DOOR,
    DISINTEGRATE, DISPEL_MAGIC, EARTHQUAKE, FEAR, FINGER_OF_DEATH, FIREBALL, FIRE_BOLT,
    FIRE_SHIELD, FLAME_STRIKE, FORCECAGE, GLOBE_OF_INVULNERABILITY, GREATER_INVISIBILITY, HASTE,
    HEAT_METAL, HOLD_MONSTER, HYPNOTIC_PATTERN, ICE_STORM, INVISIBILITY, LIGHTNING_BOLT,
    MAGE_ARMOR, MAGIC_MISSILE, MAGIC_WEAPON, MASS_SUGGESTION, METEOR_SWARM, MIND_SLIVER, MIND_WHIP,
    MIRROR_IMAGE, MISTY_STEP, PHANTASMAL_KILLER, POISON_SPRAY, POLYMORPH, POWER_WORD_KILL,
    POWER_WORD_STUN, RAY_OF_FROST, RAY_OF_SICKNESS, SCORCHING_RAY, SHATTER, SHIELD, SHOCKING_GRASP,
    SLEEP, SLOW, SPIKE_GROWTH, STINKING_CLOUD, STONESKIN, SUGGESTION, SUNBEAM, SYNAPTIC_STATIC,
    TASHAS_HIDEOUS_LAUGHTER, TELEKINESIS, THUNDERWAVE, TIME_STOP, TOLL_THE_DEAD, TRUE_STRIKE,
    VAMPIRIC_TOUCH, WALL_OF_FIRE, WALL_OF_FORCE, WEB, WISH, WITCH_BOLT,
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
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*GREATER_INVISIBILITY);
    actions.push(&*ICE_STORM);
    actions.push(&*WITCH_BOLT);
    actions.push(&*TASHAS_HIDEOUS_LAUGHTER);
    actions.push(&*CLOUD_OF_DAGGERS);
    actions.push(&*CROWN_OF_MADNESS);
    actions.push(&*PHANTASMAL_KILLER);
    actions.push(&*BANISHMENT);
    actions.push(&*STONESKIN);
    actions.push(&*SYNAPTIC_STATIC);
    actions.push(&*DISINTEGRATE);
    actions.push(&*FINGER_OF_DEATH);
    actions.push(&*POWER_WORD_STUN);
    actions.push(&*SUGGESTION);
    actions.push(&*MASS_SUGGESTION);
    actions.push(&*POWER_WORD_KILL);
    actions.push(&*METEOR_SWARM);
    actions.push(&*SUNBEAM);
    // Newly added wizard spells (lv4 Dimension Door / Wall of Fire / Fire
    // Shield; lv5 Cloudkill / Wall of Force).
    actions.push(&*DIMENSION_DOOR);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*FIRE_SHIELD);
    actions.push(&*CLOUDKILL);
    actions.push(&*WALL_OF_FORCE);
    // Latest additions: lv2 Spike Growth (control), lv3 Counterspell
    // (anti-caster), lv4 Polymorph (transformation buff), lv5
    // Telekinesis (forced movement), lv6 Globe of Invulnerability
    // (mass damage reduction).
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*COUNTERSPELL);
    actions.push(&*POLYMORPH);
    actions.push(&*TELEKINESIS);
    actions.push(&*GLOBE_OF_INVULNERABILITY);
    // Latest spell additions: cantrip Booming Blade (melee thunder rider
    // on movement), lv2 Mind Whip (INT save psychic + action-economy
    // debuff), lv7 Forcecage (CHA save imprisonment) + Crown of Stars
    // (radiant per-hit rider self-buff), lv8 Earthquake (STR save AoE
    // bludgeoning + prone), lv9 Time Stop (extra action / bonus action),
    // lv9 Wish (mass-heal allies).
    actions.push(&*BOOMING_BLADE);
    actions.push(&*MIND_WHIP);
    actions.push(&*FORCECAGE);
    actions.push(&*CROWN_OF_STARS);
    actions.push(&*EARTHQUAKE);
    actions.push(&*TIME_STOP);
    actions.push(&*WISH);
    // Latest arcane addition: lv3 Fear (cone WIS save → Frightened),
    // a clean illusion-control option missing from the wizard list.
    actions.push(&*FEAR);
    // Latest evocation / transmutation additions: lv2 Heat Metal (con
    // DoT + attack disadvantage), lv5 Flame Strike (mixed fire+radiant
    // AoE), lv6 Chain Lightning (forks to 3 nearby creatures).
    actions.push(&*HEAT_METAL);
    actions.push(&*FLAME_STRIKE);
    actions.push(&*CHAIN_LIGHTNING);
    // Latest necromancy / divination additions: lv3 Animate Dead (raise
    // skeleton ally) and lv9 Foresight (single-target apex buff).
    actions.push(&*crate::actions::spells::ANIMATE_DEAD);
    actions.push(&*crate::actions::spells::FORESIGHT);
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
        // 4/3/3/2/2/1/1/1/1 — typical level-17 wizard archmage loadout.
        // The high-level slots (6+) fuel exactly one Disintegrate / Heal,
        // Finger of Death, and Power Word Stun apiece — late-game
        // emergency buttons rather than spam fodder. Mid-level slots
        // (3-5) still cover Fireball / Haste / Slow / Stinking Cloud /
        // Cone of Cold / Hold Monster / Synaptic Static.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
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
