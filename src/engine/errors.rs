use std::fmt;

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
