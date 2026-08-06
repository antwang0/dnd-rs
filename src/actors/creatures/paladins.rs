use crate::actions::class_features::{
    ABJURE_ENEMY, ABJURE_ENEMY_TAG, AURA_OF_ALACRITY_TAG, AURA_OF_THE_SENTINEL_TAG,
    CLEANSING_TOUCH, CLEANSING_TOUCH_TAG, DIVINE_SMITE, DREADFUL_ASPECT, DREADFUL_ASPECT_TAG,
    FANATICAL_FOCUS_TAG, IMPROVED_DIVINE_SMITE_TAG, LAY_ON_HANDS, LAY_ON_HANDS_TAG, NATURES_WRATH,
    NATURES_WRATH_TAG, PALADIN_CHANNEL_DIVINITY_TAG, REBUKE_THE_VIOLENT,
    REBUKE_THE_VIOLENT_TAG, SACRED_WEAPON, SACRED_WEAPON_TAG,
    TURN_THE_FAITHLESS, TURN_THE_FAITHLESS_TAG, UNDYING_SENTINEL_TAG, VOW_OF_ENMITY,
    VOW_OF_ENMITY_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATSWORD;
use crate::actions::spells::{
    AURA_OF_LIFE, AURA_OF_PURITY, BANISHING_SMITE, BLESS, BLINDING_SMITE, BRANDING_SMITE,
    COMPELLED_DUEL, CURE_WOUNDS, DESTRUCTIVE_WAVE, HEALING_WORD, LESSER_RESTORATION,
    SEARING_SMITE, SHIELD_OF_FAITH, STAGGERING_SMITE, THUNDEROUS_SMITE, WRATHFUL_SMITE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Paladin PC template. The classic CHA-flavored holy warrior: half-caster
/// for divine spells, full martial for weapon hits, plus a stacked set of
/// once-per-rest class features. Signature mechanics:
/// - **Divine Smite** (bonus action + level-1 slot): primes the next
///   melee hit with +2d8 radiant damage via the Smiting condition.
/// - **Lay on Hands** (action, 1/rest): touch heal for `5 * level + CHA` HP.
/// - **Channel Divinity: Sacred Weapon** (action, 1/rest): +CHA to attack
///   rolls for 10 rounds via the Sacred condition.
/// - **Compelled Duel** (bonus action, level-1 slot): WIS save → Dueled,
///   target eats disadvantage attacking anyone but the paladin.
///
/// Stats target a level-3 paladin: 27 HP (3d10+6 like the Fighter),
/// AC 18 (chain mail + shield baseline), STR 16, CHA 14, half-caster
/// slots (4/2 = level-1 + level-2). Stretches Lay on Hands' big single
/// chunk + a smite or two before the slot pool dries.
pub static PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATSWORD);
    actions.push(&*LAY_ON_HANDS);
    actions.push(&*DIVINE_SMITE);
    actions.push(&*SACRED_WEAPON);
    actions.push(&*BLESS);
    actions.push(&*CURE_WOUNDS);
    actions.push(&HEALING_WORD);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*COMPELLED_DUEL);
    // 5e Find Steed (lv2 conjuration) — the paladin's own summon, and
    // the party's route into `engine::mounts`. No concentration, so it
    // costs the paladin nothing they were going to spend on a smite.
    actions.push(&crate::actions::spells::FIND_STEED);
    // Smite spells — bonus-action concentration primes that lay extra
    // rider damage (and a follow-up effect for Wrathful / Branding /
    // Blinding) on the paladin's next melee hit. Slot-cost varies per
    // spell (1 / 1 / 2 / 3); the half-caster slot table supports them.
    actions.push(&SEARING_SMITE);
    actions.push(&WRATHFUL_SMITE);
    actions.push(&THUNDEROUS_SMITE);
    actions.push(&BRANDING_SMITE);
    actions.push(&BLINDING_SMITE);
    // Higher-tier smite primes — lv4 Staggering Smite (psychic + WIS-save
    // Stunned-1) and lv5 Banishing Smite (force + HP≤50 auto-banish via
    // the Mazed envelope). Slot cost climbs but the rider impact does
    // too; the half-caster slot bump below fuels both.
    actions.push(&STAGGERING_SMITE);
    actions.push(&BANISHING_SMITE);
    // Aura of Life — lv4 abjuration, concentration; allies in 30ft sphere
    // gain DeathWarded (next killing-blow drop intercepted) for the
    // duration. Bumps the paladin into the lv4 slot table; combined with
    // the half-caster ramp below it gives the late-game paladin a true
    // mass-save-the-party button alongside the smite primes.
    actions.push(&*AURA_OF_LIFE);
    // Aura of Purity — lv4 abjuration, concentration; allies in the same
    // 30ft sphere pick up Purified, granting dynamic immunity to
    // Charmed / Frightened / Poisoned installs plus poison resistance
    // for the duration. Trades Aura of Life's killing-blow interception
    // for broader debuff coverage — the AI's smite picker steers between
    // them based on which axis the incoming encounter pressures.
    actions.push(&*AURA_OF_PURITY);
    // Destructive Wave — lv5 evocation. Self-burst (6-tile radius)
    // enemy-only CON-save burst dealing 5d6 thunder + 5d6 radiant +
    // prone-on-fail. Non-concentration — pairs cleanly with whichever
    // smite the paladin's currently holding. The split damage type
    // slips past single-element resistance the same way Flame Strike
    // (fire + radiant) does.
    actions.push(&*DESTRUCTIVE_WAVE);
    // Circle of Power — lv5 abjuration, concentration. The paladin's
    // answer to an enemy caster: every ally in a 30-ft radius saves
    // against spells at advantage and takes no damage at all on a save
    // that would otherwise have halved it. Competes with Aura of Life /
    // Aura of Purity for the concentration slot along a third axis —
    // those two blunt what lands, this one stops it landing.
    actions.push(&*crate::actions::spells::CIRCLE_OF_POWER);
    // Warding Bond — lv2 abjuration. Touch-range damage-share bond.
    // The paladin already takes the hits up front (high HP, AC 18) —
    // bonding a frailer ally (e.g. cleric / wizard) halves their
    // incoming damage at the cost of mirroring the rest onto the
    // paladin's much larger HP pool. Non-concentration, so it stacks
    // with whichever smite is currently holding the slot.
    actions.push(&*crate::actions::spells::WARDING_BOND);
    // Holy Weapon — lv5 evocation, concentration. Self-only buff: every
    // weapon hit gains +2d8 radiant via the on_hit_riders table. Persistent
    // for the duration (not consumed on trigger) — distinct from the
    // one-shot smite primes that hold the same concentration slot, so the
    // AI's smite picker steers around it when it's already up.
    actions.push(&*crate::actions::spells::HOLY_WEAPON);
    // Cleansing Touch — Oath capstone (RAW: lv14). Once-per-long-rest
    // action that ends one spell on a willing target. The three-tier
    // dispel logic (drop concentration → strip spell-debuff → strip
    // beneficial buff) lives inside `CleansingTouchOn`; the action here
    // just spends the feature charge and queues the dispel. Headline use
    // case: clear Hold Person / Charm / Fear off an ally without burning
    // a Greater Restoration slot.
    actions.push(&*CLEANSING_TOUCH);
    // Latest paladin additions:
    //   - lv4 **Freedom of Movement**: touch ally-buff that strips active
    //     Paralyzed / Restrained / Grappled installs AND grants dynamic
    //     immunity for the duration. Slots cleanly into the paladin's
    //     touch-cleanse / touch-support kit alongside Lay on Hands.
    //   - lv5 **Raise Dead**: touch revive a dying ally to 1 HP — the
    //     paladin's high-tier panic option, costlier than Revivify but
    //     usable when only level-5 slots remain.
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    actions.push(&*crate::actions::spells::RAISE_DEAD);
    // lv3 **Elemental Weapon** — touch ally weapon-buff: +1 attack and
    // +1d4 fire per melee hit (concentration). RAW paladin spell list.
    // Sibling to Holy Weapon (lv5 self-only +2d8 radiant rider) on the
    // paladin's weapon-buff lane; distinguished by the ally-target reach
    // (lets the paladin power up the party's fighter / barbarian) AND
    // the cheaper lv3 slot cost. Mutually exclusive with Holy Weapon at
    // the concentration lane — the AI's smite picker steers around it
    // when it's already up.
    actions.push(&*crate::actions::spells::ELEMENTAL_WEAPON);
    // lv5 **Summon Celestial** (TCE) — the paladin's only summon, and
    // the top of their slot table, so it is the most expensive thing a
    // paladin can do with a turn that isn't a smite. What it buys is the
    // thing a paladin structurally cannot do: threaten something at
    // range while staying in the front rank where their auras are worth
    // having.
    actions.push(&crate::actions::spells::SUMMON_CELESTIAL);
    CreatureTemplate {
        name: "Paladin",
        glyph: 'P',
        ac: 18,
        hitpoints: "3d10+6".parse().unwrap(),
        strength: 16,
        dexterity: 12,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 14, // spellcasting ability + Sacred Weapon scaling
        languages: HashSet::from([Language::Common, Language::Celestial]),
        cr: 1.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Half-caster ramp: 4/3/3/2/1. Bumps lv3 → 3 slots and adds a
        // lv5 slot for Banishing Smite; lv4 stays at 2 to fuel Aura of
        // Life + Staggering Smite. Earlier 4/3/1 envelope is preserved
        // on lv1-2 so Bless / Divine Smite / Compelled Duel spam stays
        // unchanged.
        spell_slots_by_level: vec![4, 3, 3, 2, 1],
        rolls_death_saves: true,
        // Paladins are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        features: HashSet::from([
            LAY_ON_HANDS_TAG,
            SACRED_WEAPON_TAG,
            CLEANSING_TOUCH_TAG,
            // The pool Sacred Weapon and every oath's Channel Divinity
            // spend from — one press per rest between all of them, which
            // is RAW at every level this chassis represents. Inherited
            // by every oath template through `..PALADIN_TEMPLATE.clone()`.
            PALADIN_CHANNEL_DIVINITY_TAG,
            // Improved Divine Smite (level 11+): passive +1d8 radiant
            // on every melee weapon hit. The rider fires in
            // `engine::attack::resolve_attack_outcome` right after the
            // ON_HIT_RIDERS loop — gated on `has_passive_feature`. The
            // CR 1.5 template lists this above its strict RAW level
            // gate for the same reason every other class template runs
            // above strict RAW level (templates target a balanced
            // playable level, not lockstep PHB progression).
            IMPROVED_DIVINE_SMITE_TAG,
        ]),
        has_extra_attack: true,
        // 5e Paladin **Fighting Style: Defense** (lv2 pick): passive +1 AC
        // while wearing armor. RAW gate collapses to "always on" since the
        // engine doesn't model armor tiers. The plate-baseline paladin
        // (AC 18) becomes AC 19 with the style pick — folds into
        // `armor_class` next to the item / condition AC lanes. Composes
        // cleanly with the paladin's aura + smite kit: the +1 AC keeps
        // the paladin standing longer for the aura bubble to keep
        // ticking. Distinct from Dueling (Fighter baseline) — the
        // paladin's greatsword is two-handed so Dueling doesn't apply
        // RAW, but Defense is the natural pick for a shield-forward
        // (or plate-forward) paladin build.
        has_defense_style: true,
        // 5e Paladin **Fighting Style: Great Weapon Fighting** (lv2 pick,
        // second-style pickup at higher levels — same reasoning that
        // ships Defense here alongside the class's smite kit): reroll
        // any 1 / 2 on a melee weapon damage die once. RAW pairs with
        // two-handed / versatile-two-handed weapons; the paladin's
        // greatsword (`2d6` slashing) is the canonical fit, so the flag
        // rides the greatsword-baseline paladin. Composes cleanly with
        // Improved Divine Smite (+1d8 radiant on hit) and every smite
        // spell prime (Searing / Wrathful / Thunderous / Branding /
        // Blinding / Staggering / Banishing) — those riders roll fresh
        // dice on hit and don't share the weapon damage bundle, so the
        // GWF reroll doesn't double-tax them. The per-die reroll routes
        // through `EncounterInstance::roll_weapon_damage_dice`, applied
        // to both the base greatsword swing AND the crit's doubled dice.
        has_great_weapon_fighting: true,
        // Aura of Protection (level 6+): allies within 10ft add the
        // paladin's CHA mod (min +1) to all saves. The headline late-
        // game paladin feature — turns the squishy wizard adjacent to
        // the paladin into a save-throwing tank. Engine reads via
        // `EncounterInstance::aura_of_protection_bonus`.
        has_aura_of_protection: true,
        // Aura of Courage (level 10+): allies within 10ft are immune to
        // Frightened. Suppresses installs at the `ApplyCondition::apply`
        // site so Cause Fear / Wrathful Smite / dragon-fear all bounce
        // off the aura bubble.
        has_aura_of_courage: true,
        skills: HashSet::from([Skill::Athletics]),
        ..CreatureTemplate::defaults()
    }
});

