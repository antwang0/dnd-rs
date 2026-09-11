use crate::actions::action_template::Action;
use crate::actions::class_features::{
    BARDIC_INSPIRATION, BARDIC_INSPIRATION_TAG, COUNTERCHARM, COUNTERCHARM_TAG, CUTTING_WORDS,
    CUTTING_WORDS_TAG, FONT_OF_INSPIRATION_TAG, PSYCHIC_BLADES_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    BLESS, CHARM_PERSON, COUNTERSPELL, CURE_WOUNDS, DISSONANT_WHISPERS, FAERIE_FIRE, FIREBALL,
    HEALING_WORD, HEROISM, HOLD_PERSON, MASS_HEALING_WORD, PROTECTION_FROM_EVIL_AND_GOOD,
    SUGGESTION, VICIOUS_MOCKERY,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Shared bard subclass builder — clones the baseline `BARD_TEMPLATE`
/// envelope wholesale and layers on the per-subclass swaps: (a) display
/// name + glyph, (b) the combat-style template flags (`has_extra_attack`,
/// `has_dueling_style`), (c) any extra actions on top of the inherited
/// baseline action list (Additional Magical Secrets picks or similar
/// per-subclass action layers), and (d) any subclass feature tags
/// inserted into the baseline features set — per-rest charges read via
/// `feature_available` and passives read via `has_passive_feature`.
///
/// Users:
///   - **College of Valor** (`VALOR_BARD_TEMPLATE`) — flips
///     `has_extra_attack: true` only. Combat-bard lane, no additional
///     actions, no subclass tag.
///   - **College of Swords** (`SWORDS_BARD_TEMPLATE`) — flips both
///     `has_extra_attack: true` AND `has_dueling_style: true`. Combat-
///     bard lane with a per-swing damage floor lift, no subclass tag.
///   - **College of Lore** (`LORE_BARD_TEMPLATE`) — flips neither
///     combat flag; adds Counterspell + Fireball via Additional Magical
///     Secrets. Caster-bard lane, no subclass tag.
///   - **College of Whispers** (`WHISPERS_BARD_TEMPLATE`) — flips
///     neither combat flag; layers `PSYCHIC_BLADES_TAG` (passive once-
///     per-turn +1d6 Psychic weapon-hit rider on the shared
///     `ONCE_PER_TURN_RIDER_TAGS` cohort). Skirmisher-bard lane.
///   - **College of Eloquence** (`ELOQUENCE_BARD_TEMPLATE`) — flips
///     neither combat flag; adds the Unsettling Words action and layers
///     two tags, its charge and the passive Unfailing Inspiration.
///     Debuff-bard lane.
///
/// Collapses the four near-identical `CreatureTemplate {..BARD_TEMPLATE.clone()}`
/// struct literals into a single call per `LazyLock`. Matches the way
/// `subclass_barbarian_template` collapses the Totem / Storm Herald
/// barbarian family on the barbarian chassis and `subclass_warlock_template`
/// collapses the tag-only Otherworldly Patron family on the warlock
/// chassis — same shape, different chassis. Adding a new bard subclass
/// with the same envelope (a future College of Glamour or Spirits
/// variant, etc.) lands as a one-line entry.
///
/// `subclass_tags` is a slice rather than the `Option<&str>` it started
/// as: the Eloquence bard carries two (a per-rest charge and a passive),
/// and a second optional parameter would have been the third way to say
/// the same thing. `&[]` reads as "no subclass tag" at least as clearly
/// as `None` did.
fn subclass_bard_template(
    name: &'static str,
    glyph: char,
    has_extra_attack: bool,
    has_dueling_style: bool,
    extra_actions: &[&'static (dyn Action + Send + Sync)],
    subclass_tags: &[&'static str],
) -> CreatureTemplate {
    // Extra actions layer on top of the baseline action list rather
    // than replacing it — every bard subclass ships a strict superset
    // of the baseline bard's kit (spell slots, Bardic Inspiration /
    // Cutting Words / Font of Inspiration, the CC-heavy spell lineup).
    // The `..BARD_TEMPLATE.clone()` tail below carries every other
    // field forward without an N-line field-by-field copy.
    let mut actions = BARD_TEMPLATE.actions.clone();
    for &a in extra_actions {
        actions.push(a);
    }
    // Subclass tags — inserted into a fresh clone of the baseline
    // features set so the baseline bard's Bardic Inspiration / Cutting
    // Words / Font of Inspiration tags still ride through. An empty
    // slice collapses to a no-op, which is what the three subclasses
    // that sit purely on flag flips and action picks pass.
    let mut features = BARD_TEMPLATE.features.clone();
    for &tag in subclass_tags {
        features.insert(tag);
    }
    CreatureTemplate {
        name,
        glyph,
        has_extra_attack,
        has_dueling_style,
        actions,
        features,
        ..BARD_TEMPLATE.clone()
    }
}

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
    // Starry Wisp — the bard's one radiant cantrip, and their only
    // at-will answer to an invisible attacker: a ranged spell attack
    // that leaves the target shedding dim light and unable to benefit
    // from the Invisible condition until the end of the bard's next
    // turn. Vicious Mockery is the other at-will pick and is a WIS save
    // with no damage type at all, so the two do not compete.
    actions.push(&*crate::actions::spells::STARRY_WISP);
    // Light — the evocation cantrip every one of these classes has on
    // its list, and the party's answer to an unlit board: touch an ally
    // (or yourself) and they carry 20 ft of bright light and 20 ft of
    // dim light with them for the rest of the fight. Declines to cast
    // on a board that is already bright, and declines to re-light
    // somebody who is already lit, so it costs nothing on the ambient
    // default and is there when the lights are out.
    actions.push(&*crate::actions::spells::LIGHT);
    // Dancing Lights — the cantrip that lights ground the party has
    // not walked onto yet. Free and remote at once, which neither the
    // Light cantrip (free, but has to be touched onto somebody) nor
    // Daylight (remote, but a level-3 slot) manages. Holds
    // concentration, so it competes with the real spells rather than
    // stacking on them, and declines to cast on a board that is
    // already bright. See `spells::DANCING_LIGHTS`.
    actions.push(&*crate::actions::spells::DANCING_LIGHTS);
    // Mislead — the level-5 illusion that is the two halves the engine
    // already priced, in one action: `Invisible` and the Trickery
    // Cleric's `Duplicity`, which until now no spell could reach. The
    // bard is the class it fits best: a full caster with no other
    // invisibility on its list at all. See `spells::MISLEAD`.
    actions.push(&*crate::actions::spells::MISLEAD);
    actions.push(&*BARDIC_INSPIRATION);
    // Countercharm — the bard's Action-cost performance that hands the
    // whole party advantage on saves against being charmed or
    // frightened. See `class_features::COUNTERCHARM`; it is the first
    // feature in the engine written against the condition-scoped
    // save-advantage cohort rather than rounded to an immunity.
    actions.push(&*COUNTERCHARM);
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
    // (their `Charmed` back-link points at the bard, blocking the engine's
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
    // lv3 **Summon Fey** (TCE) — the bard's one summon, and RAW's own
    // pick. Fits the chassis better than the number suggests: a Fey
    // Spirit's charm rider lands on the same axis as Vicious Mockery and
    // Hypnotic Pattern, so a bard's whole kit points one way, and the
    // spirit's speed 40 covers the ground a bard would rather not.
    // The two XGE / TCE spells on the bard's RAW list. Both are
    // enchantment-adjacent control, which is the lane the whole chassis
    // already points down: Enemies Abound is a single-target Confusion
    // one slot earlier than Confusion itself and off an Intelligence
    // save, and Intellect Fortress protects the mental saves a bard's
    // own concentration depends on keeping.
    actions.push(&*crate::actions::spells::INTELLECT_FORTRESS);
    actions.push(&*crate::actions::spells::ENEMIES_ABOUND);
    actions.push(&crate::actions::spells::SUMMON_FEY);
    // The ward family — lv3 Glyph of Warding and lv7 Symbol, both on
    // the bard list RAW. See `ZoneEffect::ward`.
    actions.push(&*crate::actions::spells::GLYPH_OF_WARDING);
    actions.push(&*crate::actions::spells::SYMBOL);
    // lv1 **Animal Friendship** — on the bard list RAW, and the cheapest
    // thing on any list here that removes a creature from a fight. The
    // bard is the chassis that already spends its first-level slots on
    // charms; this is the one that works on the wolves.
    actions.push(&*crate::actions::spells::ANIMAL_FRIENDSHIP);
    // lv9 **Prismatic Wall** — the bard's apex, and RAW gives it to
    // exactly two lists. Terrain, light and a twenty-foot glare in one
    // action, and no concentration, so the bard raises it and then goes
    // back to inspiring people. See `spells::PRISMATIC_WALL`.
    actions.push(&*crate::actions::spells::PRISMATIC_WALL);
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
            // 5e **Feather Fall** — on the bard list RAW, carried as a
            // tag because its trigger is a fall rather than a turn. See
            // `EncounterInstance::try_feather_fall`.
            crate::actions::class_features::FEATHER_FALL_TAG,
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
            // 5e Bard **Countercharm** (RAW lv6). Ships on the CR-2
            // (level-7) baseline for the same reason Font of
            // Inspiration does — the template targets a balanced
            // playable level rather than lockstep PHB progression, and
            // level 7 is past the gate anyway.
            COUNTERCHARM_TAG,
        ]),
        skills: HashSet::from([Skill::Acrobatics, Skill::Perception]),
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
    // Subclass-of pattern via the shared `subclass_bard_template` helper —
    // clones the baseline Bard envelope wholesale and flips
    // `has_extra_attack: true`. The clone tail inside the helper picks
    // up every other field — spell slots, stats, actions, Bardic
    // Inspiration / Cutting Words / Font of Inspiration features —
    // without an N-line field-by-field copy. Sibling helper users on
    // the "class-scoped subclass builder" lane: `subclass_barbarian_template`
    // (Totem / Storm Herald / Berserker / Zealot family) and
    // `subclass_warlock_template` (tag-only Otherworldly Patron family).
    subclass_bard_template("Valor Bard", 'V', true, false, &[], &[])
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
    // Subclass-of pattern via the shared `subclass_bard_template` helper —
    // clones the baseline Bard envelope wholesale and flips the two
    // subclass template flags. Same shape as `VALOR_BARD_TEMPLATE`, and
    // the same "flip two boolean flags on the clone" shape
    // `SWASHBUCKLER_ROGUE_TEMPLATE` uses for its Rakish Audacity +
    // Fancy Footwork pair.
    subclass_bard_template("Swords Bard", 'W', true, true, &[], &[])
});

