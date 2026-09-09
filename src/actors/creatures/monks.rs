use crate::actions::class_features::{
    EMPTY_BODY, EMPTY_BODY_TAG, FANGS_OF_THE_FIRE_SNAKE, FANGS_OF_THE_FIRE_SNAKE_TAG,
    FLURRY_OF_BLOWS, KI_EMPOWERED_STRIKES_TAG, KI_POINTS_TAG, PATIENT_DEFENSE, PURITY_OF_BODY_TAG,
    SHADOW_ARTS_TAG, SHADOW_STEP, SHADOW_STEP_TAG, STEP_OF_THE_WIND, STILLNESS_OF_MIND,
    STUNNING_STRIKE, STUNNING_STRIKE_TAG, TOUCH_OF_DEATH_TAG, UNARMORED_MOVEMENT_TAG,
    WATER_WHIP, WHOLENESS_OF_BODY, WHOLENESS_OF_BODY_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MONK_UNARMED_STRIKE;
use crate::actions::spells::{
    BURNING_HANDS, CONE_OF_COLD, FIREBALL, FLY, GUST_OF_WIND, HOLD_PERSON, PASS_WITHOUT_TRACE,
    SHATTER, SILENCE, STONESKIN, THUNDERWAVE, WALL_OF_FIRE, WALL_OF_STONE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Monk PC template. Unarmored (Wisdom + Dex AC scaling — we collapse
/// the formula into a flat AC 15 for now), DEX-primary, with a high
/// WIS secondary that anchors the Stunning Strike DC. Headline
/// mechanics:
/// - **Martial Arts** (action): 1d8+DEX bludgeoning unarmed strike,
///   the staple attack.
/// - **Stunning Strike** (bonus action, 1/rest): primes the next melee
///   hit; on connect, target makes a CON save (8 + prof + WIS) or is
///   Stunned for 1 round.
/// - **Patient Defense** (bonus action, at-will): take the Dodge action
///   for free defensive disadvantage on incoming attacks.
/// - **Flurry of Blows** (bonus action, at-will): grants an extra Action
///   for a follow-up Martial Arts strike — doubles the per-turn swing
///   cap when the bonus action is otherwise idle.
///
/// Stats target a level-5 monk: 33 HP (5d8+5), AC 15 (unarmored
/// defense baseline), DEX 16 / WIS 14, no spells. PC flag flips on so
/// the monk enters Dying at 0 HP rather than dropping straight to dead.
pub static MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MONK_UNARMED_STRIKE);
    actions.push(&*STUNNING_STRIKE);
    actions.push(&*PATIENT_DEFENSE);
    actions.push(&*FLURRY_OF_BLOWS);
    actions.push(&*STILLNESS_OF_MIND);
    actions.push(&STEP_OF_THE_WIND);
    // Empty Body — RAW level 18 monk capstone-adjacent, once per long
    // rest. Ships on the CR-1.5 monk template above its strict RAW
    // level gate for the same reason Diamond Soul / Purity of Body do
    // (class templates target a balanced playable level, not lockstep
    // PHB progression). Action; installs Invisible + DamageResistant
    // on self for 10 rounds (1 minute RAW).
    actions.push(&*EMPTY_BODY);
    CreatureTemplate {
        name: "Monk",
        glyph: 'M',
        ac: 15, // Unarmored Defense baseline (10 + DEX + WIS at +3/+2 = 15).
        hitpoints: "5d8+5".parse().unwrap(),
        // 5e default humanoid walking speed. The RAW Unarmored Movement
        // +10 ft bump lives on the `UNARMORED_MOVEMENT_TAG` passive-
        // feature entry below rather than pre-baked into this field —
        // the `PASSIVE_FEATURE_SPEED_BONUSES` table adds it back so
        // `speed()` still reads 40 ft, but the feature stays declarative
        // (visible on template diffs, dial-able via
        // `grant_feature_for_test`) rather than a magic-number 40 with
        // a comment.
        speed: 30.,
        strength: 12,
        dexterity: 16, // primary attack stat
        constitution: 12,
        intelligence: 10,
        wisdom: 14, // Stunning Strike DC anchor
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 1.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Monks are proficient in STR and DEX saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
        ]),
        // Passive class features:
        //   - `STUNNING_STRIKE_TAG`: once-per-rest bonus-action prime →
        //     next melee hit lands a CON-save Stun rider.
        //   - `PURITY_OF_BODY_TAG` (level 10): passive Poisoned-condition
        //     AND poison-damage immunity. RAW "immune to disease and
        //     poison" — the disease half has no combat surface in our
        //     engine, but both poison halves fire (condition install
        //     bounces at `dynamic_immunity_to`; damage zeroes at
        //     `effective_damage`). Ships on the CR-1.5 monk template
        //     above its strict RAW level gate for the same reason
        //     Improved Divine Smite ships on the CR-1.5 paladin — class
        //     templates target a balanced playable level, not lockstep
        //     PHB progression.
        //   - `UNARMORED_MOVEMENT_TAG` (level 2): always-on +10 ft
        //     walking speed. Read at the shared
        //     `PASSIVE_FEATURE_SPEED_BONUSES` chokepoint next to Fast
        //     Movement (Barbarian lv5) — same +10 magnitude, same
        //     always-on cadence, different class chassis. The Monk's
        //     `speed` field baseline is 30 ft (the default humanoid
        //     walking speed) and the tag folds the +10 back through
        //     the shared table, so `speed()` still reads 40 ft.
        features: HashSet::from([
            STUNNING_STRIKE_TAG,
            PURITY_OF_BODY_TAG,
            EMPTY_BODY_TAG,
            UNARMORED_MOVEMENT_TAG,
            //   - `KI_POINTS_TAG`: the pool Stunning Strike and Empty
            //     Body above spend from, and the one every subclass
            //     inherits through `..MONK_TEMPLATE.clone()` — which is
            //     what puts the Sun Soul's Searing Sunburst and the
            //     Drunken Master's Drunkard's Luck on the same five
            //     presses rather than on private charges of their own.
            //     Five, for the level-5 chassis; see `KI_POINTS_TAG`.
            KI_POINTS_TAG,
            //   - `KI_EMPOWERED_STRIKES_TAG` (level 6): "your unarmed
            //     strikes count as magical for the purpose of
            //     overcoming resistance and immunity to nonmagical
            //     attacks." Ships on the CR-1.5 chassis above its
            //     strict RAW level gate for the same reason Purity of
            //     Body (lv10) and Diamond Soul (lv14) do below — class
            //     templates target a balanced playable level, not
            //     lockstep PHB progression. It is also the monk's only
            //     answer to the forty stat blocks that halve mundane
            //     steel: a monk fights with their hands, so no amount
            //     of loot could have given it to them.
            KI_EMPOWERED_STRIKES_TAG,
        ]),
        has_evasion: true,
        has_deflect_missiles: true,
        has_extra_attack: true,
        // 5e Monk Diamond Soul (level 14 passive): proficient in every
        // saving throw. Ships on the CR-1.5 monk template above its
        // strict RAW level gate for the same reason Purity of Body
        // (lv10) ships here — class templates target a balanced
        // playable level, not lockstep PHB progression. Read by
        // `is_save_proficient` — the monk now rolls prof + ability on
        // every save, layering on top of Evasion (0 damage on passed
        // DEX save) and the paladin's Aura of Protection (+CHA to
        // every save when adjacent).
        has_diamond_soul: true,
        skills: HashSet::from([Skill::Acrobatics, Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});

/// Open Hand Monk — Way of the Open Hand subclass build. Identical
/// envelope to the baseline `MONK_TEMPLATE` (unarmored AC 15, unarmed
/// strike, Stunning Strike + Patient Defense + Flurry of Blows +
/// Stillness of Mind + Step of the Wind, evasion + deflect missiles +
/// extra attack) with one subclass feature layered on: **Wholeness of
/// Body** (lv6 subclass action, once per long rest) — heal self for
/// `3 × level` HP.
///
/// Pairs naturally with the monk's evasion / patient-defense kit: the
/// open-hand monk plays the staying-power skirmisher — dodges incoming
/// damage with Patient Defense, then refills the HP bar with Wholeness
/// of Body once per fight without burning a teammate's slot. Distinct
/// from `MONK_TEMPLATE` (Way of the Mercy / Shadow / Long Death-equivalent
/// baseline) so an Open-Hand-vs-baseline encounter renders unambiguously
/// by name. Glyph 'O' so the Open Hand monk shows up distinctly on the
/// map next to the baseline 'M'.
pub static OPEN_HAND_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Monk envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // actions / features). The `..base.clone()` tail picks up every
    // other field — stats, save profs, evasion / deflect missiles /
    // extra-attack — without an N-line field-by-field copy. Same shape
    // as `HUNTER_RANGER_TEMPLATE` / `ASSASSIN_ROGUE_TEMPLATE` /
    // `VENGEANCE_PALADIN_TEMPLATE`.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*WHOLENESS_OF_BODY);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(WHOLENESS_OF_BODY_TAG);
    CreatureTemplate {
        name: "Open Hand Monk",
        glyph: 'O',
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Long Death Monk — Way of the Long Death subclass build (SCAG).
/// Identical envelope to the baseline `MONK_TEMPLATE` (unarmored AC
/// 15, unarmed strike, Stunning Strike + Patient Defense + Flurry of
/// Blows + Stillness of Mind + Step of the Wind + Empty Body, evasion
/// + deflect missiles + extra attack, Purity of Body + Diamond Soul +
///   Unarmored Movement passives) with one subclass passive layered on:
///   **Touch of Death** (lv3 subclass tell) — whenever the Long Death
///   monk's damage reduces a hostile creature to 0 HP, the monk gains
///   `max(1, 1 + CON mod + monk level)` temporary HP.
///
/// Pairs naturally with the monk's strike-and-move skirmisher kit:
/// the Long Death monk chains kills into a self-refilling temp HP
/// pool without spending an action, complementing the reactive Patient
/// Defense (bonus-action Dodge) and Wholeness of Body-style hard heal
/// on the Open Hand cousin. Where Open Hand fills the HP bar once per
/// fight with a chunky heal, Long Death fills the temp HP absorb
/// buffer every time an enemy drops — the two subclasses hit the
/// staying-power lane from different angles.
///
/// The signature "kill-triggered temp HP" tell is a chassis-level
/// cousin of Fiend Warlock's **Dark One's Blessing** (CHA mod + level
/// on the same trigger). Both share the `KILL_TRIGGERED_TEMP_HP_SOURCES`
/// cohort in `EncounterInstance::pay_kill_triggered_temp_hp` — one
/// shared iteration reads whichever tag the swinger holds and applies
/// the row's stat + level formula. The two never legally co-occur on
/// a single build (Warlock Fiend Patron vs. Monk Long Death Way are
/// distinct classes with distinct subclasses).
///
/// RAW's Way of the Long Death picks up other features not shipped on
/// this template — **Hour of Reaping** (lv6: 30ft self-centered WIS
/// save vs Frightened burst; needs a per-turn cost surface for the
/// action), **Mastery of Death** (lv11: on drop to 0 HP, spend 1 ki
/// point to stay at 1 HP; needs a Downed-outcome intercept keyed off
/// a ki-point pool this engine doesn't track), and **Touch of the
/// Long Death** (lv17: cost 5+ ki for a 10d10 necrotic single-target
/// touch attack). The lv3 Touch of Death passive is the load-bearing
/// tactical feature with a first-class engine surface today, so we
/// ship that half and leave the rest as future work — matching the
/// way `OPEN_HAND_MONK_TEMPLATE` ships only the Wholeness of Body
/// (lv6) half of its RAW Way of the Open Hand kit.
///
/// Ships on the CR-1.5 monk chassis at (or above) its strict RAW lv3
/// gate for the same reason `OPEN_HAND_MONK_TEMPLATE` ships Wholeness
/// of Body (RAW lv6) and every other subclass template runs above its
/// strict RAW gate — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Distinct from `MONK_TEMPLATE` (subclass-less baseline) and
/// `OPEN_HAND_MONK_TEMPLATE` (Wholeness of Body) so a Long-Death-vs-
/// baseline / vs-Open-Hand encounter renders unambiguously by name.
/// Glyph 'D' (for the Long **D**eath way) so the Long Death monk shows
/// up distinctly on the map next to baseline 'M' and Open Hand 'O'.
pub static LONG_DEATH_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Monk envelope wholesale
    // and layers on the Way of the Long Death lv3 feature tag
    // (`TOUCH_OF_DEATH_TAG`: kill-triggered temp HP grant read at the
    // `DealDamage` chokepoint via the shared
    // `KILL_TRIGGERED_TEMP_HP_SOURCES` cohort next to Fiend Warlock's
    // Dark One's Blessing). The `..base.clone()` tail inside the helper
    // picks up every other field — actions, stats, save profs,
    // evasion / deflect missiles / extra-attack, Purity of Body /
    // Diamond Soul / Unarmored Movement passives — without an N-line
    // field-by-field copy. No new actions are pushed — Touch of Death
    // is a purely passive kill-triggered temp HP grant, not a fresh
    // action surface, so the "tag-only" shape the helper wraps is a
    // natural fit (mirrors `FORGE_CLERIC_TEMPLATE` /
    // `TWILIGHT_CLERIC_TEMPLATE` / `NECROMANCY_WIZARD_TEMPLATE` /
    // `WAR_MAGIC_WIZARD_TEMPLATE` / `LIFE_CLERIC_TEMPLATE` /
    // `SHADOW_MAGIC_SORCERER_TEMPLATE` / `ABERRANT_MIND_SORCERER_TEMPLATE`
    // / `DIVINE_SOUL_SORCERER_TEMPLATE` on the same cross-class
    // helper). Glyph 'D' — for the Long **D**eath way; distinct from
    // baseline monk 'M' and Open Hand 'O'.
    MONK_TEMPLATE.with_subclass_tag("Long Death Monk", 'D', TOUCH_OF_DEATH_TAG)
});

