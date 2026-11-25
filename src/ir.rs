//! Operations, regions, blocks, values, and the owning context.

use crate::arena::{Arena, Id};
use crate::attr::{Attr, AttrDict};
use crate::spec::OpSpec;
use crate::symbol::{Symbol, SymbolTable};
use crate::types::{Signedness, TypeArg, TypeId, TypeInterner, TypeNode};

pub type OpId = Id<Operation>;
pub type BlockId = Id<Block>;
pub type RegionId = Id<Region>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Value {
    OpResult { op: OpId, index: u32 },
    BlockArg { block: BlockId, index: u32 },
}

impl Value {
    pub fn op_result(op: OpId, index: u32) -> Self {
        Self::OpResult { op, index }
    }

    pub fn block_arg(block: BlockId, index: u32) -> Self {
        Self::BlockArg { block, index }
    }
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: Symbol,
    pub operands: Vec<Value>,
    pub results: Vec<TypeId>,
    pub attrs: AttrDict,
    pub regions: Vec<RegionId>,
    pub successors: Vec<BlockId>,
    pub parent: Option<BlockId>,
    pub spec: Option<&'static OpSpec>,
}

impl Operation {
    pub fn new(name: Symbol) -> Self {
        Self {
            name,
            operands: Vec::new(),
            results: Vec::new(),
            attrs: AttrDict::new(),
            regions: Vec::new(),
            successors: Vec::new(),
            parent: None,
            spec: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub blocks: Vec<BlockId>,
    pub parent: Option<OpId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub args: Vec<TypeId>,
    pub ops: Vec<OpId>,
    pub parent: RegionId,
}

#[derive(Debug, Default)]
pub struct Context {
    symbols: SymbolTable,
    types: TypeInterner,
    ops: Arena<Operation>,
    blocks: Arena<Block>,
    regions: Arena<Region>,
    roots: Vec<OpId>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, name: &str) -> Symbol {
        self.symbols.intern(name)
    }

    pub fn symbol_str(&self, symbol: Symbol) -> &str {
        self.symbols.get(symbol)
    }

    pub fn lookup_symbol(&self, name: &str) -> Option<Symbol> {
        self.symbols.lookup(name)
    }

    pub fn op(&self, id: OpId) -> &Operation {
        self.ops.get(id).expect("dangling OpId")
    }

    pub fn op_mut(&mut self, id: OpId) -> &mut Operation {
        self.ops.get_mut(id).expect("dangling OpId")
    }

    pub fn try_op(&self, id: OpId) -> Option<&Operation> {
        self.ops.get(id)
    }

    pub fn block(&self, id: BlockId) -> &Block {
        self.blocks.get(id).expect("dangling BlockId")
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut Block {
        self.blocks.get_mut(id).expect("dangling BlockId")
    }

    pub fn try_block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(id)
    }

    pub fn region(&self, id: RegionId) -> &Region {
        self.regions.get(id).expect("dangling RegionId")
    }

    pub fn region_mut(&mut self, id: RegionId) -> &mut Region {
        self.regions.get_mut(id).expect("dangling RegionId")
    }

    pub fn try_region(&self, id: RegionId) -> Option<&Region> {
        self.regions.get(id)
    }

    /// Every live operation, including ones detached from any block or region.
    pub fn iter_ops(&self) -> impl Iterator<Item = (OpId, &Operation)> + '_ {
        self.ops.iter()
    }

