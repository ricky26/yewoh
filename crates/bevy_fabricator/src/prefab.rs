use std::any::TypeId;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use bevy::log::Level;
use bevy::prelude::*;
use bevy::reflect::{DynamicEnum, DynamicList, DynamicStruct, DynamicTuple, DynamicTupleStruct, DynamicVariant, ReflectKind, ReflectRef, TypeInfo, TypeRegistration, TypeRegistry, VariantInfo};
use bevy::utils::{tracing, HashMap};
use smallvec::SmallVec;

use crate::document::{Document, Expression, Import, Number, Path, Visibility};
use crate::string::parse_string;
use crate::traits::{ReflectEvaluate, ReflectApply, Context, ReflectConvert};
use crate::{Fabricated, FabricationParameter, Fabricator};
use crate::parser::FormatterFn;

type RegisterValue = Option<Arc<dyn PartialReflect>>;
type RegisterValues = Vec<RegisterValue>;

enum TypeOrVariant<'a> {
    Type(TypeId),
    Variant(TypeId, &'a str),
}

impl TypeOrVariant<'_> {
    pub fn id(&self) -> TypeId {
        match self {
            TypeOrVariant::Type(id) => *id,
            TypeOrVariant::Variant(id, _) => *id,
        }
    }
}

fn lookup_type(type_registry: &TypeRegistry, path: &Path) -> Option<TypeId> {
    let full_name = path.to_string();
    type_registry.get_with_type_path(&full_name).map(|r| r.type_id())
}

fn lookup_type_or_variant<'a>(
    type_registry: &'a TypeRegistry, path: &'a Path,
) -> Option<TypeOrVariant<'a>> {
    if path.len() == 1 {
        lookup_type(type_registry, path).map(TypeOrVariant::Type)
    } else {
        let full_name = path.to_string();
        let variant_name = *path.0.last().unwrap();
        let variant_len = variant_name.len();
        let enum_name = &full_name[..(full_name.len() - variant_len - 2)];
        type_registry.get_with_type_path(&full_name)
            .map(|reg| TypeOrVariant::Type(reg.type_id()))
            .or_else(|| {
                let reg = type_registry.get_with_type_path(enum_name)?;
                if reg.type_info().as_enum().is_ok() {
                    Some(TypeOrVariant::Variant(reg.type_id(), variant_name))
                } else {
                    None
                }
            })
    }
}

fn resolve_alias<'a>(aliases: &HashMap<String, Path<'a>>, path: &Path<'a>) -> Path<'a> {
    if let Some(first) = path.0.first() {
        if let Some(existing) = aliases.get(*first) {
            let mut segments = SmallVec::new();
            segments.extend_from_slice(&existing.0);
            segments.extend_from_slice(&path.0[1..]);
            return Path(segments);
        }
    }

    path.clone()
}

pub trait FabricatorSource {
    fn get(&self, path: &str) -> Option<Fabricator>;
}

#[derive(Default)]
pub struct FabricatorMap(pub HashMap<String, Fabricator>);

impl FabricatorSource for FabricatorMap {
    fn get(&self, path: &str) -> Option<Fabricator> {
        self.0.get(path).cloned()
    }
}

macro_rules! impl_load_number {
    ($steps:expr, $index:expr, $type_id:expr, $n:expr, $conv:ident, $ty:ty) => {
        if $type_id == Some(TypeId::of::<$ty>()) {
            let value = Arc::new(match $n {
                Number::I64(v) => *v as $ty,
                Number::U64(v) => *v as $ty,
                Number::F64(v) => *v as $ty,
            });
            let index = $index;
            $steps.push(Box::new(move |ctx, registers, _| {
                if registers[index].is_none() {
                    let value = value.as_ref().clone_value();
                    let value = $conv.convert(ctx, value)?;
                    registers[index] = Some(value.into());
                }
                Ok(())
            }));
            continue;
        }
    };
}

#[derive(Clone)]
struct Constructor {
    type_path: &'static str,
    default: Option<ReflectDefault>,
    from_world: Option<ReflectFromWorld>,
    from_reflect: Option<ReflectFromReflect>,
    dynamic_ctor: Option<fn() -> Box<dyn PartialReflect>>,
}

