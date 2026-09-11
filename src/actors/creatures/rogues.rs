use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{
    ASSASSINATE_TAG, CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE, CUNNING_STRIKE_DAZE,
    CUNNING_STRIKE_KNOCK_OUT, CUNNING_STRIKE_OBSCURE, CUNNING_STRIKE_POISON, CUNNING_STRIKE_TRIP,
    CUNNING_STRIKE_WITHDRAW, MAGICAL_AMBUSH_TAG,
    STEADY_AIM, SUPERIOR_MOBILITY_TAG, VERSATILE_TRICKSTER, VERSATILE_TRICKSTER_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::two_weapon::OFF_HAND_DAGGER;
use crate::actions::spells::{
    COLOR_SPRAY, INVISIBILITY, MIND_SLIVER, MIRROR_IMAGE, RAY_OF_FROST, SLEEP,
    TASHAS_HIDEOUS_LAUGHTER,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
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
    actions.push(&CUNNING_DASH);
    actions.push(&CUNNING_DISENGAGE);
    actions.push(&CUNNING_HIDE);
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
    // 5e 2024 Rogue Devious Strikes (lv14): the other three, on the same
    // lane and at RAW's steeper prices. Each refuses to prime until the
    // rogue's sneak pool is bigger than what it costs — RAW will not
    // reduce that pool below one die — so Obscure comes online around
    // level 7 and Knock Out around level 13, on a chassis that starts at
    // level 1 and climbs by encounter. Carried from the start anyway:
    // the gate is the level, and a sheet that grows an entry mid-campaign
    // is a sheet with two sources of truth about what a rogue knows.
    actions.push(&*CUNNING_STRIKE_OBSCURE);
    actions.push(&*CUNNING_STRIKE_KNOCK_OUT);
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
        // SRD 5.2's **Alert** Origin feat, and the rogue is where it
        // reads truest: the class whose whole plan is to be somewhere
        // before anybody has decided where anybody is. The proficiency
        // bonus on the initiative roll is the first thing a rogue
        // spends a feat on at most tables, and in this engine going
        // early is worth more than the number suggests — the opening
        // round decides who is standing where when the first area spell
        // lands. See `crate::actions::feats::ALERT_TAG`.
        features: HashSet::from([crate::actions::feats::ALERT_TAG]),
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
        skills: HashSet::from([Skill::Acrobatics, Skill::Perception, Skill::Stealth]),
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

/// The baseline rogue's feature set plus `extra` — the shape every
/// rogue subclass in this file should build its own set with.
///
/// Exists because the obvious alternative is wrong in a way that is
/// invisible: `features: HashSet::from([SUBCLASS_TAG])` sitting beside
/// a `..ROGUE_TEMPLATE.clone()` tail *replaces* the baseline set rather
/// than adding to it, so a subclass written that way quietly opts out
/// of every feature the chassis has or ever gains. Three did.
fn rogue_features_with(extra: &[&'static str]) -> HashSet<&'static str> {
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.extend(extra.iter().copied());
    features
}

/// Assassin Rogue — subclass build. Identical envelope to the baseline
/// `ROGUE_TEMPLATE` (level-7 build, shortsword + cunning suite, evasion,
/// uncanny dodge) with one subclass feature layered on: **Assassinate**
/// (level 3) — advantage on every attack roll against any creature that
/// hasn't taken a turn in the combat yet.
///
/// The "alpha-strike" rogue: opens combat with a guaranteed-advantage
/// shortsword swing (the once-per-turn Sneak Attack rider keys off
/// advantage as one of its triggers, so the alpha hit lands the +Nd6
/// without needing a flanking ally). RAW's other half — "any hit you
/// score against a surprised creature is a critical hit" — lands too,
/// through `EncounterInstance::target_grants_auto_crit`, and unlike the
/// rest of that cohort it carries no five-foot clause: the assassin's
/// bolt crits an unaware sentry from across the room.
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
        // Extended rather than replaced. `features: HashSet::from([..])`
        // beside a `..ROGUE_TEMPLATE.clone()` tail overwrites the
        // baseline set outright, which reads as "add this feature" and
        // means "have only this feature" — so every feature the
        // baseline rogue gains from here on would silently miss this
        // subclass. Three of these did exactly that, and the Alert feat
        // landing on the chassis is what made it visible. The
        // clone-and-insert shape the Soulknife and the Thief already
        // use is the one that says what it means.
        features: rogue_features_with(&[ASSASSINATE_TAG]),
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
    //
    // The off-hand dagger is the one action-list difference. RAW's
    // Swashbuckler is the blade-in-each-hand duelist — the archetype's
    // whole shape is closing to contact alone and staying there, which
    // Rakish Audacity rewards and Fancy Footwork makes survivable —
    // and the bonus-action swing is what a duelist does with a bonus
    // action once Cunning Action has nothing to disengage from.
    //
    // Notably *without* the Two-Weapon Fighting style: rogues get no
    // fighting style in RAW, so the off-hand dagger rolls 1d4 and
    // nothing else. That is the plain-RAW half of two-weapon fighting,
    // and it is on the roster next to the Two-Weapon Ranger's styled
    // version so the difference between them is visible in play rather
    // than only in the rules.
    //
    // It is worth more to a rogue than the die suggests, for a reason
    // the estimate does not show: Sneak Attack is once per *turn*, not
    // once per attack, so a rogue whose main hand missed has a second
    // roll to land it on. The whole sneak pool rides whichever swing
    // connects first.
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&OFF_HAND_DAGGER);
    // SRD 5.2's **Speedy** feat. The Swashbuckler's whole shape is
    // closing to contact alone and staying mobile inside it — Fancy
    // Footwork buys the exit from one melee and ten feet of speed is
    // what pays for the entrance to the next. See
    // `crate::actions::feats::SPEEDY_TAG`.
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.insert(crate::actions::feats::SPEEDY_TAG);
    CreatureTemplate {
        name: "Swashbuckler Rogue",
        glyph: 'S',
        charisma: 14,
        actions,
        features,
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
/// swimming speeds: the climbing half folds into the walking `speed()`
/// accessor for want of 3D terrain, and the swimming half is a row on
/// `SWIM_SPEED_SOURCES`, which makes the Scout and the Ranger the only
/// two player builds that cross a pool for free and swing out of one
/// without disadvantage. Read at the shared
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
        features: rogue_features_with(&[SUPERIOR_MOBILITY_TAG]),
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
        features: rogue_features_with(&[MAGICAL_AMBUSH_TAG, VERSATILE_TRICKSTER_TAG]),
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Soulknife Rogue — Roguish Archetype **Soulknife** subclass build
/// (TCoE). The sixth rogue in the engine, and the first one that can
/// land a Sneak Attack from across the room.
///
/// Three subclass features ship:
///
///   - **Psychic Blades** (lv3) — a blade of psionic force manifested in
///     the hand and thrown up to 60 ft. 1d6 psychic, finesse, and it
///     carries the Sneak Attack rider like any other rogue weapon.
///   - **Second blade** (lv3) — immediately after the Attack action, a
///     bonus action manifests another for 1d4.
///   - **Homing Strikes** (Soul Blades, lv9) — once per rest, a blade
///     that missed gets a 1d8 added to the roll.
///
/// The range is the whole build. A rogue's damage is Sneak Attack, not
/// their weapon die, and every other rogue on the roster has to be
/// standing next to something to collect it — which is a bad place for a
/// d8-hit-die character in leather to be. The Soulknife collects it from
/// 60 ft away, so Uncanny Dodge and Evasion stop being the things that
/// keep them alive and start being insurance they rarely need.
///
/// The damage type does the rest. Psychic is the least-resisted type in
/// the bestiary — the undead shrug off poison and necrotic, constructs
/// shrug off most of the physical types, and almost nothing on the
/// roster resists a thought. A Soulknife's 4d6 sneak lands in full
/// against creatures that a shortsword would barely scratch.
///
/// What it gives up is real. Sneak Attack still needs advantage or an
/// ally in contact with the target, and a rogue standing 60 ft back has
/// neither by default — so the Soulknife spends the fight hunting for
/// the shot rather than taking it automatically, which the melee rogues
/// get for free by standing next to the fighter.
///
/// Homing Strikes is the answer to the other half of that problem. A
/// rogue who has waited for the one turn their Sneak Attack is live and
/// then rolls a 9 has wasted the whole setup; one charge per rest buys
/// that turn back. It is a per-rest "add a die to a swing that missed",
/// which is the attack-roll twin of the failed-save rerolls Indomitable
/// and Fanatical Focus already ride.
///
/// Left out: **Psi-Bolstered Knack** and **Psychic Whispers** (lv3) are
/// an ability-check boost and a telepathy grant, neither of which combat
/// asks for; **Psychic Teleportation** (lv13) needs a self-teleport
/// scaled by a die roll, which the fixed-distance teleport lane doesn't
/// express; **Rend Mind** (lv17) sits above this chassis's level.
///
/// Glyph 'M' — for Soul**M**ind, since 'S' is the Scout's. Distinct from
/// baseline Rogue 'R', Assassin 'A', Scout 'S', Swashbuckler 'K' and
/// Arcane Trickster 'T'.
pub static SOULKNIFE_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_attacks::{PSYCHIC_BLADE, PSYCHIC_BLADE_FLOURISH};
    use crate::actions::class_features::HOMING_STRIKES_TAG;
    // The shortsword stays on the sheet. RAW doesn't take it away, and
    // an enemy that closes to contact should not find the Soulknife
    // holding nothing — the blades work at any range, but the AI's
    // attack picker prefers the longer reach, so leaving the sword in
    // costs nothing and covers the case where a blade is the wrong tool.
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&*PSYCHIC_BLADE);
    actions.push(&*PSYCHIC_BLADE_FLOURISH);
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.insert(HOMING_STRIKES_TAG);
    CreatureTemplate {
        name: "Soulknife Rogue",
        glyph: 'M',
        actions,
        features,
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Thief Rogue — the PHB's original rogue subclass, and the one whose
/// whole identity is action economy rather than damage. Two features,
/// both of which spend turns rather than dice.
///
/// **Fast Hands** (lv3) lets the rogue's Cunning Action bonus action be
/// spent on handling an object. Every other rogue on the roster who
/// wants a potion pays their Action for it — which is the entire turn,
/// because a rogue's Action *is* their Sneak Attack. The Thief drinks
/// and stabs in the same six seconds. The template ships with a belt of
/// three potions so the feature is live from round one instead of
/// waiting on a loot drop; nothing else on the roster carries starting
/// gear, and nothing else on the roster has a feature that is inert
/// without it.
///
/// **Thief's Reflexes** (lv17) is a whole extra turn in round 1, taken
/// ten points down the initiative order. It is modelled as a real second
/// slot in the queue — see `EncounterInstance::grant_extra_turn_slot` —
/// so the Thief opens the fight with two Actions, two bonus actions, two
/// movement budgets and, decisively, two Sneak Attacks, while the table
/// has had one turn each.
///
/// The two compose into the roster's sharpest alpha strike, and it is
/// the *combination* that gets there rather than either half. Round one
/// is: shortsword and Sneak Attack, potion of speed off the bonus
/// action, then ten initiative points later a second Sneak Attack with
/// the Hasted extra Action behind it. No other template can spend a
/// consumable and still swing on the same turn, and none of them get to
/// do it twice before the enemy's second turn.
///
/// What the Thief gives up is everything the other subclasses put on the
/// dice. The Assassin has advantage on the same opening, the Soulknife
/// throws its damage 60 ft, the Swashbuckler collects Sneak Attack
/// without needing anyone's help. The Thief's answer to all three is
/// that it simply takes more turns than they do, and then runs out of
/// potions.
///
/// Left out: **Second-Story Work** (lv3, climbing costs no extra
/// movement, longer running jumps) and **Supreme Sneak** (lv9,
/// advantage on a Stealth check after moving at half speed) both key off
/// systems the engine doesn't have — vertical movement and contested
/// Stealth checks; Hide here installs a condition rather than rolling.
/// **Use Magic Device** (lv13) lifts class and attunement restrictions
/// on magic items, and the engine has never had any: every item on the
/// loot table is usable by every actor already.
///
/// Glyph 'F' — for the fast hands. 'T' is the Arcane Trickster's, 'S'
/// the Scout's, 'A' the Assassin's, 'K' the Swashbuckler's and 'M' the
/// Soulknife's.
pub static THIEF_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{FAST_HANDS_TAG, THIEFS_REFLEXES_TAG};
    use crate::actions::item_actions::{
        DRINK_HEALING_POTION, DRINK_POTION_OF_BLUR, DRINK_POTION_OF_INVISIBILITY,
    };
    use crate::items::item_template::{POTION_OF_BLUR, POTION_OF_HEALING, POTION_OF_INVISIBILITY};
    // Carrying an item is what puts it in the inventory; the matching
    // action still has to be on the sheet for the rogue (or the AI) to
    // reach for it, and each one's `custom_validate_input` re-checks the
    // inventory, so a spent potion drops off the list on its own.
    //
    // All three cost an Action for everybody else — deliberately. A
    // belt of things that were already bonus actions (greater healing,
    // speed, heroism) would have shown the Thief nothing their own
    // subclass feature bought them.
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&DRINK_HEALING_POTION);
    actions.push(&DRINK_POTION_OF_INVISIBILITY);
    actions.push(&DRINK_POTION_OF_BLUR);
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.insert(FAST_HANDS_TAG);
    features.insert(THIEFS_REFLEXES_TAG);
    CreatureTemplate {
        name: "Thief Rogue",
        glyph: 'F',
        actions,
        features,
        // One heal, one opener, one defence — the three things a rogue
        // wants a spare hand for, so whichever the fight calls for is
        // already on the belt. Invisibility is the pick that most
        // rewards the feature: it hands the rogue advantage, which is
        // one of Sneak Attack's two triggers, on the same turn they
        // still get to swing.
        items: vec![
            &POTION_OF_HEALING,
            &POTION_OF_INVISIBILITY,
            &POTION_OF_BLUR,
        ],
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Phantom Rogue — subclass build (TCE). Every other rogue on the roster
/// answers the same question — how do I get my Sneak Attack onto the
/// right creature? The Assassin gets advantage on the opener, the
/// Soulknife throws it 60 ft, the Swashbuckler collects it without help,
/// the Thief takes an extra turn to land it twice. The Phantom asks a
/// different question: once the dice have landed, who else does the
/// damage reach?
///
/// **Wails from the Grave** (lv3) is the answer. Immediately after the
/// sneak dice resolve, a second creature the rogue can see within 30 ft
/// *of the victim* takes half that damage again as necrotic. It is the
/// engine's first secondary-damage rider anchored on the target rather
/// than on the attacker or on a point — Sweeping Attack splashes to
/// whoever is standing next to the victim, and everything else is either
/// a burst or a rider on the swing. Here the rogue's own position
/// decides nothing except whether they can see the second creature.
///
/// What that buys is a rogue whose damage does not fall off in a crowd.
/// A Sneak Attack is a single-target spike by construction; this one
/// spills roughly a third of the rogue's total round damage onto a
/// second body, chosen for the kill rather than for convenience —
/// `push_wails_from_the_grave` takes the lowest-HP eligible enemy,
/// because half a sneak pool is a rounding error against a healthy ogre
/// and a finishing blow against a bloodied kobold.
///
/// It also makes the Phantom the only rogue on the roster who cares
/// where the *enemies* are standing relative to each other. Every other
/// build's geometry problem is "can I reach the target"; this one's is
/// "is the target standing near something I want dead".
///
/// Left out: **Whispers of the Dead** (lv3) grants a skill proficiency
/// of the rogue's choice after each rest, and the engine rolls no skill
/// checks. **Tokens of the Departed** (lv9) harvests a soul trinket
/// from a creature that dies nearby and spends it for advantage on a
/// save, an extra Wails use, or a question put to a corpse — the first
/// needs a per-encounter inventory of tokens the engine has no lane
/// for, and the third has no combat surface. **Ghost Walk** (lv13,
/// incorporeal movement through creatures and objects) needs vertical
/// and pass-through movement, neither of which exists here. **Death's
/// Friend** (lv17) removes the Wails target restriction entirely.
///
/// Glyph 'H' — free on the rogue family, where 'R', 'A', 'S', 'K', 'T',
/// 'M' and 'F' are taken.
pub static PHANTOM_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::WAILS_FROM_THE_GRAVE_TAG;
    // Tag-only: the wail has no action of its own — it rides whatever
    // swing carried the Sneak Attack — so there is nothing to push onto
    // the action list and nothing for a controller to choose. Same
    // shape as `LONG_DEATH_MONK_TEMPLATE`'s Touch of Death.
    ROGUE_TEMPLATE.with_subclass_tag("Phantom Rogue", 'H', WAILS_FROM_THE_GRAVE_TAG)
});

/// Inquisitive Rogue — **Roguish Archetype: Inquisitive** (XGtE), and
/// the eighth rogue on a roster where every previous build answers the
/// same question from a different angle: how do I get my Sneak Attack
/// onto the right creature? The Assassin gets advantage on the opener,
/// the Soulknife throws it 60 ft, the Swashbuckler collects it without
/// help, the Scout survives being reached, the Thief takes an extra
/// turn, the Phantom spills it onto a second body.
///
/// This one stops asking. **Insightful Fighting** (lv3) is a bonus
/// action that reads a creature within 30 ft and marks it; from then on
/// the rogue sneak-attacks that creature whether or not anything else on
/// the board cooperates. It is the only path through
/// `class_attacks::sneak_attack_eligible` the rogue can manufacture —
/// the other three are things the board happens to be doing.
///
/// **Eye for Weakness** (lv17) is what makes the mark worth a bonus
/// action even against a target the rogue could already sneak-attack:
/// +3d6 into the sneak pool against the creature they read. On this
/// chassis that is a 3d6 pool becoming 6d6, which roughly doubles the
/// rogue's round.
///
/// The two together give the roster its first rogue with a *setup* turn.
/// Every other build here spends its bonus action on Cunning Action —
/// Dash to reach, Disengage to leave, Hide to re-arm the advantage —
/// and the Inquisitive spends one turn's worth of that to make the rest
/// of the fight unconditional. Which is a real trade on a d8 chassis
/// standing in reach: the turn the rogue reads its target is a turn it
/// cannot disengage out of contact.
///
/// Left out: **Ear for Deceit** and **Eye for Detail** (lv3) and
/// **Insightful Manipulator** (lv9) are all skill checks, and the engine
/// rolls none. **Steady Eye** (lv3) is advantage on Perception and
/// Investigation, same problem. **Unerring Eye** (lv9) senses illusions
/// and shapeshifters within 30 ft; the engine's nearest surface is
/// Truesight, and granting a rogue permanent Truesight to model a
/// once-per-rest hunch would be a much larger feature than the one RAW
/// wrote.
///
/// Glyph 'I' — for **I**nquisitive. Free on the rogue family, where
/// 'R', 'A', 'S', 'K', 'T', 'M', 'F' and 'H' are taken.
pub static INQUISITIVE_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{EYE_FOR_WEAKNESS_TAG, INSIGHTFUL_FIGHTING};
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&*INSIGHTFUL_FIGHTING);
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.insert(EYE_FOR_WEAKNESS_TAG);
    CreatureTemplate {
        name: "Inquisitive Rogue",
        glyph: 'I',
        actions,
        features,
        ..ROGUE_TEMPLATE.clone()
    }
});

