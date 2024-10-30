use std::any::TypeId;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use bevy::log::Level;
use bevy::prelude::*;
use bevy::reflect::{DynamicArray, DynamicEnum, DynamicList, DynamicStruct, DynamicTuple, DynamicTupleStruct, ReflectRef, TypeInfo, TypeRegistration, TypeRegistry, VariantInfo};
use bevy::utils::{tracing, HashMap};
use smallvec::SmallVec;

use crate::document::{Document, Expression, Import, Number, Path, Visibility};
use crate::string::parse_string;
use crate::traits::{ReflectEvaluate, ReflectApply};
use crate::{Fabricated, FabricationParameter, Fabricator};
use crate::parser::FormatterFn;

type RegisterValue = Option<Arc<dyn PartialReflect>>;
type RegisterValues = Vec<RegisterValue>;

fn lookup_type(type_registry: &TypeRegistry, path: &Path) -> Option<TypeId> {
    let full_name = path.to_string();
    type_registry.get_with_type_path(&full_name).map(|r| r.type_id())
}

fn lookup_type_or_variant(type_registry: &TypeRegistry, path: &Path) -> Option<TypeId> {
    if path.len() == 1 {
        lookup_type(type_registry, path)
    } else {
        let full_name = path.to_string();
        let variant_len = path.0.last().unwrap().len();
        let enum_name = &full_name[..(full_name.len() - variant_len - 2)];
        type_registry.get_with_type_path(&full_name)
            .or_else(|| type_registry.get_with_type_path(enum_name))
            .map(|r| r.type_id())
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
    ($steps:expr, $index:expr, $type_id:expr, $n:expr, $ty:ty) => {
        if $type_id == Some(TypeId::of::<$ty>()) {
            let value = Arc::new(match $n {
                Number::I64(v) => *v as $ty,
                Number::U64(v) => *v as $ty,
                Number::F64(v) => *v as $ty,
            });
            let index = $index;
            $steps.push(Box::new(move |registers, _, _, _| {
                if registers[index].is_none() {
                    registers[index] = Some(value.clone());
                }
                Ok(())
            }));
            continue;
        }
    };
}

struct ValueConverter {
    type_path: &'static str,
    from_reflect: ReflectFromReflect,
}

impl ValueConverter {
    pub fn try_from_registration(type_registration: &TypeRegistration) -> anyhow::Result<ValueConverter> {
        let type_path = type_registration.type_info().ty().path();
        let from_reflect = type_registration.data::<ReflectFromReflect>()
            .cloned()
            .ok_or_else(|| anyhow!("FromReflect not implemented for {type_path}"))?;

        Ok(ValueConverter {
            type_path,
            from_reflect,
        })
    }

    pub fn convert(
        &self,
        value: &dyn PartialReflect,
        _world: &mut World,
    ) -> anyhow::Result<Box<dyn PartialReflect>> {
        let value = self.from_reflect.from_reflect(value)
            .ok_or_else(|| anyhow!("failed to reflect {} with value {value:?}", self.type_path))
            ?.into_partial_reflect();
        Ok(value)
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
        src: Box<dyn PartialReflect>,
        world: &mut World,
        fabricate: &mut Fabricated,
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

            Ok(evaluate.evaluate(world, fabricate)?.into())
        } else {
            Ok(src.into())
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
        src: &Arc<dyn PartialReflect>,
        world: &mut World,
        entity: Entity,
        fabricated: &mut Fabricated,
    ) -> anyhow::Result<()> {
        if let Some(reflect_apply) = &self.apply {
            let Some(src) = src.as_ref().try_as_reflect() else {
                bail!("{src:?} is not a concrete type");
            };

            let Some(apply) = reflect_apply.get(src) else {
                bail!("{src:?} does not implement Command");
            };

            apply.apply(world, entity, fabricated)?;
        } else if let Some(reflect_component) = &self.component {
            let type_registry = world.resource::<AppTypeRegistry>().clone();
            let type_registry = type_registry.read();
            let mut entity_mut = world.entity_mut(entity);
            reflect_component.insert(&mut entity_mut, src.as_ref(), &*type_registry);
        } else {
            bail!("unknown apply type: {}", self.type_path);
        }

        Ok(())
    }
}

fn build_dynamic_struct<'a>(
    body: &[(&'a str, usize)],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let body = body.iter()
        .map(|(k, v)| Ok((k.to_string(), *v)))
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, _world: &mut World, _fabricated: &mut Fabricated| {
        let mut value = DynamicStruct::default();
        for (key, src) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = (**field_value).clone_value();
            value.insert_boxed(key, field_value);
        }
        Ok(Box::new(value) as Box<dyn PartialReflect>)
    })
}

