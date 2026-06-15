use crate::actions::class_features::{
    BARDIC_INSPIRATION, BARDIC_INSPIRATION_TAG, CUTTING_WORDS, CUTTING_WORDS_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    BLESS, CHARM_PERSON, CURE_WOUNDS, DISSONANT_WHISPERS, FAERIE_FIRE, HEALING_WORD, HEROISM,
    HOLD_PERSON, MASS_HEALING_WORD, PROTECTION_FROM_EVIL_AND_GOOD, SUGGESTION, VICIOUS_MOCKERY,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Bard PC template. CHA-primary full caster with a support-flavored
/// spell list (heals, debuffs, crowd-control). Headline mechanic:
/// **Bardic Inspiration** — bonus action that grants an ally the
/// Inspired condition (flat +3 to their next attack roll or save).
///
/// Loadout: Vicious Mockery / Cure Wounds / Healing Word as workhorse
/// cantrip + heals, Heroism / Bless / Charm Person / Faerie Fire /
/// Protection from Evil and Good for support, Hold Person / Suggestion
/// / Mass Healing Word for higher-leverage utility, **Compulsion** at
/// the lv4 capstone for crowd-control, plus a Scimitar for when the
/// slots run dry. PC flag flips on so the bard enters Dying at 0 HP
/// rather than dropping outright.
pub static BARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*VICIOUS_MOCKERY);
    actions.push(&*BARDIC_INSPIRATION);
    actions.push(&*CURE_WOUNDS);
    actions.push(&HEALING_WORD);
    actions.push(&*HEROISM);
    actions.push(&*BLESS);
    actions.push(&*CHARM_PERSON);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SUGGESTION);
    actions.push(&*MASS_HEALING_WORD);
    actions.push(&*PROTECTION_FROM_EVIL_AND_GOOD);
    // Dissonant Whispers — lv1 enchantment (bard-only RAW). 3d6 psychic
    // WIS save-for-half + on-fail forced-move flee away from the caster
    // at the target's full walking speed (routed through `PushActor`).
    // Gives the bard a lv1 damage-with-control option to round out the
    // existing save-or-suck lineup (Charm Person / Faerie Fire / Sleep).
    actions.push(&*DISSONANT_WHISPERS);
    // Latest bard additions:
    //   - cantrip **Blade Ward**: self damage-resistance till next turn.
    //     A defensive cantrip alternative when the bard is out of slots
    //     and Vicious Mockery is the only offense.
    //   - lv1 **Earth Tremor**: self-centered DEX-save bludgeoning +
    //     prone. The bard gets a clean lv1 AoE option to pair with the
    //     Dissonant Whispers single-target lane.
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::EARTH_TREMOR);
    actions.push(&*crate::actions::spells::SILVERY_BARBS);
    actions.push(&*crate::actions::spells::CLOUD_OF_DAGGERS);
    actions.push(&*crate::actions::spells::HEALING_SPIRIT);
    // Compulsion — lv4 enchantment (bard-only RAW), concentration. Self-
    // centered 12-tile burst; each enemy in range makes a WIS save vs the
    // bard's CHA-based DC or is Charmed by the bard for the duration
    // (their `charmed_by` link points at the bard, blocking the engine's
    // existing hostile-action gate). The bard's flagship lv4 crowd-control
    // option — slots between Charm Person (lv1) / Hypnotic Pattern (lv3,
    // here on wizard / warlock only) / Charm Monster (lv4, single-target)
    // / Mass Suggestion (lv6) on the enchantment ladder.
    actions.push(&*crate::actions::spells::COMPULSION);
    // Cutting Words — Bard signature defensive feature, once per short
    // rest. Applies Mocked (disadvantage on next attack) to one enemy
    // within 60ft. Collapsing the RAW reactive cast into a bonus action
    // pre-empt loses some flavor but slots cleanly into the action
    // pipeline without a reaction-trigger framework.
    actions.push(&*CUTTING_WORDS);
    // lv1 **Longstrider** (transmutation): touch +10 ft speed for 1 hour,
    // no concentration. Bard's pre-combat ally mobility buff — pairs
    // cleanly with Bardic Inspiration's accuracy bump and the bard's
    // role as the party's tempo-setter.
    actions.push(&*crate::actions::spells::LONGSTRIDER);
    // lv2 **Enhance Ability** (transmutation): touch ally buff — 2d6 temp
    // HP + flat +2 saves for the duration (concentration). Slots cleanly
    // into the bard's support lane next to Bless / Heroism — the
    // single-target temp HP differentiates it from Bless's burst attack-
    // roll buff and Heroism's flat-mod temp HP.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    // lv2 **Pyrotechnics** (transmutation, XGtE): 2-radius CON-save fire
    // burst (1d8) + Blinded-on-fail. Cheap entry-tier elemental burst on
    // the bard's lv2 lane — complements the bard's existing crowd-control
    // toolkit (Hold Person, Suggestion) with a typed-damage option.
    actions.push(&*crate::actions::spells::PYROTECHNICS);
    CreatureTemplate {
        name: "Bard",
        glyph: 'B',
        ac: 14,
        hitpoints: "7d8+7".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 16, // primary spellcasting ability + Bardic Inspiration die
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Level-7 full-caster slots: 4/3/3/1. The lv4 slot fuels exactly
        // one Compulsion per encounter (the bard's flagship crowd-control
        // option), while the lv1-3 spread covers the CC + heal staples
        // (Healing Word / Bless / Charm Person / Hold Person / Suggestion
        // / Mass Healing Word).
        spell_slots_by_level: vec![4, 3, 3, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Bards are proficient in DEX and CHA saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([BARDIC_INSPIRATION_TAG, CUTTING_WORDS_TAG]),
        regen_per_round: 0,
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
        has_extra_attack: false,
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
