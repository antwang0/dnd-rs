use crate::actions::class_features::{BEAR_SPIRIT_TAG, UNICORN_SPIRIT_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::ELEMENTAL_CONDITION_IMMUNITIES;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The shared body of a Circle of the Shepherd totem — an incorporeal
/// spirit that stands where it was called and does nothing but be
/// somewhere.
///
/// **It is the first creature in the bestiary with no attack at all.**
/// The wildfire spirit shoots, the tentacle lashes, the drake bites; a
/// totem's entire contribution is the thirty feet around it. That makes
/// it the purest version of the "your summon's position is your
/// resource" build the engine already has three of, and the one where
/// the summoner's decision is undiluted by whether the summon is also
/// worth having as a body.
///
/// RAW's spirit "can't be attacked" and has no hit points at all. The
/// engine has no unattackable lane — every occupant of a tile is a
/// target — so the totem is given a small pool and a wide immunity
/// envelope instead. The departure is real and it cuts the right way:
/// an enemy that spends a turn punching a spirit is an enemy not
/// punching the druid, and a totem that dies takes the aura with it,
/// which is a decision the enemy AI is allowed to make.
///
/// Speed 0 for the reason the tentacle's is: RAW roots it where it
/// appeared, and the engine's mover reads `speed()` directly, so a zero
/// here is the whole implementation of "it does not move".
fn spirit_totem_template(
    name: &'static str,
    glyph: char,
    beacon: &'static str,
) -> CreatureTemplate {
    CreatureTemplate {
        name,
        glyph,
        // Nothing to hit and nothing to dodge with. AC 10 keeps the
        // spirit from being a wall an enemy has to solve; what protects
        // it is the immunity envelope and the fact that hitting it
        // achieves nothing but ending the aura.
        ac: 10,
        hitpoints: "2d6".parse().unwrap(),
        speed: 0.,
        strength: 1,
        dexterity: 10,
        constitution: 10,
        intelligence: 1,
        wisdom: 14,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Celestial,
        // The default list and nothing else — no attack, which is the
        // whole point. It can still Dodge, which is what a spirit that
        // wants to keep an aura up should be doing anyway.
        actions: DEFAULT_ACTIONS.clone(),
        // Poison is the closest the engine gets to RAW's "it has no
        // body"; the rest of the incorporeal envelope is the shared
        // elemental condition set.
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        features: HashSet::from([beacon]),
        ..CreatureTemplate::defaults()
    }
}

/// Bear Spirit — the totem a Circle of the Shepherd druid calls when
/// what the party needs is to still be standing next round.
///
/// Its whole payout happens at the moment it arrives: everyone within
/// thirty feet takes on a shield of temporary hit points, and the
/// spirit then stands there having already done its job. Which makes it
/// the one totem whose *placement* barely matters and whose *timing*
/// matters entirely — a bear called before the party has closed up is a
/// bear that shielded two people instead of four.
///
/// Glyph 'r' (lowercase) — for bea**r**, paired with the Bear Shepherd
/// druid's 'R' the way the wildfire spirit's 'w' pairs with its druid's
/// 'W'.
pub static BEAR_SPIRIT_TOTEM_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| spirit_totem_template("Bear Spirit", 'r', BEAR_SPIRIT_TAG));

/// Unicorn Spirit — the totem whose payout is spread across the rest of
/// the fight instead of spent on arrival.
///
/// While it stands, every healing spell the druid casts also spills
/// onto each ally inside the aura. That inverts the bear's trade: the
/// unicorn does nothing on the turn it is called, and everything on
/// every turn afterwards where the druid spends a slot on a heal — so
/// where it stands is the decision, and a unicorn planted where the
/// party will *be* is worth more than one planted where the party is.
///
/// Glyph 'u' (lowercase) — for **u**nicorn, paired with the Unicorn
/// Shepherd druid's 'U'.
pub static UNICORN_SPIRIT_TOTEM_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| spirit_totem_template("Unicorn Spirit", 'u', UNICORN_SPIRIT_TAG));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn totem(template: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// Each totem carries its own beacon and only its own — the two
    /// auras are different features and a druid who called one must not
    /// collect the other's payout.
    #[test]
    fn each_totem_carries_exactly_its_own_beacon() {
        let bear = totem(&BEAR_SPIRIT_TOTEM_TEMPLATE);
        assert!(bear.has_passive_feature(BEAR_SPIRIT_TAG));
        assert!(!bear.has_passive_feature(UNICORN_SPIRIT_TAG));
        let unicorn = totem(&UNICORN_SPIRIT_TOTEM_TEMPLATE);
        assert!(unicorn.has_passive_feature(UNICORN_SPIRIT_TAG));
        assert!(!unicorn.has_passive_feature(BEAR_SPIRIT_TAG));
    }

    /// Rooted and unarmed: the two facts that make a totem a place
    /// rather than a combatant.
    #[test]
    fn a_totem_neither_moves_nor_swings() {
        for template in [&*BEAR_SPIRIT_TOTEM_TEMPLATE, &*UNICORN_SPIRIT_TOTEM_TEMPLATE] {
            assert_eq!(template.speed, 0.);
            assert!(
                template.actions.iter().all(|a| !a.deals_damage()),
                "{} brought an attack to a placement feature",
                template.name
            );
        }
    }
}
