use crate::actions::class_features::{
    BARDIC_INSPIRATION, BARDIC_INSPIRATION_TAG, CUTTING_WORDS, CUTTING_WORDS_TAG,
    FONT_OF_INSPIRATION_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    BLESS, CHARM_PERSON, CURE_WOUNDS, DISSONANT_WHISPERS, FAERIE_FIRE, HEALING_WORD, HEROISM,
    HOLD_PERSON, MASS_HEALING_WORD, PROTECTION_FROM_EVIL_AND_GOOD, SUGGESTION, VICIOUS_MOCKERY,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bard PC template. CHA-primary full caster with a support-flavored
/// spell list (heals, debuffs, crowd-control). Headline mechanic:
/// **Bardic Inspiration** — bonus action that grants an ally the
/// Inspired condition (flat +3 to their next attack roll or save).
///
/// Loadout: Vicious Mockery / Cure Wounds / Healing Word as workhorse
/// cantrip + heals, Heroism / Bless / Charm Person / Faerie Fire /
/// Protection from Evil and Good for support, Hold Person / Suggestion
/// / Mass Healing Word for higher-leverage utility, **Compulsion** at
/// the lv4 capstone for crowd-control, plus a Scimitar for when the
/// slots run dry. PC flag flips on so the bard enters Dying at 0 HP
/// rather than dropping outright.
pub static BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*VICIOUS_MOCKERY);
    actions.push(&*BARDIC_INSPIRATION);
    actions.push(&*CURE_WOUNDS);
    actions.push(&HEALING_WORD);
    actions.push(&*HEROISM);
    actions.push(&*BLESS);
    actions.push(&*CHARM_PERSON);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SUGGESTION);
    actions.push(&*MASS_HEALING_WORD);
    actions.push(&*PROTECTION_FROM_EVIL_AND_GOOD);
    // Dissonant Whispers — lv1 enchantment (bard-only RAW). 3d6 psychic
    // WIS save-for-half + on-fail forced-move flee away from the caster
    // at the target's full walking speed (routed through `PushActor`).
    // Gives the bard a lv1 damage-with-control option to round out the
    // existing save-or-suck lineup (Charm Person / Faerie Fire / Sleep).
    actions.push(&*DISSONANT_WHISPERS);
    // Latest bard additions:
    //   - cantrip **Blade Ward**: self damage-resistance till next turn.
    //     A defensive cantrip alternative when the bard is out of slots
    //     and Vicious Mockery is the only offense.
    //   - lv1 **Earth Tremor**: self-centered DEX-save bludgeoning +
    //     prone. The bard gets a clean lv1 AoE option to pair with the
    //     Dissonant Whispers single-target lane.
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::EARTH_TREMOR);
    actions.push(&*crate::actions::spells::SILVERY_BARBS);
    actions.push(&*crate::actions::spells::CLOUD_OF_DAGGERS);
    actions.push(&*crate::actions::spells::HEALING_SPIRIT);
    // Compulsion — lv4 enchantment (bard-only RAW), concentration. Self-
    // centered 12-tile burst; each enemy in range makes a WIS save vs the
    // bard's CHA-based DC or is Charmed by the bard for the duration
    // (their `charmed_by` link points at the bard, blocking the engine's
    // existing hostile-action gate). The bard's flagship lv4 crowd-control
    // option — slots between Charm Person (lv1) / Hypnotic Pattern (lv3,
    // here on wizard / warlock only) / Charm Monster (lv4, single-target)
    // / Mass Suggestion (lv6) on the enchantment ladder.
    actions.push(&*crate::actions::spells::COMPULSION);
    // Cutting Words — Bard signature defensive feature, once per short
    // rest. Applies Mocked (disadvantage on next attack) to one enemy
    // within 60ft. Collapsing the RAW reactive cast into a bonus action
    // pre-empt loses some flavor but slots cleanly into the action
    // pipeline without a reaction-trigger framework.
    actions.push(&*CUTTING_WORDS);
    // lv1 **Longstrider** (transmutation): touch +10 ft speed for 1 hour,
    // no concentration. Bard's pre-combat ally mobility buff — pairs
    // cleanly with Bardic Inspiration's accuracy bump and the bard's
    // role as the party's tempo-setter.
    actions.push(&*crate::actions::spells::LONGSTRIDER);
    // lv2 **Enhance Ability** (transmutation): touch ally buff — 2d6 temp
    // HP + flat +2 saves for the duration (concentration). Slots cleanly
    // into the bard's support lane next to Bless / Heroism — the
    // single-target temp HP differentiates it from Bless's burst attack-
    // roll buff and Heroism's flat-mod temp HP.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    // lv2 **Pyrotechnics** (transmutation, XGtE): 2-radius CON-save fire
    // burst (1d8) + Blinded-on-fail. Cheap entry-tier elemental burst on
    // the bard's lv2 lane — complements the bard's existing crowd-control
    // toolkit (Hold Person, Suggestion) with a typed-damage option.
    actions.push(&*crate::actions::spells::PYROTECHNICS);
    // Latest bard utility additions: lv2 Silence (illusion zone that
    // shuts down rival casters — a bard's signature anti-magic option)
    // and lv4 Freedom of Movement (ally-buff restraint cleanse).
    actions.push(&*crate::actions::spells::SILENCE);
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    CreatureTemplate {
        name: "Bard",
        glyph: 'B',
        ac: 14,
        hitpoints: "7d8+7".parse().unwrap(),
        strength: 10,
        dexterity: 14,
        constitution: 12,
        intelligence: 12,
        wisdom: 12,
        charisma: 16, // primary spellcasting ability + Bardic Inspiration die
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Level-7 full-caster slots: 4/3/3/1. The lv4 slot fuels exactly
        // one Compulsion per encounter (the bard's flagship crowd-control
        // option), while the lv1-3 spread covers the CC + heal staples
        // (Healing Word / Bless / Charm Person / Hold Person / Suggestion
        // / Mass Healing Word).
        spell_slots_by_level: vec![4, 3, 3, 1],
        rolls_death_saves: true,
        // Bards are proficient in DEX and CHA saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Charisma,
        ]),
        features: HashSet::from([
            BARDIC_INSPIRATION_TAG,
            CUTTING_WORDS_TAG,
            // 5e Bard Font of Inspiration (level 5 passive): Bardic
            // Inspiration die refreshes on a short rest instead of a
            // long rest. Ships on the CR-2 (level-7) baseline template
            // above its strict RAW level gate for the same reason
            // Improved Divine Smite (lv11) ships on the CR-1.5 paladin
            // — class templates target a balanced playable level, not
            // lockstep PHB progression. Read at
            // `ActorInstance::short_rest`: the `SHORT_REST_FEATURES`
            // registry lists `BARDIC_INSPIRATION_TAG` unconditionally,
            // and the short-rest cascade only restores it when the
            // holder ALSO has this Font of Inspiration tag — so a
            // lv1-4 bard without the tag still has to long-rest to
            // reset the die. Composes cleanly with Cutting Words
            // (already once-per-short-rest) so both bard per-rest
            // charges refresh together.
            FONT_OF_INSPIRATION_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// College of Valor Bard — subclass build. Identical envelope to the
/// baseline `BARD_TEMPLATE` (level-7 build, CHA-primary full caster,
/// Bardic Inspiration / Cutting Words / Font of Inspiration, Compulsion
/// capstone slot) with one subclass feature layered on: **Extra Attack**
/// (Valor subclass level 6) — an Action-cost scimitar swing chains a
/// second scimitar swing on the same Action via the shared
/// `maybe_chain_extra_attack` helper. The `has_extra_attack: true` flag
/// on this template is the sole mechanical differentiator between the
/// baseline Bard and the Valor Bard — every other field (spell slots,
/// stats, actions, features) inherits through the `..BARD_TEMPLATE.clone()`
/// tail.
///
/// Pairs naturally with the bard's existing support kit: the Valor
/// bard sits in melee range with a scimitar between spell casts (twice
/// per Action) while Bardic Inspiration primes an ally's next roll —
/// the RAW "combat bard" tell. Distinct from the future College of Lore
/// bard (Peerless Skill + Additional Magical Secrets — the intelligence-
/// flavored support build). The Valor bard's extra swing composes cleanly
/// with the shared caster-side damage bumps in `MELEE_CASTER_BUMPS` (a
/// Two-Weapon Fighting valor bard picks up +STR mod on both scimitar
/// swings) and every rider that fires on hit.
///
/// Ships the CR-2 template above the strict RAW level-6 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10) and `DRACONIC_SORCERER_TEMPLATE` ships Draconic
/// Resilience (RAW lv6): class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'V' — distinct from baseline bard 'B'; 'V' for "Valor" and
/// aligns with the Vengeance Paladin glyph on the "combat-flavored
/// subclass sharing the letter 'V'" pattern (both templates share the
/// combat-heavy flavor; the color / team gates disambiguate them in
/// combat).
pub static VALOR_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Bard envelope wholesale
    // and flip `has_extra_attack: true`. The `..BARD_TEMPLATE.clone()`
    // tail picks up every other field — spell slots, stats, actions,
    // Bardic Inspiration / Cutting Words / Font of Inspiration
    // features — without an N-line field-by-field copy. Same shape as
    // `HUNTER_RANGER_TEMPLATE` / `ASSASSIN_ROGUE_TEMPLATE` /
    // `GREAT_OLD_ONE_WARLOCK_TEMPLATE`'s subclass build.
    CreatureTemplate {
        name: "Valor Bard",
        glyph: 'V',
        has_extra_attack: true,
        ..BARD_TEMPLATE.clone()
    }
});

