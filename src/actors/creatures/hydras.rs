use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HYDRA_BITE, HYDRA_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hydra — CR 8 monstrosity. Huge multi-headed serpent with a 5-bite
/// multiattack and the iconic regeneration: 10 HP per round while
/// combat-active. The MM hydra's signature feature — "as long as a head
/// remains alive, severed heads regrow" — is approximated as a flat
/// regen (we don't model head-counting / fire-cauterize mechanics).
///
/// Stat profile (MM RAW): AC 15, ~172 HP (15d12+75), STR 20, DEX 12,
/// CON 20. Five heads, 5 attacks per Action. No language slot (the
/// hydra is non-sentient). Hold Breath, Reactive Heads, Wakeful — none
/// modeled directly here; the engine's blanket "regen unless suppressed"
/// covers Reactive Heads' "always alert" flavor close enough.
pub static HYDRA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HYDRA_MULTI);
    actions.push(&HYDRA_BITE);
    CreatureTemplate {
        name: "Hydra",
        // 'Y' (uppercase) — distinct from 'y' (Wyvern), 'H' (Hippogriff),
        // 'h' (Hell Hound). Visual reads as a tall serpent-headed beast.
        glyph: 'Y',
        ac: 15,
        // 15d12+75 ≈ 172 average per MM (CR 8).
        hitpoints: "15d12+75".parse().unwrap(),
        strength: 20,
        dexterity: 12,
        constitution: 20,
        intelligence: 2,
        wisdom: 10,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 8.0,
        size: Size::Huge,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Hydras are mindless; no Charm / Frighten resistance — they
        // simply don't process those effects (we leave the immunity
        // off to keep the spell list interactive).
        condition_immunities: HashSet::from([Condition::Unconscious]),
        // 10 HP/round regen — the iconic hydra trait. No suppressor
        // (we don't model fire-cauterizing head stumps).
        regen_per_round: 10,
        has_extra_attack: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});
