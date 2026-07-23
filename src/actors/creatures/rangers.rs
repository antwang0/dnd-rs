use crate::actions::class_features::{
    COLOSSUS_SLAYER_TAG, DREADFUL_STRIKES_TAG, FOE_SLAYER_TAG, MULTIATTACK_DEFENSE_TAG,
    PLANAR_WARRIOR_TAG, ROVING_TAG, VANISH, VANISH_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, SCIMITAR};
use crate::actions::spells::{
    CONJURE_VOLLEY, CURE_WOUNDS, FAERIE_FIRE, HAIL_OF_THORNS, HUNTERS_MARK, LESSER_RESTORATION,
    LIGHTNING_ARROW, SPIKE_GROWTH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ranger PC template. DEX-primary half-caster martial. Plays as a
/// kiter — longbow as the workhorse attack, Hunter's Mark for the
/// per-hit +1d6 rider, Hail of Thorns for an opening AoE on the lead
/// shot, Cure Wounds + Lesser Restoration for self-sustain. Mid-CR PC
/// with a small but focused spell list that the AI's existing
/// targeting heuristics already exercise (kite + ranged-with-rider).
///
/// Stats target a level-5 ranger: ~32 HP (5d10+5), AC 15 (studded
/// leather + DEX), DEX 16, WIS 14, half-caster slots (4/2). Scimitar
/// as the melee fallback when an enemy closes through the kite.
pub static RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGBOW);
    actions.push(&SCIMITAR);
    actions.push(&*HUNTERS_MARK);
    actions.push(&*HAIL_OF_THORNS);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*SPIKE_GROWTH);
    // Fog Cloud — lv1 conjuration on the ranger's RAW spell list. The
    // ranger uses it as a kite-cover: drop a 20-ft sphere of heavy
    // obscurement on advancing melee threats, then fall back behind it
    // (the longbow keeps firing — RAW ranged attacks into the cloud
    // have disadvantage but kite range tends to keep the shooter outside
    // the burst). Concentration-bound; the ranger's only existing
    // concentration spell is Hunter's Mark, so the AI picks whichever
    // is higher leverage when only one slot is free.
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    // Newest ranger additions:
    //   - lv3 **Lightning Arrow** (`SmiteSpell` chassis): bonus-action
    //     concentration prime that loads the next ranged weapon attack
    //     with +4d8 lightning. The rider table gates on
    //     `ranged_only=true` so a melee scimitar swing won't burn the
    //     prime. Pairs naturally with the longbow workhorse.
    //   - lv5 **Conjure Volley**: 8d8 piercing in a 40-ft burst,
    //     friend-or-foe agnostic. Ranger's apex AoE — slots between
    //     Lightning Arrow (lv3 single-shot) and the spellcaster-tier
    //     evocations on the half-caster spell ladder.
    actions.push(&LIGHTNING_ARROW);
    actions.push(&*CONJURE_VOLLEY);
    // lv2 **Barkskin**: ranger half-caster pickup. Touch concentration
    // buff that floors the target's AC at 16 — pairs cleanly with the
    // ranger's longbow kite (cast on self before the fight, then plink
    // from cover) or supports a frailer ally (wizard / cleric).
    actions.push(&*crate::actions::spells::BARKSKIN);
    // lv2 **Pass Without Trace**: ranger half-caster pickup. 30ft
    // concentration aura that imposes attack-disadvantage on attackers
    // — the ranger's signature stealth utility, slotted in the lv2 lane
    // alongside Spike Growth / Hunter's Mark.
    actions.push(&*crate::actions::spells::PASS_WITHOUT_TRACE);
    actions.push(&*crate::actions::spells::ABSORB_ELEMENTS);
    // lv1 **Longstrider**: ranger half-caster pickup. Touch +10 ft speed
    // for 1 hour, no concentration. Pairs cleanly with the ranger's
    // Hunter's Mark + longbow kite — the ranger pre-buffs themselves /
    // an ally before the engagement and the speed boost composes with
    // any later Pass Without Trace / Spider Climb stack.
    actions.push(&*crate::actions::spells::LONGSTRIDER);
    // lv3 **Flame Arrows** (transmutation, XGtE): concentration self-buff
    // that grants +1d6 fire on every ranged weapon hit (ranged-only via
    // the OnHitRider table). Slots cleanly into the ranger's lv3 lane
    // alongside Lightning Arrow (single-shot +4d8 prime) — Flame Arrows
    // is the sustained-DPS sibling that bleeds extra fire every swing
    // for the duration. Mirrors Hunter's Mark's per-hit rider envelope
    // but typed (fire) and gated to ranged.
    actions.push(&*crate::actions::spells::FLAME_ARROWS);
    // Latest ranger additions:
    //   - lv2 **Silence**: ranger SRD lv2 staple; perfect for shutting
    //     down enemy spellcasters from sniping range.
    //   - lv4 **Freedom of Movement**: ranger SRD lv4 staple; ally-buff
    //     that breaks Paralyzed / Restrained / Grappled installs and
    //     locks out future ones for the duration.
    actions.push(&*crate::actions::spells::SILENCE);
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    // 5e Ranger **Vanish** (class feature, level 14). Bonus-action Hide
    // gated on `VANISH_TAG` — same one-shot attack-advantage rider as
    // the baseline Hide action, at the cheaper bonus-action cost.
    // Signature "kite-and-vanish" tell that composes with the ranger's
    // Hunter's Mark + longbow loop: mark a target, plink, then vanish
    // as a bonus action so the next arrow lands with advantage
    // (Hidden-attacker rider). Distinct from CunningHide (Rogue) —
    // both are bonus-action Hides, but ship on different chassis so a
    // multiclass rogue/ranger doesn't accidentally double-fire the
    // action. The RAW "can't be tracked by nonmagical means" clause is
    // a narrative rider with no combat surface — no wiring needed.
    // Ships on the CR-1 (level-5) baseline template above its strict
    // RAW lv14 gate for the same reason Foe Slayer (lv20) does —
    // class templates target a balanced playable level, not lockstep
    // PHB progression.
    actions.push(&*VANISH);
    CreatureTemplate {
        name: "Ranger",
        glyph: 'R',
        ac: 15,
        hitpoints: "5d10+5".parse().unwrap(),
        strength: 12,
        dexterity: 16,
        constitution: 12,
        intelligence: 10,
        wisdom: 14, // spellcasting ability
        charisma: 10,
        languages: HashSet::from([Language::Common, Language::Elvish]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Half-caster slots: bumped to a level-9 ranger loadout so the
        // new lv3 (Lightning Arrow) and lv5 (Conjure Volley) spells
        // have slots to fire on. 4/3/3/1/1 matches a level-9 ranger.
        spell_slots_by_level: vec![4, 3, 3, 1, 1],
        rolls_death_saves: true,
        // Rangers are proficient in STR and DEX saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
        ]),
        has_extra_attack: true,
        // 5e Ranger **Fighting Style: Archery** (lv2 class feature). Passive
        // +2 to ranged weapon attack rolls. Read at `resolve_attack` gated
        // on `!is_melee && !is_spell` so a longbow shot picks up the bonus
        // but a Fire Bolt spell attack doesn't. Composes cleanly with the
        // ranger's longbow kite — the extra +2 accuracy on every arrow
        // stacks with Hunter's Mark's per-hit rider and Colossus Slayer's
        // once-per-turn +1d8.
        has_archery_style: true,
        // 5e Ranger **Feral Senses** (level 18 capstone). Passive
        // concealment-piercer: unbounded-range immunity to the
        // Invisible / Blurred / Displaced attack-mode tax on both
        // sides (the ranger's swings against invisible targets don't
        // suffer disadvantage, and invisible attackers don't gain
        // advantage against the ranger). Ships on the CR-1 baseline
        // template above its strict RAW level gate for the same reason
        // Foe Slayer (lv20 capstone) does — class templates target a
        // balanced playable level, not lockstep PHB progression. Read
        // at the `EncounterInstance::pierces_illusion_of` chokepoint
        // next to Truesight. Composes cleanly with Colossus Slayer /
        // Multiattack Defense (Hunter subclass features inherited via
        // `..RANGER_TEMPLATE.clone()`) and Faerie Fire (the ranger's
        // Outline install already breaks Invisible / Blurred /
        // Displaced offensively — Feral Senses is the defensive half
        // of the "we don't lose accuracy to concealment" identity).
        has_feral_senses: true,
        // 5e Ranger **Foe Slayer** (level 20 capstone). Passive once-
        // per-turn +WIS-mod flat damage rider on any weapon hit. Ships
        // on the CR-1 baseline template above its strict RAW level
        // gate for the same reason Colossus Slayer / Multiattack
        // Defense do on the Hunter template — class templates target
        // a balanced playable level, not lockstep PHB progression. The
        // capstone lives on the baseline template so the subclass
        // (Hunter Ranger) picks it up via `..RANGER_TEMPLATE.clone()`
        // alongside the Hunter's Prey / Defensive Tactics riders.
        //
        // 5e Ranger **Vanish** (lv14 class feature): gates the paired
        // `VANISH` bonus-action Hide action. See the action pushed
        // above for the full RAW envelope; the tag lives here so a
        // future non-Hunter subclass template (Beast Master, Gloom
        // Stalker, etc.) inherits it for free via `..RANGER_TEMPLATE.clone()`.
        // 5e Ranger **Roving** (optional class feature, 2024 PHB level 6):
        // passive +5 ft walking speed (RAW also grants climbing +
        // swimming speeds matching walking, but only the walking-speed
        // bump has a combat surface in this engine — climbing / swimming
        // fold into the same `speed()` accessor with no 3D terrain to
        // differentiate). Ships on the CR-1 baseline template above its
        // strict RAW lv6 gate for the same reason Feral Senses (lv18) /
        // Foe Slayer (lv20) already ride here — class templates target
        // a balanced playable level, not lockstep PHB progression. Read
        // at the shared `passive_feature_speed_bonus` chokepoint next to
        // Barbarian Fast Movement / Tiger Totem — the ranger picks up
        // +5 ft always-on, half a step further than Fast Movement's
        // +10 but sibling on the same lane.
        features: HashSet::from([FOE_SLAYER_TAG, VANISH_TAG, ROVING_TAG]),
        ..CreatureTemplate::defaults()
    }
});

