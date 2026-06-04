use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HYDRA_BITE, HYDRA_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
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
        speed: 30.,
        strength: 20,
        intelligence: 2,
        dexterity: 12,
        wisdom: 10,
        constitution: 20,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(), // Hydras don't speak.
        cr: 8.0,
        size: Size::Huge,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // Hydras are mindless; no Charm / Frighten resistance — they
        // simply don't process those effects (we leave the immunity
        // off to keep the spell list interactive).
        condition_immunities: HashSet::from([Condition::Unconscious]),
        features: HashSet::new(),
        // 10 HP/round regen — the iconic hydra trait. No suppressor
        // (we don't model fire-cauterizing head stumps).
        regen_per_round: 10,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
