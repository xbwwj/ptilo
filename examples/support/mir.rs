#![allow(dead_code)]

use ptilo::{
    Attr, AttrConstraint, BlockId, Context, OpId, OpSpec, Result, TypeArg, TypeId, Value, attr,
    opd, opd_var, region, res,
};

pub const MODULE: OpSpec = OpSpec {
    name: "builtin.module",
    regions: &[region("body")],
    ..OpSpec::DEFAULT
};

pub const FUNC: OpSpec = OpSpec {
    name: "mir.func",
    regions: &[region("body")],
    attrs: &[
        attr("sym_name", true, AttrConstraint::Symbol),
        attr("type", true, AttrConstraint::Type),
    ],
    verify: Some(verify_func),
    ..OpSpec::DEFAULT
};

pub const CONSTANT: OpSpec = OpSpec {
    name: "mir.constant",
    results: &[res("value")],
    attrs: &[attr("value", true, AttrConstraint::Int)],
    verify: Some(verify_constant),
    ..OpSpec::DEFAULT
};

pub const ADD: OpSpec = OpSpec {
    name: "mir.add",
    operands: &[opd("lhs"), opd("rhs")],
    results: &[res("res")],
    verify: Some(verify_same_type),
    ..OpSpec::DEFAULT
};

pub const LT: OpSpec = OpSpec {
    name: "mir.lt",
    operands: &[opd("lhs"), opd("rhs")],
    results: &[res("res")],
    verify: Some(verify_comparison),
    ..OpSpec::DEFAULT
};

pub const MUL: OpSpec = OpSpec {
    name: "mir.mul",
    operands: &[opd("lhs"), opd("rhs")],
    results: &[res("res")],
    verify: Some(verify_same_type),
    ..OpSpec::DEFAULT
};

pub const ALLOCA: OpSpec = OpSpec {
    name: "mir.alloca",
    results: &[res("ptr")],
    attrs: &[attr("promotable", false, AttrConstraint::Bool)],
    verify: Some(verify_alloca),
    ..OpSpec::DEFAULT
};

pub const LOAD: OpSpec = OpSpec {
    name: "mir.load",
    operands: &[opd("ptr")],
    results: &[res("value")],
    verify: Some(verify_load),
    ..OpSpec::DEFAULT
};

pub const STORE: OpSpec = OpSpec {
    name: "mir.store",
    operands: &[opd("value"), opd("ptr")],
    verify: Some(verify_store),
    ..OpSpec::DEFAULT
};

pub const GOTO: OpSpec = OpSpec {
    name: "mir.goto",
    operands: &[opd_var("args")],
    terminator: true,
    ..OpSpec::DEFAULT
};

pub const COND_BR: OpSpec = OpSpec {
    name: "mir.cond_br",
    operands: &[opd("cond"), opd_var("args")],
    terminator: true,
    ..OpSpec::DEFAULT
};

pub const RETURN: OpSpec = OpSpec {
    name: "mir.return",
    operands: &[opd_var("values")],
    terminator: true,
    ..OpSpec::DEFAULT
};

pub fn module(ctx: &mut Context) -> (OpId, BlockId) {
    let op = ctx.create_op_spec(&MODULE).finish();
    let op = ctx.add_root(op);
    let region = ctx.add_region(op);
    let block = ctx.new_block(region, &[]);
    (op, block)
}

pub fn ptr_type(ctx: &mut Context, pointee: TypeId) -> TypeId {
    ctx.opaque_type("mir.ptr", vec![TypeArg::Type(pointee)])
}