/// Hunter Ranger — Conclave subclass build. Identical envelope to the
/// baseline `RANGER_TEMPLATE` (level-9 half-caster, longbow + scimitar,
/// 4/3/3/1/1 slot ladder, same kiter spell list) with one subclass
/// feature layered on: **Colossus Slayer** (level 3) — once per turn,
/// a weapon hit on a wounded target lays +1d8 of the weapon's damage
/// type. Pairs naturally with the ranger's longbow kite — the first
/// arrow of the round usually has a clean shot at full HP, but every
/// subsequent shot through Extra Attack / Hunter's Mark loops connects
/// against a wounded target and stacks the Colossus Slayer rider on
/// top of the Mark's +1d6 necrotic.
///
/// Distinct from `RANGER_TEMPLATE` (Conclave-less baseline) so a
/// Hunter-vs-Hunter or Hunter-vs-baseline encounter renders
/// unambiguously by name and the subclass feature doesn't accidentally
/// stack RAW-illegally on a single PC build. Glyph 'H' so the Hunter
/// shows up distinctly on the map next to the baseline 'R'.
pub static HUNTER_RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Ranger envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // features). The `..base.clone()` tail picks up every other field
    // — actions, spell slots, extra-attack, save profs — without an
    // N-line field-by-field copy.
    CreatureTemplate {
        name: "Hunter Ranger",
        glyph: 'H',
        // Hunter subclass features layered onto the baseline ranger
        // envelope:
        //   - `COLOSSUS_SLAYER_TAG`: Hunter's Prey (lv3, "Colossus
        //     Slayer" option). Once-per-turn +1d8 weapon-typed rider
        //     on any hit against a wounded target. Fires in
        //     `resolve_attack_outcome`.
        //   - `MULTIATTACK_DEFENSE_TAG`: Defensive Tactics (lv7,
        //     "Multiattack Defense" option). Passive +4 AC vs any
        //     attacker who has already landed a hit this turn — the
        //     "shrug off the second swing" envelope that pairs
        //     naturally with the ranger's kite pattern (drop the first
        //     hit, walk out of range before the follow-up connects).
        //     Ships on the CR-1 Hunter Ranger template above its
        //     strict RAW level gate for the same reason Colossus
        //     Slayer does — class templates target a balanced playable
        //     level, not lockstep PHB progression.
        features: HashSet::from([
            COLOSSUS_SLAYER_TAG,
            MULTIATTACK_DEFENSE_TAG,
            // Foe Slayer (lv20 ranger capstone) — inherited from the
            // baseline `RANGER_TEMPLATE` on the subclass since we
            // override the whole `features` set here rather than
            // extending it. Kept aligned with baseline so a Hunter
            // Ranger drops the +WIS-mod once-per-turn damage rider
            // alongside the Hunter-specific Colossus Slayer.
            FOE_SLAYER_TAG,
            // 5e Ranger Vanish (lv14) — same "override the whole
            // features set" caveat: re-listed here so the Hunter
            // ranger's `VANISH` action (inherited via the baseline
            // action list) still passes its `has_passive_feature`
            // gate. Falling back on the baseline copy would leave
            // the paired action gated off silently.
            VANISH_TAG,
            // 5e Ranger Roving (2024 PHB lv6 optional class feature) —
            // same "override the whole features set" caveat as Foe
            // Slayer / Vanish above: re-listed here so the Hunter
            // Ranger's passive +5 ft walking speed reads through the
            // `passive_feature_speed_bonus` chokepoint. The subclass
            // doesn't itself pick up Roving RAW; the tag rides on
            // baseline Ranger so any subclass (Hunter, future Beast
            // Master / Gloom Stalker) picks it up when it fully
            // inherits the baseline features HashSet.
            ROVING_TAG,
        ]),
        // 5e Hunter Ranger Superior Hunter's Defense (lv15, "Evasion"
        // option): on DEX saves for half damage, take 0 on a pass and
        // half on a fail instead of half / full. Ships on the CR-1
        // template above its strict RAW level gate alongside Colossus
        // Slayer and Multiattack Defense for the same reason — class
        // templates target a balanced playable level. Composes cleanly
        // with the ranger's DEX-primary stat spread: the DEX save is
        // already the ranger's strong lane, and Evasion turns a
        // passed save into 0 damage on Fireball / Lightning Bolt /
        // Cone of Cold.
        has_evasion: true,
        ..RANGER_TEMPLATE.clone()
    }
});

