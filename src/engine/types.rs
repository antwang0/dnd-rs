use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityScoreType {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

impl fmt::Display for AbilityScoreType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AbilityScoreType::Strength => write!(f, "STR"),
            AbilityScoreType::Dexterity => write!(f, "DEX"),
            AbilityScoreType::Constitution => write!(f, "CON"),
            AbilityScoreType::Intelligence => write!(f, "INT"),
            AbilityScoreType::Wisdom => write!(f, "WIS"),
            AbilityScoreType::Charisma => write!(f, "CHA"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

impl DamageType {
    /// Every damage type, in declaration order. The backing array for
    /// `DamageTypeSet`'s bit assignment — index `i` in this slice is bit
    /// `i` in the mask — and the list any consumer that needs to walk
    /// the whole axis should read rather than re-typing thirteen
    /// variants.
    pub const ALL: [DamageType; 13] = [
        DamageType::Acid,
        DamageType::Bludgeoning,
        DamageType::Cold,
        DamageType::Fire,
        DamageType::Force,
        DamageType::Lightning,
        DamageType::Necrotic,
        DamageType::Piercing,
        DamageType::Poison,
        DamageType::Psychic,
        DamageType::Radiant,
        DamageType::Slashing,
        DamageType::Thunder,
    ];

    /// This type's bit position in a `DamageTypeSet`. Kept as an
    /// exhaustive `match` rather than a scan of `ALL` so the compiler
    /// makes adding a fourteenth damage type a build error here instead
    /// of a silently-aliased bit.
    const fn bit_index(self) -> u16 {
        match self {
            DamageType::Acid => 0,
            DamageType::Bludgeoning => 1,
            DamageType::Cold => 2,
            DamageType::Fire => 3,
            DamageType::Force => 4,
            DamageType::Lightning => 5,
            DamageType::Necrotic => 6,
            DamageType::Piercing => 7,
            DamageType::Poison => 8,
            DamageType::Psychic => 9,
            DamageType::Radiant => 10,
            DamageType::Slashing => 11,
            DamageType::Thunder => 12,
        }
    }
}

/// A `Copy` set of damage types, packed into one `u16`.
///
/// Exists because `CastContext` — the frame every in-flight spell
/// resolution reads its school and slot level off — is `Copy`, and a
/// whole family of 5e features gates on *what damage the spell deals*
/// rather than on its school:
///
///   - Draconic Sorcerer **Elemental Affinity**: "when you cast a spell
///     that deals damage of the type associated with your draconic
///     ancestry, add your Charisma modifier to one damage roll".
///   - Circle of Wildfire Druid **Enhanced Bond**: "when you cast a
///     spell that deals fire damage or restores hit points…".
///
/// A `Vec<DamageType>` on the frame would cost an allocation per action
/// executed (not per spell — `Action::execute` opens a frame for every
/// weapon swing and Move too) and would make the frame non-`Copy`,
/// which `current_cast` relies on. Thirteen types fit in a `u16` with
/// three bits to spare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DamageTypeSet(u16);

impl DamageTypeSet {
    /// The set no damage type belongs to — what a non-spell action, or
    /// a spell that declares no `damage_types()`, puts on the frame.
    /// Every gate reading this set therefore fails closed.
    pub const EMPTY: DamageTypeSet = DamageTypeSet(0);

    pub fn from_types(types: &[DamageType]) -> Self {
        let mut set = DamageTypeSet::EMPTY;
        for &t in types {
            set.insert(t);
        }
        set
    }

    pub fn insert(&mut self, damage_type: DamageType) {
        self.0 |= 1 << damage_type.bit_index();
    }

    pub fn contains(self, damage_type: DamageType) -> bool {
        self.0 & (1 << damage_type.bit_index()) != 0
    }
}

impl fmt::Display for DamageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DamageType::Acid => write!(f, "acid"),
            DamageType::Bludgeoning => write!(f, "bludgeoning"),
            DamageType::Cold => write!(f, "cold"),
            DamageType::Fire => write!(f, "fire"),
            DamageType::Force => write!(f, "force"),
            DamageType::Lightning => write!(f, "lightning"),
            DamageType::Necrotic => write!(f, "necrotic"),
            DamageType::Piercing => write!(f, "piercing"),
            DamageType::Poison => write!(f, "poison"),
            DamageType::Psychic => write!(f, "psychic"),
            DamageType::Radiant => write!(f, "radiant"),
            DamageType::Slashing => write!(f, "slashing"),
            DamageType::Thunder => write!(f, "thunder"),
        }
    }
}

