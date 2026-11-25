#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use mir::MirInsert as _;
use ptilo::{Context, Signedness, Value};

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let i1_ty = ctx.int_type(1, Signedness::Signless);
    let ptr_i32 = mir::ptr_type(&mut ctx, i32_ty);
    let fn_ty = ctx.func_type(&[i32_ty], &[i32_ty]);

    let (_, module_block) = mir::module(&mut ctx);
    let (func, entry) = module_block.insert_func(&mut ctx, "sum_to", fn_ty, &[i32_ty]);
    let body = ctx.op(func).regions[0];
    let loop_block = ctx.new_block(body, &[i32_ty]);
    let exit = ctx.new_block(body, &[i32_ty]);

    let n = Value::block_arg(entry, 0);
    let i = Value::block_arg(loop_block, 0);
    let result = Value::block_arg(exit, 0);

    let zero = entry.insert_constant(&mut ctx, i32_ty, 0);
    let one = entry.insert_constant(&mut ctx, i32_ty, 1);
    let slot = entry.insert_alloca(&mut ctx, ptr_i32, true);
    entry.insert_store(&mut ctx, zero, slot);
    entry.insert_goto(&mut ctx, loop_block, &[zero]);

    let acc = loop_block.insert_load(&mut ctx, slot, i32_ty);
    let next_acc = loop_block.insert_add(&mut ctx, acc, i, i32_ty);
    loop_block.insert_store(&mut ctx, next_acc, slot);
    let next_i = loop_block.insert_add(&mut ctx, i, one, i32_ty);
    let cond = loop_block.insert_lt(&mut ctx, next_i, n, i1_ty);
    loop_block.insert_cond_br(&mut ctx, cond, loop_block, &[next_i], exit, &[next_acc]);

    exit.insert_return(&mut ctx, &[result]);

    verify::all(&ctx)?;
    println!("{ctx}");

    let text = ctx.to_string();
    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
