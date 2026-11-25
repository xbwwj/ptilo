#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use mir::MirInsert as _;
use ptilo::{BlockId, Cfg, Context, Signedness, Value};

struct FuncCfg<'a> {
    ctx: &'a Context,
    entry: BlockId,
}

impl Cfg for FuncCfg<'_> {
    type Block = BlockId;

    fn entry(&self) -> Self::Block {
        self.entry
    }

    fn successors(&self, block: Self::Block) -> Vec<Self::Block> {
        self.ctx.block_successors(block)
    }

    fn predecessors(&self, block: Self::Block) -> Vec<Self::Block> {
        self.ctx.block_predecessors(block)
    }
}

fn reachable<C: Cfg>(cfg: &C) -> Vec<C::Block>
where
    C::Block: Eq + Hash,
{
    let mut seen = HashSet::new();
    let mut stack = vec![cfg.entry()];
    let mut order = Vec::new();
    while let Some(block) = stack.pop() {
        if !seen.insert(block) {
            continue;
        }
        order.push(block);
        stack.extend(cfg.successors(block));
    }
    order
}

fn dominators<C: Cfg>(cfg: &C) -> HashMap<C::Block, HashSet<C::Block>>
where
    C::Block: Copy + Eq + Hash,
{
    let blocks = reachable(cfg);
    let entry = cfg.entry();
    let all: HashSet<C::Block> = blocks.iter().copied().collect();
    let mut dom: HashMap<C::Block, HashSet<C::Block>> = HashMap::new();

    for block in &blocks {
        if *block == entry {
            dom.insert(*block, HashSet::from([*block]));
        } else {
            dom.insert(*block, all.clone());
        }
    }

    loop {
        let mut changed = false;
        for block in &blocks {
            if *block == entry {
                continue;
            }
            let preds = cfg.predecessors(*block);
            let mut new_set = if preds.is_empty() {
                HashSet::new()
            } else {
                let mut iter = preds.iter();
                let first = *iter.next().unwrap();
                let mut set = dom.get(&first).cloned().unwrap_or_default();
                for pred in iter {
                    let other = dom.get(pred).cloned().unwrap_or_default();
                    set.retain(|block| other.contains(block));
                }
                set
            };
            new_set.insert(*block);
            if dom.get(block) != Some(&new_set) {
                dom.insert(*block, new_set);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    dom
}

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let i1_ty = ctx.int_type(1, Signedness::Signless);
    let fn_ty = ctx.func_type(&[i32_ty], &[i32_ty]);

    let (_, module_block) = mir::module(&mut ctx);
    let (func, entry) = module_block.insert_func(&mut ctx, "count_to", fn_ty, &[i32_ty]);
    let body = ctx.op(func).regions[0];
    let loop_block = ctx.new_block(body, &[i32_ty]);
    let exit = ctx.new_block(body, &[i32_ty]);

    let n = Value::block_arg(entry, 0);
    let i = Value::block_arg(loop_block, 0);
    let result = Value::block_arg(exit, 0);

    let zero = entry.insert_constant(&mut ctx, i32_ty, 0);
    let one = entry.insert_constant(&mut ctx, i32_ty, 1);
    entry.insert_goto(&mut ctx, loop_block, &[zero]);

    let next_i = loop_block.insert_add(&mut ctx, i, one, i32_ty);
    let cond = loop_block.insert_lt(&mut ctx, next_i, n, i1_ty);
    loop_block.insert_cond_br(&mut ctx, cond, loop_block, &[next_i], exit, &[next_i]);
    exit.insert_return(&mut ctx, &[result]);
    verify::all(&ctx)?;

    let cfg = FuncCfg { ctx: &ctx, entry };
    for block in [entry, loop_block, exit] {
        println!(
            "bb{}: succ={:?} pred={:?}",
            block.index(),
            cfg.successors(block)
                .iter()
                .map(|b| b.index())
                .collect::<Vec<_>>(),
            cfg.predecessors(block)
                .iter()
                .map(|b| b.index())
                .collect::<Vec<_>>()
        );
    }

    let dom = dominators(&cfg);
    for block in [entry, loop_block, exit] {
        let mut blocks: Vec<_> = dom[&block].iter().map(|b| b.index()).collect();
        blocks.sort_unstable();
        println!("dom(bb{}) = {:?}", block.index(), blocks);
    }

    let uses = ctx.uses(n);
    println!(
        "uses of bb{}(%n) = {:?}",
        entry.index(),
        uses.iter()
            .map(|(op, index)| (ctx.op_name(*op), *index))
            .collect::<Vec<_>>()
    );

    Ok(())
}
