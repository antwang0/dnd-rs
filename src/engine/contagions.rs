//! **Magical Contagions** — SRD 5.2's *Curses and Magical Contagions*,
//! and the one section of the Gameplay Toolbox that is written entirely
//! in clauses this engine already knows how to say.
//!
//! > *Alchemists, potion brewers, and areas of wild magic are credited
//! > with creating the first magical contagions. An outbreak of such a
//! > contagion can form the basis of an adventure as characters search
//! > for a cure and try to stop the contagion's spread.*
//!
//! Three are printed — **Cackle Fever**, **Sewer Plague** and **Sight
//! Rot** — and between them they use a saving throw, the Exhaustion
//! ladder, the Blinded and Incapacitated conditions, a start-of-turn
//! Emanation, an on-hit rider, a repeated end-of-turn save and the long
//! rest. Every one of those is a chokepoint the engine has had for a
//! while. What it did not have was the *ledger*: a fact about a
//! creature that outlives the fight it was caught in.
//!
//! ## Why a ledger on the actor and not a condition
//!
//! Because a contagion is the one status in the book that a night's
//! sleep is explicitly **not** enough to shake. `ActorInstance::
//! long_rest` ends with `conditions.clear()`, which is right for
//! everything the condition map holds — a fight's worth of poison,
//! fear, grapples and buffs — and would be exactly wrong here: *"At the
//! end of each Long Rest, an infected creature makes a DC 13
//! Constitution saving throw. After the creature succeeds on three of
//! these saves, the contagion ends on it."* A contagion that the rest
//! wiped could never be fought off, because it would never survive to
//! be rolled against.
//!
//! So [`Infection`] lives in its own field, beside `exhaustion` and
//! `suffocation_exhaustion` and for the same reason those are not
//! conditions either. What the contagion *does* is still spelled in
//! ordinary conditions — Sight Rot is `Blinded`, Cackle Fever's
//! laughter is [`Condition::Cackling`] — and those are re-applied by
//! the rest rather than surviving it, which is the right way round: the
//! symptom is downstream of the disease.
//!
//! ## The compression: 1d4 days becomes one night
//!
//! Two of the three print an incubation period — *"A creature suffers
//! the following effects 1d4 days after infection"* — and the engine's
//! clock has two granularities, the six-second round and the long rest
//! between one room of the dungeon and the next. There is no third.
//!
//! An incubation of 1d4 days therefore rounds to **the next long
//! rest**: you are wounded by the otyugh in room two, you bed down, and
//! you wake up in room three with a level of Exhaustion. That is a
//! compression and it is named as one — but it is the compression that
//! keeps the rule playable, and it preserves the thing the rule is
//! *for*: the fight where you catch it is never the fight where it
//! costs you. [`Infection::incubating`] is the flag, and
//! [`ActorInstance::contagion_night`] is the sunrise.
//!
//! ## What is in and what is out
//!
//! | clause | where it lives |
//! |--------|----------------|
//! | Sewer Plague's carriers (otyugh, rats) | `CreatureTemplate::carries` → the tail of `attack::push_on_hit_riders` |
//! | Cackle Fever's 10-ft spread | `EncounterInstance::spread_contagions`, at the top of each turn |
//! | Cackle Fever's laughter | `EncounterInstance::cackle_at_damage`, on the damage chokepoint |
//! | the laughter's end-of-turn escape | [`CACKLING_ESCAPE`], through `engine::repeat_saves` |
//! | Sight Rot's Blinded | re-applied every sunrise by `contagion_night` |
//! | the "three successes" cure, the dawn save | `contagion_night` |
//! | Sewer Plague's Weakness and Restlessness | `ActorInstance::short_rest` / `long_rest` |
//! | Sight Rot's *"a Heal or Lesser Restoration spell ends it"* | `ActorInstance::cure_contagions_by_magic` |
//! | the 24-hour "not from that creature again" | the encounter's `trait_immunities` ledger |
//!
//! Three clauses are deliberately absent, and they are absent for the
//! same reason the six hour-long *Environmental Effects* are absent
//! from [`crate::engine::weather`] — the engine has no surface to hang
//! them on:
//!
//!   - **Sight Rot's herbalism ointment.** *"A character who is
//!     proficient with an Herbalism Kit can use it to create one dose
//!     of nonmagical ointment, which takes 1 hour."* Three doses over
//!     72 hours is a rule about a week, and there is no tool
//!     proficiency layer to ask.
//!   - **Sight Rot's skin contact.** *"Any Humanoid that makes skin
//!     contact with a creature infected with Sight Rot must succeed on
//!     a DC 15 Constitution saving throw."* An Emanation has a radius
//!     and this does not — "skin contact" is not something the board
//!     models, and reading it as adjacency would make Sight Rot spread
//!     faster than Cackle Fever, which is backwards. It is reachable
//!     as a carrier rider instead: a creature that *declares* it in
//!     `carries` passes it on with a wounding blow, which is at least
//!     contact.
//!   - **Rest and Recuperation.** *"If a creature infected with a
//!     magical contagion spends 3 days recuperating … the creature has
//!     Advantage on saving throws to fight off the magical contagion
//!     for the next 24 hours."* Three days of doing nothing is not a
//!     thing a party in a dungeon can spend.
//!
//! ## Where an outbreak starts
//!
//! Sewer Plague names its carriers in RAW — *"sometimes transmitted by
//! creatures that dwell in such areas, including otyughs and rats"* —
//! so it needs no help: the bestiary carries it, and a party that
//! fights an otyugh is exposed to it the ordinary way.
//!
//! The other two name none. Cackle Fever comes out of *"cheaply made
//! potions and elixirs"* and Sight Rot out of *"water tainted by Sight
//! Rot"*, neither of which is a creature and neither of which is
//! something that happens during a fight. They enter a run the way the
//! rain and the traps do: as a fact about the *place*, asked for on the
//! command line and carried from room to room —
//! [`crate::engine::board::BoardSettings::outbreak`]. That is not a
//! liberty so much as the book's own framing; the section opens by
//! calling an outbreak the basis of an adventure.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::repeat_saves::RepeatSave;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageType};

