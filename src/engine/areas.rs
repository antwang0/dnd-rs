//! 5e's areas of effect, on a square grid — the **Cone** and the
//! **Line** that come out of a creature's body, and the **Burst** that
//! is thrown at a point.
//!
//! The engine has had exactly one area shape since it had any: a Burst,
//! which is a footprint-Chebyshev disc around a point somebody chose.
//! That covers Fireball and everything shaped like it, and it has been
//! standing in for every cone in the game — dragon breath most visibly,
//! where `BreathWeapon`'s own comment conceded it: *"RAW dragon breath:
//! 60 ft cone collapses to a burst-4 / range-6 envelope in this 2.5 ft
//! grid."*
//!
//! That substitution is not a rounding error, it is a different rule.
//! A burst is centred on a point the caster picks at range, so it can be
//! dropped behind a wall of allies, wrapped around a corner, or aimed at
//! a spot the caster is nowhere near. A cone comes out of the caster's
//! own body in one direction, which is the entire tactical content of
//! the shape: to catch three creatures in it they have to be lined up
//! *from where the dragon is standing*, and the dragon's own escort is
//! never in it because the dragon is at the apex. A party that spreads
//! out beats a cone and cannot beat a burst; a party that surrounds the
//! dragon beats a cone from three sides at once.
//!
//! ## The definition, and the arithmetic that implements it
//!
//! SRD 5.2: *"A Cone is an area of effect that extends in straight lines
//! from a point of origin in a direction its creator chooses. A Cone's
//! width at any point along its length is equal to that point's distance
//! from the point of origin. … A Cone's point of origin isn't included
//! in the area of effect."*
//!
//! "Width equals distance" is a statement about an angle. A cone of
//! length *d* is *d* wide at its far end, so its half-width there is
//! *d/2* and its half-angle is `atan(1/2)` — about 26.6°, and a total
//! spread of about 53°. That is narrower than the 45°-per-side wedge
//! most grid implementations draw, and on a 2.5 ft grid the difference
//! is enormous: a dragon's 60-foot breath is 24 tiles long, where the
//! wide reading would make it 122 feet across at the mouth and the
//! right one makes it 60.
//!
//! So the test is the angle, and it is done in exact integer
//! arithmetic with no trigonometry and no floating point. For a tile
//! offset `u` from the apex and an aim vector `v`:
//!
//!   - `u · v > 0` — the tile is in front rather than behind.
//!   - `2·|u × v| ≤ u · v` — the tile is inside the spread, because
//!     `|u × v| / (u · v)` is exactly `tan θ` and the bound is
//!     `tan θ ≤ 1/2`.
//!
//! Both sides scale with `|v|`, so the aim point's *distance* cancels
//! and only its direction matters — which is what lets a caller aim a
//! cone by naming any tile in the direction they want it, at any range,
//! without the shape changing.
//!
//! ## Where this rounds, and which way
//!
//! Two places, both outward, both because a grid has no half-tiles:
//!
//!   - **Length** is capped in Chebyshev tiles, which is how every
//!     other distance in the engine is measured. A diagonal cone is
//!     therefore about 1.4× longer in a straight line than an axial one
//!     of the same declared length. That is not a cone-specific
//!     compromise; it is the same diagonal the whole board is measured
//!     in, and a cone that alone used Euclidean distance would be the
//!     odd one out.
//!   - **Width** admits the tile whose *centre* is on the boundary, and
//!     a symmetric cross-section on a square grid has a middle tile and
//!     so an odd number of tiles. A cone is therefore exactly `d` tiles
//!     across at odd distances and `d + 1` at even ones — never
//!     narrower than RAW, at most one tile wider. Rounding the other
//!     way would make the first two rungs of every cone a single tile
//!     wide, which is a line, not a cone.
//!
//! ## The apex
//!
//! RAW puts the point of origin anywhere on the caster the caster
//! likes. The engine uses the tile of the caster's footprint nearest
//! the aim point, which is the same choice a player would make and the
//! one that matters for a Gargantuan dragon: its breath comes out of
//! the front of it rather than out of the corner tile that happens to
//! anchor its 6×6 box. The caster's own footprint is then excluded
//! wholesale — RAW excludes the origin, and a creature standing inside
//! its own breath is not a rule anybody wants.

