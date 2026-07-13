use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    ACID_SPLASH, ANIMATE_DEAD, ARMOR_OF_AGATHYS, BANISHMENT, BESTOW_CURSE, BLINDNESS,
    BURNING_HANDS, CHARM_PERSON, CHILL_TOUCH, COUNTERSPELL, DIMENSION_DOOR, ELDRITCH_BLAST,
    EYEBITE, FEAR, FIRE_BOLT, FLY, HELLISH_REBUKE, HEX, HOLD_MONSTER, HOLD_PERSON,
    HYPNOTIC_PATTERN, INVISIBILITY, LIGHTNING_LURE, MAGE_ARMOR, MISTY_STEP, POISON_SPRAY,
    POWER_WORD_KILL, POWER_WORD_STUN, SHIELD, SICKENING_RADIANCE, SLEEP, SUGGESTION,
    VAMPIRIC_TOUCH, WITCH_BOLT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Warlock PC template. CHA-primary half-caster with Pact Magic — RAW
/// the warlock's defining feature is short-rest spell slots: a small
/// pool (~2-4) that all sit at the warlock's highest available slot
/// level, refreshing on a short rest. The engine doesn't model short
/// rests as a discrete event today (long rest is the only refresh
/// trigger), so we approximate with a flat 4 slots concentrated at
/// level 5 — the load-bearing apex slot the warlock blasts with — plus
/// a thin lower-level spread for situational picks.
///
/// Loadout philosophy:
/// - **Eldritch Blast** as the at-will ranged cantrip (the warlock's
///   signature cantrip, scales with caster level).
/// - **Hex** as the per-encounter rider buff (concentration; pairs with
///   EB swings).
/// - **Hellish Rebuke** as the bonus-action reactive damage.
/// - **Witch Bolt** as the lv1 sustained zap (concentration).
/// - **Hold Person / Suggestion / Hypnotic Pattern** as enchantment
///   control.
/// - **Eyebite / Power Word Kill** as the apex single-target threats.
///
/// Stat shape: AC 12 (unarmored + DEX), 8d8+8 HP (~44), CHA 18.
/// Proficient WIS + CHA saves (RAW). Speaks Common + Infernal (the
/// patron's tongue).
pub static WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // Cantrips (at-will)
    actions.push(&*ELDRITCH_BLAST);
    actions.push(&*FIRE_BOLT);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*ACID_SPLASH);
    actions.push(&*POISON_SPRAY);
    actions.push(&*LIGHTNING_LURE);
    // Level 1 — Hex defines the warlock's rider loop; Hellish Rebuke
    // for reactive burst; Witch Bolt for sustained zap; Mage Armor /
    // Shield for survivability; Charm / Sleep for soft control.
    actions.push(&*HEX);
    actions.push(&*HELLISH_REBUKE);
    actions.push(&*WITCH_BOLT);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*SHIELD);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    actions.push(&*BURNING_HANDS);
    // Armor of Agathys — the warlock's signature self-buff: 5 temp HP
    // plus 5 cold damage reflected on melee hit. Pairs with the
    // warlock's lv1 slot economy as a pre-fight tank-up.
    actions.push(&*ARMOR_OF_AGATHYS);
    // Level 2 — Misty Step (escape), Hold Person (control), Invisibility,
    // Blindness, Suggestion (single-target charm).
    actions.push(&*MISTY_STEP);
    actions.push(&*HOLD_PERSON);
    actions.push(&*INVISIBILITY);
    actions.push(&*BLINDNESS);
    actions.push(&*SUGGESTION);
    // Level 3 — Fear (cone Frightened), Counterspell (anti-caster),
    // Hypnotic Pattern (AoE charm), Vampiric Touch (sustained life
    // drain), Bestow Curse (single-target debuff), Fly, Animate Dead.
    actions.push(&*FEAR);
    actions.push(&*COUNTERSPELL);
    actions.push(&*HYPNOTIC_PATTERN);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*FLY);
    actions.push(&*ANIMATE_DEAD);
    // Level 4 — Banishment (single-target removal), Dimension Door
    // (teleport). Sickening Radiance: enemy-only 30ft burst with
    // Exhausted-on-fail; fits the warlock's "control burst" niche.
    actions.push(&*BANISHMENT);
    actions.push(&*DIMENSION_DOOR);
    actions.push(&*SICKENING_RADIANCE);
    // Level 5 — Hold Monster (single-target paralysis on a bigger fish).
    actions.push(&*HOLD_MONSTER);
    // Negative Energy Flood — necromancy lv5 burst that fits the
    // patron's flavor; CON-save halve, 5d12 necrotic on fail.
    actions.push(&*crate::actions::spells::NEGATIVE_ENERGY_FLOOD);
    // Level 6 — Eyebite (single-target sleep), the warlock's apex
    // control. RAW gates Eyebite at lv6; we put it at lv6 here too.
    actions.push(&*EYEBITE);
    // Level 9 — Power Word Stun and Power Word Kill (single-target
    // boss-killers; the warlock's apex damage button).
    actions.push(&*POWER_WORD_STUN);
    actions.push(&*POWER_WORD_KILL);
    // Newest warlock additions:
    //   - cantrip **Thunderclap**: self-centered CON-save burst.
    //   - lv2 **Mind Spike**: single-target psychic save-for-half.
    //   - lv4 **Psychic Lance**: psychic save-for-half + Incapacitated
    //     rider on fail. Pair with Hex for a +1d6 necrotic rider on the
    //     base damage.
    actions.push(&*crate::actions::spells::THUNDERCLAP);
    actions.push(&*crate::actions::spells::MIND_SPIKE);
    actions.push(&*crate::actions::spells::PSYCHIC_LANCE);
    // Latest warlock additions (lv0-1):
    //   - cantrip **Sword Burst**: 1-tile force burst around caster —
    //     a melee-flavored at-will for warlocks who close into reach.
    //   - cantrip **Blade Ward**: self damage-resistance till next turn
    //     (rare defensive cantrip option for the squishy chassis).
    //   - lv1 **Fog Cloud**: concentration heavy-obscurement burst.
    actions.push(&*crate::actions::spells::SWORD_BURST);
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    // Signature warlock pickup:
    //   - lv1 **Arms of Hadar** (warlock-only): self-centered necrotic
    //     burst with STR save for half + no-reactions rider on fail.
    //     Punishes melee swarms that close on the warlock.
    actions.push(&*crate::actions::spells::ARMS_OF_HADAR);
    // lv3 **Hunger of Hadar** — warlock signature: cold + acid sphere.
    actions.push(&*crate::actions::spells::HUNGER_OF_HADAR);
    // lv5 **Eldritch Smite** — warlock melee burst + prone on fail.
    actions.push(&*crate::actions::spells::ELDRITCH_SMITE);
    // Telekinetic — cantrip bonus-action shove. 5ft pull on a failed
    // STR save, no slot. Cheap repositioning for the warlock's
    // bonus-action lane (otherwise empty between Hex / Hex re-target).
    actions.push(&*crate::actions::spells::TELEKINETIC);
    // Green-Flame Blade — cantrip CHA-scaled melee touch (1d8 fire +
    // ability-modifier fire leap to the lowest-HP adjacent enemy on a
    // hit). Pact-of-the-Blade-style melee cantrip for the warlock —
    // pairs with Eldritch Blast for a melee-vs-ranged at-will lane.
    actions.push(&*crate::actions::spells::GREEN_FLAME_BLADE);
    actions.push(&*crate::actions::spells::THUNDER_STEP);
    actions.push(&*crate::actions::spells::SHADOW_BLADE);
    actions.push(&*crate::actions::spells::CLOUD_OF_DAGGERS);
    // Warlock capstones:
    //   - lv6 **Flesh to Stone** (transmutation): CON save vs the
    //     warlock's spell DC; on fail target is Petrified for ~1 minute
    //     (concentration-bound). Single-target lockdown lane that
    //     complements Eyebite (WIS save, Asleep) at the same slot tier —
    //     the warlock can pick whichever save the target is weakest at.
    //   - lv9 **Psychic Scream** (enchantment): self-centered 8-tile
    //     burst, 14d6 psychic INT-save for half + Stunned-on-fail. The
    //     warlock's lv9 mass-control button — burst stuns the whole
    //     hostile back rank in one tap, distinct from Power Word Kill
    //     (HP-gated single-target) and Power Word Stun (HP-gated single
    //     stun).
    actions.push(&*crate::actions::spells::FLESH_TO_STONE);
    actions.push(&*crate::actions::spells::PSYCHIC_SCREAM);
    // Latest warlock additions:
    //   - lv8 **Maddening Darkness** (evocation, XGtE): 6-tile burst,
    //     8d8 psychic WIS-save for half (concentration-bound). The
    //     warlock's lv8 mass-control burst — distinct from Power Word
    //     Stun (lv8 single-target HP-gated). Pairs cleanly with the
    //     warlock's CHA-anchored DC and the lv8 slot in the chassis.
    actions.push(&*crate::actions::spells::MADDENING_DARKNESS);
    // Mobility / utility additions (RAW warlock list):
    //   - lv1 **Expeditious Retreat**: bonus-action self-buff that grants
    //     +30 ft speed for 10 rounds, concentration. Kiting tool that
    //     pairs with the warlock's at-will Eldritch Blast — drink the
    //     buff, then plink from extended range.
    //   - lv2 **Earthbind** (XGtE): single-target STR-save ground; strips
    //     `Flying` / `InvestedInWind` on a failed save. Warlock's anti-
    //     air control option.
    actions.push(&*crate::actions::spells::EXPEDITIOUS_RETREAT);
    actions.push(&*crate::actions::spells::EARTHBIND);
    // lv6 **Tasha's Otherworldly Guise** (transmutation, TCE): the warlock's
    // top-tier self-buff. The celestial-form envelope (+2 AC, +60 ft fly,
    // radiant/poison resistance, Charmed/Frightened/Poisoned dynamic
    // immunity, +2d6 radiant melee weapon rider) gives the warlock a
    // single-spell legendary buff that pairs with Eldritch Blast's
    // ranged kit (the rider is melee-only, but the resistance + flight +
    // immunity envelope hardens the warlock against incoming damage).
    actions.push(&*crate::actions::spells::OTHERWORLDLY_GUISE);
    // lv2 **Darkness**: a warlock signature (Devil's Sight invocation
    // historically lets warlocks see through their own darkness;
    // engine-side we just install the concentration-bound symmetric-
    // blind zone). Goes on the warlock list as part of their Pact of
    // the Fiend / Pact of the Chain SRD baseline.
    actions.push(&*crate::actions::spells::DARKNESS);
    // lv4 **Shadow of Moil** (XGtE evocation, concentration). Self-only
    // shadow wrap: 2d8 necrotic retaliation on every melee hit AND
    // attackers swing with disadvantage. Sibling to Fire Shield (radiant
    // tier, no attacker-debuff) on the self-shield lane — the necrotic
    // typing leans into the warlock's death-flavored kit.
    actions.push(&*crate::actions::spells::SHADOW_OF_MOIL);
    CreatureTemplate {
        name: "Warlock",
        // 'L' (uppercase) — distinct from 'l' (Lich), 'W' (Wolf glyph),
        // 'M' (Wizard / Mage). Reads as a robed CHA-caster.
        glyph: 'L',
        ac: 12,
        hitpoints: "8d8+8".parse().unwrap(),
        strength: 8,
        dexterity: 14,
        constitution: 14,
        intelligence: 12,
        wisdom: 12,
        charisma: 18, // primary spellcasting ability
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Pact Magic compromise: a flat 4 lv5 slots (the warlock's
        // top-level slots all sit at the highest available slot level
        // RAW). Lower levels carry just 1 slot apiece so the situational
        // picks (Misty Step, Counterspell, etc.) still have ammunition
        // without diluting the "blast with the apex slot" feel.
        // Index: lv1=2, lv2=1, lv3=1, lv4=1, lv5=4, lv6+1 (Eyebite),
        // lv7=0, lv8=0, lv9=1 (Power Word Kill / Stun).
        spell_slots_by_level: vec![2, 1, 1, 1, 4, 1, 0, 0, 1],
        rolls_death_saves: true,
        // Warlocks are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // 5e Warlock Eldritch Invocations:
        //   - Agonizing Blast: +CHA mod to each Eldritch Blast beam.
        //   - Repelling Blast: 10ft (4-tile) push on hit, Large-or-
        //     smaller targets only.
        //   - Eldritch Mind: advantage on Constitution saves to maintain
        //     concentration (read at the damage chokepoint).
        // RAW a level-5 warlock picks 3 invocations; the kit pre-picks
        // these three since EB is the signature cantrip and
        // concentration-bound spells (Hex / Hunger of Hadar) form the
        // back half of the warlock's lockdown plan. Permanent passive
        // features — never consumed; the relevant cast / save sites
        // read them via `feature_available`.
        features: HashSet::from([
            crate::actions::class_features::AGONIZING_BLAST_TAG,
            crate::actions::class_features::REPELLING_BLAST_TAG,
            crate::actions::class_features::ELDRITCH_MIND_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Fiend Warlock — Otherworldly Patron **The Fiend** subclass build.
/// Identical envelope to the baseline `WARLOCK_TEMPLATE` (CHA-primary
/// half-caster with Pact Magic, Eldritch Blast + Hex + Witch Bolt at
/// will, Agonizing / Repelling / Eldritch Mind invocations) with one
/// subclass feature layered on: **Dark One's Blessing** (Fiend
/// subclass level 1) — passive: whenever the warlock reduces a
/// hostile to 0 HP they gain `max(1, CHA mod + warlock level)` temp
/// HP.
///
/// The signature "fiendish resilience" tell — a Fiend warlock who
/// lands the killing blow on a downed enemy walks into the next
/// swing armored with a fresh temp-HP buffer. Pairs naturally with
/// the warlock's Eldritch Blast finisher pattern: the last beam that
/// drops a target reads the pool cap (CHA 18 → +4, level 5 → total
/// 9 temp HP), sizeable enough to soak the next Guiding Bolt or a
/// second-tier Fireball beam.
///
/// Distinct from `WARLOCK_TEMPLATE` (Patron-less baseline). RAW's
/// Fiend patron picks up an expanded spell list (Burning Hands /
/// Command / Blindness / Scorching Ray / Fireball / Stinking Cloud
/// / Wall of Fire / Fire Shield / Insect Plague) — the baseline
/// template's spell list already covers most of the overlap
/// (Burning Hands, Blindness, Fear at lv3, Sickening Radiance at
/// lv4) so we don't re-add the whole spell list here; the subclass
/// tell is the passive kill-triggered temp-HP well.
///
/// Ships the CR-4 template above the strict RAW gate for the same
/// reason every other subclass template runs above strict RAW level
/// (class templates target a balanced playable level, not lockstep
/// PHB progression). Glyph 'F' so the Fiend warlock shows up
/// distinctly on the map next to the baseline warlock 'L'.
pub static FIEND_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the Fiend Patron features:
    //   - `DARK_ONES_BLESSING_TAG` (lv1): passive kill-triggered temp-HP
    //     buffer.
    //   - `DARK_ONES_OWN_LUCK_TAG` (lv6): once-per-short-rest auto-fire
    //     "add 1d10 to a failed save" gate. Registered in
    //     `SHORT_REST_FEATURES` so the charge refreshes alongside the
    //     other short-rest features on this chassis.
    //   - `HURL_THROUGH_HELL` action + `HURL_THROUGH_HELL_TAG` (lv14
    //     subclass capstone): once-per-short-rest single-target 60ft
    //     10d10 psychic damage burst via CHA save for half. Ships
    //     above its strict RAW lv14 gate for the same reason Fiendish
    //     Resilience (lv10) does — the CR-4 template targets a
    //     balanced playable level, not lockstep PHB progression.
    let mut actions = WARLOCK_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::class_features::HURL_THROUGH_HELL);
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::DARK_ONES_BLESSING_TAG);
    features.insert(crate::actions::class_features::DARK_ONES_OWN_LUCK_TAG);
    features.insert(crate::actions::class_features::HURL_THROUGH_HELL_TAG);
    CreatureTemplate {
        name: "Fiend Warlock",
        glyph: 'F',
        actions,
        features,
        // 5e Warlock Fiend Patron **Fiendish Resilience** (level 10) —
        // passive template flag. RAW's rest-cycle "choose one damage
        // type" surface collapses to a fixed Fire lock (thematic for
        // the Fiend patron's fire-heavy identity — Burning Hands /
        // Fireball / Wall of Fire on the expanded spell list, Dark
        // One's Blessing as the kill-triggered temp-HP well). Read at
        // the shared `PASSIVE_TYPED_RESISTANCES` cohort in
        // `effective_damage` next to Dwarven Resilience's poison-
        // halving half; folds into the standard "one halving per
        // damage instance" rule so a Fiend warlock hit by Fireball
        // takes /2 damage cleanly even if they were also concentrating
        // on Blade Ward (which grants a blanket resistance the folder
        // would already have caught). Ships on the CR-4 template above
        // its strict RAW lv10 gate for the same reason Dark One's Own
        // Luck (RAW lv6) does — class templates target a balanced
        // playable level, not lockstep PHB progression.
        has_fiendish_resilience: true,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Undying Warlock — Otherworldly Patron **The Undying** subclass build
/// (SCAG). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex +
/// Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
/// invocations) with one patron-flavored eldritch invocation layered
/// on: **Aspect of the Moon** (Undying-patron-restricted invocation) —
/// passive: the warlock no longer needs to sleep and can't be forced to
/// sleep by any means.
///
/// The signature "you can't put me under" tell — where a baseline
/// Warlock eats a Sleep spell (5e enchantment; installs the `Asleep`
/// condition on a failed save), the Undying Warlock just shrugs it off
/// while the Aspect of the Moon flag holds. Read at the shared
/// `FLAG_DRIVEN_IMMUNITIES` cohort in `actor_template.rs` next to Fey
/// Ancestry's Asleep row — same "install bounces" wire, different
/// source (racial trait vs. patron-gated invocation).
///
/// Distinct from `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing +
/// Dark One's Own Luck + Fiendish Resilience — the fiery kill-focused
/// build) and `WARLOCK_TEMPLATE` (patron-less baseline). RAW's Undying
/// patron picks up an expanded spell list (False Life / Spare the
/// Dying / Blindness/Deafness / Feign Death / Bestow Curse / Speak with
/// Dead / Aura of Life / Death Ward / Contagion / Legend Lore) — the
/// baseline template's spell list already covers most of the overlap
/// (Blindness at lv2, Bestow Curse at lv3) so we don't re-add the whole
/// spell list here; the subclass tell is the passive Asleep immunity.
/// Glyph 'U' so the Undying warlock shows up distinctly on the map
/// next to baseline warlock 'L' and Fiend warlock 'F'.
pub static UNDYING_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the one Undying patron feature — Aspect of the Moon
    // via the `ASPECT_OF_THE_MOON_TAG` passive-feature tag. The
    // `..base.clone()` tail picks up every other field — stats, spell
    // slots, save profs, invocation-driven cantrips — without an N-line
    // field-by-field copy. Same shape as `FIEND_WARLOCK_TEMPLATE` and
    // the paladin / rogue / ranger subclass templates.
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::ASPECT_OF_THE_MOON_TAG);
    CreatureTemplate {
        name: "Undying Warlock",
        glyph: 'U',
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Great Old One Warlock — Otherworldly Patron **The Great Old One**
/// subclass build (PHB). Identical envelope to the baseline
/// `WARLOCK_TEMPLATE` (CHA-primary half-caster with Pact Magic,
/// Eldritch Blast + Hex + Witch Bolt at will, Agonizing / Repelling /
/// Eldritch Mind invocations) with one subclass feature layered on:
/// **Entropic Ward** (Great Old One lv6) — once-per-short-rest
/// reactive disadvantage on an incoming attack roll against the
/// warlock.
///
/// The signature "the patron's mind reads the attacker's intent"
/// tell — where a baseline Warlock eats an attack roll clean, the
/// Great Old One Warlock leans on their patron's alien awareness to
/// force a re-roll-with-the-lower-die on the swing. Ships as a
/// passive charge on the template; the trigger fires automatically at
/// the attack chokepoint (`resolve_attack` for weapon swings and
/// `spell_attack_outcome` for spell attacks) via
/// `EncounterInstance::apply_reactive_attack_disadvantage`, which
/// walks the shared `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort
/// (Warding Flare's sibling entry rides the same iterator).
///
/// Sibling on the reactive-per-rest-disadvantage lane to Warding
/// Flare (`LIGHT_CLERIC_TEMPLATE`) but distinct on two axes:
///   1. **Range** — Warding Flare's RAW 30ft "must be within" cap
///      collapses to 12 tiles; Entropic Ward has NO range gate (RAW:
///      the patron's telepathic tie reaches anywhere).
///   2. **Sight** — Warding Flare gates on "you can see the
///      attacker" (routes through `viewer_can_see`); Entropic Ward
///      does NOT (RAW: the ward hums against your skin regardless of
///      sight).
/// A hypothetical Light Cleric / Great Old One Warlock multiclass
/// would carry BOTH cohort rows; the iterator returns after the
/// first firing so at most one per-rest charge burns per incoming
/// attack, matching the "at most one add-die per save" ordering
/// semantics on the failed-save recovery cohort.
///
/// Distinct from `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing +
/// Dark One's Own Luck + Fiendish Resilience — the fiery kill-focused
/// build), `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the
/// insomniac's build), and `WARLOCK_TEMPLATE` (patron-less baseline).
/// RAW's Great Old One patron picks up other features not shipped
/// on this template — **Awakened Mind** (lv1: telepathy within 30ft;
/// no combat surface without a communication mechanic),
/// **Whispers of the Grave** (lv10: cast Speak with Dead at will;
/// out-of-combat), and **Create Thrall** (lv14 capstone: touch a
/// humanoid to install permanent Charmed; a one-shot ritual with no
/// clean combat surface). Only the lv6 Entropic Ward has a
/// mechanical surface on the CR-4 chassis that plugs cleanly into
/// the shared reactive-attack-disadvantage cohort, so we ship that
/// half and leave the rest as future work.
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10) and `DRACONIC_SORCERER_TEMPLATE` ships Draconic
/// Resilience (RAW lv6): class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'O' — distinct from baseline warlock 'L', Fiend warlock
/// 'F', and Undying warlock 'U'; 'O' for the alien "Old One" flavor
/// (the sorcerer as a channel for the patron's incomprehensible
/// awareness).
pub static GREAT_OLD_ONE_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the one Great Old One patron feature — Entropic
    // Ward via the `ENTROPIC_WARD_TAG` passive-feature tag registered
    // in `SHORT_REST_FEATURES`. The `..base.clone()` tail picks up
    // every other field — stats, spell slots, save profs, invocation-
    // driven cantrips — without an N-line field-by-field copy. Same
    // shape as `UNDYING_WARLOCK_TEMPLATE`'s tag-only subclass build
    // (Aspect of the Moon) and the sorcerer / paladin / rogue /
    // ranger subclass templates.
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::ENTROPIC_WARD_TAG);
    CreatureTemplate {
        name: "Great Old One Warlock",
        // 'O' — distinct from baseline warlock 'L', Fiend 'F', and
        // Undying 'U'; 'O' for the alien "Old One" flavor.
        glyph: 'O',
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Archfey Warlock — Otherworldly Patron **The Archfey** subclass build
/// (PHB). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex +
/// Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
/// invocations) with one subclass feature layered on: **Beguiling
/// Defenses** (Archfey lv10) — passive immunity to being **Charmed**.
///
/// The signature "the fey court's whispers can't take root" tell —
/// where a baseline Warlock eats a Charm Person save-fail (5e
/// enchantment; installs the `Charmed` condition), the Archfey Warlock
/// simply bounces the install while the Beguiling Defenses flag holds.
/// Ships as a passive tag on the template; the immunity fires
/// automatically at every `Charmed` install chokepoint via the shared
/// `FLAG_DRIVEN_IMMUNITIES` cohort's slice-of-conditions row (Fey
/// Ancestry's Charmed row is the sibling entry — same suppressed
/// condition, different source).
///
/// Sibling on the passive-condition-immunity lane to:
///   - **Fey Ancestry** (Elven / Half-Elven / Drow racial): Charmed
///     bounce, same condition, different source (racial trait vs.
///     patron-gated subclass feature). A Fey-Ancestry Elven Archfey
///     Warlock stacks the two rows redundantly under the OR-of-
///     cohort-hits semantics — either flag alone bounces the install.
///   - **Psychic Defenses** (Aberrant Mind Sorcerer lv14): Charmed +
///     Frightened bounce, distinct on the condition axis (Beguiling
///     Defenses covers only Charmed — RAW-exact, since the reaction
///     charm-back clause presumes the warlock stays lucid). A
///     hypothetical Archfey Warlock / Aberrant Mind Sorcerer would
///     carry both rows; either alone bounces the Charmed install, and
///     only Psychic Defenses bounces the Frightened install.
///   - **Nature's Ward** (Ancients Paladin lv15): Charmed + Frightened
///     bounce, same disparity as Psychic Defenses on the Frightened
///     axis.
///   - **Aspect of the Moon** (Undying Warlock patron invocation):
///     Asleep bounce, distinct condition entirely; the two Warlock
///     subclass tags never legally co-occur on a single build since
///     the warlock picks one Otherworldly Patron.
///
/// Distinct from `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark
/// One's Own Luck + Fiendish Resilience — the fiery kill-focused
/// build), `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the
/// insomniac's build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic
/// Ward — the alien-awareness reactive build), and `WARLOCK_TEMPLATE`
/// (patron-less baseline). RAW's Archfey patron picks up other
/// features not shipped on this template — **Fey Presence** (lv1: CD
/// 10ft-cube Charmed OR Frightened burst; a Channel-Divinity-shaped
/// burst-save-with-condition-rider we could ship as an action but not
/// yet), **Misty Escape** (lv6: reaction on-damage teleport +
/// invisibility; a reaction-triggered self-move-and-condition-install
/// that needs the reaction framework), **Dark Delirium** (lv14
/// capstone: single-target 1min Charmed OR Frightened install; a
/// spend-slot-driven CC lane). Only the lv10 Beguiling Defenses has a
/// mechanical surface on the CR-4 chassis that plugs cleanly into the
/// shared passive-condition-immunity cohort, so we ship that half and
/// leave the rest as future work.
///
/// Ships the CR-4 template above the strict RAW lv10 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10) and `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic
/// Ward (RAW lv6): class templates target a balanced playable level,
/// not lockstep PHB progression.
///
/// Glyph 'A' — distinct from baseline warlock 'L', Fiend warlock 'F',
/// Undying warlock 'U', and Great Old One warlock 'O'; 'A' for the
/// "Archfey" identity (the warlock as a court-linked emissary of a
/// fey monarch's whimsy).
pub static ARCHFEY_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the one Archfey patron feature — Beguiling Defenses
    // via the `BEGUILING_DEFENSES_TAG` passive-feature tag. The
    // `..base.clone()` tail picks up every other field — stats, spell
    // slots, save profs, invocation-driven cantrips — without an N-line
    // field-by-field copy. Same shape as
    // `GREAT_OLD_ONE_WARLOCK_TEMPLATE`'s tag-only subclass build
    // (Entropic Ward), `UNDYING_WARLOCK_TEMPLATE`'s tag-only subclass
    // build (Aspect of the Moon), and the sorcerer / paladin / rogue /
    // ranger subclass templates.
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::BEGUILING_DEFENSES_TAG);
    CreatureTemplate {
        name: "Archfey Warlock",
        // 'A' — distinct from baseline warlock 'L', Fiend 'F',
        // Undying 'U', and Great Old One 'O'; 'A' for the "Archfey"
        // identity.
        glyph: 'A',
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Celestial Warlock — Otherworldly Patron **The Celestial** subclass
/// build (XGtE). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex +
/// Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
/// invocations) with one subclass feature layered on: **Radiant Soul**
/// (Celestial lv6) — passive **resistance to radiant damage**.
///
/// The signature "the celestial patron's light shrouds you" tell —
/// where a baseline Warlock eats a Guiding Bolt / Sacred Flame /
/// Sunburst hit clean, the Celestial Warlock halves the incoming
/// radiant damage. Read at the shared `PASSIVE_TYPED_RESISTANCES`
/// cohort in `actor_template.rs` next to Fiendish Resilience's fire-
/// halving half — same lane, different patron flavor and different
/// damage axis. Pairs naturally with a party's radiant-heavy blast
/// list (Guiding Bolt from a companion Cleric, Sunburst from a party
/// Wizard, Sacred Flame overspill) — a Celestial Warlock caught in
/// their party's own AoE radiant burst walks out with half the friendly-
/// fire tax a baseline Warlock would eat.
///
/// Sibling on the passive typed-resistance patron lane to:
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire, RAW-scoped
///     to the patron's fire flavor. Distinct on the damage axis (Fire
///     vs. Radiant) — a hypothetical multi-patron carrier stacks both
///     flag closures cleanly under the "one halving per damage
///     instance" rule since the two rows never overlap on a single
///     damage type. The two Otherworldly Patron picks never legally
///     co-occur on a single build (RAW: one patron per warlock), so
///     the distinct-axis composition matters only for cross-template
///     invariants, not for a lawful PC build.
///
/// Sibling on the Otherworldly Patron subclass lane to `FIEND_WARLOCK_TEMPLATE`
/// (Dark One's Blessing + Dark One's Own Luck + Fiendish Resilience —
/// the fiery kill-focused build), `UNDYING_WARLOCK_TEMPLATE` (Aspect
/// of the Moon — the insomniac's build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE`
/// (Entropic Ward — the alien-awareness reactive build), and
/// `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling Defenses — the Charmed-bounce
/// build). RAW's Celestial patron picks up other features not shipped
/// on this template — **Bonus Cantrips** (lv1: Light + Sacred Flame
/// added; Sacred Flame lands cleanly on the shared spell list, Light
/// has no combat surface without a per-tile illumination model),
/// **Healing Light** (lv1: bonus-action pool of d6 healing dice — a
/// per-rest healing well that needs a new resource lane), **Celestial
/// Resilience** (lv10: temp HP to self + party on short rest —
/// short-rest heal buffer), and **Searing Vengeance** (lv14 capstone:
/// reactive burst on downed-ally trigger). Only the lv6 Radiant Soul
/// passive has a mechanical surface on the CR-4 chassis that plugs
/// cleanly into the shared passive typed-resistance cohort, so we
/// ship that half and leave the rest as future work.
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward
/// (RAW lv6), and `ARCHFEY_WARLOCK_TEMPLATE` ships Beguiling
/// Defenses (RAW lv10): class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'C' — distinct from baseline warlock 'L', Fiend warlock 'F',
/// Undying warlock 'U', Great Old One warlock 'O', and Archfey
/// warlock 'A'; 'C' for the "Celestial" identity (the warlock as a
/// mortal channel for celestial radiance).
pub static CELESTIAL_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the one Celestial patron feature — Radiant Soul via
    // the `RADIANT_SOUL_TAG` passive-feature tag. The `..base.clone()`
    // tail picks up every other field — stats, spell slots, save profs,
    // invocation-driven cantrips — without an N-line field-by-field
    // copy. Same shape as `ARCHFEY_WARLOCK_TEMPLATE`'s tag-only
    // subclass build (Beguiling Defenses),
    // `GREAT_OLD_ONE_WARLOCK_TEMPLATE`'s tag-only subclass build
    // (Entropic Ward), `UNDYING_WARLOCK_TEMPLATE`'s tag-only subclass
    // build (Aspect of the Moon), and the sorcerer / paladin / rogue /
    // ranger subclass templates.
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::RADIANT_SOUL_TAG);
    CreatureTemplate {
        name: "Celestial Warlock",
        // 'C' — distinct from baseline warlock 'L', Fiend 'F',
        // Undying 'U', Great Old One 'O', and Archfey 'A'; 'C' for
        // the "Celestial" identity.
        glyph: 'C',
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Marid Warlock — Otherworldly Patron **The Genie (Marid)** subclass
/// build (TCE). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex +
/// Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
/// invocations) with one subclass feature layered on: **Elemental
/// Gift** (Genie subclass level 6, Marid variant) — passive
/// **resistance to cold damage**.
///
/// The signature "the marid's tides shield me from ice" tell — where a
/// baseline Warlock eats a Cone of Cold / Ice Storm / Ray of Frost hit
/// clean, the Marid Warlock halves the incoming cold damage. Read at
/// the shared `PASSIVE_TYPED_RESISTANCES` cohort in `actor_template.rs`
/// next to Radiant Soul's radiant-halving half and Fiendish Resilience's
/// fire-halving half — same lane, different patron flavor and different
/// damage axis. First user of the Cold slot on the passive typed-
/// resistance lane; a party's fire-flavored caster (Fiendish / Draconic /
/// Efreeti Warlock) and cold-flavored caster (Marid Warlock) now cover
/// the two most common elemental blast types cleanly.
///
/// RAW's Genie Warlock (Marid variant) picks up other features not
/// shipped on this template — **Genie's Vessel** (lv1: bonus-action
/// bottle a creature into the marid vessel; a save-and-condition-install
/// resource lane that needs a per-warlock vessel tracker), **Bottled
/// Respite** (lv6: 10-min in-vessel short rest; sits outside combat),
/// **Sanctuary Vessel** (lv10: refresh temp HP on party short rest inside
/// the vessel), and **Limited Wish** (lv14 capstone: cast any lv6-or-lower
/// spell on a 1d4-day cooldown). Only the lv6 Elemental Gift passive
/// has a mechanical surface on the CR-4 chassis that plugs cleanly into
/// the shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work.
///
/// Sibling on the passive typed-resistance patron lane to:
///   - **Radiant Soul** (Celestial Warlock lv6, XGtE): Radiant, sibling
///     patron on the "Otherworldly Patron passive typed-resistance"
///     lane. Distinct on the damage axis (Radiant vs. Cold) — a
///     hypothetical multi-patron carrier stacks both flag closures
///     cleanly under the "one halving per damage instance" rule since
///     the two rows never overlap on a single damage type. The two
///     Otherworldly Patron picks never legally co-occur on a single
///     build (RAW: one patron per warlock), so the distinct-axis
///     composition matters only for cross-template invariants, not
///     for a lawful PC build.
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire — same
///     patron-lane pattern, same RAW-scoped-to-patron-flavor damage
///     axis. Distinct on the damage type: Fiend covers Fire, Marid
///     covers Cold. Composes cleanly with the "one halving per damage
///     instance" rule.
///
/// Sibling on the Otherworldly Patron subclass lane to
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck +
/// Fiendish Resilience — the fiery kill-focused build),
/// `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
/// build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the alien-
/// awareness reactive build), `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling
/// Defenses — the Charmed-bounce build), and `CELESTIAL_WARLOCK_TEMPLATE`
/// (Radiant Soul — the radiant-resistance build).
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the
/// same reason `CELESTIAL_WARLOCK_TEMPLATE` ships Radiant Soul (RAW
/// lv6), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward (RAW
/// lv6), and `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience (RAW
/// lv10): class templates target a balanced playable level, not
/// lockstep PHB progression.
///
/// Glyph 'M' — distinct from baseline warlock 'L', Fiend warlock 'F',
/// Undying warlock 'U', Great Old One warlock 'O', Archfey warlock
/// 'A', and Celestial warlock 'C'; 'M' for the "Marid" identity (the
/// warlock as a mortal bonded to a water-and-ice genie sovereign of
/// the Elemental Plane of Water).
pub static MARID_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Warlock envelope wholesale
    // and layer on the one Marid Genie patron feature — Elemental Gift
    // via the `ELEMENTAL_GIFT_TAG` passive-feature tag. The
    // `..base.clone()` tail picks up every other field — stats, spell
    // slots, save profs, invocation-driven cantrips — without an
    // N-line field-by-field copy. Same shape as
    // `CELESTIAL_WARLOCK_TEMPLATE`'s tag-only subclass build (Radiant
    // Soul), `ARCHFEY_WARLOCK_TEMPLATE`'s tag-only subclass build
    // (Beguiling Defenses), `GREAT_OLD_ONE_WARLOCK_TEMPLATE`'s tag-only
    // subclass build (Entropic Ward), `UNDYING_WARLOCK_TEMPLATE`'s
    // tag-only subclass build (Aspect of the Moon), and the sorcerer /
    // paladin / rogue / ranger subclass templates.
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::ELEMENTAL_GIFT_TAG);
    CreatureTemplate {
        name: "Marid Warlock",
        // 'M' — distinct from baseline warlock 'L', Fiend 'F',
        // Undying 'U', Great Old One 'O', Archfey 'A', and Celestial
        // 'C'; 'M' for the "Marid" identity.
        glyph: 'M',
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});
