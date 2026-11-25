//! Text round-trip and parser diagnostics.

use ptilo::{Attr, AttrDict, Context, Signedness, TypeArg, TypeNode};

fn type_attrs(ctx: &mut Context, pairs: &[(&str, i128)]) -> AttrDict {
    let ty = ctx.int_type(32, Signedness::Signless);
    let mut dict = AttrDict::new();
    for (name, value) in pairs {
        let key = ctx.intern(name);
        dict.set(key, Attr::int(ty, *value as u128));
    }
    dict
}

#[test]
fn quoted_names_roundtrip() {
    let mut ctx = Context::new();
    // A type name, a type argument and an opaque attribute name that all need
    // quoting, because none of them is a bare identifier.
    let arg = ctx.intern("a b");
    let ty = ctx.opaque_type("weird type", vec![TypeArg::Str(arg)]);
    let name = ctx.intern("odd attr");
    let text_arg = ctx.intern("line\nbreak\0");
    let opaque = Attr::Opaque {
        name,
        args: vec![Attr::string(text_arg)],
    };
    let op = ctx
        .create_op("odd op")
        .result(ty)
        .attr("odd key", opaque)
        .finish();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);
    ctx.append_op(block, op);

    let text = ctx.to_string();
    let parsed = Context::parse(&text).unwrap_or_else(|err| panic!("{err}\n{text}"));
    assert_eq!(parsed.to_string(), text);
}

#[test]
fn type_attributes_survive_a_roundtrip() {
    let mut ctx = Context::new();
    let name = ctx.intern("my.type");
    let attrs = type_attrs(&mut ctx, &[("tag", 7)]);
    let with_attrs = ctx.intern_type(TypeNode {
        name,
        args: vec![],
        attrs,
    });
    let plain = ctx.intern_type(TypeNode {
        name,
        args: vec![],
        attrs: AttrDict::new(),
    });
    assert_ne!(with_attrs, plain);

    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);
    let first = ctx.create_op("test.a").result(with_attrs).finish();
    let second = ctx.create_op("test.a").result(plain).finish();
    ctx.append_op(block, first);
    ctx.append_op(block, second);

    let text = ctx.to_string();
    let parsed = Context::parse(&text).unwrap_or_else(|err| panic!("{err}\n{text}"));
    assert_eq!(parsed.to_string(), text);

    // The two types stay distinct instead of collapsing into one.
    let ops = parsed.walk_all_ops();
    let types: Vec<_> = ops
        .iter()
        .flat_map(|op| parsed.op(*op).results.clone())
        .collect();
    assert_eq!(types.len(), 2);
    assert_ne!(types[0], types[1]);
    assert!(!parsed.type_node(types[0]).attrs.is_empty());
    assert!(parsed.type_node(types[1]).attrs.is_empty());
}

#[test]
fn attribute_dict_ignores_insertion_order() {
    let mut ctx = Context::new();
    let forward = type_attrs(&mut ctx, &[("a", 1), ("b", 2)]);
    let backward = type_attrs(&mut ctx, &[("b", 2), ("a", 1)]);
    assert_eq!(forward, backward);

    let name = ctx.intern("tagged");
    let first = ctx.intern_type(TypeNode {
        name,
        args: vec![],
        attrs: forward,
    });
    let second = ctx.intern_type(TypeNode {
        name,
        args: vec![],
        attrs: backward,
    });
    assert_eq!(first, second, "attribute order must not fork the interner");
}

