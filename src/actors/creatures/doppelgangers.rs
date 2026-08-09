use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DOPPELGANGER_MULTI, DOPPELGANGER_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Doppelganger — CR 3 monstrosity. High AC (14) and 52 average HP
/// with a vanilla slam multiattack. The signature shapeshifting is
/// still unmodeled — the engine has no disguise layer for it to hide
/// behind — but the surprise half of the MM's ambush package is now a
/// rule the board can carry: `Surprised` is a condition, decided as
/// the encounter opens by who can see whom, so a doppelganger waiting
/// in an unlit room gets the round RAW gives it. Charm immunity and
/// the strong stat line keep it distinct from other CR-3 fighters.
pub static DOPPELGANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DOPPELGANGER_SLAM);
    actions.push(&*DOPPELGANGER_MULTI);
    CreatureTemplate {
        name: "Doppelganger",
        // 'D' was free (Dire Wolf is 'd'); use 'D' for doppelganger.
        glyph: 'D',
        ac: 14,
        // 8d8+16 = 52 average per MM.
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 11,
        dexterity: 18, // The marquee stat — drives initiative + slam.
        wisdom: 12,
        constitution: 14,
        charisma: 14,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        // 5e: doppelgangers are immune to Charmed (they're the ones
        // doing the charming) — keeps the trope intact even without
        // shapeshift mechanics.
        condition_immunities: HashSet::from([Condition::Charmed]),
        ..CreatureTemplate::defaults()
    }
});
