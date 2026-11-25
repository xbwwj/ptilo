#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use std::collections::HashMap;

use mir::MirInsert as _;
use ptilo::{Attr, Context, Signedness, Value};

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let fn_ty = ctx.func_type(&[i32_ty, i32_ty], &[i32_ty]);

    let (_, module_block) = mir::module(&mut ctx);
    let (func, entry) = module_block.insert_func(&mut ctx, "add", fn_ty, &[i32_ty, i32_ty]);
    let a = Value::block_arg(entry, 0);
    let b = Value::block_arg(entry, 1);
    let sum = entry.insert_add(&mut ctx, a, b, i32_ty);
    entry.insert_return(&mut ctx, &[sum]);

    let src_region = ctx.op(func).regions[0];
    let src_block = ctx.region(src_region).blocks[0];
    let src_args = ctx.block(src_block).args.clone();

    let lowered_sym = ctx.intern("add_lowered");
    let lowered = ctx
        .create_op("llvm.func")
        .attr("sym_name", Attr::symbol(lowered_sym))
        .attr("type", Attr::ty(fn_ty))
        .finish();
    let lowered = ctx.add_root(lowered);
    let lowered_region = ctx.add_region(lowered);
    let lowered_block = ctx.new_block(lowered_region, &src_args);

    let mut value_map = HashMap::new();
    for index in 0..src_args.len() {
        value_map.insert(
            Value::block_arg(src_block, index as u32),
            Value::block_arg(lowered_block, index as u32),
        );
    }

    for op in ctx.ops_in_block(src_block).to_vec() {
        let name = ctx.op_name(op).to_owned();
        let operands = ctx.op(op).operands.clone();
        let results = ctx.op(op).results.clone();
        let lowered_op = match name.as_str() {
            "mir.constant" => {
                let value = ctx.attr(op, "value").cloned().expect("constant value");
                ctx.create_op("llvm.constant")
                    .attr("value", value)
                    .results(results)
                    .finish()
            }
            "mir.add" => {
                let operands = operands
                    .iter()
                    .map(|value| *value_map.get(value).expect("mapped operand"))
                    .collect::<Vec<_>>();
                ctx.create_op("llvm.add")
                    .operands(operands)
                    .results(results)
                    .finish()
            }
            "mir.return" => {
                let operands = operands
                    .iter()
                    .map(|value| *value_map.get(value).expect("mapped operand"))
                    .collect::<Vec<_>>();
                ctx.create_op("llvm.return").operands(operands).finish()
            }
            other => panic!("unsupported op {other}"),
        };
        let lowered_op = ctx.append_op(lowered_block, lowered_op);
        for index in 0..ctx.op(lowered_op).results.len() {
            value_map.insert(
                Value::op_result(op, index as u32),
                Value::op_result(lowered_op, index as u32),
            );
        }
    }

    verify::all(&ctx)?;
    println!("{ctx}");

    let text = ctx.to_string();
    assert!(text.contains("\"llvm.add\""), "{text}");
    assert!(text.contains("\"llvm.return\""), "{text}");
    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
