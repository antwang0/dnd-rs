use crate::actions::class_features::{
    DIVINE_SMITE, LAY_ON_HANDS, LAY_ON_HANDS_TAG, SACRED_WEAPON, SACRED_WEAPON_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATSWORD;
use crate::actions::spells::{
    AURA_OF_LIFE, BANISHING_SMITE, BLESS, BLINDING_SMITE, BRANDING_SMITE, COMPELLED_DUEL,
    CURE_WOUNDS, DESTRUCTIVE_WAVE, HEALING_WORD, LESSER_RESTORATION, SEARING_SMITE,
    SHIELD_OF_FAITH, STAGGERING_SMITE, THUNDEROUS_SMITE, WRATHFUL_SMITE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
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
    actions.push(&THUNDEROUS_SMITE);
    actions.push(&BRANDING_SMITE);
    actions.push(&BLINDING_SMITE);
    // Higher-tier smite primes — lv4 Staggering Smite (psychic + WIS-save
    // Stunned-1) and lv5 Banishing Smite (force + HP≤50 auto-banish via
    // the Mazed envelope). Slot cost climbs but the rider impact does
    // too; the half-caster slot bump below fuels both.
    actions.push(&STAGGERING_SMITE);
    actions.push(&BANISHING_SMITE);
    // Aura of Life — lv4 abjuration, concentration; allies in 30ft sphere
    // gain DeathWarded (next killing-blow drop intercepted) for the
    // duration. Bumps the paladin into the lv4 slot table; combined with
    // the half-caster ramp below it gives the late-game paladin a true
    // mass-save-the-party button alongside the smite primes.
    actions.push(&*AURA_OF_LIFE);
    // Destructive Wave — lv5 evocation. Self-burst (6-tile radius)
    // enemy-only CON-save burst dealing 5d6 thunder + 5d6 radiant +
    // prone-on-fail. Non-concentration — pairs cleanly with whichever
    // smite the paladin's currently holding. The split damage type
    // slips past single-element resistance the same way Flame Strike
    // (fire + radiant) does.
    actions.push(&*DESTRUCTIVE_WAVE);
    // Warding Bond — lv2 abjuration. Touch-range damage-share bond.
    // The paladin already takes the hits up front (high HP, AC 18) —
    // bonding a frailer ally (e.g. cleric / wizard) halves their
    // incoming damage at the cost of mirroring the rest onto the
    // paladin's much larger HP pool. Non-concentration, so it stacks
    // with whichever smite is currently holding the slot.
    actions.push(&*crate::actions::spells::WARDING_BOND);
    // Holy Weapon — lv5 evocation, concentration. Self-only buff: every
    // weapon hit gains +2d8 radiant via the on_hit_riders table. Persistent
    // for the duration (not consumed on trigger) — distinct from the
    // one-shot smite primes that hold the same concentration slot, so the
    // AI's smite picker steers around it when it's already up.
    actions.push(&*crate::actions::spells::HOLY_WEAPON);
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
        creature_type: CreatureType::Humanoid,
        actions,
        // Half-caster ramp: 4/3/3/2/1. Bumps lv3 → 3 slots and adds a
        // lv5 slot for Banishing Smite; lv4 stays at 2 to fuel Aura of
        // Life + Staggering Smite. Earlier 4/3/1 envelope is preserved
        // on lv1-2 so Bless / Divine Smite / Compelled Duel spam stays
        // unchanged.
        spell_slots_by_level: vec![4, 3, 3, 2, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Paladins are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([LAY_ON_HANDS_TAG, SACRED_WEAPON_TAG]),
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
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        // Aura of Protection (level 6+): allies within 10ft add the
        // paladin's CHA mod (min +1) to all saves. The headline late-
        // game paladin feature — turns the squishy wizard adjacent to
        // the paladin into a save-throwing tank. Engine reads via
        // `EncounterInstance::aura_of_protection_bonus`.
        has_aura_of_protection: true,
        // Aura of Courage (level 10+): allies within 10ft are immune to
        // Frightened. Suppresses installs at the `ApplyCondition::apply`
        // site so Cause Fear / Wrathful Smite / dragon-fear all bounce
        // off the aura bubble.
        has_aura_of_courage: true,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