pub trait MirInsert {
    fn insert_func(
        &self,
        ctx: &mut Context,
        name: &str,
        fn_ty: TypeId,
        arg_types: &[TypeId],
    ) -> (OpId, BlockId);
    fn insert_constant(&self, ctx: &mut Context, ty: TypeId, bits: u128) -> Value;
    fn insert_add(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value;
    fn insert_mul(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value;
    fn insert_lt(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value;
    fn insert_alloca(&self, ctx: &mut Context, ptr_ty: TypeId, promotable: bool) -> Value;
    fn insert_load(&self, ctx: &mut Context, ptr: Value, ty: TypeId) -> Value;
    fn insert_store(&self, ctx: &mut Context, value: Value, ptr: Value);
    fn insert_goto(&self, ctx: &mut Context, target: BlockId, args: &[Value]);
    fn insert_cond_br(
        &self,
        ctx: &mut Context,
        cond: Value,
        true_block: BlockId,
        true_args: &[Value],
        false_block: BlockId,
        false_args: &[Value],
    );
    fn insert_return(&self, ctx: &mut Context, values: &[Value]);
}

impl MirInsert for BlockId {
    fn insert_func(
        &self,
        ctx: &mut Context,
        name: &str,
        fn_ty: TypeId,
        arg_types: &[TypeId],
    ) -> (OpId, BlockId) {
        let sym = ctx.intern(name);
        let op = ctx
            .create_op_spec(&FUNC)
            .attr("sym_name", Attr::symbol(sym))
            .attr("type", Attr::ty(fn_ty))
            .finish();
        let op = ctx.append_op(*self, op);
        let region = ctx.add_region(op);
        let entry = ctx.new_block(region, arg_types);
        (op, entry)
    }

    fn insert_constant(&self, ctx: &mut Context, ty: TypeId, bits: u128) -> Value {
        let op = ctx
            .create_op_spec(&CONSTANT)
            .attr("value", Attr::int(ty, bits))
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_add(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value {
        let op = ctx
            .create_op_spec(&ADD)
            .operands([lhs, rhs])
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_mul(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value {
        let op = ctx
            .create_op_spec(&MUL)
            .operands([lhs, rhs])
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_lt(&self, ctx: &mut Context, lhs: Value, rhs: Value, ty: TypeId) -> Value {
        let op = ctx
            .create_op_spec(&LT)
            .operands([lhs, rhs])
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_alloca(&self, ctx: &mut Context, ptr_ty: TypeId, promotable: bool) -> Value {
        let op = ctx
            .create_op_spec(&ALLOCA)
            .attr("promotable", Attr::boolean(promotable))
            .result(ptr_ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_load(&self, ctx: &mut Context, ptr: Value, ty: TypeId) -> Value {
        let op = ctx.create_op_spec(&LOAD).operand(ptr).result(ty).finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_store(&self, ctx: &mut Context, value: Value, ptr: Value) {
        let op = ctx.create_op_spec(&STORE).operands([value, ptr]).finish();
        ctx.append_op(*self, op);
    }

    fn insert_goto(&self, ctx: &mut Context, target: BlockId, args: &[Value]) {
        let op = ctx
            .create_op_spec(&GOTO)
            .operands(args.iter().copied())
            .successor(target)
            .finish();
        ctx.append_op(*self, op);
    }

    fn insert_cond_br(
        &self,
        ctx: &mut Context,
        cond: Value,
        true_block: BlockId,
        true_args: &[Value],
        false_block: BlockId,
        false_args: &[Value],
    ) {
        let op = ctx
            .create_op_spec(&COND_BR)
            .operand(cond)
            .operands(true_args.iter().copied())
            .operands(false_args.iter().copied())
            .successor(true_block)
            .successor(false_block)
            .finish();
        ctx.append_op(*self, op);
    }

    fn insert_return(&self, ctx: &mut Context, values: &[Value]) {
        let op = ctx
            .create_op_spec(&RETURN)
            .operands(values.iter().copied())
            .finish();
        ctx.append_op(*self, op);
    }
}

fn verify_same_type(ctx: &Context, op_id: OpId) -> Result<()> {
    let op = ctx.op(op_id);
    let lhs = ctx.value_type(op.operands[0]);
    let rhs = ctx.value_type(op.operands[1]);
    if lhs != rhs || lhs != op.results[0] {
        return Err(ptilo::Error::new(
            "mir.add expects identical operand and result types",
        ));
    }
    Ok(())
}

fn verify_comparison(ctx: &Context, op_id: OpId) -> Result<()> {
    let op = ctx.op(op_id);
    let lhs = ctx.value_type(op.operands[0]);
    let rhs = ctx.value_type(op.operands[1]);
    if lhs != rhs {
        return Err(ptilo::Error::new("mir.lt expects identical operand types"));
    }
    Ok(())
}

fn verify_constant(ctx: &Context, op_id: OpId) -> Result<()> {
    let op = ctx.op(op_id);
    let Some(Attr::Int { ty, .. }) = ctx.attr(op_id, "value") else {
        return Err(ptilo::Error::new("mir.constant missing integer value"));
    };
    if *ty != op.results[0] {
        return Err(ptilo::Error::new(
            "mir.constant value type must match result type",
        ));
    }
    Ok(())
}

fn verify_alloca(ctx: &Context, op: OpId) -> Result<()> {
    let ty = ctx.op(op).results[0];
    if ptr_pointee(ctx, ty).is_none() {
        return Err(ptilo::Error::new("mir.alloca result must be mir.ptr"));
    }
    Ok(())
}

fn verify_load(ctx: &Context, op: OpId) -> Result<()> {
    let op = ctx.op(op);
    let ptr_ty = ctx.value_type(op.operands[0]);
    let Some(pointee) = ptr_pointee(ctx, ptr_ty) else {
        return Err(ptilo::Error::new("mir.load operand must be mir.ptr"));
    };
    if pointee != op.results[0] {
        return Err(ptilo::Error::new(
            "mir.load result type must match pointer pointee",
        ));
    }
    Ok(())
}

fn verify_store(ctx: &Context, op: OpId) -> Result<()> {
    let op = ctx.op(op);
    let value_ty = ctx.value_type(op.operands[0]);
    let ptr_ty = ctx.value_type(op.operands[1]);
    let Some(pointee) = ptr_pointee(ctx, ptr_ty) else {
        return Err(ptilo::Error::new("mir.store pointer must be mir.ptr"));
    };
    if value_ty != pointee {
        return Err(ptilo::Error::new(
            "mir.store value type must match pointer pointee",
        ));
    }
    Ok(())
}

fn verify_func(ctx: &Context, op_id: OpId) -> Result<()> {
    let op = ctx.op(op_id);
    let Some(Attr::Type(fn_ty)) = ctx.attr(op_id, "type") else {
        return Err(ptilo::Error::new("mir.func missing function type"));
    };
    let (inputs, _) = ctx
        .func_parts(*fn_ty)
        .ok_or_else(|| ptilo::Error::new("mir.func type is not a function type"))?;
    let region = op.regions[0];
    let Some(entry) = ctx.region(region).blocks.first() else {
        return Err(ptilo::Error::new("mir.func body is empty"));
    };
    if ctx.block(*entry).args != inputs {
        return Err(ptilo::Error::new(
            "mir.func entry block arguments do not match function inputs",
        ));
    }
    Ok(())
}

fn ptr_pointee(ctx: &Context, ty: TypeId) -> Option<TypeId> {
    let data = ctx.type_node(ty);
    if ctx.symbol_str(data.name) != "mir.ptr" {
        return None;
    }
    match data.args.first()? {
        TypeArg::Type(pointee) => Some(*pointee),
        _ => None,
    }
}