/// 5e damage modifier categories for a creature against a damage type.
/// Resistance halves incoming damage, immunity nullifies it, vulnerability
/// doubles it. A creature can declare any subset across damage types via
/// `CreatureTemplate.damage_modifiers`. Stacking rules (5e):
/// - Immunity wins over everything else.
/// - Resistance and vulnerability of the same type cancel (we follow this
///   by simply not allowing both at once on the same template).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageModifier {
    Resistance,
    Immunity,
    Vulnerability,
}

impl DamageModifier {
    pub fn apply(self, raw: u32) -> u32 {
        match self {
            DamageModifier::Resistance => raw / 2,
            DamageModifier::Immunity => 0,
            DamageModifier::Vulnerability => raw.saturating_mul(2),
        }
    }
}

impl fmt::Display for DamageModifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DamageModifier::Resistance => write!(f, "resistant"),
            DamageModifier::Immunity => write!(f, "immune"),
            DamageModifier::Vulnerability => write!(f, "vulnerable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl Size {
    /// Numeric ordering for size comparison. Tiny=0, Small=1, ..., Gargantuan=5.
    pub fn ordinal(self) -> i32 {
        match self {
            Size::Tiny => 0,
            Size::Small => 1,
            Size::Medium => 2,
            Size::Large => 3,
            Size::Huge => 4,
            Size::Gargantuan => 5,
        }
    }

    /// 5e grapple / shove gate: the target must be no more than one size
    /// category larger than the attacker. E.g. a Medium creature can
    /// grapple up to Large, but not Huge.
    pub fn can_grapple(self, target: Size) -> bool {
        target.ordinal() <= self.ordinal() + 1
    }

    /// Inverse of `ordinal`, clamped to the enum's range at both ends.
    ///
    /// The clamp is the point. 5e's growth and shrink effects all say
    /// "one size category larger / smaller", and a Gargantuan creature
    /// that is told to grow simply stays Gargantuan — the same at the
    /// Tiny end. Callers move along the ladder by arithmetic on
    /// `ordinal` and land here, so no caller has to carry the
    /// saturation.
    pub fn from_ordinal(n: i32) -> Size {
        match n {
            i32::MIN..=0 => Size::Tiny,
            1 => Size::Small,
            2 => Size::Medium,
            3 => Size::Large,
            4 => Size::Huge,
            _ => Size::Gargantuan,
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Size::Tiny => write!(f, "Tiny"),
            Size::Small => write!(f, "Small"),
            Size::Medium => write!(f, "Medium"),
            Size::Large => write!(f, "Large"),
            Size::Huge => write!(f, "Huge"),
            Size::Gargantuan => write!(f, "Gargantuan"),
        }
    }
}

/// The eight 5e schools of magic. Every spell belongs to exactly one;
/// the school is the axis a whole family of subclass features keys off
/// ("when you cast an abjuration spell of 1st level or higher…",
/// "when you roll damage for a wizard evocation spell…").
///
/// Surfaced on the `Action` trait as `school() -> Option<SpellSchool>`,
/// defaulting to `None`. `None` means "not a spell, or a spell whose
/// school has no mechanical surface yet" — weapon attacks, monster
/// attacks, class features, and item actions all sit there, and so do
/// the spell impls whose school nothing currently reads. Every consumer
/// gates on an explicit `Some(school)` match, so an untagged spell
/// fails the gate closed (no ward recharge, no damage bump) rather than
/// firing on the wrong school.
///
/// Six variants have consumers today:
///
///   - **Abjuration** — the Abjuration Wizard's Arcane Ward
///     form/recharge hook.
///   - **Conjuration** — the Conjuration Wizard's Focused Conjuration
///     (unbreakable concentration) and Benign Transposition recharge.
///   - **Divination** — the Divination Wizard's Expert Divination slot
///     refund.
///   - **Enchantment** — the Enchantment Wizard's Split Enchantment
///     doubling.
///   - **Evocation** — the Evocation Wizard's Sculpt Spells / Potent
///     Cantrip / Empowered Evocation trio.
///   - **Necromancy** — the Death Domain Cleric's Reaper, which doubles
///     a single-target necromancy cantrip onto a second creature
///     standing beside the first.
///
/// Illusion and Transmutation are declared but unread: the two wizard
/// traditions that carry those names key off reactions and passives
/// rather than off the school of what they cast, so tagging their
/// spells would add rows nothing consults. A future feature that does
/// read one lands as spell-side `school()` overrides plus one consumer,
/// with no enum churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellSchool {
    Abjuration,
    Conjuration,
    Divination,
    Enchantment,
    Evocation,
    Illusion,
    Necromancy,
    Transmutation,
}

