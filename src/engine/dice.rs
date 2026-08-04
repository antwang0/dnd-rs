use std::fmt;
use std::str::FromStr;

use fastrand::Rng;

/// `count` rolls of a fair `1..=faces` die.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dice {
    pub count: u32,
    pub faces: u32,
}

impl Dice {
    pub const fn new(count: u32, faces: u32) -> Self {
        Self { count, faces }
    }

    /// The maximum possible sum of this dice pool: every die comes up on
    /// its top face. Used at "maximize the roll" chokepoints — the Grave
    /// Cleric's Circle of Mortality substitution (max dice on a 0-HP
    /// healing target) is the current caller; any future "no roll,
    /// take max" surface (Empowered Evocation crits, a hypothetical
    /// Assassinate max-damage variant, etc.) lands on the same accessor.
    /// Saturating multiplication keeps the accessor safe against a
    /// pathological `u32` overflow (`count * faces >= 2^32`); real dice
    /// pools clear that bound by orders of magnitude but the guard is
    /// cheap.
    pub const fn max_roll(&self) -> u32 {
        self.count.saturating_mul(self.faces)
    }

    /// The mean sum of this dice pool: `count * (faces + 1) / 2`.
    ///
    /// For estimating, not for rolling — nothing that resolves an effect
    /// may call this, or the effect stops being random. The caller is
    /// the AI's attack picker, which needs to compare two swings it has
    /// not made yet, and comparing means a number rather than a die.
    /// Returned as `f32` because the half is load-bearing: a d6 averages
    /// 3.5 and a d7 does not exist, so rounding here would make 2d6 and
    /// 1d12 look like the same weapon.
    pub fn average_roll(&self) -> f32 {
        self.count as f32 * (self.faces as f32 + 1.0) / 2.0
    }
}

impl fmt::Display for Dice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d{}", self.count, self.faces)
    }
}

/// 5e advantage / disadvantage. Applied to attack rolls and saving throws
/// (not damage). `combine` cancels opposite sources and idempotently
/// folds same-direction sources — matching 5e's "you don't stack
/// advantage; one of each cancels."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollMode {
    Normal,
    Advantage,
    Disadvantage,
}

impl RollMode {
    pub fn combine(self, other: RollMode) -> RollMode {
        use RollMode::*;
        match (self, other) {
            (Normal, x) | (x, Normal) => x,
            (Advantage, Advantage) => Advantage,
            (Disadvantage, Disadvantage) => Disadvantage,
            (Advantage, Disadvantage) | (Disadvantage, Advantage) => Normal,
        }
    }

    /// Short suffix for log messages — empty when Normal so we don't
    /// pollute every roll line.
    pub fn log_suffix(self) -> &'static str {
        match self {
            RollMode::Normal => "",
            RollMode::Advantage => " (adv)",
            RollMode::Disadvantage => " (dis)",
        }
    }
}

/// Anything that can produce a sum-of-dice roll. The trait stays minimal so
/// tests can swap in a deterministic stub (see tests below).
pub trait Roller {
    /// Sum of `dice.count` independent rolls of a 1..=`dice.faces` die.
    /// `count == 0` or `faces == 0` returns 0 (treat as a no-op).
    fn roll(&mut self, dice: &Dice) -> u32;

    /// Convenience for the canonical 5e check / attack / save / ability
    /// roll. Equivalent to `self.roll(&Dice::new(1, 20))`, but lets call
    /// sites read as `roller.roll_d20()` instead of carrying the
    /// `Dice::new(1, 20)` literal — the most common single-die shape in
    /// the engine. Returns 1..=20.
    fn roll_d20(&mut self) -> u32 {
        self.roll(&Dice::new(1, 20))
    }
}

/// `fastrand`-backed roller. Use [`FastRandRoller::with_seed`] for repeatable
/// runs (encounter seeding) or [`Default::default`] for non-deterministic.
pub struct FastRandRoller {
    rng: Rng,
}

impl FastRandRoller {
    pub fn with_seed(seed: u64) -> Self {
        Self {
            rng: Rng::with_seed(seed),
        }
    }
}

impl Default for FastRandRoller {
    fn default() -> Self {
        Self { rng: Rng::new() }
    }
}

impl Roller for FastRandRoller {
    fn roll(&mut self, dice: &Dice) -> u32 {
        if dice.count == 0 || dice.faces == 0 {
            return 0;
        }
        let mut sum: u32 = 0;
        for _ in 0..dice.count {
            // 1..=faces is inclusive of faces; matches D&D dice ranges.
            sum = sum.saturating_add(self.rng.u32(1..=dice.faces));
        }
        sum
    }
}

/// A dice expression of the form `[N]dM[±K]`, or a bare integer constant.
/// Examples that parse: `"2d8+6"`, `"1d20"`, `"d6"` (count=1), `"-3"`, `"10"`.
/// Whitespace inside the expression is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiceExpr {
    pub dice: Option<Dice>,
    pub constant: i32,
}

impl DiceExpr {
    pub const fn constant(value: i32) -> Self {
        Self {
            dice: None,
            constant: value,
        }
    }

