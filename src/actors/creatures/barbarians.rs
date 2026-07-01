use crate::actions::action_template::Action;
use crate::actions::class_features::{
    BEAR_TOTEM_TAG, EAGLE_DIVE, EAGLE_TOTEM_TAG, FAST_MOVEMENT_TAG, FRENZY, FRENZY_TAG, RAGE,
    RAGE_TAG, RELENTLESS_RAGE_TAG, TIGER_TOTEM_TAG, WOLF_TOTEM_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Shared totem-barbarian build. Every Path of the Totem Warrior sub
/// (Bear / Wolf / Eagle / Tiger) lands on the same level-9 envelope —
/// 76 HP (9d12+18), AC 15 (unarmored), STR/CON 18, Reckless Attack /
/// Brutal Critical 1d / Relentless Rage. The only per-totem swaps are
/// (a) display name + glyph, (b) the totem feature tag in `features`,
/// and (c) an optional extra action (Eagle Dive on the Eagle variant).
/// One helper collapses the four 30-line struct literals into a single
/// call per LazyLock — adding a fifth sub (Wild Heart 2024 Elk / Wolverine
/// / etc.) lands as a one-line entry.
fn totem_barbarian_template(
    name: &'static str,
    glyph: char,
    totem_tag: &'static str,
    extra_actions: &[&'static (dyn Action + Send + Sync)],
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&*RAGE);
    actions.push(&*RECKLESS_ATTACK);
    for &a in extra_actions {
        actions.push(a);
    }
    CreatureTemplate {
        name,
        glyph,
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
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        features: HashSet::from([RAGE_TAG, totem_tag, RELENTLESS_RAGE_TAG, FAST_MOVEMENT_TAG]),
        has_danger_sense: true,
        has_extra_attack: true,
        brutal_critical_dice: 1,
        // 5e Barbarian Feral Instinct (level 7 passive): advantage on
        // initiative rolls. Level-9 template picks this up per RAW.
        // Read by `ActorInstance::roll_initiative` — the d20 is rolled
        // twice and the higher is kept. Every totem subclass inherits
        // through the shared helper so a Bear / Wolf / Eagle / Tiger
        // totem barbarian all open the round competitively.
        has_feral_instinct: true,
        ..CreatureTemplate::defaults()
    }
}

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
        //   - `RELENTLESS_RAGE_TAG`: Barbarian level 11 passive — a
        //     killing blow against a raging barbarian rolls a CON save
        //     (DC 10, +5 each successful use, resets on rest); on a
        //     pass HP pins at 1 instead of dropping the barbarian. The
        //     CR-4 (level-9) template lists this above its strict RAW
        //     level gate for the same reason Improved Divine Smite
        //     ships on the CR-1.5 paladin template — class templates
        //     target a balanced playable level, not lockstep PHB
        //     progression.
        // Bear Totem (Path of the Totem Warrior) lives on the separate
        // `TOTEM_BARBARIAN_TEMPLATE` below so subclass features don't
        // stack RAW-illegally on a single PC build.
        //
        // `FAST_MOVEMENT_TAG` (level 5) is a base-class feature — every
        // barbarian picks it up regardless of subclass, so it lives here
        // AND on the shared totem helper below. +10 ft walking speed
        // always-on (RAW gates on "not wearing heavy armor" but our
        // engine doesn't model armor tiers).
        features: HashSet::from([RAGE_TAG, FRENZY_TAG, RELENTLESS_RAGE_TAG, FAST_MOVEMENT_TAG]),
        has_danger_sense: true,
        has_extra_attack: true,
        // Level 9 Brutal Critical: +1 weapon die on melee crits.
        brutal_critical_dice: 1,
        // 5e Barbarian Feral Instinct (level 7 passive): advantage on
        // initiative rolls. Level-9 template picks this up per RAW.
        // Read by `ActorInstance::roll_initiative` — the d20 is rolled
        // twice and the higher is kept. Composes with Danger Sense's
        // DEX-save advantage on the first round so the barbarian
        // survives the enemy caster's opener even if they win init.
        has_feral_instinct: true,
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
    // Glyph 'T' — distinct from 'B' (baseline Barbarian) so a Berserker-
    // vs-Totem encounter renders unambiguously on the map. The Bear Totem
    // damage envelope reads off `has_passive_feature` at
    // `has_condition_resistance` so Bear Totem composes with the level-11
    // Relentless Rage save-intercept — a Bear Totem barbarian halves the
    // incoming hit *and* gets a chance to pin at 1 HP if it still kills.
    totem_barbarian_template("Totem Barbarian", 'T', BEAR_TOTEM_TAG, &[])
});