/// College of Lore Bard — subclass build (PHB). Identical envelope to
/// the baseline `BARD_TEMPLATE` (level-7 build, CHA-primary full caster,
/// Bardic Inspiration / Cutting Words / Font of Inspiration, Compulsion
/// capstone slot) with **Additional Magical Secrets** (Lore subclass
/// lv6) layered on as two extra prepared spells picked from any class
/// list:
///
/// - **Counterspell** (wizard/sorcerer/warlock lv3 abjuration): reaction
///   to cancel an enemy spell mid-cast. Slots into the bard's existing
///   anti-caster lane next to Silence and Cutting Words — where Silence
///   shuts down a caster area-of-effect and Cutting Words spikes a
///   single-target attack roll, Counterspell hard-cancels the entire
///   spell before it resolves. The classic "party's caster answer" pick
///   for Additional Magical Secrets — Counterspell is on nearly every
///   Lore Bard build's shortlist since it plugs the bard chassis's only
///   real counter-caster gap.
///
/// - **Fireball** (wizard/sorcerer lv3 evocation): the workhorse AoE
///   damage cantrip-plus. The baseline bard's damage lineup tops out at
///   Dissonant Whispers (single-target 3d6 psychic) and Cloud of
///   Daggers (single-tile 4d4 slashing) — Fireball's 8d6 fire on a 20-ft
///   radius closes the "burst damage against clustered enemies" gap
///   that no bard-list spell fills. Second classic Lore pick alongside
///   Counterspell — the two together (single-target hard-answer +
///   multi-target burst) round out the bard's kit into a full-caster
///   damage-and-control shape.
///
/// The two picks bring the CR-2 chassis's spell loadout in line with
/// what a level-7 Lore Bard would actually have prepared — most Lore
/// Bard tables converge on Counterspell + Fireball at lv6 (RAW's
/// Additional Magical Secrets grants two spells picked from any class,
/// of any level ≤ half the bard's level rounded up, capped at spells
/// available in the bard's slot table). Both spells are level 3, which
/// the bard's `spell_slots_by_level` = `[4, 3, 3, 1]` covers cleanly
/// through the three lv3 slots.
///
/// Cutting Words is the Lore signature feature RAW-wise — every Lore
/// Bard ships it at lv3 — and it already lives on the baseline
/// `BARD_TEMPLATE` (the CR-2 chassis is generous on the CC-heavy
/// support kit; every bard subclass template picks up Cutting Words
/// through the `..BARD_TEMPLATE.clone()` tail), so the Lore build
/// doesn't need to explicitly re-add it here — the clone tail carries
/// it forward.
///
/// Distinct from `VALOR_BARD_TEMPLATE` (Extra Attack — combat lane) and
/// `SWORDS_BARD_TEMPLATE` (Extra Attack + Dueling — melee-swinger lane)
/// on the identity axis: three bard subclass lanes now cover distinct
/// bard archetypes — Valor / Swords converge on the "combat bard" tell
/// (twice-per-Action scimitar cadence, per-swing damage floor on
/// Swords), and Lore covers the "full-caster bard" tell (two extra
/// non-bard spells rounding the CC-heavy baseline kit into a full
/// damage/control caster). The three templates never legally co-occur
/// on a single PC build (RAW: one Bard College pick per bard).
///
/// Ships the CR-2 template above the strict RAW lv6 gate for the same
/// reason `VALOR_BARD_TEMPLATE` / `SWORDS_BARD_TEMPLATE` ship Extra
/// Attack (RAW lv6) and `NECROMANCY_WIZARD_TEMPLATE` ships Inured to
/// Undeath (RAW lv10) — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'L' — 'L' for "Lore" reads as the loremaster / archivist bard
/// identity on the map. Distinct from baseline bard 'B', Valor Bard
/// 'V', and Swords Bard 'W'. Collides with several NPC creature
/// templates (Lizard, Lich) but the team-color-and-team-id combo
/// disambiguates in a mixed encounter — same overlap policy as the
/// other PC subclass glyphs (Assassin 'A' shares with Ape, Scout Rogue
/// 'K' shares with Killer Whale, etc.).
///
/// RAW's Lore Bard picks up other features not shipped on this
/// template — **Bonus Proficiencies** (three skill proficiencies at
/// lv3 — ribbon-only surface, no combat lane) and **Peerless Skill**
/// (lv14: expend one BI die on your OWN ability check as a self-buff
/// — needs an ability-check surface, which the combat-focused engine
/// doesn't currently model), so we ship the Additional Magical Secrets
/// half alone — matching the way `NECROMANCY_WIZARD_TEMPLATE` ships
/// only Inured to Undeath and `SWORDS_BARD_TEMPLATE` ships only Extra
/// Attack + Dueling from the RAW subclass kit.
pub static LORE_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `subclass_bard_template` helper —
    // clones the baseline Bard envelope wholesale and layers the
    // Additional Magical Secrets picks (Counterspell + Fireball) onto
    // the inherited action list via `extra_actions`. Same shape as
    // `VALOR_BARD_TEMPLATE` / `SWORDS_BARD_TEMPLATE`, differing only in
    // that Lore layers actions on rather than combat template flags —
    // no Extra Attack, no Dueling — Lore is a caster subclass, not a
    // melee-lane subclass.
    subclass_bard_template("Lore Bard", 'L', false, false, &[&*COUNTERSPELL, &*FIREBALL], &[])
});