/// Which of SRD 5.2's three sample contagions this is.
///
/// A small `Copy` tag rather than a `&'static Contagion` on the actor's
/// ledger, because the ledger is cloned across the encounter boundary
/// with the party and compared for equality in a dozen places; an enum
/// makes both free. [`ContagionKind::row`] is the one-hop widening back
/// to the printed entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContagionKind {
    CackleFever,
    SewerPlague,
    SightRot,
}

impl ContagionKind {
    /// Every contagion the book prints, in the order it prints them.
    /// The list any sweep should walk rather than re-typing three
    /// variants.
    pub const ALL: [ContagionKind; 3] = [
        ContagionKind::CackleFever,
        ContagionKind::SewerPlague,
        ContagionKind::SightRot,
    ];

    /// The printed entry this tag stands for.
    pub const fn row(self) -> &'static Contagion {
        match self {
            ContagionKind::CackleFever => &CACKLE_FEVER,
            ContagionKind::SewerPlague => &SEWER_PLAGUE,
            ContagionKind::SightRot => &SIGHT_ROT,
        }
    }

    /// The contagion's name as printed — "Cackle Fever".
    pub const fn name(self) -> &'static str {
        self.row().name
    }

    /// Parse a command-line spelling, for
    /// [`crate::engine::board::BoardSettings::outbreak`]. `None` for
    /// anything unrecognized, so the caller can report it rather than
    /// guess — the contract [`crate::engine::weather::Weather::parse`]
    /// established.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "cackle" | "cacklefever" | "fever" => Some(ContagionKind::CackleFever),
            "sewer" | "sewerplague" | "plague" => Some(ContagionKind::SewerPlague),
            "sight" | "sightrot" | "rot" => Some(ContagionKind::SightRot),
            _ => None,
        }
    }

    /// Every spelling `parse` accepts as a *flag*, for the help text and
    /// for the sweep that pins the two in agreement.
    pub const NAMES: &'static [&'static str] = &["cackle", "sewer", "sight"];
}

/// One sample contagion from SRD 5.2's *Magical Contagions*, in the
/// shape the book prints it.
///
/// Every field is a clause of the entry, so a row can be checked
/// against the page line by line — the bargain
/// [`crate::engine::poisons::Poison`] and [`crate::engine::traps::Trap`]
/// both make, and for the same reason: three contagions that differ by
/// four numbers should differ by four numbers here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contagion {
    /// The contagion's name as printed. Doubles as the log tag and as
    /// the third key of the encounter's 24-hour immunity ledger, so a
    /// creature that has shrugged off one carrier's Sewer Plague is
    /// still in danger from the next carrier's.
    pub name: &'static str,
    /// RAW's *"must succeed on a DC N Constitution saving throw or
    /// become infected"* — the number rolled at the moment of exposure.
    ///
    /// Always a Constitution save in all three entries, which is why
    /// there is no ability field: a contagion that asked for something
    /// else would be a different kind of thing, and the day the book
    /// prints one this grows a sibling to [`Self::escape_save`].
    pub exposure_dc: i32,
    /// The creature types that can catch it at all — RAW's *"affects
    /// Humanoids only"* and *"Any Beast or Humanoid"*. Empty would mean
    /// "anything", and nothing in this section says that.
    pub susceptible: &'static [CreatureType],
    /// Exhaustion levels the onset of symptoms grants — RAW's *Fever*
    /// and *Fatigue* lines. Zero for Sight Rot, whose whole symptom is
    /// the blindness.
    pub onset_exhaustion: u32,
    /// Whether that Exhaustion level is *held* for as long as the
    /// contagion lasts.
    ///
    /// The one-word difference between the two entries that grant a
    /// level, and it is worth a field because it is most of what makes
    /// them feel different at the table. Cackle Fever's is *"1
    /// Exhaustion level, **which lasts until the contagion ends on the
    /// creature**"* — a floor, so the night's ordinary −1 cannot pay it
    /// off and the fever's second symptom stays armed. Sewer Plague's
    /// is a bare *"gains 1 Exhaustion level"*, and its own dawn save is
    /// what moves the number afterwards.
    pub onset_exhaustion_persists: bool,
    /// RAW's *"A creature suffers the following effects **1d4 days
    /// after infection**"*, which two of the three entries print and
    /// one does not.
    ///
    /// Declared rather than inferred. This was read off the shape of
    /// the row for one commit — *"an entry with a condition and no
    /// Exhaustion has no incubation"* — which is true of the three
    /// rows the book happens to print and is not a rule about
    /// anything. Sight Rot's symptoms land in the round it is caught
    /// because its entry has no incubation line, not because its
    /// symptom is a condition.
    pub incubates: bool,
    /// A condition the onset installs and the cure removes — Sight
    /// Rot's *"have the Blinded condition until the contagion ends"*.
    ///
    /// Re-installed at every sunrise rather than made permanent,
    /// because `long_rest` clears the condition map wholesale. See the
    /// module docstring on why the symptom is downstream of the
    /// disease.
    pub onset_condition: Option<Condition>,
    /// RAW's *"Fighting the Contagion"* save, as `(DC, how the failure
    /// and success branches read)`, or `None` for the one entry that
    /// gives the victim no roll at all.
    ///
    /// Sight Rot is that entry: *"Magic such as a Heal or Lesser
    /// Restoration spell ends the contagion immediately"*, and nothing
    /// else does. A creature that catches it and has no caster is
    /// blind for the rest of the dungeon, which is exactly what a
    /// DC-15 failure is supposed to buy.
    pub escape_save: Option<EscapeClause>,
    /// RAW's *"Spreading the Contagion"* Emanation, or `None`.
    pub spread: Option<Spread>,
    /// The clause of the log line for the moment symptoms begin —
    /// "wakes with a fever and cannot stop laughing".
    pub onset_flavor: &'static str,
    /// The clause of the log line for the moment it ends.
    pub cure_flavor: &'static str,
}

