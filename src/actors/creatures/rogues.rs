use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{
    ASSASSINATE_TAG, CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE, CUNNING_STRIKE_DAZE,
    CUNNING_STRIKE_POISON, CUNNING_STRIKE_TRIP, CUNNING_STRIKE_WITHDRAW, MAGICAL_AMBUSH_TAG,
    STEADY_AIM, SUPERIOR_MOBILITY_TAG, VERSATILE_TRICKSTER, VERSATILE_TRICKSTER_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    COLOR_SPRAY, INVISIBILITY, MIND_SLIVER, MIRROR_IMAGE, RAY_OF_FROST, SLEEP,
    TASHAS_HIDEOUS_LAUGHTER,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rogue PC template. Light armor (AC 14: leather + DEX), modest HP,
/// DEX-primary. The headline mechanic is **Sneak Attack** — the
/// shortsword (finesse, DEX-based 1d6) deals an extra 1d6 once per turn
/// when the rogue has advantage OR an ally is adjacent to the target.
/// `rolls_death_saves: true` (PC) so it enters the dying state at 0 HP.
pub static ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ROGUE_SHORTSWORD);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    // 5e Tasha's Rogue Steady Aim (lv3): bonus action; advantage on next
    // attack at the cost of zeroing speed for the rest of the turn.
    // Pairs naturally with Sneak Attack's advantage trigger so a sniping
    // rogue can fire mid-encounter without needing an adjacent ally.
    actions.push(&*STEADY_AIM);
    // 5e 2024 Rogue Cunning Strike (lv5): bonus-action primes that trade
    // Sneak Attack dice for tactical effects on the next sneak hit.
    // Mutually exclusive (one prime at a time) — the shortsword's
    // `consume_cunning_strike` chokepoint picks the first active prime
    // and applies its effect.
    actions.push(&*CUNNING_STRIKE_POISON);
    actions.push(&*CUNNING_STRIKE_TRIP);
    actions.push(&*CUNNING_STRIKE_WITHDRAW);
    actions.push(&*CUNNING_STRIKE_DAZE);
    CreatureTemplate {
        name: "Rogue",
        glyph: 'R',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        strength: 10,
        dexterity: 16, // primary
        constitution: 12,
        intelligence: 12,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Rogues are proficient in DEX and INT saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        has_evasion: true,
        has_uncanny_dodge: true,
        // 5e Rogue Elusive (level 18 capstone): no attack roll has
        // advantage against the rogue while they aren't Incapacitated.
        // Ships on the CR-1 rogue template above its strict RAW level
        // gate for the same reason Improved Divine Smite ships on the
        // CR-1.5 paladin and Purity of Body ships on the CR-1.5 monk —
        // class templates target a balanced playable level, not
        // lockstep PHB progression. Composes cleanly with Evasion
        // (already on) and Uncanny Dodge (already on): the elusive
        // rogue drops Advantage on incoming hits, halves whichever hits
        // land (Uncanny Dodge, once per round), and takes no damage on
        // successful DEX saves (Evasion).
        has_elusive: true,
        // 5e Rogue Slippery Mind (level 15): proficient in Wisdom
        // saves. The narrower sibling to the monk's Diamond Soul (all
        // six saves) — Slippery Mind converts the rogue's WIS save
        // from a "bad save" into a "good save" so Hold Person / Dominate
        // Person / Compulsion / Fear no longer reliably lock the rogue
        // down. Ships on the CR-1 rogue template above its strict RAW
        // level gate alongside Elusive for the same reason (class
        // templates target a balanced playable level, not lockstep PHB
        // progression). Composes cleanly with the Paladin's Aura of
        // Protection — the CHA-bonus save layer stacks on top of the
        // slippery-mind proficiency floor.
        has_slippery_mind: true,
        // 5e Rogue **Blindsense** (level 14 class feature). Passive
        // concealment-piercer with a 10-ft footprint envelope: while
        // able to hear, the rogue is aware of any hidden or invisible
        // creature within 10 ft — so their attacks against unseen
        // targets (and unseen attackers' swings at them) resolve at
        // Normal instead of Disadvantage / Advantage. Ships on the
        // CR-1 baseline rogue template above its strict RAW level gate
        // alongside Elusive (lv18) / Slippery Mind (lv15) for the same
        // reason — class templates target a balanced playable level,
        // not lockstep PHB progression. Read at the
        // `EncounterInstance::pierces_illusion_of` chokepoint next to
        // Truesight / Feral Senses; the 10-ft envelope and the
        // Deafened gate both live in the helper. Composes cleanly
        // with the rogue's Assassin subclass identity — the alpha-
        // strike opening favors close-quarters engagement where
        // Blindsense's 10-ft envelope covers the crit / advantage
        // trigger window.
        has_blindsense: true,
        ..CreatureTemplate::defaults()
    }
});

