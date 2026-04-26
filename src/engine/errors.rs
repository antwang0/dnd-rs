use std::fmt;

use crate::engine::types::Coordinate;

#[derive(Debug, Clone)]
pub struct RngTryError;

impl fmt::Display for RngTryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exceeded max tries for rng")
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
    msg: String,
    input: Option<String>,
}

impl ParseError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            input: None,
        }
    }

    pub fn with_input(msg: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            input: Some(input.into()),
        }
    }

    pub fn message(&self) -> &str {
        &self.msg
    }

    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.input {
            Some(input) => write!(f, "{}: {:?}", self.msg, input),
            None => write!(f, "{}", self.msg),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub struct NegativeAbsCoord {
    coord: Coordinate,
}

impl NegativeAbsCoord {
    pub fn new(coord: Coordinate) -> Self {
        Self { coord }
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

#[derive(Debug, Clone)]
pub enum ActionError {
    InvalidArgs(String),
    InsufficientResources,
    UnknownActor(usize),
    UnknownAction(String),
    InvalidTarget(String),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidArgs(s) => write!(f, "invalid args: {}", s),
            Self::InsufficientResources => write!(f, "insufficient resources"),
            Self::UnknownActor(id) => write!(f, "unknown actor id {}", id),
            Self::UnknownAction(n) => write!(f, "unknown action {:?}", n),
            Self::InvalidTarget(s) => write!(f, "invalid target: {}", s),
        }
    }
}

impl std::error::Error for ActionError {}
