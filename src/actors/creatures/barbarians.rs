use crate::actions::action_template::Action;
use crate::actions::class_features::{
    BEAR_TOTEM_TAG, DIVINE_FURY_TAG, EAGLE_DIVE, EAGLE_TOTEM_TAG, ELEMENTAL_CLEAVER,
    ELK_TOTEM_TAG, FAST_MOVEMENT_TAG, FRENZY, FRENZY_TAG, GIANT_STATURE_TAG,
    INTIMIDATING_PRESENCE, INTIMIDATING_PRESENCE_TAG, MINDLESS_RAGE_TAG, PANTHER_TOTEM_TAG, RAGE,
    RAGE_TAG, RELENTLESS_RAGE_TAG, TIGER_TOTEM_TAG, WOLF_TOTEM_TAG, WOLVERINE_TOTEM_TAG,
    ZEALOUS_PRESENCE, ZEALOUS_PRESENCE_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, HANDAXE, RECKLESS_ATTACK, THROWN_HANDAXE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Shared level-9 barbarian subclass build. Every barbarian subclass
/// template that pairs the standard Rage / Reckless Attack / Relentless
/// Rage / Fast Movement / Danger Sense / Feral Instinct / Persistent
/// Rage / Brutal Critical(1d) / Extra Attack envelope with **one
/// subclass feature tag** (and optionally one extra action) lands here.
/// The per-subclass swaps are (a) display name + glyph, (b) the
/// subclass feature tag in `features`, and (c) an optional extra
/// action (Eagle Dive on the Eagle Totem variant).
///
/// Users:
///   - **Path of the Totem Warrior / Wild Heart** (Bear / Wolf / Eagle
///     / Tiger / Elk / Wolverine / Panther) — the totem-spirit family
///     (RAW lv3 subclass tell for each), the original users of this
///     helper.
///   - **Path of the Storm Herald (Sea)** — Storm Soul (Sea) passive
///     lightning resistance (RAW lv6 tell); shares the same level-9
///     envelope with a distinct subclass tag on a distinct primal path.
///
/// The subclass-of pattern (single-tag layer on top of a shared
/// envelope) matches the way `MARID_WARLOCK_TEMPLATE` /
/// `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` /
/// `CELESTIAL_WARLOCK_TEMPLATE` / `NECROMANCY_WIZARD_TEMPLATE` layer
/// their single-tag subclass tells on top of their respective baseline
/// class envelopes — this helper is the barbarian-family analogue,
/// collapsing what would otherwise be N ~30-line struct literals into a
/// single call per `LazyLock`. Adding a new subclass with the same
/// envelope (Path of the Battlerager, another Storm Herald variant,
/// etc.) lands as a one-line entry.
fn subclass_barbarian_template(
    name: &'static str,
    glyph: char,
    subclass_tag: &'static str,
    extra_actions: &[&'static (dyn Action + Send + Sync)],
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    // The handaxe on the barbarian's belt, and the same handaxe in
    // flight. RAW's barbarian is proficient with both and canonically
    // carries a few; the engine's barbarian, until now, was the only
    // player family on the roster with no way at all to hurt something
    // it could not walk to — no bow, no cantrip, nothing thrown.
    //
    // Deliberately the worse option, and deliberately still there.
    // Rage's damage bonus is melee-only, the greataxe's d12 dwarfs the
    // handaxe's d6, and Reckless Attack buys advantage on Strength
    // *melee* swings — so every incentive the chassis has points at
    // closing to contact, which is what a barbarian should want. What
    // the throw changes is the round where closing is not on offer:
    // the flier overhead, the archer across the chasm, the caster
    // behind a wall of fire. Those rounds used to be spent walking.
    actions.push(&HANDAXE);
    actions.push(&THROWN_HANDAXE);
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
        // The two SRD 5.2 feats the chassis takes, and both read as
        // barbarian for the same reason: this is the class whose plan
        // is to put its hands on something and hit it very hard.
        //
        //   - **Savage Attacker** (Origin) — once a turn, roll the
        //     weapon's damage dice twice and keep the better. Worth
        //     most to whoever swings the biggest die, which on this
        //     roster is the greataxe.
        //   - **Grappler** (General) — advantage on attacks against a
        //     creature you are holding. Pairs with Rage's melee damage
        //     bonus and with `Grappled`'s own clause taxing the
        //     captive's swings at everyone but the grappler.
        //
        // See `crate::actions::feats`.
        features: HashSet::from([
            RAGE_TAG,
            subclass_tag,
            RELENTLESS_RAGE_TAG,
            FAST_MOVEMENT_TAG,
            crate::actions::feats::SAVAGE_ATTACKER_TAG,
            crate::actions::feats::GRAPPLER_TAG,
        ]),
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
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        // 5e (2024 / SRD 5.2) **Weapon Mastery** — the level-1 class
        // feature of all five martial classes, and the switch that
        // turns on the mastery property printed beside every weapon in
        // this template's kit. Inherited by every subclass template in
        // this file through its `..BASE.clone()` tail, which is why it
        // is set once on the chassis rather than at each subclass.
        has_weapon_mastery: true,
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
    // The belt handaxe and the same handaxe in flight — see
    // `barbarian_subclass` above for why a chassis whose every
    // incentive points at contact carries a throw anyway.
    actions.push(&HANDAXE);
    actions.push(&THROWN_HANDAXE);
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
        // The same two feats the shared subclass chassis takes — see
        // `subclass_barbarian_template`. Spelled out again because the
        // Berserker builds its own literal rather than going through
        // that helper.
        features: HashSet::from([
            RAGE_TAG,
            FRENZY_TAG,
            RELENTLESS_RAGE_TAG,
            FAST_MOVEMENT_TAG,
            crate::actions::feats::SAVAGE_ATTACKER_TAG,
            crate::actions::feats::GRAPPLER_TAG,
        ]),
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
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        // 5e (2024 / SRD 5.2) **Weapon Mastery** — the level-1 class
        // feature of all five martial classes, and the switch that
        // turns on the mastery property printed beside every weapon in
        // this template's kit. Inherited by every subclass template in
        // this file through its `..BASE.clone()` tail, which is why it
        // is set once on the chassis rather than at each subclass.
        has_weapon_mastery: true,
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
    subclass_barbarian_template("Totem Barbarian", 'T', BEAR_TOTEM_TAG, &[])
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
    subclass_barbarian_template("Wolf Totem Barbarian", 'W', WOLF_TOTEM_TAG, &[])
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
    subclass_barbarian_template(
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
    subclass_barbarian_template("Tiger Totem Barbarian", 'I', TIGER_TOTEM_TAG, &[])
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
    subclass_barbarian_template("Elk Totem Barbarian", 'E', ELK_TOTEM_TAG, &[])
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
    subclass_barbarian_template("Wolverine Totem Barbarian", 'V', WOLVERINE_TOTEM_TAG, &[])
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
    subclass_barbarian_template("Panther Totem Barbarian", 'P', PANTHER_TOTEM_TAG, &[])
});

/// Sea Storm Herald Barbarian — Path of the Storm Herald, **Sea** flavor
/// (level 6 subclass tell, XGtE). First non-totem user of the shared
/// `subclass_barbarian_template` helper — same level-9 envelope as the
/// totem-warrior family (76 HP, AC 15, STR / CON 18, Reckless Attack /
/// Brutal Critical 1d / Relentless Rage / Feral Instinct / Persistent
/// Rage / Fast Movement / Danger Sense) with the subclass feature
/// swapped from a totem-spirit tag to the sea storm herald's passive
/// lightning-resistance tell.
///
/// Headline mechanic: **Storm Soul (Sea)** — passive **resistance to
/// lightning damage**. Read at the shared `PASSIVE_TYPED_RESISTANCES`
/// cohort in `actor_template.rs` next to Heart of the Storm's
/// lightning + thunder row, the Warlock Elemental Gift rows (Marid
/// Cold / Dao Bludgeoning / Djinni Thunder), Radiant Soul's radiant
/// row, Fiendish / Draconic Resilience's fire rows, Psychic Defenses'
/// psychic row, and Inured to Undeath's necrotic row — same halving
/// rule, different subclass source. First **Barbarian**-chassis row on
/// that cohort (every prior row came off a racial trait or a Warlock /
/// Sorcerer / Wizard subclass).
///
/// Distinct from every other barbarian subclass template:
///   - Distinct from the **Totem Warrior / Wild Heart family** (Bear /
///     Wolf / Eagle / Tiger / Elk / Wolverine / Panther) in that the
///     resistance is **always-on**, not rage-gated — a Sea Storm
///     Herald outside of rage still halves the incoming Chain
///     Lightning / Lightning Bolt. Distinct from Bear Totem's rage-
///     gated broad resistance (on `RAGE_GATED_BROAD_RESISTANCES`) on
///     both axis (typed, not broad) and gate (always-on, not rage-
///     gated).
///   - Distinct from the **Berserker Barbarian** (Frenzy bonus-action
///     swing + Mindless Rage) and **Zealot Barbarian** (Divine Fury
///     radiant on-hit rider + Iron Mind WIS-save + Zealous Presence
///     ally-buff) on the entire subclass feature axis — Storm Herald
///     is a defensive resistance chassis rather than an offensive /
///     resource-focused chassis.
///
/// RAW's Path of the Storm Herald picks up other features not shipped
/// on this template — **Storm Aura (Sea)** (lv3: while raging, one
/// enemy within 10 ft eats a DEX-save 1d6-per-half-barbarian-level
/// lightning bolt at the start of each of the barbarian's turns; a
/// per-turn friend-agnostic aura mechanic that needs a per-turn aura
/// fire hook and a target-picking policy), **Shielding Storm** (lv10:
/// allies within 10 ft of the raging barbarian ALSO gain Storm Soul's
/// resistance; an ally-aura extension mechanic that needs a per-tile
/// ally sweep at the resistance-lookup chokepoint), and **Raging
/// Storm (Sea)** (lv14: reaction-on-attacker-hit STR-save vs. knock-
/// prone rider). Only the lv6 Storm Soul passive has a mechanical
/// surface that plugs cleanly into the shared passive typed-
/// resistance cohort, so we ship that half and leave the rest as
/// future work — matching the way `NECROMANCY_WIZARD_TEMPLATE` ships
/// only the lv10 Inured to Undeath passive half of its RAW School of
/// Necromancy kit, and `MARID_WARLOCK_TEMPLATE` /
/// `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` each ship only
/// the Elemental Gift resistance half of their RAW Genie patron kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `MARID_WARLOCK_TEMPLATE` /
/// `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
/// Gift (RAW lv6), `NECROMANCY_WIZARD_TEMPLATE` ships Inured to
/// Undeath (RAW lv10), and `ABERRANT_MIND_SORCERER_TEMPLATE` ships
/// Psychic Defenses (RAW lv14) — class templates target a balanced
/// playable level, not lockstep PHB progression.
///
/// Glyph 'H' (for storm Herald) — distinct from every other barbarian
/// subclass glyph (baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W',
/// Eagle 'A', Tiger 'I', Elk 'E', Wolverine 'V', Panther 'P',
/// Berserker 'Z', Zealot 'X'). 'H' for the "storm Herald" identity.
pub static SEA_STORM_HERALD_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Sea Storm Herald Barbarian",
        'H',
        crate::actions::class_features::STORM_SOUL_SEA_TAG,
        &[],
    )
});

