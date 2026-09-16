use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WARHAMMER;
use crate::actions::species::{STONECUNNING, STONECUNNING_TAG};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Mountain Dwarf Defender — CON-tank fighter chassis. The defining
/// racial trait is **Dwarven Resilience**: advantage on saving throws
/// against poison AND resistance to poison damage. Hooked into
/// `compute_save_mode` (advantage on CON saves, which the engine treats
/// as the poison-save proxy since saves aren't tagged by source type)
/// and `effective_damage` (poison resistance, folded into the template-
/// resistance lane next to `DamageModifier::Resistance`).
///
/// Stat shape targets a level-3 mountain dwarf fighter: AC 18 (chain
/// mail + shield approximation baked into the flat 18), 33 HP (3d10+12
/// — CON 16 + the Dwarven Toughness +1/level baked into the dice
/// expression), STR 16, warhammer as the signature 1d8 bludgeoning
/// swing. Fighter chassis (Second Wind + Action Surge) gives the dwarf
/// a reliable second swing and a self-heal — pairs with the high CON
/// to push them well past the half-orc / fighter baseline survivability.
///
/// 5e Dwarven Toughness (RAW: +1 HP per character level) is folded into
/// the HP roll (3d10+12 versus a non-dwarf fighter's 3d10+9 — the +3
/// represents 1 HP per level over the fighter's standard +3 CON mod).
pub static DWARF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WARHAMMER);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    // SRD 5.2 Dwarf **Stonecunning** — the species trait that had no
    // surface until tremorsense grew one. See
    // `crate::actions::species::STONECUNNING_TAG`.
    actions.push(&*STONECUNNING);
    CreatureTemplate {
        name: "Mountain Dwarf Defender",
        // 'D' — distinct from 'd' (drow, druid). Mountain dwarf reads as
        // a sturdy upright capital.
        glyph: 'D',
        ac: 18,
        // 3d10 (fighter hit die) + 12 (CON 16 = +3 × 3 levels) + 3
        // (Dwarven Toughness, +1 HP per level over 3 levels). Folded
        // into the dice expression as +12 since the engine doesn't yet
        // model per-level HP bumps separately.
        hitpoints: "3d10+12".parse().unwrap(),
        // SRD 5.2 Dwarf: *"Speed: 30 feet"*. The species used to be
        // written here at 25 with a comment calling that RAW — which it
        // was, in the previous printing, where the dwarf traded five
        // feet for the clause about heavy armour. SRD 5.2 prints 30 and
        // drops the trade, so the engine's dwarf was paying a price for
        // a rule it never had (this engine models no armour speed
        // penalty, so the clause was worth nothing and the five feet
        // were worth five feet).
        speed: 30.,
        strength: 16,
        dexterity: 11,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 10,
        // SRD 5.2 Dwarf: *"Darkvision. You have Darkvision with a
        // range of 120 feet."* Twice the sixty the previous printing
        // gave, and the same 120 the Orc lineage already carries — the
        // two are the deep-dark species and the sheet should say so on
        // both.
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::Dwarvish]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        // The **Crusher** feat, and the dwarf is who it belongs to: the
        // one chassis on the roster whose signature weapon is a
        // warhammer, and the one whose whole build is standing in front
        // of somebody. Both halves of the feat are about what the hammer
        // does to a target's footing — five feet of it on any hit, and
        // an open guard on a critical. See
        // `crate::actions::feats::CRUSHER_TAG`.
        features: HashSet::from([
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            STONECUNNING_TAG,
            crate::actions::feats::CRUSHER_TAG,
        ]),
        // 5e Dwarven Resilience: advantage on saves vs poison AND
        // resistance to poison damage. The single flag drives both halves.
        has_dwarven_resilience: true,
        ..CreatureTemplate::defaults()
    }
});
