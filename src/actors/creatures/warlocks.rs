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
    // Mislead — the level-5 illusion that is the two halves the engine
    // already priced, in one action: `Invisible` and the Trickery
    // Cleric's `Duplicity`, which until now no spell could reach. Not a
    // better Greater Invisibility but a differently shaped one — bought
    // to *not* attack from, and cashed out whenever one doubly
    // advantaged swing is worth ending it for. See `spells::MISLEAD`.
    actions.push(&*crate::actions::spells::MISLEAD);
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
    actions.push(&ANIMATE_DEAD);
    // The Tasha's summon family, warlock half. RAW gives the warlock
    // Fey, Undead, Aberration and Fiend, and the shape of that list is
    // the patron: everything the warlock can call is something that
    // could plausibly have sent it.
    //
    // The warlock is the class these spells change most, because Pact
    // Magic's slots all cast at the warlock's highest level and come
    // back on a short rest. A warlock spending one on Summon Fiend is
    // spending a resource they get again in ten minutes — which makes
    // them the one caster in the engine for whom "summon a large amount
    // of monster" is a repeatable opening rather than a daily
    // centerpiece.
    actions.push(&crate::actions::spells::SUMMON_FEY);
    actions.push(&crate::actions::spells::SUMMON_UNDEAD);
    actions.push(&crate::actions::spells::SUMMON_ABERRATION);
    actions.push(&crate::actions::spells::SUMMON_FIEND);
    // lv6 **Conjure Fey** — RAW is druid and warlock, and on the
    // warlock it lands somewhere it lands nowhere else: Pact Magic's
    // slots are all top-level and all come back on a short rest, so
    // this is the only class that can put a green hag on the board
    // twice in an afternoon. Sibling to Summon Fiend on the same rung
    // and the same recharge; the fiend is the tougher body, the hag is
    // the one with an action list.
    actions.push(&crate::actions::spells::CONJURE_FEY);
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
    // Imprisonment — the ninth-level abjuration that ends one creature
    // and asks for nothing else: one Wisdom save, no concentration, no
    // timer. Where Maze spends a level-8 slot *and* the caster's whole
    // concentration to remove somebody for ten rounds, this removes
    // them for the fight and leaves the concentration free. Against the
    // one enemy the party cannot beat, that is the purchase.
    // See `spells::IMPRISONMENT`.
    actions.push(&*crate::actions::spells::IMPRISONMENT);
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
    // lv2 **Darkness**: a warlock signature, and no longer a symmetric
    // blind. The Devil's Sight invocation in this template's `features`
    // set is what RAW pairs it with, and the pairing is the whole
    // combo: the sphere is heavy obscurement to everybody else and
    // clear air to the warlock standing in it. Goes on the warlock list
    // as part of their Pact of the Fiend / Pact of the Chain SRD
    // baseline.
    actions.push(&*crate::actions::spells::DARKNESS);
    // lv4 **Shadow of Moil** (XGtE evocation, concentration). Self-only
    // shadow wrap: 2d8 necrotic retaliation on every melee hit AND
    // attackers swing with disadvantage. Sibling to Fire Shield (radiant
    // tier, no attacker-debuff) on the self-shield lane — the necrotic
    // typing leans into the warlock's death-flavored kit.
    // The XGE / TCE lane on the warlock's RAW list. Four spells, and
    // the pact-magic chassis changes what each is worth: every slot a
    // warlock has is its highest, and they all come back on a short
    // rest, so the two that convert one cast into a whole turn's worth
    // of repeated bonus actions (Far Step, Blade of Disaster) are worth
    // strictly more here than on a wizard who is rationing.
    actions.push(&*crate::actions::spells::INTELLECT_FORTRESS);
    actions.push(&*crate::actions::spells::ENEMIES_ABOUND);
    actions.push(&*crate::actions::spells::FAR_STEP);
    actions.push(&*crate::actions::spells::BLADE_OF_DISASTER);
    actions.push(&*crate::actions::spells::SHADOW_OF_MOIL);
    // Gaseous Form — the trade: Resistance to the physical trio,
    // Immunity to Prone, Advantage on every physical save, and in
    // exchange a 10 ft speed and no attacking or casting at all. RAW
    // has it on the warlock list, where it reads as the escape hatch a
    // class with one slot needs.
    actions.push(&*crate::actions::spells::GASEOUS_FORM);
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
        //   - Lance of Lethargy: once on each of the warlock's turns, a
        //     landed beam takes 10 ft off the target's speed. Picked as
        //     the fifth invocation because of what it does *to* the
        //     fourth: Repelling Blast opens ten feet of gap and the
        //     lance takes away the ten feet that would have closed it
        //     again, which is the whole of the kiting warlock and the
        //     reason this pair is the one every table reaches for.
        //   - Eldritch Mind: advantage on Constitution saves to maintain
        //     concentration (read at the damage chokepoint).
        //   - Devil's Sight: sees normally in magical darkness, which
        //     is what turns the Darkness on this list from a symmetric
        //     blind into the warlock's own cover — the caster shoots
        //     out of a sphere nothing inside can see out of. Inert
        //     until the warlock actually casts it, so it costs the kit
        //     nothing on a lit board.
        // RAW a level-5 warlock picks 3 invocations; the kit pre-picks
        // these five since EB is the signature cantrip, Darkness is on
        // the list, and concentration-bound spells (Hex / Hunger of
        // Hadar) form the back half of the warlock's lockdown plan. Permanent passive
        // features — never consumed; the relevant cast / save sites
        // read them via `feature_available`.
        features: HashSet::from([
            crate::actions::class_features::AGONIZING_BLAST_TAG,
            crate::actions::class_features::REPELLING_BLAST_TAG,
            crate::actions::class_features::LANCE_OF_LETHARGY_TAG,
            crate::actions::class_features::ELDRITCH_MIND_TAG,
            crate::actions::class_features::DEVILS_SIGHT_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Shared "clone baseline WARLOCK_TEMPLATE + insert one subclass tag"
/// helper for every Otherworldly Patron subclass whose distinguishing
/// mechanical surface collapses to a single passive-feature tag added
/// on top of the baseline envelope. The per-subclass swaps are exactly
/// three axes:
///   1. Display `name` (e.g. `"Marid Warlock"`, `"Undying Warlock"`).
///   2. Map `glyph` (single character, per-subclass identity marker).
///   3. The one subclass feature `subclass_tag` inserted into the shared
///      `WARLOCK_TEMPLATE.features` set.
///
/// Users:
///   - **The Undying** (SCAG) — `ASPECT_OF_THE_MOON_TAG`
///     (`UNDYING_WARLOCK_TEMPLATE`).
///   - **The Great Old One** (PHB) — `ENTROPIC_WARD_TAG`
///     (`GREAT_OLD_ONE_WARLOCK_TEMPLATE`).
///   - **The Archfey** (PHB) — `BEGUILING_DEFENSES_TAG`
///     (`ARCHFEY_WARLOCK_TEMPLATE`).
///   - **The Celestial** (XGtE) — `RADIANT_SOUL_TAG`
///     (`CELESTIAL_WARLOCK_TEMPLATE`).
///   - **The Genie (Marid)** (TCE) — `MARID_ELEMENTAL_GIFT_TAG`
///     (`MARID_WARLOCK_TEMPLATE`).
///   - **The Genie (Dao)** (TCE) — `DAO_ELEMENTAL_GIFT_TAG`
///     (`DAO_WARLOCK_TEMPLATE`).
///   - **The Genie (Djinni)** (TCE) — `DJINNI_ELEMENTAL_GIFT_TAG`
///     (`DJINNI_WARLOCK_TEMPLATE`).
///
/// `FIEND_WARLOCK_TEMPLATE` is intentionally NOT a user — the Fiend
/// patron adds a full action (`HURL_THROUGH_HELL`), a struct-field
/// flag (`has_fiendish_resilience`), and three tags rather than one, so
/// it falls outside the "tag-only" envelope this helper covers.
///
/// The subclass-of pattern (single-tag layer on top of a shared
/// envelope) matches the way `subclass_barbarian_template` collapses
/// the Totem / Storm Herald barbarian family on the barbarian chassis
/// and `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` used to hand-roll a 10-line struct
/// literal before this helper folded the shape end-to-end. Adding a
/// new tag-only warlock subclass (a future Fathomless / Hexblade
/// tag-only variant, or the RAW Efreeti Genie patron for Fire
/// resistance) lands as a one-line entry.
fn subclass_warlock_template(
    name: &'static str,
    glyph: char,
    subclass_tag: &'static str,
) -> CreatureTemplate {
    // Delegate to the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — collapses the five-line "clone WARLOCK_TEMPLATE
    // features + insert one tag + rebuild struct" body every tag-only
    // warlock subclass template used to inline (before the class-scoped
    // helper landed) into a single method call. Sibling users on the
    // "clone base + insert one tag" cross-class helper lane:
    //   - `LIFE_CLERIC_TEMPLATE`   (Cleric baseline + `DISCIPLE_OF_LIFE_TAG`)
    //   - `NECROMANCY_WIZARD_TEMPLATE` (Wizard baseline + `INURED_TO_UNDEATH_TAG`)
    //   - `ABERRANT_MIND_SORCERER_TEMPLATE` (Sorcerer baseline + `PSYCHIC_DEFENSES_TAG`)
    //   - `DIVINE_SOUL_SORCERER_TEMPLATE`   (Sorcerer baseline + `FAVORED_BY_THE_GODS_TAG`)
    //
    // Keeping the class-scoped `subclass_warlock_template` wrapper on top
    // of the shared method preserves the "callsite explicitly names the
    // chassis" tell — every warlock subclass literal reads
    // `subclass_warlock_template(...)` next to the sibling Undying / Great
    // Old One / Archfey / etc. templates, rather than
    // `WARLOCK_TEMPLATE.with_subclass_tag(...)` which is functionally
    // identical but scatters the "warlock" identity across each callsite's
    // template-path prefix.
    WARLOCK_TEMPLATE.with_subclass_tag(name, glyph, subclass_tag)
}

/// Take the three Eldritch Blast invocations off a warlock that is
/// going to be swinging a sword instead.
///
/// **A warlock has a number of invocations, not all of them.** RAW hands
/// out five by level nine; the baseline chassis spends all five on the
/// cantrip (Agonizing Blast, Repelling Blast, Lance of Lethargy,
/// Eldritch Mind, Devil's Sight), which is the right build for a warlock
/// whose Action is always going to be a blast. A Pact of the Blade
/// warlock spends the same five somewhere else, and a template that
/// carried both lists would be a character sheet nobody could legally
/// write.
///
/// **It is also the difference between the blade being used and not.**
/// Agonizing Blast adds Charisma to *every beam*, which on a level-9
/// chassis is three beams — twelve points of flat damage on a cantrip
/// that costs nothing. Against that, the pact weapon loses the AI's
/// lane comparison outright, and a warlock with three swings a turn
/// conjures its weapon in round one and never once swings it. The
/// Fathomless template makes the same argument one invocation at a time
/// for the same reason: an invocation that works against the rest of
/// the build is worse than an empty slot.
///
/// Repelling Blast is the sharpest case, and it would be wrong even if
/// the damage worked out: it shoves the target ten feet away, and every
/// other feature on a blade warlock's sheet is priced on standing next
/// to something.
fn drop_the_blast_invocations(features: &mut HashSet<&'static str>) {
    features.remove(crate::actions::class_features::AGONIZING_BLAST_TAG);
    features.remove(crate::actions::class_features::REPELLING_BLAST_TAG);
    features.remove(crate::actions::class_features::LANCE_OF_LETHARGY_TAG);
}

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
    // SRD 5.2's **Pact of the Blade** and **Thirsting Blade** — the
    // conjured weapon and the second swing with it. On this patron
    // because Dark One's Blessing is the one feature in the warlock's
    // whole roster that pays for *finishing* creatures: temp HP every
    // time one drops. A blaster collects that occasionally; a warlock
    // standing in contact with a 1d8 Charisma weapon and two swings a
    // turn collects it as a rhythm, and the temp HP is what makes
    // standing there survivable on a d8 hit die. The three features
    // are one build, and this is the chassis that was missing two
    // thirds of it.
    actions.push(&*crate::actions::class_features::CONJURE_PACT_WEAPON);
    actions.push(&crate::actions::class_features::PACT_WEAPON);
    // **Eldritch Smite** on top of them, because this is the patron
    // that can afford it. RAW prices the smite in Pact Magic slots, and
    // a warlock has four of those a fight; spending one is spending a
    // Hold Monster. What makes it worth spending here is the other end
    // of the exchange: Dark One's Blessing pays temp HP when a creature
    // *drops*, and 2d8 of force plus a knockdown on the swing that was
    // going to be close is how a creature drops. The invocation and the
    // patron feature are the same plan read from both ends.
    actions.push(&*crate::actions::class_features::ELDRITCH_SMITE);
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::ELDRITCH_SMITE_TAG);
    drop_the_blast_invocations(&mut features);
    features.insert(crate::actions::class_features::DARK_ONES_BLESSING_TAG);
    features.insert(crate::actions::class_features::DARK_ONES_OWN_LUCK_TAG);
    features.insert(crate::actions::class_features::HURL_THROUGH_HELL_TAG);
    features.insert(crate::actions::class_features::PACT_OF_THE_BLADE_TAG);
    features.insert(crate::actions::class_features::THIRSTING_BLADE_TAG);
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
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — folds the "clone baseline warlock envelope + insert one
    // subclass tag" shape end-to-end. The Undying patron's mechanical
    // surface collapses to a single passive-feature tag
    // (`ASPECT_OF_THE_MOON_TAG`) on top of the shared envelope, so the
    // whole subclass template lands as a three-argument helper call
    // (name, glyph, tag) rather than an inline clone-and-insert.
    subclass_warlock_template(
        "Undying Warlock",
        'U',
        crate::actions::class_features::ASPECT_OF_THE_MOON_TAG,
    )
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
///      A hypothetical Light Cleric / Great Old One Warlock multiclass
///      would carry BOTH cohort rows; the iterator returns after the
///      first firing so at most one per-rest charge burns per incoming
///      attack, matching the "at most one add-die per save" ordering
///      semantics on the failed-save recovery cohort.
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
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Great Old One patron's mechanical surface collapses
    // to one passive-feature tag (`ENTROPIC_WARD_TAG`, registered in
    // `SHORT_REST_FEATURES` for per-rest charge refresh) on top of the
    // shared envelope. 'O' — distinct from baseline warlock 'L', Fiend
    // 'F', and Undying 'U'; 'O' for the alien "Old One" flavor.
    let mut goo = subclass_warlock_template(
        "Great Old One Warlock",
        'O',
        crate::actions::class_features::ENTROPIC_WARD_TAG,
    );
    // SRD 5.2's **Boon of Truesight**, and the Great Old One is who it
    // belongs to: a patron whose every other feature is about perceiving
    // what was not meant to be perceived. Sixty feet of seeing through
    // an illusion, a Mirror Image, a Blur, and the dark — granted into
    // the senses set at instantiation by `senses_with_feature_grants`
    // rather than written onto this template's own `senses`, so the
    // sight belongs to the feat and leaves with it. See
    // `crate::actions::feats::BOON_OF_TRUESIGHT_TAG`.
    goo.features
        .insert(crate::actions::feats::BOON_OF_TRUESIGHT_TAG);
    goo
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
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Archfey patron's mechanical surface collapses to one
    // passive-feature tag (`BEGUILING_DEFENSES_TAG`, Charmed install-
    // immunity) on top of the shared envelope. 'A' — distinct from
    // baseline warlock 'L', Fiend 'F', Undying 'U', and Great Old One
    // 'O'; 'A' for the "Archfey" identity.
    let mut archfey = subclass_warlock_template(
        "Archfey Warlock",
        'A',
        crate::actions::class_features::BEGUILING_DEFENSES_TAG,
    );
    // SRD 5.2's Eldritch Invocation **Witch Sight**: "You have Truesight
    // with a range of 30 feet." The Archfey is who it belongs to — a
    // patron whose whole court runs on glamour, and the one warlock for
    // whom "what is actually standing there" is a question with a
    // different answer. Thirty feet of it reaches exactly as far as the
    // melee band and one step past, so it is spent on a Blur, a Mirror
    // Image and a Displacer Beast's flicker rather than on scouting.
    //
    // Granted into the senses set at instantiation by
    // `senses_with_feature_grants`, so the sight belongs to the
    // invocation and leaves with it. See
    // `crate::actions::class_features::WITCH_SIGHT_TAG`.
    archfey
        .features
        .insert(crate::actions::class_features::WITCH_SIGHT_TAG);
    archfey
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
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Celestial patron's mechanical surface collapses to
    // one passive-feature tag (`RADIANT_SOUL_TAG`, radiant resistance)
    // on top of the shared envelope. 'C' — distinct from baseline
    // warlock 'L', Fiend 'F', Undying 'U', Great Old One 'O', and
    // Archfey 'A'; 'C' for the "Celestial" identity.
    let mut celestial = subclass_warlock_template(
        "Celestial Warlock",
        'C',
        crate::actions::class_features::RADIANT_SOUL_TAG,
    );
    // SRD 5.2's Eldritch Invocation **Gift of the Protectors**: a page
    // of names, and the first ally on it to fall drops to 1 hit point
    // instead. The Celestial is the patron it belongs to — every other
    // feature RAW gives this pact is about keeping the party upright
    // (Healing Light, Celestial Resilience, Searing Vengeance), and
    // this chassis ships none of them, so the page is the one place
    // the subclass's whole reason for existing reaches the table.
    //
    // One charge, refreshed on a long rest, spent on whoever falls
    // first — see `crate::actions::class_features::
    // GIFT_OF_THE_PROTECTORS_TAG` and the intercept that reads it.
    celestial
        .features
        .insert(crate::actions::class_features::GIFT_OF_THE_PROTECTORS_TAG);
    celestial
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
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Marid Genie patron's mechanical surface collapses to
    // one passive-feature tag (`MARID_ELEMENTAL_GIFT_TAG`, cold
    // resistance) on top of the shared envelope. 'M' — distinct from
    // baseline warlock 'L', Fiend 'F', Undying 'U', Great Old One 'O',
    // Archfey 'A', and Celestial 'C'; 'M' for the "Marid" identity.
    subclass_warlock_template(
        "Marid Warlock",
        'M',
        crate::actions::class_features::MARID_ELEMENTAL_GIFT_TAG,
    )
});

