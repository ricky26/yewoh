use std::any::type_name;
use anyhow::{anyhow, bail};
use bevy::prelude::*;

pub fn reflect_optional_field<T: FromReflect>(
    s: &dyn Struct, field_name: &str,
) -> anyhow::Result<Option<T>> {
    let Some(value) = s.field(field_name) else {
        return Ok(None);
    };
    let value = T::from_reflect(value)
        .ok_or_else(|| anyhow!("could not parse field '{field_name}' as {} from {value:?}", type_name::<T>()))?;
    Ok(Some(value))
}

pub fn reflect_field<T: FromReflect>(s: &dyn Struct, field_name: &str) -> anyhow::Result<T> {
    let value = reflect_optional_field(s, field_name)?
        .ok_or_else(|| anyhow!("missing field '{field_name}'"))?;
    Ok(value)
}

pub fn assert_struct_fields(s: &dyn Struct, field_names: &[&str]) -> anyhow::Result<()> {
    for field_index in 0..s.field_len() {
        let name = s.name_at(field_index).expect("struct field missing name");
        if !field_names.iter().any(|n| *n == name) {
            bail!("unknown struct field {name}");
        }
    }

    Ok(())
}
