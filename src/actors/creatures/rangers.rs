use crate::actions::class_features::COLOSSUS_SLAYER_TAG;
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
    let base = &*RANGER_TEMPLATE;
    CreatureTemplate {
        name: "Hunter Ranger",
        glyph: 'H',
        ac: base.ac,
        hitpoints: base.hitpoints,
        strength: base.strength,
        dexterity: base.dexterity,
        constitution: base.constitution,
        intelligence: base.intelligence,
        wisdom: base.wisdom,
        charisma: base.charisma,
        languages: base.languages.clone(),
        cr: base.cr,
        size: base.size,
        creature_type: base.creature_type,
        actions: base.actions.clone(),
        spell_slots_by_level: base.spell_slots_by_level.clone(),
        rolls_death_saves: base.rolls_death_saves,
        proficient_saves: base.proficient_saves.clone(),
        features: HashSet::from([COLOSSUS_SLAYER_TAG]),
        has_extra_attack: base.has_extra_attack,
        ..CreatureTemplate::defaults()
    }
});
