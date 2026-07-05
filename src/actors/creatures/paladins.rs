use crate::actions::class_features::{
    CLEANSING_TOUCH, CLEANSING_TOUCH_TAG, DIVINE_SMITE, IMPROVED_DIVINE_SMITE_TAG, LAY_ON_HANDS,
    LAY_ON_HANDS_TAG, SACRED_WEAPON, SACRED_WEAPON_TAG, UNDYING_SENTINEL_TAG, VOW_OF_ENMITY,
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
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
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
    CreatureTemplate {
        name: "Devotion Paladin",
        glyph: 'D',
        // Aura of Devotion is the entire subclass surface here — a
        // passive template flag rather than an added action, so the
        // subclass-of pattern collapses to name + glyph + the aura flag.
        has_aura_of_devotion: true,
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
    // Two subclass features layer onto the baseline paladin envelope:
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
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(UNDYING_SENTINEL_TAG);
    CreatureTemplate {
        name: "Ancients Paladin",
        glyph: 'A',
        // Nature's Ward — passive template flag, always on.
        has_natures_ward: true,
        // Undying Sentinel — once-per-long-rest cheat-death. Layered
        // onto the inherited paladin feature set (Lay on Hands,
        // Sacred Weapon, Cleansing Touch, Improved Divine Smite)
        // rather than clobbering it.
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
    let mut features = PALADIN_TEMPLATE.features.clone();
    features.insert(VOW_OF_ENMITY_TAG);
    CreatureTemplate {
        name: "Vengeance Paladin",
        glyph: 'V',
        actions,
        features,
        ..PALADIN_TEMPLATE.clone()
    }
});
