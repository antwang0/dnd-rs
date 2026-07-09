use crate::actions::action_template::Action;
use crate::actions::class_features::{
    BEAR_TOTEM_TAG, DIVINE_FURY_TAG, EAGLE_DIVE, EAGLE_TOTEM_TAG, ELK_TOTEM_TAG,
    FAST_MOVEMENT_TAG, FRENZY, FRENZY_TAG, INTIMIDATING_PRESENCE, INTIMIDATING_PRESENCE_TAG,
    MINDLESS_RAGE_TAG, PANTHER_TOTEM_TAG, RAGE, RAGE_TAG, RELENTLESS_RAGE_TAG, TIGER_TOTEM_TAG,
    WOLF_TOTEM_TAG, WOLVERINE_TOTEM_TAG, ZEALOUS_PRESENCE, ZEALOUS_PRESENCE_TAG,
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
        // 5e Barbarian **Persistent Rage** (level 15) — see the
        // baseline `BARBARIAN_TEMPLATE` for the full envelope. Ships
        // on every totem subclass alongside the baseline chassis so
        // the "raging window doubles" identity is uniform across the
        // barbarian family. Composes especially cleanly with Wolf
        // Totem (allies get advantage on the paladin's every swing
        // for twice as many rounds) and Tiger Totem (the +10 ft
        // speed bump holds for the doubled window).
        has_persistent_rage: true,
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
        // 5e Barbarian **Persistent Rage** (level 15 class feature).
        // Passive template flag: the Rage condition installs with a
        // doubled `Rounds(20)` timer instead of the baseline
        // `Rounds(10)`. Read by `Rage::side_effects` — the timer swap
        // is the only mechanical surface. Ships on the CR-4 (level-9)
        // baseline above its strict RAW level gate for the same
        // reason Relentless Rage / Brutal Critical (1d) do — class
        // templates target a balanced playable level, not lockstep
        // PHB progression. Composes cleanly with every other rage-
        // gated feature on the barbarian chassis (Frenzy / Divine
        // Fury / totem spirits / Reckless Attack) — more rage rounds
        // means more turns where those riders fire.
        has_persistent_rage: true,
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

/// Elk Totem Barbarian — Path of the Totem Warrior, **Elk Spirit** flavor
/// (level 3, XGtE expansion of the RAW PHB Bear / Wolf / Eagle triad).
/// Sixth sibling of the totem family — Berserker (Frenzy), Bear (damage
/// envelope), Wolf (ally-aura), Eagle (bonus-action Dash), Tiger
/// (always-on +10 ft rage mobility), Elk (always-on +15 ft rage
/// mobility). Same level-9 envelope with the subclass feature swapped
/// to the bigger sprint totem.
///
/// Headline mechanic: **Elk Totem Spirit** — while raging, the elk
/// barbarian's walking speed increases by 15 ft. Bigger than Tiger's
/// +10 ft but on the same rage gate; the two totem spirits never
/// legally co-occur on a single PC (RAW: one totem spirit pick per
/// barbarian), so the +15 ft magnitude is what distinguishes an Elk
/// build from a Tiger build in the mechanics-visible sense. Stacks
/// additively with Fast Movement (Barbarian lv5, +10 ft always-on) on
/// the shared `PASSIVE_FEATURE_SPEED_BONUSES` table — a raging elk
/// barbarian at level 5+ opens at 55 ft (30 base + 15 Elk + 10 Fast
/// Movement), a full extra move on the opening round vs. a Tiger
/// totem's 50 ft stack.
///
/// Glyph 'E' — distinct from 'A' (Eagle) which is the closest phonetic
/// neighbor, and unused elsewhere in the totem family (Berserker 'Z',
/// baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W', Eagle 'A', Tiger
/// 'I', Zealot handled separately).
pub static ELK_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template("Elk Totem Barbarian", 'E', ELK_TOTEM_TAG, &[])
});

