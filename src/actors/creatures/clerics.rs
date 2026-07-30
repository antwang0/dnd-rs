use crate::actions::class_features::{
    ARCANE_ABJURATION, ARCANE_ABJURATION_TAG, CIRCLE_OF_MORTALITY_TAG, DAMPEN_ELEMENTS_TAG,
    DESTROY_UNDEAD_TAG, DISCIPLE_OF_LIFE_TAG, DIVINE_STRIKE,
    DIVINE_STRIKE_TAG, GUIDED_STRIKE, GUIDED_STRIKE_TAG, PATH_TO_THE_GRAVE, PATH_TO_THE_GRAVE_TAG,
    PRESERVE_LIFE, PRESERVE_LIFE_TAG, RADIANCE_OF_THE_DAWN, RADIANCE_OF_THE_DAWN_TAG,
    SOUL_OF_THE_FORGE_TAG, TURN_UNDEAD, TURN_UNDEAD_TAG, VIGILANT_BLESSING_TAG, WAR_PRIEST,
    WAR_PRIEST_TAG, WARDING_FLARE_TAG, WRATH_OF_THE_STORM, WRATH_OF_THE_STORM_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    AID, ANIMATE_DEAD, AURA_OF_VITALITY, BANE, BEACON_OF_HOPE, BESTOW_CURSE, BLESS, CALM_EMOTIONS,
    COMMAND, COUNTERSPELL, CROWN_OF_STARS, CRUSADERS_MANTLE, CURE_WOUNDS, DAYLIGHT, DEATH_WARD,
    DISPEL_MAGIC, DIVINE_FAVOR, EARTHQUAKE, FAERIE_FIRE, FEAR, FLAME_STRIKE, FLY,
    GREATER_RESTORATION, GUIDING_BOLT, HASTE, HEAL_SPELL_HIGH, HEALING_SPIRIT, HEALING_WORD,
    HEROES_FEAST, HEROISM, HOLD_PERSON, HOLY_AURA, HOLY_WORD, INFLICT_WOUNDS, INSECT_PLAGUE,
    LESSER_RESTORATION, MASS_CURE_WOUNDS, MASS_HEAL, MASS_HEALING_WORD, PLANT_GROWTH,
    POWER_WORD_HEAL, PRAYER_OF_HEALING, PROTECTION_FROM_EVIL_AND_GOOD, RESURRECTION, REVIVIFY,
    SACRED_BURST, SACRED_FLAME, SANCTUARY, SHIELD_OF_FAITH, SPARE_THE_DYING, SPIKE_GROWTH,
    SPIRIT_GUARDIANS, SPIRIT_SHROUD, SPIRITUAL_WEAPON, STONESKIN, SUGGESTION, SUNBEAM, SUNBURST,
    THORN_WHIP, TOLL_THE_DEAD, TRUE_RESURRECTION, WISH, WORD_OF_RADIANCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Acolyte-style spellcaster. WIS-primary; Sacred Flame as the staple
/// damage option, Healing Word and Cure Wounds for support, Bless for
/// pre-buff, Hold Person for lockdown. Modeled to be roughly equivalent
/// to MM Acolyte (CR 1/4) — light HP, medium AC, no melee.
pub static CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
    actions.push(&HEALING_WORD);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*BLESS);
    actions.push(&*GUIDING_BOLT);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*BANE);
    actions.push(&*SPIRITUAL_WEAPON);
    actions.push(&*AID);
    actions.push(&*INFLICT_WOUNDS);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*THORN_WHIP);
    actions.push(&*SPARE_THE_DYING);
    actions.push(&*TOLL_THE_DEAD);
    actions.push(&*HEROISM);
    actions.push(&*MASS_HEALING_WORD);
    actions.push(&*PROTECTION_FROM_EVIL_AND_GOOD);
    actions.push(&*COMMAND);
    actions.push(&*DIVINE_FAVOR);
    actions.push(&*SPIRIT_GUARDIANS);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*MASS_CURE_WOUNDS);
    actions.push(&*HASTE);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*DEATH_WARD);
    actions.push(&*REVIVIFY);
    actions.push(&*BEACON_OF_HOPE);
    actions.push(&*STONESKIN);
    actions.push(&*HEAL_SPELL_HIGH);
    actions.push(&*WORD_OF_RADIANCE);
    actions.push(&*CALM_EMOTIONS);
    actions.push(&*SUGGESTION);
    actions.push(&*SUNBURST);
    actions.push(&*MASS_HEAL);
    actions.push(&*PRAYER_OF_HEALING);
    actions.push(&*SUNBEAM);
    actions.push(&*RESURRECTION);
    actions.push(&*POWER_WORD_HEAL);
    // Newly added spells (lv1 Sanctuary; lv2 Healing Spirit; lv3 Daylight
    // and Aura of Vitality; lv5 Insect Plague; lv9 True Resurrection).
    actions.push(&*SANCTUARY);
    actions.push(&*HEALING_SPIRIT);
    actions.push(&*DAYLIGHT);
    actions.push(&*AURA_OF_VITALITY);
    actions.push(&*INSECT_PLAGUE);
    actions.push(&*TRUE_RESURRECTION);
    // Druid-flavored Spike Growth (lv2 control) plus Counterspell at
    // lv3 (anti-caster reaction approximation).
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*COUNTERSPELL);
    // Newest divine additions: lv3 Crusader's Mantle (+1d4 radiant per
    // hit aura buff), lv7 Crown of Stars (+1d8 radiant per hit halo),
    // lv8 Earthquake (STR save AoE prone), lv9 Wish (mass-heal allies).
    actions.push(&*CRUSADERS_MANTLE);
    actions.push(&*CROWN_OF_STARS);
    actions.push(&*EARTHQUAKE);
    actions.push(&*WISH);
    // Latest divine additions: lv3 Fear (cone WIS save → Frightened) and
    // lv5 Greater Restoration (cleanse + heal). Both are workhorse
    // mid-level utilities the cleric was missing.
    actions.push(&*FEAR);
    actions.push(&*GREATER_RESTORATION);
    // Flame Strike — lv5 mixed-damage AoE (fire + radiant). Slips past
    // fire-resistant fiends (radiant lands) and radiant-resistant
    // celestials (fire lands).
    actions.push(&*FLAME_STRIKE);
    // Turn Undead — Cleric Channel Divinity, once per long rest.
    actions.push(&*TURN_UNDEAD);
    // Divine Strike — Cleric class feature (RAW: passive at level 8; we
    // model it as a once-per-rest bonus-action prime that lands +1d8
    // radiant on the next melee hit via the OnHitRider table). Pairs
    // well with the cleric's melee cantrip (Thorn Whip) and weapon
    // attacks for the rare hit-and-spike moment.
    actions.push(&*DIVINE_STRIKE);
    // Latest cross-school additions: lv3 Animate Dead (necromancy ally
    // spawn) + Spirit Shroud (concentration on-hit cold rider), and the
    // lv8 Holy Aura (concentration save-advantage burst aura).
    actions.push(&*ANIMATE_DEAD);
    actions.push(&*SPIRIT_SHROUD);
    actions.push(&*HOLY_AURA);
    // Latest druidic-flavored additions for the cleric kit: lv3 Plant
    // Growth (Entangle AoE) and lv3 Fly (concentration ally speed buff).
    actions.push(&*PLANT_GROWTH);
    actions.push(&*FLY);
    // Level-6 apex pre-fight buff: ally-burst temp HP + heal + Heroic.
    // Costs the cleric's only level-6 slot, so it's a one-off opener.
    actions.push(&*HEROES_FEAST);
    // Level-7 apex anti-enemy radiant burst with HP-tiered conditions.
    // Pairs with Resurrection for the cleric's level-7 slot economy.
    actions.push(&*HOLY_WORD);
    // Latest cleric additions: lv4 Guardian of Faith (radiant burst,
    // flat-20 / save-half) and lv6 Blade Barrier (concentration slashing
    // burst). Both round out the cleric's high-tier damage lane with
    // mid-cost AoE options between Flame Strike (lv5) and Sunburst (lv8).
    actions.push(&*crate::actions::spells::GUARDIAN_OF_FAITH);
    actions.push(&*crate::actions::spells::BLADE_BARRIER);
    // Guidance — cleric / druid divination cantrip. Touch range; applies
    // the Inspired flat-buff (+3 to next attack / save / check) on the
    // target. Fills the "pre-fight ally prime" cantrip lane that was
    // previously empty for clerics. Custom-validate gates against
    // re-priming an already-inspired ally.
    actions.push(&*crate::actions::spells::GUIDANCE);
    // Warding Bond — lv2 abjuration. Touch-range damage-share bond:
    // bonded ally gains +1 AC, +1 saves, resistance to all damage; the
    // caster takes the same (post-resistance) damage every time the
    // ally is hit. Pairs the cleric's defensive lane with a damage-
    // sink role — a frontline fighter behind the bond effectively gets
    // 50% damage reduction while the cleric pays the other 50%.
    actions.push(&*crate::actions::spells::WARDING_BOND);
    // Latest divine additions:
    //   - lv6 **Harm**: 14d6 necrotic single-target CON-save for half
    //     plus max-HP drain on fail (cleric's signature offensive nuke,
    //     opposite of Heal in the lv6 slot lane).
    //   - lv6 **Circle of Death**: 8d6 necrotic 30ft-radius CON-save
    //     burst (mass damage that pairs cleanly against undead
    //     necrotic-resistant enemies via the necrotic-immunity routing).
    //   - lv7 **Regenerate**: 4d8+15 touch heal (high-burst single-
    //     target heal, fills the lv7 slot lane next to Resurrection).
    actions.push(&*crate::actions::spells::HARM);
    actions.push(&*crate::actions::spells::CIRCLE_OF_DEATH);
    actions.push(&*crate::actions::spells::REGENERATE);
    actions.push(&*crate::actions::spells::PROTECTION_FROM_ENERGY);
    actions.push(&*crate::actions::spells::REMOVE_CURSE);
    actions.push(&*crate::actions::spells::ANTILIFE_SHELL);
    // Latest cleric additions:
    //   - lv2 **Enhance Ability** (transmutation): touch single-target
    //     buff — 2d6 temp HP + flat +2 saves for the duration
    //     (concentration). Slots cleanly into the cleric's support lane
    //     alongside Bless / Heroism / Aid; the single-target temp HP
    //     differentiates it from Bless's burst attack-roll buff.
    //   - lv5 **Contagion** (necromancy): single-target touch CON-save
    //     vs the cleric's spell DC; on fail target picks up Poisoned for
    //     10 rounds. Slots between Bestow Curse (lv3 WIS-save) and Hold
    //     Monster (lv5 WIS-save) on the single-target lockdown ladder —
    //     a CON-save lane bites a different stat profile.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    actions.push(&*crate::actions::spells::CONTAGION);
    // Latest cleric utility additions:
    //   - lv2 **Silence**: 20ft sphere of magical silence. Locks down
    //     enemy spellcasters caught in the burst (verbal-component proxy
    //     via the SpellSlot gate) and grants thunder-damage immunity to
    //     everyone inside. The cleric's anti-caster zone option.
    //   - lv4 **Freedom of Movement**: ally-buff that grants dynamic
    //     immunity to Paralyzed / Restrained / Grappled AND strips any
    //     active install of those three. Pairs with the cleric's
    //     frontline-support kit alongside Aid / Aura of Purity.
    //   - lv5 **Raise Dead**: touch revive a dying ally to 1 HP. Slots
    //     between Revivify (lv3) and Resurrection (lv7) — same touch
    //     envelope, higher slot, no cleanse rider.
    actions.push(&*crate::actions::spells::SILENCE);
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    actions.push(&*crate::actions::spells::RAISE_DEAD);
    // Latest cleric support additions:
    //   - lv2 **Protection from Poison** (abjuration): touch cleanse of
    //     `Poisoned` + `Purified` install for 1 hour. Sibling to Lesser
    //     Restoration (lv2 single-condition cleanse) on the lv2 support
    //     lane — focused on the poison lane with a lingering buff.
    //   - lv6 **True Seeing** (divination): touch ally buff that
    //     suppresses the invisibility / illusion attack-mode penalties
    //     on the holder. Fills the cleric's lv6 utility slot next to
    //     Heal / Word of Recall / Sunbeam — defensive enabler against
    //     illusionist / invisible-stalker opponents.
    actions.push(&*crate::actions::spells::PROTECTION_FROM_POISON);
    actions.push(&*crate::actions::spells::TRUE_SEEING);
    // Preserve Life — Cleric Channel Divinity (Life Domain in RAW; we
    // expose it generically here). Once per short rest pool of 5 × level
    // HP, healing the most-wounded allies first up to half max HP each.
    // Mass-stabilizer to balance the cleric's offensive Channel Divinity
    // (Turn Undead) — the same action-economy slot, different lane.
    actions.push(&*PRESERVE_LIFE);
    CreatureTemplate {
        name: "Cleric",
        glyph: 'C',
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        strength: 10,
        dexterity: 10,
        constitution: 12,
        intelligence: 10,
        wisdom: 14, // primary spellcasting ability
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 4/3/3/2/2/1/1/1/3 — cleric loadout extended to support the
        // full SRD spell list now in their kit. Level-3 slot covers
        // Mass Healing Word / Spirit Guardians / Beacon of Hope /
        // Haste / Daylight / Aura of Vitality; level-4 slot covers
        // Stoneskin / Death Ward; level-5 covers Mass Cure Wounds /
        // Insect Plague; level-6 fuels one Heal or one Sunbeam
        // (concentration — only one at a time anyway); the new
        // level-7 slot powers exactly one Resurrection; the level-8
        // slot powers a single Sunburst; and the level-9 row jumps to
        // 3 so Mass Heal, Power Word Heal, and True Resurrection can
        // each fire once per long rest.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 3],
        // Clerics are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        // Destroy Undead (RAW cleric lv5 passive) ships on the baseline
        // cleric so every subclass (War / Light / Tempest / Devotion /
        // future) inherits the "failed Turn Undead save on a low-CR
        // undead destroys instead of Frightens" branch without each
        // subclass template needing to opt in. Ships on the CR-0.25
        // template above its strict RAW lv5 gate for the same reason
        // Preserve Life (lv2) and Divine Strike (lv8) do — class
        // templates target a balanced playable level, not lockstep PHB
        // progression. Sibling to `TURN_UNDEAD_TAG` (the per-rest
        // charge that gates the action's availability) — Destroy
        // Undead is the always-on passive that piggy-backs on Turn
        // Undead's failed saves.
        features: HashSet::from([
            TURN_UNDEAD_TAG,
            DIVINE_STRIKE_TAG,
            PRESERVE_LIFE_TAG,
            DESTROY_UNDEAD_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// War Domain Cleric — subclass build. Identical envelope to the baseline
/// `CLERIC_TEMPLATE` (WIS-primary caster, Sacred Flame / Guiding Bolt /
/// Cure Wounds / Bless / Turn Undead / Preserve Life / Divine Strike,
/// full cleric spell ladder) with two subclass features layered on:
///
/// - **War Priest** (lv1 subclass): bonus-action extra weapon swing after
///   the Attack action. Once per short rest. Same action-economy trade
///   as Flurry of Blows / Frenzy / Action Surge — spend a bonus action
///   to buy a follow-up Action. Pairs naturally with the cleric's melee
///   fallback (Thorn Whip cantrip / Spiritual Weapon primary swing) or
///   any weapon added to the template (an added mace, spear, etc.).
/// - **Guided Strike** (lv2 Channel Divinity): bonus-action prime that
///   installs `GuidedStriking` on the cleric for +10 to their next
///   attack roll. Once per short rest. The largest single-swing
///   accuracy buff in the game — turns a marginal near-miss into a
///   guaranteed connect. Pairs with the cleric's ranged offense
///   (Sacred Flame DEX save has no attack roll — no benefit; but
///   Guiding Bolt IS an attack roll — Guided Strike + Guiding Bolt
///   guarantees the +4d6 radiant lands AND applies the Outlined
///   condition for the follow-up attacker advantage rider).
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) so a
/// War-vs-baseline encounter renders unambiguously by name. Glyph 'W'
/// so the War Cleric shows up distinctly on the map next to the
/// baseline 'C'.
pub static WAR_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Cleric envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // actions / features). Mirrors HUNTER_RANGER_TEMPLATE /
    // ASSASSIN_ROGUE_TEMPLATE / VENGEANCE_PALADIN_TEMPLATE shape —
    // the `..base.clone()` tail picks up every other field (stats,
    // slots, save profs) without an N-line field-by-field copy.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    actions.push(&*WAR_PRIEST);
    actions.push(&*GUIDED_STRIKE);
    let mut features = CLERIC_TEMPLATE.features.clone();
    features.insert(WAR_PRIEST_TAG);
    features.insert(GUIDED_STRIKE_TAG);
    CreatureTemplate {
        name: "War Cleric",
        glyph: 'W',
        actions,
        features,
        ..CLERIC_TEMPLATE.clone()
    }
});