#[test]
fn duplicate_attribute_keys_are_rejected() {
    let err = Context::parse(r#""test.c"() {a = 1 : i32, a = 2 : i32} : () -> ()"#)
        .expect_err("duplicate attribute name must be an error");
    assert!(err.message.contains("duplicate attribute"), "{err}");
}

#[test]
fn integer_literals_must_fit_their_type() {
    assert!(Context::parse(r#""test.c"() {v = 127 : i8} : () -> ()"#).is_ok());
    assert!(Context::parse(r#""test.c"() {v = 255 : i8} : () -> ()"#).is_ok());
    assert!(Context::parse(r#""test.c"() {v = -128 : i8} : () -> ()"#).is_ok());
    let err = Context::parse(r#""test.c"() {v = 300 : i8} : () -> ()"#)
        .expect_err("out of range literal must be an error");
    assert!(err.message.contains("does not fit"), "{err}");
    // i128::MIN used to fail because the magnitude was parsed first.
    assert!(
        Context::parse(
            r#""test.c"() {v = -170141183460469231731687303715884105728 : i128} : () -> ()"#
        )
        .is_ok()
    );
}

#[test]
fn parse_into_rolls_back_on_error() {
    let mut ctx = Context::new();
    let keep_op = ctx.create_op("test.keep").finish();
    let keep = ctx.add_root(keep_op);
    let before = (
        ctx.op_count(),
        ctx.iter_blocks().count(),
        ctx.iter_regions().count(),
        ctx.roots().len(),
    );

    let bad = r#"
"test.wrap"() ({
  ^bb0:
    %0 = "test.c"() : () -> i32
    "test.use"(%missing) : (i32) -> ()
}) : () -> ()
"#;
    let err = ctx
        .parse_into(bad)
        .expect_err("undefined value must be an error");
    assert!(err.message.contains("undefined SSA value"), "{err}");
    assert_eq!(
        before,
        (
            ctx.op_count(),
            ctx.iter_blocks().count(),
            ctx.iter_regions().count(),
            ctx.roots().len(),
        ),
        "a failed parse must not leave partial IR behind"
    );
    assert!(ctx.try_op(keep).is_some());
}

#[test]
fn errors_carry_line_and_column() {
    let err = Context::parse("// comment\n\n\"test.c\"() {@nope} : () -> ()").unwrap_err();
    assert!(err.message.contains("3:"), "{err}");
}

#[test]
fn print_refuses_unrepresentable_ir() {
    let mut ctx = Context::new();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);

    // A successor in a different (unreachable) region cannot be given a name.
    let stray_region = ctx.new_region();
    let stray_block = ctx.new_block(stray_region, &[]);
    let br = ctx.create_op("test.br").successor(stray_block).finish();
    ctx.append_op(block, br);

    let err = ptilo::print(&ctx).expect_err("printing must not emit `^?`");
    assert!(err.message.contains("not part of any region"), "{err}");
    assert!(ctx.to_string().starts_with("<invalid IR:"));
}

#[test]
fn lexer_accepts_tight_arrow_and_comments() {
    let text = r#"
// a tight function type, no spaces around the arrow
"test.c"() {t = (i32)->i32} : () -> () // trailing comment
"#;
    let ctx = Context::parse(text).unwrap_or_else(|err| panic!("{err}"));
    let op = ctx.roots()[0];
    let (inputs, results) = ctx
        .func_parts(match ctx.attr(op, "t") {
            Some(Attr::Type(ty)) => *ty,
            other => panic!("expected a type attribute, found {other:?}"),
        })
        .expect("function type");
    assert_eq!((inputs.len(), results.len()), (1, 1));
}

#[test]
fn negative_and_float_literals() {
    let mut ctx =
        Context::parse(r#""test.c"() {i = -42 : i64, f = -1.5e3 : f64, g = 2.5 : f32} : () -> ()"#)
            .unwrap_or_else(|err| panic!("{err}"));
    let op = ctx.roots()[0];
    assert_eq!(ctx.op(op).attrs.len(), 3);
    let i64ty = ctx.int_type(64, Signedness::Signless);
    // Literals are stored as their two's complement bit pattern.
    let expected = Attr::int(i64ty, (1u128 << 64) - 42);
    let value = ctx.attr(op, "i").expect("attribute i").clone();
    assert_eq!(value, expected);
}

#[test]
fn block_arguments_stay_visible_to_later_blocks() {
    // MLIR's parser keeps one scope per region, so a value defined in an
    // earlier block may be referenced later; dominance is checked separately.
    let text = r#"
"test.m"() ({
  ^bb0(%a: i32):
    "test.br"() [^bb1] : () -> ()
  ^bb1(%b: i32):
    "test.use"(%a, %b) : (i32, i32) -> ()
}) : () -> ()
"#;
    let ctx = Context::parse(text).unwrap_or_else(|err| panic!("{err}"));
    ctx.verify().unwrap();
    // Printing renumbers values, so compare against the renumbered form.
    let printed = ctx.to_string();
    assert!(printed.contains("^bb0(%0: i32)"), "{printed}");
    assert!(printed.contains("^bb1(%1: i32)"), "{printed}");
    assert!(printed.contains(r#""test.use"(%0, %1)"#), "{printed}");
    let reparse = Context::parse(&printed).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(reparse.to_string(), printed);
}

#[test]
fn rollback_restores_recycled_slots() {
    let mut ctx = Context::new();
    let module = ctx.create_op("test.m").finish();
    let root = ctx.add_root(module);
    let region = ctx.add_region(root);
    let block = ctx.new_block(region, &[]);

    // Free a slot so that the failed parse has a hole to recycle.
    let doomed = ctx.create_op("test.doomed").finish();
    let doomed_id = ctx.append_op(block, doomed);
    let survivor = ctx.create_op("test.survivor").finish();
    let survivor_id = ctx.append_op(block, survivor);
    ctx.erase_op(doomed_id);

    let before = ctx.to_string();
    let err = ctx
        .parse_into(r#""test.wrap"() ({ ^bb0: "test.use"(%nope) : (i32) -> () }) : () -> ()"#)
        .expect_err("undefined value must be an error");
    assert!(err.message.contains("undefined SSA value"), "{err}");

    assert_eq!(ctx.to_string(), before, "rollback must restore the context");
    assert_eq!(ctx.op_count(), 2);
    ctx.verify().unwrap();

    // The freed slot has to be reusable again, with a bumped generation.
    let extra = ctx.create_op("test.extra").finish();
    let extra_id = ctx.append_op(block, extra);
    assert_eq!(extra_id.index(), doomed_id.index());
    assert_ne!(extra_id, doomed_id);
    assert!(ctx.try_op(survivor_id).is_some());
    ctx.verify().unwrap();
}