/// How a victim gets out from under a contagion between fights — RAW's
/// *"Fighting the Contagion"* paragraph, whose two entries read
/// completely differently on a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EscapeClause {
    /// The Constitution DC rolled once per long rest.
    pub dc: i32,
    /// How many successes end it, or `0` for the ladder branch below.
    ///
    /// Cackle Fever's *"after the creature succeeds on three of these
    /// saves, the contagion ends on it"* — cumulative, not consecutive,
    /// which is the same reading [`crate::engine::repeat_saves`] gives
    /// the identical sentence.
    pub successes_needed: u32,
    /// Sewer Plague's branch instead: *"On a failed save, the creature
    /// gains 1 Exhaustion level as its fatigue worsens. On a successful
    /// save, the creature's Exhaustion level decreases by 1. If the
    /// creature's Exhaustion level is reduced to 0, the contagion
    /// ends."*
    ///
    /// A `bool` rather than a second struct because the two branches
    /// are mutually exclusive in the book and because the whole of this
    /// one is "the counter is the Exhaustion ladder". A row that sets
    /// it ignores `successes_needed`.
    pub counted_in_exhaustion: bool,
}

/// RAW's *"Spreading the Contagion"* clause — a start-of-turn
/// Emanation that hands the contagion on.
///
/// Deliberately not a [`crate::engine::emanations::Emanation`], though
/// it is the same sentence with the same shape, and the difference is
/// the one that matters: an `Emanation` is a **fact about a stat
/// block**, declared on the template and never acquired, and what it
/// installs is a condition that lasts until the start of the victim's
/// next turn. This one is acquired, it is carried by whoever caught it
/// last, and what it installs is the thing that will be doing the
/// spreading tomorrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spread {
    /// RAW's *"within a 10-foot Emanation originating from a creature
    /// infected with"* it, in feet. Converted by [`Spread::radius_tiles`]
    /// on the same convention the paladin auras and the stat-block
    /// emanations share.
    pub radius_feet: u32,
    /// The DC of the save to avoid catching it from a neighbour.
    ///
    /// Distinct from [`Contagion::exposure_dc`] and lower in the one
    /// row that has both, which is RAW being deliberate: DC 10 to avoid
    /// catching Cackle Fever off a laughing ally, against the DC 13 its
    /// own symptoms are rolled at.
    pub dc: i32,
}

impl Spread {
    /// The clause's radius as a footprint-Chebyshev **gap** cap.
    ///
    /// Shares [`crate::engine::emanations::Emanation::radius_tiles`]'s
    /// convention exactly — 10 ft is 4 tiles on the 2.5 ft grid — so a
    /// contagion's ten feet and a hezrou's ten feet are the same ten
    /// feet. See there for why that convention rather than
    /// `MELEE_REACH`'s.
    pub const fn radius_tiles(&self) -> isize {
        crate::engine::util::tiles_from_feet(self.radius_feet) as isize
    }
}

impl Contagion {
    /// True if `victim_type` is one this contagion can catch at all.
    ///
    /// No empty-means-anything escape hatch, unlike
    /// [`crate::engine::emanations::Emanation::catches_type`]: all
    /// three entries name their types, and a row that named none would
    /// be a row somebody forgot to fill in rather than a plague of
    /// everything.
    pub fn catches_type(&self, victim_type: CreatureType) -> bool {
        self.susceptible.contains(&victim_type)
    }

    /// The Exhaustion level this contagion holds a symptomatic victim
    /// at — RAW's *"which lasts until the contagion ends on the
    /// creature"*, and `0` for the two entries that do not say it.
    ///
    /// Read by `ActorInstance::exhaustion_floor`, which is what stops
    /// the long rest's ordinary −1 from paying off a fever.
    pub const fn exhaustion_floor(&self) -> u32 {
        if self.onset_exhaustion_persists {
            self.onset_exhaustion
        } else {
            0
        }
    }
}