/// Light Domain Cleric — subclass build. Identical envelope to the
/// baseline `CLERIC_TEMPLATE` (WIS-primary caster, Sacred Flame / Guiding
/// Bolt / Cure Wounds / Bless / Turn Undead / Preserve Life / Divine
/// Strike, full cleric spell ladder) with two subclass features layered
/// on:
///
/// - **Warding Flare** (lv1 subclass): passive reaction that imposes
///   disadvantage on an incoming attack from within 30 ft. Fires at
///   the attack-mode chokepoint (weapon + spell) — no action surface,
///   once per short rest (RAW: WIS-mod uses per long rest; collapsed).
/// - **Radiance of the Dawn** (lv2 Channel Divinity) — action-cost
///   30ft self-centered radiant burst, once per short rest. Every
///   enemy in range makes a CON save vs the cleric's spell save DC;
///   on fail they eat `2d10 + cleric level` radiant, on save they
///   take half.
///
/// The Light Domain's damage-lane sibling to Turn Undead (Frightened
/// install, undead-only) and Preserve Life (mass heal). Where Turn
/// Undead is single-type and Preserve Life is friend-only, Radiance of
/// the Dawn is undiscriminating enemy damage — the "burst of dawn"
/// signature the RAW's Light Domain gets three levels earlier than the
/// baseline cleric's other AoE (Spirit Guardians at level 5, Flame
/// Strike at level 9).
///
/// Pairs naturally with the cleric's radiant single-target lane —
/// Guiding Bolt (4d6 radiant) at range, Sacred Flame (1d8 radiant) as
/// a cantrip fallback, and now Radiance of the Dawn (2d10 + level
/// radiant) as the burst. The Light cleric leans hardest into the
/// radiant-type advantage against fiends / undead (both commonly
/// vulnerable or non-resistant to radiant).
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) and
/// `WAR_CLERIC_TEMPLATE` (War Domain subclass) so a Light-vs-War-vs-
/// baseline encounter renders unambiguously by name. Glyph 'L' so the
/// Light Cleric shows up distinctly on the map next to the baseline
/// 'C' and War Cleric 'W'.
pub static LIGHT_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: mirrors the War Cleric shape above — clone
    // the baseline Cleric envelope wholesale and layer on the subclass
    // action + feature tag. The `..CLERIC_TEMPLATE.clone()` tail picks
    // up the full spell ladder, save profs, stats, and slots without
    // an N-line field-by-field copy.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    actions.push(&*RADIANCE_OF_THE_DAWN);
    let mut features = CLERIC_TEMPLATE.features.clone();
    features.insert(RADIANCE_OF_THE_DAWN_TAG);
    // Warding Flare (lv1 Light subclass): passive reaction that imposes
    // disadvantage on an incoming attack from within 30 ft. No action
    // surface — the trigger fires automatically at every attack
    // chokepoint (weapon + spell). Registered in `SHORT_REST_FEATURES`
    // so the charge refreshes alongside Radiance of the Dawn.
    features.insert(WARDING_FLARE_TAG);
    CreatureTemplate {
        name: "Light Cleric",
        glyph: 'L',
        actions,
        features,
        ..CLERIC_TEMPLATE.clone()
    }
});