use crate::engine::types::{Coordinate, Size};
use crate::engine::util::{feet_from_tiles, footprint_chebyshev, get_tiles_from_size};

/// The tiles of a footprint anchored at `anchor` with side `span`.
fn footprint_tiles(anchor: Coordinate, span: isize) -> impl Iterator<Item = Coordinate> {
    (0..span).flat_map(move |dy| {
        (0..span).map(move |dx| Coordinate::new(anchor.x + dx, anchor.y + dy))
    })
}

/// Chebyshev distance between two tiles — the board's metric, used here
/// for the cone's length cap so a cone reaches exactly as far as every
/// other range in the engine measures.
fn chebyshev(a: Coordinate, b: Coordinate) -> isize {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// Where a cone cast by a creature standing at `anchor` (with a
/// `span`×`span` footprint) and aimed at `aim` comes out of: the tile of
/// that footprint nearest the aim point.
///
/// Ties are broken by scan order — lowest row, then lowest column —
/// which only ever picks between two tiles equidistant from the aim, and
/// only needs to be *stable*, not clever.
///
/// A Medium creature's 2×2 box makes this observable even at the small
/// end: a wizard aiming east breathes from its eastern pair of tiles
/// rather than from its south-west anchor, which is the difference
/// between a cone that starts at the wizard's front and one that starts
/// half a body behind them.
pub fn cone_apex(anchor: Coordinate, span: isize, aim: Coordinate) -> Coordinate {
    footprint_tiles(anchor, span)
        .min_by_key(|t| (chebyshev(*t, aim), t.y, t.x))
        .unwrap_or(anchor)
}

/// True if `tile` lies inside the cone of length `length` (in Chebyshev
/// tiles) with its apex at `apex`, aimed in the direction of `aim`.
///
/// `aim` supplies a *direction*, not an endpoint: only the direction of
/// `aim - apex` is read, so aiming at a tile one step away and at one
/// twenty steps away along the same line describe the same cone. A cone
/// aimed at its own apex has no direction and covers nothing.
///
/// The apex tile itself is never covered, per RAW's "a Cone's point of
/// origin isn't included in the area of effect".
pub fn cone_covers(apex: Coordinate, aim: Coordinate, length: isize, tile: Coordinate) -> bool {
    let vx = aim.x - apex.x;
    let vy = aim.y - apex.y;
    if vx == 0 && vy == 0 {
        return false;
    }
    let ux = tile.x - apex.x;
    let uy = tile.y - apex.y;
    // RAW: the point of origin is out.
    if ux == 0 && uy == 0 {
        return false;
    }
    if chebyshev(apex, tile) > length {
        return false;
    }
    let dot = ux * vx + uy * vy;
    if dot <= 0 {
        return false;
    }
    let cross = (ux * vy - uy * vx).abs();
    // `cross / dot` is `tan θ`; the cone's half-angle is `atan(1/2)`.
    2 * cross <= dot
}

/// Every tile of a cone, in row-major order.
///
/// Built by sweeping the bounding square of the length cap rather than
/// by walking rays, because rays on a grid miss tiles and double-count
/// others, and the membership test above is exact — so the honest
/// implementation is to ask it about every tile that could possibly
/// qualify. The sweep is `(2·length+1)²` tests, which for the longest
/// cone in the bestiary (a dragon's 24) is under 2,500 integer
/// comparisons, once, on the turn somebody breathes.
///
/// Wanted by the callers that need the *area* rather than a hit list:
/// the UI's target preview, and any effect that retypes ground.
pub fn cone_tiles(apex: Coordinate, aim: Coordinate, length: isize) -> Vec<Coordinate> {
    let mut out = Vec::new();
    if length <= 0 {
        return out;
    }
    for y in (apex.y - length)..=(apex.y + length) {
        for x in (apex.x - length)..=(apex.x + length) {
            let tile = Coordinate::new(x, y);
            if cone_covers(apex, aim, length, tile) {
                out.push(tile);
            }
        }
    }
    out
}

/// True if any tile of the footprint anchored at `anchor` with side
/// `span` is inside the cone.
///
/// Any-tile rather than all-tiles or centre-tile, which is the reading
/// every area in the engine already uses: `actors_in_burst` measures the
/// *gap* between footprints, so a Huge creature with one toe in a
/// Fireball is in the Fireball. A cone that asked for the whole body
/// would let a dragon walk half of itself out of a Cone of Cold.
pub fn cone_catches_footprint(
    apex: Coordinate,
    aim: Coordinate,
    length: isize,
    anchor: Coordinate,
    span: isize,
) -> bool {
    footprint_tiles(anchor, span).any(|t| cone_covers(apex, aim, length, t))
}

/// True if `tile` lies inside the line of length `length` and half-width
/// `half_width` (both in tiles) running from `apex` toward `aim`.
///
/// The same integer test as the cone with one term changed: a cone caps
/// the *ratio* of the across-distance to the along-distance, and a line
/// caps the across-distance flat. The comparison is made without
/// dividing, by multiplying through by `|v|`:
///
///   - `u · v > 0` — in front rather than behind.
///   - `(u × v)² ≤ half_width² · |v|²` — within the strip, because
///     `|u × v| / |v|` is exactly the perpendicular distance.
///
/// Length is capped in Chebyshev tiles, exactly as the cone's is, so
/// the two shapes agree with each other and with every other distance
/// on the board. It also makes an axial line and a diagonal one cover
/// the same number of tiles, which is the tidiest evidence that the
/// diagonal is not being charged twice.
///
/// The apex is excluded, per RAW's "a Line's point of origin isn't
/// included in the Line's area of effect."
pub fn line_covers(
    apex: Coordinate,
    aim: Coordinate,
    length: isize,
    half_width: isize,
    tile: Coordinate,
) -> bool {
    let vx = aim.x - apex.x;
    let vy = aim.y - apex.y;
    let v_sq = vx * vx + vy * vy;
    if v_sq == 0 {
        return false;
    }
    let ux = tile.x - apex.x;
    let uy = tile.y - apex.y;
    if ux == 0 && uy == 0 {
        return false;
    }
    if chebyshev(apex, tile) > length {
        return false;
    }
    let dot = ux * vx + uy * vy;
    if dot <= 0 {
        return false;
    }
    let cross = ux * vy - uy * vx;
    cross * cross <= half_width * half_width * v_sq
}

/// Every tile of a line, in row-major order. The sweep and its bounding
/// box work the way `cone_tiles`' do; see there.
pub fn line_tiles(
    apex: Coordinate,
    aim: Coordinate,
    length: isize,
    half_width: isize,
) -> Vec<Coordinate> {
    let mut out = Vec::new();
    if length <= 0 {
        return out;
    }
    // A line can reach `length` along its axis and `half_width` across
    // it, so the bounding square has to hold both.
    let reach = length + half_width;
    for y in (apex.y - reach)..=(apex.y + reach) {
        for x in (apex.x - reach)..=(apex.x + reach) {
            let tile = Coordinate::new(x, y);
            if line_covers(apex, aim, length, half_width, tile) {
                out.push(tile);
            }
        }
    }
    out
}

/// One area of effect, in the shapes 5e actually prints.
///
/// The engine had one of these three and called it `TargetingSchema::
/// Burst`. That was enough while every area in the bestiary was
/// rounded to a sphere, and it is the reason a dragon's sixty-foot
/// breath was a ten-foot ball thrown fifteen feet: the shape was not
/// approximated, it was substituted.
///
/// Every shape is aimed the same way — the caster and one point — which
/// is what lets one enum serve the target picker, the AI's placement
/// search, and the resolution chokepoints without any of them caring
/// which variant they are holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaShape {
    /// A footprint-Chebyshev disc centred on the aim point, which is
    /// where 5e's Sphere, Cube, Cylinder and Emanation all land on a
    /// grid this fine. The caster's own position is not read at all: a
    /// Fireball is wherever it was thrown.
    Burst { radius: isize },
    /// 5e's Cone, thrown from the caster's own body toward the aim
    /// point. See the module docstring.
    Cone { length: isize },
    /// 5e's Line — "a 90-foot-long, 5-foot-wide Line" — thrown from the
    /// caster's body toward the aim point.
    ///
    /// `half_width` is in tiles, so RAW's 5-foot line is 1 and its
    /// 10-foot line is 2. A strip symmetric about an axis through a
    /// tile's centre always covers an odd number of tiles, so those
    /// come out 3 and 5 tiles across — one tile wider than RAW's 2 and
    /// 4, the same outward rounding the cone's width takes and for the
    /// same reason. Rounding the other way would make a 5-foot line one
    /// tile wide, which is 2.5 feet.
    Line { length: isize, half_width: isize },
}

