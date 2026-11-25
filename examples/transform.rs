#[path = "support/mir.rs"]
mod mir;
#[path = "support/verify.rs"]
mod verify;

use std::collections::HashSet;

use mir::MirInsert as _;
use ptilo::{Attr, Context, OpId, Signedness, TypeId, Value};

fn const_int(ctx: &Context, value: Value) -> Option<(TypeId, u128)> {
    let Value::OpResult { op, index } = value else {
        return None;
    };
    if index != 0 || !ctx.is_op(op, "mir.constant") {
        return None;
    }
    match ctx.attr(op, "value")? {
        Attr::Int { ty, bits } => Some((*ty, *bits)),
        _ => None,
    }
}

fn fold_constants(ctx: &mut Context, root: OpId) -> bool {
    let mut changed = false;
    for op in ctx.walk_ops(root) {
        if ctx.try_op(op).is_none() {
            continue;
        }
        let name = ctx.op_name(op).to_owned();
        if !matches!(name.as_str(), "mir.add" | "mir.mul") {
            continue;
        }
        let operands = ctx.op(op).operands.clone();
        if operands.len() != 2 {
            continue;
        }
        let Some((lhs_ty, lhs)) = const_int(ctx, operands[0]) else {
            continue;
        };
        let Some((rhs_ty, rhs)) = const_int(ctx, operands[1]) else {
            continue;
        };
        if lhs_ty != rhs_ty {
            continue;
        }
        let bits = match name.as_str() {
            "mir.add" => lhs.wrapping_add(rhs),
            "mir.mul" => lhs.wrapping_mul(rhs),
            _ => unreachable!(),
        };
        let new_op = ctx
            .create_op_spec(&mir::CONSTANT)
            .attr("value", Attr::int(lhs_ty, bits))
            .result(lhs_ty)
            .finish();
        let parent = ctx.op(op).parent.expect("folded op is in a block");
        let index = ctx
            .ops_in_block(parent)
            .iter()
            .position(|id| *id == op)
            .expect("folded op is in its parent block");
        let new_op = ctx.insert_op(parent, index, new_op);
        ctx.replace_value(Value::op_result(op, 0), Value::op_result(new_op, 0));
        ctx.erase_op(op);
        changed = true;
    }
    changed
}

fn is_pure(name: &str) -> bool {
    matches!(name, "mir.constant" | "mir.add" | "mir.mul" | "mir.lt")
}

fn dce(ctx: &mut Context, root: OpId) -> bool {
    let mut changed = false;
    loop {
        let mut used = HashSet::new();
        for op in ctx.walk_ops(root) {
            for operand in &ctx.op(op).operands {
                used.insert(*operand);
            }
        }

        let mut removed = false;
        for op in ctx.walk_ops(root) {
            if ctx.try_op(op).is_none() {
                continue;
            }
            if !is_pure(ctx.op_name(op)) {
                continue;
            }
            let result_count = ctx.op(op).results.len();
            if result_count == 0 {
                continue;
            }
            let dead =
                (0..result_count).all(|index| !used.contains(&Value::op_result(op, index as u32)));
            if dead {
                ctx.erase_op(op);
                removed = true;
                changed = true;
            }
        }
        if !removed {
            break;
        }
    }
    changed
}

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let fn_ty = ctx.func_type(&[], &[i32_ty]);

    let (module, module_block) = mir::module(&mut ctx);
    let (_, entry) = module_block.insert_func(&mut ctx, "fold", fn_ty, &[]);

    let c1 = entry.insert_constant(&mut ctx, i32_ty, 1);
    let c2 = entry.insert_constant(&mut ctx, i32_ty, 2);
    let sum = entry.insert_add(&mut ctx, c1, c2, i32_ty);
    let c4 = entry.insert_constant(&mut ctx, i32_ty, 4);
    let c5 = entry.insert_constant(&mut ctx, i32_ty, 5);
    let dead = entry.insert_add(&mut ctx, c4, c5, i32_ty);
    let _ = dead;
    let prod = entry.insert_mul(&mut ctx, sum, sum, i32_ty);
    entry.insert_return(&mut ctx, &[prod]);

    println!("before:\n{ctx}");
    assert!(fold_constants(&mut ctx, module));
    assert!(dce(&mut ctx, module));
    verify::all(&ctx)?;

    let text = ctx.to_string();
    println!("after:\n{text}");
    assert!(text.contains("value = 9 : i32"), "{text}");
    assert!(!text.contains("value = 4 : i32"), "{text}");
    assert!(!text.contains("value = 5 : i32"), "{text}");

    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
