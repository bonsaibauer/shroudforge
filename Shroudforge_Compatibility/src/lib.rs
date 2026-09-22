//! Internal boundary between parser output and the public ShroudForge API.
//! Mods never receive this object and never select a build profile.

use serde::{Deserialize, Serialize};
use shroudforge_parser::GameSchema;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameContract {
    schema: GameSchema,
    runtime: BTreeMap<String, Availability>,
}

impl GameContract {
    pub fn schema(&self) -> &GameSchema {
        &self.schema
    }

    pub fn into_schema(self) -> GameSchema {
        self.schema
    }

    pub fn runtime(&self, operation: &str) -> Availability {
        self.runtime
            .get(operation)
            .cloned()
            .unwrap_or_else(|| Availability::Unavailable {
                reason: format!(
                    "operation {operation} has not been verified for game build {}",
                    self.schema.game_version
                ),
            })
    }

    pub fn has_runtime(&self, operation: &str) -> bool {
        matches!(self.runtime(operation), Availability::Available)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Compatibility {
    runtime: BTreeMap<String, Availability>,
}

impl Compatibility {
    pub fn new(verified_operations: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            runtime: verified_operations
                .into_iter()
                .map(|name| (name.into(), Availability::Available))
                .collect(),
        }
    }

    pub fn unavailable(mut self, operation: impl Into<String>, reason: impl Into<String>) -> Self {
        self.runtime.insert(
            operation.into(),
            Availability::Unavailable {
                reason: reason.into(),
            },
        );
        self
    }

    pub fn resolve(mut self, schema: GameSchema) -> GameContract {
        let runtime_anchor = schema.types.get("keen::ecs::CurrentTransform");
        let runtime_schema_valid = runtime_anchor
            .is_some_and(|definition| definition.size > 0 && definition.primitive == "Struct");
        if !runtime_schema_valid {
            let reason = "required game type keen::ecs::CurrentTransform is absent or invalid";
            for operation in [
                "runtime.ecs.query",
                "runtime.ecs.resolve",
                "runtime.ecs.read",
                "runtime.ecs.write",
            ] {
                self.runtime.insert(
                    operation.into(),
                    Availability::Unavailable {
                        reason: reason.into(),
                    },
                );
            }
        }
        GameContract {
            schema,
            runtime: self.runtime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_runtime_operation_is_scoped_and_explained() {
        let schema = GameSchema {
            parser: "test".into(),
            game_version: "build-1".into(),
            types: BTreeMap::new(),
            resources: BTreeMap::new(),
        };
        let contract = Compatibility::default().resolve(schema);
        assert!(!contract.has_runtime("runtime.ecs.write"));
        assert!(matches!(
            contract.runtime("runtime.ecs.write"),
            Availability::Unavailable { .. }
        ));
    }

    #[test]
    fn invalid_runtime_schema_disables_only_ecs_operations() {
        let schema = GameSchema {
            parser: "test".into(),
            game_version: "build-1".into(),
            types: BTreeMap::new(),
            resources: BTreeMap::new(),
        };
        let contract =
            Compatibility::new(["runtime.lifecycle", "runtime.ecs.write"]).resolve(schema);
        assert!(contract.has_runtime("runtime.lifecycle"));
        assert!(!contract.has_runtime("runtime.ecs.write"));
    }
}