/// Devotion Paladin — Oath of Devotion subclass build. Identical
/// envelope to the baseline `PALADIN_TEMPLATE` (greatsword + smite suite,
/// half-caster slot ladder, Lay on Hands / Sacred Weapon / Cleansing
/// Touch, Improved Divine Smite passive, Aura of Protection / Aura of
/// Courage) with one subclass feature layered on: **Aura of Devotion**
/// (Devotion subclass level 7) — passive 10ft ally-aura that suppresses
/// Charmed installs.
///
/// Pairs naturally with the paladin's existing anti-social-debuff kit:
/// the Devotion paladin's aura sits on top of Aura of Courage (Frightened
/// suppression) and Aura of Protection (save bonus), forming a three-
/// aura bubble that shuts down the classic charm-fear-save pressure
/// spellcasters lean on. The RAW Charmed-suppression on Devotion is the
/// signature "unshakeable defender" tell — Sacred Weapon (Channel
/// Divinity: +CHA to attack rolls, already on the baseline) is the
/// Devotion action-lane, the aura is the ambient-lane.
///
/// Distinct from `VENGEANCE_PALADIN_TEMPLATE` (Vow of Enmity flavor) and
/// `PALADIN_TEMPLATE` (baseline). Glyph 'D' so the Devotion paladin
/// shows up distinctly next to Vengeance 'V' and baseline 'P'.
pub static DEVOTION_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Paladin envelope wholesale
    // and layer on Devotion-specific features:
    //   - `has_aura_of_devotion` (lv7): passive 10ft ally aura suppressing
    //     Charmed installs. Read at the flag-driven immunity table so
    //     adjacent allies (and the paladin themselves) bounce Charm
    //     Person / Suggestion / Dominate Person installs.
    //   - `TURN_THE_FAITHLESS_TAG` (lv3 CD): once-per-short-rest 30ft
    //     WIS-save burst that Frightens fey / fiend on fail. Ships as a
    //     paired action + feature charge; the shared `resolve_turn_burst`
    //     helper drives both this and Turn Undead so save/log rule
    //     changes land once.
    //   - `REBUKE_THE_VIOLENT_TAG` (lv15 subclass feature): once-per-
    //     short-rest 30ft single-target 4d10 radiant WIS-save burst.
    //     Ships above its strict RAW level gate for the same reason
    //     Nature's Ward / Undying Sentinel (lv15) ship on the CR-1.5
    //     Ancients paladin — class templates target a balanced
    //     playable level, not lockstep PHB progression. Rounds out
    //     the Devotion paladin's CD lane with a damage-burst sibling
    //     to Turn the Faithless's Frighten-burst.
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    actions.push(&*TURN_THE_FAITHLESS);
    actions.push(&*REBUKE_THE_VIOLENT);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(TURN_THE_FAITHLESS_TAG);
    features.insert(REBUKE_THE_VIOLENT_TAG);
    CreatureTemplate {
        name: "Devotion Paladin",
        glyph: 'D',
        has_aura_of_devotion: true,
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});