/// Desert Storm Herald Barbarian — Path of the Storm Herald, **Desert**
/// flavor (level 6 subclass tell, XGtE). Second Storm Herald elemental
/// variant to land on this chassis (`SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
/// blazed the trail with the Sea flavor). Same level-9 envelope as every
/// other user of the `subclass_barbarian_template` helper (76 HP, AC 15,
/// STR / CON 18, Reckless Attack / Brutal Critical 1d / Relentless Rage
/// / Feral Instinct / Persistent Rage / Fast Movement / Danger Sense)
/// with the subclass feature swapped from the sea storm herald's
/// lightning-resistance tell to the desert storm herald's passive
/// fire-resistance tell.
///
/// Headline mechanic: **Storm Soul (Desert)** — passive **resistance to
/// fire damage**. Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort
/// in `actor_template.rs` next to the sibling Sea flavor's lightning
/// row, Fiendish / Draconic Resilience's fire rows, the Warlock Elemental
/// Gift rows (Marid Cold / Dao Bludgeoning / Djinni Thunder), Radiant
/// Soul's radiant row, Heart of the Storm's lightning + thunder row,
/// Psychic Defenses' psychic row, and Inured to Undeath's necrotic row
/// — same halving rule, different subclass source. Second **Barbarian**-
/// chassis row on that cohort — the Sea flavor's lightning row was the
/// first.
///
/// Distinct from every other barbarian subclass template:
///   - Distinct from the sibling **Sea Storm Herald** on the elemental-
///     flavor axis — Sea covers Lightning (a storm's electric-arc half),
///     Desert covers Fire (a desert's sun-scorched half); the two never
///     legally co-occur on a single build (RAW: one Storm Herald flavor
///     picked at lv3).
///   - Distinct from the **Totem Warrior / Wild Heart family** (Bear /
///     Wolf / Eagle / Tiger / Elk / Wolverine / Panther) in that the
///     resistance is **always-on**, not rage-gated — a Desert Storm
///     Herald outside of rage still halves the incoming Fireball /
///     Burning Hands / Fire Bolt. Distinct from Bear Totem's rage-gated
///     broad resistance (on `RAGE_GATED_BROAD_RESISTANCES`) on both axis
///     (typed, not broad) and gate (always-on, not rage-gated).
///   - Distinct from the **Berserker Barbarian** (Frenzy bonus-action
///     swing + Mindless Rage) and **Zealot Barbarian** (Divine Fury
///     radiant on-hit rider + Iron Mind WIS-save + Zealous Presence
///     ally-buff) on the entire subclass feature axis — Storm Herald
///     is a defensive resistance chassis rather than an offensive /
///     resource-focused chassis.
///
/// Overlaps the Fire axis with Fiendish Resilience (Warlock Fiend Patron
/// lv10) and Draconic Resilience (Sorcerer Draconic Bloodline lv6) on
/// different chassis — the three never legally co-occur on a single
/// build (Barbarian vs. Warlock vs. Sorcerer subclass), and a
/// hypothetical multiclass carrier caps at a single /2 per Fire hit via
/// the "one halving per damage instance" rule.
///
/// RAW's Path of the Storm Herald picks up other features not shipped
/// on this template — **Storm Aura (Desert)** (lv3: while raging, every
/// hostile within 10 ft eats a fixed 2 fire damage at the start of each
/// of the barbarian's turns, no save; a per-turn AoE aura mechanic that
/// needs a per-turn aura fire hook and a target-filtering policy),
/// **Shielding Storm** (lv10: allies within 10 ft of the raging
/// barbarian ALSO gain Storm Soul's resistance; an ally-aura extension
/// mechanic that needs a per-tile ally sweep at the resistance-lookup
/// chokepoint), and **Raging Storm (Desert)** (lv14: reaction-on-
/// attacker-melee-hit fire damage rider). Only the lv6 Storm Soul
/// passive has a mechanical surface that plugs cleanly into the shared
/// passive typed-resistance cohort, so we ship that half and leave the
/// rest as future work — matching the way `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
/// ships only the lv6 Storm Soul (Sea) passive half of its RAW Path of
/// the Storm Herald (Sea) kit, and `NECROMANCY_WIZARD_TEMPLATE` ships
/// only the lv10 Inured to Undeath passive half of its RAW School of
/// Necromancy kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
/// ships Storm Soul (Sea) (RAW lv6), `MARID_WARLOCK_TEMPLATE` /
/// `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
/// Gift (RAW lv6), `NECROMANCY_WIZARD_TEMPLATE` ships Inured to Undeath
/// (RAW lv10), and `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic
/// Defenses (RAW lv14) — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'R' (for desert / aRid, since Sea already claimed 'H' for
/// storm Herald) — distinct from every other barbarian subclass glyph
/// (baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W', Eagle 'A', Tiger
/// 'I', Elk 'E', Wolverine 'V', Panther 'P', Berserker 'Z', Zealot 'X',
/// Sea Storm Herald 'H').
pub static DESERT_STORM_HERALD_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Desert Storm Herald Barbarian",
        'R',
        crate::actions::class_features::STORM_SOUL_DESERT_TAG,
        &[],
    )
});