impl Constructor {
    pub fn from_registration(type_registration: &TypeRegistration) -> Constructor {
        let type_path = type_registration.type_info().ty().path();
        let default = type_registration.data::<ReflectDefault>().cloned();
        let from_world = type_registration.data::<ReflectFromWorld>().cloned();
        let from_reflect = type_registration.data::<ReflectFromReflect>().cloned();
        let mut dynamic_ctor: Option<fn() -> Box<dyn PartialReflect>> = None;

        fn empty_struct() -> Box<dyn PartialReflect> {
            Box::new(DynamicStruct::default())
        }

        fn empty_tuple_struct() -> Box<dyn PartialReflect> {
            Box::new(DynamicTupleStruct::default())
        }

        fn empty_tuple() -> Box<dyn PartialReflect> {
            Box::new(DynamicTuple::default())
        }

        fn empty_list() -> Box<dyn PartialReflect> {
            Box::new(DynamicList::default())
        }

        match type_registration.type_info() {
            TypeInfo::Struct(struct_info) if struct_info.field_len() == 0 =>
                dynamic_ctor = Some(empty_struct),
            TypeInfo::TupleStruct(struct_info) if struct_info.field_len() == 0 =>
                dynamic_ctor = Some(empty_tuple_struct),
            TypeInfo::Tuple(tuple_info) if tuple_info.field_len() == 0 =>
                dynamic_ctor = Some(empty_tuple),
            TypeInfo::List(_) =>
                dynamic_ctor = Some(empty_list),
            _ => {}
        }

        Constructor {
            type_path,
            default,
            from_world,
            from_reflect,
            dynamic_ctor,
        }
    }

    pub fn construct(
        &self,
        ctx: &mut Context,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        if let Some(default) = self.default.as_ref() {
            Ok(default.default().into_partial_reflect())
        } else if let Some(from_world) = self.from_world.as_ref() {
            Ok(from_world.from_world(ctx.world).into_partial_reflect())
        } else if let (Some(ctor), Some(from_reflect)) = (self.dynamic_ctor.as_ref(), self.from_reflect.as_ref()) {
            let seed_value = ctor();
            let new_value = from_reflect.from_reflect(seed_value.as_ref())
                .ok_or_else(|| anyhow!("failed to create empty value for {}", self.type_path))?;
            Ok(new_value.into_partial_reflect())
        } else {
            bail!("unable to construct an instance of {}", self.type_path);
        }
    }
}

#[derive(Clone)]
struct ValueConverter {
    type_id: TypeId,
    convert: Option<ReflectConvert>,
    constructor: Constructor,
}

impl ValueConverter {
    pub fn from_registration(type_registration: &TypeRegistration) -> ValueConverter {
        let type_id = type_registration.type_id();
        let convert = type_registration.data::<ReflectConvert>().cloned();
        let constructor = Constructor::from_registration(type_registration);

        ValueConverter {
            type_id,
            convert,
            constructor,
        }
    }

    pub fn convert(
        &self,
        ctx: &mut Context,
        value: Box<dyn PartialReflect>,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        if let Some(type_info) = value.get_represented_type_info() {
            if type_info.type_id() == self.type_id {
                return Ok(value);
            }
        }

        if let Some(convert) = self.convert.as_ref() {
            return convert.convert(value);
        }

        if let Some(from_reflect) = self.constructor.from_reflect.as_ref() {
            if let Some(reflected) = from_reflect.from_reflect(value.as_ref()) {
                return Ok(reflected.into_partial_reflect());
            }
        }

        let mut new_value = self.constructor.construct(ctx)?;
        new_value.try_apply(value.as_ref().as_partial_reflect())?;
        Ok(new_value.into_partial_reflect())
    }
}

struct Evaluator {
    evaluate: Option<ReflectEvaluate>,
}

impl Evaluator {
    pub fn from_registration(type_registration: &TypeRegistration) -> Evaluator {
        let evaluate = type_registration.data::<ReflectEvaluate>().cloned();
        Evaluator {
            evaluate,
        }
    }

    pub fn evaluate(
        &self,
        ctx: &mut Context<'_>,
        src: Box<dyn PartialReflect>,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        if let Some(evaluate) = &self.evaluate {
            let src = match src.try_into_reflect() {
                Ok(x) => x,
                Err(src) => {
                    bail!("{src:?} is not a concrete type");
                }
            };

            let Some(evaluate) = evaluate.get(src.as_ref()) else {
                bail!("{src:?} does not implement Evaluate");
            };

            Ok(evaluate.evaluate(ctx)?)
        } else {
            Ok(src)
        }
    }
}