/// Ancients Paladin — Oath of the Ancients subclass build. Identical
/// envelope to the baseline `PALADIN_TEMPLATE` (greatsword + smite suite,
/// half-caster slot ladder, Lay on Hands / Sacred Weapon / Cleansing
/// Touch, Improved Divine Smite passive, Aura of Protection / Aura of
/// Courage) with one subclass feature layered on: **Nature's Ward**
/// (Ancients subclass level 15 capstone) — passive self-immunity to
/// Charmed AND Frightened installs. RAW also grants immunity to disease
/// and no aging; neither has a mechanical surface in the combat engine
/// so both halves are RAW no-ops we don't wire up.
///
/// Pairs naturally with the paladin's aura family: the Ancients paladin
/// covers themselves (via Nature's Ward) AND every 10ft-adjacent ally
/// (via Aura of Courage — Frightened suppression). Distinct from Devotion
/// Paladin (Aura of Devotion → ally Charmed suppression) — the Ancients
/// paladin's Charmed-immunity is self-only, but the Frightened-immunity
/// is redundant with Aura of Courage on themselves and extended to allies
/// only through the aura. The CR-1.5 template ships this capstone above
/// its strict RAW level gate for the same reason every other class
/// template runs above strict RAW level (templates target a balanced
/// playable level, not lockstep PHB progression).
///
/// Ships the CR-1.5 template above strict RAW level gate. Distinct from
/// `PALADIN_TEMPLATE` (baseline), `DEVOTION_PALADIN_TEMPLATE` (Aura of
/// Devotion), and `VENGEANCE_PALADIN_TEMPLATE` (Vow of Enmity). Glyph
/// 'A' so the Ancients paladin shows up distinctly next to baseline 'P',
/// Devotion 'D', and Vengeance 'V'.
pub static ANCIENTS_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Three subclass features layer onto the baseline paladin envelope:
    //   - `has_aura_of_warding` (lv7): passive 10ft ally-aura that
    //     halves spell-typical damage on nearby allies. Template-flag
    //     lane, no charge. Read at `DealDamage::apply` via
    //     `EncounterInstance::is_in_aura_of_warding`. Fires alongside
    //     the paladin's other aura auto-scans (Aura of Protection at
    //     lv6 for saves and Aura of Courage at lv10 for Frightened
    //     suppression) so an Ancients paladin bubbles allies with
    //     three overlapping aura effects simultaneously.
    //   - `has_natures_ward` (lv15): passive self-immunity to Charmed /
    //     Frightened installs. Template-flag lane, no charge.
    //   - `UNDYING_SENTINEL_TAG` (lv15): once-per-long-rest "drop to 1
    //     HP instead of 0" cheat-death. Feature-set lane, refreshed on
    //     long rest via the `features_max` copy in
    //     `ActorInstance::new_from_template`. Mechanically identical
    //     to Half-Orc Relentless Endurance — both route through the
    //     shared `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort in
    //     `take_typed_damage` so a multiclass (half-orc Ancients
    //     paladin) spends the tags in order rather than double-dipping
    //     on the same lethal hit.
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    // 5e Ancients Paladin level-3 Channel Divinity — Nature's Wrath.
    // Single-target 10ft STR-save Restrained install; once per short
    // rest. Ships alongside the CD family Turn the Faithless (Devotion)
    // / Guided Strike (War) / Radiance of the Dawn (Light) as the
    // paladin's per-subclass Channel Divinity pick. The Ancients paladin
    // trades Turn the Faithless's fey/fiend-only 30ft burst for Nature's
    // Wrath's single-target 10ft lock — better focus fire on a single
    // priority target (the paladin's smite loop wants adjacency anyway,
    // so the shorter range is a wash), worse round-clear on a swarm.
    actions.push(&*NATURES_WRATH);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(UNDYING_SENTINEL_TAG);
    // Nature's Wrath charge — once per short rest, refreshed via
    // SHORT_REST_FEATURES alongside the Devotion / War / Light CDs.
    features.insert(NATURES_WRATH_TAG);
    CreatureTemplate {
        name: "Ancients Paladin",
        glyph: 'A',
        // Aura of Warding — passive template flag, always on. The
        // 10ft aura's spell-damage halving lives at
        // `is_in_aura_of_warding` on the encounter and the halving
        // pass in `DealDamage::apply`. Ships on the CR-1.5 template
        // above its strict RAW lv7 gate for the same reason Nature's
        // Ward / Undying Sentinel ride here — class templates target
        // a balanced playable level, not lockstep PHB progression.
        has_aura_of_warding: true,
        // Nature's Ward — passive template flag, always on.
        has_natures_ward: true,
        // Undying Sentinel — once-per-long-rest cheat-death. Layered
        // onto the inherited paladin feature set (Lay on Hands,
        // Sacred Weapon, Cleansing Touch, Improved Divine Smite)
        // rather than clobbering it.
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});