/// Wolverine Totem Barbarian — Path of the Wild Heart, **Wolverine
/// Spirit** flavor (level 3, 2024 PHB Path of the Wild Heart lineup
/// expansion of the RAW XGtE Bear / Wolf / Eagle / Tiger / Elk
/// roster). Seventh sibling of the totem family — Berserker (Frenzy),
/// Bear (damage envelope), Wolf (ally-aura), Eagle (bonus-action
/// Dash), Tiger (rage-gated +10 ft mobility), Elk (rage-gated +15 ft
/// mobility), Wolverine (rage-gated +10 ft mobility). Same level-9
/// envelope with the subclass feature swapped to a Tiger-tier speed
/// bump on a distinct flavor / tag / template.
///
/// Headline mechanic: **Wolverine Totem Spirit** — while raging, the
/// wolverine barbarian's walking speed increases by 10 ft. Matches
/// Tiger's magnitude but on a distinct tag so a Tiger-vs-Wolverine
/// encounter renders unambiguously and the two flags never legally
/// co-occur on a single PC (RAW: one totem spirit pick per
/// barbarian). Stacks additively with Fast Movement (Barbarian lv5,
/// +10 ft always-on) on the shared `PASSIVE_FEATURE_SPEED_BONUSES`
/// table — a raging wolverine barbarian at level 5+ opens at 50 ft
/// (30 base + 10 Wolverine + 10 Fast Movement), matching the Tiger
/// stack magnitude but on a different totem chassis.
///
/// Glyph 'V' — distinct from every other totem glyph (Berserker 'Z',
/// baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W', Eagle 'A',
/// Tiger 'I', Elk 'E', Zealot 'X').
pub static WOLVERINE_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template("Wolverine Totem Barbarian", 'V', WOLVERINE_TOTEM_TAG, &[])
});

/// Panther Totem Barbarian — Path of the Totem Warrior, **Panther
/// Spirit** flavor (level 3, XGtE expansion of the RAW PHB Bear /
/// Wolf / Eagle triad, sibling of Elk). Eighth totem in the codebase's
/// full totem roster — Berserker (Frenzy), Bear (damage envelope),
/// Wolf (ally-aura), Eagle (bonus-action Dash), Tiger (rage-gated
/// +10 ft mobility), Elk (rage-gated +15 ft mobility), Wolverine
/// (rage-gated +10 ft mobility, distinct tag), Panther (rage-gated
/// +5 ft mobility). Same level-9 envelope with the subclass feature
/// swapped to the slinkiest of the sprint-totem magnitudes.
///
/// Headline mechanic: **Panther Totem Spirit** — while raging, the
/// panther barbarian's walking speed increases by 5 ft. RAW XGtE
/// grants climbing speed equal to walking speed; the engine folds
/// that into a smaller flat walking-speed bump since there's no 3D
/// terrain to differentiate the climb axis. Distinct from every
/// other rage-gated totem by magnitude alone: Panther +5, Tiger +10,
/// Wolverine +10, Elk +15. Stacks additively with Fast Movement
/// (Barbarian lv5, +10 ft always-on): a raging panther barbarian at
/// level 5+ opens at 45 ft (30 base + 5 Panther + 10 Fast Movement).
///
/// Glyph 'P' — distinct from every other totem glyph (Berserker 'Z',
/// baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W', Eagle 'A',
/// Tiger 'I', Elk 'E', Wolverine 'V', Zealot 'X').
pub static PANTHER_TOTEM_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    totem_barbarian_template("Panther Totem Barbarian", 'P', PANTHER_TOTEM_TAG, &[])
});

