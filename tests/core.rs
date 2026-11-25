#[path = "../examples/support/verify.rs"]
mod verify;

use ptilo::{Attr, AttrDict, Context, OpSpec, Signedness, TypeArg, Value};

fn int_attr(ty: ptilo::TypeId, value: u128) -> Attr {
    Attr::Int { ty, bits: value }
}

#[test]
fn generic_roundtrip_function() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let fn_ty = ctx.func_type(&[], &[i32_ty]);

    let sym = ctx.intern("main");
    let module = ctx.create_op("builtin.module").finish();
    let module_id = ctx.add_root(module);
    let module_region = ctx.add_region(module_id);
    let module_block = ctx.new_block(module_region, &[]);

    let func = ctx
        .create_op("mir.func")
        .attr("sym_name", Attr::Symbol(sym))
        .attr("type", Attr::Type(fn_ty))
        .finish();
    let func_id = ctx.append_op(module_block, func);
    let func_region = ctx.add_region(func_id);
    let entry = ctx.new_block(func_region, &[]);

    let c1 = ctx
        .create_op("mir.constant")
        .attr("value", int_attr(i32_ty, 1))
        .result(i32_ty)
        .finish();
    let c1 = ctx.append_op(entry, c1);
    let c2 = ctx
        .create_op("mir.constant")
        .attr("value", int_attr(i32_ty, 2))
        .result(i32_ty)
        .finish();
    let c2 = ctx.append_op(entry, c2);

    let add = ctx
        .create_op("mir.add")
        .operands([Value::op_result(c1, 0), Value::op_result(c2, 0)])
        .result(i32_ty)
        .finish();
    let add = ctx.append_op(entry, add);
    let ret = ctx
        .create_op("mir.return")
        .operand(Value::op_result(add, 0))
        .finish();
    ctx.append_op(entry, ret);

    verify::all(&ctx).unwrap();
    let text = ctx.to_string();
    assert!(text.contains("\"mir.constant\"()"));
    assert!(text.contains("{value = 1 : i32}"));

    let parsed = Context::parse(&text).unwrap();
    verify::all(&parsed).unwrap();
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn two_regions_and_successors_roundtrip() {
    let mut ctx = Context::new();
    let i1_ty = ctx.int_type(1, Signedness::Signless);
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let module = ctx.create_op("builtin.module").finish();
    let root = ctx.add_root(module);
    let body_region = ctx.add_region(root);
    let entry = ctx.new_block(body_region, &[]);

    let cond = ctx.create_op("test.cond").result(i1_ty).finish();
    let cond = ctx.append_op(entry, cond);
    let cond_v = Value::op_result(cond, 0);

    let then_block = ctx.new_block(body_region, &[]);
    let exit = ctx.new_block(body_region, &[]);

    let br = ctx
        .create_op("test.cond_br")
        .operand(cond_v)
        .successor(then_block)
        .successor(exit)
        .finish();
    ctx.append_op(entry, br);

    let then_region = ctx.new_region();
    let then_region_block = ctx.new_block(then_region, &[]);
    let y1 = ctx
        .create_op("test.yield")
        .attr("value", int_attr(i32_ty, 1))
        .finish();
    ctx.append_op(then_region_block, y1);

    let else_region = ctx.new_region();
    let else_region_block = ctx.new_block(else_region, &[]);
    let y2 = ctx
        .create_op("test.yield")
        .attr("value", int_attr(i32_ty, 2))
        .finish();
    ctx.append_op(else_region_block, y2);

    let if_op = ctx
        .create_op("test.if")
        .operand(cond_v)
        .result(i32_ty)
        .region(then_region)
        .region(else_region)
        .finish();
    let if_op = ctx.append_op(entry, if_op);

    let ret = ctx
        .create_op("test.return")
        .operand(Value::op_result(if_op, 0))
        .finish();
    ctx.append_op(exit, ret);

    verify::all(&ctx).unwrap();
    let text = ctx.to_string();
    assert!(text.contains("({"), "{text}");
    assert!(text.contains("}, {"), "{text}");
    assert!(text.contains("[^bb"), "{text}");

    let parsed = Context::parse(&text).unwrap();
    verify::all(&parsed).unwrap();
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn attribute_shapes_roundtrip() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let f32_ty = ctx.float_type(32);

    let mut dict = AttrDict::new();
    let key = ctx.intern("inner");
    dict.set(key, Attr::Bool(true));

    let hello = ctx.intern("hello");
    let sym_main = ctx.intern("main");
    let kind = ctx.intern("test.kind");
    let sym_a = ctx.intern("A");
    let op = ctx
        .create_op("test.attrs")
        .attr("unit", Attr::Unit)
        .attr("flag", Attr::Bool(false))
        .attr("text", Attr::Str(hello))
        .attr("sym", Attr::Symbol(sym_main))
        .attr("ty", Attr::Type(i32_ty))
        .attr(
            "ints",
            Attr::Array(vec![
                Attr::Int {
                    ty: i32_ty,
                    bits: 1,
                },
                Attr::Int {
                    ty: i32_ty,
                    bits: 2,
                },
            ]),
        )
        .attr(
            "f",
            Attr::Float {
                ty: f32_ty,
                bits: 1.5f32.to_bits() as u64,
            },
        )
        .attr("dict", Attr::Dict(dict))
        .attr(
            "opaque",
            Attr::Opaque {
                name: kind,
                args: vec![Attr::Symbol(sym_a)],
            },
        )
        .finish();
    ctx.add_root(op);

    verify::all(&ctx).unwrap();
    let text = ctx.to_string();
    let parsed = Context::parse(&text).unwrap();
    verify::all(&parsed).unwrap();
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn opaque_and_function_types_roundtrip() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let f64_ty = ctx.float_type(64);

    let field = ctx.intern("x");
    let record = ctx.opaque_type(
        "test.record",
        vec![TypeArg::Field(field, i32_ty), TypeArg::Type(f64_ty)],
    );
    let func = ctx.func_type(&[i32_ty, f64_ty], &[record]);
    let op = ctx
        .create_op("test.types")
        .attr("record", Attr::Type(record))
        .attr("func", Attr::Type(func))
        .finish();
    ctx.add_root(op);

    verify::all(&ctx).unwrap();
    let text = ctx.to_string();
    assert!(text.contains("!test.record<x: i32, f64>"), "{text}");
    let parsed = Context::parse(&text).unwrap();
    verify::all(&parsed).unwrap();
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn parse_generic_text() {
    let text = r#"
%0 = "arith.constant"() {value = 42 : i32} : () -> i32
"test.use"(%0) : (i32) -> ()
"#;
    let ctx = Context::parse(text).unwrap();
    verify::all(&ctx).unwrap();
    let printed = ctx.to_string();
    let reparsed = Context::parse(&printed).unwrap();
    assert_eq!(reparsed.to_string(), printed);
}

#[test]
fn verify_rejects_bad_operand() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let op = ctx.create_op("test.op").result(i32_ty).finish();
    let op = ctx.add_root(op);

    let use_op = ctx
        .create_op("test.use")
        .operand(Value::op_result(op, 9))
        .finish();
    ctx.add_root(use_op);

    assert!(verify::all(&ctx).is_err());
}