/// Dao Warlock — Otherworldly Patron **The Genie (Dao)** subclass build
/// (TCE). Identical envelope to the baseline `WARLOCK_TEMPLATE` (CHA-
/// primary half-caster with Pact Magic, Eldritch Blast + Hex + Witch
/// Bolt at will, Agonizing / Repelling / Eldritch Mind invocations) with
/// one subclass feature layered on: **Elemental Gift** (Genie subclass
/// level 6, Dao variant) — passive **resistance to bludgeoning damage**.
///
/// The signature "the dao's stony grip shields me from clubs" tell —
/// where a baseline Warlock eats a maul / warhammer / greatclub swing
/// clean, the Dao Warlock halves the incoming bludgeoning damage. Read
/// at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to the Marid Elemental Gift's cold-halving
/// row, Radiant Soul's radiant-halving row, and Fiendish Resilience's
/// fire-halving row — same lane, different patron flavor and different
/// damage axis. First user of the Bludgeoning slot on the passive
/// typed-resistance lane; the physical damage trio (Bludgeoning /
/// Piercing / Slashing) was uncovered by any passive typed-resistance
/// cohort row until this template landed.
///
/// RAW's Genie Warlock (Dao variant) picks up other features not
/// shipped on this template — **Genie's Vessel** (lv1: bonus-action
/// bottle a creature into the dao vessel; a save-and-condition-install
/// resource lane that needs a per-warlock vessel tracker), **Bottled
/// Respite** (lv6: 10-min in-vessel short rest; sits outside combat),
/// **Sanctuary Vessel** (lv10: refresh temp HP on party short rest inside
/// the vessel), and **Limited Wish** (lv14 capstone: cast any lv6-or-lower
/// spell on a 1d4-day cooldown). Only the lv6 Elemental Gift passive
/// has a mechanical surface on the CR-4 chassis that plugs cleanly into
/// the shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `MARID_WARLOCK_TEMPLATE` ships only the Elemental Gift resistance
/// half of its RAW kit.
///
/// Sibling on the passive typed-resistance patron lane to:
///   - **Elemental Gift (Marid)** (Marid Warlock lv6, TCE): Cold,
///     sibling patron on the "Otherworldly Patron: The Genie" family —
///     same feature name, different damage axis (Bludgeoning vs. Cold).
///     The two never legally co-occur on a single build (RAW: one
///     genie kind per warlock).
///   - **Radiant Soul** (Celestial Warlock lv6, XGtE): Radiant.
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire.
///
/// Sibling on the Otherworldly Patron subclass lane to
/// `MARID_WARLOCK_TEMPLATE` (Marid genie kind — Cold),
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck +
/// Fiendish Resilience — the fiery kill-focused build),
/// `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
/// build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the alien-
/// awareness reactive build), `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling
/// Defenses — the Charmed-bounce build), and `CELESTIAL_WARLOCK_TEMPLATE`
/// (Radiant Soul — the radiant-resistance build).
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the
/// same reason `MARID_WARLOCK_TEMPLATE` ships Elemental Gift (RAW lv6),
/// `CELESTIAL_WARLOCK_TEMPLATE` ships Radiant Soul (RAW lv6),
/// `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward (RAW lv6), and
/// `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience (RAW lv10): class
/// templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Glyph 'D' — collides with the Draconic Sorcerer 'D' glyph, but the
/// two never co-occur on a single team and the CHA-caster / STR-caster
/// axes disambiguate them on the "glyph as encounter tell" pattern. If a
/// hypothetical cross-team mix ever ships (Dao warlock on team A,
/// Draconic sorcerer on team B), the team-color tint disambiguates them
/// on the map. 'D' for the "Dao" identity (the warlock as a mortal
/// bonded to an earth-and-stone genie sovereign of the Elemental Plane
/// of Earth).
pub static DAO_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Dao Genie patron's mechanical surface collapses to
    // one passive-feature tag (`DAO_ELEMENTAL_GIFT_TAG`, bludgeoning
    // resistance) on top of the shared envelope. 'D' — collides with
    // the Draconic Sorcerer 'D', but the two subclass templates never
    // legally co-occur on a single team (one glyph per team-color-and-
    // team-id combo suffices to disambiguate them in a mixed encounter).
    // Distinct from baseline warlock 'L', Fiend 'F', Undying 'U', Great
    // Old One 'O', Archfey 'A', Celestial 'C', and Marid 'M'; 'D' for
    // the "Dao" identity.
    subclass_warlock_template(
        "Dao Warlock",
        'D',
        crate::actions::class_features::DAO_ELEMENTAL_GIFT_TAG,
    )
});

