use std::collections::HashMap;
use std::fmt::Write as _;

use crate::attr::{Attr, AttrDict};
use crate::ir::{BlockId, Context, OpId, RegionId, Value};
use crate::types::{Signedness, TypeArg, TypeId};
use crate::{Error, Result};

/// Render the context in MLIR generic form.
///
/// Fails instead of emitting placeholders when the IR cannot be represented:
/// values or blocks that are not reachable from a root, or attributes whose
/// payload does not match their type.
pub fn print(ctx: &Context) -> Result<String> {
    let mut printer = Printer::new(ctx);
    printer.number_all();
    let mut out = String::new();
    for root in ctx.roots() {
        printer.print_op(*root, 0, &mut out)?;
    }
    Ok(out)
}

struct Printer<'a> {
    ctx: &'a Context,
    value_names: HashMap<Value, usize>,
    block_names: HashMap<BlockId, usize>,
    next_value: usize,
    next_block: usize,
}

impl<'a> Printer<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            value_names: HashMap::new(),
            block_names: HashMap::new(),
            next_value: 0,
            next_block: 0,
        }
    }

    fn number_all(&mut self) {
        for root in self.ctx.roots() {
            self.number_op(*root);
        }
    }

    fn number_op(&mut self, op: OpId) {
        for index in 0..self.ctx.op(op).results.len() {
            self.number_value(Value::op_result(op, index as u32));
        }
        for region in &self.ctx.op(op).regions {
            for block in &self.ctx.region(*region).blocks {
                self.number_block(*block);
                for index in 0..self.ctx.block(*block).args.len() {
                    self.number_value(Value::block_arg(*block, index as u32));
                }
                for child in &self.ctx.block(*block).ops {
                    self.number_op(*child);
                }
            }
        }
    }

    fn number_value(&mut self, value: Value) -> usize {
        if let Some(name) = self.value_names.get(&value) {
            return *name;
        }
        let name = self.next_value;
        self.next_value += 1;
        self.value_names.insert(value, name);
        name
    }

    fn number_block(&mut self, block: BlockId) -> usize {
        if let Some(name) = self.block_names.get(&block) {
            return *name;
        }
        let name = self.next_block;
        self.next_block += 1;
        self.block_names.insert(block, name);
        name
    }

    fn write_indent(&self, out: &mut String, indent: usize) {
        for _ in 0..indent {
            out.push_str("  ");
        }
    }

    fn write_value(&self, out: &mut String, value: Value) -> Result<()> {
        let Some(name) = self.value_names.get(&value) else {
            return Err(Error::new(format!(
                "value {value:?} is not defined anywhere reachable from a root"
            )));
        };
        out.push('%');
        let _ = write!(out, "{name}");
        Ok(())
    }

    fn write_block_name(&self, out: &mut String, block: BlockId) -> Result<()> {
        let Some(name) = self.block_names.get(&block) else {
            return Err(Error::new(format!(
                "block {block:?} is not part of any region reachable from a root"
            )));
        };
        out.push('^');
        let _ = write!(out, "bb{name}");
        Ok(())
    }

    fn print_op(&self, op_id: OpId, indent: usize, out: &mut String) -> Result<()> {
        let op = self.ctx.op(op_id);

        if !op.results.is_empty() {
            for index in 0..op.results.len() {
                if index != 0 {
                    out.push_str(", ");
                }
                self.write_value(out, Value::op_result(op_id, index as u32))?;
            }
            out.push_str(" = ");
        }

        out.push('"');
        escape_into(out, self.ctx.symbol_str(op.name));
        out.push('"');

        out.push('(');
        for (index, operand) in op.operands.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            self.write_value(out, *operand)?;
        }
        out.push(')');

        if !op.successors.is_empty() {
            out.push(' ');
            out.push('[');
            for (index, successor) in op.successors.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                self.write_block_name(out, *successor)?;
            }
            out.push(']');
        }

        if !op.regions.is_empty() {
            out.push(' ');
            out.push('(');
            for (index, region) in op.regions.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                self.print_region(*region, indent, out)?;
            }
            out.push(')');
        }

        if !op.attrs.is_empty() {
            out.push(' ');
            self.fmt_attr_dict(&op.attrs, out)?;
        }

        out.push_str(" : (");
        for (index, operand) in op.operands.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            self.fmt_type(self.ctx.value_type(*operand), out)?;
        }
        out.push_str(") -> ");
        if op.results.len() == 1 {
            self.fmt_type(op.results[0], out)?;
        } else {
            out.push('(');
            for (index, result) in op.results.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                self.fmt_type(*result, out)?;
            }
            out.push(')');
        }

        out.push('\n');
        Ok(())
    }

    fn print_region(&self, region: RegionId, indent: usize, out: &mut String) -> Result<()> {
        out.push_str("{\n");
        for block in &self.ctx.region(region).blocks {
            self.write_indent(out, indent + 1);
            self.write_block_name(out, *block)?;

            let args = &self.ctx.block(*block).args;
            if !args.is_empty() {
                out.push('(');
                for (index, ty) in args.iter().enumerate() {
                    if index != 0 {
                        out.push_str(", ");
                    }
                    self.write_value(out, Value::block_arg(*block, index as u32))?;
                    out.push_str(": ");
                    self.fmt_type(*ty, out)?;
                }
                out.push(')');
            }

            out.push_str(":\n");
            for op in &self.ctx.block(*block).ops {
                self.write_indent(out, indent + 2);
                self.print_op(*op, indent + 2, out)?;
            }
        }
        self.write_indent(out, indent);
        out.push('}');
        Ok(())
    }

    fn fmt_type(&self, ty: TypeId, out: &mut String) -> Result<()> {
        let Some(data) = self.ctx.try_type_node(ty) else {
            return Err(Error::new(format!("dangling type {ty:?}")));
        };
        let name = self.ctx.symbol_str(data.name);
        match name {
            "int" => {
                let (width, signedness) = self
                    .ctx
                    .int_info(ty)
                    .ok_or_else(|| Error::new(format!("malformed integer type {ty:?}")))?;
                match signedness {
                    Signedness::Signless => {
                        let _ = write!(out, "i{width}");
                    }
                    Signedness::Signed => {
                        let _ = write!(out, "si{width}");
                    }
                    Signedness::Unsigned => {
                        let _ = write!(out, "ui{width}");
                    }
                }
            }
            "float" => {
                let width = self
                    .ctx
                    .float_width(ty)
                    .ok_or_else(|| Error::new(format!("malformed float type {ty:?}")))?;
                let _ = write!(out, "f{width}");
            }
            "index" => out.push_str("index"),
            "none" => out.push_str("none"),
            "func" => {
                let (inputs, results) = self
                    .ctx
                    .func_parts(ty)
                    .ok_or_else(|| Error::new(format!("malformed function type {ty:?}")))?;
                out.push('(');
                for (index, input) in inputs.iter().enumerate() {
                    if index != 0 {
                        out.push_str(", ");
                    }
                    self.fmt_type(*input, out)?;
                }
                out.push_str(") -> ");
                if results.len() == 1 {
                    self.fmt_type(results[0], out)?;
                } else {
                    out.push('(');
                    for (index, result) in results.iter().enumerate() {
                        if index != 0 {
                            out.push_str(", ");
                        }
                        self.fmt_type(*result, out)?;
                    }
                    out.push(')');
                }
            }
            _ => {
                out.push('!');
                escape_bare_into(out, name);
                if !data.args.is_empty() {
                    out.push('<');
                    for (index, arg) in data.args.iter().enumerate() {
                        if index != 0 {
                            out.push_str(", ");
                        }
                        self.fmt_type_arg(arg, out)?;
                    }
                    out.push('>');
                }
            }
        }
        if !data.attrs.is_empty() {
            out.push(' ');
            self.fmt_attr_dict(&data.attrs, out)?;
        }
        Ok(())
    }

    fn fmt_type_arg(&self, arg: &TypeArg, out: &mut String) -> Result<()> {
        match arg {
            TypeArg::Type(ty) => self.fmt_type(*ty, out)?,
            TypeArg::Int(value) => {
                let _ = write!(out, "{value}");
            }
            TypeArg::Str(name) => escape_bare_into(out, self.ctx.symbol_str(*name)),
            TypeArg::Field(name, ty) => {
                escape_bare_into(out, self.ctx.symbol_str(*name));
                out.push_str(": ");
                self.fmt_type(*ty, out)?;
            }
            TypeArg::Bool(value) => {
                let _ = write!(out, "{value}");
            }
        }
        Ok(())
    }

    fn fmt_attr_dict(&self, dict: &AttrDict, out: &mut String) -> Result<()> {
        let mut entries: Vec<_> = dict.iter().collect();
        entries.sort_by(|a, b| self.ctx.symbol_str(a.0).cmp(self.ctx.symbol_str(b.0)));

        out.push('{');
        for (index, (key, value)) in entries.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            escape_bare_into(out, self.ctx.symbol_str(*key));
            out.push_str(" = ");
            self.fmt_attr(value, out)?;
        }
        out.push('}');
        Ok(())
    }

    fn fmt_attr(&self, attr: &Attr, out: &mut String) -> Result<()> {
        match attr {
            Attr::Unit => out.push_str("unit"),
            Attr::Bool(value) => {
                let _ = write!(out, "{value}");
            }
            Attr::Int { ty, bits } => {
                let (width, signedness) = self.ctx.int_info(*ty).ok_or_else(|| {
                    Error::new(format!("integer attribute has non-integer type {ty:?}"))
                })?;
                out.push_str(&format_int(*bits, width, signedness));
                out.push_str(" : ");
                self.fmt_type(*ty, out)?;
            }
            Attr::Float { ty, bits } => {
                let width = self.ctx.float_width(*ty).ok_or_else(|| {
                    Error::new(format!("float attribute has non-float type {ty:?}"))
                })?;
                out.push_str(&format_float(*bits, width)?);
                out.push_str(" : ");
                self.fmt_type(*ty, out)?;
            }
            Attr::Str(name) => {
                out.push('"');
                escape_into(out, self.ctx.symbol_str(*name));
                out.push('"');
            }
            Attr::Symbol(name) => {
                out.push('@');
                escape_bare_into(out, self.ctx.symbol_str(*name));
            }
            Attr::Type(ty) => self.fmt_type(*ty, out)?,
            Attr::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        out.push_str(", ");
                    }
                    self.fmt_attr(value, out)?;
                }
                out.push(']');
            }
            Attr::Dict(dict) => self.fmt_attr_dict(dict, out)?,
            Attr::Opaque { name, args } => {
                out.push('#');
                escape_bare_into(out, self.ctx.symbol_str(*name));
                if !args.is_empty() {
                    out.push('<');
                    for (index, arg) in args.iter().enumerate() {
                        if index != 0 {
                            out.push_str(", ");
                        }
                        self.fmt_attr(arg, out)?;
                    }
                    out.push('>');
                }
            }
        }
        Ok(())
    }
}

