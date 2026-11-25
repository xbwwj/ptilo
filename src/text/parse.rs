use std::collections::HashMap;

use crate::attr::{Attr, AttrDict};
use crate::ir::{BlockId, Context, Operation, RegionId, Value};
use crate::text::lex::{Spanned, Tok, lex};
use crate::types::{Signedness, TypeArg, TypeId, TypeNode};
use crate::{Error, Result};

pub fn parse(text: &str) -> Result<Context> {
    let mut ctx = Context::new();
    parse_into(&mut ctx, text)?;
    Ok(ctx)
}

/// Parse `text` and append the result to `ctx`. On error `ctx` is left exactly
/// as it was: the parser only ever appends, so the work is rolled back.
pub fn parse_into(ctx: &mut Context, text: &str) -> Result<()> {
    let tokens = lex(text)
        .map_err(|err| Error::new(format!("{} at {}", err.message, location(text, err.start))))?;
    let checkpoint = ctx.checkpoint();
    let result = Parser::new(tokens, ctx, text).parse_top();
    if result.is_err() {
        ctx.rollback(checkpoint);
    }
    result
}

/// Rendered as `line:col`, 1-based.
fn location(text: &str, offset: usize) -> String {
    let mut line = 1;
    let mut col = 1;
    for (index, ch) in text.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    format!("{line}:{col}")
}