    pub fn iter_blocks(&self) -> impl Iterator<Item = (BlockId, &Block)> + '_ {
        self.blocks.iter()
    }

    pub fn iter_regions(&self) -> impl Iterator<Item = (RegionId, &Region)> + '_ {
        self.regions.iter()
    }

    /// Live operation count, i.e. slots currently in use.
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    pub fn roots(&self) -> &[OpId] {
        &self.roots
    }

    pub fn op_name(&self, op: OpId) -> &str {
        self.symbol_str(self.op(op).name)
    }

    pub fn is_op(&self, op: OpId, name: &str) -> bool {
        self.op_name(op) == name
    }

    pub fn attr(&self, op: OpId, key: &str) -> Option<&Attr> {
        let key = self.lookup_symbol(key)?;
        self.op(op).attrs.get(key)
    }

    pub fn ops_in_block(&self, block: BlockId) -> &[OpId] {
        &self.block(block).ops
    }

    pub fn block_successors(&self, block: BlockId) -> Vec<BlockId> {
        let mut out = Vec::new();
        for op in self.ops_in_block(block) {
            out.extend(self.op(*op).successors.iter().copied());
        }
        out
    }

    pub fn block_predecessors(&self, block: BlockId) -> Vec<BlockId> {
        let region = self.block(block).parent;
        let mut out = Vec::new();
        for candidate in &self.region(region).blocks {
            if self.block_successors(*candidate).contains(&block) {
                out.push(*candidate);
            }
        }
        out
    }

    pub fn uses(&self, value: Value) -> Vec<(OpId, usize)> {
        let mut out = Vec::new();
        for op in self.walk_all_ops() {
            for (index, operand) in self.op(op).operands.iter().enumerate() {
                if *operand == value {
                    out.push((op, index));
                }
            }
        }
        out
    }

    pub fn value_type(&self, value: Value) -> TypeId {
        self.try_value_type(value)
            .unwrap_or_else(|| panic!("dangling value {value:?}"))
    }

    pub fn try_value_type(&self, value: Value) -> Option<TypeId> {
        match value {
            Value::OpResult { op, index } => self.try_op(op)?.results.get(index as usize).copied(),
            Value::BlockArg { block, index } => {
                self.try_block(block)?.args.get(index as usize).copied()
            }
        }
    }

    pub fn type_node(&self, id: TypeId) -> &TypeNode {
        self.types.get(id)
    }

    pub fn try_type_node(&self, id: TypeId) -> Option<&TypeNode> {
        self.types.try_get(id)
    }

    pub fn intern_type(&mut self, data: TypeNode) -> TypeId {
        self.types.intern(data)
    }

    pub fn int_type(&mut self, width: u16, signedness: Signedness) -> TypeId {
        let name = self.intern("int");
        let sign = self.intern(signedness.as_str());
        self.intern_type(TypeNode {
            name,
            args: vec![TypeArg::Int(width as u64), TypeArg::Str(sign)],
            attrs: AttrDict::new(),
        })
    }

    pub fn float_type(&mut self, width: u16) -> TypeId {
        let name = self.intern("float");
        self.intern_type(TypeNode {
            name,
            args: vec![TypeArg::Int(width as u64)],
            attrs: AttrDict::new(),
        })
    }

    pub fn index_type(&mut self) -> TypeId {
        let name = self.intern("index");
        self.intern_type(TypeNode {
            name,
            args: Vec::new(),
            attrs: AttrDict::new(),
        })
    }

    pub fn none_type(&mut self) -> TypeId {
        let name = self.intern("none");
        self.intern_type(TypeNode {
            name,
            args: Vec::new(),
            attrs: AttrDict::new(),
        })
    }

    pub fn func_type(&mut self, inputs: &[TypeId], results: &[TypeId]) -> TypeId {
        let name = self.intern("func");
        let mut args = Vec::with_capacity(1 + inputs.len() + results.len());
        args.push(TypeArg::Int(inputs.len() as u64));
        args.extend(inputs.iter().copied().map(TypeArg::Type));
        args.extend(results.iter().copied().map(TypeArg::Type));
        self.intern_type(TypeNode {
            name,
            args,
            attrs: AttrDict::new(),
        })
    }

    pub fn opaque_type(&mut self, name: &str, args: Vec<TypeArg>) -> TypeId {
        let name = self.intern(name.trim_start_matches('!'));
        self.intern_type(TypeNode {
            name,
            args,
            attrs: AttrDict::new(),
        })
    }

    pub fn int_info(&self, id: TypeId) -> Option<(u16, Signedness)> {
        let data = self.types.try_get(id)?;
        if self.symbol_str(data.name) != "int" {
            return None;
        }
        let width = match data.args.first()? {
            TypeArg::Int(width) => *width as u16,
            _ => return None,
        };
        let signedness = match data.args.get(1)? {
            TypeArg::Str(sign) => Signedness::from_name(self.symbol_str(*sign))?,
            _ => return None,
        };
        Some((width, signedness))
    }

    pub fn float_width(&self, id: TypeId) -> Option<u16> {
        let data = self.types.try_get(id)?;
        if self.symbol_str(data.name) != "float" {
            return None;
        }
        match data.args.first()? {
            TypeArg::Int(width) => Some(*width as u16),
            _ => None,
        }
    }

    pub fn func_parts(&self, id: TypeId) -> Option<(Vec<TypeId>, Vec<TypeId>)> {
        let data = self.types.try_get(id)?;
        if self.symbol_str(data.name) != "func" {
            return None;
        }
        let input_count = match data.args.first()? {
            TypeArg::Int(n) => *n as usize,
            _ => return None,
        };
        let mut inputs = Vec::with_capacity(input_count);
        let mut results = Vec::new();
        for (index, arg) in data.args.iter().skip(1).enumerate() {
            let TypeArg::Type(ty) = arg else {
                return None;
            };
            if index < input_count {
                inputs.push(*ty);
            } else {
                results.push(*ty);
            }
        }
        Some((inputs, results))
    }

    pub fn create_op(&mut self, name: &str) -> OpBuilder<'_> {
        let name = self.intern(name);
        OpBuilder {
            ctx: self,
            spec: None,
            op: Operation::new(name),
        }
    }

    /// Create an operation from a static spec.
    pub fn create_op_spec(&mut self, spec: &'static OpSpec) -> OpBuilder<'_> {
        let name = self.intern(spec.name);
        let mut op = Operation::new(name);
        op.spec = Some(spec);
        OpBuilder {
            ctx: self,
            spec: Some(spec),
            op,
        }
    }

    pub fn op_spec(&self, op: OpId) -> Option<&'static OpSpec> {
        self.op(op).spec
    }

    pub fn new_region(&mut self) -> RegionId {
        self.regions.alloc(Region {
            blocks: Vec::new(),
            parent: None,
        })
    }

    pub fn new_block(&mut self, region: RegionId, args: &[TypeId]) -> BlockId {
        let id = self.blocks.alloc(Block {
            args: args.to_vec(),
            ops: Vec::new(),
            parent: region,
        });
        self.region_mut(region).blocks.push(id);
        id
    }

    /// Append a region to `op`. Panics when the operation's [`OpSpec`] does not
    /// declare another region, mirroring the checks `OpBuilder::finish` does for
    /// operands, results and attributes.
    pub fn add_region(&mut self, op: OpId) -> RegionId {
        if let Some(spec) = self.op(op).spec
            && self.op(op).regions.len() >= spec.regions.len()
        {
            panic!("{} declares {} regions", spec.name, spec.regions.len());
        }
        let region = self.new_region();
        self.region_mut(region).parent = Some(op);
        self.op_mut(op).regions.push(region);
        region
    }

    pub fn alloc_op(&mut self, op: Operation) -> OpId {
        let regions = op.regions.clone();
        let id = self.ops.alloc(op);
        for region in regions {
            self.region_mut(region).parent = Some(id);
        }
        id
    }

    /// Unlink `op` from its block or from the root list, without reclaiming it.
    pub fn detach_op(&mut self, op: OpId) -> Option<BlockId> {
        let parent = self.try_op(op).and_then(|op| op.parent);
        if let Some(block) = parent
            && let Some(data) = self.blocks.get_mut(block)
        {
            data.ops.retain(|id| *id != op);
        }
        if let Some(data) = self.ops.get_mut(op) {
            data.parent = None;
        }
        self.roots.retain(|id| *id != op);
        parent
    }

    /// Attach `op` to `block`. The operation is removed from its previous
    /// parent first, so an operation can never live in two blocks at once.
    pub fn attach_op(&mut self, block: BlockId, op: OpId) {
        self.detach_op(op);
        self.op_mut(op).parent = Some(block);
        self.block_mut(block).ops.push(op);
    }

    pub fn append_op(&mut self, block: BlockId, op: Operation) -> OpId {
        let id = self.alloc_op(op);
        self.attach_op(block, id);
        id
    }

    pub fn insert_op(&mut self, block: BlockId, index: usize, op: Operation) -> OpId {
        let id = self.alloc_op(op);
        self.detach_op(id);
        self.op_mut(id).parent = Some(block);
        let len = self.block(block).ops.len();
        self.block_mut(block).ops.insert(index.min(len), id);
        id
    }

    pub fn add_root(&mut self, op: Operation) -> OpId {
        let id = self.alloc_op(op);
        self.roots.push(id);
        id
    }

    pub fn push_root(&mut self, op: OpId) {
        self.detach_op(op);
        if !self.roots.contains(&op) {
            self.roots.push(op);
        }
    }

    /// Unlink an operation and reclaim it, its regions, their blocks and the
    /// operations nested inside them.
    pub fn erase_op(&mut self, op: OpId) {
        if self.try_op(op).is_none() {
            return;
        }
        self.detach_op(op);
        self.free_op(op);
    }

    fn free_block(&mut self, block: BlockId) {
        let Some(data) = self.blocks.get_mut(block) else {
            return;
        };
        let ops = std::mem::take(&mut data.ops);
        let region = data.parent;
        for child in ops {
            self.free_op(child);
        }
        if let Some(region) = self.regions.get_mut(region) {
            region.blocks.retain(|id| *id != block);
        }
        self.blocks.remove(block);
    }

    fn free_op(&mut self, op: OpId) {
        let Some(data) = self.ops.get_mut(op) else {
            return;
        };
        let regions = std::mem::take(&mut data.regions);
        for region in regions {
            if let Some(data) = self.regions.get_mut(region) {
                let blocks = std::mem::take(&mut data.blocks);
                for block in blocks {
                    self.free_block(block);
                }
            }
            self.regions.remove(region);
        }
        self.ops.remove(op);
    }

    pub fn walk_ops(&self, root: OpId) -> Vec<OpId> {
        let mut out = Vec::new();
        self.collect_ops(root, &mut out);
        out
    }

    fn collect_ops(&self, op: OpId, out: &mut Vec<OpId>) {
        out.push(op);
        for region in &self.op(op).regions {
            for block in &self.region(*region).blocks {
                for child in &self.block(*block).ops {
                    self.collect_ops(*child, out);
                }
            }
        }
    }

    pub fn walk_all_ops(&self) -> Vec<OpId> {
        let mut out = Vec::new();
        for root in &self.roots {
            self.collect_ops(*root, &mut out);
        }
        out
    }

    pub fn replace_value(&mut self, old: Value, new: Value) {
        for op in self.walk_all_ops() {
            for operand in &mut self.op_mut(op).operands {
                if *operand == old {
                    *operand = new;
                }
            }
        }
    }

    /// Restore point for [`Context::rollback`].
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            ops: self.ops.checkpoint(),
            blocks: self.blocks.checkpoint(),
            regions: self.regions.checkpoint(),
            roots: self.roots.len(),
            symbols: self.symbols.len(),
            types: self.types.len(),
        }
    }

    /// Drop everything allocated since `checkpoint`, restoring the context to
    /// the exact state it had then.
    pub fn rollback(&mut self, checkpoint: Checkpoint) {
        self.ops.rollback(checkpoint.ops);
        self.blocks.rollback(checkpoint.blocks);
        self.regions.rollback(checkpoint.regions);
        self.roots.truncate(checkpoint.roots);
        self.symbols.truncate(checkpoint.symbols);
        self.types.truncate(checkpoint.types);
    }

    /// Full structural check of the whole context: every handle resolves, every
    /// parent/child link is mirrored, every spec matches, and no operation is
    /// orphaned from both the root list and any block.
    pub fn verify(&self) -> crate::Result<()> {
        self.verify_structure()?;
        for (op_id, op) in self.iter_ops() {
            if let Some(spec) = op.spec {
                spec.check_operation(self, op)?;
                if let Some(verify) = spec.verify {
                    verify(self, op_id)?;
                }
            }
        }
        Ok(())
    }

    fn verify_structure(&self) -> crate::Result<()> {
        for (block_id, block) in self.iter_blocks() {
            let Some(region) = self.try_region(block.parent) else {
                return Err(crate::Error::new(format!(
                    "block {block_id:?} points at a dangling region {:?}",
                    block.parent
                )));
            };
            if !region.blocks.contains(&block_id) {
                return Err(crate::Error::new(format!(
                    "block {block_id:?} is not listed by its region {:?}",
                    block.parent
                )));
            }
            for ty in &block.args {
                self.verify_type_tree(*ty)?;
            }
            for op_id in &block.ops {
                let Some(op) = self.try_op(*op_id) else {
                    return Err(crate::Error::new(format!(
                        "block {block_id:?} holds a dangling operation {op_id:?}"
                    )));
                };
                if op.parent != Some(block_id) {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} sits in block {block_id:?} but its parent is {:?}",
                        op.parent
                    )));
                }
            }
        }

        for (region_id, region) in self.iter_regions() {
            for block_id in &region.blocks {
                let Some(block) = self.try_block(*block_id) else {
                    return Err(crate::Error::new(format!(
                        "region {region_id:?} holds a dangling block {block_id:?}"
                    )));
                };
                if block.parent != region_id {
                    return Err(crate::Error::new(format!(
                        "block {block_id:?} sits in region {region_id:?} but its parent is {:?}",
                        block.parent
                    )));
                }
            }
            if let Some(parent) = region.parent
                && self
                    .try_op(parent)
                    .is_none_or(|op| !op.regions.contains(&region_id))
            {
                return Err(crate::Error::new(format!(
                    "region {region_id:?} is not listed by its parent operation {parent:?}"
                )));
            }
        }

        for (op_id, op) in self.iter_ops() {
            if let Some(block) = op.parent {
                let Some(data) = self.try_block(block) else {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} points at a dangling block {block:?}"
                    )));
                };
                if !data.ops.contains(&op_id) {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} claims block {block:?} but is not listed there"
                    )));
                }
            }
            if op.parent.is_none() && !self.roots.contains(&op_id) {
                return Err(crate::Error::new(format!(
                    "operation {op_id:?} is neither in a block nor a root"
                )));
            }
            for ty in &op.results {
                self.verify_type_tree(*ty)?;
            }
            for attr in op.attrs.iter().map(|(_, attr)| attr) {
                self.verify_attr(attr)?;
            }
            for operand in &op.operands {
                if self.try_value_type(*operand).is_none() {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} uses a dangling operand {operand:?}"
                    )));
                }
            }
            for region_id in &op.regions {
                let Some(region) = self.try_region(*region_id) else {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} holds a dangling region {region_id:?}"
                    )));
                };
                if region.parent != Some(op_id) {
                    return Err(crate::Error::new(format!(
                        "region {region_id:?} sits in operation {op_id:?} but its parent is {:?}",
                        region.parent
                    )));
                }
            }
            for block_id in &op.successors {
                if self.try_block(*block_id).is_none() {
                    return Err(crate::Error::new(format!(
                        "operation {op_id:?} branches to a dangling block {block_id:?}"
                    )));
                }
            }
        }

        for root in &self.roots {
            let Some(op) = self.try_op(*root) else {
                return Err(crate::Error::new(format!(
                    "root list holds a dangling operation {root:?}"
                )));
            };
            if op.parent.is_some() {
                return Err(crate::Error::new(format!(
                    "root operation {root:?} has a parent block"
                )));
            }
        }
        Ok(())
    }

    fn verify_type_tree(&self, ty: TypeId) -> crate::Result<()> {
        let Some(data) = self.try_type_node(ty) else {
            return Err(crate::Error::new(format!("dangling type {ty:?}")));
        };
        for arg in &data.args {
            match arg {
                TypeArg::Type(inner) | TypeArg::Field(_, inner) => self.verify_type_tree(*inner)?,
                TypeArg::Int(_) | TypeArg::Str(_) | TypeArg::Bool(_) => {}
            }
        }
        for attr in data.attrs.iter().map(|(_, attr)| attr) {
            self.verify_attr(attr)?;
        }
        Ok(())
    }

    fn verify_attr(&self, attr: &Attr) -> crate::Result<()> {
        match attr {
            Attr::Int { ty, .. } => {
                self.verify_type_tree(*ty)?;
                if self.int_info(*ty).is_none() {
                    return Err(crate::Error::new(format!(
                        "integer attribute has non-integer type {ty:?}"
                    )));
                }
            }
            Attr::Float { ty, .. } => {
                self.verify_type_tree(*ty)?;
                if self.float_width(*ty).is_none() {
                    return Err(crate::Error::new(format!(
                        "float attribute has non-float type {ty:?}"
                    )));
                }
            }
            Attr::Type(ty) => self.verify_type_tree(*ty)?,
            Attr::Array(values) => {
                for value in values {
                    self.verify_attr(value)?;
                }
            }
            Attr::Dict(dict) => {
                for (_, value) in dict.iter() {
                    self.verify_attr(value)?;
                }
            }
            Attr::Opaque { args, .. } => {
                for arg in args {
                    self.verify_attr(arg)?;
                }
            }
            Attr::Unit | Attr::Bool(_) | Attr::Str(_) | Attr::Symbol(_) => {}
        }
        Ok(())
    }
}

