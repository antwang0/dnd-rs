use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{
    CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE, CUNNING_STRIKE_DAZE, CUNNING_STRIKE_POISON,
    CUNNING_STRIKE_TRIP, CUNNING_STRIKE_WITHDRAW, STEADY_AIM,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rogue PC template. Light armor (AC 14: leather + DEX), modest HP,
/// DEX-primary. The headline mechanic is **Sneak Attack** — the
/// shortsword (finesse, DEX-based 1d6) deals an extra 1d6 once per turn
/// when the rogue has advantage OR an ally is adjacent to the target.
/// `rolls_death_saves: true` (PC) so it enters the dying state at 0 HP.
pub static ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ROGUE_SHORTSWORD);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    // 5e Tasha's Rogue Steady Aim (lv3): bonus action; advantage on next
    // attack at the cost of zeroing speed for the rest of the turn.
    // Pairs naturally with Sneak Attack's advantage trigger so a sniping
    // rogue can fire mid-encounter without needing an adjacent ally.
    actions.push(&*STEADY_AIM);
    // 5e 2024 Rogue Cunning Strike (lv5): bonus-action primes that trade
    // Sneak Attack dice for tactical effects on the next sneak hit.
    // Mutually exclusive (one prime at a time) — the shortsword's
    // `consume_cunning_strike` chokepoint picks the first active prime
    // and applies its effect.
    actions.push(&*CUNNING_STRIKE_POISON);
    actions.push(&*CUNNING_STRIKE_TRIP);
    actions.push(&*CUNNING_STRIKE_WITHDRAW);
    actions.push(&*CUNNING_STRIKE_DAZE);
    CreatureTemplate {
        name: "Rogue",
        glyph: 'R',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        strength: 10,
        dexterity: 16, // primary
        constitution: 12,
        intelligence: 12,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Rogues are proficient in DEX and INT saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        has_evasion: true,
        has_uncanny_dodge: true,
        ..CreatureTemplate::defaults()
    }
});
