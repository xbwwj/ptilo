#![allow(dead_code)]

use ptilo::{
    Attr, AttrConstraint, BlockId, Context, OpId, OpSpec, RegionId, TypeId, Value, attr, opd,
    region, region_optional, res,
};

pub const FILE: OpSpec = OpSpec {
    name: "jsir.file",
    regions: &[region("program")],
    ..OpSpec::DEFAULT
};

pub const FUNCTION_DECLARATION: OpSpec = OpSpec {
    name: "jsir.function_declaration",
    regions: &[region("body")],
    attrs: &[attr("name", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

pub const NUMERIC_LITERAL: OpSpec = OpSpec {
    name: "jsir.numeric_literal",
    results: &[res("value")],
    attrs: &[attr("value", true, AttrConstraint::Float)],
    ..OpSpec::DEFAULT
};

pub const BINARY_EXPRESSION: OpSpec = OpSpec {
    name: "jsir.binary_expression",
    operands: &[opd("left"), opd("right")],
    results: &[res("value")],
    attrs: &[attr("operator", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

pub const EXPRESSION_STATEMENT: OpSpec = OpSpec {
    name: "jsir.expression_statement",
    operands: &[opd("expression")],
    ..OpSpec::DEFAULT
};

pub const IDENTIFIER: OpSpec = OpSpec {
    name: "jsir.identifier",
    results: &[res("value")],
    attrs: &[attr("name", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

pub const IDENTIFIER_REF: OpSpec = OpSpec {
    name: "jsir.identifier_ref",
    results: &[res("value")],
    attrs: &[attr("name", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

pub const ASSIGNMENT_EXPRESSION: OpSpec = OpSpec {
    name: "jsir.assignment_expression",
    operands: &[opd("left"), opd("right")],
    results: &[res("value")],
    attrs: &[attr("operator", true, AttrConstraint::Str)],
    ..OpSpec::DEFAULT
};

pub const IF_STATEMENT: OpSpec = OpSpec {
    name: "jsir.if_statement",
    operands: &[opd("test")],
    regions: &[region("consequent"), region_optional("alternate")],
    ..OpSpec::DEFAULT
};

pub fn any_type(ctx: &mut Context) -> TypeId {
    ctx.opaque_type("jsir.any", vec![])
}

pub fn file(ctx: &mut Context) -> (OpId, BlockId) {
    let op = ctx.create_op_spec(&FILE).finish();
    let op = ctx.add_root(op);
    let region = ctx.add_region(op);
    let block = ctx.new_block(region, &[]);
    (op, block)
}

pub trait JsirInsert {
    fn insert_function(&self, ctx: &mut Context, name: &str) -> (OpId, BlockId);
    fn insert_numeric(
        &self,
        ctx: &mut Context,
        result_ty: TypeId,
        value_ty: TypeId,
        value: f64,
    ) -> Value;
    fn insert_binary(
        &self,
        ctx: &mut Context,
        result_ty: TypeId,
        operator: &str,
        lhs: Value,
        rhs: Value,
    ) -> Value;
    fn insert_expression_statement(&self, ctx: &mut Context, expression: Value);
    fn insert_identifier(&self, ctx: &mut Context, ty: TypeId, name: &str) -> Value;
    fn insert_identifier_ref(&self, ctx: &mut Context, ty: TypeId, name: &str) -> Value;
    fn insert_assignment(
        &self,
        ctx: &mut Context,
        ty: TypeId,
        operator: &str,
        left: Value,
        right: Value,
    ) -> Value;
    fn insert_if(
        &self,
        ctx: &mut Context,
        test: Value,
        consequent: RegionId,
        alternate: Option<RegionId>,
    );
}

impl JsirInsert for BlockId {
    fn insert_function(&self, ctx: &mut Context, name: &str) -> (OpId, BlockId) {
        let name = ctx.intern(name);
        let op = ctx
            .create_op_spec(&FUNCTION_DECLARATION)
            .attr("name", Attr::string(name))
            .finish();
        let op = ctx.append_op(*self, op);
        let region = ctx.add_region(op);
        let block = ctx.new_block(region, &[]);
        (op, block)
    }

    fn insert_numeric(
        &self,
        ctx: &mut Context,
        result_ty: TypeId,
        value_ty: TypeId,
        value: f64,
    ) -> Value {
        let op = ctx
            .create_op_spec(&NUMERIC_LITERAL)
            .attr("value", Attr::float(value_ty, value.to_bits()))
            .result(result_ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_binary(
        &self,
        ctx: &mut Context,
        result_ty: TypeId,
        operator: &str,
        lhs: Value,
        rhs: Value,
    ) -> Value {
        let operator = ctx.intern(operator);
        let op = ctx
            .create_op_spec(&BINARY_EXPRESSION)
            .attr("operator", Attr::string(operator))
            .operands([lhs, rhs])
            .result(result_ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_expression_statement(&self, ctx: &mut Context, expression: Value) {
        let op = ctx
            .create_op_spec(&EXPRESSION_STATEMENT)
            .operand(expression)
            .finish();
        ctx.append_op(*self, op);
    }

    fn insert_identifier(&self, ctx: &mut Context, ty: TypeId, name: &str) -> Value {
        let name = ctx.intern(name);
        let op = ctx
            .create_op_spec(&IDENTIFIER)
            .attr("name", Attr::string(name))
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_identifier_ref(&self, ctx: &mut Context, ty: TypeId, name: &str) -> Value {
        let name = ctx.intern(name);
        let op = ctx
            .create_op_spec(&IDENTIFIER_REF)
            .attr("name", Attr::string(name))
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_assignment(
        &self,
        ctx: &mut Context,
        ty: TypeId,
        operator: &str,
        left: Value,
        right: Value,
    ) -> Value {
        let operator = ctx.intern(operator);
        let op = ctx
            .create_op_spec(&ASSIGNMENT_EXPRESSION)
            .attr("operator", Attr::string(operator))
            .operands([left, right])
            .result(ty)
            .finish();
        let op = ctx.append_op(*self, op);
        Value::op_result(op, 0)
    }

    fn insert_if(
        &self,
        ctx: &mut Context,
        test: Value,
        consequent: RegionId,
        alternate: Option<RegionId>,
    ) {
        let mut builder = ctx
            .create_op_spec(&IF_STATEMENT)
            .operand(test)
            .region(consequent);
        if let Some(alternate) = alternate {
            builder = builder.region(alternate);
        }
        let op = builder.finish();
        ctx.append_op(*self, op);
    }
}
