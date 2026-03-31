use std::fmt;

use crate::engine::types::Coordinate;

#[derive(Debug, Clone)]
pub struct RngTryError;

impl fmt::Display for RngTryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exceede max tries for rng")
    }
}

impl std::error::Error for RngTryError {}

#[derive(Debug, Clone)]
pub struct NoLegalPosition;

impl fmt::Display for NoLegalPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not find a legal position")
    }
}

impl std::error::Error for NoLegalPosition {}

#[derive(Debug, Clone)]
pub struct ParseError {
    input: String,
}

impl ParseError {
    pub fn new(s: &str) -> Self {
        Self {
            input: s.to_string(),
        }
    }
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to parse input: {}", self.input())
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub struct NegativeAbsCoord {
    coord: Coordinate,
}

impl NegativeAbsCoord {
    pub fn new(coord: Coordinate) -> Self {
        Self { coord: coord }
    }
    pub fn coord(&self) -> Coordinate {
        self.coord
    }
}

impl fmt::Display for NegativeAbsCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Got absolute coordinate with negative value: {}",
            self.coord
        )
    }
}

impl std::error::Error for NegativeAbsCoord {}