/// Assassin Rogue — subclass build. Identical envelope to the baseline
/// `ROGUE_TEMPLATE` (level-7 build, shortsword + cunning suite, evasion,
/// uncanny dodge) with one subclass feature layered on: **Assassinate**
/// (level 3) — advantage on every attack roll against any creature that
/// hasn't taken a turn in the combat yet.
///
/// The "alpha-strike" rogue: opens combat with a guaranteed-advantage
/// shortsword swing (the once-per-turn Sneak Attack rider keys off
/// advantage as one of its triggers, so the alpha hit lands the +Nd6
/// without needing a flanking ally). RAW also lets a hit against a
/// surprised target be a critical, but we don't model the Surprised
/// state — the advantage half (the load-bearing piece) survives intact.
///
/// Distinct from `ROGUE_TEMPLATE` (Thief-equivalent baseline) so an
/// Assassin-vs-Thief or Assassin-vs-baseline encounter renders
/// unambiguously by name and the subclass feature doesn't accidentally
/// stack RAW-illegally on a single PC build. Glyph 'A' so the Assassin
/// shows up distinctly on the map next to the baseline 'R'.
pub static ASSASSIN_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Rogue envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // features). The `..base.clone()` tail picks up every other field
    // — actions, save profs, evasion, uncanny dodge, HP dice — without
    // an N-line field-by-field copy. Same shape as
    // `HUNTER_RANGER_TEMPLATE`.
    CreatureTemplate {
        name: "Assassin Rogue",
        glyph: 'A',
        features: HashSet::from([ASSASSINATE_TAG]),
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Swashbuckler Rogue — Roguish Archetype **Swashbuckler** subclass
/// build (XGtE). Identical envelope to the baseline `ROGUE_TEMPLATE`
/// (level-7 build, shortsword + cunning suite, evasion, uncanny dodge,
/// elusive, slippery mind, blindsense) with two subclass features
/// layered on:
///
/// - **Fancy Footwork** (level 3): passive template flag. RAW: "on
///   your turn, if you make a melee attack against a creature, that
///   creature can't make opportunity attacks against you for the rest
///   of your turn." Read at
///   `EncounterInstance::dispatch_opportunity_attacks` — every reactor
///   the swash has swung at this turn (tracked in
///   `melee_attack_targets_this_turn`) is silently skipped when the
///   swash moves out of their reach. Distinct from Disengage / Cunning
///   Disengage: those blanket-suppress every OA on the turn; Fancy
///   Footwork surgically suppresses only the swash's chosen melee
///   targets, freeing the bonus action for a Cunning Strike prime
///   instead. The engine also collapses the "on your turn" clause
///   because the ledger is cleared at the swash's own turn-start
///   reset — a swash who's not on their turn hasn't cleared / marked
///   anyone, so the suppression naturally vanishes.
///
/// - **Rakish Audacity** (level 3): passive two-part template flag.
///   RAW: "you gain the following benefits: (1) you can add your
///   Charisma modifier to your initiative rolls; (2) you don't need
///   advantage on the attack roll to use your Sneak Attack against a
///   creature if you are within 5 feet of it, no other creatures are
///   within 5 feet of you, and you don't have disadvantage on the
///   attack roll." Read at two chokepoints:
///     - `initiative_flat_bonus`: the CHA-mod bump joins the same
///       "flat number added to initiative total" lane as Remarkable
///       Athlete.
///     - `class_attacks::sneak_attack_eligible`: opens a third Sneak
///       Attack qualification path (the "solo duelist" path) — the
///       swash reliably lands Sneak Attack on their turn even without
///       a flanking ally.
///
/// Pairs naturally with the rogue's Steady Aim: the swash opens with
/// advantage (Steady Aim) → guaranteed Sneak Attack, then uses Fancy
/// Footwork to dance away without eating OAs. RAW's high-mobility
/// duelist tell — the swash reliably lands Sneak Attack on their turn
/// (Rakish Audacity's solo-duelist path fires when the swash stands
/// alone with the target, and Steady Aim is the fallback when they
/// don't) and then hops out of reach without penalty (Fancy Footwork
/// covers the retreat).
///
/// Distinct from `ROGUE_TEMPLATE` (Thief-equivalent baseline) and
/// `ASSASSIN_ROGUE_TEMPLATE` (Assassinate — advantage on first-turn
/// swings). The three lanes converge on the same "reliable Sneak
/// Attack" identity from different angles: Assassin via advantage,
/// Swashbuckler via the solo-duelist path, baseline Rogue via Steady
/// Aim's bonus-action advantage prime.
///
/// Ships the CR-1 template above the strict RAW level-3 gate for the
/// same reason `ASSASSIN_ROGUE_TEMPLATE` ships Assassinate on the
/// CR-1 chassis: class templates target a balanced playable level.
/// Glyph 'S' — distinct from baseline rogue 'R' and Assassin 'A'; 'S'
/// for "Swashbuckler" reads as a cavalier duelist.
pub static SWASHBUCKLER_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Rogue envelope wholesale
    // and flip the two subclass template flags. The `..base.clone()`
    // tail picks up every other field — actions, save profs, evasion,
    // uncanny dodge, elusive, slippery mind, blindsense — without an
    // N-line field-by-field copy. Same shape as
    // `ASSASSIN_ROGUE_TEMPLATE`.
    //
    // Bump CHA from the baseline rogue's 10 to 14 (+2 mod) so Rakish
    // Audacity's CHA-mod initiative bump has teeth. RAW's Swashbuckler
    // is a CHA-flavored duelist — the subclass features (Rakish
    // Audacity, Panache at lv9, Elegant Maneuver at lv13) all key off
    // Charisma, so the higher stat lands on the archetype's identity
    // rather than the shared-with-baseline STR 10 / CHA 10 stat block.
    // Matches the bard's CHA 16 primary spread on the "CHA-flavored PC
    // class template" lane — the rogue chassis's DEX 16 primary stays
    // intact for the shortsword's attack roll, so the bump is additive
    // rather than a stat swap.
    CreatureTemplate {
        name: "Swashbuckler Rogue",
        glyph: 'S',
        charisma: 14,
        has_rakish_audacity: true,
        has_fancy_footwork: true,
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Scout Rogue — Roguish Archetype **Scout** subclass build (XGtE).
/// Identical envelope to the baseline `ROGUE_TEMPLATE` (level-7 build,
/// shortsword + cunning suite, evasion, uncanny dodge, elusive,
/// slippery mind, blindsense) with one subclass feature layered on:
/// **Superior Mobility** (level 9, XGtE) — passive +10 ft walking-speed
/// bump on the scout chassis. RAW also grants matching climbing +
/// swimming speeds; both fold into the walking `speed()` accessor since
/// the engine has no 3D terrain to differentiate. Read at the shared
/// `passive_feature_speed_bonus` chokepoint via `SUPERIOR_MOBILITY_TAG`
/// — same lane as the barbarian's Fast Movement (+10), the monk's
/// Unarmored Movement (+10), and the ranger's Roving (+5).
///
/// The wilderness-tracker rogue: opens combat at 40 ft walking speed
/// (30 ft base + 10 ft Superior Mobility) — a full extra move step over
/// the baseline rogue on the opening round. Composes cleanly with the
/// rogue's Cunning Dash bonus-action Dash (60 ft one-turn move) and
/// with Steady Aim's bonus-action advantage prime (no move-cost drag on
/// the "sniper who moves 40 ft to a rooftop, then Steady Aims" opener).
///
/// Distinct from `ROGUE_TEMPLATE` (Thief-equivalent baseline),
/// `ASSASSIN_ROGUE_TEMPLATE` (Assassinate — advantage on first-turn
/// swings), and `SWASHBUCKLER_ROGUE_TEMPLATE` (Fancy Footwork + Rakish
/// Audacity — solo duelist). Four Roguish Archetype lanes converge on
/// the same "reliable Sneak Attack" identity from different angles:
/// Assassin via advantage, Swashbuckler via the solo-duelist path,
/// Scout via superior mobility (repositioning to advantageous angles),
/// baseline Rogue via Steady Aim's bonus-action advantage prime.
///
/// RAW's Scout Rogue picks up other features not shipped on this
/// template — **Skirmisher** (lv3: reactively move half your speed
/// when a hostile ends its turn within 5 feet; needs a per-turn
/// end-of-turn hostile-trigger hook), **Survivalist** (lv3: expertise
/// in Nature / Survival; skill-check surface, no combat lane),
/// **Ambush Master** (lv13: advantage on initiative + first-round
/// attack rider; needs an initiative-time closure), **Sudden Strike**
/// (lv17 capstone: bonus-action second Attack action + guaranteed
/// Sneak Attack rider on the follow-up; needs an Attack-action
/// duplication hook). Only the lv9 Superior Mobility passive has a
/// mechanical surface on the CR-1 chassis that plugs cleanly into the
/// shared `PASSIVE_FEATURE_SPEED_BONUSES` cohort, so we ship that half
/// and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only Inured to Undeath and
/// `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` each ship only the Elemental Gift
/// resistance half of their RAW Genie patron kit.
///
/// Ships the CR-1 template above the strict RAW level-9 gate for the
/// same reason `ASSASSIN_ROGUE_TEMPLATE` / `SWASHBUCKLER_ROGUE_TEMPLATE`
/// ship their lv3 subclass features on the CR-1 baseline chassis:
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Glyph 'K' — 'K' for "scoutinK" / "sneaK ahead" reads as a light-
/// armored wilderness scout on the map. Distinct from baseline rogue
/// 'R', Assassin 'A', and Swashbuckler 'S'. Collides with several NPC
/// creature templates (Death Knight, Killer Whale, Crocodile) but the
/// team-color-and-team-id combo disambiguates them in a mixed
/// encounter — same overlap policy as the other PC subclass glyphs
/// (e.g. Assassin 'A' shares with Ape / Awakened Shrub / etc.).
pub static SCOUT_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Rogue envelope wholesale
    // and layer on the one Scout Roguish Archetype subclass feature
    // (`SUPERIOR_MOBILITY_TAG`). The `..base.clone()` tail picks up
    // every other field — actions, save profs, evasion, uncanny dodge,
    // elusive, slippery mind, blindsense — without an N-line
    // field-by-field copy. Same shape as `ASSASSIN_ROGUE_TEMPLATE`.
    CreatureTemplate {
        name: "Scout Rogue",
        glyph: 'K',
        features: HashSet::from([SUPERIOR_MOBILITY_TAG]),
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Arcane Trickster Rogue — Roguish Archetype **Arcane Trickster**
/// subclass build (PHB). The second of the two casting third-casters in
/// the engine, alongside `ELDRITCH_KNIGHT_FIGHTER_TEMPLATE`, and the
/// one that casts to *set up* rather than to supplement.
///
/// Two subclass features ship:
///
///   - **Magical Ambush** (lv9) — cast while Hidden and the target's
///     save against the spell is at disadvantage.
///   - **Versatile Trickster** (lv13) — bonus action, one enemy within
///     30 ft: advantage on your next attack against them.
///
/// The two are the same feature pointed at the rogue's two win
/// conditions, and they compete for the same resource. The bonus action
/// is the rogue's scarcest slot — Cunning Action already wants it for
/// Hide, Dash and Disengage, and the Cunning Strike primes want it too.
/// Spending it on Hide arms Magical Ambush, which makes the *spell*
/// land; spending it on Versatile Trickster makes the *swing* land.
/// Neither is strictly better, and the rogue has to decide before
/// knowing which one the turn will need.
///
/// That is a genuinely different shape from the three Roguish
/// Archetypes already in the tree, which all converge on "reliable
/// Sneak Attack" from different angles — Assassin via first-turn
/// advantage, Swashbuckler via the solo-duelist path, Scout via
/// repositioning speed, and the baseline via Steady Aim's
/// speed-for-advantage trade. The Trickster is the only one whose
/// answer to "how do I get advantage" costs the same resource as its
/// answer to "how do I land control", so it is the only one where the
/// two halves of the turn are in tension.
///
/// **The spell list honors RAW's enchantment / illusion restriction**
/// (plus the two free picks), the same way the Eldritch Knight honors
/// its abjuration / evocation one — a flavor rule that is also a
/// balance rule. Ray of Frost is one free pick, and it is the load-
/// bearing one: without a cantrip that just deals damage, a Trickster
/// whose slots are spent has no ranged turn at all. Mind Sliver is the
/// other, and it is the setup cantrip — its own −1d4 rider on the
/// target's next save stacks with Magical Ambush's disadvantage, so a
/// Hidden Trickster who opens with Mind Sliver is casting the follow-up
/// lv1 into a save the target is rolling twice and subtracting from.
///
/// Level 1 is Sleep (the HP-threshold no-save lockdown — the one spell
/// on the list Magical Ambush does *nothing* for, which is the point:
/// it is the answer when the rogue isn't hidden), Color Spray (the same
/// shape in a cone) and Tasha's Hideous Laughter (the save-based
/// single-target lock that Magical Ambush most wants). Level 2 is
/// Invisibility and Mirror Image — the survival pair for a d8 chassis
/// that has to walk into melee to sneak-attack.
///
/// **Stats.** Third-caster slots `[4, 3]`, matching the Eldritch
/// Knight, which puts this at roughly rogue level 13 — where Versatile
/// Trickster comes online. INT rises from the baseline rogue's 12 to
/// 16, which is both the spell DC anchor (8 + 3 prof + 3 = 14) and the
/// stat every spell on the list rolls off; DEX stays 16 so the
/// shortsword and Sneak Attack are untouched. Everything else —
/// AC 14, the shortsword, the whole Cunning Action and Cunning Strike
/// suite, Evasion, Uncanny Dodge, Elusive, Slippery Mind, Blindsense —
/// inherits through the `..ROGUE_TEMPLATE.clone()` tail.
///
/// Mage Hand Legerdemain (lv3) and Spell Thief (lv17) are left out.
/// The first is a pure out-of-combat ribbon — RAW's combat surface for
/// it is exactly the Versatile Trickster clause, which ships. The
/// second needs a counterspell-shaped intercept that also *transfers*
/// the stolen spell onto the rogue's own list for a rest, which the
/// engine's action lists (compile-time `&'static` slices) can't
/// express without a per-actor override layer.
///
/// Glyph 'T' — for **T**rickster. Distinct from baseline rogue 'R',
/// Assassin 'A', Swashbuckler 'S' and Scout 'K'.
pub static ARCANE_TRICKSTER_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the tag-only clone the Assassin / Scout use: this subclass
    // adds seven spells, a bonus action, two tags, a slot table and an
    // INT bump. The `..ROGUE_TEMPLATE.clone()` tail still carries the
    // whole rogue chassis — shortsword, Cunning Action / Cunning Strike
    // suites, Steady Aim, Evasion, Uncanny Dodge, Elusive, Slippery
    // Mind, Blindsense — without an N-line field-by-field copy.
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&*MIND_SLIVER);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*SLEEP);
    actions.push(&*COLOR_SPRAY);
    actions.push(&*TASHAS_HIDEOUS_LAUGHTER);
    actions.push(&*INVISIBILITY);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*VERSATILE_TRICKSTER);
    CreatureTemplate {
        name: "Arcane Trickster Rogue",
        glyph: 'T',
        // INT 16 (+3) anchors every spell on the list — Mind Sliver,
        // Hideous Laughter and Ray of Frost all read Intelligence
        // directly — and puts the save DC at 14.
        intelligence: 16,
        spell_slots_by_level: vec![4, 3],
        actions,
        features: HashSet::from([MAGICAL_AMBUSH_TAG, VERSATILE_TRICKSTER_TAG]),
        ..ROGUE_TEMPLATE.clone()
    }
});