/// Gloom Stalker Ranger — XGtE subclass build. Identical envelope to the
/// baseline `RANGER_TEMPLATE` (level-9 half-caster, longbow + scimitar,
/// 4/3/3/1/1 slot ladder, same kiter spell list) with two subclass
/// features layered on:
///
/// - **Dread Ambusher** (level 3): passive +WIS-mod to initiative rolls.
///   The Gloom Stalker's headline "always strikes first" tell — folded
///   into the shared `initiative_flat_bonus` lane next to Rakish
///   Audacity's CHA-mod bump and Remarkable Athlete's `+ceil(prof / 2)`
///   so a hypothetical multiclass stacks every bump cleanly. Ships on
///   this template above its strict RAW lv3 gate — the whole template is
///   pinned at a level-9 loadout, matching the baseline ranger. RAW's
///   companion first-turn extra-attack + `+1d8` bonus damage half is
///   left as future work; the initiative bump alone is the load-bearing
///   Gloom Stalker tell for combat pacing.
/// - **Iron Mind** (level 7, XGtE): passive proficiency in Wisdom
///   saves. Added to `proficient_saves` on top of the baseline ranger's
///   `Strength` / `Dexterity` set — the Gloom Stalker's mental-defense
///   augment reads through the shared save-modifier chokepoint. Ships
///   above its strict RAW lv7 gate for the same reason Dread Ambusher
///   does — class templates target a balanced playable level, not
///   lockstep PHB progression.
///
/// Distinct from `RANGER_TEMPLATE` (baseline) and `HUNTER_RANGER_TEMPLATE`
/// (Hunter subclass) so a Gloom-vs-Hunter or Gloom-vs-baseline encounter
/// renders unambiguously by name. Glyph 'G' so the Gloom Stalker shows
/// up distinctly on the map next to the baseline 'R' and the Hunter 'H'.
pub static GLOOM_STALKER_RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Ranger envelope wholesale
    // and overwrite only the per-subclass differences. The `..base.clone()`
    // tail picks up every other field — actions, spell slots, extra-
    // attack, Feral Senses, Foe Slayer, Vanish, Roving, Archery Style —
    // without an N-line field-by-field copy.
    CreatureTemplate {
        name: "Gloom Stalker Ranger",
        glyph: 'G',
        // 5e Gloom Stalker Ranger Dread Ambusher (level 3) — passive
        // +WIS-mod initiative bump. Read at `initiative_flat_bonus`.
        has_dread_ambusher: true,
        // 5e Gloom Stalker Ranger Iron Mind (level 7, XGtE) — passive
        // proficiency in Wisdom saves. Added to the baseline ranger's
        // STR / DEX save-proficiency set so the ranger's saving-throw
        // chokepoint picks up the extra proficiency bonus on WIS saves
        // (against Charm Person, Hold Person, Suggestion, and similar
        // mind-affecting effects that thematically target the Gloom
        // Stalker's shadow-touched mind).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
        ]),
        ..RANGER_TEMPLATE.clone()
    }
});