    pub fn eval(&self, roller: &mut impl Roller) -> i32 {
        let dice_total = self.dice.map_or(0, |d| roller.roll(&d) as i32);
        dice_total + self.constant
    }

    /// What `eval` returns on average, without a roller.
    ///
    /// The `DiceExpr` half of `Dice::average_roll` — same formula, plus
    /// the constant term. Exists for the questions that are about the
    /// *expression* rather than about one roll of it: comparing two
    /// creature templates' hit points, sanity-checking that a stat block
    /// lands where its CR implies, or reporting an expected value
    /// without perturbing the encounter's RNG stream. Rolling a throwaway
    /// sample to answer those is both noisier and, on a seeded encounter,
    /// actively wrong — every draw from the shared roller shifts every
    /// subsequent roll in the fight.
    pub fn average_roll(&self) -> f32 {
        self.dice.map_or(0.0, |d| d.average_roll()) + self.constant as f32
    }
}

impl fmt::Display for DiceExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.dice, self.constant) {
            (Some(d), 0) => write!(f, "{}", d),
            (Some(d), k) if k > 0 => write!(f, "{}+{}", d, k),
            (Some(d), k) => write!(f, "{}{}", d, k), // negative k already prints sign
            (None, k) => write!(f, "{}", k),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiceError {
    input: String,
    reason: &'static str,
}

impl fmt::Display for ParseDiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid dice expression {:?}: {}", self.input, self.reason)
    }
}

impl std::error::Error for ParseDiceError {}