/// Tundra Storm Herald Barbarian — Path of the Storm Herald, **Tundra**
/// flavor (level 6 subclass tell, XGtE). Third Storm Herald elemental
/// variant to land on this chassis, completing the three-flavor RAW
/// Storm Herald elemental trio (Sea Lightning /
/// `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`, Desert Fire /
/// `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE`, Tundra Cold / this
/// template). Same level-9 envelope as every other user of the
/// `subclass_barbarian_template` helper (76 HP, AC 15, STR / CON 18,
/// Reckless Attack / Brutal Critical 1d / Relentless Rage / Feral
/// Instinct / Persistent Rage / Fast Movement / Danger Sense) with the
/// subclass feature swapped from the sibling Sea / Desert flavors'
/// lightning / fire tells to the tundra storm herald's passive
/// cold-resistance tell.
///
/// Headline mechanic: **Storm Soul (Tundra)** — passive **resistance to
/// cold damage**. Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort
/// in `actor_template.rs` next to the sibling Sea flavor's lightning
/// row, the Desert flavor's fire row, Fiendish / Draconic Resilience's
/// fire rows, the Warlock Elemental Gift rows (Marid Cold / Dao
/// Bludgeoning / Djinni Thunder), Radiant Soul's radiant row, Heart of
/// the Storm's lightning + thunder row, Psychic Defenses' psychic row,
/// and Inured to Undeath's necrotic row — same halving rule, different
/// subclass source. Third **Barbarian**-chassis row on that cohort —
/// the Sea flavor's lightning row was the first, the Desert flavor's
/// fire row was the second.
///
/// Distinct from every other barbarian subclass template:
///   - Distinct from the sibling **Sea Storm Herald** and **Desert
///     Storm Herald** on the elemental-flavor axis — Sea covers
///     Lightning (a storm's electric-arc half), Desert covers Fire (a
///     desert's sun-scorched half), Tundra covers Cold (a tundra's
///     numbing-chill half); the three never legally co-occur on a
///     single build (RAW: one Storm Herald flavor picked at lv3).
///   - Distinct from the **Totem Warrior / Wild Heart family** (Bear /
///     Wolf / Eagle / Tiger / Elk / Wolverine / Panther) in that the
///     resistance is **always-on**, not rage-gated — a Tundra Storm
///     Herald outside of rage still halves the incoming Cone of Cold /
///     Ray of Frost / Ice Storm. Distinct from Bear Totem's rage-gated
///     broad resistance (on `RAGE_GATED_BROAD_RESISTANCES`) on both
///     axis (typed, not broad) and gate (always-on, not rage-gated).
///   - Distinct from the **Berserker Barbarian** (Frenzy bonus-action
///     swing + Mindless Rage) and **Zealot Barbarian** (Divine Fury
///     radiant on-hit rider + Iron Mind WIS-save + Zealous Presence
///     ally-buff) on the entire subclass feature axis — Storm Herald
///     is a defensive resistance chassis rather than an offensive /
///     resource-focused chassis.
///
/// Overlaps the Cold axis with Marid's Elemental Gift (Warlock Genie
/// Marid Patron lv6) on a different chassis — the two never legally
/// co-occur on a single build (Barbarian vs. Warlock subclass), and a
/// hypothetical multiclass carrier caps at a single /2 per Cold hit via
/// the "one halving per damage instance" rule.
///
/// RAW's Path of the Storm Herald picks up other features not shipped
/// on this template — **Storm Aura (Tundra)** (lv3: while raging, every
/// friendly within 10 ft gains 2 temp HP at the start of each of the
/// barbarian's turns; a per-turn ally-buff aura mechanic that needs a
/// per-turn aura fire hook and a target-filtering policy), **Shielding
/// Storm** (lv10: allies within 10 ft of the raging barbarian ALSO gain
/// Storm Soul's resistance; an ally-aura extension mechanic that needs
/// a per-tile ally sweep at the resistance-lookup chokepoint), and
/// **Raging Storm (Tundra)** (lv14: reaction-on-attacker-melee-hit
/// STR-save vs. speed-0 rider). Only the lv6 Storm Soul passive has a
/// mechanical surface that plugs cleanly into the shared passive
/// typed-resistance cohort, so we ship that half and leave the rest as
/// future work — matching the way `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
/// / `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE` each ship only the lv6
/// Storm Soul passive half of their RAW Path of the Storm Herald kit,
/// and `NECROMANCY_WIZARD_TEMPLATE` ships only the lv10 Inured to
/// Undeath passive half of its RAW School of Necromancy kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`
/// / `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE` ship their Storm Soul
/// flavors (RAW lv6), `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE`
/// / `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6),
/// `NECROMANCY_WIZARD_TEMPLATE` ships Inured to Undeath (RAW lv10), and
/// `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic Defenses (RAW lv14)
/// — class templates target a balanced playable level, not lockstep
/// PHB progression.
///
/// Glyph 'N' (for tuNdra, since Sea already claimed 'H' for storm
/// Herald and Desert claimed 'R' for aRid; 'T' would collide with
/// Totem/Bear and 'C' would collide with Champion) — distinct from
/// every other barbarian subclass glyph (baseline Barbarian 'B',
/// Totem/Bear 'T', Wolf 'W', Eagle 'A', Tiger 'I', Elk 'E', Wolverine
/// 'V', Panther 'P', Berserker 'Z', Zealot 'X', Sea Storm Herald 'H',
/// Desert Storm Herald 'R').
pub static TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Tundra Storm Herald Barbarian",
        'N',
        crate::actions::class_features::STORM_SOUL_TUNDRA_TAG,
        &[],
    )
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