/// Djinni Warlock — Otherworldly Patron **The Genie (Djinni)** subclass
/// build (TCE). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex + Witch
/// Bolt at will, Agonizing / Repelling / Eldritch Mind invocations) with
/// one subclass feature layered on: **Elemental Gift** (Genie subclass
/// level 6, Djinni variant) — passive **resistance to thunder damage**.
///
/// The signature "the djinni's storm-song blunts the thundercrack" tell
/// — where a baseline Warlock eats a Shatter / Thunderwave / Thunderclap
/// zap clean, the Djinni Warlock halves the incoming thunder damage.
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to the Marid Elemental Gift's cold row, the
/// Dao Elemental Gift's bludgeoning row, Radiant Soul's radiant row,
/// Fiendish / Draconic Resilience's fire rows, Heart of the Storm's
/// lightning + thunder row, and Psychic Defenses' psychic row — same
/// lane, different patron flavor, different damage axis.
///
/// Third of the four RAW Genie variants to ship on the warlock family
/// — completes the physical-element trio (Cold / Bludgeoning / Thunder)
/// alongside `MARID_WARLOCK_TEMPLATE` (Cold, water/ice) and
/// `DAO_WARLOCK_TEMPLATE` (Bludgeoning, earth/stone). The remaining
/// **Efreeti** (Fire) variant is a semantic duplicate of Fiendish
/// Resilience on the resistance axis (both grant Fire) and is left as
/// future work — the four-genie completeness is a taxonomic goal, not
/// a mechanical-coverage goal.
///
/// RAW's Genie Warlock (Djinni variant) picks up other features not
/// shipped on this template — **Genie's Vessel** (lv1: bonus-action
/// bottle a creature into the djinni vessel; a save-and-condition-install
/// resource lane that needs a per-warlock vessel tracker), **Bottled
/// Respite** (lv6: 10-min in-vessel short rest; sits outside combat),
/// **Sanctuary Vessel** (lv10: refresh temp HP on party short rest inside
/// the vessel), and **Limited Wish** (lv14 capstone: cast any lv6-or-lower
/// spell on a 1d4-day cooldown). Only the lv6 Elemental Gift passive has
/// a mechanical surface on the CR-4 chassis that plugs cleanly into the
/// shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `MARID_WARLOCK_TEMPLATE` and `DAO_WARLOCK_TEMPLATE` each ship only the
/// Elemental Gift resistance half of their RAW kit.
///
/// Sibling on the passive typed-resistance patron lane to:
///   - **Elemental Gift (Marid)** (Marid Warlock lv6, TCE): Cold.
///   - **Elemental Gift (Dao)** (Dao Warlock lv6, TCE): Bludgeoning.
///     All three Genie variants share the "one feature tag drives one
///     cohort row" declarative-table shape, different damage axis. Any
///     two never legally co-occur on a single build (RAW: one genie
///     kind per warlock).
///   - **Radiant Soul** (Celestial Warlock lv6, XGtE): Radiant.
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire.
///   - **Heart of the Storm** (Storm Sorcerer lv6): Lightning + Thunder
///     — overlaps this row on the Thunder axis, but the two never co-
///     occur on a single chassis (Warlock vs. Sorcerer). A hypothetical
///     multiclass carrier caps at a single /2 per Thunder hit via the
///     "one halving per damage instance" rule.
///
/// Sibling on the Otherworldly Patron subclass lane to
/// `MARID_WARLOCK_TEMPLATE` (Marid genie kind — Cold),
/// `DAO_WARLOCK_TEMPLATE` (Dao genie kind — Bludgeoning),
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck +
/// Fiendish Resilience — the fiery kill-focused build),
/// `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
/// build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the alien-
/// awareness reactive build), `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling
/// Defenses — the Charmed-bounce build), and `CELESTIAL_WARLOCK_TEMPLATE`
/// (Radiant Soul — the radiant-resistance build).
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the same
/// reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` ship
/// Elemental Gift (RAW lv6): class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'J' — distinct from baseline warlock 'L', Fiend 'F', Undying
/// 'U', Great Old One 'O', Archfey 'A', Celestial 'C', Marid 'M', and
/// Dao 'D'; 'J' for the "Djinni" identity (the warlock as a mortal
/// bonded to an air-and-storm genie sovereign of the Elemental Plane
/// of Air).
pub static DJINNI_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Djinni Genie patron's mechanical surface collapses
    // to one passive-feature tag (`DJINNI_ELEMENTAL_GIFT_TAG`, thunder
    // resistance) on top of the shared envelope. 'J' for the "Djinni"
    // identity — distinct from baseline warlock 'L', Fiend 'F', Undying
    // 'U', Great Old One 'O', Archfey 'A', Celestial 'C', Marid 'M', and
    // Dao 'D'.
    subclass_warlock_template(
        "Djinni Warlock",
        'J',
        crate::actions::class_features::DJINNI_ELEMENTAL_GIFT_TAG,
    )
});