/// Tempest Domain Cleric — subclass build. Identical envelope to the
/// baseline `CLERIC_TEMPLATE` (WIS-primary caster, Sacred Flame / Guiding
/// Bolt / Cure Wounds / Bless / Turn Undead / Preserve Life / Divine
/// Strike, full cleric spell ladder) with one subclass feature layered
/// on: **Wrath of the Storm** (lv1 subclass) — once-per-short-rest
/// single-target 5ft-close 2d8 lightning damage burst on a DEX save vs
/// the cleric's WIS-anchored spell save DC.
///
/// The Tempest Domain's melee-retaliation-lane sibling to the Light
/// Domain (Warding Flare + Radiance of the Dawn — disadvantage
/// reaction plus 30ft radiant burst) and the War Domain (War Priest +
/// Guided Strike — extra swing plus +10 accuracy prime). Where Light
/// leans radiant burst and War leans melee accuracy, Tempest leans
/// lightning close-range zaps: the cleric's adjacent-target damage
/// lane that doesn't burn a spell slot, mirroring the RAW Tempest
/// flavor (the cleric who calls down thunder and lightning on any foe
/// foolish enough to close within 5 ft).
///
/// Pairs naturally with the cleric's melee cantrip (Thorn Whip) and
/// weapon fallback (`SCIMITAR` if added later) — the Tempest cleric
/// wants an enemy adjacent to fire Wrath of the Storm, so closing the
/// gap sets up both the retaliation zap AND the follow-up cantrip /
/// weapon swing on the same turn. Distinct from `WAR_CLERIC_TEMPLATE`
/// (War Priest bonus-action swing + Guided Strike accuracy) and
/// `LIGHT_CLERIC_TEMPLATE` (Warding Flare + Radiance of the Dawn) —
/// glyph 'S' (Storm) so tempest-vs-war-vs-light-vs-baseline renders
/// unambiguously on the map next to baseline 'C', War 'W', and Light
/// 'L'.
pub static TEMPEST_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: mirrors the War Cleric / Light Cleric shape
    // above — clone the baseline Cleric envelope wholesale and layer on
    // the subclass action + feature tag. The `..CLERIC_TEMPLATE.clone()`
    // tail picks up the full spell ladder, save profs, stats, and slots
    // without an N-line field-by-field copy.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    actions.push(&*WRATH_OF_THE_STORM);
    let mut features = CLERIC_TEMPLATE.features.clone();
    // Wrath of the Storm charge — once per short rest, refreshed via
    // `SHORT_REST_FEATURES` alongside the War / Light CDs.
    features.insert(WRATH_OF_THE_STORM_TAG);
    CreatureTemplate {
        name: "Tempest Cleric",
        glyph: 'S',
        actions,
        features,
        ..CLERIC_TEMPLATE.clone()
    }
});

