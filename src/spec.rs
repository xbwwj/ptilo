//! Static operation metadata.

use crate::Result;
use crate::attr::Attr;
use crate::error::Error;
use crate::ir::{Context, OpId, Operation};

#[derive(Debug, Clone, Copy)]
pub struct OpSpec {
    pub name: &'static str,
    pub operands: &'static [OperandSpec],
    pub results: &'static [ResultSpec],
    pub regions: &'static [RegionSpec],
    pub attrs: &'static [AttrSpec],
    pub terminator: bool,
    pub verify: Option<fn(&Context, OpId) -> Result<()>>,
}

impl OpSpec {
    pub const DEFAULT: Self = Self {
        name: "<unnamed>",
        operands: &[],
        results: &[],
        regions: &[],
        attrs: &[],
        terminator: false,
        verify: None,
    };

    pub fn operand_count_ok(&self, count: usize) -> bool {
        count_matches(self.operands, count, |spec| spec.variadic)
    }

    pub fn result_count_ok(&self, count: usize) -> bool {
        count_matches(self.results, count, |spec| spec.variadic)
    }

    pub fn region_count_ok(&self, count: usize) -> bool {
        let required = self.regions.iter().filter(|spec| !spec.optional).count();
        count >= required && count <= self.regions.len()
    }

    pub fn check_builder(&self, ctx: &Context, op: &Operation) -> Result<()> {
        if !self.operand_count_ok(op.operands.len()) {
            return Err(Error::new(format!(
                "{} expects {} operands, found {}",
                self.name,
                describe_arity(
                    self.operands.len(),
                    self.operands.last().is_some_and(|s| s.variadic)
                ),
                op.operands.len()
            )));
        }
        if !self.result_count_ok(op.results.len()) {
            return Err(Error::new(format!(
                "{} expects {} results, found {}",
                self.name,
                describe_arity(
                    self.results.len(),
                    self.results.last().is_some_and(|s| s.variadic)
                ),
                op.results.len()
            )));
        }
        for spec in self.attrs {
            let key = ctx.lookup_symbol(spec.name);
            match key.and_then(|key| op.attrs.get(key)) {
                Some(attr) => {
                    if !spec.constraint.matches(attr) {
                        return Err(Error::new(format!(
                            "{} attribute {:?} has the wrong shape",
                            self.name, spec.name
                        )));
                    }
                }
                None if spec.required => {
                    return Err(Error::new(format!(
                        "{} is missing required attribute {:?}",
                        self.name, spec.name
                    )));
                }
                None => {}
            }
        }
        Ok(())
    }

    pub fn check_operation(&self, ctx: &Context, op: &Operation) -> Result<()> {
        self.check_builder(ctx, op)?;
        if !self.region_count_ok(op.regions.len()) {
            let required = self.regions.iter().filter(|spec| !spec.optional).count();
            let expected = if required == self.regions.len() {
                format!("{required}")
            } else {
                format!("{required}..={}", self.regions.len())
            };
            return Err(Error::new(format!(
                "{} expects {expected} regions, found {}",
                self.name,
                op.regions.len()
            )));
        }
        Ok(())
    }
}

fn count_matches<T>(specs: &[T], count: usize, is_variadic: impl Fn(&T) -> bool) -> bool {
    match specs.split_last() {
        Some((last, rest)) if is_variadic(last) => count >= rest.len(),
        _ => count == specs.len(),
    }
}

fn describe_arity(declared: usize, variadic: bool) -> String {
    if variadic {
        format!("at least {}", declared.saturating_sub(1))
    } else {
        declared.to_string()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OperandSpec {
    pub name: &'static str,
    pub variadic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ResultSpec {
    pub name: &'static str,
    pub variadic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RegionSpec {
    pub name: &'static str,
    pub single_block: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AttrSpec {
    pub name: &'static str,
    pub required: bool,
    pub constraint: AttrConstraint,
}

#[derive(Debug, Clone, Copy)]
pub enum AttrConstraint {
    Int,
    Float,
    Bool,
    Str,
    Symbol,
    Type,
}

impl AttrConstraint {
    pub fn matches(self, attr: &Attr) -> bool {
        match self {
            AttrConstraint::Int => matches!(attr, Attr::Int { .. }),
            AttrConstraint::Float => matches!(attr, Attr::Float { .. }),
            AttrConstraint::Bool => matches!(attr, Attr::Bool(_)),
            AttrConstraint::Str => matches!(attr, Attr::Str(_)),
            AttrConstraint::Symbol => matches!(attr, Attr::Symbol(_)),
            AttrConstraint::Type => matches!(attr, Attr::Type(_)),
        }
    }
}

pub const fn opd(name: &'static str) -> OperandSpec {
    OperandSpec {
        name,
        variadic: false,
    }
}

pub const fn opd_var(name: &'static str) -> OperandSpec {
    OperandSpec {
        name,
        variadic: true,
    }
}

pub const fn res(name: &'static str) -> ResultSpec {
    ResultSpec {
        name,
        variadic: false,
    }
}

pub const fn region(name: &'static str) -> RegionSpec {
    RegionSpec {
        name,
        single_block: false,
        optional: false,
    }
}

pub const fn region_optional(name: &'static str) -> RegionSpec {
    RegionSpec {
        name,
        single_block: false,
        optional: true,
    }
}

pub const fn attr(name: &'static str, required: bool, constraint: AttrConstraint) -> AttrSpec {
    AttrSpec {
        name,
        required,
        constraint,
    }
}
