//! **Goliaths** — SRD 5.2's giant-blooded species, and the roster's
//! sixth playable one after the Dragonborn, Tiefling, Dwarf, Halfling,
//! Half-Orc, Gnome and Aasimar builds.
//!
//! Six templates, one per **Giant Ancestry** benefit, laid out the way
//! `dragonborn` lays out its fifteen: one shared chassis, and the
//! ancestry is the only thing that differs. That is exactly how RAW
//! writes it — a goliath chooses one of six supernatural boons and is
//! otherwise a goliath — and it is why the family reads as six rows
//! rather than six stat blocks.
//!
//! ## The chassis
//!
//! Every one of the six is a level-3-ish fighter build, sized to sit
//! beside `HALF_ORC_TEMPLATE` on the bench rather than above it: AC 16,
//! 3d10+9 hit points, STR 17 / CON 16, a greatclub for the two-handed
//! swing and a handaxe to throw. Second Wind and Action Surge come
//! along for the same reason they do on the dwarf and the half-orc — a
//! species template still needs a class under it, and the fighter
//! chassis is the one that gets out of the species' way.
//!
//! The greatclub is a deliberate pick over the half-orc's greataxe:
//! two average points lighter, and **reach 2** in exchange, which is
//! the one weapon property that compounds with Large Form. A Large
//! goliath swinging a ten-foot club threatens a ring two tiles wider
//! than the body it grew, and that ring is what the species is for.
//!
//! What is *not* the half-orc's: **Speed 35**, which RAW gives the
//! goliath and gives nobody else on the roster, and which the species
//! then extends to 45 for whoever spends their Large Form. A martial
//! that closes 45 feet and then threatens ten more is a genuinely
//! different piece to play against a caster line.
//!
//! ## What every goliath carries
//!
//! **Powerful Build** and **Large Form**, on all six. See
//! `crate::actions::species` for what each does and where it is read.
//!
//! ## Where the ancestries live
//!
//! Nowhere here. Five of the six are one row on a cohort that already
//! existed — the damage-rider table, the on-hit mark table, the
//! reactive-clamp table, the reflect table — and the sixth is an action
//! beside Large Form in `actions::species`. These templates carry a tag
//! apiece and nothing else, which is the whole argument for the cohort
//! shape: a new ancestry is a row, not a stat block.

use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATCLUB, THROWN_HANDAXE};
use crate::actions::species::{
    CLOUDS_JAUNT, CLOUDS_JAUNT_TAG, FIRES_BURN_TAG, FROSTS_CHILL_TAG, HILLS_TUMBLE_TAG,
    LARGE_FORM, LARGE_FORM_TAG, POWERFUL_BUILD_TAG, STONES_ENDURANCE_TAG, STORMS_THUNDER_TAG,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The shared goliath chassis, with no Giant Ancestry chosen.
///
/// Not registered as a playable template on its own — RAW makes the
/// ancestry a required choice at character creation, so a goliath
/// without one is half a species. It exists as the base every ancestry
/// template clones, which is the same relationship
/// `dragonborn::DRAGONBORN_TEMPLATE` has to its fifteen colours except
/// that the dragonborn's base *is* playable (its breath has a default
/// damage type; Giant Ancestry has no default benefit).
fn goliath_chassis(
    name: &'static str,
    glyph: char,
    ancestry: &'static str,
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATCLUB);
    actions.push(&THROWN_HANDAXE);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    actions.push(&*LARGE_FORM);
    CreatureTemplate {
        name,
        // Greek capitals, one per ancestry — the same scheme
        // `dragonborn` uses for its fifteen colours, and for the same
        // reason: a family whose members differ in one trait has to be
        // told apart on the map, and every Latin capital a goliath
        // might want is already carrying a humanoid. See each template
        // for which letter and why.
        glyph,
        ac: 16,
        // 3d10+9 — the half-orc's pool exactly, so the species reads as
        // a lateral pick beside it rather than a strictly better one.
        // Everything the goliath has over the half-orc it pays for by
        // giving up Savage Attacks and Relentless Endurance.
        hitpoints: "3d10+9".parse().unwrap(),
        // RAW: "Speed: 35 feet." The only 35 on the roster's humanoid
        // bench, and the species' quietest advantage — an extra tile of
        // closing every round, before Large Form's +10 is counted.
        speed: 35.,
        strength: 17,
        dexterity: 12,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 10,
        // Athletics is the skill Powerful Build is written about: the
        // grapple-escape contest picks the better of Athletics and
        // Acrobatics, and a goliath proficient in neither would be
        // rolling a bare d20 with advantage. RAW hands out no skill
        // with the species, but the chassis under it is a fighter, and
        // Athletics is the fighter's pick.
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 2.0,
        // RAW: "Size: Medium (about 7–8 feet tall)." The seven feet are
        // flavour — a goliath occupies one tile like anything else
        // Medium, until Large Form says otherwise.
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        features: HashSet::from([
            POWERFUL_BUILD_TAG,
            LARGE_FORM_TAG,
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            ancestry,
        ]),
        ..CreatureTemplate::defaults()
    }
}