/// Shadow Monk — Monastic Tradition **Way of Shadow** subclass build
/// (PHB), and with it every PHB Monastic Tradition that has a combat
/// surface has a build in the engine: Open Hand
/// (`OPEN_HAND_MONK_TEMPLATE`) and this one. Two subclass features
/// ship:
///
///   - **Shadow Arts** (lv3) — ki spent as spellcasting: Silence and
///     Pass without Trace.
///   - **Shadow Step** (lv6) — bonus action, teleport 60 ft, and
///     advantage on the first melee attack that follows.
///
/// Shadow Step is the whole subclass in one button, and what makes it
/// interesting is that it is an *arrival* tool rather than an escape
/// one. Misty Step — the engine's other short-range blink — costs a
/// level-2 slot for 30 ft and is what a caster uses to leave. Shadow
/// Step costs a bonus action for 60 ft and hands the monk advantage
/// when they get there. On this chassis that is not a small rider:
/// Stunning Strike is a bonus-action prime whose CON save only ever
/// happens if the swing lands, so the shadow monk's two halves are
/// "teleport into reach with advantage" and "stun what you find" — and
/// they compete for the same bonus action, which means the monk can
/// only ever do one of them per turn. Choosing the teleport is
/// choosing to land the swing; choosing the stun is betting they
/// already can.
///
/// Which is a different axis from the two siblings on the chassis, both
/// of which are staying-power builds: Open Hand refills the HP bar once
/// a fight with Wholeness of Body, Long Death refills the temp HP
/// buffer every time something dies. The Shadow Monk doesn't answer
/// "how do I survive the round" at all — it answers "how do I reach the
/// caster in the back line on round one", which no other monk build in
/// the engine does.
///
/// **The slot table is Shadow Arts' ki, and nothing else.** RAW's
/// Shadow Arts spends 2 ki per cast on Darkness, Darkvision, Pass
/// without Trace or Silence; all three surviving options are level-2
/// spells, so `[0, 2]` — zero level-1 slots, two level-2 — reads
/// exactly as "two Shadow Arts casts, and no other magic." Darkness is
/// on the list now that the lighting layer gives it something to act
/// on; it was dropped for years alongside Darkvision for exactly that
/// reason, and Darkvision stays dropped because the monk's own eyes
/// are not a thing the engine has a way to change mid-fight. Minor
/// Illusion has no combat surface. The one deviation worth naming is
/// the rest cadence: ki refreshes on a short rest and spell slots
/// refresh on a long one, so a shadow monk gets fewer Shadow Arts casts
/// across a multi-fight day than RAW allows.
///
/// Darkness in *this* kit is not the warlock's. The warlock pairs it
/// with Devil's Sight and shoots out of it; the monk has no such
/// invocation, so the sphere blinds the monk too. What the monk has
/// instead is Shadow Step, and the pair is the actual combo: drop a
/// sphere on the enemy line, blink to its far edge, and swing at
/// creatures who cannot see the attacker while the attacker cannot see
/// them either — which cancels to a normal roll for the monk and
/// disadvantage for everyone shooting back.
///
/// Of the two spells, Silence is the load-bearing one and it is a
/// genuinely different tool than anything else the monk carries: a
/// monk who teleports into a caster's face can drop a 20 ft hush on
/// the spot and lock the caster out of levelled spells entirely — the
/// only lockdown in the monk's kit that doesn't route through a save.
/// Pass without Trace's `Untracked` is the escape half.
///
/// Cloak of Shadows (lv13) is deliberately not shipped. It grants
/// invisibility that breaks on attack — and the baseline monk chassis
/// already carries Empty Body, which grants invisibility *and* damage
/// resistance for ten rounds and doesn't break on attack at all. A
/// Cloak of Shadows on this template would be a strictly worse button
/// sitting next to a strictly better one, which teaches the player
/// nothing and gives the AI a trap pick. Opportunist (lv17) is left out
/// for the usual structural reason: it needs an ally-hit reaction hook
/// the engine doesn't expose.
///
/// Glyph 'W' — for the **W**ay of Shadow. Distinct from baseline monk
/// 'M', Open Hand 'O' and Long Death 'D'.
pub static SHADOW_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner Long Death uses: this
    // subclass adds three actions, two tags and a slot table. The
    // `..MONK_TEMPLATE.clone()` tail still carries the whole monk
    // chassis — AC 15, the unarmed strike, Stunning Strike, Patient
    // Defense, Flurry of Blows, Stillness of Mind, Step of the Wind,
    // Empty Body, Evasion, Deflect Missiles, Extra Attack, and the
    // Purity of Body / Diamond Soul / Unarmored Movement passives.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*SHADOW_STEP);
    actions.push(&*SILENCE);
    actions.push(&*PASS_WITHOUT_TRACE);
    actions.push(&*crate::actions::spells::DARKNESS);
    // The Bonus Action half of the Epic Boon layered on below. Pushed
    // beside the subclass's own three casts rather than onto the shared
    // monk chassis, because it is the boon's action and the boon is this
    // template's: `MergeWithShadows` refuses to fire for anyone who does
    // not hold the tag, but an action nobody can use still clutters
    // every other monk's list.
    actions.push(&*crate::actions::feats::MERGE_WITH_SHADOWS);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(SHADOW_ARTS_TAG);
    features.insert(SHADOW_STEP_TAG);
    // SRD 5.2's **Boon of the Night Spirit**, and the Way of Shadow is
    // who it belongs to: a subclass that already spends its bonus
    // actions on the dark and its ki on being somewhere else. The boon
    // pays the dark back — resistance to nearly everything while the
    // light does not reach the monk, and a bonus action that makes them
    // simply not there. See
    // `crate::actions::feats::BOON_OF_THE_NIGHT_SPIRIT_TAG`.
    features.insert(crate::actions::feats::BOON_OF_THE_NIGHT_SPIRIT_TAG);
    CreatureTemplate {
        name: "Shadow Monk",
        glyph: 'W',
        // Shadow Arts' ki budget, expressed in the only casting
        // currency the engine has: no level-1 slots, two level-2s —
        // exactly two casts of Silence, Pass without Trace or
        // Darkness.
        spell_slots_by_level: vec![0, 2],
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Four Elements Monk — Monastic Tradition **Way of the Four Elements**
/// subclass build (PHB), and with it every PHB Monastic Tradition has a
/// build in the engine: Open Hand, Shadow, and this one.
///
/// The subclass that spends a resource no other monk needs. Where Open
/// Hand refills the HP bar, Long Death refills the temp HP buffer and
/// Shadow buys a 60 ft arrival, the Four Elements monk buys *numbers* —
/// and the whole build is the question of whether a d8 chassis is the
/// right place to put them.
///
/// Three surfaces ship:
///
///   - **Water Whip** (2 ki) — a bonus action: DEX save vs the monk's ki
///     DC, 3d10 bludgeoning save-for-half, and prone on a fail.
///   - **Fangs of the Fire Snake** (1 ki) — a bonus-action prime: +1d10
///     fire on the next melee hit, the largest die on the engine's
///     on-hit rider table.
///   - **The elemental disciplines proper** — RAW's "you can spend ki
///     to cast this spell", carried as a spell list.
///
/// **Ki is spelled as spell slots**, exactly as `SHADOW_MONK_TEMPLATE`
/// spells Shadow Arts' ki, and here the mapping is RAW's own rather
/// than an approximation: every discipline in the PHB is priced in ki
/// at (spell level + 1), so a slot table *is* a ki budget once you read
/// the level as the discipline's tier. `[2, 2, 1, 1, 1]` is a monk with
/// enough ki for a couple of openers and one apex press, which is the
/// shape of a real Four Elements turn: whip something prone, spend the
/// next turns swinging, and hold the level-5 slot for the moment a
/// Breath of Winter is worth more than three unarmed strikes.
///
/// **The spell list is the discipline list, translated.** Each entry is
/// RAW's named discipline and the spell it casts:
///
///   - lv1 — Sweeping Cinder Strike (*Burning Hands*), Fist of Four
///     Thunders (*Thunderwave*).
///   - lv2 — Rush of the Gale Spirits (*Gust of Wind*), Clench of the
///     North Wind (*Hold Person*), Gong of the Summit (*Shatter*).
///   - lv3 — Flames of the Phoenix (*Fireball*), Ride the Wind (*Fly*).
///   - lv4 — Eternal Mountain Defense (*Stoneskin*), River of Hungry
///     Flame (*Wall of Fire*).
///   - lv5 — Breath of Winter (*Cone of Cold*), Wave of Rolling Earth
///     (*Wall of Stone*).
///
/// Every save-based entry now anchors its DC on the best of the
/// caster's mental stats rather than a hardcoded Intelligence, which is
/// what makes this list playable on a WIS chassis at all — see the
/// `best_spell_save_dc` promotion on Burning Hands, Thunderwave,
/// Shatter, Fireball, Cone of Cold and Hold Person. A monk casting
/// Fireball off INT 10 would have been rolling a DC 10 save against
/// creatures the same monk's Stunning Strike hits at DC 14.
///
/// Shape the Flowing River, Water Whip's pull variant, Mist Stance
/// (*Gaseous Form*) and Elemental Attunement are left out — the first
/// and last have no combat surface, the pull needs a per-cast rider
/// choice the action surface can't express, and *Gaseous Form* isn't
/// in the engine.
///
/// **The honest cost.** This is the only monk template that can run out
/// of subclass. Open Hand, Long Death and Shadow all carry features
/// that refresh or never deplete; a Four Elements monk who spends its
/// slots is a baseline monk with a 1d10 rider, and the AI will spend
/// them. That is the RAW criticism of the subclass reproduced faithfully
/// rather than balanced away, and it is why Water Whip is priced in the
/// shared pool instead of a per-rest charge: the interesting decision is
/// *which* ki to spend, and a per-rest charge would have removed it.
///
/// Glyph 'E' — for the four **E**lements. Distinct from baseline monk
/// 'M', Open Hand 'O', Long Death 'D' and Shadow 'W'.
pub static FOUR_ELEMENTS_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner Long Death uses: this
    // subclass adds two actions, eleven spells, a tag and a slot table.
    // The `..MONK_TEMPLATE.clone()` tail still carries the whole monk
    // chassis — AC 15, the unarmed strike, Stunning Strike, Patient
    // Defense, Flurry of Blows, Stillness of Mind, Step of the Wind,
    // Empty Body, Evasion, Deflect Missiles, Extra Attack, and the
    // Purity of Body / Diamond Soul / Unarmored Movement passives.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*WATER_WHIP);
    actions.push(&*FANGS_OF_THE_FIRE_SNAKE);
    // lv1 disciplines.
    actions.push(&*BURNING_HANDS);
    actions.push(&*THUNDERWAVE);
    // lv2 disciplines.
    actions.push(&*GUST_OF_WIND);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SHATTER);
    // lv3 disciplines.
    actions.push(&*FIREBALL);
    actions.push(&*FLY);
    // lv4 disciplines.
    actions.push(&*STONESKIN);
    actions.push(&*WALL_OF_FIRE);
    // lv5 disciplines.
    actions.push(&*CONE_OF_COLD);
    actions.push(&*WALL_OF_STONE);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(FANGS_OF_THE_FIRE_SNAKE_TAG);
    CreatureTemplate {
        name: "Four Elements Monk",
        glyph: 'E',
        // The ki budget, expressed in the only casting currency the
        // engine has. RAW prices each discipline at (spell level + 1)
        // ki, so a slot table read by tier *is* a ki pool.
        spell_slots_by_level: vec![2, 2, 1, 1, 1],
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Kensei Monk — Monastic Tradition **Way of the Kensei** subclass
/// build (XGtE), and the first monk on the roster that wants to be
/// anywhere but in contact.
///
/// Two subclass features, plus the weapon that makes them mean
/// something:
///
///   - **Kensei's Shot** (lv3, bonus action, at will): until the end of
///     the turn, every ranged weapon attack the monk lands carries an
///     extra 1d4. Free — RAW costs the bonus action and no ki.
///
///   - **Deft Strike** (lv6): once on each of the monk's turns, a
///     connecting kensei-weapon hit deals an extra martial-arts die of
///     the weapon's own damage type. RAW's 1 ki is dropped for the same
///     reason Flurry of Blows' is: it is a rider on a swing the monk
///     was making anyway, so the once-per-turn cap is the limiting
///     resource and `KI_POINTS_TAG` deliberately leaves it alone.
///
///   - **A longbow.** RAW's Kensei Weapons clause is a proficiency
///     grant, which on its own has no surface here — but a monk with no
///     ranged weapon has nothing for either feature to ride, so the bow
///     is the feature.
///
/// The chassis is what makes this interesting rather than a strictly
/// worse Hunter Ranger. Every bonus action a monk already has —
/// Flurry of Blows, Patient Defense, Step of the Wind — is priced for
/// standing next to the thing you are hitting. Kensei's Shot is priced
/// the same and points the opposite way, so the Kensei spends the fight
/// choosing between a bonus action that rewards closing and one that
/// rewards holding, on a body with 45 ft of movement and Deflect
/// Missiles. No other monk build has that decision; the baseline monk
/// can only ever answer "close".
///
/// RAW features not shipped. **Agile Parry** (lv3 — +2 AC when the monk
/// makes an unarmed strike as part of the Attack action while holding a
/// kensei weapon) needs a within-turn "have you already swung
/// unarmed?" trigger the AC lane can't see, and shipping the +2 as a
/// flat passive would be a real over-grant on a chassis that already
/// runs unarmored-defense AC. **Sharpen the Blade** (lv11) and
/// **Unerring Accuracy** (lv17) need a per-weapon enhancement lane and
/// a miss-reroll lane respectively; neither exists.
///
/// Glyph 'K' — for **K**ensei. Distinct from baseline monk 'M', Open
/// Hand 'O', Shadow 'W' and Four Elements 'E'; Long Death inherits 'M'.
pub static KENSEI_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{DEFT_STRIKE_TAG, KENSEIS_SHOT};
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*KENSEIS_SHOT);
    actions.push(&crate::actions::monster_attacks::LONGBOW);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(DEFT_STRIKE_TAG);
    CreatureTemplate {
        name: "Kensei Monk",
        glyph: 'K',
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// **Sun Shield** — Way of the Sun Soul Monk (subclass level 17). RAW:
/// the monk is wreathed in light, and "whenever a creature within 5 feet
/// of you hits you with a melee attack, you can use your reaction to
/// deal radiant damage to the creature… equal to 5 + your Wisdom
/// modifier."
///
/// Rides the engine's natural-melee-reflect lane — the same one the
/// Salamander's Heated Body and the Black Pudding's Corrosive Form sit
/// on — which is a passive: it fires on every melee hit, with no
/// reaction spent and nothing to switch on.
///
/// Both divergences from RAW pull in the same direction and are
/// deliberate. The reaction cost goes because a monk's reaction is
/// already the most contested one on the roster (Deflect Missiles and
/// Slow Fall both want it), and a reflect that competes with Deflect
/// Missiles would fire about as often as never. The bonus-action
/// activation goes because the lane has no notion of an off state, and
/// an always-on shield is closer to the feature's intent than a shield
/// nobody remembers to turn on.
///
/// A flat 7 is 5 + the monk chassis's WIS 14, computed once here rather
/// than read at the hit site, because `MeleeReflect` is plain const data
/// with no access to its holder. Every level-scaled number on the class
/// templates is pinned the same way and for the same reason.
static SUN_SHIELD: crate::engine::attack::MeleeReflect = crate::engine::attack::MeleeReflect {
    damage: crate::engine::attack::ReflectDamage::Flat(7),
    damage_type: crate::engine::types::DamageType::Radiant,
    label: "sun shield",
};

/// Sun Soul Monk — Monastic Tradition **Way of the Sun Soul** (XGtE),
/// and the answer to a gap the roster has had since the monk arrived:
/// every monk here has been a melee creature with a d8 hit die and no
/// armour, which is a bad combination to be standing in contact for.
///
/// The Kensei got out of contact by picking up a longbow. The Sun Soul
/// does it without a weapon at all, and that difference is the subclass:
///
///   - **Radiant Sun Bolt** (lv3): a ranged attack made with the body —
///     DEX to hit, the martial-arts die, radiant, 30 ft. It costs an
///     Action like any other attack, so Extra Attack throws it twice and
///     Flurry of Blows throws a third. RAW spends 1 ki as a bonus action
///     for the extra bolts; the chassis already has a bonus-action
///     button that hands over an extra Action, so the ki clause lands on
///     the button that was already there rather than on a second one
///     beside it.
///
///   - **Searing Sunburst** (lv11): Fireball's geometry, a sixth of its
///     damage, and no damage at all on a successful save. Once per short
///     rest. It is the only ranged area damage any monk on the roster
///     has.
///
///   - **Sun Shield** (lv17): 7 radiant back at anything that lands a
///     melee hit — see the `SUN_SHIELD` declaration above for why it is
///     passive here rather than a reaction.
///
///   - **Searing Arc Strike** (lv6) is Burning Hands, cast for ki. It
///     ships as Burning Hands on a two-slot level-1 table, which is the
///     Four Elements Monk's model for the same problem: RAW prices its
///     disciplines in ki, and a slot table read by tier *is* a ki pool.
///     The divergence is the action cost — RAW makes the arc a bonus
///     action after the Attack action, and the spell is priced as a
///     spell.
///
/// The radiant typing is what the kit trades for its range. It is the
/// most polarised damage type in the bestiary: the undead and the
/// fiends, which shrug off most of what a monk can throw, take it in
/// full and several take it doubled — and the celestials on the roster
/// are outright immune. A Sun Soul in a crypt is the best monk here; a
/// Sun Soul against a deva is throwing nothing at all, and has to walk
/// back into contact and punch.
///
/// Glyph 'U' — for s**U**n, since 'S' is not free and 'M', 'O', 'W',
/// 'E' and 'K' are the other monks'.
pub static SUN_SOUL_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{SEARING_SUNBURST, SEARING_SUNBURST_TAG};
    use crate::actions::monster_attacks::RADIANT_SUN_BOLT;
    // The unarmed strike stays on the sheet. The bolt is the longer
    // reach and the AI's attack picker prefers it, but radiant is the
    // one damage type on the roster that some creatures are immune to —
    // and a monk who can only throw light has nothing at all to do
    // against a deva.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&RADIANT_SUN_BOLT);
    actions.push(&*SEARING_SUNBURST);
    actions.push(&*BURNING_HANDS);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(SEARING_SUNBURST_TAG);
    CreatureTemplate {
        name: "Sun Soul Monk",
        glyph: 'U',
        // Searing Arc Strike's ki, in the only casting currency the
        // engine has — the same trick the Four Elements Monk's table is.
        spell_slots_by_level: vec![2],
        actions,
        features,
        natural_melee_reflect: Some(SUN_SHIELD),
        ..MONK_TEMPLATE.clone()
    }
});

