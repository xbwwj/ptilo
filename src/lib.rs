use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Write as _};

/// An operation.
///
/// SSA values are represented by plain `usize` IDs. Result types are stored
/// directly as strings; there is no Type or Value wrapper.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Op {
    pub name: String,
    pub operands: Vec<usize>,
    pub results: Vec<(usize, String)>,
    pub attrs: BTreeMap<String, String>,
    pub regions: Vec<Region>,
}

/// A region is an ordered list of blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Region {
    pub blocks: Vec<Block>,
}

/// A block is an ordered list of operations with typed SSA arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub args: Vec<(usize, String)>,
    pub ops: Vec<Op>,
}

impl Op {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn operands(mut self, values: impl IntoIterator<Item = usize>) -> Self {
        self.operands.extend(values);
        self
    }

    /// Append result definitions as `(value_id, type_name)` pairs.
    pub fn results<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (usize, S)>,
        S: Into<String>,
    {
        self.results
            .extend(values.into_iter().map(|(id, ty)| (id, ty.into())));
        self
    }

    pub fn result(mut self, id: usize, ty: impl Into<String>) -> Self {
        self.results.push((id, ty.into()));
        self
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name.into(), value.into());
        self
    }

    pub fn with_region(mut self, region: Region) -> Self {
        self.regions.push(region);
        self
    }

    pub fn push_region(&mut self, region: Region) -> &mut Self {
        self.regions.push(region);
        self
    }
}

impl Region {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_block(mut self, block: Block) -> Self {
        self.blocks.push(block);
        self
    }

    pub fn push_block(&mut self, block: Block) -> &mut Self {
        self.blocks.push(block);
        self
    }
}

impl Block {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn arg(mut self, id: usize, ty: impl Into<String>) -> Self {
        self.args.push((id, ty.into()));
        self
    }

    pub fn push_arg(&mut self, id: usize, ty: impl Into<String>) -> &mut Self {
        self.args.push((id, ty.into()));
        self
    }

    pub fn push(&mut self, op: Op) -> &mut Op {
        self.ops.push(op);
        self.ops.last_mut().expect("the operation was just pushed")
    }
}

/// Walk all operations in preorder, including `root` itself.
pub fn walk_ops(root: &Op, visitor: &mut impl FnMut(&Op)) {
    visitor(root);

    for region in &root.regions {
        for block in &region.blocks {
            for op in &block.ops {
                walk_ops(op, visitor);
            }
        }
    }
}

/// Mutable counterpart of [`walk_ops`], also in preorder.
pub fn walk_ops_mut(root: &mut Op, visitor: &mut impl FnMut(&mut Op)) {
    visitor(root);

    for region in &mut root.regions {
        for block in &mut region.blocks {
            for op in &mut block.ops {
                walk_ops_mut(op, visitor);
            }
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_op(self, f, 0, &mut HashMap::new())
    }
}

fn indent(f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    for _ in 0..depth {
        f.write_str("  ")?;
    }
    Ok(())
}

fn fmt_result_type_suffix(op: &Op, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if op.results.is_empty() {
        return Ok(());
    }

    f.write_str(" : ")?;

    if op.results.len() > 1 {
        f.write_str("(")?;
    }

    for (index, (_, ty)) in op.results.iter().enumerate() {
        if index != 0 {
            f.write_str(", ")?;
        }
        f.write_str(ty)?;
    }

    if op.results.len() > 1 {
        f.write_str(")")?;
    }

    Ok(())
}

/// For an operation without results (for example `func.return`), print the
/// operand types after `:`. The type map is populated from block arguments
/// and preceding operation results while printing the containing block.
fn fmt_operand_type_suffix(
    op: &Op,
    f: &mut fmt::Formatter<'_>,
    value_types: &HashMap<usize, String>,
) -> fmt::Result {
    if op.results.is_empty() && !op.operands.is_empty() {
        f.write_str(" : ")?;

        if op.operands.len() > 1 {
            f.write_str("(")?;
        }

        for (index, id) in op.operands.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            f.write_str(value_types.get(id).map(String::as_str).unwrap_or("?"))?;
        }

        if op.operands.len() > 1 {
            f.write_str(")")?;
        }
    }

    Ok(())
}

