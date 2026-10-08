use kfc::reflection::{TypeMetadata, TypeRegistry};

#[derive(Clone, Copy)]
pub(crate) enum HashKind {
    Qualified,
    Internal,
    Name,
    Impact,
}

impl HashKind {
    pub(crate) fn parse(name: &str) -> Result<Self, String> {
        match name {
            "qualified" => Ok(Self::Qualified),
            "internal" => Ok(Self::Internal),
            "name" => Ok(Self::Name),
            "impact" => Ok(Self::Impact),
            _ => Err(format!(
                "unknown hash domain '{name}'; expected qualified, internal, name or impact"
            )),
        }
    }

    pub(crate) fn value(self, ty: &TypeMetadata) -> u32 {
        match self {
            Self::Qualified => ty.qualified_hash,
            Self::Internal => ty.internal_hash,
            Self::Name => ty.name_hash,
            Self::Impact => ty.impact_hash,
        }
    }
}

/// Internal/layout hashes are frequently shared. Never choose the first collision.
pub(crate) fn by_hash(
    registry: &TypeRegistry,
    hash: u32,
    kind: HashKind,
) -> Result<Option<&TypeMetadata>, String> {
    unique_match(registry.iter().filter(|ty| kind.value(ty) == hash)).map_err(|()| {
        format!("ambiguous type hash 0x{hash:08x}; use a qualified name or qualified hash")
    })
}

fn unique_match<T>(mut values: impl Iterator<Item = T>) -> Result<Option<T>, ()> {
    let first = values.next();
    if values.next().is_some() {
        Err(())
    } else {
        Ok(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_collision_never_selects_an_arbitrary_type() {
        assert_eq!(unique_match(["client", "server"].into_iter()), Err(()));
        assert_eq!(unique_match(["exact"].into_iter()), Ok(Some("exact")));
        assert_eq!(unique_match(std::iter::empty::<&str>()), Ok(None));
    }

    #[test]
    fn qualified_hash_resolves_while_shared_internal_hash_is_rejected() {
        let types: Vec<_> = (0..2)
            .map(|index| {
                serde_json::json!({
                    "index": index, "name": format!("Type{index}"),
                    "impactName": format!("ecs.Type{index}"),
                    "qualifiedName": format!("keen::ecs::Type{index}"),
                    "size": 4, "alignment": 4, "elementAlignment": 4,
                    "fieldCount": 0, "primitiveType": "STRUCT", "flags": "",
                    "nameHash": 100 + index, "impactHash": 200 + index,
                    "qualifiedHash": 300 + index, "internalHash": 777
                })
            })
            .collect();
        let registry: TypeRegistry = serde_json::from_value(serde_json::json!({
            "version": "test", "types": types
        }))
        .unwrap();
        assert_eq!(
            by_hash(&registry, 301, HashKind::Qualified)
                .unwrap()
                .unwrap()
                .qualified_name,
            "keen::ecs::Type1"
        );
        assert!(
            by_hash(&registry, 302, HashKind::Qualified)
                .unwrap()
                .is_none()
        );
        assert!(
            by_hash(&registry, 777, HashKind::Internal)
                .unwrap_err()
                .contains("ambiguous")
        );
    }
}
