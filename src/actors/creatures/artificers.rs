use crate::actions::class_features::{
    ALCHEMICAL_SAVANT_TAG, ARCANE_FIREARM_TAG, ARCANE_JOLT_TAG, DEFENSIVE_FIELD,
    DEFENSIVE_FIELD_TAG, ELDRITCH_CANNON_TAG, EXPERIMENTAL_ELIXIR, EXPERIMENTAL_ELIXIR_TAG,
    FLASH_OF_GENIUS_TAG, LIGHTNING_LAUNCHER_TAG, STEEL_DEFENDER_TAG, SUMMON_FLAMETHROWER_CANNON,
    SUMMON_FORCE_BALLISTA_CANNON, SUMMON_PROTECTOR_CANNON, SUMMON_STEEL_DEFENDER,
    THUNDER_GAUNTLETS_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    ARCANE_INFUSED_WEAPON, LIGHTNING_LAUNCHER, SHORTSWORD, THUNDER_GAUNTLETS,
};
use crate::actions::spells::{
    ABSORB_ELEMENTS, ACID_SPLASH, AID, BLINK, BLUR, CATAPULT, CREATE_BONFIRE, CURE_WOUNDS,
    DISPEL_MAGIC, ELEMENTAL_WEAPON, ENHANCE_ABILITY, ENLARGE_REDUCE, EXPEDITIOUS_RETREAT,
    FAERIE_FIRE, FALSE_LIFE, FIRE_BOLT, FLAME_ARROWS, FLY, FREEDOM_OF_MOVEMENT, GREASE, GUIDANCE,
    HASTE, HEAT_METAL, INVISIBILITY, LESSER_RESTORATION, LEVITATE, LONGSTRIDER, MAGIC_STONE,
    MAGIC_WEAPON, OTILUKES_RESILIENT_SPHERE, PROTECTION_FROM_ENERGY, PROTECTION_FROM_POISON,
    PYROTECHNICS, RAY_OF_FROST, REVIVIFY, SANCTUARY, SEE_INVISIBILITY, SHOCKING_GRASP,
    SPARE_THE_DYING, SPIDER_CLIMB, STONESKIN, SWORD_BURST, TASHAS_CAUSTIC_BREW, THORN_WHIP,
    THUNDERCLAP, WEB,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Artificer PC template — the thirteenth class on the roster and the
/// first half-caster keyed to Intelligence.
///
/// **What the class is for.** Every caster already here answers the
/// question "what do I do on my turn". The Artificer answers "what did I
/// build", and the difference shows up in the shape of a turn rather
/// than in its contents: three of the four subclasses spend their
/// opening Action putting something permanent on the board — a turret, a
/// construct, a flask — and then spend the rest of the fight fighting
/// beside it. That makes the Artificer the only chassis whose first
/// round is reliably worse than a wizard's and whose fifth is reliably
/// better.
///
/// **Half-caster, Intelligence-anchored.** Four slot tiers rather than
/// nine, on a d8 hit die with medium armour, which puts the envelope
/// between the ranger's and the wizard's: AC 17 and 55-odd hit points
/// with a spell list that tops out at level 4. Intelligence 18 anchors
/// the save DC, and it is the same score the Armorer's gauntlets, the
/// Battle Smith's sword and the Artillerist's cannon all swing off —
/// the class's design principle is that the artificer has exactly one
/// number and everything they made reads it.
///
/// **The spell list is the class's other half.** No damage above
/// Vitriolic Sphere's tier and nothing resembling a Fireball: the
/// artificer's list is buffs, restoration and battlefield furniture —
/// Web, Grease, Faerie Fire, Heat Metal, Blur, Haste, Freedom of
/// Movement, Revivify, Stoneskin. That is the trade for the constructs.
/// A party fielding an artificer is not fielding a second blaster; it is
/// fielding somebody who makes the blaster it already has hit harder and
/// live longer, plus a machine.
///
/// **Flash of Genius** is the one baseline feature with an engine
/// surface, and it is the reason the artificer is worth standing next
/// to: once per fight, a nearby ally's failed save becomes a passed one.
/// It fires from the artificer's reaction on somebody else's roll, which
/// no other feature on the roster does — see `FLASH_OF_GENIUS_TAG`.
///
/// RAW's Magical Tinkering, Infuse Item, The Right Tool for the Job and
/// Spell-Storing Item are all out of scope for the same reason: each is
/// an out-of-combat crafting or utility lane, and the engine's unit of
/// play is an encounter. Infuse Item's mechanical residue — the +1
/// weapon and armour every artificer build actually takes — is folded
/// into the chassis's AC and into the Battle Smith's infused weapon
/// rather than modelled as an inventory step.
pub static ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SHORTSWORD);
    // Cantrips. The artificer's list is the tool-user's: two attack
    // cantrips, a burst, a heal-adjacent, and the two utility rows that
    // read at an engine site (Guidance's check bonus, Spare the Dying's
    // stabilise).
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*ACID_SPLASH);
    actions.push(&*SHOCKING_GRASP);
    actions.push(&*SWORD_BURST);
    actions.push(&*THUNDERCLAP);
    actions.push(&*MAGIC_STONE);
    actions.push(&*THORN_WHIP);
    actions.push(&*CREATE_BONFIRE);
    actions.push(&*GUIDANCE);
    // Light — the evocation cantrip every one of these classes has on
    // its list, and the party's answer to an unlit board: touch an ally
    // (or yourself) and they carry 20 ft of bright light and 20 ft of
    // dim light with them for the rest of the fight. Declines to cast
    // on a board that is already bright, and declines to re-light
    // somebody who is already lit, so it costs nothing on the ambient
    // default and is there when the lights are out.
    actions.push(&*crate::actions::spells::LIGHT);
    // Dancing Lights — the cantrip that lights ground the party has
    // not walked onto yet. Free and remote at once, which neither the
    // Light cantrip (free, but has to be touched onto somebody) nor
    // Daylight (remote, but a level-3 slot) manages. Holds
    // concentration, so it competes with the real spells rather than
    // stacking on them, and declines to cast on a board that is
    // already bright. See `spells::DANCING_LIGHTS`.
    actions.push(&*crate::actions::spells::DANCING_LIGHTS);
    actions.push(&*SPARE_THE_DYING);
    // Level 1 — the whole RAW tier that has a combat surface.
    actions.push(&*CURE_WOUNDS);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*FALSE_LIFE);
    actions.push(&*GREASE);
    actions.push(&*SANCTUARY);
    actions.push(&*LONGSTRIDER);
    actions.push(&*EXPEDITIOUS_RETREAT);
    actions.push(&*ABSORB_ELEMENTS);
    actions.push(&*CATAPULT);
    actions.push(&*TASHAS_CAUSTIC_BREW);
    // Level 2
    actions.push(&*AID);
    actions.push(&*BLUR);
    actions.push(&*ENHANCE_ABILITY);
    actions.push(&*ENLARGE_REDUCE);
    actions.push(&*HEAT_METAL);
    actions.push(&*INVISIBILITY);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*LEVITATE);
    actions.push(&*MAGIC_WEAPON);
    actions.push(&*PROTECTION_FROM_POISON);
    actions.push(&*PYROTECHNICS);
    actions.push(&*SEE_INVISIBILITY);
    actions.push(&*SPIDER_CLIMB);
    actions.push(&*WEB);
    // Level 3
    actions.push(&*BLINK);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*ELEMENTAL_WEAPON);
    actions.push(&*FLAME_ARROWS);
    actions.push(&*FLY);
    actions.push(&*HASTE);
    actions.push(&*PROTECTION_FROM_ENERGY);
    actions.push(&*REVIVIFY);
    // Level 4
    actions.push(&*FREEDOM_OF_MOVEMENT);
    actions.push(&*OTILUKES_RESILIENT_SPHERE);
    // The two TCE spells on the artificer's own list. Rime's Binding
    // Ice is the chassis's only cone, and Intellect Fortress is the
    // clearest fit of anything in this batch — the artificer's RAW
    // list carries it, and a half-caster in the front rank is exactly
    // who wants psychic resistance and mental-save advantage.
    actions.push(&*crate::actions::spells::RIMES_BINDING_ICE);
    actions.push(&*crate::actions::spells::INTELLECT_FORTRESS);
    actions.push(&*STONESKIN);
    CreatureTemplate {
        name: "Artificer",
        glyph: 'A',
        // Half plate (15) + DEX 12 (+1) + the shield every artificer
        // build carries, which is also where RAW's Enhanced Defense
        // infusion lands. See the type docs on Infuse Item.
        ac: 17,
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 12,
        constitution: 14,
        // The one number the whole class reads.
        intelligence: 18,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common, Language::Gnomish]),
        cr: 1.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // 5e Artificer save proficiencies: Constitution and
        // Intelligence (PHB-style class table, TCE).
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
        ]),
        // Half-caster slot table at the level the chassis is written to
        // (13): 4/3/3/1. The ranger and paladin carry the same shape at
        // a lower tier, which is what "half-caster" means here.
        spell_slots_by_level: vec![4, 3, 3, 1],
        features: HashSet::from([
            FLASH_OF_GENIUS_TAG,
            // 5e **Feather Fall** — on the artificer list RAW, and the
            // second reaction on this chassis whose window belongs to
            // somebody else's misfortune. See
            // `EncounterInstance::try_feather_fall`.
            crate::actions::class_features::FEATHER_FALL_TAG,
        ]),
        skills: HashSet::from([Skill::Arcana, Skill::Perception]),
        ..CreatureTemplate::defaults()
    }
});