/// Ancestral Guardian Barbarian — Primal Path **Path of the Ancestral
/// Guardian** subclass build (XGtE), and the only barbarian on the
/// roster whose rage protects someone else.
///
/// One subclass feature, **Ancestral Protectors** (lv3), with two
/// clauses that only make sense together. While the barbarian is
/// raging, the first creature they hit each turn is marked, and until
/// the start of the barbarian's next turn that creature:
///
///   - has disadvantage on any attack roll that isn't against the
///     barbarian, and
///   - deals halved damage to anyone who isn't the barbarian.
///
/// Read separately, either clause is a mediocre debuff. Read together
/// they are a redirect: the marked creature can attack the barbarian at
/// full effect, or attack the wizard at disadvantage for half damage.
/// Nothing forces the choice — RAW never says the creature *must* target
/// the barbarian — but the arithmetic does, which is a more interesting
/// way to taunt than the Cavalier's Unwavering Mark or Compelled Duel,
/// both of which impose the disadvantage half and stop there.
///
/// The mark is exclusive and moves. `MarkCadence::FirstHitOfTurn` is
/// what encodes that: the once-per-turn ledger stops a barbarian with
/// Extra Attack from marking two creatures in a turn, and the
/// move-the-mark sweep stops last turn's target from keeping it. Both
/// halves are needed, and the second is the one that is easy to miss —
/// without it the guardian slowly accumulates a crowd of half-damage
/// enemies, which is a much stronger feature than RAW wrote.
///
/// RAW's later features aren't shipped. **Spirit Shield** (lv6 — a
/// reaction that reduces damage to an ally within 30 ft by 2d6) would
/// fit the reactive-damage-clamp cohort except that its scope is
/// ally-only-not-self and its reactor is a third party who has to be
/// raging; **Consult the Spirits** (lv10) is a divination with no
/// combat surface; **Vengeful Ancestors** (lv14) is Spirit Shield plus
/// a reflect and therefore waits on it.
///
/// Glyph 'G' — for **G**uardian. Distinct from baseline barbarian 'B',
/// Berserker 'Z' and Zealot 'X'; the totem and storm-herald builds all
/// inherit 'B' from the baseline.
pub static ANCESTRAL_GUARDIAN_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| {
        // Tag-only subclass: Ancestral Protectors adds no action, and
        // its whole surface is a row on `ON_HIT_CONDITION_MARKS` plus
        // the two clauses that read the mark's back-link. The shared
        // `with_subclass_tag` helper carries the rest of the chassis —
        // greataxe, Rage, Reckless Attack, Danger Sense, Fast Movement,
        // Brutal Critical, Relentless Rage — unchanged.
        BARBARIAN_TEMPLATE.with_subclass_tag(
            "Ancestral Guardian Barbarian",
            'G',
            crate::actions::class_features::ANCESTRAL_PROTECTORS_TAG,
        )
    });

