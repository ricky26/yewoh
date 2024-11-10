use anyhow::{anyhow, bail};
use bevy::prelude::*;
use bevy::reflect::ReflectRef;

use crate::traits::{Convert, ReflectConvert};

macro_rules! impl_vec3 {
    ($vec:ident, $ty:ty) => {
        impl Convert for $vec {
            fn convert(from: Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>> {
                fn from_components(
                    x: &dyn PartialReflect,
                    y: &dyn PartialReflect,
                    z: &dyn PartialReflect,
                ) -> anyhow::Result<Box<dyn PartialReflect>> {
                    let x = <$ty>::from_reflect(x)
                        .ok_or_else(|| anyhow!("failed convert x from {x:?}"))?;
                    let y = <$ty>::from_reflect(y)
                        .ok_or_else(|| anyhow!("failed convert y from {x:?}"))?;
                    let z = <$ty>::from_reflect(z)
                        .ok_or_else(|| anyhow!("failed convert z from {x:?}"))?;
                    Ok(Box::new($vec { x, y, z }))
                }

                let from = match from.try_downcast::<$vec>() {
                    Ok(value) => return Ok(value),
                    Err(value) => value,
                };

                match from.reflect_ref() {
                    ReflectRef::Struct(_) => {
                        if let Some(value) = <$vec>::from_reflect(from.as_ref()) {
                            return Ok(Box::new(value));
                        }
                    }
                    ReflectRef::TupleStruct(value) => {
                        if value.field_len() == 3 {
                            let x = value.field(0).unwrap();
                            let y = value.field(1).unwrap();
                            let z = value.field(2).unwrap();
                            return from_components(x, y, z);
                        }
                    }
                    ReflectRef::Tuple(value) => {
                        if value.field_len() == 3 {
                            let x = value.field(0).unwrap();
                            let y = value.field(1).unwrap();
                            let z = value.field(2).unwrap();
                            return from_components(x, y, z);
                        }
                    }
                    _ => {}
                }

                bail!(concat!("cannot convert from {from:?} to ", stringify!($vec)));
            }
        }
    };
}

macro_rules! impl_vec2 {
    ($vec:ident, $ty:ty) => {
        impl Convert for $vec {
            fn convert(from: Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>> {
                fn from_components(
                    x: &dyn PartialReflect,
                    y: &dyn PartialReflect,
                ) -> anyhow::Result<Box<dyn PartialReflect>> {
                    let x = <$ty>::from_reflect(x)
                        .ok_or_else(|| anyhow!("failed convert x from {x:?}"))?;
                    let y = <$ty>::from_reflect(y)
                        .ok_or_else(|| anyhow!("failed convert y from {x:?}"))?;
                    Ok(Box::new($vec { x, y }))
                }

                let from = match from.try_downcast::<$vec>() {
                    Ok(value) => return Ok(value),
                    Err(value) => value,
                };

                match from.reflect_ref() {
                    ReflectRef::Struct(_) => {
                        if let Some(value) = <$vec>::from_reflect(from.as_ref()) {
                            return Ok(Box::new(value));
                        }
                    }
                    ReflectRef::TupleStruct(value) => {
                        if value.field_len() == 2 {
                            let x = value.field(0).unwrap();
                            let y = value.field(1).unwrap();
                            return from_components(x, y);
                        }
                    }
                    ReflectRef::Tuple(value) => {
                        if value.field_len() == 2 {
                            let x = value.field(0).unwrap();
                            let y = value.field(1).unwrap();
                            return from_components(x, y);
                        }
                    }
                    _ => {}
                }

                bail!(concat!("cannot convert from {from:?} to ", stringify!($vec)));
            }
        }
    };
}

impl_vec3!(Vec3, f32);
impl_vec3!(IVec3, i32);
impl_vec3!(UVec3, u32);

impl_vec2!(Vec2, f32);
impl_vec2!(IVec2, i32);
impl_vec2!(UVec2, u32);

pub fn register(app: &mut App) {
    app
        .register_type::<Vec3>()
        .register_type::<IVec3>()
        .register_type::<UVec3>()
        .register_type::<Vec2>()
        .register_type::<IVec2>()
        .register_type::<UVec2>()
        .register_type_data::<Vec3, ReflectConvert>()
        .register_type_data::<IVec3, ReflectConvert>()
        .register_type_data::<UVec3, ReflectConvert>()
        .register_type_data::<Vec2, ReflectConvert>()
        .register_type_data::<IVec2, ReflectConvert>()
        .register_type_data::<UVec2, ReflectConvert>();
}