/// College of Whispers Bard — subclass build (XGtE). Identical envelope
/// to the baseline `BARD_TEMPLATE` (level-7 build, CHA-primary full
/// caster, Bardic Inspiration / Cutting Words / Font of Inspiration,
/// Compulsion capstone slot) with **Psychic Blades** (Whispers subclass
/// lv3) layered on as a passive once-per-turn weapon-hit rider:
///
/// - **Psychic Blades** (lv3): on any weapon hit, lay +1d6 Psychic
///   damage on the target. Read at the shared once-per-turn weapon-die
///   rider chokepoint in `engine::attack::resolve_attack_outcome` right
///   after the Dreadful Strikes block, keyed off the
///   `PSYCHIC_BLADES_TAG` entry on the shared `ONCE_PER_TURN_RIDER_TAGS`
///   ledger. RAW-strict Psychic Blades expends a Bardic Inspiration die
///   per activation and scales the die pool with bard level (2d6 lv3 →
///   3d6 lv5 → 5d6 lv10 → 8d6 lv15); we collapse both the BI-die cost
///   AND the level-scaled pool to a plain +1d6 on the shared once-per-
///   turn ledger, matching the same "no ammo cost, one-die-typed" corner
///   Dreadful Strikes sits at on the ranger chassis. Trades a small
///   approximation of the level-scaled RAW pool for slotting cleanly
///   into the existing rider chokepoint without a separate accounting
///   surface.
///
/// The tag layer brings the CR-2 chassis's per-swing damage in line with
/// what a level-7 Whispers Bard would land on the opening scimitar
/// stroke — the mid-die-size 1d6 (Colossus Slayer 1d8 > Psychic Blades
/// 1d6 > Dreadful Strikes 1d4) matches Psychic Blades' RAW-lv3 anchor
/// where the equivalent Fey Wanderer / Hunter riders also unlock. The
/// bard chassis picks up an initiation-style damage lift on the opening
/// swing per turn — closes the "opening-round damage gap" against the
/// combat-bard Valor / Swords lanes' second-swing / +2-Dueling lifts.
///
/// Pairs naturally with the bard's existing support kit: the Whispers
/// bard sits at scimitar range on the opening round, lays +1d6 Psychic
/// on the first hit while Bardic Inspiration primes an ally's next
/// roll — the RAW "psychological warfare bard" tell. Distinct from
/// `VALOR_BARD_TEMPLATE` (Extra Attack — combat lane), `SWORDS_BARD_TEMPLATE`
/// (Extra Attack + Dueling — melee-swinger lane), and `LORE_BARD_TEMPLATE`
/// (Counterspell + Fireball — caster lane) on the identity axis: four
/// bard subclass lanes now cover distinct archetypes.
///
/// Sibling on the "once-per-turn +XdN weapon-hit rider" cross-class
/// lane to `COLOSSUS_SLAYER_TAG` (Hunter Ranger lv3 — +1d8 weapon-typed
/// with a wounded-target gate), `DREADFUL_STRIKES_TAG` (Fey Wanderer
/// Ranger lv3 — +1d4 Psychic on any weapon hit), `FOE_SLAYER_TAG`
/// (Ranger lv20 capstone — flat +WIS-mod on any weapon hit),
/// `DIVINE_FURY_TAG` (Zealot Barbarian lv3 — +1d6 + level/2 Radiant
/// while raging), and `SNEAK_ATTACK_TAG` (Rogue once-per-turn +Nd6 with
/// the qualifying-attack gate). The six rider tags share the
/// `ONCE_PER_TURN_RIDER_TAGS` ledger on `ActorInstance` — each fires at
/// most once per turn on the shared per-actor gate.
///
/// Ships the CR-2 template at (or slightly above) its strict RAW lv3
/// gate for the same reason `VALOR_BARD_TEMPLATE` / `SWORDS_BARD_TEMPLATE`
/// ship Extra Attack (RAW lv6) and `NECROMANCY_WIZARD_TEMPLATE` ships
/// Inured to Undeath (RAW lv10) — class templates target a balanced
/// playable level, not lockstep PHB progression.
///
/// Glyph 'P' — 'P' for "Psychic" reads as the Psychic Blades signature
/// on the map. Distinct from baseline bard 'B', Valor Bard 'V', Swords
/// Bard 'W', and Lore Bard 'L'. Collides with a handful of NPC creature
/// templates (Paladin subclasses, Pit Fiends, Purple Worms, Pixies) but
/// the team-color-and-team-id combo disambiguates in a mixed encounter —
/// same overlap policy as the other PC subclass glyphs (Assassin 'A'
/// shares with Ape, Scout Rogue 'K' shares with Killer Whale, etc.).
///
/// RAW's Whispers Bard picks up other features not shipped on this
/// template — **Words of Terror** (lv3: 1-minute Frightened condition
/// via a WIS-save-once-per-long-rest CHA-based DC — needs a per-target
/// long-rest social-skill hook the combat-focused engine doesn't model),
/// **Mantle of Whispers** (lv6: reaction on a nearby death to steal
/// the corpse's identity — ribbon-only surface, no combat lane),
/// **Shadow Lore** (lv14: single-target save-or-blindly-follow-orders
/// enchantment burst — needs a per-target charm-with-strict-commands
/// hook the engine doesn't currently model). Only the lv3 Psychic
/// Blades passive has a mechanical surface on the CR-2 chassis that
/// plugs cleanly into the shared `ONCE_PER_TURN_RIDER_TAGS` ledger, so
/// we ship that half alone — matching the way `NECROMANCY_WIZARD_TEMPLATE`
/// ships only Inured to Undeath, `SWORDS_BARD_TEMPLATE` ships only
/// Extra Attack + Dueling, and `LORE_BARD_TEMPLATE` ships only
/// Additional Magical Secrets from their respective RAW subclass kits.
pub static WHISPERS_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `subclass_bard_template` helper
    // — clones the baseline Bard envelope wholesale and layers the
    // PSYCHIC_BLADES_TAG onto the inherited features set via the helper's
    // `subclass_tag` axis. First bard-chassis user of the helper's
    // `subclass_tag` field — every prior bard subclass (Valor / Swords /
    // Lore) passes `None` because those three sit purely on combat-flag
    // flips and action-layer picks. Same shape as the sibling
    // `with_subclass_tag` cross-class helper on the standalone lane, just
    // routed through the bard-chassis-scoped builder so Whispers picks
    // up every baseline bard field (spell slots, actions, features,
    // stats, saves) through the `..BARD_TEMPLATE.clone()` tail.
    subclass_bard_template("Whispers Bard", 'P', false, false, &[], &[PSYCHIC_BLADES_TAG])
});

