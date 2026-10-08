//! Owned KFC values. No process pointers or engine allocation are exposed.
use super::{
    game::value::{convert_lua_to_value, convert_value_to_lua, validate_and_clone_lua_value},
    registry,
};
use crate::{
    alias::{MappedValue, TypeHandle},
    lua::LuaValue,
};
use kfc::reflection::{PrimitiveType, TypeIndex, TypeRegistry};
use std::{collections::HashSet, rc::Rc};

fn supported(
    registry: &TypeRegistry,
    index: TypeIndex,
    seen: &mut HashSet<TypeIndex>,
) -> Result<(), String> {
    if !seen.insert(index) {
        return Ok(());
    }
    let ty = registry.get(index).ok_or("unresolved value type")?;
    if matches!(
        ty.primitive_type,
        PrimitiveType::DsArray
            | PrimitiveType::DsString
            | PrimitiveType::DsOptional
            | PrimitiveType::DsVariant
    ) {
        return Err(format!(
            "{} requires engine allocation/ownership; the KFC value codec cannot construct it",
            ty.qualified_name
        ));
    }
    registry::inheritance(registry, index)?;
    if let Some(inner) = ty.inner_type {
        supported(registry, inner, seen)?;
    }
    for field in ty.struct_fields.values() {
        supported(registry, field.r#type, seen)?;
    }
    Ok(())
}

fn handle(registry: &Rc<TypeRegistry>, selector: &LuaValue) -> Result<TypeHandle, String> {
    let index = registry::resolve(registry, selector)?;
    validate_owned_type(registry, index)?;
    Ok(TypeHandle::new(registry.clone(), index))
}

pub(crate) fn validate_owned_type(registry: &TypeRegistry, index: TypeIndex) -> Result<(), String> {
    supported(registry, index, &mut HashSet::new())
}

fn decode(lua: &mlua::Lua, ty: &TypeHandle, bytes: &[u8]) -> Result<LuaValue, String> {
    if bytes.len() < ty.size as usize {
        return Err(format!(
            "truncated {}: expected at least {} bytes",
            ty.qualified_name, ty.size
        ));
    }
    let owned: Rc<[u8]> = bytes.into();
    let value =
        MappedValue::from_bytes(ty.type_registry(), ty, &owned).map_err(|e| e.to_string())?;
    convert_value_to_lua(&value, lua).map_err(|e| e.to_string())
}

fn outcome(result: Result<LuaValue, String>) -> (Option<LuaValue>, Option<String>) {
    match result {
        Ok(value) => (Some(value), None),
        Err(reason) => (None, Some(reason)),
    }
}

pub(crate) fn create(lua: &mlua::Lua, registry: Rc<TypeRegistry>) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    let registry_decode = registry.clone();
    table.set(
        "decode",
        lua.create_function(move |lua, (selector, bytes): (LuaValue, mlua::String)| {
            Ok(outcome(
                handle(&registry_decode, &selector)
                    .and_then(|ty| decode(lua, &ty, &bytes.as_bytes())),
            ))
        })?,
    )?;
    let registry_new = registry.clone();
    table.set(
        "new",
        lua.create_function(move |lua, selector: LuaValue| {
            Ok(outcome(handle(&registry_new, &selector).and_then(|ty| {
                let bytes = ty
                    .default_value
                    .as_ref()
                    .ok_or_else(|| format!("no reflected default for {}", ty.qualified_name))?;
                decode(lua, &ty, bytes)
            })))
        })?,
    )?;
    table.set(
        "encode",
        lua.create_function(move |lua, (selector, value): (LuaValue, LuaValue)| {
            let result = handle(&registry, &selector).and_then(|ty| {
                let checked =
                    validate_and_clone_lua_value(&value, &ty, lua).map_err(|e| e.to_string())?;
                let value = convert_lua_to_value(&checked, &ty).map_err(|e| e.to_string())?;
                let bytes = value.to_bytes(&registry, &ty).map_err(|e| e.to_string())?;
                lua.create_string(bytes)
                    .map(LuaValue::String)
                    .map_err(|e| e.to_string())
            });
            Ok(outcome(result))
        })?,
    )?;
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_values_round_trip_inherited_fields_defaults_and_validate_errors() {
        let lua = mlua::Lua::new();
        let registry = Rc::new(super::super::registry::tests::fixture());
        lua.globals()
            .set("values", create(&lua, registry).unwrap())
            .unwrap();
        lua.load(
            r#"
            local a, reason = values.new('test::Child')
            assert(a and not reason and a.x == 17 and a.y == 23)
            local b = assert(values.new('test::Child'))
            a.x = 1234
            assert(b.x == 17)
            local bytes = assert(values.encode('test::Child', a))
            assert(#bytes == 8)
            local decoded = assert(values.decode('test::Child', bytes))
            assert(decoded.x == 1234 and decoded.y == 23)
            local missing, why = values.decode('test::Child', 'short')
            assert(missing == nil and why:find('truncated'))
            local invalid, error = values.encode('test::U32', -1)
            assert(invalid == nil and error)
            local ds, ds_error = values.new('test::HasDs')
            assert(ds == nil and ds_error:find('engine allocation'))
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    #[ignore = "set SHROUDFORGE_TEST_REGISTRY to an installed parser cache"]
    fn installed_registries_resolve_all_indices_when_explicitly_requested() {
        use mlua::ObjectLike;
        let path =
            std::env::var("SHROUDFORGE_TEST_REGISTRY").expect("registry cache path required");
        let registry: Rc<TypeRegistry> =
            Rc::new(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap());
        let lua = mlua::Lua::new();
        let definitions = crate::definition::generator::generate(&registry);
        for ty in registry.iter() {
            assert_eq!(
                registry::resolve(&registry, &LuaValue::Integer(ty.qualified_hash as i64)).unwrap(),
                ty.index
            );
            registry::inheritance(&registry, ty.index).unwrap();
            registry::fields(&registry, ty.index).unwrap();
            let value = lua
                .create_userdata(super::super::Type::new(TypeHandle::new(
                    registry.clone(),
                    ty.index,
                )))
                .unwrap();
            assert_eq!(value.get::<u8>("flags_bits").unwrap(), ty.flags.bits());
            assert_eq!(value.get::<usize>("index").unwrap(), ty.index.as_usize());
            assert_eq!(
                value.get::<u32>("qualified_hash").unwrap(),
                ty.qualified_hash
            );
            if ty.primitive_type == PrimitiveType::Struct {
                assert!(
                    definitions.contains(&format!(
                        "---@class {}",
                        ty.qualified_name.replace("::", ".")
                    )),
                    "missing generated type: {}",
                    ty.qualified_name
                );
            }
        }
        eprintln!(
            "Lua metadata verified for all {} types ({})",
            registry.len(),
            registry.version
        );
    }
}
