#[path = "../examples/support/verify.rs"]
mod verify;

use ptilo::{Attr, AttrConstraint, Context, OpSpec, Signedness, TypeId, Value, attr, opd, res};

const ADD: OpSpec = OpSpec {
    name: "test.add",
    operands: &[opd("lhs"), opd("rhs")],
    results: &[res("res")],
    attrs: &[attr("tag", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

fn value(ctx: &mut Context, ty: TypeId) -> Value {
    let op = ctx.create_op("test.value").result(ty).finish();
    let op = ctx.add_root(op);
    Value::op_result(op, 0)
}

#[test]
fn op_spec_build_and_verify() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let a = value(&mut ctx, i32_ty);
    let b = value(&mut ctx, i32_ty);
    let tag = ctx.intern("ok");
    let op = ctx
        .create_op_spec(&ADD)
        .operands([a, b])
        .result(i32_ty)
        .attr("tag", Attr::string(tag))
        .finish();
    ctx.add_root(op);
    verify::all(&ctx).unwrap();
}

#[test]
fn op_spec_rejects_wrong_arity() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let a = value(&mut ctx, i32_ty);
    let tag = ctx.intern("ok");
    let op = ctx
        .create_op("test.add")
        .operand(a)
        .result(i32_ty)
        .attr("tag", Attr::string(tag))
        .finish();
    assert!(ADD.check_operation(&ctx, &op).is_err());
}

#[test]
fn op_spec_rejects_missing_attr() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let a = value(&mut ctx, i32_ty);
    let b = value(&mut ctx, i32_ty);
    let op = ctx
        .create_op("test.add")
        .operands([a, b])
        .result(i32_ty)
        .finish();
    assert!(ADD.check_operation(&ctx, &op).is_err());
}
