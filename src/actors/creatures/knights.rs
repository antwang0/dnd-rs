use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, KNIGHT_MULTI, LANCE, LONGSWORD};
use crate::conditions::Condition;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Knight — CR 3 plate-armored melee specialist. Plate (AC 18) + heavy
/// crossbow ranged option + double longsword multiattack at the high
/// end. The lance gives them a reach-2 swing option for opening
/// engagements. Pair them with mooks to soak attacks for the big swing.
///
/// SRD 5.2's knight prints *"Immunities Frightened"* and a `+2` WIS
/// save, and the block below carries both — the immunity on
/// `condition_immunities`, the save on `proficient_saves`. An older
/// version of this docstring said the immunity had been *"lifted to a
/// proficient WIS save instead, since the engine doesn't yet model the
/// fear-immunity nuance"*, which was a claim about an engine that had
/// a `condition_immunities` field the whole time.
///
/// The **Parry** reaction ships too — *"the knight adds 2 to its AC
/// against that attack, possibly causing it to miss"* — on
/// `parry_bonus` below, which is the field the whole SRD parry list
/// reads. It is the other half of what makes a knight the thing a party
/// puts between itself and a dragon.
///
/// The **Leadership** reaction is still skipped: it needs a
/// shouted-orders channel nothing in the engine has.
pub static KNIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    actions.push(&LANCE);
    actions.push(&HEAVY_CROSSBOW);
    actions.push(&*KNIGHT_MULTI);
    CreatureTemplate {
        name: "Knight",
        // 'K' for knight — distinct from the existing letter pool.
        glyph: 'K',
        ac: 18,
        hitpoints: "8d8+16".parse().unwrap(),
        strength: 16,
        dexterity: 11,
        constitution: 14,
        intelligence: 11,
        wisdom: 11,
        charisma: 15,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        // SRD 5.2 "Immunities Frightened" — the knight's **Brave**, and
        // the whole reason a knight is the thing a party puts between
        // itself and a dragon's Frightful Presence.
        condition_immunities: HashSet::from([Condition::Frightened]),
        actions,
        // Knights are proficient in CON and WIS saves (5e MM); WIS
        // proficiency stands in for the Brave / Bravery features.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        has_extra_attack: true,
        // The Mounted Combatant feat. A knight's stat block carries a
        // Lance, which is a weapon that exists to be used from a horse
        // — RAW gives it disadvantage against anything within 5 feet
        // and a reach of 10 — and the MM entry describes knights as
        // "warriors who have sworn fealty… frequently mounted". Now
        // that `engine::mounts` exists, the AI's mount rung puts one on
        // any warhorse that spawns beside it, and the feat is what
        // makes that worth doing. See `feats::MOUNTED_COMBATANT_TAG`.
        features: HashSet::from([crate::actions::feats::MOUNTED_COMBATANT_TAG]),
        // SRD 5.2 **Parry** (Reaction): *"the knight adds 2 to its AC
        // against that attack, possibly causing it to miss."* The
        // other half of what makes a knight the thing a party puts
        // between itself and a dragon — the Frightened immunity keeps
        // them standing there and this keeps them standing.
        parry_bonus: 2,
        ..CreatureTemplate::defaults()
    }
});
