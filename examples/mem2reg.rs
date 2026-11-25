#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use mir::MirInsert as _;
use ptilo::{Attr, BlockId, Context, Mem2Reg, OpId, Signedness, Value};

struct MirMem2Reg<'a> {
    ctx: &'a mut Context,
    func: OpId,
}

impl<'a> MirMem2Reg<'a> {
    fn new(ctx: &'a mut Context, func: OpId) -> Self {
        Self { ctx, func }
    }

    fn function_blocks(&self) -> Vec<BlockId> {
        let mut blocks = Vec::new();
        for region in &self.ctx.op(self.func).regions {
            blocks.extend(self.ctx.region(*region).blocks.iter().copied());
        }
        blocks
    }
}

impl Mem2Reg for MirMem2Reg<'_> {
    type Func = OpId;
    type Block = BlockId;
    type Value = Value;
    type Alloca = OpId;
    type Load = OpId;
    type Store = OpId;

    fn blocks(&self, _func: Self::Func) -> Vec<Self::Block> {
        self.function_blocks()
    }

    fn predecessors(&self, _block: Self::Block) -> Vec<Self::Block> {
        Vec::new()
    }

    fn allocas(&self, block: Self::Block) -> Vec<Self::Alloca> {
        self.ctx
            .ops_in_block(block)
            .iter()
            .copied()
            .filter(|op| self.ctx.is_op(*op, "mir.alloca"))
            .collect()
    }

    fn is_promotable(&self, alloca: Self::Alloca) -> bool {
        matches!(self.ctx.attr(alloca, "promotable"), Some(Attr::Bool(true)))
    }

    fn loads(&self, alloca: Self::Alloca) -> Vec<Self::Load> {
        let pointer = Value::op_result(alloca, 0);
        self.ctx
            .walk_ops(self.func)
            .into_iter()
            .filter(|op| {
                self.ctx.is_op(*op, "mir.load")
                    && self.ctx.op(*op).operands.first() == Some(&pointer)
            })
            .collect()
    }

    fn stores(&self, alloca: Self::Alloca) -> Vec<Self::Store> {
        let pointer = Value::op_result(alloca, 0);
        self.ctx
            .walk_ops(self.func)
            .into_iter()
            .filter(|op| {
                self.ctx.is_op(*op, "mir.store")
                    && self.ctx.op(*op).operands.get(1) == Some(&pointer)
            })
            .collect()
    }

    fn load_value(&self, load: Self::Load) -> Self::Value {
        Value::op_result(load, 0)
    }

    fn store_value(&self, store: Self::Store) -> Self::Value {
        self.ctx.op(store).operands[0]
    }

    fn insert_phi(&mut self, _block: Self::Block) -> Self::Value {
        unimplemented!("the example only promotes straight-line allocas")
    }

    fn replace_value(&mut self, old: Self::Value, new: Self::Value) {
        self.ctx.replace_value(old, new);
    }

    fn erase_load(&mut self, load: Self::Load) {
        self.ctx.erase_op(load);
    }

    fn erase_store(&mut self, store: Self::Store) {
        self.ctx.erase_op(store);
    }

    fn erase_alloca(&mut self, alloca: Self::Alloca) {
        self.ctx.erase_op(alloca);
    }
}

fn promote_single_store<M: Mem2Reg>(mem2reg: &mut M, func: M::Func) {
    for block in mem2reg.blocks(func) {
        for alloca in mem2reg.allocas(block) {
            if !mem2reg.is_promotable(alloca) {
                continue;
            }
            let stores = mem2reg.stores(alloca);
            if stores.len() != 1 {
                continue;
            }
            let value = mem2reg.store_value(stores[0]);
            for load in mem2reg.loads(alloca) {
                let loaded = mem2reg.load_value(load);
                mem2reg.replace_value(loaded, value);
                mem2reg.erase_load(load);
            }
            mem2reg.erase_store(stores[0]);
            mem2reg.erase_alloca(alloca);
        }
    }
}

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let ptr_i32 = mir::ptr_type(&mut ctx, i32_ty);
    let fn_ty = ctx.func_type(&[i32_ty], &[i32_ty]);

    let (_, module_block) = mir::module(&mut ctx);
    let (func, entry) = module_block.insert_func(&mut ctx, "promote_me", fn_ty, &[i32_ty]);
    let x = Value::block_arg(entry, 0);
    let slot = entry.insert_alloca(&mut ctx, ptr_i32, true);
    entry.insert_store(&mut ctx, x, slot);
    let load_a = entry.insert_load(&mut ctx, slot, i32_ty);
    let load_b = entry.insert_load(&mut ctx, slot, i32_ty);
    let sum = entry.insert_add(&mut ctx, load_a, load_b, i32_ty);
    entry.insert_return(&mut ctx, &[sum]);

    verify::all(&ctx)?;
    println!("before:\n{ctx}");
    assert!(ctx.to_string().contains("\"mir.load\""));

    {
        let mut mem2reg = MirMem2Reg::new(&mut ctx, func);
        promote_single_store(&mut mem2reg, func);
    }

    verify::all(&ctx)?;
    let text = ctx.to_string();
    println!("after:\n{text}");
    assert!(!text.contains("\"mir.load\""), "{text}");
    assert!(!text.contains("\"mir.store\""), "{text}");
    assert!(!text.contains("\"mir.alloca\""), "{text}");
    assert!(text.contains("\"mir.add\""), "{text}");

    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