/// Life Domain Cleric — subclass build. Identical envelope to the
/// baseline `CLERIC_TEMPLATE` (WIS-primary caster, Sacred Flame /
/// Guiding Bolt / Cure Wounds / Bless / Turn Undead / Preserve Life /
/// Divine Strike, full cleric spell ladder) with one subclass feature
/// layered on: **Disciple of Life** (lv1 subclass passive) — every
/// leveled heal the cleric casts pours an extra `2 + slot_level` HP
/// into each target (RAW: any spell of 1st level or higher that
/// restores hit points; the bonus lands per creature per cast, not per
/// die).
///
/// The Life Domain's support-flavored sibling to Light (radiant burst
/// + Warding Flare), War (extra weapon swing + accuracy prime), and
/// Tempest (close-range lightning zap). Where the other domain
/// subclasses hand the cleric a fresh offensive lever, Life leans into
/// the healing lane the baseline cleric already carries and amplifies
/// it in place:
///   - Healing Word (lv1) heals 1d4 + WIS + 3 instead of 1d4 + WIS.
///   - Cure Wounds (lv1) heals 1d8 + WIS + 3 instead of 1d8 + WIS.
///   - Mass Healing Word (lv3) heals 1d4 + WIS + 5 *each* to up to 6
///     allies instead of the flat 1d4 + WIS baseline.
///   - Mass Cure Wounds (lv5) heals 3d8 + WIS + 7 *each* to up to 6
///     allies inside the burst.
/// Composes cleanly with Preserve Life (Channel Divinity) already on
/// the baseline template — the Life Cleric's turn-1 opener is Preserve
/// Life for the mass-stabilize pool, then Mass Healing Word (bonus
/// action) for the +5-per-ally follow-up. The two features between
/// them make the Life Cleric the strongest healer in the party at any
/// given round.
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) and the
/// War / Light / Tempest cousins so a Life-vs-Baseline / vs-War /
/// vs-Light / vs-Tempest encounter renders unambiguously by name. Glyph
/// 'V' (for Vita) so the Life Cleric shows up distinctly on the map
/// next to baseline 'C', War 'W', Light 'L', and Tempest 'S'.
pub static LIFE_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Cleric envelope wholesale
    // and layers on the Disciple of Life passive tag. The `..base.clone()`
    // tail inside the helper picks up the full spell ladder, save profs,
    // stats, and slots without an N-line field-by-field copy. No new
    // actions are pushed — Disciple of Life is a purely passive amplifier
    // read at the leveled-heal chokepoints (HealSpell / CureWounds /
    // MassHealingWord / MassCureWounds), not a fresh action surface, so
    // the "tag-only" shape the helper wraps is a natural fit. Sibling
    // helper users on the "clone base + insert one tag" cross-class lane:
    // every tag-only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `NECROMANCY_WIZARD_TEMPLATE`,
    // `ABERRANT_MIND_SORCERER_TEMPLATE`, `DIVINE_SOUL_SORCERER_TEMPLATE`.
    // WAR / LIGHT / TEMPEST cleric subclasses layer actions plus their
    // subclass tags so they stay on the explicit clone-and-insert body.
    CLERIC_TEMPLATE.with_subclass_tag("Life Cleric", 'V', DISCIPLE_OF_LIFE_TAG)
});