/// Berserker Barbarian — Path of the Berserker subclass build. Identical
/// envelope to the baseline `BARBARIAN_TEMPLATE` (greataxe + reckless
/// attack + rage + brutal critical, level-9 stat block) with one subclass
/// feature layered on top of the existing Frenzy: **Mindless Rage**
/// (Berserker subclass level 6) — passive immunity to Charmed and
/// Frightened while raging.
///
/// The Berserker's signature "no matter what you do, I keep swinging"
/// tell — where a raging barbarian without Mindless Rage still eats a
/// Fear cone / Charm Person and stops attacking, the Berserker just
/// shrugs both off while the rage holds. Read at the flag-driven
/// immunity table in `actor_template.rs` — the closure checks both
/// `has_passive_feature(MINDLESS_RAGE_TAG)` AND `has_condition(Raging)`
/// so the immunity flips off the moment rage drops.
///
/// The baseline `BARBARIAN_TEMPLATE` ships Frenzy already (it's
/// effectively the Berserker chassis at levels 3-5); this template
/// stacks the level-6 Berserker feature on top, so the Berserker is
/// distinct from the baseline by name / glyph AND by the layered
/// Mindless Rage passive. Distinct from the Totem Warrior siblings
/// (`TOTEM_BARBARIAN_TEMPLATE` / `WOLF_TOTEM_BARBARIAN_TEMPLATE` /
/// `EAGLE_TOTEM_BARBARIAN_TEMPLATE` / `TIGER_TOTEM_BARBARIAN_TEMPLATE`)
/// so subclass features don't stack RAW-illegally on a single PC build.
///
/// Glyph 'Z' (for berZerker) so berserker-vs-baseline-vs-totem renders
/// unambiguously on the map next to baseline 'B' / Totem 'T' / Wolf 'W'
/// / Eagle 'A' / Tiger 'I'.
pub static BERSERKER_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Barbarian envelope wholesale
    // (which already ships FRENZY_TAG + FRENZY action) and layer on the
    // MINDLESS_RAGE_TAG passive plus the Intimidating Presence action +
    // charge. Mindless Rage is a pure passive with no active surface
    // (unlike Frenzy which has the paired `FRENZY` bonus-action swing);
    // Intimidating Presence pairs an action-cost single-target Frighten
    // install with a short-rest charge, so both the action and the tag
    // ship together on this subclass template.
    let mut actions = BARBARIAN_TEMPLATE.actions.clone();
    actions.push(&*INTIMIDATING_PRESENCE);
    let mut features = BARBARIAN_TEMPLATE.features.clone();
    features.insert(MINDLESS_RAGE_TAG);
    features.insert(INTIMIDATING_PRESENCE_TAG);
    CreatureTemplate {
        name: "Berserker Barbarian",
        glyph: 'Z',
        actions,
        features,
        ..BARBARIAN_TEMPLATE.clone()
    }
});

