#[path = "support/jsir.rs"]
mod jsir;
#[path = "support/verify.rs"]
mod verify;

use jsir::JsirInsert as _;
use ptilo::Context;

fn main() -> ptilo::Result<()> {
    let mut ctx = Context::new();

    let any_ty = jsir::any_type(&mut ctx);
    let f64_ty = ctx.float_type(64);

    let (_, file_block) = jsir::file(&mut ctx);
    let (_, entry) = file_block.insert_function(&mut ctx, "foo");

    let one = entry.insert_numeric(&mut ctx, any_ty, f64_ty, 1.0);
    let two = entry.insert_numeric(&mut ctx, any_ty, f64_ty, 2.0);
    let sum = entry.insert_binary(&mut ctx, any_ty, "+", one, two);
    entry.insert_expression_statement(&mut ctx, sum);

    let x_ref = entry.insert_identifier_ref(&mut ctx, any_ty, "x");
    let seven = entry.insert_numeric(&mut ctx, any_ty, f64_ty, 7.0);
    let assign = entry.insert_assignment(&mut ctx, any_ty, "=", x_ref, seven);
    entry.insert_expression_statement(&mut ctx, assign);

    let cond = entry.insert_identifier(&mut ctx, any_ty, "cond");

    let then_region = ctx.new_region();
    let then_block = ctx.new_block(then_region, &[]);
    let a = then_block.insert_identifier(&mut ctx, any_ty, "a");
    then_block.insert_expression_statement(&mut ctx, a);

    let else_region = ctx.new_region();
    let else_block = ctx.new_block(else_region, &[]);
    let b = else_block.insert_identifier(&mut ctx, any_ty, "b");
    else_block.insert_expression_statement(&mut ctx, b);

    entry.insert_if(&mut ctx, cond, then_region, Some(else_region));

    verify::all(&ctx)?;
    println!("{ctx}");

    let text = ctx.to_string();
    let parsed = Context::parse(&text)?;
    verify::all(&parsed)?;
    assert_eq!(parsed.to_string(), text);
    Ok(())
}
