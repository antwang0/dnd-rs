use crate::actions::class_features::{BEAR_TOTEM_TAG, FRENZY, FRENZY_TAG, RAGE, RAGE_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Barbarian PC template. The classic STR-melee bruiser: heavy HP from
/// the 1d12 hit die, modest AC (unarmored — relies on the rage damage
/// reduction instead), STR-primary. Headline mechanics:
/// - **Rage** (bonus action, 1/long rest): resistance to bludgeoning /
///   piercing / slashing, advantage on STR checks/saves.
/// - **Reckless Attack** (bonus action): grants advantage on the next
///   melee swing this turn at the cost of attackers having advantage
///   against the barbarian until their next turn.
/// - **Brutal Critical** (level 9): on a critical melee weapon hit, roll
///   one additional damage die of the weapon's type. Scales to 2 dice
///   at level 13 and 3 at level 17 — we model the level-9 baseline here.
/// - Greataxe (1d12 slashing) as the signature damage weapon.
///
/// Stats target a level-9 barbarian: 76 HP (9d12+18), AC 15 (unarmored
/// defense ≈ 10 + DEX(+1) + CON(+4) at CON 18), STR 18, CON 18 — the
/// classic "rage tank" loadout. Bumped from the prior level-3 build to
/// surface the Brutal Critical rider at the lowest level that grants it.
pub static BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&*RAGE);
    actions.push(&*RECKLESS_ATTACK);
    // Path of the Berserker — Frenzy (subclass lv3 feature). Bonus
    // action while raging that grants a fresh Action for one extra
    // melee swing. The `FRENZY` action gates on the holder having the
    // `FRENZY_TAG` passive feature AND the `Raging` condition active,
    // so a non-Berserker subclass build wouldn't fire it even if both
    // BARBARIAN_TEMPLATE and CHAMPION_TEMPLATE shared the action pool.
    actions.push(&*FRENZY);
    CreatureTemplate {
        name: "Barbarian",
        glyph: 'B',
        ac: 15,
        hitpoints: "9d12+18".parse().unwrap(),
        strength: 18,
        dexterity: 12,
        constitution: 18,
        intelligence: 8,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Barbarians are proficient in STR and CON saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        // Subclass features layered onto the baseline Rage:
        //   - `FRENZY_TAG`: Path of the Berserker (level 3) — enables the
        //     bonus-action Frenzy strike while Raging.
        // Bear Totem (Path of the Totem Warrior) lives on the separate
        // `TOTEM_BARBARIAN_TEMPLATE` below so subclass features don't
        // stack RAW-illegally on a single PC build.
        features: HashSet::from([RAGE_TAG, FRENZY_TAG]),
        has_danger_sense: true,
        has_extra_attack: true,
        // Level 9 Brutal Critical: +1 weapon die on melee crits.
        brutal_critical_dice: 1,
        ..CreatureTemplate::defaults()
    }
});

/// Totem Barbarian — Path of the Totem Warrior, **Bear Spirit** flavor
/// (level 3). Distinct from `BARBARIAN_TEMPLATE` (Berserker / Frenzy
/// flavor) so PCs can be set up against either subclass by name without
/// the two subclass features stacking RAW-illegally on a single build.
///
/// Headline mechanic: **Bear Totem Spirit** — while raging, resistance
/// to every damage type except psychic. Read at the damage-pipeline
/// chokepoint `ActorInstance::has_condition_resistance` so the standard
/// 5e "one halving per damage instance" rule still holds (Bear Totem
/// doesn't stack with a template resistance — the dwarven barbarian
/// still only gets one /2 on poison).
///
/// Same stat envelope as the baseline Barbarian (CR 4, level-9 build,
/// 76 HP, AC 15, STR/CON 18, Brutal Critical 1d, Reckless Attack); the
/// only swap is the subclass feature lane: `BEAR_TOTEM_TAG` replaces
/// `FRENZY_TAG`, and the `FRENZY` action is omitted from the action
/// pool since it gates on the (now-absent) `FRENZY_TAG` flag.
pub static TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&*RAGE);
    actions.push(&*RECKLESS_ATTACK);
    CreatureTemplate {
        name: "Totem Barbarian",
        // 'T' — distinct from 'B' (baseline Barbarian) so a Berserker-
        // vs-Totem encounter renders unambiguously on the map.
        glyph: 'T',
        ac: 15,
        hitpoints: "9d12+18".parse().unwrap(),
        strength: 18,
        dexterity: 12,
        constitution: 18,
        intelligence: 8,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        features: HashSet::from([RAGE_TAG, BEAR_TOTEM_TAG]),
        has_danger_sense: true,
        has_extra_attack: true,
        brutal_critical_dice: 1,
        ..CreatureTemplate::defaults()
    }
});