/// Vengeance Paladin — Oath of Vengeance subclass build. Identical
/// envelope to the baseline `PALADIN_TEMPLATE` (greatsword + smite suite,
/// half-caster slot ladder, Lay on Hands / Sacred Weapon / Cleansing
/// Touch, Improved Divine Smite passive, Aura of Protection /
/// Aura of Courage) with one subclass feature layered on:
/// **Vow of Enmity** (lv3 Channel Divinity) — bonus action, once per
/// long rest. Mark a hostile creature within 10 ft; the paladin gets
/// advantage on attack rolls against that target for up to 10 rounds.
///
/// Pairs naturally with the paladin's smite primes: vow first to lock
/// in advantage on the target, then bonus-action a smite prime, then
/// swing the greatsword for the guaranteed-advantage Smite hit (and
/// trigger the once-per-turn Improved Divine Smite rider on top).
///
/// Distinct from `PALADIN_TEMPLATE` (Devotion-equivalent baseline) so a
/// Vengeance-vs-Devotion or Vengeance-vs-baseline encounter renders
/// unambiguously by name and the subclass feature doesn't accidentally
/// stack RAW-illegally on a single PC build. Glyph 'V' so the Vengeance
/// paladin shows up distinctly on the map next to the baseline 'P'.
pub static VENGEANCE_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Paladin envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // actions / features). The `..base.clone()` tail picks up every
    // other field — stats, slots, save profs, auras, extra-attack —
    // without an N-line field-by-field copy. Same shape as
    // `HUNTER_RANGER_TEMPLATE` and `ASSASSIN_ROGUE_TEMPLATE`.
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    actions.push(&*VOW_OF_ENMITY);
    // Abjure Enemy (lv3 Vengeance CD): single-target 60ft WIS-save
    // Frighten install; once per short rest. RAW gives the Vengeance
    // paladin the CHOICE between Abjure Enemy and Vow of Enmity when
    // spending a CD charge. We ship both on the template as distinct
    // per-rest tags so the AI can pick either depending on whether it
    // wants an attack prime (Vow of Enmity) or a target debuff (Abjure
    // Enemy). Same short-rest gate as the sibling paladin CDs (Nature's
    // Wrath / Turn the Faithless / Guided Strike).
    actions.push(&*ABJURE_ENEMY);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(VOW_OF_ENMITY_TAG);
    features.insert(ABJURE_ENEMY_TAG);
    CreatureTemplate {
        name: "Vengeance Paladin",
        glyph: 'V',
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});

/// Oathbreaker Paladin — Oath-broken subclass build (DMG). The dark
/// counterpart to the Devotion / Ancients / Vengeance chassis:
/// identical envelope to the baseline `PALADIN_TEMPLATE` (greatsword
/// + smite suite, half-caster slot ladder, Lay on Hands / Sacred
///   Weapon / Cleansing Touch, Improved Divine Smite passive, Aura of
///   Protection / Aura of Courage) with two subclass features layered
///   on:
///
///   - **Aura of Hate** (Oathbreaker subclass level 7) — passive
///     template flag: +CHA modifier (min +1) to melee weapon damage
///     rolls. Read at the shared `MELEE_CASTER_BUMPS` table next to
///     Rage / Dueling / Two-Weapon Fighting. RAW's ally-side aura
///     on adjacent fiends / undead is dropped since the engine
///     doesn't tag those as an aura-eligible cohort at the template
///     level; the self-side +CHA bump is the mechanical core.
///
///   - **Fanatical Focus** (Oathbreaker subclass level 15) — once-
///     per-short-rest passive: on a failed saving throw, re-roll
///     once with the same modifier / mode. Auto-fires at the save
///     site — no Action call, no pre-priming (distinct from
///     Fighter Indomitable). Ships as a `FANATICAL_FOCUS_TAG`
///     feature charge on the template's `features` set; refreshes
///     via `SHORT_REST_FEATURES`.
///
/// Ships the CR-1.5 template above the strict RAW level gates
/// (Aura of Hate lv7, Fanatical Focus lv15) for the same reason
/// Nature's Ward / Undying Sentinel ride on `ANCIENTS_PALADIN_TEMPLATE`
/// — class templates target a balanced playable level, not lockstep
/// PHB progression. Distinct from Ancients / Devotion / Vengeance
/// paladins on the aura family: Ancients projects self-immunity
/// (Nature's Ward), Devotion projects ally-Charmed suppression
/// (Aura of Devotion), Vengeance holds the Vow of Enmity attack
/// prime, and Oathbreaker plays the "raw damage aura + auto-reroll"
/// lane. Glyph 'O' so the Oathbreaker paladin shows up distinctly
/// on the map next to baseline 'P', Devotion 'D', Ancients 'A',
/// and Vengeance 'V'.
pub static OATHBREAKER_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Layer three subclass features onto the inherited paladin feature
    // set (Lay on Hands, Sacred Weapon, Cleansing Touch, Improved
    // Divine Smite) rather than clobbering it. Fanatical Focus is a
    // once-per-short-rest charge — spawning the tag in `features`
    // seeds both `features_remaining` and `features_max` at
    // instantiation, and the `SHORT_REST_FEATURES` registry copies
    // the max back into remaining on short rest.
    //
    // Dreadful Aspect (lv3 subclass Channel Divinity) ships as a paired
    // action + short-rest feature charge — the Oathbreaker's CD-lane
    // pickup (RAW subclass CD choice: Control Undead or Dreadful
    // Aspect). Sibling to the other paladin subclass CD picks: Turn
    // the Faithless (Devotion), Nature's Wrath (Ancients), Abjure
    // Enemy (Vengeance). Routes through the shared `resolve_turn_burst`
    // helper with a pass-through creature-type filter — every combat-
    // active hostile within 30ft rolls the WIS save.
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    actions.push(&*DREADFUL_ASPECT);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(FANATICAL_FOCUS_TAG);
    features.insert(DREADFUL_ASPECT_TAG);
    CreatureTemplate {
        name: "Oathbreaker Paladin",
        glyph: 'O',
        // Aura of Hate — passive template flag, read in
        // `MELEE_CASTER_BUMPS` at every melee swing site.
        has_aura_of_hate: true,
        // Fanatical Focus — once-per-short-rest failed-save reroll.
        // Dreadful Aspect — once-per-short-rest CD action + tag.
        // Both layered onto the inherited paladin feature set.
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});