/// Alchemist Artificer — the subclass that answers the round with a
/// flask.
///
/// **Experimental Elixir** is a bonus action that rolls for its own
/// effect, which makes the Alchemist the only chassis on the roster
/// whose best turn cannot be planned: healing, a Haste-shaped posture,
/// or an AC bump, and the alchemist finds out at the same time everyone
/// else does. **Alchemical Savant** then quietly makes the artificer's
/// acid / fire / necrotic / poison spells the ones worth casting, which
/// is why the baseline list's Tasha's Caustic Brew and Create Bonfire
/// stop being filler on this chassis and start being the plan.
///
/// The pairing is the subclass. Every other artificer builds one thing
/// and then fights beside it; the Alchemist builds a different thing
/// every fight and bends its own spell list around whichever one came
/// out.
///
/// RAW's Restorative Reagents (a free Lesser Restoration per rest, plus
/// temp HP on every elixir) and Chemical Mastery (acid and poison
/// resistance) are folded away: the first is a slot-economy feature on
/// a spell the baseline chassis already carries, and the second is a
/// level-15 defensive ribbon on a chassis written to 13.
pub static ALCHEMIST_ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = ARTIFICER_TEMPLATE.actions.clone();
    actions.push(&*EXPERIMENTAL_ELIXIR);
    CreatureTemplate {
        name: "Alchemist Artificer",
        glyph: 'L',
        actions,
        features: HashSet::from([
            FLASH_OF_GENIUS_TAG,
            EXPERIMENTAL_ELIXIR_TAG,
            ALCHEMICAL_SAVANT_TAG,
        ]),
        ..ARTIFICER_TEMPLATE.clone()
    }
});