/// Efreeti Warlock — Otherworldly Patron **The Genie (Efreeti)** subclass
/// build (TCE). Identical envelope to the baseline `WARLOCK_TEMPLATE`
/// (CHA-primary half-caster with Pact Magic, Eldritch Blast + Hex + Witch
/// Bolt at will, Agonizing / Repelling / Eldritch Mind invocations) with
/// one subclass feature layered on: **Elemental Gift** (Genie subclass
/// level 6, Efreeti variant) — passive **resistance to fire damage**.
///
/// The signature "the efreeti's flame ward blunts the blaze" tell — where
/// a baseline Warlock eats a Fireball / Wall of Fire / Burning Hands hit
/// clean, the Efreeti Warlock halves the incoming fire damage. Read at
/// the shared `PASSIVE_TYPED_RESISTANCES` cohort in `actor_template.rs`
/// next to the Marid Elemental Gift's cold row, the Dao Elemental Gift's
/// bludgeoning row, the Djinni Elemental Gift's thunder row, Radiant
/// Soul's radiant row, Fiendish / Draconic Resilience's fire rows, Storm
/// Soul (Sea / Desert / Tundra)'s Lightning / Fire / Cold rows, Heart of
/// the Storm's lightning + thunder row, Psychic Defenses' psychic row,
/// and Inured to Undeath's necrotic row — same halving rule, different
/// patron flavor and different damage axis.
///
/// Fourth (and final) of the four RAW Genie variants to ship on the
/// warlock family — completes the four-genie taxonomic set alongside
/// `MARID_WARLOCK_TEMPLATE` (Cold, water/ice), `DAO_WARLOCK_TEMPLATE`
/// (Bludgeoning, earth/stone), and `DJINNI_WARLOCK_TEMPLATE` (Thunder,
/// sky/storm). Semantic duplicate on the resistance axis of
/// `FIEND_WARLOCK_TEMPLATE`'s Fiendish Resilience (Fire, via the
/// struct-field `has_fiendish_resilience` flag) and
/// `DRACONIC_SORCERER_TEMPLATE`'s Draconic Resilience (Fire, via the
/// struct-field `has_draconic_resilience` flag) — all three cover the
/// Fire axis. The duplication is a **taxonomic completeness** grant,
/// not a mechanical-coverage grant: the Efreeti variant lands so the
/// four-genie Genie patron family reads as a full quadrant on the map
/// even though the Fire axis is already covered by two other passive-
/// resistance rows on distinct chassis. The three Fire-resistance
/// rows never legally co-occur on a single build (Warlock Fiend vs.
/// Warlock Efreeti vs. Sorcerer Draconic subclass), and a hypothetical
/// multiclass carrier caps at a single /2 per Fire hit under the "one
/// halving per damage instance" rule — the triple coverage is a
/// taxonomic tell rather than a stacking bug. Also overlaps the Fire
/// axis with Storm Soul (Desert) on the Barbarian chassis — same "one
/// halving per damage instance" cap applies.
///
/// RAW's Genie Warlock (Efreeti variant) picks up other features not
/// shipped on this template — **Genie's Vessel** (lv1: bonus-action
/// bottle a creature into the efreeti vessel; a save-and-condition-install
/// resource lane that needs a per-warlock vessel tracker), **Bottled
/// Respite** (lv6: 10-min in-vessel short rest; sits outside combat),
/// **Sanctuary Vessel** (lv10: refresh temp HP on party short rest inside
/// the vessel), and **Limited Wish** (lv14 capstone: cast any lv6-or-lower
/// spell on a 1d4-day cooldown). Only the lv6 Elemental Gift passive has
/// a mechanical surface on the CR-4 chassis that plugs cleanly into the
/// shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` each ship only the Elemental Gift resistance
/// half of their RAW kit.
///
/// Sibling on the passive typed-resistance patron lane to:
///   - **Elemental Gift (Marid)** (Marid Warlock lv6, TCE): Cold.
///   - **Elemental Gift (Dao)** (Dao Warlock lv6, TCE): Bludgeoning.
///   - **Elemental Gift (Djinni)** (Djinni Warlock lv6, TCE): Thunder.
///     All four Genie variants share the "one feature tag drives one
///     cohort row" declarative-table shape, different damage axis. Any
///     two never legally co-occur on a single build (RAW: one genie
///     kind per warlock).
///   - **Radiant Soul** (Celestial Warlock lv6, XGtE): Radiant.
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire — same
///     damage axis, different patron flavor. Never legally co-occur.
///   - **Draconic Resilience** (Draconic Sorcerer lv6): Fire — same
///     damage axis, different chassis.
///   - **Storm Soul (Desert)** (Desert Storm Herald Barbarian lv6):
///     Fire — same damage axis, different chassis.
///
/// Sibling on the Otherworldly Patron subclass lane to
/// `MARID_WARLOCK_TEMPLATE` (Marid — Cold),
/// `DAO_WARLOCK_TEMPLATE` (Dao — Bludgeoning),
/// `DJINNI_WARLOCK_TEMPLATE` (Djinni — Thunder),
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck +
/// Fiendish Resilience — the fiery kill-focused build),
/// `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
/// build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the alien-
/// awareness reactive build), `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling
/// Defenses — the Charmed-bounce build), and `CELESTIAL_WARLOCK_TEMPLATE`
/// (Radiant Soul — the radiant-resistance build).
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the same
/// reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6): class
/// templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Glyph 'Y' — distinct from baseline warlock 'L', Fiend 'F', Undying
/// 'U', Great Old One 'O', Archfey 'A', Celestial 'C', Marid 'M', Dao
/// 'D', and Djinni 'J'; 'Y' for the "Efreetiy" identity (the warlock as
/// a mortal bonded to a fire-and-flame genie sovereign of the Elemental
/// Plane of Fire). 'E' would collide with the Elk Totem Barbarian on
/// the barbarian family; a hypothetical cross-team mix (Efreeti Warlock
/// on team A, Elk Totem Barbarian on team B) would then need the team-
/// color tint to disambiguate them on the map, which is a per-encounter
/// disambiguation cost we avoid by picking a fresh glyph up front.
pub static EFREETI_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `subclass_warlock_template`
    // helper — the Efreeti Genie patron's mechanical surface collapses
    // to one passive-feature tag (`EFREETI_ELEMENTAL_GIFT_TAG`, fire
    // resistance) on top of the shared envelope. 'Y' for the "Efreetiy"
    // identity — distinct from baseline warlock 'L', Fiend 'F', Undying
    // 'U', Great Old One 'O', Archfey 'A', Celestial 'C', Marid 'M',
    // Dao 'D', and Djinni 'J'.
    subclass_warlock_template(
        "Efreeti Warlock",
        'Y',
        crate::actions::class_features::EFREETI_ELEMENTAL_GIFT_TAG,
    )
});

