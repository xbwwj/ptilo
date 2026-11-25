use ptilo::{AttrConstraint, Context, OpSpec, TypeArg, TypeId, attr};

pub const TYPE_ALIAS: OpSpec = OpSpec {
    name: "ts.type_alias",
    attrs: &[
        attr("name", true, AttrConstraint::Symbol),
        attr("type", true, AttrConstraint::Type),
    ],
    ..OpSpec::DEFAULT
};

pub fn atom(ctx: &mut Context, name: &str) -> TypeId {
    ctx.opaque_type(name, vec![])
}

pub fn array(ctx: &mut Context, element: TypeId) -> TypeId {
    ctx.opaque_type("ts.array", vec![TypeArg::Type(element)])
}

pub fn tuple(ctx: &mut Context, elements: &[TypeId]) -> TypeId {
    ctx.opaque_type(
        "ts.tuple",
        elements.iter().copied().map(TypeArg::Type).collect(),
    )
}

pub fn union(ctx: &mut Context, elements: &[TypeId]) -> TypeId {
    ctx.opaque_type(
        "ts.union",
        elements.iter().copied().map(TypeArg::Type).collect(),
    )
}

pub fn intersection(ctx: &mut Context, elements: &[TypeId]) -> TypeId {
    ctx.opaque_type(
        "ts.intersection",
        elements.iter().copied().map(TypeArg::Type).collect(),
    )
}

pub fn record(ctx: &mut Context, fields: &[(&str, TypeId)]) -> TypeId {
    let mut args = Vec::with_capacity(fields.len());
    for (name, ty) in fields {
        let name = ctx.intern(name);
        args.push(TypeArg::Field(name, *ty));
    }
    ctx.opaque_type("ts.type_literal", args)
}

pub fn fn_type(ctx: &mut Context, params: &[TypeId], result: TypeId) -> TypeId {
    let mut args = Vec::with_capacity(params.len() + 2);
    args.push(TypeArg::Int(params.len() as u64));
    args.extend(params.iter().copied().map(TypeArg::Type));
    args.push(TypeArg::Type(result));
    ctx.opaque_type("ts.fn", args)
}

pub fn type_reference(ctx: &mut Context, name: &str, args: &[TypeId]) -> TypeId {
    let name = ctx.intern(name);
    let mut type_args = Vec::with_capacity(args.len() + 1);
    type_args.push(TypeArg::Str(name));
    type_args.extend(args.iter().copied().map(TypeArg::Type));
    ctx.opaque_type("ts.type_reference", type_args)
}

pub fn keyof(ctx: &mut Context, ty: TypeId) -> TypeId {
    ctx.opaque_type("ts.keyof", vec![TypeArg::Type(ty)])
}

pub fn indexed_access(ctx: &mut Context, object: TypeId, index: TypeId) -> TypeId {
    ctx.opaque_type(
        "ts.indexed_access",
        vec![TypeArg::Type(object), TypeArg::Type(index)],
    )
}

pub fn conditional(
    ctx: &mut Context,
    check: TypeId,
    extends: TypeId,
    true_ty: TypeId,
    false_ty: TypeId,
) -> TypeId {
    ctx.opaque_type(
        "ts.conditional",
        vec![
            TypeArg::Type(check),
            TypeArg::Type(extends),
            TypeArg::Type(true_ty),
            TypeArg::Type(false_ty),
        ],
    )
}

pub fn optional(ctx: &mut Context, ty: TypeId) -> TypeId {
    ctx.opaque_type("ts.optional", vec![TypeArg::Type(ty)])
}

pub fn rest(ctx: &mut Context, ty: TypeId) -> TypeId {
    ctx.opaque_type("ts.rest", vec![TypeArg::Type(ty)])
}

pub fn named_tuple_member(ctx: &mut Context, label: &str, ty: TypeId, optional: bool) -> TypeId {
    let label = ctx.intern(label);
    ctx.opaque_type(
        "ts.named_tuple_member",
        vec![
            TypeArg::Str(label),
            TypeArg::Type(ty),
            TypeArg::Bool(optional),
        ],
    )
}

pub fn literal_string(ctx: &mut Context, value: &str) -> TypeId {
    let value = ctx.intern(value);
    ctx.opaque_type("ts.literal", vec![TypeArg::Str(value)])
}