/// Mastermind Rogue — **Roguish Archetype: Mastermind** (XGtE), and the
/// only rogue on the roster whose two shipped features both act on
/// somebody other than the rogue.
///
/// **Master of Tactics** (lv3) is the Help action as a bonus action at
/// 30 ft. Both halves matter and they matter together: the bonus action
/// is the one a Mastermind standing safely at range has least use for,
/// and the thirty feet is what lets the rogue hand the front line
/// advantage from wherever a d8 chassis wants to be standing. It is
/// also, on this roster, the only repeatable advantage-granting button
/// that costs nothing and never runs out — the bard's Inspiration is a
/// pool of three, Commander's Strike spends a superiority die, and Help
/// itself costs the helper their whole turn.
///
/// **Misdirection** (lv13) is the other side of the same coin, and it is
/// the least sentimental feature in the engine: while somebody is
/// standing between the rogue and a shooter, an attack that would land
/// on the rogue can be made to land on them instead. RAW does not ask
/// whether that somebody is a friend, and neither does this. See
/// `MISDIRECTION_TAG` and `engine::attack::ATTACK_REDIRECTS`.
///
/// Read together the archetype is a rogue who fights entirely through
/// other people's bodies — theirs to swing with, theirs to hide behind.
/// Which is a genuinely different lane from the seven builds beside it,
/// every one of which is a question about the rogue's own Sneak Attack.
/// The Mastermind's Sneak Attack is the baseline's, unimproved, and the
/// build is still worth playing because the fighter's is better.
///
/// Left out: **Soul of Deceit** (lv13's other half) protects the rogue's
/// thoughts from telepathy and magical lie-detection, neither of which
/// the engine models. **Insightful Manipulator** (lv9) and the lv3
/// tool / language proficiencies are ribbons.
///
/// Glyph 'D' — for the masterminD, since 'M' is the Soulknife's. Free on
/// the rogue family, where 'R', 'A', 'S', 'K', 'T', 'M', 'F', 'H' and
/// 'I' are taken.
pub static MASTERMIND_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::MISDIRECTION_TAG;
    use crate::actions::default_actions::MASTER_OF_TACTICS;
    let mut actions = ROGUE_TEMPLATE.actions.clone();
    actions.push(&*MASTER_OF_TACTICS);
    let mut features = ROGUE_TEMPLATE.features.clone();
    features.insert(MISDIRECTION_TAG);
    CreatureTemplate {
        name: "Mastermind Rogue",
        glyph: 'D',
        actions,
        features,
        ..ROGUE_TEMPLATE.clone()
    }
});