impl AreaShape {
    /// Does this area, thrown by a creature of `caster_size` standing at
    /// `caster_anchor` and aimed at `aim`, catch the footprint anchored
    /// at `anchor` with side `span`?
    ///
    /// The caster's position and size are ignored by `Burst`, which is
    /// the whole difference between a thrown area and a projected one.
    pub fn catches_footprint(
        self,
        caster_anchor: Coordinate,
        caster_size: Size,
        aim: Coordinate,
        anchor: Coordinate,
        span: usize,
    ) -> bool {
        match self {
            AreaShape::Burst { radius } => {
                footprint_chebyshev(anchor, span, aim, 1) <= radius
            }
            AreaShape::Cone { length } => {
                let caster_span = get_tiles_from_size(caster_size) as isize;
                let apex = cone_apex(caster_anchor, caster_span, aim);
                cone_catches_footprint(apex, aim, length, anchor, span as isize)
            }
            AreaShape::Line { length, half_width } => {
                let caster_span = get_tiles_from_size(caster_size) as isize;
                let apex = cone_apex(caster_anchor, caster_span, aim);
                footprint_tiles(anchor, span as isize)
                    .any(|t| line_covers(apex, aim, length, half_width, t))
            }
        }
    }

    /// The point RAW measures this area's cover from — a burst's centre,
    /// or the apex a projected area comes out of.
    ///
    /// Wanted by `resolve_burst_targets`, which reduces a target's DC by
    /// whatever is standing between it and the area's origin. Getting
    /// this wrong for a cone would measure cover from a tile at the far
    /// end of the breath rather than from the dragon's mouth.
    pub fn origin(
        self,
        caster_anchor: Coordinate,
        caster_size: Size,
        aim: Coordinate,
    ) -> Coordinate {
        match self {
            AreaShape::Burst { .. } => aim,
            AreaShape::Cone { .. } | AreaShape::Line { .. } => {
                cone_apex(caster_anchor, get_tiles_from_size(caster_size) as isize, aim)
            }
        }
    }