/// Armorer Artificer (Guardian model) — the subclass that answers the
/// round by standing in front of you.
///
/// Two features that only make sense together. **Thunder Gauntlets**
/// swing off Intelligence for 1d8 thunder and leave whatever they hit at
/// disadvantage against anybody but the artificer; **Defensive Field**
/// is a bonus action for a slab of temporary hit points. One says "hit
/// me instead", the other says "that was a mistake", and the chassis
/// carries no other melee weapon precisely so the first one is honest —
/// see `THUNDER_GAUNTLETS`.
///
/// This is the artificer that plays like a fighter, and it is the only
/// build on the roster that taunts by hitting rather than by shouting:
/// the Cavalier's Unwavering Mark and Compelled Duel install the same
/// `Dueled` condition, but one costs a Channel-Divinity-shaped resource
/// and the other costs a spell slot and concentration. This one costs a
/// swing the artificer was making anyway.
///
/// AC 18 rather than the baseline 17 — RAW's Arcane Armor is heavy
/// plate the artificer is permanently attuned to, and the Guardian model
/// is its front-line configuration.
pub static ARMORER_ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // The gauntlets *replace* the shortsword rather than joining it: the
    // Guardian's mark is keyed to the holder rather than to the weapon,
    // and a second melee option on the list would make the mark fire off
    // a swing RAW does not attach it to.
    let mut actions: Vec<&'static (dyn crate::actions::action_template::Action + Send + Sync)> =
        ARTIFICER_TEMPLATE
            .actions
            .iter()
            .copied()
            .filter(|a| a.name() != SHORTSWORD.display_name)
            .collect();
    actions.push(&THUNDER_GAUNTLETS);
    actions.push(&*DEFENSIVE_FIELD);
    CreatureTemplate {
        name: "Armorer Artificer",
        glyph: 'R',
        ac: 18,
        actions,
        features: HashSet::from([
            FLASH_OF_GENIUS_TAG,
            THUNDER_GAUNTLETS_TAG,
            DEFENSIVE_FIELD_TAG,
        ]),
        ..ARTIFICER_TEMPLATE.clone()
    }
});