#[test]
fn signed_and_unsigned_int_attrs_roundtrip() {
    let mut ctx = Context::new();
    let i32_ty = ctx.int_type(32, Signedness::Signless);
    let u32_ty = ctx.int_type(32, Signedness::Unsigned);
    let op = ctx
        .create_op("test.consts")
        .attr(
            "signed",
            Attr::Int {
                ty: i32_ty,
                bits: (-1i128) as u128,
            },
        )
        .attr(
            "unsigned",
            Attr::Int {
                ty: u32_ty,
                bits: 42,
            },
        )
        .finish();
    ctx.add_root(op);

    verify::all(&ctx).unwrap();
    let text = ctx.to_string();
    assert!(text.contains("signed = -1 : i32"), "{text}");
    assert!(text.contains("unsigned = 42 : ui32"), "{text}");
    let parsed = Context::parse(&text).unwrap();
    verify::all(&parsed).unwrap();
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn arena_reclaims_slots_and_rejects_stale_ids() {
    let mut ctx = Context::new();
    let module_op = ctx.create_op("builtin.module").finish();
    let module = ctx.add_root(module_op);
    let region = ctx.add_region(module);
    let block = ctx.new_block(region, &[]);

    let (a_op, b_op) = (
        ctx.create_op("test.a").finish(),
        ctx.create_op("test.b").finish(),
    );
    let a = ctx.append_op(block, a_op);
    let b = ctx.append_op(block, b_op);
    assert_eq!(ctx.op_count(), 3);

    ctx.erase_op(a);
    assert_eq!(ctx.op_count(), 2);
    assert!(ctx.try_op(a).is_none(), "erased id must not resolve");
    assert_eq!(ctx.block(block).ops, vec![b]);

    // The freed slot is handed back out with a new generation, so the stale
    // handle stays invalid instead of silently aliasing the new operation.
    let c_op = ctx.create_op("test.c").finish();
    let c = ctx.append_op(block, c_op);
    assert_eq!(c.index(), a.index());
    assert_ne!(c, a);
    assert!(ctx.try_op(a).is_none());
    assert!(ctx.is_op(c, "test.c"));

    // Erasing a container reclaims its regions, blocks and nested operations.
    ctx.erase_op(module);
    assert_eq!(ctx.op_count(), 0);
    assert_eq!(ctx.iter_blocks().count(), 0);
    assert_eq!(ctx.iter_regions().count(), 0);
    assert!(ctx.roots().is_empty());
}

#[test]
fn attaching_an_op_moves_it() {
    let mut ctx = Context::new();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let first = ctx.new_block(region, &[]);
    let second = ctx.new_block(region, &[]);

    let op = ctx.create_op("test.c").finish();
    let id = ctx.append_op(first, op);
    ctx.attach_op(second, id);

    assert!(ctx.block(first).ops.is_empty());
    assert_eq!(ctx.block(second).ops, vec![id]);
    assert_eq!(ctx.op(id).parent, Some(second));

    // Pushing a block-attached op to the roots detaches it from the block.
    ctx.push_root(id);
    assert!(ctx.block(second).ops.is_empty());
    assert_eq!(ctx.roots(), &[root, id]);
    // ...and pushing it again does not duplicate the root entry.
    ctx.push_root(id);
    assert_eq!(ctx.roots(), &[root, id]);

    ctx.verify().unwrap();
}

#[test]
fn verify_rejects_orphan_operations() {
    let mut ctx = Context::new();
    let doomed = ctx.create_op("test.orphan").finish();
    let id = ctx.alloc_op(doomed);
    let err = ctx
        .verify()
        .expect_err("an orphan always fails verification");
    assert!(
        err.message.contains("neither in a block nor a root"),
        "{err}"
    );

    ctx.erase_op(id);
    ctx.verify().unwrap();
}

#[test]
fn verify_rejects_broken_back_links() {
    let mut ctx = Context::new();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);
    let op = ctx.create_op("test.c").finish();
    let id = ctx.append_op(block, op);
    ctx.verify().unwrap();

    // Corrupt the link the way a hand-written transform could: point the op at
    // a block that does not list it.
    let stray_region = ctx.new_region();
    let stray_block = ctx.new_block(stray_region, &[]);
    ctx.op_mut(id).parent = Some(stray_block);
    let err = ctx
        .verify()
        .expect_err("a broken parent link must be caught");
    assert!(err.message.contains("parent"), "{err}");
}

