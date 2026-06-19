use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    LONGBOW, MEDUSA_MULTI, MEDUSA_PETRIFYING_GAZE, MEDUSA_SNAKE_HAIR,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Medusa — CR 6 monstrosity. The iconic petrifying-gaze threat:
/// - **Multiattack** (snake hair + petrifying gaze in one Action). The
///   gaze forces a CON save vs DC 14; on fail the target is Petrified
///   for 1 round (we cap at 1 round so a single hit doesn't game-over;
///   the action-economy lockout while Petrified is brutal enough).
/// - **Snake Hair** (standalone): 1d4+DEX piercing + 4d6 poison rider.
/// - **Petrifying Gaze** (standalone): the pure stone-lock save.
/// - **Longbow**: ranged piercing option for when the gaze is on cooldown.
///
/// MM RAW: AC 15, ~127 HP (17d8+51), DEX 15, CON 16. No damage or
/// condition immunities (medusas are mortal monstrosities). Darkvision
/// 60. Speaks Common in this version (RAW says "the languages it knew
/// in life" — we tag Common as the baseline).
pub static MEDUSA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MEDUSA_MULTI);
    actions.push(&*MEDUSA_SNAKE_HAIR);
    actions.push(&*MEDUSA_PETRIFYING_GAZE);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Medusa",
        // 'd' (lowercase) — distinct from 'M' (Wizard/Mage). Reads as
        // a smaller-statured humanoid with the snake-hair flourish.
        glyph: 'd',
        ac: 15,
        // 17d8+51 ≈ 127 average per MM (CR 6).
        hitpoints: "17d8+51".parse().unwrap(),
        strength: 10,
        dexterity: 15,
        constitution: 16,
        intelligence: 12,
        wisdom: 13,
        charisma: 15,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