/// Bite Beast Barbarian — **Path of the Beast** (TCE), Form of the
/// Beast: Bite.
///
/// The Path of the Beast is one subclass with three weapons, and RAW
/// re-picks between them on every rage. It ships here as three
/// templates for the same reason the seven totem spirits do: a template
/// is this engine's unit of "a build", and the seven totems are seven
/// templates rather than one Totem Warrior with a spirit field. See
/// `FORM_OF_THE_BEAST_BITE_TAG` for the full reasoning.
///
/// The Bite is the sustain form. 1d8 piercing, and once per turn a
/// landed bite on a barbarian below half their hit points heals them
/// for their proficiency bonus — the only self-heal anywhere on the
/// barbarian chassis. Every other barbarian on the roster survives by
/// not taking the damage (Rage halves it, Danger Sense dodges it,
/// Relentless Rage refuses the last of it); this one takes it and then
/// gets some of it back, and only once it is already losing. That gate
/// is what keeps it from being a flat damage-per-turn buff: an unhurt
/// bite barbarian heals nothing at all.
///
/// It pairs with Relentless Rage in a way none of the other forms do.
/// Relentless Rage is what keeps a barbarian at 1 HP standing; the bite
/// is what walks them back off it, and both are live in exactly the
/// same window.
///
/// RAW features not shipped, shared across all three forms:
/// **Bestial Soul** (lv6 — the natural weapons count as magical, plus a
/// swim / climb / jump utility rider), **Infectious Fury** (lv10 — a
/// WIS save on a natural-weapon hit, forcing the target to swing at its
/// own ally or take 2d12 psychic), and **Call the Hunt** (lv14 — a
/// party-wide damage rider bought with the barbarian's own hit points).
///
/// Glyph 'J' — for **J**aws. 'B' is the baseline barbarian and 'T' the
/// Totem chassis. Distinct from every other barbarian glyph (Wolf 'W',
/// Eagle 'A', Tiger 'I', Elk 'E', Wolverine 'V', Panther 'P', Sea Storm
/// 'H', Desert Storm 'R', Tundra Storm 'N', Berserker 'Z', Zealot 'X',
/// Ancestral Guardian 'G') and from its two siblings (Claws 'C', Tail
/// 'L').
pub static BITE_BEAST_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Bite Beast Barbarian",
        'J',
        crate::actions::class_features::FORM_OF_THE_BEAST_BITE_TAG,
        &[&*crate::actions::class_attacks::BEAST_BITE],
    )
});