/// Grave Domain Cleric — subclass build (XGtE). Identical envelope to
/// the baseline `CLERIC_TEMPLATE` (WIS-primary caster, Sacred Flame /
/// Guiding Bolt / Cure Wounds / Bless / Turn Undead / Preserve Life /
/// Divine Strike, full cleric spell ladder) with one subclass Channel
/// Divinity feature layered on: **Path to the Grave** (lv2 subclass)
/// — action-cost single-target curse (`MarkedForGrave`) that grants
/// advantage to the next attack against the cursed target, once per
/// short rest.
///
/// The Grave Domain's setup-flavored sibling to the offensive
/// Channel Divinities on the other Cleric subclasses:
///   - **War** (Guided Strike): CASTER-side self-prime +10 accuracy.
///   - **Light** (Radiance of the Dawn): 30ft radiant burst.
///   - **Tempest** (Wrath of the Storm): 5ft reactive lightning zap.
///   - **Life** (Disciple of Life): amplify healing.
///   - **Grave** (Path to the Grave): TARGET-side curse — advantage
///     on the next incoming attack against the marked target.
///
/// Where War's Guided Strike helps only the cleric's own next swing,
/// Path to the Grave helps the WHOLE PARTY's next attack — the
/// cleric's teammates get advantage against the cursed target too.
/// Pairs naturally with the Cleric's radiant single-target lane
/// (Sacred Flame / Guiding Bolt) and with any melee striker on the
/// same team: the cleric marks the boss with CD, the party's fighter
/// / paladin / rogue drops advantage on the follow-up swing.
///
/// RAW's Path to the Grave clause has a second half — the marked
/// target has **vulnerability** (double damage) on the attack that
/// consumes the curse. We ship the advantage-on-next-attack half
/// (the accuracy multiplier) as the load-bearing tactical effect;
/// the vulnerability half needs a target-side incoming-damage
/// multiplier hook that today's engine doesn't expose as a first-
/// class surface, and would slot in later as a `MarkedForGrave`-
/// gated damage multiplier at the `effective_damage` chokepoint —
/// matching the way `CUTTING_WORDS_TAG` ships only the disadvantage
/// half of RAW's "-die on attack / ability / damage" shape.
///
/// The Grave Domain's `CIRCLE_OF_MORTALITY_TAG` (lv1) rides alongside
/// the Channel Divinity — passive: leveled heal spells cast on a
/// creature at 0 hit points substitute the max face-value of every
/// die (dropping variance from every downed-ally clutch heal). Read
/// at the shared `HealSpell` chokepoint (Cure Wounds / Healing Word)
/// via `should_use_max_heal_dice(caster, target)`; the gate folds
/// "caster has the tag AND target is at 0 HP" so a Grave Cleric
/// healing a healthy ally rolls normally. Sibling to Life Cleric's
/// `DISCIPLE_OF_LIFE_TAG` on the "cleric domain heal-amplifier" lane
/// — Disciple adds a flat `2 + slot_level`, Circle of Mortality
/// replaces the dice roll with its max on the "target at 0 HP" gate.
///
/// RAW's Grave Domain picks up other features not shipped on this
/// template — **Eyes of the Grave** (lv1: per-day passive undead-
/// detection with a divination range; out-of-combat dialog-gate
/// ribbon), **Sentinel at Death's Door** (lv6: reaction to turn a
/// crit vs an ally within 30ft into a normal hit; needs a target-side
/// crit-cancel hook), **Potent Spellcasting** (lv8: +WIS to cantrip
/// damage; needs a per-cantrip damage-bonus hook), and **Keeper of
/// Souls** (lv17 capstone). The two lv1-lv2 always-on features
/// (Circle of Mortality + Path to the Grave) are the load-bearing
/// tactical features with first-class engine surfaces today, so we
/// ship both halves and leave the rest as future work — matching the
/// way the War / Light / Tempest cleric subclass templates each ship
/// their Channel Divinity + one passive rider rather than the full
/// RAW subclass suite.
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) and the
/// War / Light / Tempest / Life cousins so a Grave-vs-Baseline /
/// vs-War / vs-Light / vs-Tempest / vs-Life encounter renders
/// unambiguously by name. Glyph 'G' (for Grave) so the Grave Cleric
/// shows up distinctly on the map next to baseline 'C', War 'W',
/// Light 'L', Tempest 'S', and Life 'V'.
pub static GRAVE_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: mirrors the War Cleric / Light Cleric /
    // Tempest Cleric shape above — clone the baseline Cleric envelope
    // wholesale and layer on the subclass action + feature tag. The
    // `..CLERIC_TEMPLATE.clone()` tail picks up the full spell ladder,
    // save profs, stats, and slots without an N-line field-by-field
    // copy.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    actions.push(&*PATH_TO_THE_GRAVE);
    let mut features = CLERIC_TEMPLATE.features.clone();
    // Path to the Grave charge — once per short rest, refreshed via
    // `SHORT_REST_FEATURES` alongside the War / Light / Tempest CDs.
    features.insert(PATH_TO_THE_GRAVE_TAG);
    // Circle of Mortality — always-on passive read at the shared
    // `HealSpell` chokepoint via `should_use_max_heal_dice(caster,
    // target)`. Not in `SHORT_REST_FEATURES` / long-rest tables —
    // nothing consumes it, so the tag stays on the template's
    // features set permanently. Sibling to `DISCIPLE_OF_LIFE_TAG`
    // on the "cleric domain heal-amplifier" passive lane.
    features.insert(CIRCLE_OF_MORTALITY_TAG);
    CreatureTemplate {
        name: "Grave Cleric",
        glyph: 'G',
        actions,
        features,
        ..CLERIC_TEMPLATE.clone()
    }
});