struct Applicator {
    type_path: &'static str,
    apply: Option<ReflectApply>,
    component: Option<ReflectComponent>,
}

impl Applicator {
    pub fn from_registration(type_registration: &TypeRegistration) -> Applicator {
        let type_path = type_registration.type_info().type_path();
        let apply = type_registration.data::<ReflectApply>().cloned();
        let component = type_registration.data::<ReflectComponent>().cloned();

        Applicator {
            type_path,
            apply,
            component,
        }
    }

    pub fn apply(
        &self,
        ctx: &mut Context<'_>,
        src: &dyn PartialReflect,
        entity: Entity,
    ) -> anyhow::Result<()> {
        if let Some(reflect_apply) = &self.apply {
            let Some(src) = src.try_as_reflect() else {
                bail!("{src:?} is not a concrete type");
            };

            let Some(apply) = reflect_apply.get(src) else {
                bail!("{src:?} does not implement Command");
            };

            apply.apply(ctx, entity)?;
        } else if let Some(reflect_component) = &self.component {
            let type_registry = ctx.world.resource::<AppTypeRegistry>().clone();
            let type_registry = type_registry.read();
            let mut entity_mut = ctx.world.entity_mut(entity);
            reflect_component.insert(&mut entity_mut, src, &type_registry);
        } else {
            bail!("unknown apply type: {}", self.type_path);
        }

        Ok(())
    }
}

fn build_struct<'a>(
    body: &[(&'a str, usize)],
) -> impl Fn(
    &mut Context, &mut RegisterValues,
) -> anyhow::Result<DynamicStruct> {
    let body = body.iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect::<Vec<_>>();
    move |_ctx: &mut Context, registers: &mut RegisterValues| {
        let mut value = DynamicStruct::default();
        for (key, src) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = (**field_value).clone_value();
            value.insert_boxed(key, field_value);
        }
        Ok(value)
    }
}

fn build_tuple(
    body: &[usize],
) -> impl Fn(
    &mut Context, &mut RegisterValues,
) -> anyhow::Result<DynamicTuple> {
    let body = body.iter()
        .copied()
        .collect::<SmallVec<[_; 8]>>();
    move |_ctx: &mut Context, registers: &mut RegisterValues| {
        let mut value = DynamicTuple::default();
        for src in body.iter().copied() {
            let field_value = registers[src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", src))?;
            let field_value = (**field_value).clone_value();
            value.insert_boxed(field_value);
        }
        Ok(value)
    }
}

fn build_tuple_struct(
    body: &[usize],
) -> impl Fn(
    &mut Context, &mut RegisterValues,
) -> anyhow::Result<DynamicTupleStruct> {
    let body = body.iter()
        .copied()
        .collect::<Vec<_>>();
    move |_ctx: &mut Context, registers: &mut RegisterValues| {
        let mut value = DynamicTupleStruct::default();
        for src in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = field_value.as_ref().clone_value();
            value.insert_boxed(field_value);
        }
        Ok(value)
    }
}

fn build_list(
    body: &[usize],
) -> impl Fn(
    &mut Context, &mut RegisterValues,
) -> anyhow::Result<DynamicList> {
    let body = body.to_vec();
    move |_ctx: &mut Context, registers: &mut RegisterValues| {
        let mut value = DynamicList::default();
        for src in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = field_value.as_ref().clone_value();
            value.push_box(field_value);
        }
        Ok(value)
    }
}