/// Claw Beast Barbarian — **Path of the Beast** (TCE), Form of the
/// Beast: Claws.
///
/// 1d6 slashing, and RAW's "one additional attack with them as part of
/// the Attack action" — which stacks on top of Extra Attack rather than
/// replacing it, so the level-9 chassis swings three times a turn.
///
/// The die is smaller than the greataxe it replaces (1d6 against 1d12),
/// and that is the trade: the claws are worth taking precisely when
/// something multiplies per-swing value rather than per-die value.
/// Reckless Attack is the obvious one — advantage on three swings beats
/// advantage on two — but so is every flat per-hit rider the barbarian
/// can pick up, and so is a target whose AC is low enough that the
/// third swing lands as often as the first.
///
/// The extra swing is suppressed inside a Multiattack expansion, for
/// the same reason Extra Attack is: the wrapper already encodes the
/// per-Action swing count, and double-counting it would silently double
/// the turn's damage budget.
///
/// See `BITE_BEAST_BARBARIAN_TEMPLATE` for the shared RAW omissions and
/// for why the three forms ship as three templates.
///
/// Glyph 'C' — for **C**laws.
pub static CLAW_BEAST_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Claw Beast Barbarian",
        'C',
        crate::actions::class_features::FORM_OF_THE_BEAST_CLAWS_TAG,
        &[&*crate::actions::class_attacks::BEAST_CLAWS],
    )
});