    /// How far from the caster the aim point may be.
    ///
    /// For a projected area this is its own length, because a cone or a
    /// line is aimed by naming a tile inside it. For a burst it is not
    /// answerable here at all — a Fireball's 150-foot range has nothing
    /// to do with its 20-foot radius — so the burst arm returns `None`
    /// and its action keeps declaring its own `reach_tiles`.
    pub fn aim_reach(self) -> Option<isize> {
        match self {
            AreaShape::Burst { .. } => None,
            AreaShape::Cone { length } | AreaShape::Line { length, .. } => Some(length),
        }
    }

    /// The shape as a player reads it on a stat block — "60 ft cone",
    /// "90 ft line", "burst radius 4" — for the target prompt and the
    /// action log.
    pub fn label(self) -> String {
        match self {
            AreaShape::Burst { radius } => format!("burst radius {radius}"),
            AreaShape::Cone { length } => {
                format!("{} ft cone", feet_from_tiles(length.max(0) as u32))
            }
            AreaShape::Line { length, .. } => {
                format!("{} ft line", feet_from_tiles(length.max(0) as u32))
            }
        }
    }

    /// Every tile this area covers, for the callers that want the
    /// ground rather than the hit list. A burst's disc is generated
    /// here too, so the UI and any terrain effect can ask one question
    /// of one type.
    ///
    /// The caster's own footprint is removed from the projected shapes,
    /// which is the second half of RAW's origin clause: a Gargantuan
    /// creature's apex is one tile of a sixty-four-tile box, and
    /// without this the other sixty-three would be squarely inside the
    /// cone coming out of them.
    ///
    /// **The contract is that a creature standing on a tile this
    /// returns is a creature `catches_footprint` catches.** That is
    /// what makes the list safe to draw on a map, and it is not free
    /// for the burst arm: a burst's radius is a footprint *gap*, so a
    /// creature whose nearest tile is `radius + 1` away has a gap of
    /// `radius` and is inside the blast. The disc drawn here is one
    /// tile wider than the declared radius for exactly that reason,
    /// and a preview that took the radius literally would have shaded
    /// a ring smaller than the spell.
    pub fn tiles(
        self,
        caster_anchor: Coordinate,
        caster_size: Size,
        aim: Coordinate,
    ) -> Vec<Coordinate> {
        let span = get_tiles_from_size(caster_size) as isize;
        let apex = cone_apex(caster_anchor, span, aim);
        let mut tiles = match self {
            AreaShape::Burst { radius } => {
                let reach = radius + 1;
                let mut out = Vec::new();
                for y in (aim.y - reach)..=(aim.y + reach) {
                    for x in (aim.x - reach)..=(aim.x + reach) {
                        out.push(Coordinate::new(x, y));
                    }
                }
                return out;
            }
            AreaShape::Cone { length } => cone_tiles(apex, aim, length),
            AreaShape::Line { length, half_width } => {
                line_tiles(apex, aim, length, half_width)
            }
        };
        tiles.retain(|t| {
            !(t.x >= caster_anchor.x
                && t.x < caster_anchor.x + span
                && t.y >= caster_anchor.y
                && t.y < caster_anchor.y + span)
        });
        tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::Size;

    fn c(x: isize, y: isize) -> Coordinate {
        Coordinate::new(x, y)
    }

    /// The rule the whole module exists to implement: a cone is as wide
    /// as it is long. An axial cone's cross-section at distance `d` is
    /// the odd number nearest `d` — `d` itself where `d` is odd, and one
    /// tile wider where it is even, because a symmetric cross-section on
    /// a square grid has to have a middle tile and therefore an odd
    /// count. It cannot be `d` for both parities and it is never
    /// narrower than `d`.
    #[test]
    fn a_cones_width_tracks_its_length() {
        let apex = c(0, 0);
        let aim = c(1, 0);
        for d in 1..=10isize {
            let across: Vec<isize> = (-12..=12)
                .filter(|dy| cone_covers(apex, aim, 24, c(d, *dy)))
                .collect();
            let expected = 2 * (d / 2) + 1;
            assert_eq!(
                across.len() as isize,
                expected,
                "at distance {d} the cone should be {expected} tiles across, got {across:?}"
            );
            assert!(
                across.len() as isize >= d,
                "and never narrower than RAW's {d}"
            );
            // Symmetric about the axis, which is what makes it a cone
            // rather than a wedge that leans.
            assert_eq!(across.first().map(|v| -v), across.last().copied());
        }
    }

    /// The engine's grid is 2.5 ft, so a 60-foot breath is 24 tiles
    /// long and should be about 60 feet across at the far end — 25
    /// tiles. The 45°-per-side reading every naive grid cone uses would
    /// make it 49, which is a hundred and twenty feet of dragon breath.
    #[test]
    fn a_sixty_foot_breath_is_sixty_feet_wide_at_the_mouth() {
        let width = (-40..=40)
            .filter(|dy| cone_covers(c(0, 0), c(1, 0), 24, c(24, *dy)))
            .count();
        assert_eq!(width, 25);
    }

    /// Only the direction of the aim vector is read, so naming a tile
    /// one step out and one twenty steps out along the same line
    /// describes the same cone. This is what lets a caller aim by
    /// clicking any tile rather than by computing a unit vector.
    #[test]
    fn the_aim_point_supplies_a_direction_and_nothing_else() {
        let apex = c(5, 5);
        for tile in cone_tiles(apex, c(6, 5), 8) {
            assert!(cone_covers(apex, c(25, 5), 8, tile), "{tile:?}");
        }
        assert_eq!(
            cone_tiles(apex, c(6, 5), 8).len(),
            cone_tiles(apex, c(25, 5), 8).len()
        );
    }

    /// Two clauses of RAW that are easy to lose: the origin is not in
    /// the area, and neither is anything behind it.
    #[test]
    fn a_cone_covers_neither_its_own_apex_nor_anything_behind_it() {
        let apex = c(10, 10);
        let aim = c(11, 10);
        assert!(!cone_covers(apex, aim, 10, apex));
        for d in 1..=10isize {
            assert!(
                !cone_covers(apex, aim, 10, c(10 - d, 10)),
                "the tile {d} behind the apex is not in a cone pointing away from it"
            );
        }
    }

    /// A diagonal cone is the same cone rotated, which the integer
    /// cross/dot test gets for free and a hand-written eight-direction
    /// mask would have had to spell out.
    ///
    /// It is not the *same size*, and the reason is the board's metric
    /// rather than the cone's arithmetic: a Chebyshev length of 12
    /// reaches 12 tiles east and 12 tiles north-east, and the
    /// north-east one is √2 further away in a straight line. So a
    /// diagonal cone of a given declared length is genuinely bigger,
    /// by about the 41% that diagonal has always been worth on this
    /// board — every move, every reach and every burst in the engine
    /// pays the same exchange rate.
    #[test]
    fn a_diagonal_cone_is_the_same_cone_rotated() {
        let apex = c(0, 0);
        let axial = cone_tiles(apex, c(1, 0), 12).len() as f32;
        let diagonal = cone_tiles(apex, c(1, 1), 12).len() as f32;
        // Area goes as the square of the reach, so the expected ratio
        // is the diagonal's √2 stretch along the axis — the width is
        // capped by the angle either way, so it is one factor of √2
        // rather than two.
        let ratio = diagonal / axial;
        assert!(
            (1.2..=1.6).contains(&ratio),
            "axial {axial} vs diagonal {diagonal} (ratio {ratio})"
        );
    }

    /// An arbitrary aim direction — neither axial nor diagonal — still
    /// produces a cone of the right spread, which is the payoff for
    /// testing the angle rather than snapping to eight directions.
    #[test]
    fn an_oblique_cone_keeps_its_spread() {
        // Aim at a 2:1 slope. The far cross-section, measured across
        // the axis, should still be about as wide as the cone is long.
        let apex = c(0, 0);
        let aim = c(10, 5);
        let tiles = cone_tiles(apex, aim, 10);
        assert!(!tiles.is_empty());
        // Every covered tile is inside the declared length and in front.
        for t in &tiles {
            assert!(chebyshev(apex, *t) <= 10);
            assert!(t.x * 10 + t.y * 5 > 0);
        }
        // The tile straight along the axis is in; one well off it is not.
        assert!(cone_covers(apex, aim, 10, c(4, 2)));
        assert!(!cone_covers(apex, aim, 10, c(2, 8)));
    }

    /// The apex is the caster's near edge, not its anchor corner. For a
    /// Huge creature that is a three-tile difference, and it is the
    /// difference between breathing out of your mouth and out of your
    /// tail.
    #[test]
    fn a_big_creature_breathes_out_of_the_side_it_is_facing() {
        // A Huge creature is 6×6, anchored at (10,10) so it spans
        // x,y ∈ 10..=15.
        let anchor = c(10, 10);
        let span = 6;
        assert_eq!(cone_apex(anchor, span, c(40, 12)).x, 15, "aiming east");
        assert_eq!(cone_apex(anchor, span, c(-40, 12)).x, 10, "aiming west");
        assert_eq!(cone_apex(anchor, span, c(12, 40)).y, 15, "aiming north");
        assert_eq!(cone_apex(anchor, span, c(12, -40)).y, 10, "aiming south");
    }

    /// RAW's origin clause, applied to a body rather than a point: none
    /// of the caster's own tiles are in its own cone or its own line.
    #[test]
    fn a_caster_is_never_inside_its_own_projected_area() {
        let anchor = c(10, 10);
        for shape in [
            AreaShape::Cone { length: 24 },
            AreaShape::Line {
                length: 36,
                half_width: 1,
            },
        ] {
            let tiles = shape.tiles(anchor, Size::Huge, c(40, 12));
            assert!(!tiles.is_empty(), "{shape:?}");
            for t in &tiles {
                assert!(
                    !(t.x >= 10 && t.x < 16 && t.y >= 10 && t.y < 16),
                    "{t:?} is inside the dragon ({shape:?})"
                );
            }
        }
    }

    /// A 5-foot-wide line is a strip three tiles across — RAW's two
    /// rounded outward to the odd count a symmetric strip has to have —
    /// and it stops at its declared length rather than running off the
    /// board.
    #[test]
    fn a_line_is_a_strip_of_its_declared_width_and_stops_at_its_length() {
        let apex = c(0, 0);
        let aim = c(1, 0);
        for d in 1..=12isize {
            let across: Vec<isize> = (-6..=6)
                .filter(|dy| line_covers(apex, aim, 12, 1, c(d, *dy)))
                .collect();
            assert_eq!(across, vec![-1, 0, 1], "at distance {d}");
        }
        assert!(!line_covers(apex, aim, 12, 1, c(13, 0)), "past the end");
        assert!(!line_covers(apex, aim, 12, 1, c(-1, 0)), "behind the apex");
        assert!(!line_covers(apex, aim, 12, 1, apex), "the origin itself");
    }

    /// A line does not widen with distance — that is the whole of what
    /// separates it from a cone, and it is why a 90-foot line dragon
    /// wants the party in a row where a 90-foot cone dragon wants them
    /// in a clump.
    #[test]
    fn a_line_does_not_spread_and_a_cone_does() {
        let apex = c(0, 0);
        let aim = c(1, 0);
        let far = c(20, 6);
        assert!(cone_covers(apex, aim, 24, far), "the cone is wide out there");
        assert!(
            !line_covers(apex, aim, 24, 1, far),
            "the line is still one strip"
        );
    }

    /// A diagonal line keeps its width, which the `|v|`-scaled
    /// perpendicular test gets right and a naive "same row" test would
    /// not — and, because the length cap is Chebyshev like everything
    /// else on this board, it covers exactly as many tiles as the axial
    /// one. Equal counts are the tidiest evidence that the diagonal is
    /// charged once rather than twice.
    #[test]
    fn a_diagonal_line_is_as_wide_as_an_axial_one() {
        let apex = c(0, 0);
        let axial = line_tiles(apex, c(1, 0), 12, 1).len();
        let diagonal = line_tiles(apex, c(1, 1), 12, 1).len();
        assert_eq!(axial, diagonal, "axial {axial} diagonal {diagonal}");
        // And the strip really is three tiles across at both angles,
        // rather than the counts matching for some other reason.
        assert_eq!(axial, 36);
    }

    /// A burst is the shape that does not read the caster at all —
    /// which is the distinction the enum exists to draw, and the reason
    /// a Fireball can be dropped behind a wall of allies where a breath
    /// cannot.
    #[test]
    fn only_a_burst_ignores_where_the_caster_is_standing() {
        let aim = c(20, 20);
        let victim = c(21, 20);
        let burst = AreaShape::Burst { radius: 4 };
        let cone = AreaShape::Cone { length: 24 };
        for caster in [c(0, 0), c(40, 40), c(19, 19)] {
            assert!(
                burst.catches_footprint(caster, Size::Medium, aim, victim, 2),
                "a burst lands where it was thrown, whoever threw it"
            );
        }
        // The cone catches the same victim only when it is thrown in
        // the victim's direction. From the west, aiming east, it does;
        // from the west, aiming *west*, the victim is behind the apex
        // and takes nothing however close it is standing.
        assert!(cone.catches_footprint(c(0, 20), Size::Medium, aim, victim, 2));
        assert!(!cone.catches_footprint(c(30, 20), Size::Medium, c(40, 20), victim, 2));
    }

    /// Cover against an area is measured from where the area comes
    /// from: the centre of a burst, and the apex of anything projected.
    #[test]
    fn an_areas_origin_is_where_it_comes_from() {
        let caster = c(10, 10);
        let aim = c(30, 11);
        assert_eq!(
            AreaShape::Burst { radius: 4 }.origin(caster, Size::Medium, aim),
            aim
        );
        let apex = AreaShape::Cone { length: 24 }.origin(caster, Size::Medium, aim);
        assert_eq!(apex.x, 11, "the caster's eastern column, aiming east");
    }

    /// A projected area is aimed by naming a tile inside it, so its
    /// reach is its own length. A burst's is not answerable from the
    /// shape — Fireball's range is 150 feet and its radius is 20.
    #[test]
    fn a_projected_area_supplies_its_own_reach_and_a_burst_does_not() {
        assert_eq!(AreaShape::Cone { length: 24 }.aim_reach(), Some(24));
        assert_eq!(
            AreaShape::Line {
                length: 36,
                half_width: 1
            }
            .aim_reach(),
            Some(36)
        );
        assert_eq!(AreaShape::Burst { radius: 8 }.aim_reach(), None);
    }

    /// The label is what a player reads on the prompt, so it is in the
    /// rulebook's units rather than the board's.
    #[test]
    fn an_area_names_itself_in_feet() {
        assert_eq!(AreaShape::Cone { length: 24 }.label(), "60 ft cone");
        assert_eq!(
            AreaShape::Line {
                length: 36,
                half_width: 1
            }
            .label(),
            "90 ft line"
        );
        assert_eq!(AreaShape::Burst { radius: 4 }.label(), "burst radius 4");
    }

    /// One toe in the cone is in the cone — the same reading
    /// `actors_in_burst` gives a Fireball, so a Huge creature cannot
    /// keep five sixths of itself out of a breath and take nothing.
    #[test]
    fn one_tile_of_a_footprint_is_enough_to_be_caught() {
        let apex = c(0, 0);
        let aim = c(1, 0);
        // A 4×4 Large creature anchored so that only its westernmost
        // column is inside a narrow cone.
        assert!(cone_catches_footprint(apex, aim, 6, c(4, 0), 4));
        // And one entirely off the axis is not caught at all.
        assert!(!cone_catches_footprint(apex, aim, 6, c(1, 6), 4));
    }

    /// A cone with no direction is not a cone. Guards the degenerate
    /// call — a caster who aimed at the tile it is standing on.
    #[test]
    fn a_cone_aimed_at_its_own_apex_covers_nothing() {
        assert!(cone_tiles(c(3, 3), c(3, 3), 10).is_empty());
        assert!(cone_tiles(c(3, 3), c(9, 3), 0).is_empty());
    }

    /// `cone_tiles` and `cone_covers` are the same predicate, which is
    /// worth pinning because the sweep's bounding box is derived from
    /// the length cap and an off-by-one there would silently clip the
    /// far corners of every wide cone.
    #[test]
    fn the_tile_list_agrees_with_the_membership_test() {
        for (ax, ay) in [(1, 0), (0, 1), (-1, 0), (1, 1), (3, 1), (-2, 5)] {
            let apex = c(0, 0);
            let aim = c(ax, ay);
            let listed: std::collections::HashSet<(isize, isize)> = cone_tiles(apex, aim, 9)
                .into_iter()
                .map(|t| (t.x, t.y))
                .collect();
            for y in -20..=20isize {
                for x in -20..=20isize {
                    assert_eq!(
                        listed.contains(&(x, y)),
                        cone_covers(apex, aim, 9, c(x, y)),
                        "aim ({ax},{ay}) tile ({x},{y})"
                    );
                }
            }
        }
    }

    /// The contract `tiles` promises the map: standing on a tile it
    /// returns is being in the area.
    ///
    /// Swept over every shape and every tile in a window rather than
    /// spot-checked, because the direction that fails silently is the
    /// one where the preview is *tighter* than the effect — the player
    /// steps out of the shaded region and gets hit anyway, and nothing
    /// on the screen was wrong enough to notice. That is exactly what
    /// the burst arm did before it drew its disc a tile wider than the
    /// declared radius, because a burst's radius is a footprint gap.
    #[test]
    fn standing_on_a_drawn_tile_is_being_in_the_area() {
        let caster = c(20, 20);
        for shape in [
            AreaShape::Burst { radius: 0 },
            AreaShape::Burst { radius: 3 },
            AreaShape::Cone { length: 8 },
            AreaShape::Line {
                length: 10,
                half_width: 1,
            },
            AreaShape::Line {
                length: 10,
                half_width: 2,
            },
        ] {
            for aim in [c(28, 20), c(26, 26), c(20, 12), c(27, 23)] {
                let drawn: std::collections::HashSet<(isize, isize)> = shape
                    .tiles(caster, Size::Medium, aim)
                    .into_iter()
                    .map(|t| (t.x, t.y))
                    .collect();
                for y in 0..40isize {
                    for x in 0..40isize {
                        if !drawn.contains(&(x, y)) {
                            continue;
                        }
                        // A Tiny creature is one tile, so "caught" and
                        // "this tile is in the area" are the same
                        // question for it.
                        assert!(
                            shape.catches_footprint(
                                caster,
                                Size::Medium,
                                aim,
                                c(x, y),
                                1
                            ),
                            "{shape:?} aimed at {aim:?} draws ({x},{y}) but does not catch it"
                        );
                    }
                }
            }
        }
    }

}
