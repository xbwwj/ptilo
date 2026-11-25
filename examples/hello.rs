#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use mir::MirInsert as _;
use ptilo::{Context, Signedness};

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let fn_ty = ctx.func_type(&[], &[i32_ty]);

    let (_, module_block) = mir::module(&mut ctx);
    let (_, entry) = module_block.insert_func(&mut ctx, "main", fn_ty, &[]);

    let c40 = entry.insert_constant(&mut ctx, i32_ty, 40);
    let c2 = entry.insert_constant(&mut ctx, i32_ty, 2);
    let sum = entry.insert_add(&mut ctx, c40, c2, i32_ty);
    entry.insert_return(&mut ctx, &[sum]);

    verify::all(&ctx)?;
    println!("{ctx}");

    let text = ctx.to_string();
    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