/// Hexblade Warlock — Otherworldly Patron **The Hexblade** (XGtE). The
/// only patron on the warlock chassis whose build is martial rather than
/// purely a spell-list variation, which is why it is the one that
/// doesn't collapse to `subclass_warlock_template`'s single-tag shape.
///
/// Four changes to the baseline envelope, and they interlock:
///
///   - **Hex Warrior** (lv1) — the pact blade, `PACT_BLADE`: a 1d8
///     slashing weapon keyed to **Charisma**. On a CHA-18 chassis that
///     is +7 to hit where the baseline warlock's STR-8 dagger is +1.
///     Ships as a weapon rather than as an engine flag because
///     `SimpleWeapon` already carries its own ability — a CHA-typed
///     blade *is* the feature, and the baseline dagger stays on the
///     template so the STR-8 fallback is still visible for what it is.
///   - **Medium armor and shield proficiency** (lv1) — AC 12 → 18. The
///     RAW clause nobody quotes, and the one that actually lets the
///     subclass exist: a d8-hit-die caster in the front rank at AC 12
///     is a caster who dies in the front rank.
///   - **Hexblade's Curse** (lv1) — `HEXBLADES_CURSE`, bonus action,
///     once per short rest. +proficiency to damage against one creature,
///     crits on 19-20 against it, and `level + CHA` hit points back when
///     it drops. See `HEXBLADES_CURSE_TAG`.
///   - **Armor of Hexes** (lv10) — the cursed quarry's attacks on the
///     hexblade miss on a d6 of 4+. See `ARMOR_OF_HEXES_TAG`.
///
/// **How it plays differently from the other ten patrons.** Every other
/// warlock in the engine wins at range and loses on contact; the
/// Hexblade wants contact. The curse rewards hitting one creature over
/// and over, the pact blade is the only thing on the chassis that swings
/// at a martial's accuracy, AC 18 is what survives the round it takes to
/// close, and Armor of Hexes only ever protects against the one creature
/// the curse named — which is to say, all four features pay out on the
/// same play: pick a target, walk to it, and stay there. The baseline
/// warlock's Eldritch Blast is still on the template and still good, and
/// the curse's damage bonus rides it too (both attack-damage chokepoints
/// sum `curse_damage_bonus`), so the build degrades gracefully into a
/// blaster when closing isn't safe.
///
/// **Deliberately not shipped.** *Accursed Specter* (lv6) — a slain
/// humanoid rises to serve until the next long rest — belongs on the
/// summon lane rather than here, and RAW's *Master of Hexes* (lv14)
/// moves the curse on the quarry's death, which is a second decision
/// point on a feature whose whole cost is the first one.
///
/// Glyph 'X' for the hex — distinct from baseline warlock 'L', Fiend
/// 'F', Undying 'U', Great Old One 'O', Archfey 'A', Celestial 'C',
/// Marid 'M', Dao 'D', Djinni 'J', and Efreeti 'Y'.
pub static HEXBLADE_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = WARLOCK_TEMPLATE.actions.clone();
    actions.push(&crate::actions::monster_attacks::PACT_BLADE);
    actions.push(&*crate::actions::class_features::HEXBLADES_CURSE);
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::HEXBLADES_CURSE_TAG);
    features.insert(crate::actions::class_features::ARMOR_OF_HEXES_TAG);
    CreatureTemplate {
        name: "Hexblade Warlock",
        glyph: 'X',
        // Medium armor (half plate, 15) + shield (+2) + DEX 14 capped at
        // +1 by medium armor = 18. The single largest stat departure any
        // warlock subclass makes from the baseline chassis, and the
        // reason the rest of the kit is playable.
        ac: 18,
        actions,
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Undead Warlock — Otherworldly Patron **The Undead** subclass build
/// (VRGtR). One subclass feature, **Form of Dread** (lv1), and it is
/// three clauses that all point the same way.
///
/// As a bonus action the warlock spends one minute wearing something
/// closer to their patron's shape: `1d10 + 9` temporary hit points,
/// immunity to Frightened, and — once on each of their turns — a
/// creature they hit must make a Wisdom save against their spell DC or
/// be Frightened of them until the end of the warlock's next turn.
///
/// The two fear clauses are the point. Every other Frightened source in
/// the engine is a burst the caster fires and then lives with: Dreadful
/// Aspect, Conquering Presence, Abjure Enemy, the dragon's Frightful
/// Presence. Form of Dread turns fear into an attrition tool — every
/// turn the warlock connects, someone else picks it up — and the
/// immunity is what makes standing in the middle of that safe. The temp
/// HP is the third leg: the form is about being the most frightening
/// thing in the room for long enough that it matters.
///
/// Not a `subclass_warlock_template` user. That helper covers the
/// tag-only patrons whose whole surface is a passive flag, and Form of
/// Dread is an action plus a tag — the same reason the Fiend patron
/// sits outside it.
///
/// RAW's later Undead features aren't shipped. **Grave Touched** (lv6 —
/// swap a damage type for necrotic and add a die on a Form of Dread
/// turn) needs a per-hit damage-type override the weapon path doesn't
/// expose; **Necrotic Husk** (lv10 — necrotic immunity plus a
/// death-burst) and **Spirit Projection** (lv14) are both well past this
/// chassis.
///
/// Glyph 'W' — for the **W**ight the form resembles. Distinct from
/// baseline warlock 'L', Fiend 'F', Undying 'U', Great Old One 'O',
/// Archfey 'A', Celestial 'C', Marid 'M', Dao 'D', Djinni 'J', Efreeti
/// 'Y' and Hexblade 'X'.
pub static UNDEAD_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{FORM_OF_DREAD, FORM_OF_DREAD_TAG};
    let mut actions = WARLOCK_TEMPLATE.actions.clone();
    actions.push(&*FORM_OF_DREAD);
    // SRD 5.2's **Pact of the Blade** with both blade invocations on
    // top of it — **Thirsting Blade** and **Devouring Blade**, three
    // swings a turn with the conjured weapon.
    //
    // On this patron because Form of Dread is priced per turn and paid
    // per *hit*: "once on each of your turns when you hit a creature,
    // that creature must make a Wisdom save or be Frightened." A
    // warlock with one swing a turn cashes that clause when the swing
    // lands and not otherwise; a warlock with three cashes it nearly
    // every turn of the fight, which is the difference between a fear
    // effect and an attrition engine. The two invocations are the only
    // thing in the book that makes the subclass's own arithmetic work.
    //
    // Devouring Blade's RAW prerequisite is Thirsting Blade and both
    // ship here; see `extra_attack_swings` for why the pair combines
    // with a `max` rather than a sum.
    actions.push(&*crate::actions::class_features::CONJURE_PACT_WEAPON);
    actions.push(&crate::actions::class_features::PACT_WEAPON);
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(FORM_OF_DREAD_TAG);
    features.insert(crate::actions::class_features::PACT_OF_THE_BLADE_TAG);
    features.insert(crate::actions::class_features::THIRSTING_BLADE_TAG);
    features.insert(crate::actions::class_features::DEVOURING_BLADE_TAG);
    // **Lifedrinker** with them, and it is the invocation the build
    // needs rather than the one it wants. Three swings a turn is three
    // turns spent inside an enemy's reach on a d8 hit die, and RAW's
    // answer to that is this: a die back on the first hit of every
    // turn. It also rides the pact weapon's damage menu for free — the
    // rider inherits whichever of Necrotic / Psychic / Radiant /
    // Slashing the weapon chose, which is RAW's own three-way choice
    // already made against the same target. See `LIFEDRINKER_TAG`.
    features.insert(crate::actions::class_features::LIFEDRINKER_TAG);
    drop_the_blast_invocations(&mut features);
    CreatureTemplate {
        name: "Undead Warlock",
        glyph: 'W',
        actions,
        features,
        ..WARLOCK_TEMPLATE.clone()
    }
});