/// Armorer Artificer (Infiltrator model) — the Guardian's sibling, and
/// the same armour configured for the opposite job.
///
/// **Lightning Launcher** is a 1d6 lightning shot with 90 ft of normal
/// range and 300 ft of maximum — the longest reach any PC weapon on the
/// roster carries — plus an extra 1d6 on the turn's first connecting
/// shot. So the Infiltrator's damage is 2d6 once a turn and 1d6
/// afterwards, against the Guardian's flat 1d8 every swing: the same
/// budget spent on one good shot rather than on every one.
///
/// The models are one subclass in RAW, swapped between long rests. Here
/// they are two templates for the reason every other either/or on the
/// roster is — the engine instantiates a template and the choice is made
/// before initiative, which is exactly when RAW makes it too.
///
/// No Defensive Field and no mark: RAW gives the Infiltrator model
/// Powered Steps and Dampening Field instead, which are a speed bump and
/// a stealth bonus. What survives on this chassis is the launcher and
/// the reach, which is what the model is picked for.
pub static INFILTRATOR_ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = ARTIFICER_TEMPLATE.actions.clone();
    actions.push(&LIGHTNING_LAUNCHER);
    CreatureTemplate {
        name: "Infiltrator Artificer",
        glyph: 'I',
        actions,
        features: HashSet::from([FLASH_OF_GENIUS_TAG, LIGHTNING_LAUNCHER_TAG]),
        ..ARTIFICER_TEMPLATE.clone()
    }
});

/// Artillerist Artificer — the subclass that answers the round by
/// building a gun.
///
/// **Eldritch Cannon** is one Action for a turret that fights for the
/// rest of the encounter, in one of three modes the artificer picks once
/// and lives with: a fire burst, a force sniper, or a pulse of temporary
/// hit points over the front line. All three share
/// `ELDRITCH_CANNON_TAG`, which is how "you can have only one cannon at
/// a time" is spelled here — calling any one spends the charge the other
/// two needed.
///
/// **Arcane Firearm** is the other half, and it is why the Artillerist
/// is not simply a summoner: a d8 on one damage roll of every spell the
/// artificer casts, with no gate at all beyond the action being a spell.
/// The Wildfire Druid's Enhanced Bond is the same die on the same
/// cohort and asks where the druid's spirit is standing; this asks
/// nothing. That makes the Artillerist the most reliable of the four
/// and the least positional, which is a strange thing to say about the
/// one that plants a turret — but the turret is the round-one
/// investment and the firearm is what the other nine rounds are made
/// of.
///
/// RAW's Explosive Cannon (detonate the turret for 3d8) is left out: it
/// is a second Action that destroys the thing the first Action bought,
/// and the engine's summons have no self-destruct lane.
pub static ARTILLERIST_ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = ARTIFICER_TEMPLATE.actions.clone();
    actions.push(&SUMMON_FLAMETHROWER_CANNON);
    actions.push(&SUMMON_FORCE_BALLISTA_CANNON);
    actions.push(&SUMMON_PROTECTOR_CANNON);
    CreatureTemplate {
        name: "Artillerist Artificer",
        glyph: 'Y',
        actions,
        features: HashSet::from([
            FLASH_OF_GENIUS_TAG,
            ELDRITCH_CANNON_TAG,
            ARCANE_FIREARM_TAG,
        ]),
        ..ARTIFICER_TEMPLATE.clone()
    }
});