/// **Cackle Fever** (SRD 5.2, *Magical Contagions*).
///
/// > Cheaply made potions and elixirs are sometimes tainted by Cackle
/// > Fever, which affects Humanoids only (gnomes are strangely immune).
/// > A creature suffers the following effects 1d4 days after infection:
/// >
/// > **Fever.** The creature gains 1 Exhaustion level, which lasts
/// > until the contagion ends on the creature.
/// >
/// > **Uncontrollable Laughter.** While the creature has the Exhaustion
/// > condition, the creature makes a DC 13 Constitution saving throw
/// > each time it takes damage other than Psychic damage. On a failed
/// > save, the creature takes 5 (1d10) Psychic damage and has the
/// > Incapacitated condition as it laughs uncontrollably. At the end of
/// > each of its turns, the creature repeats the save, ending the
/// > effect on itself on a success. After 1 minute, it succeeds
/// > automatically.
/// >
/// > **Fighting the Contagion.** At the end of each Long Rest, an
/// > infected creature makes a DC 13 Constitution saving throw. After
/// > the creature succeeds on three of these saves, the contagion ends
/// > on it, and the creature is immune to Cackle Fever for 1 year.
/// >
/// > **Spreading the Contagion.** Any Humanoid (other than a gnome)
/// > that starts its turn within a 10-foot Emanation originating from a
/// > creature infected with Cackle Fever must succeed on a DC 10
/// > Constitution saving throw or also become infected with the
/// > contagion. On a successful save, the Humanoid can't catch the
/// > contagion from that particular infected creature for the next 24
/// > hours.
///
/// The one of the three with a **per-round** surface, and the reason
/// this module is worth having at all: a fevered fighter who takes a
/// goblin's scimitar to the ribs rolls a DC 13 Constitution save
/// against losing its entire next turn. Every other clause in the
/// section is measured in nights.
///
/// The gnome clause is the one piece of the entry that is not a number,
/// and it is spelled as an immunity on the sheet rather than as a
/// species check here — see `CreatureTemplate::contagion_immunities`.
pub static CACKLE_FEVER: Contagion = Contagion {
    name: "Cackle Fever",
    // Cackle Fever prints no exposure DC of its own — the only way to
    // catch it is the spread clause, which prints DC 10. The number is
    // repeated here so a carrier declared in `CreatureTemplate::carries`
    // (a tainted potion-seller, an outbreak seeded from the command
    // line) has a DC to roll against, and it is the spread's number
    // because that is the only one the book gives.
    exposure_dc: 10,
    susceptible: &[CreatureType::Humanoid],
    incubates: true,
    onset_exhaustion: 1,
    onset_exhaustion_persists: true,
    onset_condition: None,
    escape_save: Some(EscapeClause {
        dc: 13,
        successes_needed: 3,
        counted_in_exhaustion: false,
    }),
    spread: Some(Spread {
        radius_feet: 10,
        dc: 10,
    }),
    onset_flavor: "wakes shivering, with a laugh they cannot swallow",
    cure_flavor: "shakes off the last of the fever",
};

/// **Sewer Plague** (SRD 5.2, *Magical Contagions*).
///
/// > Fouled potions and alchemical waste can give rise to Sewer Plague,
/// > which incubates in sewers and refuse heaps and is sometimes
/// > transmitted by creatures that dwell in such areas, including
/// > otyughs and rats. Any Humanoid that is wounded by a creature that
/// > carries the contagion or that comes into contact with contaminated
/// > filth or offal must succeed on a DC 11 Constitution saving throw
/// > or become infected with Sewer Plague. A creature suffers the
/// > following effects 1d4 days after infection:
/// >
/// > **Fatigue.** The creature gains 1 Exhaustion level.
/// >
/// > **Weakness.** While the creature has any Exhaustion levels, it
/// > regains only half the normal number of Hit Points from spending
/// > Hit Point Dice.
/// >
/// > **Restlessness.** While the creature has any Exhaustion levels,
/// > finishing a Long Rest neither restores lost Hit Points nor reduces
/// > the creature's Exhaustion level.
/// >
/// > **Fighting the Contagion.** Daily at dawn, an infected creature
/// > makes a DC 11 Constitution saving throw. On a failed save, the
/// > creature gains 1 Exhaustion level as its fatigue worsens. On a
/// > successful save, the creature's Exhaustion level decreases by 1.
/// > If the creature's Exhaustion level is reduced to 0, the contagion
/// > ends on the creature.
///
/// The only one of the three with a **carrier** in the bestiary, which
/// makes it the only one a party can catch without anybody asking for
/// an outbreak: the otyugh in the cistern and the rats in the walls
/// both print it, and a wounding blow from either is an exposure.
///
/// It is also the nastiest, and it is nasty in a way no condition in
/// the engine had been able to be. *Restlessness* takes the long rest
/// away — the party wakes at the hit points they went to sleep on — and
/// *Weakness* halves what an hour of sitting down buys. A plagued party
/// that keeps pushing through the dungeon walks into each room worse
/// off than it left the last one, and the only way out is the dawn save
/// that is also the thing making them worse.
pub static SEWER_PLAGUE: Contagion = Contagion {
    name: "Sewer Plague",
    exposure_dc: 11,
    susceptible: &[CreatureType::Humanoid],
    incubates: true,
    onset_exhaustion: 1,
    // A bare "gains 1 Exhaustion level" — no holding clause. The dawn
    // save is what moves it afterwards, in both directions.
    onset_exhaustion_persists: false,
    onset_condition: None,
    escape_save: Some(EscapeClause {
        dc: 11,
        successes_needed: 0,
        counted_in_exhaustion: true,
    }),
    // "Transmitted by creatures that dwell in such areas" is a carrier
    // clause, not an Emanation: it rides a wounding blow. See
    // `CreatureTemplate::carries`.
    spread: None,
    onset_flavor: "wakes grey-faced and sweating",
    cure_flavor: "throws off the plague at last",
};