fn escape_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
}

/// Whether `text` can be printed without quotes and read back as a bare name.
fn is_bare(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if text.ends_with('-') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '$' | '-'))
}

fn escape_bare_into(out: &mut String, text: &str) {
    if is_bare(text) {
        out.push_str(text);
    } else {
        out.push('"');
        escape_into(out, text);
        out.push('"');
    }
}

fn format_int(bits: u128, width: u16, signedness: Signedness) -> String {
    match signedness {
        Signedness::Unsigned => bits.to_string(),
        Signedness::Signless | Signedness::Signed => {
            if width == 0 || width >= 128 {
                (bits as i128).to_string()
            } else {
                let sign_bit = 1u128 << (width - 1);
                if bits & sign_bit == 0 {
                    bits.to_string()
                } else {
                    let mask = (1u128 << width) - 1;
                    let value = (bits & mask) as i128 - (1i128 << width);
                    value.to_string()
                }
            }
        }
    }
}

fn format_float(bits: u64, width: u16) -> Result<String> {
    match width {
        32 => Ok(format!("{:?}", f32::from_bits(bits as u32))),
        64 => Ok(format!("{:?}", f64::from_bits(bits))),
        _ => Err(Error::new(format!(
            "cannot print {width}-bit float attribute"
        ))),
    }
}