fn build_dynamic_tuple<'a>(
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let body = body.iter()
        .copied()
        .collect::<SmallVec<[_; 8]>>();

    Ok(move |registers: &mut RegisterValues, _world: &mut World, _fabricated: &mut Fabricated| {
        let mut value = DynamicTuple::default();
        for src in body.iter().copied() {
            let field_value = registers[src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", src))?;
            let field_value = (**field_value).clone_value();
            value.insert_boxed(field_value);
        }
        Ok(Box::new(value) as Box<dyn PartialReflect>)
    })
}

fn build_struct<'a>(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[(&'a str, usize)],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing struct type info"))?;
    let struct_info = type_reg.type_info().as_struct()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .map(|(k, v)| {
            let field = struct_info.field(*k)
                .ok_or_else(|| anyhow!("missing field {k}"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((k.to_string(), *v, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicStruct::default();
        for (key, src, converter) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(key, field_value);
        }

        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_struct_from_tuple(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing struct type info"))?;
    let struct_info = type_reg.type_info().as_struct()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .enumerate()
        .map(|(field_index, register_index)| {
            let field = struct_info.field_at(field_index)
                .ok_or_else(|| anyhow!("field {field_index} is out of range"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((field.name(), *register_index, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicStruct::default();
        for (key, src, converter) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(*key, field_value);
        }

        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_tuple_struct(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing tuple struct type info"))?;
    let struct_info = type_reg.type_info().as_tuple_struct()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .enumerate()
        .map(|(field_index, register_index)| {
            let field = struct_info.field_at(field_index)
                .ok_or_else(|| anyhow!("field {field_index} is out of range"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((*register_index, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicTupleStruct::default();
        for (src, converter) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(field_value);
        }

        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_tuple(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing tuple type info"))?;
    let struct_info = type_reg.type_info().as_tuple()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .enumerate()
        .map(|(field_index, register_index)| {
            let field = struct_info.field_at(field_index)
                .ok_or_else(|| anyhow!("field {field_index} is out of range"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((*register_index, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicTuple::default();
        for (src, converter) in body.iter() {
            let field_value = registers[*src].as_ref()
                .ok_or_else(|| anyhow!("unfilled register {}", *src))?;
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(field_value);
        }

        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_enum_tuple(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    variant: &str,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing tuple struct type info"))?;
    let enum_info = type_reg.type_info().as_enum()?;
    let variant = enum_info.variant(variant)
        .ok_or_else(|| anyhow!("unknown variant {}", enum_info.variant_path(variant)))?;
    let variant_name = variant.name();
    let struct_info = variant.as_tuple_variant()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .enumerate()
        .map(|(field_index, register_index)| {
            let field = struct_info.field_at(field_index)
                .ok_or_else(|| anyhow!("field {field_index} is out of range"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((*register_index, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicTuple::default();
        for (src, converter) in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(field_value);
        }

        let value = DynamicEnum::new(variant_name, value);
        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_enum_struct<'a>(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    variant: &str,
    body: &[(&'a str, usize)],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing tuple struct type info"))?;
    let enum_info = type_reg.type_info().as_enum()?;
    let variant = enum_info.variant(variant)
        .ok_or_else(|| anyhow!("unknown variant {}", enum_info.variant_path(variant)))?;
    let variant_name = variant.name();
    let struct_info = variant.as_struct_variant()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .map(|(k, v)| {
            let field = struct_info.field(*k)
                .ok_or_else(|| anyhow!("missing field {k}"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((k.to_string(), *v, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicStruct::default();
        for (key, src, converter) in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(key, field_value);
        }

        let value = DynamicEnum::new(variant_name, value);
        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_enum_struct_from_tuple<'a>(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    variant: &str,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing tuple struct type info"))?;
    let enum_info = type_reg.type_info().as_enum()?;
    let variant = enum_info.variant(variant)
        .ok_or_else(|| anyhow!("unknown variant {}", enum_info.variant_path(variant)))?;
    let variant_name = variant.name();
    let struct_info = variant.as_struct_variant()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.iter()
        .enumerate()
        .map(|(field_index, register_index)| {
            let field = struct_info.field_at(field_index)
                .ok_or_else(|| anyhow!("field {field_index} is out of range"))?;
            let field_reg = type_registry.get(field.type_id())
                .ok_or_else(|| anyhow!("unregistered type {}", field.type_path()))?;
            let converter = ValueConverter::try_from_registration(field_reg)?;
            Ok((field.name(), *register_index, converter))
        })
        .collect::<anyhow::Result<SmallVec<[_; 8]>>>()?;

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicStruct::default();
        for (key, src, converter) in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = converter.convert(field_value.as_ref(), world)?;
            value.insert_boxed(*key, field_value);
        }

        let value = DynamicEnum::new(variant_name, value);
        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_list(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing list type info"))?;
    let list_info = type_reg.type_info().as_list()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let element_type_id = list_info.item_ty().id();
    let element_reg = type_registry.get(element_type_id)
        .ok_or_else(|| anyhow!("missing list element type info"))?;
    let element_converter = ValueConverter::try_from_registration(element_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.to_vec();

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut value = DynamicList::default();
        for src in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = element_converter.convert(field_value.as_ref(), world)?;
            value.push_box(field_value);
        }

        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

fn build_array(
    type_registry: &TypeRegistry,
    type_id: TypeId,
    body: &[usize],
) -> anyhow::Result<impl Fn(&mut RegisterValues, &mut World, &mut Fabricated) -> anyhow::Result<Box<dyn PartialReflect>>> {
    let type_reg = type_registry.get(type_id)
        .ok_or_else(|| anyhow!("missing array type info"))?;
    let array_info = type_reg.type_info().as_array()?;
    let converter = ValueConverter::try_from_registration(type_reg)?;
    let element_type_id = array_info.item_ty().id();
    let element_reg = type_registry.get(element_type_id)
        .ok_or_else(|| anyhow!("missing array element type info"))?;
    let element_converter = ValueConverter::try_from_registration(element_reg)?;
    let evaluator = Evaluator::from_registration(type_reg);
    let body = body.to_vec();

    Ok(move |registers: &mut RegisterValues, world: &mut World, fabricated: &mut Fabricated| {
        let mut values = Vec::new();
        for src in body.iter() {
            let Some(field_value) = &registers[*src] else { continue };
            let field_value = element_converter.convert(field_value.as_ref(), world)?;
            values.push(field_value);
        }

        let value = DynamicArray::from_iter(values);
        let value = converter.convert(&value, world)?;
        Ok(evaluator.evaluate(value, world, fabricated)?)
    })
}

pub fn convert(
    type_registry: &TypeRegistry,
    documents: &dyn FabricatorSource,
    doc: &Document,
) -> anyhow::Result<Fabricator> {
    type Step = Box<dyn Fn(&mut RegisterValues, Entity, &mut World, &mut Fabricated) -> anyhow::Result<()> + Send + Sync>;

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

            match &register.expression {
                Some(Expression::Import(import)) => {
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
                _ => {}
            }
        }
    }

    // Second pass: lookup types
    for (index, register) in doc.registers.iter().enumerate() {
        let mut register_type = register.variable_type.as_ref()
            .map(|path| resolve_alias(&aliases, &path))
            .map(|path| lookup_type(type_registry, &path)
                .ok_or_else(|| anyhow!("no such type {path:?}")))
            .transpose()?;

        match &register.expression {
            Some(Expression::Struct(Some(path), _)) => {
                let path = resolve_alias(&aliases, path);
                let id = lookup_type_or_variant(type_registry, &path)
                    .ok_or_else(|| anyhow!("unknown type {path:?}"))?;
                register_type = Some(id);
            }
            Some(Expression::Tuple(Some(path), _)) => {
                let path = resolve_alias(&aliases, path);
                let id = lookup_type_or_variant(type_registry, &path)
                    .ok_or_else(|| anyhow!("unknown type {path:?}"))?;
                register_type = Some(id);
            }
            Some(Expression::Path(path)) => {
                let path = resolve_alias(&aliases, path);

                if path.len() == 1 {
                    if let Some(source) = locals.get(path.0[0]) {
                        register_type = register_type.or(register_types[*source]);
                    }
                }

                if let Some(id) = lookup_type_or_variant(type_registry, &path) {
                    register_type = register_type.or(Some(id));
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
                                let type_path = resolve_alias(&aliases, &type_path);
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
                                let Some(field_info) = struct_info.field(*field_name) else {
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
                                let type_path = resolve_alias(&aliases, &type_path);
                                let variant = enum_info.variant(type_path.0.last().unwrap())
                                    .ok_or_else(|| anyhow!("unknown variant {type_path:?}"))?;
                                match variant {
                                    VariantInfo::Struct(struct_info) => {
                                        for (field_name, register_index) in body.iter() {
                                            let Some(field_info) = struct_info.field(*field_name) else {
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
                                    _ => {}
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
        match (register.visibility, register.name, register_type) {
            (Visibility::In, Some(name), Some(ty)) => {
                parameters.insert(name.to_string(), FabricationParameter {
                    parameter_type: *ty,
                    optional: register.optional,
                });
            }
            _ => {}
        }

        let register_type_id = register_types[index];
        let register_type_reg = register_type_id.and_then(|id| type_registry.get(id));

        if let Some(value) = &register.expression {
            match value {
                Expression::Number(n) => {
                    impl_load_number!(steps, index, register_type_id, n, u8);
                    impl_load_number!(steps, index, register_type_id, n, i8);
                    impl_load_number!(steps, index, register_type_id, n, u16);
                    impl_load_number!(steps, index, register_type_id, n, i16);
                    impl_load_number!(steps, index, register_type_id, n, u32);
                    impl_load_number!(steps, index, register_type_id, n, i32);
                    impl_load_number!(steps, index, register_type_id, n, f32);

                    let value = Arc::new(match n {
                        Number::I64(v) => *v as f64,
                        Number::U64(v) => *v as f64,
                        Number::F64(v) => *v,
                    });
                    steps.push(Box::new(move |registers, _, _, _| {
                        if registers[index].is_none() {
                            registers[index] = Some(value.clone());
                        }
                        Ok(())
                    }));
                }
                Expression::String(s) => {
                    let (_, value) = parse_string(s).unwrap().unwrap();
                    let value = Arc::new(value);
                    steps.push(Box::new(move |registers, _, _, _| {
                        if registers[index].is_none() {
                            registers[index] = Some(value.clone());
                        }
                        Ok(())
                    }));
                }
                Expression::Tuple(type_path, body) => {
                    let type_reg = register_type_reg
                        .ok_or_else(|| anyhow!("missing tuple type info"))?;
                    let type_id = register_type_id.unwrap();

                    match type_reg.type_info() {
                        TypeInfo::Struct(_) => {
                            let factory = build_struct_from_tuple(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        TypeInfo::TupleStruct(_) => {
                            let factory = build_tuple_struct(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        TypeInfo::Tuple(_) => {
                            let factory = build_tuple(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        TypeInfo::Enum(enum_info) => {
                            let type_path = type_path.as_ref().unwrap();
                            let type_path = resolve_alias(&aliases, type_path);
                            let variant_name = type_path.0.last().unwrap();
                            let variant = enum_info.variant(variant_name)
                                .ok_or_else(|| anyhow!("unknown enum variant {}", enum_info.variant_path(variant_name)))?;

                            match variant {
                                VariantInfo::Struct(_) => {
                                    let factory = build_enum_struct_from_tuple(
                                        type_registry, type_id, variant_name, &body)?;
                                    steps.push(Box::new(move |registers, _, world, fabricated| {
                                        if registers[index].is_none() {
                                            registers[index] = Some(factory(registers, world, fabricated)?.into());
                                        }
                                        Ok(())
                                    }));
                                }
                                VariantInfo::Tuple(_) => {
                                    let factory = build_enum_tuple(
                                        type_registry, type_id, variant_name, &body)?;
                                    steps.push(Box::new(move |registers, _, world, fabricated| {
                                        if registers[index].is_none() {
                                            registers[index] = Some(factory(registers, world, fabricated)?.into());
                                        }
                                        Ok(())
                                    }));
                                }
                                _ => unreachable!(),
                            }
                        }
                        _ => {
                            let factory = build_dynamic_tuple(&body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                    }
                }
                Expression::Struct(type_path, body) => {
                    let type_reg = register_type_reg
                        .ok_or_else(|| anyhow!("missing struct type info"))?;
                    let type_id = register_type_id.unwrap();

                    match type_reg.type_info() {
                        TypeInfo::Struct(_) => {
                            let factory = build_struct(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        TypeInfo::Enum(_) => {
                            let type_path = type_path.as_ref().unwrap();
                            let type_path = resolve_alias(&aliases, type_path);
                            let factory = build_enum_struct(
                                type_registry, type_id, type_path.0.last().unwrap(), &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        _ => {
                            let factory = build_dynamic_struct(&body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                    }
                }
                Expression::List(_, body) => {
                    let type_reg = register_type_reg
                        .ok_or_else(|| anyhow!("missing list type info"))?;
                    let type_id = register_type_id.unwrap();

                    match type_reg.type_info() {
                        TypeInfo::List(_) => {
                            let factory = build_list(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        TypeInfo::Array(_) => {
                            let factory = build_array(type_registry, type_id, &body)?;
                            steps.push(Box::new(move |registers, _, world, fabricated| {
                                if registers[index].is_none() {
                                    registers[index] = Some(factory(registers, world, fabricated)?.into());
                                }
                                Ok(())
                            }));
                        }
                        _ => unreachable!(),
                    }
                }
                Expression::Path(path) => {
                    let path = resolve_alias(&aliases, path);

                    if path.len() == 1 {
                        if let Some(fabricator) = file_imports.get(path.0[0]) {
                            let fabricator = fabricator.clone();
                            steps.push(Box::new(move |registers, _, _, _| {
                                if registers[index].is_none() {
                                    registers[index] = Some(fabricator.clone());
                                }
                                Ok(())
                            }));
                            continue;
                        }

                        if let Some(source_index) = locals.get(path.0[0]) {
                            let source = *source_index;
                            steps.push(Box::new(move |registers, _, _, _| {
                                if registers[index].is_none() {
                                    let value = registers[source].clone();
                                    registers[index] = value;
                                }
                                Ok(())
                            }));
                            continue;
                        }
                    }

                    let convert_from = |steps: &mut Vec<Step>, path: &Path, type_reg: &TypeRegistration| -> anyhow::Result<bool> {
                        let converter = ValueConverter::try_from_registration(type_reg)?;
                        let evaluator = Evaluator::from_registration(type_reg);

                        match type_reg.type_info() {
                            TypeInfo::Struct(_) => {
                                steps.push(Box::new(move |registers, _, world, fabricated| {
                                    if registers[index].is_none() {
                                        let value = DynamicStruct::default();
                                        let value = converter.convert(&value, world)?;
                                        let value = evaluator.evaluate(value, world, fabricated)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                                return Ok(true);
                            }
                            TypeInfo::TupleStruct(_) => {
                                steps.push(Box::new(move |registers, _, world, fabricated| {
                                    if registers[index].is_none() {
                                        let value = DynamicTupleStruct::default();
                                        let value = converter.convert(&value, world)?;
                                        let value = evaluator.evaluate(value, world, fabricated)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                                return Ok(true);
                            }
                            TypeInfo::Tuple(_) => {
                                steps.push(Box::new(move |registers, _, world, fabricated| {
                                    if registers[index].is_none() {
                                        let value = DynamicTuple::default();
                                        let value = converter.convert(&value, world)?;
                                        let value = evaluator.evaluate(value, world, fabricated)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                                return Ok(true);
                            }
                            TypeInfo::Enum(enum_info) => {
                                let variant_name = path.0.last().unwrap();
                                let variant = enum_info.variant(variant_name)
                                    .ok_or_else(|| anyhow!("unknown enum variant {}", enum_info.variant_path(variant_name)))?;
                                let variant_name = variant.name();

                                steps.push(Box::new(move |registers, _, world, fabricated| {
                                    if registers[index].is_none() {
                                        let value = DynamicEnum::new(variant_name, ());
                                        let value = converter.convert(&value, world)?;
                                        let value = evaluator.evaluate(value, world, fabricated)?;
                                        registers[index] = Some(value.into());
                                    }
                                    Ok(())
                                }));
                                return Ok(true);
                            }
                            _ => {}
                        }

                        Ok(false)
                    };

                    if let Some(type_reg) = register_type_reg {
                        if convert_from(&mut steps, &path, type_reg)? {
                            continue;
                        }
                    }

                    let value_type_id = lookup_type_or_variant(type_registry, &path);
                    let value_reg = value_type_id.and_then(|id| type_registry.get(id));
                    if let Some(type_reg) = value_reg {
                        if convert_from(&mut steps, &path, type_reg)? {
                            continue;
                        }
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

        steps.push(Box::new(move |registers, _, world, fabricated| {
            let Some(source_value) = &registers[source] else {
                bail!("apply source null");
            };
            let Some(target_value) = &registers[target] else {
                bail!("apply target null");
            };
            let Some(entity) = Entity::from_reflect(target_value.as_ref()) else {
                bail!("apply target was not entity: {target_value:?}");
            };
            applicator.apply(source_value, world, entity, fabricated)
        }));
    }

    let num_registers = doc.registers.len();
    let fabricate = move |entity: Entity, input: &dyn PartialReflect, world: &mut World| {
        let mut fabricated = Fabricated::default();

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
            _ => bail!("input was not a struct, got {input:?}"),
        }

        for step in &steps {
            step(&mut registers, entity, world, &mut fabricated)?;
        }

        // Debug print registers
        if tracing::enabled!(Level::TRACE) {
            for (index, v) in registers.iter().enumerate() {
                trace!("%{index} = {v:?}");
            }
        }

        Ok(fabricated)
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