/// **Sight Rot** (SRD 5.2, *Magical Contagions*).
///
/// > Any Beast or Humanoid that drinks water tainted by Sight Rot must
/// > succeed on a DC 15 Constitution saving throw or have the Blinded
/// > condition until the contagion ends.
/// >
/// > **Fighting the Contagion.** Magic such as a Heal or Lesser
/// > Restoration spell ends the contagion immediately. A character who
/// > is proficient with an Herbalism Kit can use it to create one dose
/// > of nonmagical ointment, which takes 1 hour. When applied to the
/// > eyes of a creature suffering from Sight Rot, the ointment
/// > suppresses the contagion on that creature for 24 hours. If the
/// > contagion is suppressed in this way for a total of 72 hours
/// > (requiring three doses and applications of the ointment), the
/// > contagion ends on the creature.
/// >
/// > **Spreading the Contagion.** Any Humanoid that makes skin contact
/// > with a creature infected with Sight Rot must succeed on a DC 15
/// > Constitution saving throw or also become infected with the
/// > contagion.
///
/// The odd one out in three ways, all of which follow from the same
/// fact: it is the only entry whose symptom arrives **immediately**.
/// There is no "1d4 days after infection" line, so there is no
/// incubation to compress — a creature that fails the save is blind
/// before the round is out.
///
/// It is also the only one with no save to fight it off. The victim
/// does not get better; somebody has to cast something. That makes it
/// the one contagion whose cure is a *party* decision rather than a
/// night's rest, and it is why `Heal` and `Lesser Restoration` are
/// named on the row rather than left to a generic disease-cure lane the
/// engine does not have.
pub static SIGHT_ROT: Contagion = Contagion {
    name: "Sight Rot",
    exposure_dc: 15,
    susceptible: &[CreatureType::Beast, CreatureType::Humanoid],
    // The one entry with no "1d4 days after infection" line: a failed
    // save is blind before the round is out.
    incubates: false,
    onset_exhaustion: 0,
    onset_exhaustion_persists: false,
    onset_condition: Some(Condition::Blinded),
    // No "Fighting the Contagion" save: RAW gives the victim no roll at
    // all, only a spell.
    escape_save: None,
    // The skin-contact clause is not an Emanation — see the module
    // docstring for why reading it as adjacency would make Sight Rot
    // the fastest-spreading of the three, which is backwards.
    spread: None,
    onset_flavor: "blinks at a world gone white",
    cure_flavor: "blinks, and sees",
};

/// The `CreatureTemplate::carries` list for SRD 5.2 Sewer Plague's four
/// named reservoirs — *"creatures that dwell in such areas, including
/// otyughs and rats"*.
///
/// One constant rather than four identical one-element literals,
/// because it is one sentence in the book and because the four stat
/// blocks that read it should be greppable from here: the Otyugh, the
/// Rat, the Giant Rat and the Swarm of Rats.
pub static SEWER_PLAGUE_CARRIER: &[ContagionKind] = &[ContagionKind::SewerPlague];

/// One creature's standing with one contagion — the ledger row that
/// outlives the fight.
///
/// Deliberately *not* `Copy`-cheap-and-forgettable: this is the only
/// thing in the engine besides hit points, experience and the pack that
/// a party carries from one room of the dungeon into the next, and the
/// whole design of the section is about that persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Infection {
    pub kind: ContagionKind,
    /// True between catching it and the first sunrise — RAW's *"1d4
    /// days after infection"*, compressed to one night. See the module
    /// docstring.
    ///
    /// An incubating infection does nothing at all: no Exhaustion, no
    /// Blinded, no laughter, and — the clause worth being explicit
    /// about — **no spreading**. RAW does not say an incubating carrier
    /// is contagious, and reading it the other way would make one bad
    /// save at the start of a dungeon infect the whole party before
    /// anybody knew there was anything to save against.
    pub incubating: bool,
    /// Successful *"Fighting the Contagion"* saves banked so far,
    /// against [`EscapeClause::successes_needed`]. Cumulative, never
    /// reset — RAW says *"succeeds on three of these saves"*, not three
    /// in a row.
    pub successes: u32,
}

impl Infection {
    /// A fresh catch: incubating, nothing banked.
    pub const fn caught(kind: ContagionKind) -> Self {
        Infection {
            kind,
            incubating: true,
            successes: 0,
        }
    }

    /// A catch whose symptoms are already showing — what Sight Rot
    /// does, and what an outbreak seeded onto a board does to the
    /// creatures that are already carrying it when the fight opens.
    pub const fn symptomatic(kind: ContagionKind) -> Self {
        Infection {
            kind,
            incubating: false,
            successes: 0,
        }
    }

    /// The printed entry behind this row.
    pub const fn row(&self) -> &'static Contagion {
        self.kind.row()
    }
}

/// Cackle Fever's *"At the end of each of its turns, the creature
/// repeats the save, ending the effect on itself on a success. After 1
/// minute, it succeeds automatically."*
///
/// Registered through [`crate::engine::repeat_saves`], which is the
/// ledger this exact sentence was built for, and paired with a
/// `Rounds(10)` timer that is RAW's minute — the cap the escape runs
/// against, so a victim who never rolls a 13 stops laughing on schedule
/// rather than never.
pub static CACKLING_ESCAPE: RepeatSave = RepeatSave {
    name: "cackle fever",
    condition: Condition::Cackling,
    ability: AbilityScoreType::Constitution,
    escaped_flavor: "gets its breath back",
    damage_on_failure: None,
    successes_needed: 1,
};

/// RAW's *"After 1 minute, it succeeds automatically"*, as the
/// condition timer the escape runs against. Ten rounds is one minute.
pub const CACKLING_ROUNDS: ConditionTimer = ConditionTimer::Rounds(10);

/// RAW's *"the creature takes 5 (1d10) Psychic damage"* on a failed
/// laughter save.
pub const CACKLE_LAUGHTER_DICE: Dice = Dice::new(1, 10);