/// Forge Domain Cleric — Divine Domain **Forge Domain** subclass build
/// (XGtE). Identical envelope to the baseline `CLERIC_TEMPLATE`
/// (WIS-primary caster, Sacred Flame / Guiding Bolt / Cure Wounds /
/// Bless / Turn Undead / Preserve Life / Divine Strike, full cleric
/// spell ladder) with one subclass passive layered on: **Soul of the
/// Forge** (lv6 subclass tell) — passive **resistance to fire damage**.
///
/// The signature "forge-cleric's flesh has been tempered against the
/// flame" tell — where a baseline Cleric eats a Fire Bolt / Scorching
/// Ray / Fireball / Burning Hands clean, the Forge Cleric halves the
/// incoming fire damage. Composes cleanly with the party's front-line
/// (a Forge Cleric can drop a Sunburst / Flame Strike into a huddle of
/// enemies without the reflected splash halving their own HP the way
/// a baseline cleric would).
///
/// The Forge Domain's passive-resistance-flavored sibling to the
/// Channel-Divinity-flavored siblings on the other Cleric subclasses:
///   - **War** (Guided Strike): CASTER-side self-prime +10 accuracy.
///   - **Light** (Radiance of the Dawn): 30ft radiant burst.
///   - **Tempest** (Wrath of the Storm): 5ft reactive lightning zap.
///   - **Life** (Disciple of Life): amplify healing.
///   - **Grave** (Path to the Grave): TARGET-side curse — advantage
///     on the next incoming attack against the marked target.
///   - **Forge** (Soul of the Forge): passive fire resistance —
///     always-on damage halving on the Fire axis.
///
/// Where the War / Light / Tempest / Grave subclasses each ship a
/// Channel Divinity action and Life ships a leveled-heal amplifier,
/// the Forge Domain leans on the pure passive lane — the fire
/// resistance is always on, no charge to spend, no bonus action to
/// prime, no target to pick. Sibling on the "passive typed resistance
/// as the subclass tell" cross-class lane to `INURED_TO_UNDEATH_TAG`
/// (Necromancy Wizard — Necrotic) and the four Genie Elemental Gifts
/// (Marid — Cold, Dao — Bludgeoning, Djinni — Thunder, Efreeti — Fire).
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Fiendish Resilience / Draconic
/// Resilience / Efreeti Elemental Gift / Storm Soul (Desert) — same
/// lane, different subclass flavor, same damage axis (Fire). The four
/// prior Fire-resistance rows (Fiendish / Draconic / Efreeti / Storm
/// Soul Desert) never legally co-occur with Soul of the Forge on a
/// single build (five distinct class-subclass slots), and a
/// hypothetical multiclass carrier caps at a single /2 per Fire hit
/// under the "one halving per damage instance" rule.
///
/// RAW's Forge Domain picks up other features not shipped on this
/// template — **Blessing of the Forge** (lv1: touch a mundane weapon /
/// armor, becomes +1 for 24 hours; needs a per-item bonuses-mutation
/// hook and an out-of-combat "grant on ally" surface), **Channel
/// Divinity: Artisan's Blessing** (lv2: 1-hour crafting ritual; no
/// combat surface), **Divine Strike (Fire)** (lv8: +1d8 fire on
/// weapon hits; would slot in as a per-domain damage-type override on
/// the shared `DivineStrike` bonus-action prime — the existing
/// `DIVINE_STRIKE` action ships a Radiant flavor, so future work
/// would extend the prime to read a per-holder damage-type flag), and
/// **Saint of Forge and Fire** (lv17 capstone: heavy-armor Fire
/// immunity + physical resistance while wearing heavy armor; needs a
/// heavy-armor gate and a full-immunity promotion of the passive
/// resistance). Only the lv6 Soul of the Forge passive has a
/// mechanical surface on the CR-0.5 chassis that plugs cleanly into
/// the shared passive typed-resistance cohort, so we ship that half
/// and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only the lv10 Inured to Undeath
/// passive half of its RAW School of Necromancy kit and the four
/// Genie Warlock templates each ship only the lv6 Elemental Gift
/// resistance half of their RAW Genie patron kit.
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) and the
/// War / Light / Tempest / Life / Grave cousins so a Forge-vs-
/// Baseline / vs-War / vs-Light / vs-Tempest / vs-Life / vs-Grave
/// encounter renders unambiguously by name. Glyph 'F' (for Forge) so
/// the Forge Cleric shows up distinctly on the map next to baseline
/// 'C', War 'W', Light 'L', Tempest 'S', Life 'V', and Grave 'G'.
/// Collides with the Fighter's 'F' glyph, but the two never legally
/// co-occur on a single team (a Forge Cleric isn't a Fighter, and
/// one glyph per team-color-and-team-id combo suffices to
/// disambiguate them in a mixed encounter).
///
/// Ships on the CR-0.5 cleric chassis at (or above) its strict RAW
/// lv6 gate for the same reason `NECROMANCY_WIZARD_TEMPLATE` ships
/// Inured to Undeath (RAW lv10) and every other subclass template
/// runs above its strict RAW gate — class templates target a
/// balanced playable level, not lockstep PHB progression.
pub static FORGE_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Cleric envelope wholesale
    // and layers on the Soul of the Forge passive tag. The
    // `..base.clone()` tail inside the helper picks up every other
    // field — the full cleric spell ladder, save profs, stats, slots,
    // and the TURN_UNDEAD / DIVINE_STRIKE / PRESERVE_LIFE /
    // DESTROY_UNDEAD baseline features — without an N-line
    // field-by-field copy.
    // No new actions are pushed — Soul of the Forge is a purely
    // passive fire-damage halver read at the shared
    // `PASSIVE_TYPED_RESISTANCES` cohort in `effective_damage`, not a
    // fresh action surface, so the "tag-only" shape the helper wraps
    // is a natural fit. Sibling helper users on the "clone base +
    // insert one tag" cross-class lane: every tag-only Warlock
    // Otherworldly Patron subclass (via `subclass_warlock_template`),
    // `LIFE_CLERIC_TEMPLATE`, `NECROMANCY_WIZARD_TEMPLATE`,
    // `SHADOW_MAGIC_SORCERER_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`, `LONG_DEATH_MONK_TEMPLATE`.
    // Glyph 'F' — for the "Forge" identity; distinct from baseline
    // cleric 'C', War 'W', Light 'L', Tempest 'S', Life 'V', and
    // Grave 'G' cousins.
    CLERIC_TEMPLATE.with_subclass_tag("Forge Cleric", 'F', SOUL_OF_THE_FORGE_TAG)
});