#[test]
fn erase_op_reclaims_the_whole_subtree() {
    let mut ctx = Context::new();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);

    let inner_region = ctx.new_region();
    let inner_block = ctx.new_block(inner_region, &[]);
    let inner_op = ctx.create_op("test.deep").finish();
    ctx.append_op(inner_block, inner_op);
    let outer = ctx.create_op("test.outer").region(inner_region).finish();
    let outer_id = ctx.append_op(block, outer);
    assert_eq!(ctx.op_count(), 3);

    ctx.erase_op(outer_id);
    assert_eq!(ctx.op_count(), 1);
    assert_eq!(ctx.iter_blocks().count(), 1);
    assert_eq!(ctx.iter_regions().count(), 1);
    ctx.verify().unwrap();
}

#[test]
#[should_panic(expected = "declares 0 regions")]
fn adding_an_undeclared_region_panics() {
    static LEAF: OpSpec = OpSpec {
        name: "test.leaf",
        operands: &[],
        results: &[],
        regions: &[],
        attrs: &[],
        terminator: false,
        verify: None,
    };

    let mut ctx = Context::new();
    let op = ctx.create_op_spec(&LEAF).finish();
    let id = ctx.add_root(op);
    ctx.add_region(id);
}

#[test]
fn verify_rejects_operations_with_the_wrong_region_count() {
    static BODY: OpSpec = OpSpec {
        name: "test.body",
        operands: &[],
        results: &[],
        regions: &[ptilo::region("body")],
        attrs: &[],
        terminator: false,
        verify: None,
    };

    let mut ctx = Context::new();
    let op = ctx.create_op_spec(&BODY).finish();
    let id = ctx.add_root(op);
    let err = ctx.verify().expect_err("a missing region must be caught");
    assert!(err.message.contains("expects 1 regions"), "{err}");
    ctx.add_region(id);
    ctx.verify().unwrap();
}