/// RAW's DC for that save — the same 13 the nightly *Fighting the
/// Contagion* roll uses, which is why it is read off the row rather
/// than written twice.
pub fn cackle_laughter_dc() -> i32 {
    CACKLE_FEVER
        .escape_save
        .map(|e| e.dc)
        .unwrap_or(CACKLE_FEVER.exposure_dc)
}

impl EncounterInstance {
    /// One exposure to `kind`: the Constitution save RAW asks for, and
    /// the ledger row a failure writes.
    ///
    /// **The** entry point — the spread sweep, the carrier's wounding
    /// blow and any future vector all resolve here, so the four clauses
    /// that are easy to forget are written once:
    ///
    ///   - the **type gate and the gnome clause**, through
    ///     `ActorInstance::can_catch`, which also refuses a creature
    ///     that already has it (RAW has no second dose);
    ///   - the **24-hour memory** — *"On a successful save, the
    ///     Humanoid can't catch the contagion from that particular
    ///     infected creature for the next 24 hours"* — on the
    ///     encounter's `trait_immunities` ledger, keyed by *(victim,
    ///     source, contagion name)* exactly as the stat-block
    ///     emanations key theirs. Scoped to the individual source, which
    ///     is RAW's own word, so surviving the otyugh says nothing
    ///     about the rats;
    ///   - the **incubation**, which is every entry's default and which
    ///     Sight Rot alone skips: it prints no *"1d4 days after
    ///     infection"* line, so its blindness lands in the round it was
    ///     caught;
    ///   - the **log line**, because a save nobody saw is a save that
    ///     looks like a bug three rooms later when the fever arrives.
    ///
    /// Returns `true` if the victim caught it.
    pub fn expose_to_contagion(
        &mut self,
        victim_id: usize,
        source_id: usize,
        kind: ContagionKind,
        dc: i32,
    ) -> bool {
        let row = kind.row();
        let Some(victim) = self.actors.get(&victim_id) else {
            return false;
        };
        if !victim.is_combat_active() || !victim.can_catch(kind) {
            return false;
        }
        if self.trait_immunity_banked(victim_id, source_id, row.name) {
            return false;
        }
        let victim_name = self.actor_name(victim_id);
        let source_name = self.actor_name(source_id);
        let save = self.roll_save(victim_id, AbilityScoreType::Constitution, dc);
        if save.passed() {
            // RAW's 24 hours is longer than any encounter, so the
            // ledger is written once and never swept — the reading
            // `engine::emanations` already settled for the identical
            // sentence.
            self.bank_trait_immunity(victim_id, source_id, row.name);
            self.log(format!(
                "  {}: {} is not taking it from {}.",
                row.name, victim_name, source_name
            ));
            return false;
        }
        // Sight Rot is the entry with no incubation line — its victim
        // is blind before the round is out. The other two take a night.
        let immediate = !row.incubates;
        if let Some(victim) = self.actors.get_mut(&victim_id) {
            victim.infect(kind, immediate);
        }
        self.log(if immediate {
            format!(
                "  {}: {} {}.",
                row.name, victim_name, row.onset_flavor
            )
        } else {
            format!(
                "  {}: {} has caught something from {}.",
                row.name, victim_name, source_name
            )
        });
        true
    }

    /// SRD 5.2's *"Spreading the Contagion"* Emanation, at the top of
    /// `actor_id`'s turn — today Cackle Fever's and nothing else's.
    ///
    /// A sweep over *carriers* rather than a hook on the carrier's own
    /// turn, for the reason `apply_hostile_emanations` next door is one:
    /// RAW's trigger is *"any Humanoid that **starts its turn** within a
    /// 10-foot Emanation originating from a creature infected with
    /// Cackle Fever"*, which is a fact about the victim's turn and not
    /// about anything the carrier does. A carrier that is Stunned,
    /// Prone, unconscious-but-alive or three rounds from its own
    /// initiative slot is still laughing.
    ///
    /// **Not scoped to enemies**, which is the one place this
    /// deliberately parts company with the emanation sweep beside it.
    /// Those are stat-block traits and the engine reads all four as
    /// hostile-only because a monster whose signature trait poisons its
    /// own escort reads as a bug. A plague is the opposite: catching it
    /// from the person next to you is the entire clause, the book says
    /// *"any Humanoid"* with no qualifier, and a contagion that stopped
    /// politely at the party line would not be one.
    ///
    /// Silent and nearly free on the overwhelmingly common turn — the
    /// board carries no infections at all and the sweep leaves on the
    /// first line.
    pub(crate) fn spread_contagions(&mut self, actor_id: usize) {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(victim) = self.actors.get(&actor_id) else {
            return;
        };
        if !victim.is_combat_active() {
            return;
        }
        // The cheap gate: almost every board in the game has nobody on
        // it carrying anything, and this is the one line that costs.
        if !self.actors.values().any(|a| a.is_symptomatic()) {
            return;
        }
        let victim_loc = victim.location();
        let victim_span = get_tiles_from_size(victim.size());

        // Gathered before anything resolves, in ascending id order, so
        // two carriers flanking one victim resolve the same way on
        // every run of a seed — the contract `sorted_actor_ids` exists
        // for, and the same one `apply_hostile_emanations` keeps.
        let mut exposures: Vec<(usize, ContagionKind, i32)> = Vec::new();
        for carrier_id in self.sorted_actor_ids() {
            if carrier_id == actor_id {
                continue;
            }
            let Some(carrier) = self.actors.get(&carrier_id) else {
                continue;
            };
            if !carrier.is_symptomatic() || !carrier.is_combat_active() {
                continue;
            }
            // A wall between the two is a wall between the two. Same
            // gate, same pair-scoped reading as the emanation sweep —
            // see `total_cover_separates`.
            if self.total_cover_separates(carrier_id, actor_id) {
                continue;
            }
            let gap = footprint_chebyshev(
                carrier.location(),
                get_tiles_from_size(carrier.size()),
                victim_loc,
                victim_span,
            );
            for infection in carrier.infections() {
                // An incubating carrier is not contagious. See
                // `Infection::incubating`.
                if infection.incubating {
                    continue;
                }
                let Some(spread) = infection.row().spread else {
                    continue;
                };
                if gap > spread.radius_tiles() {
                    continue;
                }
                exposures.push((carrier_id, infection.kind, spread.dc));
            }
        }
        for (carrier_id, kind, dc) in exposures {
            self.expose_to_contagion(actor_id, carrier_id, kind, dc);
        }
    }

