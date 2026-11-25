//! Algorithm-owned traits.

use std::hash::Hash;

pub trait Cfg {
    type Block: Copy + Eq + Hash;

    fn entry(&self) -> Self::Block;
    fn successors(&self, block: Self::Block) -> Vec<Self::Block>;
    fn predecessors(&self, block: Self::Block) -> Vec<Self::Block>;
}

pub trait Mem2Reg {
    type Func: Copy;
    type Block: Copy + Eq + Hash;
    type Value: Copy;
    type Alloca: Copy;
    type Load: Copy;
    type Store: Copy;

    fn blocks(&self, func: Self::Func) -> Vec<Self::Block>;
    fn predecessors(&self, block: Self::Block) -> Vec<Self::Block>;

    fn allocas(&self, block: Self::Block) -> Vec<Self::Alloca>;
    fn is_promotable(&self, alloca: Self::Alloca) -> bool;

    fn loads(&self, alloca: Self::Alloca) -> Vec<Self::Load>;
    fn stores(&self, alloca: Self::Alloca) -> Vec<Self::Store>;
    fn load_value(&self, load: Self::Load) -> Self::Value;
    fn store_value(&self, store: Self::Store) -> Self::Value;

    fn insert_phi(&mut self, block: Self::Block) -> Self::Value;
    fn replace_value(&mut self, old: Self::Value, new: Self::Value);
    fn erase_load(&mut self, load: Self::Load);
    fn erase_store(&mut self, store: Self::Store);
    fn erase_alloca(&mut self, alloca: Self::Alloca);
}