/// Zealot Barbarian — Path of the Zealot subclass build. Identical
/// envelope to the baseline `BARBARIAN_TEMPLATE` (greataxe + reckless
/// attack + rage + brutal critical, level-9 stat block) with one
/// subclass feature layered on: **Divine Fury** (Zealot subclass level
/// 3) — a passive once-per-turn on-hit rider that adds `1d6 + half
/// barbarian level` (min +1) radiant damage to the first weapon hit
/// while raging.
///
/// The "holy warrior barbarian" tell: where the baseline barbarian
/// hits with greataxe + rage's +2 melee bump, the zealot layers a
/// radiant burst on top of the opening swing that lets them chip
/// through fire-resistant fiends and slashing-resistant elementals
/// alike. Radiant damage is universally uncommon-to-vulnerable
/// coverage among the creature types the zealot is meant to hunt
/// (fiends, undead, oozes), so the rider hits the widest possible
/// resistance envelope for its slot.
///
/// Pairs naturally with:
///   - **Reckless Attack** — the swing that opens with advantage is
///     usually the one that lands, and Divine Fury fires on that
///     opener since it's rate-limited by first-hit-per-turn.
///   - **Brutal Critical (1d)** — a natural 20 on the reckless swing
///     doubles both the greataxe die AND the Divine Fury 1d6 via
///     `roll_rider`, stacking two damage-double events on the same
///     hit.
///
/// Distinct from the totem family (Bear damage envelope / Wolf ally
/// aura / Eagle bonus-action Dash / Tiger flat speed) and Berserker
/// (Frenzy bonus-action swing + Mindless Rage passive) so a Zealot-
/// vs-anything encounter renders unambiguously by name AND subclass
/// features don't stack RAW-illegally on a single PC build.
///
/// Glyph 'X' (for zealot's X-cross) so zealot-vs-berserker-vs-totem
/// renders unambiguously on the map next to baseline 'B' / Totem 'T'
/// / Wolf 'W' / Eagle 'A' / Tiger 'I' / Berserker 'Z'.
pub static ZEALOT_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Barbarian envelope
    // wholesale and layer on the DIVINE_FURY_TAG passive. Actions list
    // adds the Zealous Presence bonus-action ally-burst (subclass
    // lv10) — Divine Fury is a pure passive on-hit rider with no
    // active surface, unlike Frenzy which has the paired `FRENZY`
    // bonus-action swing.
    //
    // The Berserker's FRENZY_TAG stays on the inherited feature set
    // (baseline barbarians pick up Frenzy at level 3 in our engine),
    // but the FRENZY action still gates on FRENZY_TAG at swing time —
    // the zealot's frenzy stays engine-legal alongside Divine Fury
    // since both are passive-flag-driven. If a strict subclass
    // separation is needed later, the baseline can shed FRENZY_TAG
    // from its default feature set.
    let mut actions = BARBARIAN_TEMPLATE.actions.clone();
    actions.push(&*ZEALOUS_PRESENCE);
    let mut features = BARBARIAN_TEMPLATE.features.clone();
    features.insert(DIVINE_FURY_TAG);
    // 5e Zealot Barbarian **Zealous Presence** (subclass level 10):
    // once-per-long-rest bonus action; up to 10 allies within 60ft
    // gain Blessed for 10 rounds. Ships on the CR-4 (level-9)
    // baseline above its strict RAW lv10 gate for the same reason
    // Divine Fury (RAW lv3) and Iron Mind (RAW lv7) do on the same
    // chassis — class templates target a balanced playable level,
    // not lockstep PHB progression. The tag is a long-rest charge
    // (NOT in `SHORT_REST_FEATURES` — RAW gates on the long rest per
    // PHB text). Composes cleanly with the zealot's raging kit: the
    // pre-fight tap-in gives the whole ally cluster a passive attack
    // /save bump for the opening 10-round window, then the zealot
    // rages and takes over the melee threat.
    features.insert(ZEALOUS_PRESENCE_TAG);
    CreatureTemplate {
        name: "Zealot Barbarian",
        glyph: 'X',
        actions,
        features,
        // 5e Zealot Barbarian Iron Mind (subclass lv7 passive):
        // proficiency in Wisdom saving throws. Mechanically identical
        // to the Rogue's Slippery Mind (lv15) — both flip the same
        // is_save_proficient(WIS) gate — but exposed as a distinct
        // template flag so the two subclass features stay
        // independently swappable in tests and readable in template
        // diffs. Read via the shared
        // `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort next to Slippery
        // Mind. Ships on the CR-4 (level-9) baseline above its strict
        // RAW lv7 gate for the same reason Divine Fury (RAW lv3) does
        // on the same chassis — class templates target a balanced
        // playable level, not lockstep PHB progression. Composes
        // cleanly with the paladin's Aura of Protection (adjacent
        // CHA-bonus stacking on top of the proficiency floor) and
        // with Danger Sense (DEX-save advantage) — the raging zealot
        // now shrugs off Hold Person / Command / Dominate the way a
        // paladin does, closing the barbarian chassis's classic WIS-
        // lock vulnerability.
        has_iron_mind: true,
        ..BARBARIAN_TEMPLATE.clone()
    }
});