impl fmt::Display for SpellSchool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            SpellSchool::Abjuration => "abjuration",
            SpellSchool::Conjuration => "conjuration",
            SpellSchool::Divination => "divination",
            SpellSchool::Enchantment => "enchantment",
            SpellSchool::Evocation => "evocation",
            SpellSchool::Illusion => "illusion",
            SpellSchool::Necromancy => "necromancy",
            SpellSchool::Transmutation => "transmutation",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreatureType {
    Aberration,
    Beast,
    Celestial,
    Construct,
    Dragon,
    Elemental,
    Fey,
    Fiend,
    Giant,
    Humanoid,
    Monstrosity,
    Ooze,
    Plant,
    Undead,
}

impl fmt::Display for CreatureType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreatureType::Aberration => write!(f, "Aberration"),
            CreatureType::Beast => write!(f, "Beast"),
            CreatureType::Celestial => write!(f, "Celestial"),
            CreatureType::Construct => write!(f, "Construct"),
            CreatureType::Dragon => write!(f, "Dragon"),
            CreatureType::Elemental => write!(f, "Elemental"),
            CreatureType::Fey => write!(f, "Fey"),
            CreatureType::Fiend => write!(f, "Fiend"),
            CreatureType::Giant => write!(f, "Giant"),
            CreatureType::Humanoid => write!(f, "Humanoid"),
            CreatureType::Monstrosity => write!(f, "Monstrosity"),
            CreatureType::Ooze => write!(f, "Ooze"),
            CreatureType::Plant => write!(f, "Plant"),
            CreatureType::Undead => write!(f, "Undead"),
        }
    }
}

impl CreatureType {
    /// True if Protection from Evil and Good affects this creature type.
    /// 5e: aberrations, celestials, elementals, fey, fiends, undead.
    pub fn affected_by_protection(&self) -> bool {
        matches!(
            self,
            CreatureType::Aberration
                | CreatureType::Celestial
                | CreatureType::Elemental
                | CreatureType::Fey
                | CreatureType::Fiend
                | CreatureType::Undead
        )
    }

    /// True if Turn Undead affects this creature type.
    pub fn is_undead(&self) -> bool {
        matches!(self, CreatureType::Undead)
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub enum Language {
    Common,
    CommonSignLanguage,
    Draconic,
    Dwarvish,
    Elvish,
    Giant,
    Gnomish,
    Goblin,
    Halfling,
    Orc,
    Abyssal,
    Celestial,
    DeepSpeech,
    Druidic,
    Infernal,
    Primordial, //Aquan, Auran, Ignan, Terran
    Sylvan,
    ThievesCant,
    Undercommon,
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub enum SpecialSense {
    Blindsight(u32),
    Darkvision(u32),
    Tremorsense(u32),
    Truesight(u32),
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Copy)]
pub struct Coordinate {
    pub x: isize,
    pub y: isize,
}

impl Coordinate {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    /// Chebyshev (chessboard) distance between two single-tile points.
    /// Used for quick range checks where footprint size doesn't matter.
    pub fn chebyshev_to(self, other: Self) -> isize {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

impl Add for Coordinate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Coordinate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