/// Way of Mercy Monk — subclass build (TCE). The roster's first monk
/// whose hands do something other than damage, and the only support
/// build on a chassis that has otherwise been six variations on
/// "unarmed strike, with a rider".
///
/// **Hand of Healing** (lv3, with lv6 **Physician's Touch** folded in)
/// is a bonus-action touch that mends `1d6 + WIS` and lifts one of
/// Paralyzed, Stunned, Blinded, Poisoned or Deafened. The heal is
/// small; the cleanse is not. Nothing else on the roster ends Paralyzed
/// or Stunned without a level-5 slot or a paladin's once-per-rest
/// Cleansing Touch, and this one does it every round, for free, while
/// the monk's Action is still available to swing with. A Mercy monk
/// standing beside a paralyzed fighter is handing back a whole turn per
/// round.
///
/// **Hand of Harm** (lv3, with the same lv6 feature's poison clause) is
/// the same gesture inverted: once per turn the monk's strike carries
/// +1d6 necrotic and leaves the target Poisoned. It rides
/// `ONCE_PER_TURN_WEAPON_DIE_RIDERS` next to Deft Strike and Psychic
/// Blades, and it is the first row on that cohort that installs a
/// condition — which is where most of its value is. Poisoned means
/// disadvantage on attacks *and* on saves, and the monk's own Stunning
/// Strike is a save the target now rolls at disadvantage.
///
/// That pairing is the subclass. Every other monk here spends its bonus
/// action to swing more (Flurry), to swing safer (Patient Defense), or
/// to reposition (Step of the Wind); this one can spend it on somebody
/// else's turn instead. And the two hands compose in one direction that
/// reads as design rather than accident: Hand of Harm poisons the
/// target, Hand of Healing cures poison, and a monk fighting another
/// Mercy monk is undoing exactly what the other one just did.
///
/// The template ships both halves at their lv3 gate and folds in the
/// lv6 Physician's Touch clauses, matching the way
/// `OPEN_HAND_MONK_TEMPLATE` ships Wholeness of Body (RAW lv6) and the
/// baseline chassis ships Empty Body (RAW lv18) — class templates
/// target a balanced playable level, not lockstep PHB progression.
///
/// Left out: **Implements of Mercy** (lv3) is a proficiency ribbon.
/// **Flurry of Healing and Harm** (lv11) lets the monk replace Flurry
/// strikes with either hand, which is the level the bonus-action cost
/// modelled here already reflects — shipping it again would double-
/// count. **Hand of Ultimate Mercy** (lv17) revives a creature dead for
/// under 24 hours; the engine removes the dead, so there is nothing to
/// touch.
///
/// Glyph 'Y' — free on the monk family, where 'M', 'O', 'D', 'W', 'E',
/// 'K' and 'U' are taken.
pub static MERCY_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{HAND_OF_HARM_TAG, HAND_OF_HEALING};
    // Hand of Harm is a tag and no action — the rider rides the monk's
    // ordinary unarmed strike, so there is nothing for a controller to
    // pick. Hand of Healing is the reverse: an action and no tag, since
    // it carries no charge to spend.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*HAND_OF_HEALING);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(HAND_OF_HARM_TAG);
    CreatureTemplate {
        name: "Mercy Monk",
        glyph: 'Y',
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Way of the Astral Self Monk — Monastic Tradition **Way of the
/// Astral Self** (TCE), and the eighth monk on a roster where every
/// previous one has answered the same question by adding something to
/// the punch. Open Hand heals after it. Long Death feeds on the kill.
/// Kensei picks up a bow. Sun Soul throws light. Mercy poisons. This
/// one changes the punch.
///
/// **Arms of the Astral Self** (lv3) is a bonus action that summons
/// spectral arms for a minute, and while they hold, `ASTRAL_ARMS_STRIKE`
/// replaces the fist: Wisdom to hit and to damage, force instead of
/// bludgeoning, and 10 ft of reach instead of 5.
///
/// The reach is the half that changes the fight. Every other monk here
/// has a d8 hit die, no armour, and no way to threaten anything it is
/// not standing next to; this one threatens from a tile back, which is
/// the difference between eating an opportunity attack on the way out
/// and never being adjacent to provoke one. It also means the arms
/// reach *over* an ally in a doorway, and reach a large creature whose
/// own reach is 2 without standing inside it.
///
/// The Wisdom swap is what pays for the reach. The stat line is the
/// baseline monk's with DEX and WIS traded — 14 DEX, 16 WIS — which
/// leaves the unarmored AC where it was (10 + 2 + 3 = 15, the same
/// number by a different route) and makes the arms strictly the better
/// swing while they are up. That is deliberate: the fist stays on the
/// action list as the round-one fallback and the answer to a Dispel
/// Magic, and it is a little worse than the arms rather than
/// unplayable.
///
/// Force is the third clause and the quietest. It is the rarest-
/// resisted damage type in the bestiary — the skeletons and zombies
/// and elementals that shrug off a bludgeoning fist take it in full —
/// so the arms are also the answer to the matchups the chassis was
/// worst at.
///
/// **Empowered Arms** (lv11) adds the martial arts die once per turn
/// while the arms are up, on the `ONCE_PER_TURN_WEAPON_DIE_RIDERS`
/// cohort next to Deft Strike and Hand of Harm — and it is the first
/// row there gated on a condition rather than a tag, which is what
/// keeps the die honest on a monk whose minute has run out.
///
/// **Body of the Astral Self: Deflect Energy** (lv11) is a reaction
/// that takes 1d10 + WIS off any acid, cold, fire, force, lightning,
/// necrotic, poison, psychic, radiant or thunder damage. It rides
/// `REACTIVE_DAMAGE_CLAMPS` directly above Interception, and its real
/// value is the pairing: Deflect Missiles, three rows up on the same
/// monk, already covers ranged physical damage, so between the two the
/// only thing an Astral Self monk holding a reaction cannot blunt is a
/// melee weapon swing.
///
/// Left out: **Visage of the Astral Self** (lv6) grants Astral Sight
/// and advantage on Insight and Intimidation checks — a darkvision
/// ribbon and two skills, and the engine rolls no skill checks. RAW's
/// lv3 Strength-check-and-save substitution has the same problem for
/// its check half, and its save half would be a substitution lane of
/// its own for one subclass. **Awakened Astral Self** (lv17) grants
/// +2 AC and a third arm strike per Attack action; both are levels
/// past what this chassis targets, and the third strike is the
/// Flurry of Blows the template already carries by another name.
///
/// Glyph 'A' — free on the monk family, where 'M', 'O', 'W', 'E', 'K',
/// 'U' and 'Y' are taken.
pub static ASTRAL_SELF_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        ARMS_OF_THE_ASTRAL_SELF, DEFLECT_ENERGY_TAG, EMPOWERED_ARMS_TAG,
    };
    use crate::actions::monster_attacks::ASTRAL_ARMS_STRIKE;
    // Two actions and two tags. The arms are summoned by one action and
    // swung by another, which is the shape the Stars Druid's Starry
    // Form and Starry Bolt already use; the two lv11 features are
    // passive rows on engine cohorts and have nothing to pick.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*ARMS_OF_THE_ASTRAL_SELF);
    actions.push(&ASTRAL_ARMS_STRIKE);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(EMPOWERED_ARMS_TAG);
    features.insert(DEFLECT_ENERGY_TAG);
    CreatureTemplate {
        name: "Astral Self Monk",
        glyph: 'A',
        // The baseline monk's DEX and WIS, swapped. Unarmored Defense
        // reads 10 + DEX + WIS either way, so the AC 15 inherited from
        // the chassis is still the right number — but the arms are
        // anchored on Wisdom, and a 14 there would have made the
        // subclass's whole feature worse than the fist it replaces.
        dexterity: 14,
        wisdom: 16,
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Drunken Master Monk — Monastic Tradition **Way of the Drunken
/// Master** (XGtE), and the ninth monk on a roster where the previous
/// eight all answer the same question: what do you add to the punch?
/// This one adds nothing to it. Every feature here is about the monk
/// *not being where the swing went*, which is a lane the chassis has
/// never had.
///
///   - **Drunken Technique** (lv3): Flurry of Blows also grants the
///     Disengage benefit and +10 ft of movement. On any other monk the
///     bonus action is a fork — Flurry to hit more, Step of the Wind to
///     leave safely — and this one takes both prongs. A Drunken Master
///     can walk into contact, throw three unarmed strikes, and walk back
///     out without provoking, every single turn, for free.
///
///   - **Tipsy Sway: Redirect Attack** (lv6): when a melee attack misses
///     the monk, the monk's reaction makes it land on something else
///     standing next to them instead. Not a counter-attack and not a
///     clamp — the attacker's own damage, moved. See
///     `engine::attack::try_fire_redirect_attack`.
///
///   - **Drunkard's Luck** (lv11): once per short rest, cancel
///     disadvantage on an attack roll or a saving throw. See
///     `DRUNKARDS_LUCK_TAG`.
///
/// The three read as one idea from three directions, and the middle one
/// is the tell. Redirect Attack only pays when the monk is standing in a
/// crowd — it needs a second enemy within five feet — and Drunken
/// Technique is what makes standing in a crowd survivable, because the
/// monk can leave it at the end of the turn without eating the
/// opportunity attacks that leaving would normally cost. A Drunken
/// Master who plays the way the Kensei or the Sun Soul plays, from range
/// or from the edge, gets almost nothing out of the subclass. One who
/// wades into the middle of three goblins gets a third attack, a free
/// exit, and every miss against them turned into damage on one of the
/// other two.
///
/// Which is the opposite trade to every other monk here. The chassis's
/// standing problem is that a d8 hit die with no armour cannot afford to
/// be in contact, and the roster's answers have been to leave (Shadow's
/// teleport, Kensei's bow), to out-heal it (Open Hand, Long Death), or
/// to reach from a tile back (Astral Self). This one leans in and makes
/// the crowd the resource.
///
/// **Intoxicated Frenzy** (lv17) is deliberately not shipped. RAW lets
/// Flurry of Blows make up to three additional attacks provided each
/// targets a different creature; the engine's Flurry hands over an extra
/// *Action* rather than a count of strikes, so the RAW cap and the RAW
/// distinct-target clause have nothing to attach to — and a fourth and
/// fifth swing on a chassis that already gets three would be a much
/// larger feature than the one being modelled. **Tipsy Sway: Leap to
/// Your Feet** (lv6's other half) lets the monk stand from prone for
/// 5 ft of movement instead of half its speed; standing up is not
/// separately priced in the engine's movement lane, so the discount has
/// nothing to discount.
///
/// Glyph 'B' — free on the monk family, where 'M', 'O', 'D', 'W', 'E',
/// 'K', 'U', 'Y' and 'A' are taken.
pub static DRUNKEN_MASTER_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        DRUNKARDS_LUCK_TAG, DRUNKEN_TECHNIQUE_TAG, REDIRECT_ATTACK_TAG,
    };
    // Three tags and no new actions — which is unusual on this chassis
    // and is the subclass's own shape rather than a shortcut. Drunken
    // Technique rides the Flurry the monk already presses, Redirect
    // Attack is a reaction the engine spends, and Drunkard's Luck is a
    // charge read at two d20 chokepoints. There is nothing here for a
    // controller to pick, because the subclass never asks the monk to do
    // anything it wasn't already doing.
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(DRUNKEN_TECHNIQUE_TAG);
    features.insert(REDIRECT_ATTACK_TAG);
    features.insert(DRUNKARDS_LUCK_TAG);
    CreatureTemplate {
        name: "Drunken Master Monk",
        glyph: 'B',
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Ascendant Dragon Monk — Monastic Tradition **Way of the Ascendant
/// Dragon** (FTD), the tenth monk on the roster and the first whose
/// answer to a room full of enemies is not to pick one of them.
///
/// Three features ship, and they are one idea told three times: the
/// monk stops being a single-target creature.
///
///   - **Draconic Strike** (lv3): the martial-arts fist, retyped to the
///     ancestor's element. Ships as a second weapon beside the first —
///     see `DRACONIC_STRIKE` for why the choice belongs to the attack
///     picker rather than to a per-hit override.
///
///   - **Breath of the Dragon** (lv3): a 20-ft cone of the ancestral
///     element, DEX save for half, priced out of the ki pool. See
///     `BREATH_OF_THE_DRAGON_TAG`.
///
///   - **Aspect of the Wyrm** (lv11): every hostile within 30 ft saves
///     or is Frightened for a minute. See `ASPECT_OF_THE_WYRM_TAG`.
///
/// The subclass's whole shape is that all three want the *same board* —
/// a cluster of enemies the monk is standing in the middle of — and two
/// of them spend from the same five ki. That is the decision: a monk
/// who opens with the breath has one fewer stun, and one who frightens
/// the room has one fewer breath. Every other monk here spends ki on
/// things that do not compete for a target (Stunning Strike wants one
/// enemy, Empty Body wants none), so this is the first monk whose pool
/// is spent on a reading of the board rather than on a reading of the
/// monk's own hit points.
///
/// Contrast the Sun Soul, the roster's other area monk. Searing
/// Sunburst is thrown 150 ft at a cluster the monk is nowhere near, and
/// its damage is radiant — the most polarised type in the bestiary.
/// This one exhales from its own face at a cluster it is standing in,
/// and its damage is whatever the ancestry says. The two are the same
/// mechanic answering opposite questions about where the monk wants to
/// be, and the Ascendant Dragon's answer is the one the chassis is
/// otherwise bad at surviving — which is what Aspect of the Wyrm is
/// for.
///
/// **Fire ancestry**, declared through `draconic_ancestry` so the
/// breath and the strike read the same element from one field. Fire is
/// the most-resisted element on the roster, which is deliberate rather
/// than incidental: it is why the ordinary bludgeoning fist stays on
/// the sheet, and why the attack picker has something to decide every
/// round.
///
/// RAW features not shipped: **Wings Unfurled** (lv6 — a flying speed
/// for one turn per use of Step of the Wind; the engine's flight is a
/// template-level speed rather than something a turn can grant), the
/// resistance half of **Aspect of the Wyrm** (see its tag for the
/// missing lane), and **Ascendant Aspect**'s (lv17) blindsight and
/// third-element burst.
///
/// Glyph 'R' — for w**R**yrm; free on the monk family, where 'M', 'O',
/// 'D', 'W', 'E', 'K', 'U', 'Y', 'A' and 'B' are taken.
pub static ASCENDANT_DRAGON_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        ASPECT_OF_THE_WYRM, ASPECT_OF_THE_WYRM_TAG, BREATH_OF_THE_DRAGON,
        BREATH_OF_THE_DRAGON_TAG,
    };
    use crate::actions::monster_attacks::DRACONIC_STRIKE;
    use crate::engine::types::DamageType;
    // The ordinary fist stays: fire is the element half the bestiary
    // shrugs off, and a monk with only a fire punch has nothing to do
    // against a salamander.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&DRACONIC_STRIKE);
    actions.push(&*BREATH_OF_THE_DRAGON);
    actions.push(&*ASPECT_OF_THE_WYRM);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(BREATH_OF_THE_DRAGON_TAG);
    features.insert(ASPECT_OF_THE_WYRM_TAG);
    CreatureTemplate {
        name: "Ascendant Dragon Monk",
        glyph: 'R',
        actions,
        features,
        // Read by both Breath of the Dragon (its damage type) and — as
        // the flavor half of the same pick — the Draconic Strike's
        // fixed fire typing. One field, so the two can never disagree.
        draconic_ancestry: Some(DamageType::Fire),
        // 5e Draconic Ancestry-adjacent: the Ascendant Dragon monk does
        // not get RAW resistance to its element, and none is granted
        // here. The ancestry field is the breath's damage type and
        // nothing else.
        ..MONK_TEMPLATE.clone()
    }
});
