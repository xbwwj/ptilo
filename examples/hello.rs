use ptilo::{Block, Op, Region, walk_ops};

fn main() {
    // main(): 40 + 2
    let mut main_entry = Block::new();

    main_entry.push(
        Op::new("arith.constant")
            .result(0, "i32")
            .attr("value", "40"),
    );

    main_entry.push(
        Op::new("arith.constant")
            .result(1, "i32")
            .attr("value", "2"),
    );

    main_entry.push(Op::new("arith.addi").operands([0, 1]).result(2, "i32"));

    main_entry.push(Op::new("func.return").operands([2]));

    let main = Op::new("func.func")
        .attr("sym_name", "main")
        .with_region(Region::new().with_block(main_entry));

    // add(%3: i32, %4: i32): i32
    let mut add_entry = Block::new().arg(3, "i32").arg(4, "i32");

    add_entry.push(Op::new("arith.addi").operands([3, 4]).result(5, "i32"));

    add_entry.push(Op::new("func.return").operands([5]));

    let add = Op::new("func.func")
        .attr("sym_name", "add")
        .with_region(Region::new().with_block(add_entry));

    // Module is an ordinary operation, not a separate Module type.
    let mut module_body = Block::new();
    module_body.push(main);
    module_body.push(add);

    let module = Op::new("builtin.module").with_region(Region::new().with_block(module_body));

    println!("{module}");

    println!("Operations:");
    walk_ops(&module, &mut |op| println!("  {}", op.name));
}