    /// SRD 5.2 **Cackle Fever**'s *Uncontrollable Laughter*, at the
    /// moment the clause names: *"the creature makes a DC 13
    /// Constitution saving throw **each time it takes damage other than
    /// Psychic damage**."*
    ///
    /// Called from the damage chokepoint once the blow has actually
    /// landed, which is the right reading of "takes damage" and the same
    /// one the flinch cohort and the regeneration suppressor beside it
    /// use: a hit the victim's resistances swallowed entirely was never
    /// felt.
    ///
    /// Three gates before a die is rolled, and all three are RAW's:
    ///
    ///   - the fever's symptoms must be showing (an incubating one does
    ///     nothing at all);
    ///   - *"While the creature has the Exhaustion condition"* — which
    ///     is not redundant with the first, because the fever's own
    ///     level can be paid off by a Greater Restoration while the
    ///     contagion is still on the ledger;
    ///   - the damage must not be Psychic, or the spell's own 1d10
    ///     would re-trigger it in a loop.
    ///
    /// Returns the effects the failure owes, for the caller to apply —
    /// the damage chokepoint is mid-pipeline on the victim's sheet and
    /// is not a place to recurse into itself.
    pub(crate) fn cackle_at_damage(&mut self, victim_id: usize, damage_type: DamageType) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};

        if matches!(damage_type, DamageType::Psychic) {
            return;
        }
        let Some(victim) = self.actors.get(&victim_id) else {
            return;
        };
        if !victim.has_symptoms_of(ContagionKind::CackleFever)
            || victim.exhaustion_level() == 0
            || !victim.is_combat_active()
        {
            return;
        }
        // Already laughing: RAW's repeat is the end-of-turn one, not a
        // fresh roll per blow, and re-registering would reset the
        // minute the escape runs against.
        if victim.has_condition(Condition::Cackling) {
            return;
        }
        let name = self.actor_name(victim_id);
        let dc = cackle_laughter_dc();
        let save = self.roll_save_vs_condition(
            victim_id,
            AbilityScoreType::Constitution,
            dc,
            Condition::Cackling,
        );
        if save.passed() {
            return;
        }
        let rolled = self.roll(&CACKLE_LAUGHTER_DICE);
        self.log(format!(
            "  cackle fever: {} doubles up laughing ({} psychic).",
            name, rolled
        ));
        DealDamage {
            actor_id: victim_id,
            amount: rolled,
            damage_type: DamageType::Psychic,
        }
        .apply(self);
        // The victim can have been killed by its own laughter, which is
        // a death this engine is happy to print and not one to install a
        // condition after.
        if self
            .actors
            .get(&victim_id)
            .is_none_or(|a| !a.is_combat_active())
        {
            return;
        }
        // RAW's minute is the cap; the ledger is the escape hatch. See
        // `CACKLING_ESCAPE`.
        //
        // **`None` for the source, not the victim's own id.** There is
        // nobody to name — the fever was caught days ago and whoever
        // passed it on may not be on this board — and naming the victim
        // is the trap `PendingRepeat::source_id` documents: every
        // caster-side rider on `CASTER_SAVE_MODE_RIDERS` would then
        // read the victim's own sheet as the source's, so a sorcerer
        // would spend a Heightened Spell prime giving *itself*
        // Disadvantage and an Arcane Trickster would be dragged out of
        // hiding by its own cough.
        self.begin_repeat_save(victim_id, None, dc, &CACKLING_ESCAPE, CACKLING_ROUNDS);
    }

    /// Seed an outbreak onto the monster side of a freshly generated
    /// board — [`crate::engine::board::BoardSettings::outbreak`].
    ///
    /// Scoped to the creatures that can catch it at all, and
    /// symptomatic from the start: an outbreak the party walks into is
    /// one that has already happened. The party is deliberately left
    /// out — they are what the outbreak is *for*, and infecting them at
    /// generation would spend the whole rule before the first round.
    ///
    /// Returns how many creatures it took, so the caller can say so.
    pub fn seed_outbreak(&mut self, kind: ContagionKind) -> usize {
        let mut seeded = 0;
        for id in self.sorted_actor_ids() {
            let Some(actor) = self.actors.get_mut(&id) else {
                continue;
            };
            if actor.team() == 0 || !actor.can_catch(kind) {
                continue;
            }
            if actor.infect(kind, true) {
                seeded += 1;
            }
        }
        if seeded > 0 {
            self.log(format!(
                "{} has taken hold here: {} of the locals are already sick.",
                kind.name(),
                seeded
            ));
        }
        seeded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row's tag round-trips through `row()`, which is the one
    /// hop everything else in the module takes and the one place a
    /// copy-pasted arm would go unnoticed.
    #[test]
    fn every_tag_names_its_own_row() {
        assert_eq!(ContagionKind::CackleFever.row().name, "Cackle Fever");
        assert_eq!(ContagionKind::SewerPlague.row().name, "Sewer Plague");
        assert_eq!(ContagionKind::SightRot.row().name, "Sight Rot");
        for kind in ContagionKind::ALL {
            assert_eq!(kind.name(), kind.row().name);
        }
    }

    /// The type filters are RAW's, and the differences between them are
    /// the point: Sight Rot is the only one a horse can catch, and none
    /// of the three touches the skeleton standing next to it.
    #[test]
    fn the_type_filters_are_the_books_own() {
        assert!(CACKLE_FEVER.catches_type(CreatureType::Humanoid));
        assert!(!CACKLE_FEVER.catches_type(CreatureType::Beast));
        assert!(SEWER_PLAGUE.catches_type(CreatureType::Humanoid));
        assert!(!SEWER_PLAGUE.catches_type(CreatureType::Beast));
        assert!(SIGHT_ROT.catches_type(CreatureType::Humanoid));
        assert!(SIGHT_ROT.catches_type(CreatureType::Beast));
        for row in [&CACKLE_FEVER, &SEWER_PLAGUE, &SIGHT_ROT] {
            assert!(!row.catches_type(CreatureType::Undead));
            assert!(!row.catches_type(CreatureType::Construct));
            assert!(!row.catches_type(CreatureType::Ooze));
        }
    }

    /// Only Cackle Fever holds its Exhaustion level against the night's
    /// −1, and that one word is most of what separates the two entries
    /// that grant one.
    #[test]
    fn only_the_fever_holds_its_exhaustion_level() {
        assert_eq!(CACKLE_FEVER.exhaustion_floor(), 1);
        assert_eq!(SEWER_PLAGUE.exhaustion_floor(), 0);
        assert_eq!(SIGHT_ROT.exhaustion_floor(), 0);
        // …though both grant one on onset.
        assert_eq!(CACKLE_FEVER.onset_exhaustion, 1);
        assert_eq!(SEWER_PLAGUE.onset_exhaustion, 1);
        assert_eq!(SIGHT_ROT.onset_exhaustion, 0);
    }

    /// Sight Rot is the only entry with no way out but a spell, and
    /// Cackle Fever the only one that spreads on its own. Pinned
    /// because both absences are load-bearing rather than unfilled
    /// fields.
    #[test]
    fn the_two_absences_are_the_books_and_not_omissions() {
        assert!(SIGHT_ROT.escape_save.is_none());
        assert!(CACKLE_FEVER.escape_save.is_some());
        assert!(SEWER_PLAGUE.escape_save.is_some());

        assert!(CACKLE_FEVER.spread.is_some());
        assert!(SEWER_PLAGUE.spread.is_none());
        assert!(SIGHT_ROT.spread.is_none());
    }

    /// The two branches of *Fighting the Contagion* are exclusive: a
    /// row counts successes or it counts Exhaustion, never both.
    #[test]
    fn the_escape_branches_do_not_overlap() {
        for kind in ContagionKind::ALL {
            let Some(escape) = kind.row().escape_save else {
                continue;
            };
            assert_ne!(
                escape.counted_in_exhaustion,
                escape.successes_needed > 0,
                "{} sets both escape branches or neither",
                kind.name()
            );
        }
    }

    /// The spread radius rides the paladin-aura convention, so a
    /// contagion's ten feet and a hezrou's ten feet are the same ten
    /// feet.
    #[test]
    fn ten_feet_of_contagion_is_ten_feet_of_stench() {
        use crate::engine::emanations::HEZROU_STENCH;
        let spread = CACKLE_FEVER.spread.expect("cackle fever spreads");
        assert_eq!(spread.radius_feet, HEZROU_STENCH.radius_feet);
        assert_eq!(spread.radius_tiles(), HEZROU_STENCH.radius_tiles());
    }

    /// The spread DC is lower than the DC the fever's own symptoms are
    /// rolled at, which is RAW being deliberate rather than an
    /// accident: catching it off a neighbour is easier to avoid than
    /// living with it.
    #[test]
    fn catching_it_is_easier_to_resist_than_carrying_it() {
        let spread = CACKLE_FEVER.spread.expect("cackle fever spreads");
        assert_eq!(spread.dc, 10);
        assert_eq!(cackle_laughter_dc(), 13);
        assert!(spread.dc < cackle_laughter_dc());
    }

    /// Every advertised flag parses back to a contagion, and every
    /// contagion has at least one flag that reaches it.
    #[test]
    fn every_advertised_outbreak_flag_parses() {
        let mut reached = Vec::new();
        for name in ContagionKind::NAMES {
            let parsed = ContagionKind::parse(name)
                .unwrap_or_else(|| panic!("{name} is advertised but does not parse"));
            reached.push(parsed);
        }
        for kind in ContagionKind::ALL {
            assert!(
                reached.contains(&kind),
                "no advertised flag reaches {}",
                kind.name()
            );
        }
        assert_eq!(
            ContagionKind::parse("Cackle-Fever"),
            Some(ContagionKind::CackleFever)
        );
        assert_eq!(ContagionKind::parse("puce"), None);
    }

    /// A fresh catch incubates and a seeded one does not, and neither
    /// starts with anything banked.
    #[test]
    fn a_fresh_catch_incubates_and_a_seeded_one_does_not() {
        let caught = Infection::caught(ContagionKind::SewerPlague);
        assert!(caught.incubating);
        assert_eq!(caught.successes, 0);
        let seeded = Infection::symptomatic(ContagionKind::SewerPlague);
        assert!(!seeded.incubating);
        assert_eq!(seeded.successes, 0);
        assert_eq!(caught.row().name, seeded.row().name);
    }
}