/// **Cloud's Jaunt** — *"As a Bonus Action, you magically teleport up
/// to 30 feet to an unoccupied space you can see."*
///
/// The odd one out of the six, and the most interesting to play: every
/// other ancestry makes the goliath better at the fight it is already
/// in, and this one decides which fight that is. A martial chassis with
/// a blink is a martial chassis that can get past a front line and
/// stand on the caster behind it, twice a day, without spending a slot
/// it does not have.
///
/// It is also the one the AI reaches for unprompted, and the direction
/// it reaches in is the interesting part. The jaunt is a row on both
/// `SELF_TELEPORT_APPROACHES` and `SELF_TELEPORT_ESCAPES`, but the
/// escape rung declines for anyone whose melee lane beats their ranged
/// one — which is every goliath — so in practice the AI spends it going
/// *in*: a cloud goliath with a caster six tiles away arrives next to
/// it on a bonus action and still has its Action to swing with.
///
/// Glyph '\u{03a9}' (omega) — an arch with its feet apart, which is a
/// cloud bank seen from below. Unused elsewhere on the roster.
pub static CLOUD_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut template = goliath_chassis("Cloud Goliath", '\u{03a9}', CLOUDS_JAUNT_TAG);
    template.actions.push(&*CLOUDS_JAUNT);
    template
});

/// **Fire's Burn** — *"When you hit a target with an attack roll and
/// deal damage to it, you can also deal 1d10 Fire damage to that
/// target."*
///
/// The straightforward one: the biggest single damage die on the
/// engine's once-per-turn rider cohort, twice a day. Worth the least
/// against exactly the enemies a party most wants a martial to hit —
/// fire is the most-resisted type in the bestiary — which is the
/// ancestry's real cost and is not visible in the number.
///
/// Glyph '\u{03a8}' (psi) — three tines rising off one stem, the
/// shape of a flame. Unused elsewhere on the roster.
pub static FIRE_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| goliath_chassis("Fire Goliath", '\u{03a8}', FIRES_BURN_TAG));

/// **Frost's Chill** — *"you can also deal 1d6 Cold damage to that
/// target and reduce its Speed by 10 feet until the start of your next
/// turn."*
///
/// Fire's Burn with four points of average damage traded for a slow,
/// and the better trade in almost every fight this engine runs: a
/// −10 ft on something trying to reach the party's back line is worth
/// more than a d10, and cold is resisted by a fraction of what fire is.
///
/// Glyph '\u{039e}' (xi) — three stacked strokes, read here as strata
/// of ice. Shared with the Invisible Stalker, which is in no PC
/// family and so cannot collide where it matters.
pub static FROST_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| goliath_chassis("Frost Goliath", '\u{039e}', FROSTS_CHILL_TAG));

/// **Hill's Tumble** — *"When you hit a Large or smaller creature with
/// an attack roll and deal damage to it, you can give that target the
/// Prone condition."*
///
/// No damage at all and the strongest of the six. Prone with no save,
/// off a swing the goliath was making anyway: the target spends half
/// its movement standing, every melee ally swings at advantage until it
/// does, and it attacks at disadvantage in the meantime.
///
/// The size clause is the one thing that turns it off, and it turns it
/// off against precisely the enemies a party would most like to knock
/// down — the Huge and Gargantuan end of the bestiary, where the
/// dragons and the giants live.
///
/// Glyph '\u{03a0}' (pi) — two uprights under a lintel, a standing
/// stone. Shared with the Dao, which is in no PC family.
pub static HILL_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| goliath_chassis("Hill Goliath", '\u{03a0}', HILLS_TUMBLE_TAG));

/// **Stone's Endurance** — *"When you take damage, you can take a
/// Reaction to roll 1d12. Add your Constitution modifier to the number
/// rolled and reduce the damage by that total."*
///
/// The defensive pick, and the only one of the six that answers
/// something the goliath did not choose to be in. On this chassis' CON
/// 16 it takes an average of 9.5 off a blow — most of a hit, twice a
/// day — which reads on the board as a fighter that takes one more
/// round to go down at exactly the moment that matters.
///
/// Glyph '\u{03a3}' (sigma) — folded and squared off, a block of
/// worked stone. Shared with the War Magic Wizard, which is a
/// different family and so disambiguated by name in the initiative
/// list.
pub static STONE_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| goliath_chassis("Stone Goliath", '\u{03a3}', STONES_ENDURANCE_TAG));

/// **Storm's Thunder** — *"When you take damage from a creature within
/// 60 feet of you, you can take a Reaction to deal 1d8 Thunder damage
/// to that creature."*
///
/// Stone's Endurance's mirror: the same reaction, the same pool, spent
/// forward instead of back. Thunder is one of the least-resisted types
/// on the roster, and the retaliation reaches whoever hit the goliath
/// rather than whoever it can walk to — which makes it the ancestry
/// that most often lands damage on an archer.
///
/// Glyph '\u{0396}' (zeta) — the one Greek capital that is a
/// lightning zigzag on its own. Unused elsewhere on the roster.
pub static STORM_GOLIATH_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| goliath_chassis("Storm Goliath", '\u{0396}', STORMS_THUNDER_TAG));