/// College of Swords Bard — subclass build (XGtE). Identical envelope to
/// the baseline `BARD_TEMPLATE` (level-7 build, CHA-primary full caster,
/// Bardic Inspiration / Cutting Words / Font of Inspiration, Compulsion
/// capstone slot) with two subclass features layered on:
///
/// - **Fighting Style: Dueling** (Swords subclass lv3 pick): passive +2
///   to melee weapon damage rolls. RAW's "wielding a one-handed weapon
///   and no other weapon" gate collapses to "melee weapon attack" since
///   the engine doesn't track weapon-hand-usage — same gate collapse the
///   baseline Fighter's Dueling style uses on the scimitar chassis. Read
///   at the shared `MELEE_CASTER_BUMPS` chokepoint in `engine::attack`
///   via `has_dueling_style()`. The Swords bard picks up the +2 on every
///   scimitar swing (the baseline bard's one-handed melee weapon), so
///   the subclass's per-swing damage floor lifts one notch above the
///   Valor bard's un-styled scimitar.
///
/// - **Extra Attack** (Swords subclass lv6): an Action-cost scimitar
///   swing chains a second scimitar swing on the same Action via the
///   shared `maybe_chain_extra_attack` helper. Same flag flip as the
///   Valor bard's Extra Attack — both combat-bard subclasses converge
///   on the same "twice per Action" cadence at lv6.
///
/// Pairs naturally with the bard's existing support kit: the Swords bard
/// sits in melee range with a scimitar between spell casts (twice per
/// Action, each swing +2 damage) while Bardic Inspiration primes an
/// ally's next roll — the RAW "blade dancer" tell. Distinct from
/// `VALOR_BARD_TEMPLATE` (Extra Attack only — no Dueling style, no
/// per-swing damage floor lift) and from `BARD_TEMPLATE` (the baseline
/// support-flavored one-shot swinger). Three bard lanes now converge on
/// the same "combat bard" identity from different angles: Valor via a
/// second Action-cost swing, Swords via a second swing PLUS a per-swing
/// damage floor, baseline Bard via the CC-heavy support kit.
///
/// Ships the CR-2 template above the strict RAW level-6 gate for the
/// same reason `VALOR_BARD_TEMPLATE` ships Extra Attack (RAW lv6):
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Glyph 'W' — 'W' for "sWords" reads as a blade-flourishing duelist.
/// Distinct from baseline bard 'B' and Valor Bard 'V'. Collides with
/// several other creature templates on the map (Werewolf / Wight /
/// Warhorse / Wraith etc.) but the team-color-and-team-id combo
/// disambiguates in a mixed encounter — same overlap policy as the other
/// PC subclass glyphs (Assassin 'A' shares with Ape, Scout Rogue 'K'
/// shares with Killer Whale, etc.).
///
/// RAW's Swords Bard picks up other features not shipped on this
/// template — **Blade Flourishes** (Defensive / Slashing / Mobile
/// Flourish riders on the scimitar swing, spending a Bardic Inspiration
/// die per flourish), **Bonus Proficiencies** (medium armor + scimitar
/// as a martial weapon — ribbon-only surface here), and **Master's
/// Flourish** (lv14: flourish riders roll a d6 instead of the BI die).
/// Only the two RAW-passive halves (Dueling style + Extra Attack) have a
/// mechanical surface on the CR-2 chassis that plugs cleanly into the
/// existing flag lanes, so we ship those and leave the flourish riders
/// as future work — matching the way `NECROMANCY_WIZARD_TEMPLATE` ships
/// only Inured to Undeath and `SCOUT_ROGUE_TEMPLATE` ships only Superior
/// Mobility from the RAW subclass kit.
pub static SWORDS_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Bard envelope wholesale
    // and flip the two subclass template flags. The `..BARD_TEMPLATE.clone()`
    // tail picks up every other field — spell slots, stats, actions,
    // Bardic Inspiration / Cutting Words / Font of Inspiration features
    // — without an N-line field-by-field copy. Same shape as
    // `VALOR_BARD_TEMPLATE`, and the same "flip two boolean flags on the
    // clone" shape `SWASHBUCKLER_ROGUE_TEMPLATE` uses for its
    // Rakish Audacity + Fancy Footwork pair.
    CreatureTemplate {
        name: "Swords Bard",
        glyph: 'W',
        has_extra_attack: true,
        has_dueling_style: true,
        ..BARD_TEMPLATE.clone()
    }
});
