use crate::actions::class_features::{
    DIVINE_SMITE, LAY_ON_HANDS, LAY_ON_HANDS_TAG, SACRED_WEAPON, SACRED_WEAPON_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATSWORD;
use crate::actions::spells::{
    BLESS, BLINDING_SMITE, BRANDING_SMITE, COMPELLED_DUEL, CURE_WOUNDS, HEALING_WORD,
    LESSER_RESTORATION, SEARING_SMITE, SHIELD_OF_FAITH, WRATHFUL_SMITE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Paladin PC template. The classic CHA-flavored holy warrior: half-caster
/// for divine spells, full martial for weapon hits, plus a stacked set of
/// once-per-rest class features. Signature mechanics:
/// - **Divine Smite** (bonus action + level-1 slot): primes the next
///   melee hit with +2d8 radiant damage via the Smiting condition.
/// - **Lay on Hands** (action, 1/rest): touch heal for `5 * level + CHA` HP.
/// - **Channel Divinity: Sacred Weapon** (action, 1/rest): +CHA to attack
///   rolls for 10 rounds via the Sacred condition.
/// - **Compelled Duel** (bonus action, level-1 slot): WIS save → Dueled,
///   target eats disadvantage attacking anyone but the paladin.
///
/// Stats target a level-3 paladin: 27 HP (3d10+6 like the Fighter),
/// AC 18 (chain mail + shield baseline), STR 16, CHA 14, half-caster
/// slots (4/2 = level-1 + level-2). Stretches Lay on Hands' big single
/// chunk + a smite or two before the slot pool dries.
pub static PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATSWORD);
    actions.push(&*LAY_ON_HANDS);
    actions.push(&*DIVINE_SMITE);
    actions.push(&*SACRED_WEAPON);
    actions.push(&*BLESS);
    actions.push(&*CURE_WOUNDS);
    actions.push(&HEALING_WORD);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*COMPELLED_DUEL);
    // Smite spells — bonus-action concentration primes that lay extra
    // rider damage (and a follow-up effect for Wrathful / Branding /
    // Blinding) on the paladin's next melee hit. Slot-cost varies per
    // spell (1 / 1 / 2 / 3); the half-caster slot table supports them.
    actions.push(&SEARING_SMITE);
    actions.push(&WRATHFUL_SMITE);
    actions.push(&BRANDING_SMITE);
    actions.push(&BLINDING_SMITE);
    CreatureTemplate {
        name: "Paladin",
        glyph: 'P',
        ac: 18,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 14,
        charisma: 14, // spellcasting ability + Sacred Weapon scaling
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Celestial]),
        cr: 1.5,
        size: Size::Medium,
        actions,
        // Half-caster ramp: 4 level-1, 3 level-2, 1 level-3. Matches
        // level-7 RAW — bumped from level-5 to support the lv3 Blinding
        // Smite addition and keep enough level-1 slots for both Bless
        // and Divine Smite spam.
        spell_slots_by_level: vec![4, 3, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Paladins are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([LAY_ON_HANDS_TAG, SACRED_WEAPON_TAG]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
