use crate::actions::class_features::{
    COLOSSUS_SLAYER_TAG, FOE_SLAYER_TAG, MULTIATTACK_DEFENSE_TAG,
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
        // 5e Ranger **Foe Slayer** (level 20 capstone). Passive once-
        // per-turn +WIS-mod flat damage rider on any weapon hit. Ships
        // on the CR-1 baseline template above its strict RAW level
        // gate for the same reason Colossus Slayer / Multiattack
        // Defense do on the Hunter template — class templates target
        // a balanced playable level, not lockstep PHB progression. The
        // capstone lives on the baseline template so the subclass
        // (Hunter Ranger) picks it up via `..RANGER_TEMPLATE.clone()`
        // alongside the Hunter's Prey / Defensive Tactics riders.
        features: HashSet::from([FOE_SLAYER_TAG]),
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
