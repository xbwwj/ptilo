//! Structural IR types.

use std::collections::HashMap;

use crate::attr::AttrDict;
use crate::symbol::Symbol;

/// A handle to an interned structural type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(u32);

impl TypeId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signedness {
    Signless,
    Signed,
    Unsigned,
}

impl Signedness {
    pub fn as_str(self) -> &'static str {
        match self {
            Signedness::Signless => "signless",
            Signedness::Signed => "signed",
            Signedness::Unsigned => "unsigned",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "signless" => Some(Signedness::Signless),
            "signed" => Some(Signedness::Signed),
            "unsigned" => Some(Signedness::Unsigned),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeArg {
    Type(TypeId),
    Int(u64),
    Str(Symbol),
    Field(Symbol, TypeId),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeNode {
    pub name: Symbol,
    pub args: Vec<TypeArg>,
    pub attrs: AttrDict,
}

#[derive(Debug, Default)]
pub struct TypeInterner {
    types: Vec<TypeNode>,
    map: HashMap<TypeNode, TypeId>,
}

impl TypeInterner {
    pub fn intern(&mut self, data: TypeNode) -> TypeId {
        if let Some(id) = self.map.get(&data) {
            return *id;
        }
        let id = TypeId(self.types.len() as u32);
        self.types.push(data.clone());
        self.map.insert(data, id);
        id
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Drop types interned after `len`, see `Context::checkpoint`.
    pub(crate) fn truncate(&mut self, len: usize) {
        for node in self.types.drain(len..) {
            self.map.remove(&node);
        }
    }

    pub fn get(&self, id: TypeId) -> &TypeNode {
        &self.types[id.0 as usize]
    }

    pub fn try_get(&self, id: TypeId) -> Option<&TypeNode> {
        self.types.get(id.0 as usize)
    }
}
