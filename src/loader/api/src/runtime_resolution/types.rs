//! Reflection traversal without Lua or process access.
use indexmap::IndexMap;
use kfc::reflection::{PrimitiveType, StructFieldMetadata, TypeIndex, TypeRegistry};
use std::collections::HashSet;

pub(crate) fn inheritance(
    registry: &TypeRegistry,
    index: TypeIndex,
) -> Result<Vec<TypeIndex>, String> {
    let mut result = Vec::new();
    let mut current = Some(index);
    let mut visited = HashSet::new();
    while let Some(index) = current {
        if !visited.insert(index) {
            return Err("cyclic type inheritance".into());
        }
        let ty = registry.get(index).ok_or("unresolved base type")?;
        result.push(index);
        // Array/optional inner types are element types, not base classes.
        current = if matches!(
            ty.primitive_type,
            PrimitiveType::Struct | PrimitiveType::Typedef
        ) {
            ty.inner_type
        } else {
            None
        };
    }
    Ok(result)
}

pub(crate) fn fields(
    registry: &TypeRegistry,
    index: TypeIndex,
) -> Result<Vec<(TypeIndex, &StructFieldMetadata)>, String> {
    let mut result = IndexMap::new();
    for owner in inheritance(registry, index)?.into_iter().rev() {
        let ty = registry.get(owner).ok_or("unresolved declaring type")?;
        for field in ty.struct_fields.values() {
            result.insert(field.name.as_str(), (owner, field));
        }
    }
    Ok(result.into_values().collect())
}