/// Saved context state, see [`Context::checkpoint`].
#[derive(Debug, Clone)]
pub struct Checkpoint {
    ops: crate::arena::ArenaCheckpoint,
    blocks: crate::arena::ArenaCheckpoint,
    regions: crate::arena::ArenaCheckpoint,
    roots: usize,
    symbols: usize,
    types: usize,
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match crate::text::print(self) {
            Ok(text) => f.write_str(&text),
            Err(err) => write!(f, "<invalid IR: {err}>"),
        }
    }
}

/// Builder for a single operation.
pub struct OpBuilder<'a> {
    ctx: &'a mut Context,
    spec: Option<&'static OpSpec>,
    op: Operation,
}

impl<'a> OpBuilder<'a> {
    pub fn operand(mut self, value: Value) -> Self {
        self.op.operands.push(value);
        self
    }

    pub fn operands(mut self, values: impl IntoIterator<Item = Value>) -> Self {
        self.op.operands.extend(values);
        self
    }

    pub fn result(mut self, ty: TypeId) -> Self {
        self.op.results.push(ty);
        self
    }

    pub fn results(mut self, types: impl IntoIterator<Item = TypeId>) -> Self {
        self.op.results.extend(types);
        self
    }

    pub fn attr(mut self, key: &str, value: Attr) -> Self {
        let key = self.ctx.intern(key);
        self.op.attrs.set(key, value);
        self
    }

    pub fn successor(mut self, block: BlockId) -> Self {
        self.op.successors.push(block);
        self
    }

    pub fn region(mut self, region: RegionId) -> Self {
        self.op.regions.push(region);
        self
    }

    pub fn finish(self) -> Operation {
        if let Some(spec) = self.spec
            && let Err(err) = spec.check_builder(self.ctx, &self.op)
        {
            panic!("invalid {} construction: {err}", spec.name);
        }
        self.op
    }
}