/// College of Eloquence Bard — subclass build (XGtE). Identical
/// envelope to the baseline `BARD_TEMPLATE` with the subclass's two
/// mechanical halves layered on, and the only bard on the roster whose
/// subclass points at the enemy's saving throw rather than at the
/// bard's own weapon.
///
/// **Unsettling Words** (lv3) is the inverse of Bardic Inspiration and
/// is built out of exactly the same parts — the `Unsettled` condition
/// is a −4 on `CONDITION_SAVE_BONUSES` where `Inspired` is a +3, and
/// both spend themselves on the first save their holder rolls via
/// `CONSUMED_ON_SAVE`. What makes it worth more than that symmetry
/// suggests is what the rest of the bard's sheet does with it. Every
/// bard on the roster carries Hold Person, Suggestion, Compulsion,
/// Dissonant Whispers and Charm Person; all five are save-or-suck, and
/// this is the only template that can shave four points off the save
/// that decides one. A −4 against a CHA-anchored DC in the mid-teens is
/// roughly a fifth of the roll.
///
/// **Unfailing Inspiration** (lv6) is what happens to the die on the
/// way back. An ordinary bard's inspiration is one roll deep whether or
/// not it lands; this bard's die is spent, watched, and returned if it
/// failed to rescue the roll — so the same die can be paid out again on
/// the next save, and the next, until it actually decides something.
/// That is RAW, uncapped, and the reason the subclass is worth a slot
/// on a roster that already has four bards: the Valor and Swords bards
/// bought a second swing, the Lore bard bought Fireball, and this one
/// bought a resource that does not deplete on failure.
///
/// The pair also composes in the one direction subclass features
/// usually don't. Unsettling Words makes an enemy's save worse;
/// Unfailing Inspiration makes an ally's save keep trying. Both spend
/// through `CONSUMED_ON_SAVE`, which means a single round can see the
/// bard's die come back off a failed ally save and the enemy's die burn
/// off a save the bard's own spell forced — two rows of one table
/// firing in opposite directions.
///
/// Left out: **Silver Tongue** (lv3, a floor of 10 on Persuasion and
/// Deception checks) has no combat surface — the engine rolls no social
/// checks, so the feature would be a tag nothing reads. **Infectious
/// Inspiration** (lv14, a reaction that hands a second creature a free
/// die when the first one's die *succeeds*) needs a reactive ally-
/// target picker at the save site, which is a different lane from the
/// three the roster has today; it is the natural next addition here.
///
/// Glyph 'Q' — the one letter no bard, and nothing else on the roster,
/// has taken. Baseline bard is 'B', Valor 'V', Swords 'W', Lore 'L',
/// Whispers 'P'.
pub static ELOQUENCE_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        UNFAILING_INSPIRATION_TAG, UNSETTLING_WORDS, UNSETTLING_WORDS_TAG,
    };
    // First user of the helper's multi-tag axis, and the reason it is a
    // slice: the charge and the passive are two different lanes on the
    // same subclass — `UNSETTLING_WORDS_TAG` is spent and refreshed
    // through `SHORT_REST_FEATURES`, `UNFAILING_INSPIRATION_TAG` is
    // never spent at all and is read off this bard by whoever is
    // holding one of their dice.
    subclass_bard_template(
        "Eloquence Bard",
        'Q',
        false,
        false,
        &[&*UNSETTLING_WORDS],
        &[UNSETTLING_WORDS_TAG, UNFAILING_INSPIRATION_TAG],
    )
});

