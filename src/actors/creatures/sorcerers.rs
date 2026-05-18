use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    ACID_SPLASH, BURNING_HANDS, CHAIN_LIGHTNING, CHARM_PERSON, CHILL_TOUCH, CONE_OF_COLD,
    COUNTERSPELL, DISINTEGRATE, FEAR, FIREBALL, FIRE_BOLT, FLY, HASTE, HOLD_PERSON,
    LIGHTNING_BOLT, MAGE_ARMOR, MAGIC_MISSILE, METEOR_SWARM, MIRROR_IMAGE, MISTY_STEP,
    POLYMORPH, POWER_WORD_KILL, RAY_OF_FROST, SCORCHING_RAY, SHATTER, SHIELD, SHOCKING_GRASP,
    SLEEP, SUNBURST, TIME_STOP,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Sorcerer PC template. CHA-primary full-caster — uses the same spell
/// roster as the wizard but anchors on Charisma rather than Intelligence.
/// The defining mechanical difference (RAW): Sorcery Points + Metamagic.
/// We don't model the per-cast metamagic toggle yet (would need an
/// ActionOverride wired through every spell's resolution); the template
/// instead leans on a tighter "blaster" spell list and higher CHA-anchored
/// DCs — sorcerers in this engine play as the wizard's evocation-first
/// cousin with extra CON / CHA save resilience.
///
/// Loadout: Fire Bolt / Ray of Frost / Chill Touch / Acid Splash / Shocking
/// Grasp cantrips for at-will, Burning Hands / Magic Missile / Shield as
/// level-1 staples, Scorching Ray / Mirror Image / Shatter / Misty Step
/// at level-2, Fireball / Lightning Bolt / Counterspell / Fear / Haste at
/// level-3, Polymorph at level-4, Cone of Cold at level-5, Chain Lightning
/// at level-6, Time Stop at level-9, Power Word Kill / Meteor Swarm /
/// Sunburst for the apex slots. PC flag flips on so the sorcerer rolls
/// death saves at 0 HP.
pub static SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    // Cantrips
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*ACID_SPLASH);
    actions.push(&*SHOCKING_GRASP);
    // Level 1
    actions.push(&*BURNING_HANDS);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SHIELD);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    // Level 2
    actions.push(&*SCORCHING_RAY);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*SHATTER);
    actions.push(&*MISTY_STEP);
    actions.push(&*HOLD_PERSON);
    // Level 3
    actions.push(&*FIREBALL);
    actions.push(&*LIGHTNING_BOLT);
    actions.push(&*COUNTERSPELL);
    actions.push(&*FEAR);
    actions.push(&*HASTE);
    actions.push(&*FLY);
    // Level 4
    actions.push(&*POLYMORPH);
    // Level 5
    actions.push(&*CONE_OF_COLD);
    // Level 6
    actions.push(&*CHAIN_LIGHTNING);
    // Level 6 — Disintegrate as the sorcerer's apex single-target nuke.
    actions.push(&*DISINTEGRATE);
    // Level 8
    actions.push(&*SUNBURST);
    // Level 9
    actions.push(&*TIME_STOP);
    actions.push(&*POWER_WORD_KILL);
    actions.push(&*METEOR_SWARM);
    CreatureTemplate {
        name: "Sorcerer",
        // 'S' — distinct from Skeleton (lowercase 's'), Sage, etc.
        glyph: 'S',
        ac: 13, // unarmored, +DEX
        hitpoints: "9d6+18".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 11,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 18, // primary spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        actions,
        // Level-9 full-caster loadout — mirrors wizard / cleric / druid.
        // Sorcerers have the same slot table as wizards RAW; the
        // difference shows up in spells-known caps, not slots.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Sorcerers are proficient in CON and CHA saves (5e PHB) —
        // distinct from the wizard's INT/WIS profile and the cleric's
        // WIS/CHA, which makes them tankier against the concentration-
        // breaking CON saves that hit casters in the heat of melee.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
