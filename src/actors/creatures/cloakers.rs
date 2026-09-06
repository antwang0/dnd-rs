use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CLOAKER_ATTACH, CLOAKER_MOAN, CLOAKER_MULTI, CLOAKER_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::attachment::AttachProfile;
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The cloaker's half of SRD 5.2's attach clause: "If the target is a
/// Large or smaller creature, the cloaker attaches to it. While the
/// cloaker is attached, the target has the Blinded condition, and the
/// cloaker can't make Attach attacks against other targets. In
/// addition, the cloaker halves the damage it takes (round down), and
/// the target takes the same amount of damage… The target or a creature
/// within 5 feet of it can take an action to try to detach the cloaker,
/// doing so by succeeding on a DC 14 Strength (Athletics) check."
///
/// The damage split is what makes a wrapped cloaker a different problem
/// from a loose one, and the reason it is a `shares_damage` flag rather
/// than a resistance: the halving is not a reduction, it is a *transfer*
/// — every point the cloaker does not take, the person underneath it
/// does. A party that keeps swinging is killing its own fighter, which
/// is the whole tactical shape of the creature and the reason the DC 14
/// pry is worth an Action.
///
/// No `host_conditions_max_size`: the Large gate above already stopped
/// the latch, so restating it on the blindness would be the same number
/// written twice.
static CLOAKER_ATTACH_PROFILE: AttachProfile = AttachProfile {
    verb: "folds itself over",
    max_host_size: Some(Size::Large),
    host_conditions: &[Condition::Blinded],
    shares_damage: true,
    pry_dc: Some(14),
    ..AttachProfile::defaults()
};

/// Cloaker — CR 8 aberration. Ray-like creature that wraps around its
/// prey: an Attach and two barbed tails at 10 ft reach, thrown together
/// by `CLOAKER_MULTI`, over the top of the **Moan** that is the reason
/// the thing is CR 8 at all. See `monster_attacks::CLOAKER_MOAN` for
/// the 60 ft fear sweep and which of RAW's two gates (hearing,
/// aberration) the engine can actually ask about.
///
/// The attach clause on the Attach is `CLOAKER_ATTACH_PROFILE` above —
/// a Large-or-smaller victim goes Blinded, every blow aimed at the
/// cloaker is split down the middle between the two of them, and it
/// takes a DC 14 Strength (Athletics) check to peel off. Ground speed
/// 10 ft (flying speed 40 ft not tracked). Darkvision 60 ft.
pub static CLOAKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLOAKER_TAIL);
    actions.push(&CLOAKER_ATTACH);
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
        attach: Some(&CLOAKER_ATTACH_PROFILE),
        // SRD 5.2 "Immunities Frightened". A thing that hunts by
        // dropping out of the dark onto people is not frightened of
        // anything in the dark.
        condition_immunities: HashSet::from([Condition::Frightened]),
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