/// College of Glamour Bard — subclass build (XGtE). Six bards on the
/// roster and five of them spend the Bardic Inspiration pool the way
/// the base class does: one die, one ally, one roll, later. Valor and
/// Swords pick up a weapon and keep the pool intact. Lore and Eloquence
/// spend it backwards, onto an enemy's roll. Glamour is the one that
/// spends it sideways.
///
/// **Mantle of Inspiration** (lv3) turns one use of the pool into five
/// temporary hit points on each of up to CHA-modifier allies within
/// 60 ft — three bodies covered at once on this CHA-16 chassis, right
/// now, with no roll to wait for. That is the subclass in one button:
/// the pool is the same three charges it always was, and every use of
/// it is now a choice between one large effect later and several small
/// ones immediately. A bard whose front line is about to be swung at
/// wants the mantle; a bard whose fighter is about to swing wants the
/// die.
///
/// **Enthralling Performance** (lv3) is an Action, once per short rest:
/// every hostile within the burst saves against the bard's
/// Charisma-anchored DC or is Charmed for a minute. It rides the shared
/// `TurnBurst` chassis next to the Cleric's Turn family and the two
/// paladin fear bursts, and it is the only one of them that installs
/// Charmed on anything that can hear rather than Frightened on a
/// creature type — a charmed enemy cannot attack the bard at all, where
/// a frightened one merely swings at disadvantage.
///
/// The pair reads as one idea from two directions: the mantle makes the
/// bard's own side harder to remove from the fight, and the performance
/// removes the other side from it. Neither costs a spell slot, which is
/// what lets a Glamour bard open a fight with Enthralling Performance,
/// spend the bonus action on the mantle, and still be holding every
/// slot on the table.
///
/// Left out: **Mantle of Majesty** (lv6) casts Command for free and then
/// again as a bonus action every turn for a minute; the engine has no
/// lane for a repeating slot-free cast, and shipping only the first
/// cast would be a worse Command than the bard's own list already
/// carries. **Unbreakable Majesty** (lv14) is a passive Sanctuary that
/// each attacker saves against once and then is immune to for a day —
/// the per-attacker immunity ledger is the part the engine has nowhere
/// to keep, and without it the feature is simply Sanctuary that never
/// breaks.
///
/// Glyph 'G' — free on the bard family, where 'B', 'V', 'S', 'L', 'H'
/// and 'Q' are taken.
pub static GLAMOUR_BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        ENTHRALLING_PERFORMANCE, ENTHRALLING_PERFORMANCE_TAG, MANTLE_OF_INSPIRATION,
    };
    // Mantle of Inspiration carries no tag of its own — it draws on
    // `BARDIC_INSPIRATION_TAG`, which the baseline chassis already
    // stocks three deep, and that shared pool is the feature. Only the
    // performance needs a charge lane.
    subclass_bard_template(
        "Glamour Bard",
        'G',
        false,
        false,
        &[&*MANTLE_OF_INSPIRATION, &*ENTHRALLING_PERFORMANCE],
        &[ENTHRALLING_PERFORMANCE_TAG],
    )
});
