//! Example-level verification: structural checks live in `Context::verify`,
//! this only adds the rules the examples' dialects care about.

use ptilo::{Context, Error, Result};

pub fn all(ctx: &Context) -> Result<()> {
    ctx.verify()?;

    for (op_id, op) in ctx.iter_ops() {
        if let Some(spec) = op.spec {
            if !spec.terminator && !op.successors.is_empty() {
                return Err(Error::new(format!(
                    "{} is not marked as a terminator but has successors",
                    spec.name
                )));
            }
            for (index, region_spec) in spec.regions.iter().enumerate() {
                if let Some(region_id) = op.regions.get(index)
                    && region_spec.single_block
                    && ctx.region(*region_id).blocks.len() > 1
                {
                    return Err(Error::new(format!(
                        "region {index} of {} must be single-block",
                        spec.name
                    )));
                }
            }
        }

        if op.successors.is_empty() {
            continue;
        }
        let parent = op.parent.ok_or_else(|| {
            Error::new(format!(
                "operation {op_id:?} has successors but no parent block"
            ))
        })?;
        let region = ctx.block(parent).parent;
        for successor in &op.successors {
            let block = ctx.try_block(*successor).ok_or_else(|| {
                Error::new(format!(
                    "operation {op_id:?} has dangling successor {successor:?}"
                ))
            })?;
            if block.parent != region {
                return Err(Error::new(format!(
                    "operation {op_id:?} has successor {successor:?} outside its region"
                )));
            }
        }
    }

    Ok(())
}