/// Glory Paladin — Oath of **Glory** subclass build (TCE). Identical
/// envelope to the baseline `PALADIN_TEMPLATE` (greatsword + smite suite,
/// half-caster slot ladder, Lay on Hands / Sacred Weapon / Cleansing
/// Touch, Improved Divine Smite passive, Aura of Protection / Aura of
/// Courage) with one subclass passive layered on: **Aura of Alacrity**
/// (Glory subclass level 7, TCE) — passive **+10 ft walking-speed
/// bump**.
///
/// The signature "the glory paladin is always one step ahead of the
/// enemy line" tell — where a baseline Paladin walks at the default
/// humanoid 30 ft, the Glory Paladin opens combat at 40 ft. Composes
/// cleanly with the paladin's smite-and-melee kit — a Glory paladin
/// who's one tile deeper into the enemy line lands Divine Smite /
/// Sacred Weapon primes one round earlier — and with the aura suite
/// the class already leans on (Aura of Protection at lv6 for +CHA-mod
/// saves, Aura of Courage at lv10 for Frightened suppression, and
/// now Aura of Alacrity at lv7 for movement — three overlapping self-
/// aura effects that all fire on the same adjacent-ally scan).
///
/// The Glory Oath's passive-mobility-flavored sibling to the other
/// Paladin oaths:
///   - **Devotion** (Aura of Devotion): ally-side Charmed suppression.
///   - **Ancients** (Nature's Ward + Undying Sentinel + Aura of
///     Warding): self-side Charmed / Frightened immunity plus spell-
///     damage-halving aura plus cheat-death.
///   - **Vengeance** (Vow of Enmity + Abjure Enemy): target-side attack
///     prime plus Frighten burst.
///   - **Oathbreaker** (Aura of Hate + Fanatical Focus + Dreadful
///     Aspect): melee damage aura plus failed-save reroll plus mass-
///     Frighten CD.
///   - **Glory** (Aura of Alacrity): passive +10 ft walking speed —
///     always-on mobility on the paladin's own chassis.
///
/// Where Devotion / Ancients / Vengeance / Oathbreaker each project
/// their oath through condition installs / damage bumps / reactive
/// re-rolls, the Glory oath leans on the pure passive-mobility lane —
/// no charge to spend, no target to pick, no bonus action to prime.
/// Sibling on the "one feature tag drives one PASSIVE_FEATURE_SPEED_BONUSES
/// cohort row" declarative-table pattern to `SCOUT_ROGUE_TEMPLATE`
/// (Superior Mobility +10, XGtE lv9), the baseline `MONK_TEMPLATE` /
/// `OPEN_HAND_MONK_TEMPLATE` / `LONG_DEATH_MONK_TEMPLATE` (Unarmored
/// Movement +10, PHB lv2), the baseline `BARBARIAN_TEMPLATE` and its
/// subclasses (Fast Movement +10, PHB lv5), and the baseline
/// `RANGER_TEMPLATE` / `HUNTER_RANGER_TEMPLATE` / `GLOOM_STALKER_RANGER_TEMPLATE`
/// (Roving +5, 2024 PHB lv6) — five class chassis converge on the
/// same "passive walking-speed bump keyed off a subclass tag"
/// identity from different angles. Paladin was uncovered on the
/// passive-speed-bump cohort until this template landed; the +10 ft
/// magnitude matches the sibling +10 rows on the Barbarian / Monk /
/// Rogue chassis.
///
/// Read at the shared `PASSIVE_FEATURE_SPEED_BONUSES` cohort in
/// `actor_template.rs`. Stacks additively with the other rows per the
/// cohort's "any row hit is sufficient; all hitting rows sum" semantic:
/// a hypothetical Glory-Paladin / Scout-Rogue / Barbarian multi-classer
/// walks at +30 ft over the humanoid 30-ft baseline (three +10 rows
/// firing at once). Distinct from `INITIATIVE_ADVANTAGE_SOURCES` /
/// `ABILITY_MOD_INITIATIVE_BONUSES` cohorts (initiative-time roll
/// modifiers) — Aura of Alacrity fires on the movement chokepoint,
/// not the initiative-roll chokepoint, so a Glory Paladin who wins
/// initiative reliably (via a hypothetical multiclass) can then
/// close the extra 10 ft to the enemy line before their first smite
/// lands.
///
/// RAW's Oath of Glory picks up other features not shipped on this
/// template — **Peerless Athlete** (lv3 Channel Divinity: advantage
/// on Athletics / Acrobatics checks + carrying capacity doubles;
/// skills-only, no combat surface), **Inspiring Smite** (lv3 CD:
/// after landing Divine Smite, distribute `2d8 + paladin level` temp
/// HP among allies within 30ft; needs a per-smite-hit trigger + a
/// temp-HP distribution helper), **Glorious Defense** (lv15: reaction
/// to grant an ally within 10ft a +CHA-mod bonus to a failed save,
/// and if the save then succeeds the paladin can make one weapon
/// attack against the source; needs a save-time reaction hook plus a
/// conditional counter-attack), and **Living Legend** (lv20 capstone:
/// 1-minute self-buff — advantage on CHA checks + reroll one failed
/// save per turn + weapon crits on 19 or 20; complex multi-effect
/// self-buff). Only the lv7 Aura of Alacrity passive has a mechanical
/// surface on the CR-1.5 chassis that plugs cleanly into the shared
/// `PASSIVE_FEATURE_SPEED_BONUSES` cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit and
/// `WAR_MAGIC_WIZARD_TEMPLATE` ships only the lv2 Tactical Wit passive
/// half of its RAW School of War Magic kit.
///
/// Ships on the CR-1.5 paladin chassis at (or above) its strict RAW
/// lv7 gate for the same reason `WAR_MAGIC_WIZARD_TEMPLATE` ships
/// Tactical Wit (RAW lv2), `TWILIGHT_CLERIC_TEMPLATE` ships Vigilant
/// Blessing (RAW lv1), and `SCOUT_ROGUE_TEMPLATE` ships Superior
/// Mobility (RAW lv9) — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Distinct from `PALADIN_TEMPLATE` (subclass-less baseline) and the
/// Devotion / Ancients / Vengeance / Oathbreaker cousins so a
/// Glory-vs-Devotion / vs-Ancients / vs-Vengeance / vs-Oathbreaker /
/// vs-baseline encounter renders unambiguously by name. Glyph 'Y'
/// (for the Glor**Y** identity) — distinct from baseline paladin 'P',
/// Devotion 'D', Ancients 'A', Vengeance 'V', and Oathbreaker 'O'.
/// Collides with the Upsilon-glyph Silver Dragonborn ('Υ' vs the
/// Latin 'Y') visually, but the two never legally co-occur on a
/// single team (Silver Dragonborn is a Chromatic-family PC race
/// template, Glory Paladin is a Paladin subclass), and one glyph per
/// team-color-and-team-id combo suffices to disambiguate them in a
/// mixed encounter.
pub static GLORY_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Paladin envelope wholesale
    // and layers on the one Glory Oath subclass feature
    // (`AURA_OF_ALACRITY_TAG`: passive +10 ft walking-speed bump read
    // at the shared `PASSIVE_FEATURE_SPEED_BONUSES` cohort in
    // `actor_template.rs`). The `..base.clone()` tail inside the helper
    // picks up every other field — greatsword + smite suite, half-caster
    // slot ladder, Lay on Hands / Sacred Weapon / Cleansing Touch,
    // Improved Divine Smite passive, Defense / Great Weapon Fighting
    // fighting styles, Aura of Protection / Aura of Courage — without
    // an N-line field-by-field copy. No new actions are pushed — Aura
    // of Alacrity is a purely passive walking-speed bump, not a fresh
    // action surface, so the "tag-only" shape the helper wraps is a
    // natural fit. Sibling helper users on the "clone base + insert
    // one tag" cross-class lane: every tag-only Warlock Otherworldly
    // Patron subclass (via `subclass_warlock_template`),
    // `LIFE_CLERIC_TEMPLATE`, `FORGE_CLERIC_TEMPLATE`,
    // `TWILIGHT_CLERIC_TEMPLATE`, `NECROMANCY_WIZARD_TEMPLATE`,
    // `WAR_MAGIC_WIZARD_TEMPLATE`, `SHADOW_MAGIC_SORCERER_TEMPLATE`,
    // `ABERRANT_MIND_SORCERER_TEMPLATE`, `DIVINE_SOUL_SORCERER_TEMPLATE`,
    // `LONG_DEATH_MONK_TEMPLATE`.
    PALADIN_TEMPLATE.with_subclass_tag("Glory Paladin", 'Y', AURA_OF_ALACRITY_TAG)
});

