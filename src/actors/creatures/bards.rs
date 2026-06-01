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

/// Bard PC template. CHA-primary half-caster with a support-flavored
/// spell list (heals, debuffs, crowd-control). Headline mechanic:
/// **Bardic Inspiration** — bonus action that grants an ally the
/// Inspired condition (flat +3 to their next attack roll or save).
///
/// Loadout: Vicious Mockery / Cure Wounds / Healing Word as workhorse
/// cantrip + heals, Heroism / Bless / Charm Person / Faerie Fire /
/// Protection from Evil and Good for support, Hold Person / Suggestion
/// / Mass Healing Word for higher-leverage utility, plus a Scimitar
/// for when the slots run dry. PC flag flips on so the bard enters
/// Dying at 0 HP rather than dropping outright.
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
    // Cutting Words — Bard signature defensive feature, once per short
    // rest. Applies Mocked (disadvantage on next attack) to one enemy
    // within 60ft. Collapsing the RAW reactive cast into a bonus action
    // pre-empt loses some flavor but slots cleanly into the action
    // pipeline without a reaction-trigger framework.
    actions.push(&*CUTTING_WORDS);
    CreatureTemplate {
        name: "Bard",
        glyph: 'B',
        ac: 14,
        hitpoints: "5d8+5".parse().unwrap(),
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
        cr: 1.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Level-5 half-caster slots: 4/3/2. Plenty of slots for the
        // CC + heal staples, with two level-3 slots for high-leverage
        // Suggestion / Mass Healing Word casts per encounter.
        spell_slots_by_level: vec![4, 3, 2],
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
