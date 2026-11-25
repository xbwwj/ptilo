//! String interning.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(u32);

impl Symbol {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    strings: Vec<String>,
    map: HashMap<String, Symbol>,
}

impl SymbolTable {
    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(id) = self.map.get(name) {
            return *id;
        }
        let id = Symbol(self.strings.len() as u32);
        self.strings.push(name.to_owned());
        self.map.insert(name.to_owned(), id);
        id
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Drop symbols interned after `len`, see `Context::checkpoint`.
    pub(crate) fn truncate(&mut self, len: usize) {
        for name in self.strings.drain(len..) {
            self.map.remove(&name);
        }
    }

    pub fn get(&self, id: Symbol) -> &str {
        &self.strings[id.0 as usize]
    }

    pub fn lookup(&self, name: &str) -> Option<Symbol> {
        self.map.get(name).copied()
    }
}
