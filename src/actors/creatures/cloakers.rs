use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CLOAKER_BITE, CLOAKER_MOAN, CLOAKER_MULTI, CLOAKER_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cloaker — CR 8 aberration. Ray-like creature that wraps around its
/// prey: a bite and a barbed tail at 10 ft reach, thrown together by
/// `CLOAKER_MULTI`, over the top of the **Moan** that is the reason the
/// thing is CR 8 at all. See `monster_attacks::CLOAKER_MOAN` for the
/// 60 ft fear sweep and which of RAW's two gates (hearing, aberration)
/// the engine can actually ask about.
///
/// The attach clause on the bite is still not modeled — there is no
/// "riding on a creature's back" state for it to set. Ground speed
/// 10 ft (flying speed 40 ft not tracked). Darkvision 60 ft.
pub static CLOAKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLOAKER_TAIL);
    actions.push(&CLOAKER_BITE);
    actions.push(&*CLOAKER_MULTI);
    actions.push(&*CLOAKER_MOAN);
    CreatureTemplate {
        name: "Cloaker",
        glyph: 'c',
        ac: 14,
        hitpoints: "12d10+36".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 40 ft.
        speed: 10.0,
        fly_speed: 40.0,
        strength: 17,
        intelligence: 11,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 8.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        // 5e Cloaker **Light Sensitivity**: "while in bright light, the
        // cloaker has disadvantage on attack rolls and Wisdom
        // (Perception) checks." Mapped onto the Sensitivity tier, whose
        // clauses are exactly these two. RAW's trigger is bright light
        // rather than sunlight specifically, which is a narrowing the
        // engine cannot express — `is_sunlit` is the only light-tier
        // question the frailty lane asks — so a cloaker caught under a
        // torch gets away with it and one caught outdoors does not.
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
        ..CreatureTemplate::defaults()
    }
});