struct Parser<'a> {
    tokens: Vec<Spanned>,
    pos: usize,
    ctx: &'a mut Context,
    text: &'a str,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Spanned>, ctx: &'a mut Context, text: &'a str) -> Self {
        let end = tokens.last().map_or(0, |tok| tok.end);
        let mut tokens = tokens;
        tokens.push(Spanned::eof(end));
        Self {
            tokens,
            pos: 0,
            ctx,
            text,
        }
    }

    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn peek_at(&self, offset: usize) -> &Tok {
        self.tokens
            .get(self.pos + offset)
            .map_or(&Tok::Eof, |tok| &tok.tok)
    }

    fn bump(&mut self) -> Tok {
        let tok = self.peek().clone();
        if !matches!(tok, Tok::Eof) {
            self.pos += 1;
        }
        tok
    }

    fn at_end(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn eat(&mut self, expected: &Tok) -> bool {
        if self.peek() == expected {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Error at the current token, with a `line:col` position.
    fn error(&self, message: impl std::fmt::Display) -> Error {
        Error::new(format!(
            "{message} at {}",
            location(self.text, self.tokens[self.pos].start)
        ))
    }

    fn expect(&mut self, expected: &Tok) -> Result<()> {
        if self.eat(expected) {
            Ok(())
        } else {
            let found = self.peek().clone();
            Err(self.error(format!("expected {expected:?}, found {found:?}")))
        }
    }

    fn expect_string(&mut self) -> Result<String> {
        match self.bump() {
            Tok::Str(name) | Tok::Ident(name) => Ok(name),
            other => Err(self.error(format!("expected operation name, found {other:?}"))),
        }
    }

    fn expect_value_name(&mut self) -> Result<String> {
        match self.bump() {
            Tok::Value(name) => Ok(name),
            other => Err(self.error(format!("expected SSA value, found {other:?}"))),
        }
    }

    fn expect_block_name(&mut self) -> Result<String> {
        match self.bump() {
            Tok::Block(name) => Ok(name),
            other => Err(self.error(format!("expected block label, found {other:?}"))),
        }
    }

    fn parse_top(&mut self) -> Result<()> {
        let mut env = HashMap::new();
        while !self.at_end() {
            let mut pending = Vec::new();
            self.parse_op(None, &mut env, &mut pending)?;
            if !pending.is_empty() {
                return Err(self.error("top-level operations cannot have successors"));
            }
        }
        Ok(())
    }

    fn parse_op(
        &mut self,
        parent_block: Option<BlockId>,
        env: &mut HashMap<String, Value>,
        pending: &mut Vec<(crate::OpId, Vec<String>)>,
    ) -> Result<crate::OpId> {
        let mut result_names = Vec::new();
        if matches!(self.peek(), Tok::Value(_)) {
            loop {
                result_names.push(self.expect_value_name()?);
                if self.eat(&Tok::Comma) {
                    continue;
                }
                break;
            }
            self.expect(&Tok::Eq)?;
        }

        let name = self.expect_string()?;
        let name = self.ctx.intern(&name);

        let mut operands = Vec::new();
        if self.eat(&Tok::LParen) && !self.eat(&Tok::RParen) {
            loop {
                operands.push(self.parse_value_ref(env)?);
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(&Tok::RParen)?;
                break;
            }
        }

        let mut successor_names = Vec::new();
        if self.eat(&Tok::LBracket) && !self.eat(&Tok::RBracket) {
            loop {
                successor_names.push(self.expect_block_name()?);
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(&Tok::RBracket)?;
                break;
            }
        }

        let mut regions = Vec::new();
        if self.eat(&Tok::LParen) && !self.eat(&Tok::RParen) {
            loop {
                regions.push(self.parse_region(env)?);
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(&Tok::RParen)?;
                break;
            }
        }

        let mut attrs = AttrDict::new();
        if self.eat(&Tok::LBrace) {
            attrs = self.parse_attr_dict_body()?;
        }

        let mut operand_types = Vec::new();
        let mut result_types = Vec::new();
        if self.eat(&Tok::Colon) {
            self.expect(&Tok::LParen)?;
            if !self.eat(&Tok::RParen) {
                loop {
                    operand_types.push(self.parse_type()?);
                    if self.eat(&Tok::Comma) {
                        continue;
                    }
                    self.expect(&Tok::RParen)?;
                    break;
                }
            }
            self.expect(&Tok::Arrow)?;
            if self.eat(&Tok::LParen) {
                if !self.eat(&Tok::RParen) {
                    loop {
                        result_types.push(self.parse_type()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        self.expect(&Tok::RParen)?;
                        break;
                    }
                }
            } else {
                result_types.push(self.parse_type()?);
            }
        }

        if !operand_types.is_empty() || !operands.is_empty() {
            if operand_types.len() != operands.len() {
                return Err(self.error(format!(
                    "operation has {} operands but {} operand types",
                    operands.len(),
                    operand_types.len()
                )));
            }
            for (operand, expected) in operands.iter().zip(&operand_types) {
                let actual = self.ctx.value_type(*operand);
                if actual != *expected {
                    return Err(self.error(format!(
                        "operand type mismatch: expected {:?}, found {:?}",
                        expected, actual
                    )));
                }
            }
        }

        if result_names.len() != result_types.len() {
            return Err(self.error(format!(
                "operation has {} results but {} result types",
                result_names.len(),
                result_types.len()
            )));
        }

        let mut op = Operation::new(name);
        op.operands = operands;
        op.results = result_types;
        op.attrs = attrs;
        op.regions = regions;
        let op_id = self.ctx.alloc_op(op);

        if let Some(block) = parent_block {
            self.ctx.attach_op(block, op_id);
        } else {
            self.ctx.push_root(op_id);
        }

        for (index, result_name) in result_names.iter().enumerate() {
            let value = Value::op_result(op_id, index as u32);
            if env.insert(result_name.clone(), value).is_some() {
                return Err(self.error(format!("duplicate SSA value %{result_name}")));
            }
        }

        if !successor_names.is_empty() {
            pending.push((op_id, successor_names));
        }

        Ok(op_id)
    }

    fn parse_region(&mut self, outer_env: &HashMap<String, Value>) -> Result<RegionId> {
        self.expect(&Tok::LBrace)?;
        let region = self.ctx.new_region();
        // One scope per region: values (including block arguments) are visible
        // to every block that follows them in the region, exactly like MLIR's
        // parser. Rejecting a non-dominating use is the verifier's job, not the
        // parser's.
        let mut env = outer_env.clone();
        let mut block_map = HashMap::new();
        let mut pending = Vec::new();

        while !self.eat(&Tok::RBrace) {
            if self.at_end() {
                return Err(self.error("unterminated region"));
            }
            self.parse_block(region, &mut env, &mut block_map, &mut pending)?;
        }

        for (op_id, names) in pending {
            let mut successors = Vec::with_capacity(names.len());
            for name in names {
                let block = block_map
                    .get(&name)
                    .copied()
                    .ok_or_else(|| self.error(format!("unknown block label ^{name}")))?;
                successors.push(block);
            }
            self.ctx.op_mut(op_id).successors = successors;
        }

        Ok(region)
    }

    fn parse_block(
        &mut self,
        region: RegionId,
        env: &mut HashMap<String, Value>,
        block_map: &mut HashMap<String, BlockId>,
        pending: &mut Vec<(crate::OpId, Vec<String>)>,
    ) -> Result<()> {
        let label = self.expect_block_name()?;

        let mut arg_names = Vec::new();
        let mut arg_types = Vec::new();
        if self.eat(&Tok::LParen) && !self.eat(&Tok::RParen) {
            loop {
                arg_names.push(self.expect_value_name()?);
                self.expect(&Tok::Colon)?;
                arg_types.push(self.parse_type()?);
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(&Tok::RParen)?;
                break;
            }
        }
        self.expect(&Tok::Colon)?;

        let block = self.ctx.new_block(region, &arg_types);
        if block_map.insert(label.clone(), block).is_some() {
            return Err(self.error(format!("duplicate block label ^{label}")));
        }

        for (index, name) in arg_names.iter().enumerate() {
            let value = Value::block_arg(block, index as u32);
            if env.insert(name.clone(), value).is_some() {
                return Err(self.error(format!("duplicate SSA value %{name}")));
            }
        }

        while !matches!(self.peek(), Tok::Block(_) | Tok::RBrace | Tok::Eof) {
            self.parse_op(Some(block), env, pending)?;
        }

        Ok(())
    }

    fn parse_value_ref(&mut self, env: &HashMap<String, Value>) -> Result<Value> {
        let name = self.expect_value_name()?;
        env.get(&name)
            .copied()
            .ok_or_else(|| self.error(format!("undefined SSA value %{name}")))
    }

    fn parse_attr_dict_body(&mut self) -> Result<AttrDict> {
        let mut dict = AttrDict::new();
        if self.eat(&Tok::RBrace) {
            return Ok(dict);
        }
        loop {
            let key = match self.bump() {
                Tok::Ident(name) | Tok::Str(name) => name,
                other => {
                    return Err(self.error(format!("expected attribute name, found {other:?}")));
                }
            };
            self.expect(&Tok::Eq)?;
            let value = self.parse_attr()?;
            let key = self.ctx.intern(&key);
            if dict.get(key).is_some() {
                return Err(self.error(format!("duplicate attribute {key:?}")));
            }
            dict.set(key, value);
            if self.eat(&Tok::Comma) {
                continue;
            }
            self.expect(&Tok::RBrace)?;
            break;
        }
        Ok(dict)
    }

    fn parse_attr(&mut self) -> Result<Attr> {
        match self.peek().clone() {
            Tok::Ident(name) => match name.as_str() {
                "unit" => {
                    self.bump();
                    Ok(Attr::Unit)
                }
                "true" => {
                    self.bump();
                    Ok(Attr::Bool(true))
                }
                "false" => {
                    self.bump();
                    Ok(Attr::Bool(false))
                }
                _ if is_type_start(&name) => Ok(Attr::Type(self.parse_type()?)),
                _ => Err(self.error(format!("unexpected bare attribute {name:?}"))),
            },
            Tok::Str(text) => {
                self.bump();
                Ok(Attr::Str(self.ctx.intern(&text)))
            }
            Tok::Symbol(name) => {
                self.bump();
                Ok(Attr::Symbol(self.ctx.intern(&name)))
            }
            Tok::Attr(name) => {
                self.bump();
                let name = self.ctx.intern(&name);
                let mut args = Vec::new();
                if self.eat(&Tok::Lt) && !self.eat(&Tok::Gt) {
                    loop {
                        args.push(self.parse_attr()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        self.expect(&Tok::Gt)?;
                        break;
                    }
                }
                Ok(Attr::Opaque { name, args })
            }
            Tok::LBracket => {
                self.bump();
                let mut values = Vec::new();
                if !self.eat(&Tok::RBracket) {
                    loop {
                        values.push(self.parse_attr()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        self.expect(&Tok::RBracket)?;
                        break;
                    }
                }
                Ok(Attr::Array(values))
            }
            Tok::LBrace => {
                self.bump();
                Ok(Attr::Dict(self.parse_attr_dict_body()?))
            }
            Tok::Number(text) => {
                self.bump();
                self.expect(&Tok::Colon)?;
                let ty = self.parse_type()?;
                if text.contains('.') || text.contains('e') || text.contains('E') {
                    let bits = self.parse_float_bits(&text, ty)?;
                    Ok(Attr::Float { ty, bits })
                } else {
                    let bits = self.parse_int_bits(&text, ty)?;
                    Ok(Attr::Int { ty, bits })
                }
            }
            Tok::TypeName(_) | Tok::LParen => Ok(Attr::Type(self.parse_type()?)),
            other => Err(self.error(format!("unexpected attribute token {other:?}"))),
        }
    }

    fn parse_type(&mut self) -> Result<TypeId> {
        let ty = self.parse_type_body()?;
        // Dialect types may carry a trailing attribute dictionary.
        if matches!(self.peek(), Tok::LBrace) {
            self.bump();
            let attrs = self.parse_attr_dict_body()?;
            let data = self.ctx.type_node(ty).clone();
            return Ok(self.ctx.intern_type(TypeNode { attrs, ..data }));
        }
        Ok(ty)
    }

    fn parse_type_body(&mut self) -> Result<TypeId> {
        match self.bump() {
            Tok::TypeName(name) => {
                let name = self.ctx.intern(name.trim_start_matches('!'));
                let mut args = Vec::new();
                if self.eat(&Tok::Lt) && !self.eat(&Tok::Gt) {
                    loop {
                        args.push(self.parse_type_arg()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        self.expect(&Tok::Gt)?;
                        break;
                    }
                }
                Ok(self.ctx.intern_type(TypeNode {
                    name,
                    args,
                    attrs: AttrDict::new(),
                }))
            }
            Tok::LParen => {
                let mut inputs = Vec::new();
                if !self.eat(&Tok::RParen) {
                    loop {
                        inputs.push(self.parse_type()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        self.expect(&Tok::RParen)?;
                        break;
                    }
                }
                self.expect(&Tok::Arrow)?;
                let mut results = Vec::new();
                if self.eat(&Tok::LParen) {
                    if !self.eat(&Tok::RParen) {
                        loop {
                            results.push(self.parse_type()?);
                            if self.eat(&Tok::Comma) {
                                continue;
                            }
                            self.expect(&Tok::RParen)?;
                            break;
                        }
                    }
                } else {
                    results.push(self.parse_type()?);
                }
                Ok(self.ctx.func_type(&inputs, &results))
            }
            Tok::Ident(name) => {
                if let Some(rest) = name.strip_prefix("si")
                    && let Ok(width) = rest.parse::<u16>()
                {
                    return Ok(self.ctx.int_type(width, Signedness::Signed));
                }
                if let Some(rest) = name.strip_prefix("ui")
                    && let Ok(width) = rest.parse::<u16>()
                {
                    return Ok(self.ctx.int_type(width, Signedness::Unsigned));
                }
                if let Some(rest) = name.strip_prefix('i')
                    && let Ok(width) = rest.parse::<u16>()
                {
                    return Ok(self.ctx.int_type(width, Signedness::Signless));
                }
                if let Some(rest) = name.strip_prefix('f')
                    && let Ok(width) = rest.parse::<u16>()
                {
                    return Ok(self.ctx.float_type(width));
                }
                match name.as_str() {
                    "index" => Ok(self.ctx.index_type()),
                    "none" => Ok(self.ctx.none_type()),
                    _ => Err(self.error(format!("unknown type name {name:?}"))),
                }
            }
            other => Err(self.error(format!("expected type, found {other:?}"))),
        }
    }

    fn parse_type_arg(&mut self) -> Result<TypeArg> {
        match self.peek().clone() {
            Tok::TypeName(_) | Tok::LParen => Ok(TypeArg::Type(self.parse_type()?)),
            Tok::Ident(name) if is_type_start(&name) => Ok(TypeArg::Type(self.parse_type()?)),
            Tok::Ident(name) if name == "true" || name == "false" => {
                self.bump();
                Ok(TypeArg::Bool(name == "true"))
            }
            Tok::Ident(name) => {
                if self.peek_at(1) == &Tok::Colon {
                    self.bump();
                    self.expect(&Tok::Colon)?;
                    let ty = self.parse_type()?;
                    Ok(TypeArg::Field(self.ctx.intern(&name), ty))
                } else {
                    self.bump();
                    Ok(TypeArg::Str(self.ctx.intern(&name)))
                }
            }
            Tok::Str(text) => {
                self.bump();
                Ok(TypeArg::Str(self.ctx.intern(&text)))
            }
            Tok::Number(text) => {
                self.bump();
                Ok(TypeArg::Int(text.parse::<u64>().map_err(|_| {
                    self.error(format!("invalid integer type argument {text:?}"))
                })?))
            }
            other => Err(self.error(format!("unexpected type argument {other:?}"))),
        }
    }

    fn parse_int_bits(&self, text: &str, ty: TypeId) -> Result<u128> {
        let (width, _) = self
            .ctx
            .int_info(ty)
            .ok_or_else(|| self.error(format!("integer literal has non-integer type {ty:?}")))?;

        // `iN` accepts anything representable as either signed or unsigned N-bit.
        let (max, min_magnitude) = if width == 0 || width >= 128 {
            (u128::MAX, 1u128 << 127)
        } else {
            ((1u128 << width) - 1, 1u128 << (width - 1))
        };

        if let Ok(value) = text.parse::<i128>() {
            let magnitude = if value < 0 {
                value.unsigned_abs()
            } else {
                value as u128
            };
            let limit = if value < 0 { min_magnitude } else { max };
            if magnitude > limit {
                return Err(self.error(format!(
                    "integer literal {text:?} does not fit in {width}-bit type"
                )));
            }
            return Ok((value as u128) & max);
        }
        if let Ok(value) = text.parse::<u128>()
            && value <= max
        {
            return Ok(value);
        }
        Err(self.error(format!(
            "integer literal {text:?} does not fit in {width}-bit type"
        )))
    }

    fn parse_float_bits(&self, text: &str, ty: TypeId) -> Result<u64> {
        let width = self
            .ctx
            .float_width(ty)
            .ok_or_else(|| self.error(format!("float literal has non-float type {ty:?}")))?;
        let value = text
            .parse::<f64>()
            .map_err(|_| self.error(format!("invalid float literal {text:?}")))?;
        match width {
            32 => Ok((value as f32).to_bits() as u64),
            64 => Ok(value.to_bits()),
            _ => Err(self.error(format!(
                "unsupported float width {width} for literal {text:?}"
            ))),
        }
    }
}

fn is_type_start(name: &str) -> bool {
    if matches!(name, "index" | "none") {
        return true;
    }
    let digits = name
        .strip_prefix("si")
        .or_else(|| name.strip_prefix("ui"))
        .or_else(|| name.strip_prefix('i'))
        .or_else(|| name.strip_prefix('f'));
    digits.is_some_and(|digits| !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()))
}