/// Watchers Paladin — Oath of the **Watchers** subclass build (TCE).
/// Identical envelope to the baseline `PALADIN_TEMPLATE` (greatsword +
/// smite suite, half-caster slot ladder, Lay on Hands / Sacred Weapon /
/// Cleansing Touch, Improved Divine Smite passive, Aura of Protection /
/// Aura of Courage) with one subclass passive layered on: **Aura of the
/// Sentinel** (Watchers subclass level 7, TCE) — passive **+proficiency-
/// bonus** initiative-roll bump.
///
/// The signature "the watchers paladin acts first" tell — where a
/// baseline paladin rolls initiative at flat `d20 + DEX-mod`, the
/// Watchers paladin adds `d20 + DEX-mod + prof-bonus` (a +2 bump at
/// CR 1.5, scaling to +6 at the highest tier). Composes cleanly with
/// the paladin's smite-and-melee kit — a Watchers paladin who wins
/// initiative reliably opens the round with the enemy line's saves
/// eaten by a smite prime one round earlier — and with the aura suite
/// the class already leans on (Aura of Protection at lv6 for +CHA-mod
/// saves, Aura of Courage at lv10 for Frightened suppression, and now
/// Aura of the Sentinel at lv7 for the initiative-roll chokepoint —
/// three overlapping self-aura effects that all fire on the same
/// adjacent-ally scan).
///
/// The Watchers Oath's initiative-flavored sibling to the other
/// Paladin oaths:
///   - **Devotion** (Aura of Devotion): ally-side Charmed suppression.
///   - **Ancients** (Nature's Ward + Undying Sentinel + Aura of
///     Warding): self-side Charmed / Frightened immunity plus spell-
///     damage-halving aura plus cheat-death.
///   - **Vengeance** (Vow of Enmity + Abjure Enemy): target-side attack
///     prime plus Frighten burst.
///   - **Oathbreaker** (Aura of Hate + Fanatical Focus + Dreadful
///     Aspect): melee damage aura plus failed-save reroll plus mass-
///     Frighten CD.
///   - **Glory** (Aura of Alacrity): passive +10 ft walking speed —
///     always-on mobility on the paladin's own chassis.
///   - **Watchers** (Aura of the Sentinel): passive +prof-bonus
///     initiative bump — always-on initiative-roll augment on the
///     paladin's own chassis.
///
/// Where Glory projects through the movement chokepoint (+10 ft
/// walking speed on `PASSIVE_FEATURE_SPEED_BONUSES`), the Watchers
/// Oath projects through the initiative-roll chokepoint on the sibling
/// `PROFICIENCY_INITIATIVE_BONUSES` cohort — same "always-on passive
/// that fires on a specific engine chokepoint" pattern, different
/// axis. Sibling on the "one feature tag drives one initiative-roll
/// cohort row" declarative-table pattern to
/// `SWASHBUCKLER_ROGUE_TEMPLATE` (Rakish Audacity +CHA-mod),
/// `GLOOM_STALKER_RANGER_TEMPLATE` (Dread Ambusher +WIS-mod), and
/// `WAR_MAGIC_WIZARD_TEMPLATE` (Tactical Wit +INT-mod) on the
/// `ABILITY_MOD_INITIATIVE_BONUSES` cohort — four class chassis
/// converge on the same "passive initiative bump keyed off a subclass
/// tag" identity from different angles (CHA / WIS / INT ability mods
/// there, proficiency bonus here).
///
/// Stacks additively with the sibling Remarkable Athlete row (Champion
/// Fighter lv7, half-prof) on the same `PROFICIENCY_INITIATIVE_BONUSES`
/// cohort — a hypothetical Champion-Fighter / Watchers-Paladin multi-
/// classer stacks a +3 (full prof) + +1 (half-prof, rounded up) = +4
/// initiative bump at CR 1.5. Distinct from the sibling
/// `INITIATIVE_ADVANTAGE_SOURCES` cohort (Feral Instinct, Vigilant
/// Blessing) which flips the roll SHAPE to advantage — the two cohorts
/// stack cleanly: a hypothetical Watchers-Paladin / Twilight-Cleric
/// multiclass would roll 2d20 keep-high AND stack the prof bonus on
/// top.
///
/// RAW's Oath of the Watchers picks up other features not shipped on
/// this template — **Watcher's Will** (lv3 Channel Divinity: grant
/// allies within 30ft advantage on INT / WIS / CHA saves for 1 minute;
/// needs a per-save-check ally scan surface), **Abjure the Extraplanar**
/// (lv3 CD: 30ft WIS-save Turned on Aberrations / Celestials / Elementals
/// / Fey / Fiends; needs a creature-type-gated turn surface),
/// **Vigilant Rebuke** (lv15: reaction to grant +CHA-mod damage on a
/// creature that forced an ally within 30ft to make an INT / WIS / CHA
/// save; needs a save-time reaction hook plus counter-damage), and
/// **Mortal Bulwark** (lv20 capstone: 1-minute self-buff granting truesight,
/// advantage vs. Aberrations / Celestials / Elementals / Fey / Fiends,
/// and forced-banish on hit; complex multi-effect self-buff). Only the
/// lv7 Aura of the Sentinel passive has a mechanical surface on the
/// CR-1.5 chassis that plugs cleanly into the shared
/// `PROFICIENCY_INITIATIVE_BONUSES` cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `GLORY_PALADIN_TEMPLATE` ships only the lv7 Aura of Alacrity
/// passive half of its RAW Oath of Glory kit and
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit.
///
/// Ships on the CR-1.5 paladin chassis at (or above) its strict RAW
/// lv7 gate for the same reason `GLORY_PALADIN_TEMPLATE` ships Aura of
/// Alacrity (RAW lv7), `WAR_MAGIC_WIZARD_TEMPLATE` ships Tactical Wit
/// (RAW lv2), and `TWILIGHT_CLERIC_TEMPLATE` ships Vigilant Blessing
/// (RAW lv1) — class templates target a balanced playable level, not
/// lockstep PHB progression.
///
/// Distinct from `PALADIN_TEMPLATE` (subclass-less baseline) and the
/// Devotion / Ancients / Vengeance / Oathbreaker / Glory cousins so a
/// Watchers-vs-Devotion / vs-Ancients / vs-Vengeance / vs-Oathbreaker /
/// vs-Glory / vs-baseline encounter renders unambiguously by name.
/// Glyph 'H' (for watc**H**ers — 'W' collides with War Cleric on the
/// humanoid roster) — distinct from baseline paladin 'P', Devotion 'D',
/// Ancients 'A', Vengeance 'V', Oathbreaker 'O', and Glory 'Y'.
pub static WATCHERS_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Paladin envelope wholesale
    // and layers on the one Watchers Oath subclass feature
    // (`AURA_OF_THE_SENTINEL_TAG`: passive +prof-bonus initiative-roll
    // bump read at the shared `PROFICIENCY_INITIATIVE_BONUSES` cohort
    // in `actor_template.rs`). The `..base.clone()` tail inside the
    // helper picks up every other field — greatsword + smite suite,
    // half-caster slot ladder, Lay on Hands / Sacred Weapon / Cleansing
    // Touch, Improved Divine Smite passive, Defense / Great Weapon
    // Fighting fighting styles, Aura of Protection / Aura of Courage —
    // without an N-line field-by-field copy. No new actions are pushed
    // — Aura of the Sentinel is a purely passive initiative-roll bump,
    // not a fresh action surface, so the "tag-only" shape the helper
    // wraps is a natural fit. Sibling helper users on the "clone base
    // + insert one tag" cross-class lane: every tag-only Warlock
    // Otherworldly Patron subclass (via `subclass_warlock_template`),
    // `LIFE_CLERIC_TEMPLATE`, `FORGE_CLERIC_TEMPLATE`,
    // `TWILIGHT_CLERIC_TEMPLATE`, `NECROMANCY_WIZARD_TEMPLATE`,
    // `WAR_MAGIC_WIZARD_TEMPLATE`, `SHADOW_MAGIC_SORCERER_TEMPLATE`,
    // `ABERRANT_MIND_SORCERER_TEMPLATE`, `DIVINE_SOUL_SORCERER_TEMPLATE`,
    // `LONG_DEATH_MONK_TEMPLATE`, `GLORY_PALADIN_TEMPLATE`.
    PALADIN_TEMPLATE.with_subclass_tag("Watchers Paladin", 'H', AURA_OF_THE_SENTINEL_TAG)
});

