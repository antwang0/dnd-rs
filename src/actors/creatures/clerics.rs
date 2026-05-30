use crate::actions::class_features::{
    DIVINE_STRIKE, DIVINE_STRIKE_TAG, PRESERVE_LIFE, PRESERVE_LIFE_TAG, TURN_UNDEAD, TURN_UNDEAD_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    AID, ANIMATE_DEAD, AURA_OF_VITALITY, BANE, BEACON_OF_HOPE, BESTOW_CURSE, BLESS, CALM_EMOTIONS,
    COMMAND, COUNTERSPELL, CROWN_OF_STARS, CRUSADERS_MANTLE, CURE_WOUNDS, DAYLIGHT, DEATH_WARD,
    DISPEL_MAGIC, DIVINE_FAVOR, EARTHQUAKE, FAERIE_FIRE, FEAR, FLAME_STRIKE, FLY,
    GREATER_RESTORATION, GUIDING_BOLT, HASTE, HEAL_SPELL_HIGH, HEALING_SPIRIT, HEALING_WORD,
    HEROES_FEAST, HEROISM, HOLD_PERSON, HOLY_AURA, HOLY_WORD, INFLICT_WOUNDS, INSECT_PLAGUE,
    LESSER_RESTORATION, MASS_CURE_WOUNDS, MASS_HEAL, MASS_HEALING_WORD, PLANT_GROWTH,
    POWER_WORD_HEAL, PRAYER_OF_HEALING, PROTECTION_FROM_EVIL_AND_GOOD, RESURRECTION, REVIVIFY,
    SACRED_BURST, SACRED_FLAME, SANCTUARY, SHIELD_OF_FAITH, SPARE_THE_DYING, SPIKE_GROWTH,
    SPIRIT_GUARDIANS, SPIRIT_SHROUD, SPIRITUAL_WEAPON, STONESKIN, SUGGESTION, SUNBEAM, SUNBURST,
    THORN_WHIP, TOLL_THE_DEAD, TRUE_RESURRECTION, WISH, WORD_OF_RADIANCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Acolyte-style spellcaster. WIS-primary; Sacred Flame as the staple
/// damage option, Healing Word and Cure Wounds for support, Bless for
/// pre-buff, Hold Person for lockdown. Modeled to be roughly equivalent
/// to MM Acolyte (CR 1/4) — light HP, medium AC, no melee.
pub static CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
    actions.push(&HEALING_WORD);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*BLESS);
    actions.push(&*GUIDING_BOLT);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*BANE);
    actions.push(&*SPIRITUAL_WEAPON);
    actions.push(&*AID);
    actions.push(&*INFLICT_WOUNDS);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*THORN_WHIP);
    actions.push(&*SPARE_THE_DYING);
    actions.push(&*TOLL_THE_DEAD);
    actions.push(&*HEROISM);
    actions.push(&*MASS_HEALING_WORD);
    actions.push(&*PROTECTION_FROM_EVIL_AND_GOOD);
    actions.push(&*COMMAND);
    actions.push(&*DIVINE_FAVOR);
    actions.push(&*SPIRIT_GUARDIANS);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*MASS_CURE_WOUNDS);
    actions.push(&*HASTE);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*DEATH_WARD);
    actions.push(&*REVIVIFY);
    actions.push(&*BEACON_OF_HOPE);
    actions.push(&*STONESKIN);
    actions.push(&*HEAL_SPELL_HIGH);
    actions.push(&*WORD_OF_RADIANCE);
    actions.push(&*CALM_EMOTIONS);
    actions.push(&*SUGGESTION);
    actions.push(&*SUNBURST);
    actions.push(&*MASS_HEAL);
    actions.push(&*PRAYER_OF_HEALING);
    actions.push(&*SUNBEAM);
    actions.push(&*RESURRECTION);
    actions.push(&*POWER_WORD_HEAL);
    // Newly added spells (lv1 Sanctuary; lv2 Healing Spirit; lv3 Daylight
    // and Aura of Vitality; lv5 Insect Plague; lv9 True Resurrection).
    actions.push(&*SANCTUARY);
    actions.push(&*HEALING_SPIRIT);
    actions.push(&*DAYLIGHT);
    actions.push(&*AURA_OF_VITALITY);
    actions.push(&*INSECT_PLAGUE);
    actions.push(&*TRUE_RESURRECTION);
    // Druid-flavored Spike Growth (lv2 control) plus Counterspell at
    // lv3 (anti-caster reaction approximation).
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*COUNTERSPELL);
    // Newest divine additions: lv3 Crusader's Mantle (+1d4 radiant per
    // hit aura buff), lv7 Crown of Stars (+1d8 radiant per hit halo),
    // lv8 Earthquake (STR save AoE prone), lv9 Wish (mass-heal allies).
    actions.push(&*CRUSADERS_MANTLE);
    actions.push(&*CROWN_OF_STARS);
    actions.push(&*EARTHQUAKE);
    actions.push(&*WISH);
    // Latest divine additions: lv3 Fear (cone WIS save → Frightened) and
    // lv5 Greater Restoration (cleanse + heal). Both are workhorse
    // mid-level utilities the cleric was missing.
    actions.push(&*FEAR);
    actions.push(&*GREATER_RESTORATION);
    // Flame Strike — lv5 mixed-damage AoE (fire + radiant). Slips past
    // fire-resistant fiends (radiant lands) and radiant-resistant
    // celestials (fire lands).
    actions.push(&*FLAME_STRIKE);
    // Turn Undead — Cleric Channel Divinity, once per long rest.
    actions.push(&*TURN_UNDEAD);
    // Divine Strike — Cleric class feature (RAW: passive at level 8; we
    // model it as a once-per-rest bonus-action prime that lands +1d8
    // radiant on the next melee hit via the OnHitRider table). Pairs
    // well with the cleric's melee cantrip (Thorn Whip) and weapon
    // attacks for the rare hit-and-spike moment.
    actions.push(&*DIVINE_STRIKE);
    // Latest cross-school additions: lv3 Animate Dead (necromancy ally
    // spawn) + Spirit Shroud (concentration on-hit cold rider), and the
    // lv8 Holy Aura (concentration save-advantage burst aura).
    actions.push(&*ANIMATE_DEAD);
    actions.push(&*SPIRIT_SHROUD);
    actions.push(&*HOLY_AURA);
    // Latest druidic-flavored additions for the cleric kit: lv3 Plant
    // Growth (Entangle AoE) and lv3 Fly (concentration ally speed buff).
    actions.push(&*PLANT_GROWTH);
    actions.push(&*FLY);
    // Level-6 apex pre-fight buff: ally-burst temp HP + heal + Heroic.
    // Costs the cleric's only level-6 slot, so it's a one-off opener.
    actions.push(&*HEROES_FEAST);
    // Level-7 apex anti-enemy radiant burst with HP-tiered conditions.
    // Pairs with Resurrection for the cleric's level-7 slot economy.
    actions.push(&*HOLY_WORD);
    // Latest cleric additions: lv4 Guardian of Faith (radiant burst,
    // flat-20 / save-half) and lv6 Blade Barrier (concentration slashing
    // burst). Both round out the cleric's high-tier damage lane with
    // mid-cost AoE options between Flame Strike (lv5) and Sunburst (lv8).
    actions.push(&*crate::actions::spells::GUARDIAN_OF_FAITH);
    actions.push(&*crate::actions::spells::BLADE_BARRIER);
    // Guidance — cleric / druid divination cantrip. Touch range; applies
    // the Inspired flat-buff (+3 to next attack / save / check) on the
    // target. Fills the "pre-fight ally prime" cantrip lane that was
    // previously empty for clerics. Custom-validate gates against
    // re-priming an already-inspired ally.
    actions.push(&*crate::actions::spells::GUIDANCE);
    // Warding Bond — lv2 abjuration. Touch-range damage-share bond:
    // bonded ally gains +1 AC, +1 saves, resistance to all damage; the
    // caster takes the same (post-resistance) damage every time the
    // ally is hit. Pairs the cleric's defensive lane with a damage-
    // sink role — a frontline fighter behind the bond effectively gets
    // 50% damage reduction while the cleric pays the other 50%.
    actions.push(&*crate::actions::spells::WARDING_BOND);
    // Latest divine additions:
    //   - lv6 **Harm**: 14d6 necrotic single-target CON-save for half
    //     plus max-HP drain on fail (cleric's signature offensive nuke,
    //     opposite of Heal in the lv6 slot lane).
    //   - lv6 **Circle of Death**: 8d6 necrotic 30ft-radius CON-save
    //     burst (mass damage that pairs cleanly against undead
    //     necrotic-resistant enemies via the necrotic-immunity routing).
    //   - lv7 **Regenerate**: 4d8+15 touch heal (high-burst single-
    //     target heal, fills the lv7 slot lane next to Resurrection).
    actions.push(&*crate::actions::spells::HARM);
    actions.push(&*crate::actions::spells::CIRCLE_OF_DEATH);
    actions.push(&*crate::actions::spells::REGENERATE);
    actions.push(&*crate::actions::spells::PROTECTION_FROM_ENERGY);
    actions.push(&*crate::actions::spells::REMOVE_CURSE);
    actions.push(&*crate::actions::spells::ANTILIFE_SHELL);
    // Preserve Life — Cleric Channel Divinity (Life Domain in RAW; we
    // expose it generically here). Once per short rest pool of 5 × level
    // HP, healing the most-wounded allies first up to half max HP each.
    // Mass-stabilizer to balance the cleric's offensive Channel Divinity
    // (Turn Undead) — the same action-economy slot, different lane.
    actions.push(&*PRESERVE_LIFE);
    CreatureTemplate {
        name: "Cleric",
        glyph: 'C',
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 10,
        dexterity: 10,
        wisdom: 14, // primary spellcasting ability
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 4/3/3/2/2/1/1/1/3 — cleric loadout extended to support the
        // full SRD spell list now in their kit. Level-3 slot covers
        // Mass Healing Word / Spirit Guardians / Beacon of Hope /
        // Haste / Daylight / Aura of Vitality; level-4 slot covers
        // Stoneskin / Death Ward; level-5 covers Mass Cure Wounds /
        // Insect Plague; level-6 fuels one Heal or one Sunbeam
        // (concentration — only one at a time anyway); the new
        // level-7 slot powers exactly one Resurrection; the level-8
        // slot powers a single Sunburst; and the level-9 row jumps to
        // 3 so Mass Heal, Power Word Heal, and True Resurrection can
        // each fire once per long rest.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 3],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Clerics are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([TURN_UNDEAD_TAG, DIVINE_STRIKE_TAG, PRESERVE_LIFE_TAG]),
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
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