/// Tail Beast Barbarian — **Path of the Beast** (TCE), Form of the
/// Beast: Tail.
///
/// 1d8 piercing at 10 ft, and the only reach weapon on the barbarian
/// chassis. Reach is the one thing the class has never had an answer
/// for: a barbarian is a melee body with a bonus-action Dash at best,
/// and a caster who steps back five feet costs them their whole turn.
/// The tail swings from a tile further out — which on this grid also
/// widens the ring the barbarian threatens for opportunity attacks, so
/// the caster who steps back eats one on the way.
///
/// Against the Bite (sustain) and the Claws (volume), the tail is the
/// positional form: it is the worst of the three in a straight
/// toe-to-toe exchange and the only one that changes which exchanges
/// happen at all.
///
/// RAW's other tail clause — a reaction adding 1d8 to AC against an
/// attack that would otherwise hit — isn't shipped; see
/// `FORM_OF_THE_BEAST_TAIL_TAG` for why the engine's reactive-AC lane
/// can't express it.
///
/// See `BITE_BEAST_BARBARIAN_TEMPLATE` for the shared RAW omissions and
/// for why the three forms ship as three templates.
///
/// Glyph 'L' — for tai**L**.
pub static TAIL_BEAST_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Tail Beast Barbarian",
        'L',
        crate::actions::class_features::FORM_OF_THE_BEAST_TAIL_TAG,
        &[&*crate::actions::class_attacks::BEAST_TAIL],
    )
});