/// Conquest Paladin — Oath of **Conquest** subclass build (XGtE). Three
/// subclass features, and the only paladin oath in the engine whose
/// pieces are useless apart and lethal together:
///
///   - **Conquering Presence** (lv3 Channel Divinity): every hostile
///     within 30 ft rolls a WIS save or is Frightened for 10 rounds.
///     On its own this is Dreadful Aspect with a different name —
///     disadvantage on their attacks, and they walk away.
///
///   - **Aura of Conquest** (lv7): a Frightened creature inside 10 ft
///     can't walk away. Its speed is 0 and it takes 5 psychic at the
///     start of each of its turns. On its own this is nothing at all,
///     because nothing the paladin does frightens anyone.
///
///   - **Scornful Rebuke** (lv15): anything that hits the paladin takes
///     CHA-mod psychic back. Unlike every other retaliation in the
///     engine this one isn't melee-gated — an archer eats it too.
///
/// Put the first two together and the oath stops being a fear build.
/// The presence lands, the aura roots whoever failed inside 10 ft, and
/// the paladin is then standing in the middle of a group of enemies who
/// cannot leave, are rolling at disadvantage, are losing 5 HP a turn,
/// and — via Scornful Rebuke — are paying for every swing they do
/// land. Every other oath on the roster projects *outward*, protecting
/// allies (Devotion, Ancients, Glory, Watchers) or sharpening the
/// paladin's own swing (Oathbreaker, Vengeance). Conquest is the only
/// one that makes standing next to the paladin the mistake.
///
/// It is also the only *hostile* aura in the engine, which is why
/// `EncounterInstance::aura_emitters` grew a team-side parameter: the
/// five ally-facing paladin auras all read their emitters through a
/// helper that filters to the subject's own team, and Conquest needed
/// exactly that walk with the comparison flipped — including the
/// clauses that are easy to forget, like an unconscious paladin's aura
/// going dark.
///
/// RAW's remaining Conquest features aren't shipped: the oath spell
/// list (Armor of Agathys, Command, Hold Person, Spiritual Weapon,
/// Bestow Curse, Fear, Dominate Beast, Stoneskin, Cloudkill, Dominate
/// Person) is a spells-known change rather than a mechanical one, and
/// **Invincible Conqueror** (lv20 capstone: resistance to all damage,
/// an extra attack, crits on 19-20 for one minute) is a level-20
/// capstone on a level-3-to-10 chassis.
///
/// Glyph 'Q' — for the **Q** in Conquest. Distinct from baseline
/// paladin 'P', Devotion 'D', Ancients 'A', Vengeance 'V', Oathbreaker
/// 'O', Glory 'Y' and Watchers 'H'.
pub static CONQUEST_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{CONQUERING_PRESENCE, CONQUERING_PRESENCE_TAG};
    // Not the tag-only `with_subclass_tag` clone the Glory and Watchers
    // oaths use: Conquering Presence is an action, so the action list
    // has to grow. The two passives ride template flags rather than
    // tags — both are read by engine chokepoints that take an
    // `&ActorInstance` (the aura walker, the reflect table) rather than
    // by an action's validator, which is the same split every other
    // always-on paladin aura already sits on.
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    actions.push(&*CONQUERING_PRESENCE);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(CONQUERING_PRESENCE_TAG);
    CreatureTemplate {
        name: "Conquest Paladin",
        glyph: 'Q',
        has_aura_of_conquest: true,
        has_scornful_rebuke: true,
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});