pub fn convert(
    type_registry: &TypeRegistry,
    documents: &dyn FabricatorSource,
    doc: &Document,
) -> anyhow::Result<Fabricator> {
    type Step = Box<dyn Fn(&mut Context, &mut RegisterValues, Entity) -> anyhow::Result<()> + Send + Sync>;

    let mut parameters = HashMap::new();
    let mut aliases = HashMap::new();
    let mut file_imports = HashMap::new();
    let mut locals = HashMap::new();
    let mut inputs = HashMap::new();
    let mut requires_an_input = false;
    let mut steps: Vec<Step> = Vec::new();
    let mut register_types = vec![None; doc.registers.len()];

    // Add fixed root entity
    let root_index = register_types.len();
    register_types.push(Some(TypeId::of::<Entity>()));
    locals.insert("$".to_string(), root_index);

    // First pass: collect names & types
    for (index, register) in doc.registers.iter().enumerate() {
        if let Some(name) = register.name {
            if let Some(last_index) = locals.insert(name.to_string(), index) {
                bail!("duplicate variable name: '{name}' (index {index} and {last_index})");
            }

            if register.visibility == Visibility::In {
                inputs.insert(name.to_string(), (index, !register.optional));

                if !register.optional && register.expression.is_none() {
                    requires_an_input = true;
                }
            }

            if let Some(Expression::Import(import)) = &register.expression {
                match import {
                    Import::Path(path) => {
                        let path = resolve_alias(&aliases, path);
                        aliases.insert(name.to_string(), path);
                    }
                    Import::File(path) => {
                        let (_, unescaped_path) = parse_string(path)
                            .unwrap()
                            .unwrap();
                        let imported = documents.get(&unescaped_path)
                            .ok_or_else(|| anyhow!("missing imported prefab '{path}'"))?;
                        file_imports.insert(name.to_string(), Arc::new(imported) as Arc<dyn PartialReflect>);
                    }
                }
            }
        }
    }

    // Second pass: lookup types
    for (index, register) in doc.registers.iter().enumerate() {
        let mut register_type = register.variable_type.as_ref()
            .map(|path| resolve_alias(&aliases, path))
            .map(|path| lookup_type(type_registry, &path)
                .ok_or_else(|| anyhow!("no such type {path:?}")))
            .transpose()?;

        match &register.expression {
            Some(Expression::Struct(Some(path), _)) => {
                let path = resolve_alias(&aliases, path);
                let id = lookup_type_or_variant(type_registry, &path)
                    .ok_or_else(|| anyhow!("unknown type {path:?}"))?;
                register_type = Some(id.id());
            }
            Some(Expression::Tuple(Some(path), _)) => {
                let path = resolve_alias(&aliases, path);
                let id = lookup_type_or_variant(type_registry, &path)
                    .ok_or_else(|| anyhow!("unknown type {path:?}"))?;
                register_type = Some(id.id());
            }
            Some(Expression::List(Some(path), _)) => {
                let path = resolve_alias(&aliases, path);
                let id = lookup_type_or_variant(type_registry, &path)
                    .ok_or_else(|| anyhow!("unknown type {path:?}"))?;
                register_type = Some(id.id());
            }
            Some(Expression::Path(path)) => {
                let path = resolve_alias(&aliases, path);

                if path.len() == 1 {
                    if let Some(source) = locals.get(path.0[0]) {
                        register_type = register_type.or(register_types[*source]);
                    }
                }

                if let Some(id) = lookup_type_or_variant(type_registry, &path) {
                    register_type = register_type.or(Some(id.id()));
                }
            }
            _ => {}
        }

        register_types[index] = register_type;
    }

    // Third pass: propagate types
    for (index, register) in doc.registers.iter().enumerate().rev() {
        let Some(type_id) = register_types[index] else { continue };
        let register_type = type_registry.get(type_id).unwrap();
        if let Some(expr) = &register.expression {
            match expr {
                Expression::Tuple(type_path, body) => {
                    match register_type.type_info() {
                        TypeInfo::Struct(struct_info) => {
                            for (field_index, register_index) in body.iter().enumerate() {
                                if field_index > struct_info.field_len() {
                                    bail!("out of range field {field_index} in {}", register_type.type_info().ty().path());
                                }

                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                let field_info = struct_info.field_at(field_index).unwrap();
                                let field_ty = field_info.type_info().unwrap().ty();
                                *src_ty = Some(field_ty.id());
                            }
                        }
                        TypeInfo::TupleStruct(struct_info) => {
                            for (field_index, register_index) in body.iter().enumerate() {
                                if field_index > struct_info.field_len() {
                                    bail!("out of range field {field_index} in {}", register_type.type_info().ty().path());
                                }

                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                let field_info = struct_info.field_at(field_index).unwrap();
                                let field_ty = field_info.type_info().unwrap().ty();
                                *src_ty = Some(field_ty.id());
                            }
                        }
                        TypeInfo::Tuple(struct_info) => {
                            for (field_index, register_index) in body.iter().enumerate() {
                                if field_index > struct_info.field_len() {
                                    bail!("out of range field {field_index} in {}", register_type.type_info().ty().path());
                                }

                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                let field_info = struct_info.field_at(field_index).unwrap();
                                let field_ty = field_info.type_info().unwrap().ty();
                                *src_ty = Some(field_ty.id());
                            }
                        }
                        TypeInfo::Enum(enum_info) => {
                            if let Some(type_path) = &type_path {
                                let type_path = resolve_alias(&aliases, type_path);
                                let variant = enum_info.variant(type_path.0.last().unwrap())
                                    .ok_or_else(|| anyhow!("unknown variant {type_path:?}"))?;
                                match variant {
                                    VariantInfo::Struct(struct_info) => {
                                        for (field_index, register_index) in body.iter().enumerate() {
                                            if field_index > struct_info.field_len() {
                                                bail!("out of range field {field_index} in {}", register_type.type_info().ty().path());
                                            }

                                            let src_ty = &mut register_types[*register_index];
                                            if src_ty.is_some() {
                                                continue;
                                            }

                                            let field_info = struct_info.field_at(field_index).unwrap();
                                            let field_ty = field_info.type_info().unwrap().ty();
                                            *src_ty = Some(field_ty.id());
                                        }
                                    }
                                    VariantInfo::Tuple(struct_info) => {
                                        for (field_index, register_index) in body.iter().enumerate() {
                                            if field_index > struct_info.field_len() {
                                                bail!("out of range field {field_index} in {}", register_type.type_info().ty().path());
                                            }

                                            let src_ty = &mut register_types[*register_index];
                                            if src_ty.is_some() {
                                                continue;
                                            }

                                            let field_info = struct_info.field_at(field_index).unwrap();
                                            let field_ty = field_info.type_info().unwrap().ty();
                                            *src_ty = Some(field_ty.id());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Expression::Struct(type_path, body) => {
                    match register_type.type_info() {
                        TypeInfo::Struct(struct_info) => {
                            for (field_name, register_index) in body.iter() {
                                let Some(field_info) = struct_info.field(field_name) else {
                                    bail!("unknown field '{field_name}' on {}", register_type.type_info().ty().path());
                                };

                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                let field_ty = field_info.type_info().unwrap().ty();
                                *src_ty = Some(field_ty.id());
                            }
                        }
                        TypeInfo::Enum(enum_info) => {
                            if let Some(type_path) = &type_path {
                                let type_path = resolve_alias(&aliases, type_path);
                                let variant = enum_info.variant(type_path.0.last().unwrap())
                                    .ok_or_else(|| anyhow!("unknown variant {type_path:?}"))?;

                                if let VariantInfo::Struct(struct_info) = variant {
                                    for (field_name, register_index) in body.iter() {
                                        let Some(field_info) = struct_info.field(field_name) else {
                                            bail!("unknown field '{field_name}' on {}", register_type.type_info().ty().path());
                                        };

                                        let src_ty = &mut register_types[*register_index];
                                        if src_ty.is_some() {
                                            continue;
                                        }

                                        let field_ty = field_info.type_info().unwrap().ty();
                                        *src_ty = Some(field_ty.id());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Expression::List(_, body) => {
                    match register_type.type_info() {
                        TypeInfo::List(list_info) => {
                            let element_ty = list_info.item_ty().id();
                            for register_index in body.iter() {
                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                *src_ty = Some(element_ty);
                            }
                        }
                        TypeInfo::Array(array_info) => {
                            let element_ty = array_info.item_ty().id();
                            for register_index in body.iter() {
                                let src_ty = &mut register_types[*register_index];
                                if src_ty.is_some() {
                                    continue;
                                }

                                *src_ty = Some(element_ty);
                            }
                        }
                        _ => {}
                    }
                }
                Expression::Path(path) => {
                    if path.len() == 1 {
                        if let Some(source_index) = locals.get(path.0[0]) {
                            register_types[*source_index] = register_types[*source_index]
                                .or(register_types[index]);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if tracing::enabled!(Level::TRACE) {
        for (index, register) in doc.registers.iter().enumerate() {
            let type_id = register_types[index];
            let register_type = type_id.and_then(|id| type_registry.get(id));
            let type_path = register_type.map(|ty| ty.type_info().ty().path());
            let register_debug = FormatterFn(|f| register.fmt_with_index(index, f));
            trace!("register {register_debug} (name={}, type={})", register.name.unwrap_or("anonymous"), type_path.unwrap_or("any"));
        }
    }

    // Fourth pass: create steps
    for (index, (register, register_type)) in doc.registers.iter().zip(&register_types).enumerate() {
        if let (Visibility::In, Some(name), Some(ty)) = (register.visibility, register.name, register_type) {
            parameters.insert(name.to_string(), FabricationParameter {
                parameter_type: *ty,
                optional: register.optional,
            });
        }

        let register_type_id = register_types[index];
        let register_type_reg = register_type_id.and_then(|id| type_registry.get(id));

        if let Some(value) = &register.expression {
            match value {
                Expression::Number(n) => {
                    let type_reg = register_type_reg
                        .ok_or_else(|| anyhow!("untyped number"))?;
                    let converter = ValueConverter::from_registration(type_reg);

                    impl_load_number!(steps, index, register_type_id, n, converter, u8);
                    impl_load_number!(steps, index, register_type_id, n, converter, i8);
                    impl_load_number!(steps, index, register_type_id, n, converter, u16);
                    impl_load_number!(steps, index, register_type_id, n, converter, i16);
                    impl_load_number!(steps, index, register_type_id, n, converter, u32);
                    impl_load_number!(steps, index, register_type_id, n, converter, isize);
                    impl_load_number!(steps, index, register_type_id, n, converter, usize);
                    impl_load_number!(steps, index, register_type_id, n, converter, i32);
                    impl_load_number!(steps, index, register_type_id, n, converter, f32);

                    let value = Arc::new(match n {
                        Number::I64(v) => *v as f64,
                        Number::U64(v) => *v as f64,
                        Number::F64(v) => *v,
                    });
                    steps.push(Box::new(move |ctx, registers, _| {
                        if registers[index].is_none() {
                            let value = value.as_ref().clone_value();
                            let value = converter.convert(ctx, value)?;
                            registers[index] = Some(value.into());
                        }
                        Ok(())
                    }));
                }
                Expression::String(s) => {
                    let type_reg = register_type_reg
                        .ok_or_else(|| anyhow!("untyped string"))?;
                    let converter = ValueConverter::from_registration(type_reg);
                    let (_, value) = parse_string(s).unwrap().unwrap();
                    let value = Arc::new(value);
                    steps.push(Box::new(move |ctx, registers, _| {
                        if registers[index].is_none() {
                            let value = value.as_ref().clone_value();
                            let value = converter.convert(ctx, value)?;
                            registers[index] = Some(value.into());
                        }
                        Ok(())
                    }));
                }
                Expression::Tuple(type_path, body) => {
                    if let Some(type_path) = type_path.as_ref() {
                        let type_path = resolve_alias(&aliases, type_path);
                        let type_or_variant = lookup_type_or_variant(type_registry, &type_path)
                            .ok_or_else(|| anyhow!("unknown type: {type_path}"))?;

                        match type_or_variant {
                            TypeOrVariant::Type(id) => {
                                let factory = build_tuple_struct(body);
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {type_path:?}"))?;
                                let converter = ValueConverter::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = Box::new(factory(ctx, registers)?);
                                        let value = converter.convert(ctx, value)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                            TypeOrVariant::Variant(id, variant_name) => {
                                let factory = build_tuple(body);
                                let variant_name = variant_name.to_string();
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {type_path:?}"))?;
                                let converter = ValueConverter::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = factory(ctx, registers)?;
                                        let value = DynamicEnum::new(&variant_name, DynamicVariant::Tuple(value));
                                        let value = Box::new(value);
                                        let value = converter.convert(ctx, value)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                        }
                    } else {
                        let type_reg = register_type_reg
                            .ok_or_else(|| anyhow!("untyped tuple"))?;
                        let converter = ValueConverter::from_registration(type_reg);
                        if type_reg.type_info().kind() == ReflectKind::TupleStruct {
                            let factory = build_tuple_struct(body);
                            steps.push(Box::new(move |ctx, registers, _| {
                                if registers[index].is_none() {
                                    let value = Box::new(factory(ctx, registers)?);
                                    let value = converter.convert(ctx, value)?;
                                    registers[index] = Some(value.into());
                                }
                                Ok(())
                            }));
                        } else {
                            let factory = build_tuple(body);
                            steps.push(Box::new(move |ctx, registers, _| {
                                if registers[index].is_none() {
                                    let value = Box::new(factory(ctx, registers)?);
                                    let value = converter.convert(ctx, value)?;
                                    registers[index] = Some(value.into());
                                }
                                Ok(())
                            }));
                        }
                    }
                }
                Expression::Struct(type_path, body) => {
                    let factory = build_struct(body);

                    if let Some(type_path) = type_path.as_ref() {
                        let type_path = resolve_alias(&aliases, type_path);
                        let type_or_variant = lookup_type_or_variant(type_registry, &type_path)
                            .ok_or_else(|| anyhow!("unknown type: {type_path}"))?;

                        match type_or_variant {
                            TypeOrVariant::Type(id) => {
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {type_path:?}"))?;
                                let converter = ValueConverter::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = Box::new(factory(ctx, registers)?);
                                        let value = converter.convert(ctx, value)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                            TypeOrVariant::Variant(id, variant_name) => {
                                let variant_name = variant_name.to_string();
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {type_path:?}"))?;
                                let converter = ValueConverter::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = factory(ctx, registers)?;
                                        let value = DynamicEnum::new(&variant_name, DynamicVariant::Struct(value));
                                        let value = Box::new(value);
                                        let value = converter.convert(ctx, value)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                        }
                    } else if let Some(type_reg) = register_type_reg {
                        let converter = ValueConverter::from_registration(type_reg);
                        steps.push(Box::new(move |ctx, registers, _| {
                            if registers[index].is_none() {
                                let value = Box::new(factory(ctx, registers)?);
                                let value = converter.convert(ctx, value)?;
                                registers[index] = Some(value.into());
                            }
                            Ok(())
                        }));
                    } else {
                        steps.push(Box::new(move |ctx, registers, _| {
                            if registers[index].is_none() {
                                registers[index] = Some(Arc::new(factory(ctx, registers)?));
                            }
                            Ok(())
                        }));
                    }
                }
                Expression::List(type_path, body) => {
                    let factory = build_list(body);

                    if let Some(type_path) = type_path.as_ref() {
                        let type_path = resolve_alias(&aliases, type_path);
                        let type_id = lookup_type(type_registry, &type_path)
                            .ok_or_else(|| anyhow!("unknown type: {type_path}"))?;
                        let type_reg = type_registry.get(type_id)
                            .ok_or_else(|| anyhow!("no registration for list type: {type_path}"))?;

                        let converter = ValueConverter::from_registration(type_reg);
                        let evaluator = Evaluator::from_registration(type_reg);
                        steps.push(Box::new(move |ctx, registers, _| {
                            if registers[index].is_none() {
                                let value = Box::new(factory(ctx, registers)?);
                                let value = converter.convert(ctx, value)?;
                                let value = evaluator.evaluate(ctx, value)?;
                                registers[index] = Some(value.into());
                            }
                            Ok(())
                        }));
                    } else {
                        steps.push(Box::new(move |ctx, registers, _| {
                            if registers[index].is_none() {
                                registers[index] = Some(Arc::new(factory(ctx, registers)?));
                            }
                            Ok(())
                        }));
                    }
                }
                Expression::Path(path) => {
                    let path = resolve_alias(&aliases, path);

                    if path.len() == 1 {
                        if let Some(fabricator) = file_imports.get(path.0[0]) {
                            let fabricator = fabricator.clone();
                            steps.push(Box::new(move |_, registers, _| {
                                if registers[index].is_none() {
                                    registers[index] = Some(fabricator.clone());
                                }
                                Ok(())
                            }));
                            continue;
                        }

                        if let Some(source_index) = locals.get(path.0[0]) {
                            let source = *source_index;
                            steps.push(Box::new(move |_, registers, _| {
                                if registers[index].is_none() {
                                    let value = registers[source].clone();
                                    registers[index] = value;
                                }
                                Ok(())
                            }));
                            continue;
                        }
                    }

                    if let Some(type_or_variant) = lookup_type_or_variant(type_registry, &path) {
                        match type_or_variant {
                            TypeOrVariant::Type(id) => {
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {path:?}"))?;
                                let constructor = Constructor::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = constructor.construct(ctx)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                            TypeOrVariant::Variant(id, variant_name) => {
                                let variant_name = variant_name.to_string();
                                let type_reg = type_registry.get(id)
                                    .ok_or_else(|| anyhow!("missing type registry for {path:?}"))?;
                                let converter = ValueConverter::from_registration(type_reg);
                                let evaluator = Evaluator::from_registration(type_reg);
                                steps.push(Box::new(move |ctx, registers, _| {
                                    if registers[index].is_none() {
                                        let value = DynamicEnum::new(&variant_name, DynamicVariant::Unit);
                                        let value = Box::new(value);
                                        let value = converter.convert(ctx, value)?;
                                        let value = evaluator.evaluate(ctx, value)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                            }
                        }

                        continue;
                    }

                    bail!("unknown path {path:?}");
                }
                Expression::Import(_) => {}
            }
        }
    }

    for (index, application) in doc.applications.iter().enumerate() {
        let source = application.expression;
        let target = application.entity;
        let type_id = register_types[source];
        let type_info = type_id.and_then(|id| type_registry.get(id))
            .ok_or_else(|| anyhow!("missing apply operand type info for %{source} in application {index}, type id {type_id:?}"))?;
        let applicator = Applicator::from_registration(type_info);

        steps.push(Box::new(move |ctx, registers, _| {
            let Some(source_value) = &registers[source] else {
                bail!("apply source null");
            };
            let Some(target_value) = &registers[target] else {
                bail!("apply target null");
            };
            let Some(entity) = Entity::from_reflect(target_value.as_ref()) else {
                bail!("apply target was not entity: {target_value:?}");
            };
            applicator.apply(ctx, source_value.as_ref(), entity)
        }));
    }

    let num_registers = doc.registers.len();
    let fabricate = move |entity: Entity, input: &dyn PartialReflect, world: &mut World| {
        let mut registers: Vec<Option<Arc<dyn PartialReflect>>> = Vec::with_capacity(num_registers + 1);
        registers.extend(std::iter::repeat_with(|| None).take(num_registers));
        registers.push(Some(Arc::new(entity)));

        // Apply inputs
        match input.reflect_ref() {
            ReflectRef::Struct(struct_input) => {
                for (name, (index, required)) in &inputs {
                    if let Some(field) = struct_input.field(name) {
                        registers[*index] = Some(field.clone_value().into());
                    } else if *required {
                        bail!("missing required input '{name}'");
                    }
                }
            }
            ReflectRef::Tuple(tuple_input) => {
                if tuple_input.field_len() != 0 || requires_an_input {
                    bail!("input was not a struct, got {input:?}");
                }
            }
            ReflectRef::Map(map_input) => {
                for (name, (index, required)) in &inputs {
                    if let Some(field) = map_input.get(name) {
                        registers[*index] = Some(field.clone_value().into());
                    } else if *required {
                        bail!("missing required input '{name}'");
                    }
                }
            }
            _ => bail!("input was not a struct, got {input:?}"),
        }

        let mut ctx = Context {
            world,
            fabricated: Fabricated::default(),
        };

        for step in &steps {
            step(&mut ctx, &mut registers, entity)?;
        }

        // Debug print registers
        if tracing::enabled!(Level::TRACE) {
            for (index, v) in registers.iter().enumerate() {
                trace!("%{index} = {v:?}");
            }
        }

        Ok(ctx.fabricated)
    };

    Ok(Fabricator {
        parameters,
        factory: Arc::new(fabricate),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply() {
        use crate::operations::Spawn;

        let doc = Document::parse("
            import bevy_hierarchy::components::parent::Parent;
            import bevy_transform::components::transform::Transform;
            import bevy_ecs::entity::Entity;
            import bevy_fabricator::operations::Spawn;
            in param1: f32 = 5.0;
            in param2: f32? = 0.4;
            local child = Spawn();
            local test = Transform {
                translation: (3.0, 0.2, 0),
            };
            child <- Parent($);
            child <- test;
        ").unwrap();
        println!("doc = {doc:?}");
        let app_type_registry = AppTypeRegistry::default();
        let mut type_registry = app_type_registry.write();
        type_registry.register::<Parent>();
        type_registry.register::<Transform>();
        type_registry.register::<Spawn>();
        let fabricator = convert(&type_registry, &FabricatorMap::default(), &doc).unwrap();
        drop(type_registry);

        let mut world = World::new();
        world.insert_resource(app_type_registry);
        let target = world.spawn_empty().id();

        #[derive(Reflect)]
        struct Params {
            pub param1: f32,
        }

        let params = Params {
            param1: 42.,
        };

        fabricator.fabricate(&params, &mut world, target).unwrap();

        let mut found_child = false;
        for entity in world.iter_entities() {
            let Some(parent) = entity.get::<Parent>() else { continue };
            assert_eq!(parent.get(), target);

            let transform = entity.get::<Transform>().expect("should have transform");
            assert_eq!(transform.translation.x, 3.);

            assert!(!found_child);
            found_child = true;
        }
        assert!(found_child);
    }
}
