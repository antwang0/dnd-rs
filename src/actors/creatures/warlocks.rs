use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    ACID_SPLASH, ANIMATE_DEAD, BANISHMENT, BESTOW_CURSE, BLINDNESS, BURNING_HANDS,
    CHARM_PERSON, CHILL_TOUCH, COUNTERSPELL, DIMENSION_DOOR, ELDRITCH_BLAST, EYEBITE,
    FEAR, FIRE_BOLT, FLY, HELLISH_REBUKE, HEX, HOLD_MONSTER, HOLD_PERSON, HYPNOTIC_PATTERN,
    INVISIBILITY, MAGE_ARMOR, MISTY_STEP, POISON_SPRAY, POWER_WORD_KILL, POWER_WORD_STUN,
    SHIELD, SLEEP, SUGGESTION, VAMPIRIC_TOUCH, WITCH_BOLT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Warlock PC template. CHA-primary half-caster with Pact Magic — RAW
/// the warlock's defining feature is short-rest spell slots: a small
/// pool (~2-4) that all sit at the warlock's highest available slot
/// level, refreshing on a short rest. The engine doesn't model short
/// rests as a discrete event today (long rest is the only refresh
/// trigger), so we approximate with a flat 4 slots concentrated at
/// level 5 — the load-bearing apex slot the warlock blasts with — plus
/// a thin lower-level spread for situational picks.
///
/// Loadout philosophy:
/// - **Eldritch Blast** as the at-will ranged cantrip (the warlock's
///   signature cantrip, scales with caster level).
/// - **Hex** as the per-encounter rider buff (concentration; pairs with
///   EB swings).
/// - **Hellish Rebuke** as the bonus-action reactive damage.
/// - **Witch Bolt** as the lv1 sustained zap (concentration).
/// - **Hold Person / Suggestion / Hypnotic Pattern** as enchantment
///   control.
/// - **Eyebite / Power Word Kill** as the apex single-target threats.
///
/// Stat shape: AC 12 (unarmored + DEX), 8d8+8 HP (~44), CHA 18.
/// Proficient WIS + CHA saves (RAW). Speaks Common + Infernal (the
/// patron's tongue).
pub static WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // Cantrips (at-will)
    actions.push(&*ELDRITCH_BLAST);
    actions.push(&*FIRE_BOLT);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*ACID_SPLASH);
    actions.push(&*POISON_SPRAY);
    // Level 1 — Hex defines the warlock's rider loop; Hellish Rebuke
    // for reactive burst; Witch Bolt for sustained zap; Mage Armor /
    // Shield for survivability; Charm / Sleep for soft control.
    actions.push(&*HEX);
    actions.push(&*HELLISH_REBUKE);
    actions.push(&*WITCH_BOLT);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*SHIELD);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    actions.push(&*BURNING_HANDS);
    // Level 2 — Misty Step (escape), Hold Person (control), Invisibility,
    // Blindness, Suggestion (single-target charm).
    actions.push(&*MISTY_STEP);
    actions.push(&*HOLD_PERSON);
    actions.push(&*INVISIBILITY);
    actions.push(&*BLINDNESS);
    actions.push(&*SUGGESTION);
    // Level 3 — Fear (cone Frightened), Counterspell (anti-caster),
    // Hypnotic Pattern (AoE charm), Vampiric Touch (sustained life
    // drain), Bestow Curse (single-target debuff), Fly, Animate Dead.
    actions.push(&*FEAR);
    actions.push(&*COUNTERSPELL);
    actions.push(&*HYPNOTIC_PATTERN);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*FLY);
    actions.push(&*ANIMATE_DEAD);
    // Level 4 — Banishment (single-target removal), Dimension Door
    // (teleport).
    actions.push(&*BANISHMENT);
    actions.push(&*DIMENSION_DOOR);
    // Level 5 — Hold Monster (single-target paralysis on a bigger fish).
    actions.push(&*HOLD_MONSTER);
    // Level 6 — Eyebite (single-target sleep), the warlock's apex
    // control. RAW gates Eyebite at lv6; we put it at lv6 here too.
    actions.push(&*EYEBITE);
    // Level 9 — Power Word Stun and Power Word Kill (single-target
    // boss-killers; the warlock's apex damage button).
    actions.push(&*POWER_WORD_STUN);
    actions.push(&*POWER_WORD_KILL);
    CreatureTemplate {
        name: "Warlock",
        // 'L' (uppercase) — distinct from 'l' (Lich), 'W' (Wolf glyph),
        // 'M' (Wizard / Mage). Reads as a robed CHA-caster.
        glyph: 'L',
        ac: 12,
        hitpoints: "8d8+8".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 12,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 18, // primary spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 4.0,
        size: Size::Medium,
        actions,
        // Pact Magic compromise: a flat 4 lv5 slots (the warlock's
        // top-level slots all sit at the highest available slot level
        // RAW). Lower levels carry just 1 slot apiece so the situational
        // picks (Misty Step, Counterspell, etc.) still have ammunition
        // without diluting the "blast with the apex slot" feel.
        // Index: lv1=2, lv2=1, lv3=1, lv4=1, lv5=4, lv6+1 (Eyebite),
        // lv7=0, lv8=0, lv9=1 (Power Word Kill / Stun).
        spell_slots_by_level: vec![2, 1, 1, 1, 4, 1, 0, 0, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Warlocks are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});