/// Crown Paladin — **Oath of the Crown** (SCAG), and the roster's first
/// pure bodyguard. Every other paladin here is a striker with a defensive
/// aura attached; this one is a defensive aura with a sword attached.
///
///   - **Divine Allegiance** (lv7) is the headline, and it is a lane the
///     engine did not have. When anything adjacent takes damage, the
///     paladin can spend their reaction to take it instead — not reduce
///     it, not copy it, *take* it. It hangs off `DealDamage` rather than
///     off an attack, so unlike Interception or Warding Maneuver it also
///     catches a failed save against a fireball, a poison drip at
///     round end, or a death burst. See
///     `EncounterInstance::claim_divine_allegiance`.
///
///   - **Champion Challenge** (lv3, Channel Divinity): everything hostile
///     within 30 ft rolls WIS or is held where it stands until its next
///     turn. RAW is a 30-ft leash rather than a hold; the engine has no
///     leash and does have `Rooted`, so this trades RAW's minute-long
///     radius for one round of a harder lock.
///
///   - **Turn the Tide** (lv3, Channel Divinity): every ally within
///     30 ft at or below half HP regains `1d6 + CHA`. Small, flat, and
///     only reaches the badly hurt — which is what keeps it from
///     outclassing Lay on Hands rather than duplicating it.
///
/// The three compose into a shape no other paladin has. Champion
/// Challenge pins the enemy line where it is, Divine Allegiance means
/// the ally it was about to reach takes nothing when it swings anyway,
/// and Turn the Tide puts back what the paladin absorbed doing it. The
/// cost is entirely paid in the paladin's own hit points: Divine
/// Allegiance is free, unlimited, and lands every point of it on a
/// d10-hit-die body in plate. A Crown Paladin who guards well spends the
/// fight at half HP, which is exactly where Turn the Tide reaches — and
/// they cannot heal themselves with it and still be standing where their
/// allies need them.
///
/// Left out: **Unyielding Spirit** (lv15) is advantage on saves against
/// paralysis and stunning, which needs a per-condition save-mode filter
/// the blanket save-mode lanes don't express; **Exalted Champion**
/// (lv20) sits above this chassis's level.
///
/// Glyph 'W' — for cro**W**n. Distinct from baseline paladin 'P',
/// Devotion 'D', Ancients 'A', Vengeance 'V', Oathbreaker 'O', Glory
/// 'Y', Watchers 'H' and Conquest 'Q'.
pub static CROWN_PALADIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        CHAMPION_CHALLENGE, CHAMPION_CHALLENGE_TAG, DIVINE_ALLEGIANCE_TAG, TURN_THE_TIDE,
        TURN_THE_TIDE_TAG,
    };
    let mut actions = PALADIN_TEMPLATE.actions.clone();
    actions.push(&*CHAMPION_CHALLENGE);
    actions.push(&*TURN_THE_TIDE);
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(CHAMPION_CHALLENGE_TAG);
    features.insert(TURN_THE_TIDE_TAG);
    // Divine Allegiance is a tag rather than a template flag because the
    // engine reads it through `has_passive_feature` at the damage
    // chokepoint, not through an `&ActorInstance` accessor the way the
    // always-on auras are read.
    features.insert(DIVINE_ALLEGIANCE_TAG);
    CreatureTemplate {
        name: "Crown Paladin",
        glyph: 'W',
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});