/// Battle Smith Artificer — the subclass that answers the round with a
/// dog.
///
/// **Steel Defender** is the sturdiest body on the feature-summon lane:
/// AC 15, thirty-odd hit points, a 1d8 force swing, and immunity to the
/// poison and fear that take a wolf out of a fight without killing it.
/// **Battle Ready** moves the artificer's own swing onto Intelligence —
/// `ARCANE_INFUSED_WEAPON` is that clause, written as a weapon for the
/// reason the Astral Self Monk's arms are — and **Arcane Jolt** puts
/// 2d6 force on the turn's first connecting hit, the biggest die on
/// that whole cohort.
///
/// The three together make the Battle Smith the artificer that fights.
/// It carries the weakest weapon of the four on paper — a d8 longsword
/// against the Infiltrator's 300 ft launcher and the Artillerist's
/// turret — and lands the most damage of the four on a single target,
/// because the jolt rides whatever it hits and the defender is hitting
/// too.
pub static BATTLE_SMITH_ARTIFICER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // The infused weapon *replaces* the shortsword rather than joining
    // it, the same substitution the Guardian makes with its gauntlets:
    // RAW's Battle Ready moves the artificer's swing onto Intelligence,
    // and a DEX shortsword left on the list would be a strictly worse
    // weapon competing for the same Action.
    let mut actions: Vec<&'static (dyn crate::actions::action_template::Action + Send + Sync)> =
        ARTIFICER_TEMPLATE
            .actions
            .iter()
            .copied()
            .filter(|a| a.name() != SHORTSWORD.display_name)
            .collect();
    actions.push(&ARCANE_INFUSED_WEAPON);
    actions.push(&SUMMON_STEEL_DEFENDER);
    CreatureTemplate {
        name: "Battle Smith Artificer",
        glyph: 'S',
        actions,
        features: HashSet::from([FLASH_OF_GENIUS_TAG, STEEL_DEFENDER_TAG, ARCANE_JOLT_TAG]),
        // RAW's Extra Attack lands at Battle Smith level 5 and is the
        // other half of why this is the martial artificer.
        has_extra_attack: true,
        ..ARTIFICER_TEMPLATE.clone()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn instantiate(template: &'static LazyLock<CreatureTemplate>) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// Every artificer swings, casts and shields off one number. That is
    /// the class's design principle, and the thing that would silently
    /// break it is a subclass literal that forgets `..ARTIFICER_TEMPLATE`
    /// and re-types the stat line.
    #[test]
    fn every_artificer_anchors_on_intelligence() {
        for t in [
            &ARTIFICER_TEMPLATE,
            &ALCHEMIST_ARTIFICER_TEMPLATE,
            &ARMORER_ARTIFICER_TEMPLATE,
            &INFILTRATOR_ARTIFICER_TEMPLATE,
            &ARTILLERIST_ARTIFICER_TEMPLATE,
            &BATTLE_SMITH_ARTIFICER_TEMPLATE,
        ] {
            let a = instantiate(t);
            assert!(
                a.ability_modifier(AbilityScoreType::Intelligence) >= 4,
                "{} lost the artificer's casting stat",
                a.name()
            );
            assert!(a.has_passive_feature(FLASH_OF_GENIUS_TAG));
        }
    }

    /// The Guardian's mark is keyed to the artificer rather than to the
    /// gauntlets, which is exact only for as long as the gauntlets are
    /// the chassis's one melee weapon. This is the assertion that keeps
    /// that true — a shortsword pushed back onto the list would make the
    /// mark fire off a swing RAW does not attach it to.
    #[test]
    fn the_guardian_carries_no_melee_weapon_but_its_gauntlets() {
        let armorer = instantiate(&ARMORER_ARTIFICER_TEMPLATE);
        assert!(
            armorer
                .actions
                .iter()
                .any(|a| a.name() == THUNDER_GAUNTLETS.display_name)
        );
        assert!(
            !armorer
                .actions
                .iter()
                .any(|a| a.name() == SHORTSWORD.display_name)
        );
    }

    /// "You can have only one cannon at a time", enforced by the shared
    /// charge rather than by a board scan. If the three declarations
    /// ever drifted onto separate tags an Artillerist would field three
    /// turrets on round three, which is a different subclass.
    #[test]
    fn the_three_cannons_share_one_charge() {
        assert_eq!(SUMMON_FLAMETHROWER_CANNON.tag, ELDRITCH_CANNON_TAG);
        assert_eq!(SUMMON_FORCE_BALLISTA_CANNON.tag, ELDRITCH_CANNON_TAG);
        assert_eq!(SUMMON_PROTECTOR_CANNON.tag, ELDRITCH_CANNON_TAG);
        let artillerist = instantiate(&ARTILLERIST_ARTIFICER_TEMPLATE);
        assert!(artillerist.feature_available(ELDRITCH_CANNON_TAG));
    }
}