/// Fey Wanderer Ranger — Ranger Conclave **Fey Wanderer** subclass build
/// (TCE). Identical envelope to the baseline `RANGER_TEMPLATE` (level-9
/// half-caster, longbow + scimitar, 4/3/3/1/1 slot ladder, same kiter
/// spell list, Feral Senses / Foe Slayer / Vanish / Roving / Archery
/// Style inherited via `..base.clone()`) with one subclass feature
/// layered on: **Dreadful Strikes** (lv3 subclass tell) — passive
/// once-per-turn +1d4 Psychic damage rider on any weapon hit.
///
/// The Fey Wanderer's signature "the fey-touched ranger's weapons whisper
/// dread with every strike" tell — where the baseline ranger relies on
/// their weapon's base damage type, the Fey Wanderer's swings carry a
/// small psychic aftershock that punches through most creatures' typed
/// resistances (Psychic is a rarely-resisted damage type — only a
/// handful of aberrations / undead / constructs shrug it off, vs. the
/// broad Fire / Cold / Necrotic resistance lanes on many monster
/// chassis). Composes cleanly with the ranger's kiter kit: the +1d4 fires
/// on the opening longbow shot (or the follow-up scimitar swing when a
/// melee threat closes through the kite) at no action / concentration
/// cost.
///
/// Sibling on the "once-per-turn +XdN weapon-hit rider" cross-class lane
/// to `COLOSSUS_SLAYER_TAG` (Hunter Ranger lv3 — +1d8 weapon-typed with
/// a wounded-target gate), `FOE_SLAYER_TAG` (Ranger lv20 capstone —
/// flat +WIS-mod on any weapon hit), `DIVINE_FURY_TAG` (Zealot
/// Barbarian lv3 — +1d6 + level/2 Radiant while raging), and
/// `SNEAK_ATTACK_TAG` (Rogue once-per-turn +Nd6 with the qualifying-
/// attack gate). The four rider tags share the `ONCE_PER_TURN_RIDER_TAGS`
/// ledger on `ActorInstance` — each fires at most once per turn on the
/// shared per-actor gate, and a hypothetical multiclass carrier stacks
/// every distinct tag's die cleanly on the opening shot (Hunter Ranger
/// with Fey Wanderer multiclass: Colossus Slayer + Dreadful Strikes +
/// Foe Slayer all fire on the first wounded-target hit).
///
/// Distinct from the sibling `COLOSSUS_SLAYER_TAG` on three axes:
///   1. **No target gate** — Dreadful Strikes fires against any target;
///      Colossus Slayer requires `is_wounded()`. Fey Wanderer opens on
///      full-HP targets where Hunter opens only on softened ones.
///   2. **Fixed damage type** — Dreadful Strikes always deals Psychic;
///      Colossus Slayer inherits the weapon's damage type.
///   3. **Smaller die** — 1d4 vs. Colossus Slayer's 1d8; the Fey
///      Wanderer trades die size for the always-fires-any-target gate.
///
/// RAW's Fey Wanderer picks up other features not shipped on this
/// template — **Otherworldly Glamour** (lv3: +WIS-mod to CHA checks +
/// one CHA-skill proficiency; ribbon on this engine's combat surface),
/// **Fey Reinforcements** (lv7: cast Summon Fey once per long rest
/// without a spell slot; needs a summon action surface), **Beguiling
/// Twist** (lv11: reaction to redirect a Charmed / Frightened save; needs
/// a per-target save-redirect hook), and **Misty Wanderer** (lv15:
/// cast Misty Step at will; needs an at-will spell-cast surface). Only
/// the lv3 Dreadful Strikes passive has a mechanical surface on the CR-1
/// (level-9) chassis that plugs cleanly into the shared
/// `ONCE_PER_TURN_RIDER_TAGS` ledger, so we ship that half and leave
/// the rest as future work — matching the way `HUNTER_RANGER_TEMPLATE`
/// ships only Colossus Slayer + Multiattack Defense + Superior Hunter's
/// Defense (Evasion) from the RAW Hunter Conclave kit and every other
/// subclass template runs above its strict RAW gate.
///
/// Ships on the CR-1 (level-9) ranger chassis at (or above) its strict
/// RAW lv3 gate for the same reason `HUNTER_RANGER_TEMPLATE` and
/// `GLOOM_STALKER_RANGER_TEMPLATE` ship their Conclave features above
/// their strict RAW gates — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Distinct from `RANGER_TEMPLATE` (Conclave-less baseline),
/// `HUNTER_RANGER_TEMPLATE` (Hunter Conclave), and
/// `GLOOM_STALKER_RANGER_TEMPLATE` (Gloom Stalker Conclave) so a
/// Fey-vs-Hunter / Fey-vs-Gloom / Fey-vs-baseline encounter renders
/// unambiguously by name.
///
/// Glyph 'Y' — 'Y' for "fae**Y**" reads as a fey-touched wanderer, and
/// the letter's forked shape suggests a divining rod / dowsing branch
/// (fey iconography). Distinct from baseline ranger 'R', Hunter 'H',
/// and Gloom Stalker 'G'. Collides with a handful of NPC creature
/// templates (Yugoloth-adjacent monsters) but the team-color-and-team-
/// id combo disambiguates them in a mixed encounter — same overlap
/// policy the other PC subclass glyphs already follow.
pub static FEY_WANDERER_RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Ranger envelope wholesale
    // and layers on the Dreadful Strikes passive tag. The `..base.clone()`
    // tail inside the helper picks up every other field — the full
    // ranger spell list, save profs, stats, slots, Feral Senses / Foe
    // Slayer / Vanish / Roving / Archery Style — without an N-line
    // field-by-field copy. No new actions are pushed — Dreadful Strikes
    // is a purely passive once-per-turn weapon-hit rider read at
    // `resolve_attack_outcome` via the shared `ONCE_PER_TURN_RIDER_TAGS`
    // ledger, not a fresh action surface, so the "tag-only" shape the
    // helper wraps is a natural fit. First ranger-chassis user of the
    // `with_subclass_tag` cross-class helper — every prior ranger
    // subclass (Hunter, Gloom Stalker) layers more than a single feature
    // tag on top of the baseline (Hunter: Colossus Slayer + Multiattack
    // Defense + Evasion; Gloom Stalker: Dread Ambusher + Iron Mind) so
    // they stay on the explicit clone-and-insert body. Sibling helper
    // users on the "clone base + insert one tag" cross-class lane: every
    // tag-only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `FORGE_CLERIC_TEMPLATE`, `TWILIGHT_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `WAR_MAGIC_WIZARD_TEMPLATE`,
    // `SHADOW_MAGIC_SORCERER_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`, `LONG_DEATH_MONK_TEMPLATE`,
    // `GLORY_PALADIN_TEMPLATE`, `WATCHERS_PALADIN_TEMPLATE`.
    RANGER_TEMPLATE.with_subclass_tag("Fey Wanderer Ranger", 'Y', DREADFUL_STRIKES_TAG)
});

