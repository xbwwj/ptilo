#[path = "support/ts.rs"]
mod ts;
#[path = "support/verify.rs"]
mod verify;

use ptilo::{Attr, Context};

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let any_ty = ts::atom(&mut ctx, "ts.any");
    let unknown_ty = ts::atom(&mut ctx, "ts.unknown");
    let never_ty = ts::atom(&mut ctx, "ts.never");
    let string_ty = ts::atom(&mut ctx, "ts.string");
    let number_ty = ts::atom(&mut ctx, "ts.number");
    let boolean_ty = ts::atom(&mut ctx, "ts.boolean");

    let array_string = ts::array(&mut ctx, string_ty);
    let named_member = ts::named_tuple_member(&mut ctx, "first", string_ty, false);
    let optional_number = ts::optional(&mut ctx, number_ty);
    let rest_strings = ts::rest(&mut ctx, array_string);
    let tuple = ts::tuple(&mut ctx, &[named_member, optional_number, rest_strings]);

    let union = ts::union(&mut ctx, &[string_ty, number_ty]);
    let empty_record = ts::record(&mut ctx, &[]);
    let intersection = ts::intersection(&mut ctx, &[union, empty_record]);
    let record = ts::record(&mut ctx, &[("name", string_ty), ("age", optional_number)]);
    let fn_ty = ts::fn_type(&mut ctx, &[string_ty, number_ty], boolean_ty);
    let type_ref = ts::type_reference(&mut ctx, "Foo", &[string_ty]);
    let keyof = ts::keyof(&mut ctx, record);
    let literal_age = ts::literal_string(&mut ctx, "age");
    let indexed = ts::indexed_access(&mut ctx, record, literal_age);
    let conditional = ts::conditional(&mut ctx, string_ty, number_ty, boolean_ty, never_ty);

    let alias_name = ctx.intern("Foo");
    let alias = ctx
        .create_op_spec(&ts::TYPE_ALIAS)
        .attr("name", Attr::symbol(alias_name))
        .attr("type", Attr::ty(union))
        .finish();
    ctx.add_root(alias);

    let op = ctx
        .create_op("test.ts_types")
        .attr("any", Attr::ty(any_ty))
        .attr("unknown", Attr::ty(unknown_ty))
        .attr("array", Attr::ty(array_string))
        .attr("tuple", Attr::ty(tuple))
        .attr("union", Attr::ty(union))
        .attr("intersection", Attr::ty(intersection))
        .attr("record", Attr::ty(record))
        .attr("function", Attr::ty(fn_ty))
        .attr("reference", Attr::ty(type_ref))
        .attr("keyof", Attr::ty(keyof))
        .attr("indexed", Attr::ty(indexed))
        .attr("conditional", Attr::ty(conditional))
        .finish();
    ctx.add_root(op);

    verify::all(&ctx)?;
    println!("{ctx}");

    let text = ctx.to_string();
    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