/// Twilight Domain Cleric — Divine Domain **Twilight Domain** subclass
/// build (TCE). Identical envelope to the baseline `CLERIC_TEMPLATE`
/// (WIS-primary caster, Sacred Flame / Guiding Bolt / Cure Wounds /
/// Bless / Turn Undead / Preserve Life / Divine Strike, full cleric
/// spell ladder) with one subclass passive layered on: **Vigilant
/// Blessing** (lv1 subclass tell) — passive **advantage on initiative
/// rolls**.
///
/// The signature "twilight-cleric sees the ambush before it arrives"
/// tell — where a baseline Cleric relies on a middling DEX 10 initiative
/// mod, the Twilight Cleric rolls the initiative d20 twice and keeps
/// the higher, reliably opening the round with a pre-fight Bless prime
/// or a Guiding Bolt at a soft target. Composes cleanly with the
/// baseline cleric's high-value opener kit — a Twilight Cleric who wins
/// initiative reliably lands Bless / Bane / Spirit Guardians / Guiding
/// Bolt / Sanctuary before the first enemy swing.
///
/// The Twilight Domain's initiative-flavored sibling to the other
/// Cleric subclasses:
///   - **War** (Guided Strike): CASTER-side self-prime +10 accuracy.
///   - **Light** (Radiance of the Dawn): 30ft radiant burst.
///   - **Tempest** (Wrath of the Storm): 5ft reactive lightning zap.
///   - **Life** (Disciple of Life): amplify healing.
///   - **Grave** (Path to the Grave): TARGET-side curse — advantage
///     on the next incoming attack against the marked target.
///   - **Forge** (Soul of the Forge): passive fire resistance.
///   - **Twilight** (Vigilant Blessing): passive initiative advantage.
///
/// Where War / Light / Tempest / Grave subclasses each ship a Channel
/// Divinity action, Life ships a leveled-heal amplifier, and Forge
/// leans on the passive typed-resistance lane, the Twilight Domain
/// leans on the initiative-advantage lane — the buff fires once per
/// encounter at initiative-roll time with no charge to spend, no bonus
/// action to prime, no target to pick. Sibling on the "passive
/// initiative advantage as a subclass tell" cross-class lane to
/// `has_feral_instinct` (Barbarian Feral Instinct lv7) — same roll
/// shape (d20 twice, higher kept), different class chassis, different
/// tag surface (Cleric passive tag via `has_passive_feature` vs.
/// Barbarian struct-field flag).
///
/// Read at the shared `rolls_initiative_with_advantage` chokepoint in
/// `actor_template.rs` next to `has_feral_instinct` — same lane,
/// different source. The two never legally co-occur on a single build
/// (Twilight Cleric vs. Barbarian Feral Instinct are distinct subclass
/// / class chassis), and a hypothetical multiclass carrier still just
/// gets advantage (not "double advantage") since the roll shape floors
/// at "higher of two d20s".
///
/// RAW's Twilight Domain picks up other features not shipped on this
/// template — Eyes of Night (lv1: 300ft ally-shareable darkvision),
/// Channel Divinity: Twilight Sanctuary (lv2: mobile 30ft temp-HP /
/// condition-cleanse aura), Steps of Night (lv6: fly speed while in
/// dim light or darkness), Divine Strike (Radiant) (lv8 — the baseline
/// cleric already ships this via the shared `DIVINE_STRIKE_TAG`), and
/// Twilight Shroud (lv17: allies in the sanctuary aura get half cover).
/// Only the lv1 Vigilant Blessing passive has a mechanical surface on
/// the CR-0.5 chassis that plugs cleanly into the shared
/// `rolls_initiative_with_advantage` chokepoint, so we ship that half
/// and leave the rest as future work — matching the way the other
/// subclass templates each ship only their load-bearing tactical
/// feature (Necromancy Wizard ships only Inured to Undeath, Forge
/// Cleric ships only Soul of the Forge, etc.).
///
/// Distinct from `CLERIC_TEMPLATE` (subclass-less baseline) and the
/// War / Light / Tempest / Life / Grave / Forge cousins so a Twilight-
/// vs-Baseline / vs-War / vs-Light / vs-Tempest / vs-Life / vs-Grave /
/// vs-Forge encounter renders unambiguously by name.
///
/// Glyph 'X' (for the Twili**X**ht identity — the 'T' letter is already
/// taken by the Tempest Cleric's 'S' near-neighbor and other 'T'-glyph
/// templates; 'X' reads as a stylized crossroads-of-day-and-night
/// symbol) so the Twilight Cleric shows up distinctly on the map next
/// to baseline 'C', War 'W', Light 'L', Tempest 'S', Life 'V', Grave
/// 'G', and Forge 'F'. Collides with no other current PC subclass
/// template glyph.
///
/// Ships on the CR-0.5 cleric chassis at (or above) its strict RAW lv1
/// gate for the same reason `FORGE_CLERIC_TEMPLATE` ships Soul of the
/// Forge (RAW lv6), `NECROMANCY_WIZARD_TEMPLATE` ships Inured to
/// Undeath (RAW lv10), and every other subclass template runs above
/// its strict RAW gate — class templates target a balanced playable
/// level, not lockstep PHB progression.
pub static TWILIGHT_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Cleric envelope wholesale
    // and layers on the Vigilant Blessing passive tag. The
    // `..base.clone()` tail inside the helper picks up every other
    // field — the full cleric spell ladder, save profs, stats, slots,
    // and the TURN_UNDEAD / DIVINE_STRIKE / PRESERVE_LIFE /
    // DESTROY_UNDEAD baseline features — without an N-line
    // field-by-field copy. No new actions are pushed — Vigilant Blessing
    // is a purely passive initiative-advantage grant read at
    // `rolls_initiative_with_advantage`, not a fresh action surface, so
    // the "tag-only" shape the helper wraps is a natural fit. Sibling
    // helper users on the "clone base + insert one tag" cross-class
    // lane: every tag-only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `FORGE_CLERIC_TEMPLATE`, `NECROMANCY_WIZARD_TEMPLATE`,
    // `SHADOW_MAGIC_SORCERER_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`, `LONG_DEATH_MONK_TEMPLATE`.
    CLERIC_TEMPLATE.with_subclass_tag("Twilight Cleric", 'X', VIGILANT_BLESSING_TAG)
});