/// Horizon Walker Ranger — Ranger Conclave **Horizon Walker** subclass
/// build (XGtE). Identical envelope to the baseline `RANGER_TEMPLATE`
/// (level-9 half-caster, longbow + scimitar, 4/3/3/1/1 slot ladder, same
/// kiter spell list, Feral Senses / Foe Slayer / Vanish / Roving /
/// Archery Style inherited via `..base.clone()`) with one subclass
/// feature layered on: **Planar Warrior** (lv3 subclass tell) — passive
/// once-per-turn +1d8 Force damage rider on any weapon hit.
///
/// The Horizon Walker's signature "the planar-touched ranger's swings
/// slip partway into the Ethereal" tell — where the baseline ranger
/// relies on their weapon's base damage type, the Horizon Walker's
/// first swing each turn carries a Force aftershock that punches
/// through nearly every typed-resistance lane (Force is the rarest-
/// resisted damage type in the engine — vanishingly few monsters shrug
/// it off, vs. the broad Fire / Cold / Necrotic resistance lanes on
/// many monster chassis, and even vs. the Psychic-resistance lane a
/// handful of aberrations / undead / constructs carry that gates the
/// sibling Dreadful Strikes / Psychic Blades riders). Composes cleanly
/// with the ranger's kiter kit: the +1d8 fires on the opening longbow
/// shot (or the follow-up scimitar swing when a melee threat closes
/// through the kite) at no action / concentration cost.
///
/// Sibling on the "once-per-turn +XdN weapon-hit rider" cross-class
/// lane to `COLOSSUS_SLAYER_TAG` (Hunter Ranger lv3 — +1d8 weapon-typed
/// with a wounded-target gate), `DREADFUL_STRIKES_TAG` (Fey Wanderer
/// Ranger lv3 — +1d4 Psychic on any weapon hit), `PSYCHIC_BLADES_TAG`
/// (Whispers Bard lv3 — +1d6 Psychic on any weapon hit),
/// `FOE_SLAYER_TAG` (Ranger lv20 capstone — flat +WIS-mod on any weapon
/// hit), `DIVINE_FURY_TAG` (Zealot Barbarian lv3 — +1d6 + level/2
/// Radiant while raging), and `SNEAK_ATTACK_TAG` (Rogue once-per-turn
/// +Nd6 with the qualifying-attack gate). The seven rider tags share
/// the `ONCE_PER_TURN_RIDER_TAGS` ledger on `ActorInstance` — each
/// fires at most once per turn on the shared per-actor gate, and a
/// hypothetical multiclass carrier stacks every distinct tag's die
/// cleanly on the opening shot (Hunter Ranger / Horizon Walker
/// multiclass: Colossus Slayer + Planar Warrior + Foe Slayer all fire
/// on the first wounded-target hit).
///
/// Distinct from the sibling `COLOSSUS_SLAYER_TAG` on two axes:
///   1. **No target gate** — Planar Warrior fires against any target;
///      Colossus Slayer requires `is_wounded()`. Horizon Walker opens
///      on full-HP targets where Hunter opens only on softened ones.
///   2. **Fixed damage type** — Planar Warrior always deals Force;
///      Colossus Slayer inherits the weapon's damage type. The Force
///      type is the load-bearing tell: Force is one of the rarest-
///      resisted types in the engine, so the rider's damage stays
///      unshaved across nearly every enemy chassis, distinct from the
///      sibling Psychic riders (Dreadful Strikes / Psychic Blades) that
///      lose their damage to the aberrant / construct / undead Psychic-
///      resistance lane.
///
/// Same die size as Colossus Slayer (1d8) — two lv3 subclass riders
/// converge on the same magnitude from different angles.
///
/// RAW-strict Planar Warrior costs a bonus action to mark a specific
/// creature within 30 ft; the next weapon hit against that marked
/// target *this turn* deals the +1d8 Force. We collapse both the bonus-
/// action-mark and the per-target gate down to a plain "once-per-turn
/// +1d8 Force on any target" rider on the shared
/// `ONCE_PER_TURN_RIDER_TAGS` ledger — matching the same collapse
/// Dreadful Strikes / Psychic Blades apply to their own RAW per-target
/// gates (both trade the per-target lookup for slotting cleanly into
/// the once-per-turn ledger). Trades away the RAW "mark first, hit
/// second" two-step for slotting into the existing rider chokepoint
/// without a separate bonus-action-mark action surface plus a per-
/// target-mark ledger.
///
/// RAW's Horizon Walker picks up other features not shipped on this
/// template — **Detect Portal** (lv3: 1/rest sense a planar portal
/// within 1 mile; ribbon on this engine's combat surface), **Ethereal
/// Step** (lv7: 1/rest cast Etherealness on self as a bonus action for
/// a single turn; needs an Ethereal Plane surface the engine doesn't
/// model), **Distant Strike** (lv11: teleport up to 10 ft before each
/// attack + third attack per Action against a fresh target; needs a
/// per-attack teleport hook plus a distinct-target gate), and
/// **Spectral Defense** (lv15: reaction to halve damage from an attack;
/// needs a per-attack reaction hook). Only the lv3 Planar Warrior
/// passive has a mechanical surface on the CR-1 (level-9) chassis that
/// plugs cleanly into the shared `ONCE_PER_TURN_RIDER_TAGS` ledger, so
/// we ship that half and leave the rest as future work — matching the
/// way `FEY_WANDERER_RANGER_TEMPLATE` ships only Dreadful Strikes and
/// `HUNTER_RANGER_TEMPLATE` ships only Colossus Slayer + Multiattack
/// Defense + Superior Hunter's Defense (Evasion) from their respective
/// RAW Conclave kits.
///
/// Ships on the CR-1 (level-9) ranger chassis at (or above) its strict
/// RAW lv3 gate for the same reason `FEY_WANDERER_RANGER_TEMPLATE`,
/// `HUNTER_RANGER_TEMPLATE`, and `GLOOM_STALKER_RANGER_TEMPLATE` ship
/// their Conclave features above their strict RAW gates — class
/// templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Distinct from `RANGER_TEMPLATE` (Conclave-less baseline),
/// `HUNTER_RANGER_TEMPLATE` (Hunter Conclave),
/// `GLOOM_STALKER_RANGER_TEMPLATE` (Gloom Stalker Conclave), and
/// `FEY_WANDERER_RANGER_TEMPLATE` (Fey Wanderer Conclave) so a
/// Horizon-vs-Hunter / vs-Gloom / vs-Fey / vs-baseline encounter renders
/// unambiguously by name.
///
/// Glyph 'Z' — 'Z' for "hori**Z**on" reads as a planar-touched wanderer;
/// the letter's zigzag shape suggests the boundary between planes.
/// Distinct from baseline ranger 'R', Hunter 'H', Gloom Stalker 'G',
/// and Fey Wanderer 'Y'. Collides with a handful of NPC creature
/// templates (Zombies, Ziggurats-adjacent) but the team-color-and-team-
/// id combo disambiguates them in a mixed encounter — same overlap
/// policy the other PC subclass glyphs already follow.
pub static HORIZON_WALKER_RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Ranger envelope wholesale
    // and layers on the Planar Warrior passive tag. The `..base.clone()`
    // tail inside the helper picks up every other field — the full
    // ranger spell list, save profs, stats, slots, Feral Senses / Foe
    // Slayer / Vanish / Roving / Archery Style — without an N-line
    // field-by-field copy. No new actions are pushed — Planar Warrior
    // is a purely passive once-per-turn weapon-hit rider read at
    // `resolve_attack_outcome` via the shared
    // `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort in `engine::attack`, not
    // a fresh action surface, so the "tag-only" shape the helper wraps
    // is a natural fit. Second ranger-chassis user of the
    // `with_subclass_tag` cross-class helper (after
    // `FEY_WANDERER_RANGER_TEMPLATE`) — every prior ranger subclass
    // (Hunter, Gloom Stalker) layers more than a single feature tag on
    // top of the baseline so they stay on the explicit clone-and-insert
    // body. Sibling helper users on the "clone base + insert one tag"
    // cross-class lane: every tag-only Warlock Otherworldly Patron
    // subclass (via `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `FORGE_CLERIC_TEMPLATE`, `TWILIGHT_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `WAR_MAGIC_WIZARD_TEMPLATE`,
    // `SHADOW_MAGIC_SORCERER_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`, `LONG_DEATH_MONK_TEMPLATE`,
    // `GLORY_PALADIN_TEMPLATE`, `WATCHERS_PALADIN_TEMPLATE`,
    // `FEY_WANDERER_RANGER_TEMPLATE`.
    RANGER_TEMPLATE.with_subclass_tag("Horizon Walker Ranger", 'Z', PLANAR_WARRIOR_TAG)
});