fn fmt_op(
    op: &Op,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
    value_types: &mut HashMap<usize, String>,
) -> fmt::Result {
    indent(f, depth)?;

    if !op.results.is_empty() {
        for (index, (id, _)) in op.results.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            write!(f, "%{id}")?;
        }
        f.write_str(" = ")?;
    }

    write!(f, "{}(", op.name)?;

    for (index, operand) in op.operands.iter().enumerate() {
        if index != 0 {
            f.write_str(", ")?;
        }
        write!(f, "%{operand}")?;
    }

    f.write_str(")")?;

    if !op.attrs.is_empty() {
        f.write_str(" {")?;

        for (index, (name, value)) in op.attrs.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{name} = {value:?}")?;
        }

        f.write_str("}")?;
    }

    if !op.regions.is_empty() {
        f.write_str(" {\n")?;

        for region in &op.regions {
            fmt_region(region, f, depth + 1, value_types)?;
        }

        indent(f, depth)?;
        f.write_str("}")?;
    }

    if !op.results.is_empty() {
        fmt_result_type_suffix(op, f)?;
    } else {
        fmt_operand_type_suffix(op, f, value_types)?;
    }

    f.write_str("\n")?;

    // Results become visible to subsequent operations in the containing block.
    for (id, ty) in &op.results {
        value_types.insert(*id, ty.clone());
    }

    Ok(())
}

fn fmt_region(
    region: &Region,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
    outer_value_types: &HashMap<usize, String>,
) -> fmt::Result {
    for (block_index, block) in region.blocks.iter().enumerate() {
        // Each block starts with the enclosing values, but does not inherit
        // definitions from sibling blocks.
        let mut value_types = outer_value_types.clone();

        indent(f, depth)?;
        write!(f, "^bb{block_index}")?;

        if !block.args.is_empty() {
            f.write_str("(")?;

            for (index, (id, ty)) in block.args.iter().enumerate() {
                if index != 0 {
                    f.write_str(", ")?;
                }
                write!(f, "%{id}: {ty}")?;
                value_types.insert(*id, ty.clone());
            }

            f.write_str(")")?;
        }

        f.write_str(":\n")?;

        for op in &block.ops {
            fmt_op(op, f, depth + 1, &mut value_types)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_result_types_after_operations() {
        let mut block = Block::new();
        block.push(
            Op::new("arith.constant")
                .result(0, "i32")
                .attr("value", "40"),
        );
        block.push(
            Op::new("arith.constant")
                .result(1, "i32")
                .attr("value", "2"),
        );
        block.push(Op::new("arith.addi").operands([0, 1]).result(2, "i32"));
        block.push(Op::new("func.return").operands([2]));

        let op = Op::new("builtin.module").with_region(Region::new().with_block(block));
        let text = op.to_string();

        assert!(text.contains("%0 = arith.constant() {value = \"40\"} : i32"));
        assert!(text.contains("%2 = arith.addi(%0, %1) : i32"));
        assert!(text.contains("func.return(%2) : i32"));
    }

    #[test]
    fn prints_typed_block_arguments() {
        let block = Block::new().arg(3, "i32").arg(4, "i64");
        let op = Op::new("test.region").with_region(Region::new().with_block(block));

        assert!(op.to_string().contains("^bb0(%3: i32, %4: i64):"));
    }

    #[test]
    fn walks_nested_operations_in_preorder() {
        let inner = Op::new("test.inner").result(7, "i32");
        let outer = Op::new("test.outer").with_region(Region::new().with_block({
            let mut block = Block::new().arg(6, "i32");
            block.push(inner);
            block
        }));
        let root = Op::new("builtin.module").with_region(Region::new().with_block({
            let mut block = Block::new();
            block.push(outer);
            block
        }));

        let mut names = Vec::new();
        walk_ops(&root, &mut |op| names.push(op.name.as_str()));

        assert_eq!(names, vec!["builtin.module", "test.outer", "test.inner"]);
    }
}