/// Wolf Totem Barbarian — Path of the Totem Warrior, **Wolf Spirit** flavor
/// (level 3). Third sibling of `BARBARIAN_TEMPLATE` (Berserker / Frenzy) and
/// `TOTEM_BARBARIAN_TEMPLATE` (Bear Spirit) — same level-9 envelope with the
/// subclass feature swapped to the pack-hunter aura.
///
/// Headline mechanic: **Wolf Totem Spirit** — while raging, allies have
/// advantage on melee attacks against any creature footprint-adjacent to
/// the wolf barbarian. The "team-anchor" role: the wolf barbarian doesn't
/// gain personal damage resistance (Bear) or extra mobility (Eagle), but
/// every teammate's melee swing into an adjacent enemy lands with
/// advantage — a force multiplier on a party with multiple melee
/// attackers. Read at `compute_attack_mode` next to the Pack Tactics
/// branch (same ally-side adjacency loop, gated on raging + passive
/// feature rather than the pack-tactics template trait).
///
/// Stat envelope mirrors the other two totem variants (CR 4 / level-9
/// build / 76 HP / AC 15 / STR / CON 18 / Brutal Critical 1d / Reckless
/// Attack). Glyph 'W' so wolf-vs-bear-vs-eagle renders unambiguously on
/// the map next to baseline Barbarian 'B' / Totem 'T'.
pub static WOLF_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template("Wolf Totem Barbarian", 'W', WOLF_TOTEM_TAG, &[])
});

/// Eagle Totem Barbarian — Path of the Totem Warrior, **Eagle Spirit** flavor
/// (level 3). Fourth sibling of the totem family — Berserker (Frenzy), Bear
/// (damage envelope), Wolf (ally-aura), Eagle (mobility). Same level-9
/// envelope with the subclass feature swapped to the skirmisher kit.
///
/// Headline mechanic: **Eagle Totem Spirit** — while raging, the eagle
/// barbarian can Dash as a bonus action (the `EAGLE_DIVE` action grants
/// a fresh chunk of movement equal to the holder's speed, gated on
/// raging + passive feature). The kiter / repositioner role — combos
/// naturally with Reckless Attack so the eagle barbarian charges in,
/// strikes with advantage, and dives back out beyond easy-OA range
/// before the next enemy turn lands.
///
/// Glyph 'A' (for Aquila / Avian) so eagle-vs-wolf-vs-bear renders
/// unambiguously on the map next to baseline Barbarian 'B' / Totem 'T'
/// / Wolf 'W'.
pub static EAGLE_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template(
        "Eagle Totem Barbarian",
        'A',
        EAGLE_TOTEM_TAG,
        &[&*EAGLE_DIVE],
    )
});

/// Tiger Totem Barbarian — Path of the Totem Warrior, **Tiger Spirit** flavor
/// (level 3, 2024 PHB Path of the Wild Heart). Fifth and final sibling of
/// the totem family — Berserker (Frenzy), Bear (damage envelope), Wolf
/// (ally-aura), Eagle (bonus-action Dash), Tiger (always-on mobility).
/// Same level-9 envelope with the subclass feature swapped to the
/// always-on skirmisher kit.
///
/// Headline mechanic: **Tiger Totem Spirit** — while raging, the tiger
/// barbarian's walking speed increases by 10 ft. Unlike Eagle (bonus-action
/// Dash for one fresh movement chunk), the Tiger's speed bump is always on
/// the moment Rage lands — the bonus action stays free for Reckless Attack
/// / Frenzy / Cunning-Strike-style primes. Pairs naturally with the
/// barbarian's existing reach-closer / chase patterns.
///
/// Glyph 'I' (for tIger — 'T' is taken by the baseline Totem template) so
/// tiger-vs-eagle-vs-wolf-vs-bear-vs-baseline renders unambiguously on the
/// map.
pub static TIGER_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template("Tiger Totem Barbarian", 'I', TIGER_TOTEM_TAG, &[])
});