/// Giant Barbarian — Path of the Giant (Bigby's Presents: Glory of the
/// Giants), and the seventeenth barbarian on the roster. The one whose
/// headline feature is a *shape* rather than a number.
///
/// Two subclass features ship, both at level 3:
///
///   - **Giant's Havoc / Giant Stature** — while raging, the barbarian
///     is Large. A gated row on `RESIZING_CONDITIONS` and nothing else;
///     see `GIANT_STATURE_TAG`.
///   - **Elemental Cleaver** — a bonus action while raging that
///     kindles the greataxe, after which every hit carries an extra 1d6
///     cold. A row on `ON_HIT_RIDERS`; see `ELEMENTAL_CLEAVER`.
///
/// **Growing is the feature, and it is worth more than the die.** The
/// engine measures reach from footprints, so a Large barbarian threatens
/// a wider ring of opportunity attacks, blocks a wider piece of corridor
/// and can reach a caster standing one tile further back than a Medium
/// one could. No other barbarian on the roster changes the geometry of
/// the fight; the Bear Totem changes what the barbarian survives, the
/// Berserker changes how often it swings, and both of those are numbers
/// on a sheet. This one changes where the enemy is allowed to stand.
///
/// It also composes with the chassis in a way the other paths don't.
/// Reckless Attack buys advantage at the cost of being hit back, and a
/// barbarian who is Large is a barbarian more things can reach — so the
/// path pushes the same button the base class already pushes, twice.
/// Whether that is good is a matchup question, which is the most this
/// engine can ask of a subclass.
///
/// **The cleaver is the tax on the bonus action.** RAW hands it out with
/// the rage; here it costs a press, and the press competes with Frenzy's
/// extra swing on a chassis where the bonus action is the scarcest thing
/// the barbarian has. A giant-path barbarian spends round one raging,
/// round two kindling, and lands the first extra die on round two's
/// attack — which is a real opening cost against a fight that may be
/// three rounds long.
///
/// Glyph 'S' — for giant **S**tature, since 'G' belongs to the
/// Ancestral Guardian. Distinct from baseline Barbarian 'B', Totem 'T',
/// Wolf 'W', Eagle 'A', Tiger 'I', Elk 'E', Wolverine 'V', Panther 'P',
/// the Storm Heralds' 'H' / 'R' / 'N', Berserker 'Z', Zealot 'X',
/// Ancestral Guardian 'G' and the Beast forms' 'J' / 'C' / 'L'.
pub static GIANT_BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    subclass_barbarian_template(
        "Giant Barbarian",
        'S',
        GIANT_STATURE_TAG,
        &[&*ELEMENTAL_CLEAVER],
    )
});
