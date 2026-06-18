use crate::actions::class_features::RELENTLESS_ENDURANCE_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BERSERKER_GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Berserker — CR 2 humanoid. A high-HP melee bruiser with Reckless
/// Attack: a bonus-action self-buff that grants advantage on the next
/// attack but leaves the berserker easier to hit until the start of
/// their next turn. Pairs with a greataxe for big-die swings; the AI
/// will (eventually) learn the bonus-action / greataxe combo, but
/// even at a flat-line policy the greataxe alone makes them feel
/// dangerous.
pub static BERSERKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BERSERKER_GREATAXE);
    actions.push(&*RECKLESS_ATTACK);
    CreatureTemplate {
        name: "Berserker",
        // 'b' — distinct from 'B' (Bandit Captain).
        glyph: 'b',
        ac: 13,
        // 9d8+27 = 67 average per MM.
        hitpoints: "9d8+27".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 9,
        dexterity: 12,
        wisdom: 11,
        constitution: 17,
        charisma: 9,
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([AbilityScoreType::Strength]),
        // 5e Half-Orc racial: Relentless Endurance — once per long rest,
        // damage that would drop the berserker to 0 HP drops them to 1
        // HP instead. Modeled as a passive feature flag the take_damage
        // hook checks before transitioning to the dying / dead state.
        features: HashSet::from([RELENTLESS_ENDURANCE_TAG]),
        ..CreatureTemplate::defaults()
    }
});