/// Fathomless Warlock — Otherworldly Patron **The Fathomless** subclass
/// build (TCE), the thirteenth warlock on the roster, and the only one
/// whose signature feature is a square of the map.
///
/// Three subclass features ship:
///
///   - **Tentacle of the Deep** (lv1, bonus action, once per short
///     rest) — a rooted spectral limb rises beside the warlock. It has
///     10 hit points, never moves, and lashes for `1d8` cold at 10 ft,
///     taking ten feet of speed off whatever it hits.
///   - **Guardian Coil** (lv6, reaction, once per short rest) — the
///     warlock spends their reaction to shave `1d8` off damage taken by
///     anyone within 10 ft of the tentacle, themselves included.
///   - **Oceanic Soul** (lv10, passive) — cold resistance.
///
/// The first two are the same feature read from opposite ends, and the
/// thing they have in common is that neither measures anything from the
/// warlock. The slam's reach is from the tentacle; the coil's radius is
/// from the tentacle; the warlock can be thirty feet behind a wall.
/// That is a shape nothing else in the engine has — every other reactive
/// shield on the roster (Uncanny Dodge, Parry, Interception, Warding
/// Maneuver, Protective Field) is a radius drawn around one of the two
/// creatures already in the swing, and Guardian Coil is drawn around a
/// third body that is in neither role. It is the reason
/// `ClampScope::NearBeacon` exists.
///
/// So the decision the subclass poses is placement, once, on round one,
/// and then living with it: the tentacle cannot walk. Put it in the
/// doorway and everything that comes through arrives slow and takes
/// less from the warlock's party for it; put it beside the fighter and
/// the fighter is meaningfully harder to kill; put it badly and it is
/// ten hit points of nothing standing in a corner.
///
/// **Compare the Wildfire Druid**, the other summon-anchored build on
/// the roster and the other user of the beacon-tag idiom. Enhanced Bond
/// pays the druid for keeping the spirit near *themselves* — the summon
/// follows the summoner. Guardian Coil pays the warlock for putting the
/// tentacle somewhere they are *not* — the summon replaces the
/// summoner. Same machinery, opposite tactical instruction.
///
/// **Stats** inherit the baseline warlock wholesale — CHA 18, Pact
/// Magic, Eldritch Blast, the invocation suite — with cold resistance
/// layered on. The subclass adds a body and a reaction, not a different
/// warlock, which is the honest way to ship it.
///
/// RAW features not shipped: **Grasping Tentacles** (lv10 — a free
/// casting of Evard's Black Tentacles, which the engine has but which
/// would need a per-rest free-cast lane no other template wants),
/// **Fathomless Plunge** (lv14 — a mass teleport that needs a
/// destination picker the AI has no channel to answer, the same wall
/// the Eldritch Knight's Arcane Charge runs into), and **Cold
/// Immunity** (lv14).
///
/// Glyph 'T' — for the **T**entacle, paired with the tentacle's own
/// lowercase 't'. Distinct from baseline warlock 'L', Fiend 'F',
/// Undying 'U', Great Old One 'O', Archfey 'A', Celestial 'C', Marid
/// 'M', Dao 'D', Djinni 'J', Efreeti 'Y', Hexblade 'X' and Undead 'W'.
pub static FATHOMLESS_WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        GUARDIAN_COIL_TAG, SUMMON_TENTACLE_OF_THE_DEEP, TENTACLE_OF_THE_DEEP_TAG,
    };
    use crate::engine::types::{DamageModifier, DamageType};
    // Not a `subclass_warlock_template` user: that helper covers the
    // patrons whose whole surface is one passive flag, and this is an
    // action plus two tags plus a resistance — the same reason the Fiend
    // and Undead patrons sit outside it.
    let mut actions = WARLOCK_TEMPLATE.actions.clone();
    actions.push(&SUMMON_TENTACLE_OF_THE_DEEP);
    let mut features = WARLOCK_TEMPLATE.features.clone();
    features.insert(TENTACLE_OF_THE_DEEP_TAG);
    features.insert(GUARDIAN_COIL_TAG);
    // The one invocation swap on the roster, and the only warlock whose
    // blast pulls rather than pushes. **Grasp of Hadar** replaces
    // **Repelling Blast** — see `GRASP_OF_HADAR_TAG` for why holding
    // both is meaningless rather than merely redundant.
    //
    // It is the subclass's own instruction, not flavour. Guardian Coil
    // pays this warlock for standing somewhere the tentacle is not, and
    // the tentacle is ten hit points that cannot walk: everything it is
    // worth depends on enemies being *next to it*. A blast that shoves
    // them ten feet further away is the subclass working against itself.
    // A blast that drags them four tiles toward the warlock — who is
    // stationed across the coil from the tentacle — is the summon's
    // whole plan carried out by the cantrip.
    features.remove(crate::actions::class_features::REPELLING_BLAST_TAG);
    features.insert(crate::actions::class_features::GRASP_OF_HADAR_TAG);
    // SRD 5.2's Eldritch Invocation **Gift of the Depths**: "You can
    // breathe underwater, and you gain a Swim Speed equal to your
    // Speed." The Fathomless patron is the one warlock the sentence was
    // written for, and it is the second invocation on the roster whose
    // value is entirely a property of the board — a fight with no water
    // on it never notices, and a fight in a flooded room hands this
    // chassis a movement lane and a working dagger while everybody else
    // wades. See `crate::actions::class_features::GIFT_OF_THE_DEPTHS_TAG`.
    features.insert(crate::actions::class_features::GIFT_OF_THE_DEPTHS_TAG);
    CreatureTemplate {
        name: "Fathomless Warlock",
        glyph: 'T',
        actions,
        features,
        // 5e **Oceanic Soul** (subclass level 10): resistance to cold
        // damage. Ships on this chassis above its strict RAW gate for
        // the reason every subclass template does — class templates
        // target a balanced playable level, not lockstep progression.
        // Thematically the same water the tentacle is made of, and
        // mechanically the sibling of the Draconic Sorcerer's fire
        // resistance and the Storm Sorcerer's lightning / thunder pair
        // on the `PASSIVE_TYPED_RESISTANCES` lane.
        damage_modifiers: std::collections::HashMap::from([(
            DamageType::Cold,
            DamageModifier::Resistance,
        )]),
        ..WARLOCK_TEMPLATE.clone()
    }
});
