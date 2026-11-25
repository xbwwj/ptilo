//! Attribute values and insertion-ordered attribute dictionaries.

use crate::symbol::Symbol;
use crate::types::TypeId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Attr {
    Unit,
    Bool(bool),
    Int { ty: TypeId, bits: u128 },
    Float { ty: TypeId, bits: u64 },
    Str(Symbol),
    Symbol(Symbol),
    Type(TypeId),
    Array(Vec<Attr>),
    Dict(AttrDict),
    Opaque { name: Symbol, args: Vec<Attr> },
}

impl Attr {
    pub fn boolean(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn int(ty: TypeId, bits: u128) -> Self {
        Self::Int { ty, bits }
    }

    pub fn float(ty: TypeId, bits: u64) -> Self {
        Self::Float { ty, bits }
    }

    pub fn string(name: Symbol) -> Self {
        Self::Str(name)
    }

    pub fn symbol(name: Symbol) -> Self {
        Self::Symbol(name)
    }

    pub fn ty(ty: TypeId) -> Self {
        Self::Type(ty)
    }
}

/// An insertion-insensitive attribute dictionary.
///
/// Entries are kept sorted by [`Symbol`], which makes equality, hashing and
/// interning of the owning type independent of the order attributes were set
/// in, while iteration stays deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AttrDict(Vec<(Symbol, Attr)>);

impl AttrDict {
    pub fn new() -> Self {
        Self::default()
    }

    fn index_of(&self, key: Symbol) -> std::result::Result<usize, usize> {
        self.0.binary_search_by(|(existing, _)| existing.cmp(&key))
    }

    pub fn get(&self, key: Symbol) -> Option<&Attr> {
        self.index_of(key).ok().map(|index| &self.0[index].1)
    }

    pub fn set(&mut self, key: Symbol, value: Attr) {
        match self.index_of(key) {
            Ok(index) => self.0[index].1 = value,
            Err(index) => self.0.insert(index, (key, value)),
        }
    }

    pub fn remove(&mut self, key: Symbol) -> Option<Attr> {
        self.index_of(key).ok().map(|index| self.0.remove(index).1)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Symbol, &Attr)> + '_ {
        self.0.iter().map(|(k, v)| (*k, v))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