impl FromStr for DiceExpr {
    type Err = ParseDiceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        let err = |reason: &'static str| ParseDiceError {
            input: s.to_string(),
            reason,
        };
        if cleaned.is_empty() {
            return Err(err("empty"));
        }

        // Locate the dice marker, if any.
        let d_pos = cleaned.bytes().position(|b| b == b'd' || b == b'D');
        let Some(d_pos) = d_pos else {
            // No dice — must be a plain integer.
            let constant: i32 = cleaned.parse().map_err(|_| err("expected integer or NdM"))?;
            return Ok(DiceExpr {
                dice: None,
                constant,
            });
        };

        let count_str = &cleaned[..d_pos];
        let count: u32 = if count_str.is_empty() {
            1
        } else {
            count_str.parse().map_err(|_| err("invalid dice count"))?
        };

        let after_d = &cleaned[d_pos + 1..];
        // Modifier sign cannot appear at index 0 (that would be the faces
        // having no digits); look starting at 1.
        let mod_pos = after_d
            .bytes()
            .enumerate()
            .skip(1)
            .find(|(_, b)| *b == b'+' || *b == b'-')
            .map(|(i, _)| i);
        let (faces_str, modifier_str) = match mod_pos {
            Some(p) => (&after_d[..p], &after_d[p..]),
            None => (after_d, ""),
        };
        if faces_str.is_empty() {
            return Err(err("missing dice faces"));
        }
        let faces: u32 = faces_str.parse().map_err(|_| err("invalid dice faces"))?;
        let constant: i32 = if modifier_str.is_empty() {
            0
        } else {
            modifier_str.parse().map_err(|_| err("invalid modifier"))?
        };

        Ok(DiceExpr {
            dice: Some(Dice::new(count, faces)),
            constant,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the floor of the i'th element from the dice cycle, used to
    /// produce predictable rolls in tests.
    struct ScriptedRoller {
        next: Vec<u32>,
        idx: usize,
    }

    impl ScriptedRoller {
        fn new(values: &[u32]) -> Self {
            Self {
                next: values.to_vec(),
                idx: 0,
            }
        }
    }

    impl Roller for ScriptedRoller {
        fn roll(&mut self, dice: &Dice) -> u32 {
            // Pop one value per individual die, summing across `count`.
            let mut sum = 0;
            for _ in 0..dice.count {
                let v = self.next[self.idx % self.next.len()];
                self.idx += 1;
                assert!(v >= 1 && v <= dice.faces, "scripted value out of range");
                sum += v;
            }
            sum
        }
    }

    #[test]
    fn parse_simple_dice() {
        let e: DiceExpr = "2d8+6".parse().unwrap();
        assert_eq!(e.dice, Some(Dice::new(2, 8)));
        assert_eq!(e.constant, 6);
    }

    #[test]
    fn parse_implicit_count() {
        let e: DiceExpr = "d20".parse().unwrap();
        assert_eq!(e.dice, Some(Dice::new(1, 20)));
        assert_eq!(e.constant, 0);
    }

    #[test]
    fn parse_negative_modifier() {
        let e: DiceExpr = "1d6-1".parse().unwrap();
        assert_eq!(e.dice, Some(Dice::new(1, 6)));
        assert_eq!(e.constant, -1);
    }

    #[test]
    fn parse_constant_only() {
        let e: DiceExpr = "12".parse().unwrap();
        assert_eq!(e.dice, None);
        assert_eq!(e.constant, 12);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!("hello".parse::<DiceExpr>().is_err());
        assert!("d".parse::<DiceExpr>().is_err());
        assert!("2d".parse::<DiceExpr>().is_err());
        assert!("".parse::<DiceExpr>().is_err());
    }

    #[test]
    fn eval_uses_each_die() {
        let mut r = ScriptedRoller::new(&[3, 5]);
        let e: DiceExpr = "2d8+1".parse().unwrap();
        // 3 + 5 + 1 = 9
        assert_eq!(e.eval(&mut r), 9);
    }

    #[test]
    fn roll_mode_combine_logic() {
        use RollMode::*;
        assert_eq!(Normal.combine(Advantage), Advantage);
        assert_eq!(Normal.combine(Disadvantage), Disadvantage);
        assert_eq!(Advantage.combine(Advantage), Advantage);
        assert_eq!(Disadvantage.combine(Disadvantage), Disadvantage);
        assert_eq!(Advantage.combine(Disadvantage), Normal);
        assert_eq!(Disadvantage.combine(Advantage), Normal);
        assert_eq!(Normal.combine(Normal), Normal);
    }

    #[test]
    fn fastrand_roller_is_seedable() {
        let mut a = FastRandRoller::with_seed(42);
        let mut b = FastRandRoller::with_seed(42);
        let dice = Dice::new(5, 20);
        assert_eq!(a.roll(&dice), b.roll(&dice));
    }

    #[test]
    fn roll_d20_delegates_to_d20() {
        // The convenience method is byte-identical to the explicit
        // `Dice::new(1, 20)` form; pin the equivalence so a future
        // refactor that nudges one without the other doesn't
        // silently change the d20 cadence.
        let mut a = FastRandRoller::with_seed(7);
        let mut b = FastRandRoller::with_seed(7);
        assert_eq!(a.roll_d20(), b.roll(&Dice::new(1, 20)));
    }

    #[test]
    fn roll_d20_is_in_range() {
        let mut r = FastRandRoller::with_seed(1);
        for _ in 0..100 {
            let n = r.roll_d20();
            assert!((1..=20).contains(&n), "d20 out of range: {}", n);
        }
    }

    #[test]
    fn scripted_roller_supports_d20_shortcut() {
        // The shortcut is on the trait, not the impl — verify a
        // deterministic stub picks it up via the default body.
        let mut r = ScriptedRoller::new(&[7]);
        assert_eq!(r.roll_d20(), 7);
    }

    #[test]
    fn roll_zero_count_is_zero() {
        let mut r = FastRandRoller::with_seed(1);
        assert_eq!(r.roll(&Dice::new(0, 20)), 0);
        assert_eq!(r.roll(&Dice::new(3, 0)), 0);
    }

    #[test]
    fn max_roll_is_count_times_faces() {
        // Canonical shapes: 1d8 → 8, 3d8 → 24, 1d4 → 4, 10d6 → 60.
        // Zero-count or zero-faces degenerate to 0 the same way the
        // sibling `roll` accessor treats them, so a caller substituting
        // `dice.max_roll()` for a live roll doesn't accidentally overshoot
        // on a degenerate pool.
        assert_eq!(Dice::new(1, 8).max_roll(), 8);
        assert_eq!(Dice::new(3, 8).max_roll(), 24);
        assert_eq!(Dice::new(1, 4).max_roll(), 4);
        assert_eq!(Dice::new(10, 6).max_roll(), 60);
        assert_eq!(Dice::new(0, 20).max_roll(), 0);
        assert_eq!(Dice::new(3, 0).max_roll(), 0);
    }

    #[test]
    fn max_roll_dominates_random_roll() {
        // A random roll of the pool must never exceed the max-roll
        // ceiling. Loop enough seeds to cover the tail of the roller.
        let dice = Dice::new(5, 20);
        let ceiling = dice.max_roll();
        for seed in 0..64 {
            let mut r = FastRandRoller::with_seed(seed);
            let rolled = r.roll(&dice);
            assert!(
                rolled <= ceiling,
                "seed {}: rolled {} > max_roll {}",
                seed,
                rolled,
                ceiling
            );
        }
    }

    /// `DiceExpr::average_roll` agrees with the mean of `eval`, on both
    /// legs of the expression: the dice term and the constant.
    ///
    /// The constant leg is the one worth pinning. `Dice::average_roll`
    /// has no constant to forget, so the obvious wrong implementation of
    /// the `DiceExpr` version — delegate and stop — is wrong only for
    /// expressions like `"5d8+8"`, which is exactly the shape every
    /// creature template's hit points are written in.
    #[test]
    fn dice_expr_average_covers_both_legs_of_the_expression() {
        // Bare constant: no dice, so the average is the constant.
        assert_eq!(DiceExpr::constant(7).average_roll(), 7.0);
        // Bare dice: matches the `Dice` half exactly.
        let bare: DiceExpr = "2d6".parse().unwrap();
        assert_eq!(bare.average_roll(), Dice::new(2, 6).average_roll());
        // Both legs: 5d8 averages 22.5, plus 8.
        let both: DiceExpr = "5d8+8".parse().unwrap();
        assert_eq!(both.average_roll(), 30.5);
        // A negative constant subtracts rather than being dropped.
        let negative: DiceExpr = "1d4-2".parse().unwrap();
        assert_eq!(negative.average_roll(), 0.5);
    }
}
