//! A small structural IR core.

pub mod algorithms;
pub mod arena;
pub mod attr;
pub mod ir;
pub mod spec;
pub mod symbol;
pub mod text;
pub mod types;

pub use algorithms::{Cfg, Mem2Reg};
pub use arena::{Arena, Id};
pub use attr::{Attr, AttrDict};
pub use ir::{
    Block, BlockId, Checkpoint, Context, OpBuilder, OpId, Operation, Region, RegionId, Value,
};
pub use spec::{
    AttrConstraint, AttrSpec, OpSpec, OperandSpec, RegionSpec, ResultSpec, attr, opd, opd_var,
    region, region_optional, res,
};
pub use symbol::Symbol;
pub use text::print;
pub use types::{Signedness, TypeArg, TypeId, TypeNode};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

impl Context {
    /// Parse MLIR generic-form text into a fresh IR.
    pub fn parse(text: &str) -> Result<Self> {
        text::parse(text)
    }

    /// Append MLIR generic-form text to this IR.
    pub fn parse_into(&mut self, text: &str) -> Result<()> {
        text::parse_into(self, text)
    }
}