/// Arcana Domain Cleric — Divine Domain **Arcana Domain** subclass build
/// (SCAG). The domain for a cleric whose god is a god of magic, and the
/// only cleric build in the engine whose Channel Divinity points at
/// outsiders rather than the dead.
///
/// Two subclass features ship:
///
///   - **Arcane Abjuration** (Channel Divinity, lv2) — an action: every
///     celestial, elemental, fey or fiend within 30 ft rolls a WIS save
///     vs the cleric's DC or is Frightened for 10 rounds. One charge per
///     short rest, via the shared `TurnBurst` config.
///   - **Spell Resistance** (lv6) — advantage on saving throws against
///     spells, mapped onto the engine's existing `has_magic_resistance`
///     lane.
///
/// The type filter is the exact complement of Turn Undead's, and that
/// complementarity is the build's argument for existing. Every other
/// cleric in the engine answers the undead; this one answers everything
/// that walked in from another plane. A party running an Arcana Cleric
/// beside any other cleric has a Channel Divinity for every extraplanar
/// creature type the engine has — and the Arcana Cleric keeps Turn Undead
/// too, inherited from the baseline chassis, so it loses nothing by
/// specializing.
///
/// Spell Resistance is the more quietly valuable half on a d8 chassis.
/// The engine doesn't distinguish spell saves from other saves, so the
/// flag grants advantage on all of them — broader than RAW. That
/// over-grant is the same one every monster carrying Magic Resistance
/// already gets (see `has_magic_resistance`), and it is what makes this
/// the most survivable cleric template: a WIS-primary caster with
/// advantage on every save is very hard to lock down, which is precisely
/// the failure mode that kills the other seven domain builds.
///
/// RAW's other Arcana features are left out for the usual reasons.
/// Arcane Initiate (lv1) grants two wizard cantrips — the baseline cleric
/// already carries Sacred Flame and Toll the Dead, and the engine has no
/// per-class spell-list gate for the grant to be interesting against.
/// Potent Spellcasting (lv8) adds WIS to cantrip damage, which needs a
/// cantrip-damage-bonus lane nothing else in the engine wants yet. Arcane
/// Mastery (lv17) grants 6th-9th level spells the CR-0.5 chassis has no
/// slots for.
///
/// Glyph 'A' — for **A**rcana. Distinct from baseline cleric 'C', War
/// 'W', Light 'L', Tempest 'S', Life 'V', Grave 'G', Forge 'F' and
/// Twilight 'X'.
///
/// Ships on the CR-0.5 cleric chassis at (or above) its strict RAW lv2 /
/// lv6 gates for the same reason `FORGE_CLERIC_TEMPLATE` ships Soul of
/// the Forge (RAW lv6) — class templates target a balanced playable
/// level, not lockstep PHB progression.
pub static ARCANA_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner the Forge / Twilight domains
    // use: this subclass adds an action, a tag, and a struct-field flag.
    // The `..CLERIC_TEMPLATE.clone()` tail still picks up the full cleric
    // spell ladder, save profs, stats, slots, and the baseline
    // TURN_UNDEAD / DIVINE_STRIKE / PRESERVE_LIFE / DESTROY_UNDEAD
    // feature set.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    actions.push(&*ARCANE_ABJURATION);
    let mut features = CLERIC_TEMPLATE.features.clone();
    features.insert(ARCANE_ABJURATION_TAG);
    CreatureTemplate {
        name: "Arcana Cleric",
        glyph: 'A',
        actions,
        features,
        // 5e Arcana Domain **Spell Resistance** (subclass level 6):
        // advantage on saves against spells. Rides the engine's existing
        // Magic Resistance lane, which grants advantage on *every* save
        // — the engine doesn't split spell saves from the rest. Same
        // over-grant every Magic Resistance monster already enjoys.
        has_magic_resistance: true,
        ..CLERIC_TEMPLATE.clone()
    }
});

/// Nature Domain Cleric — Divine Domain **Nature Domain** subclass build
/// (PHB). The tenth cleric domain in the engine, and the one whose
/// signature reaction cares about the *type* of damage coming in rather
/// than how it arrives.
///
/// Two subclass features ship:
///
///   - **Acolyte of Nature** (lv1) — a druid cantrip and the Nature
///     skill. Thorn Whip is the cantrip: a ranged attack that drags the
///     target 10 ft toward the cleric, which is the only forced-movement
///     tool on any cleric template.
///   - **Dampen Elements** (lv6) — a reaction granting the cleric or an
///     ally within 30 ft resistance to acid, cold, fire, lightning or
///     thunder damage. A row on the shared `REACTIVE_DAMAGE_CLAMPS`
///     cohort, and the row that put a damage-type filter on it.
///
/// Dampen Elements is the sharpest clamp in the engine and the narrowest
/// at the same time. Against a dragon's breath or a lightning-heavy
/// caster it halves the biggest number on the table from across the
/// battlefield; against a room full of scimitars it never fires. That
/// makes this a domain you pick knowing what you're walking into — the
/// opposite of the Cavalier's Warding Maneuver, which halves anything but
/// only at arm's length.
///
/// Thorn Whip pairs with the pull the domain otherwise lacks: a Nature
/// Cleric can drag an archer out of position into the party's melee, then
/// spend the reaction halving the fire damage the archer's wizard aims at
/// the fighter. Neither feature is loud on its own; together they make
/// the build the one that decides *where* the fight happens.
///
/// RAW's other Nature features are left out. Channel Divinity: Charm
/// Animals and Plants (lv2) is a 30 ft WIS-save burst against beasts and
/// plants — mechanically a `TurnBurst` sibling, except it applies Charmed
/// rather than Frightened, and Charmed's `charmed_by` back-link isn't on
/// the shared `condition_link_side_effect` dispatch the burst resolver
/// would need. Dampen Elements is the load-bearing half anyway. Divine
/// Strike (lv8) is already on the baseline chassis via
/// `DIVINE_STRIKE_TAG`; Master of Nature (lv17) commands charmed beasts,
/// which needs the charm half first.
///
/// Glyph 'N' — for **N**ature. Distinct from baseline cleric 'C', War
/// 'W', Light 'L', Tempest 'S', Life 'V', Grave 'G', Forge 'F', Twilight
/// 'X' and Arcana 'A'.
///
/// Ships on the CR-0.5 cleric chassis at (or above) its strict RAW lv1 /
/// lv6 gates for the same reason `FORGE_CLERIC_TEMPLATE` ships Soul of
/// the Forge (RAW lv6) — class templates target a balanced playable
/// level, not lockstep PHB progression.
pub static NATURE_CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner the Forge / Twilight domains
    // use: this subclass adds an action, a tag and a skill. The
    // `..CLERIC_TEMPLATE.clone()` tail still picks up the full cleric
    // spell ladder, save profs, stats, slots, and the baseline
    // TURN_UNDEAD / DIVINE_STRIKE / PRESERVE_LIFE / DESTROY_UNDEAD
    // feature set.
    let mut actions = CLERIC_TEMPLATE.actions.clone();
    // 5e Acolyte of Nature (lv1): one druid cantrip of the cleric's
    // choice. Thorn Whip is the pick with the most combat surface — the
    // 10 ft pull is forced movement no other cleric template has.
    actions.push(&*THORN_WHIP);
    let mut features = CLERIC_TEMPLATE.features.clone();
    features.insert(DAMPEN_ELEMENTS_TAG);
    let mut skills = CLERIC_TEMPLATE.skills.clone();
    // Acolyte of Nature's other half: proficiency in Animal Handling,
    // Nature or Survival. Nature is the on-the-nose pick.
    skills.insert(Skill::Nature);
    CreatureTemplate {
        name: "Nature Cleric",
        glyph: 'N',
        actions,
        features,
        skills,
        ..CLERIC_TEMPLATE.clone()
    }
});
