use mlua::{Function, Table};
use mod_loader::Mod;

fn registry_key(id: &str) -> String {
    format!("shroudforge.runtime.overrides.{id}")
}

pub(crate) fn create(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<Table> {
    create_scoped(lua, &r#mod.info().id)
}

fn create_scoped(lua: &mlua::Lua, id: &str) -> mlua::Result<Table> {
    let key = registry_key(id);
    let factory_key = format!("{key}.factory");
    if let Some(factory) = lua.named_registry_value::<Option<Table>>(&factory_key)? {
        return Ok(factory);
    }
    let factory: Table = lua
        .load(include_str!("track_runtime_overrides.lua"))
        .set_name("runtime.overrides")
        .eval()?;
    let new: Function = factory.raw_get("new")?;
    let trackers = lua.create_table()?;
    lua.set_named_registry_value(&key, trackers.clone())?;
    let get_session =
        lua.create_function(|_, ()| Ok(super::loader::runtime_provider::world_session_id()))?;
    factory.raw_set(
        "new",
        lua.create_function(move |_, ()| {
            let tracker: Table = new.call(get_session.clone())?;
            trackers.raw_set(trackers.raw_len() + 1, tracker.clone())?;
            Ok(tracker)
        })?,
    )?;
    lua.set_named_registry_value(&factory_key, factory.clone())?;
    Ok(factory)
}

pub(crate) fn cleanup(lua: &mlua::Lua, id: &str) -> Result<(), String> {
    let trackers: Option<Table> = lua
        .named_registry_value(&registry_key(id))
        .map_err(|error| error.to_string())?;
    let Some(trackers) = trackers else {
        return Ok(());
    };
    for tracker in trackers.sequence_values::<Table>() {
        let tracker = tracker.map_err(|error| error.to_string())?;
        let call = |name: &str| -> mlua::Result<()> {
            tracker.raw_get::<Function>(name)?.call(tracker.clone())
        };
        call("begin_update").map_err(|error| error.to_string())?;
        call("restore_all").map_err(|error| error.to_string())?;
        if let Some(fault) = tracker
            .raw_get::<Option<String>>("fault")
            .map_err(|error| error.to_string())?
        {
            return Err(format!("Runtime API cleanup requires inspection: {fault}"));
        }
        let owned: bool = tracker
            .raw_get::<Function>("has_ownership")
            .and_then(|function| function.call(tracker.clone()))
            .map_err(|error| error.to_string())?;
        if owned {
            return Err(tracker
                .raw_get::<Function>("summary")
                .and_then(|function| {
                    function.call((tracker.clone(), "Runtime API cleanup is pending"))
                })
                .unwrap_or_else(|error| error.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cleanup, create_scoped};

    #[test]
    fn attribute_targets_reuse_names_without_reusing_live_root_snapshots() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        let factory: mlua::Function = lua
            .load(include_str!("create_override_targets.lua"))
            .eval()?;
        lua.globals().set("target_factory", factory)?;
        lua.load(
            r#"
            local descriptors, snapshots, acquired, claims = 0, 0, 0, 0
            local current = 0
            local runtime = {
                world={session_id=function() return 1 end},
                ecs={
                    get_attribute=function(selector)
                        descriptors = descriptors + 1
                        if selector ~= "Stamina_Min" then return nil, "attribute not found" end
                        return {name=selector, root_hash=1}
                    end,
                    read_attributes=function()
                        snapshots = snapshots + 1
                        return {
                            entity=7, root_hash=1,
                            component={qualified_name="keen::ecs::Stamina"},
                            values={Stamina_Min=current},
                        }
                    end,
                },
                overrides={get_component_claim=function()
                    claims = claims + 1
                    return {acquire=function() acquired = acquired + 1; return true end}
                end},
            }
            local targets = target_factory(runtime, {})
            for value = 1, 5 do
                current = value
                local target = assert(targets.attribute(7, "Stamina_Min"))
                assert(target.claim:acquire())
                assert(target.read().value == value)
            end
            assert(descriptors == 1, "warm targets reuse only immutable catalog names")
            assert(snapshots == 5, "each target reuses its complete initial root snapshot")
            assert(acquired == 5 and claims == 5, "every target retains the checked claim path")
            local other_mod = target_factory(runtime, {})
            assert(other_mod.attribute(7, "Stamina_Min"))
            assert(descriptors == 2, "metadata reuse remains scoped to its mod environment")
            for _ = 1, 2 do assert(not targets.attribute(7, "Missing")) end
            assert(descriptors == 4, "failed lookups are not cached")
        "#,
        )
        .exec()
    }

    #[test]
    fn lifecycle_restores_api_values_without_a_mod_unload_callback() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        lua.globals()
            .set("overrides", create_scoped(&lua, "third-party-mod")?)?;
        lua.load(
            r#"
            value, busy = 1, false
            local tracker = overrides.new()
            tracker:apply("field", "setting", {
                claim={acquire=function() return true end, release=function() end},
                read=function() return {value=value} end,
                write=function(snapshot, target)
                    assert(snapshot.value == value)
                    if busy then return false, nil, "Scene gate is busy" end
                    value = target
                    return true, target
                end,
            }, function() return 2 end)
            busy = true
        "#,
        )
        .exec()?;
        let mut item = crate::Lifecycle {
            id: "third-party-mod".into(),
            load: None,
            update: None,
            unload: None,
            update_interval: std::time::Duration::from_millis(100),
            next_update: std::time::Instant::now(),
            active: true,
            cleanup_pending: false,
        };
        assert!(item.try_unload(&lua).is_err());
        assert!(item.active && item.cleanup_pending);
        assert_eq!(lua.globals().get::<i64>("value")?, 2);
        lua.globals().set("busy", false)?;
        item.try_unload(&lua)?;
        assert!(!item.active && !item.cleanup_pending);
        assert_eq!(lua.globals().get::<i64>("value")?, 1);
        Ok(())
    }

    #[test]
    fn failed_mod_unload_still_runs_registered_api_restoration() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        lua.globals()
            .set("overrides", create_scoped(&lua, "third-party-mod")?)?;
        lua.load(
            r#"
            value = 1
            local tracker = overrides.new()
            tracker:apply("field", "setting", {
                claim={acquire=function() return true end, release=function() end},
                read=function() return {value=value} end,
                write=function(_, target) value = target; return true, target end,
            }, function() return 2 end)
        "#,
        )
        .exec()?;
        let unload = lua
            .load("return function() error('foreign unload failed') end")
            .eval::<mlua::Function>()?;
        let mut item = crate::Lifecycle {
            id: "third-party-mod".into(),
            load: None,
            update: None,
            unload: Some(lua.create_registry_value(unload)?),
            update_interval: std::time::Duration::from_millis(100),
            next_update: std::time::Instant::now(),
            active: true,
            cleanup_pending: false,
        };
        assert!(item.try_unload(&lua).is_err());
        assert_eq!(lua.globals().get::<i64>("value")?, 1);
        assert!(
            item.active && item.cleanup_pending,
            "the mod's own failing cleanup remains pending"
        );
        cleanup(&lua, "third-party-mod").unwrap();
        Ok(())
    }

    #[test]
    fn automatic_cleanup_is_mod_scoped_and_retries_busy_writers() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        lua.globals().set("first", create_scoped(&lua, "first")?)?;
        lua.globals()
            .set("second", create_scoped(&lua, "second")?)?;
        lua.load(
            r#"
            first_value, second_value, busy = 1, false, false
            local first_tracker = first.new()
            first_tracker:apply("field", "setting", {
                claim={acquire=function() return true end, release=function() end},
                read=function() return {value=first_value} end,
                write=function(snapshot, value)
                    assert(snapshot.value == first_value)
                    if busy then return false, nil, "Scene gate is busy" end
                    first_value = value
                    return true, value
                end,
            }, function() return 2 end)
            local second_tracker = second.new()
            second_tracker:apply("field", "setting", {
                claim={acquire=function() return true end, release=function() end},
                read=function() return {value=second_value} end,
                write=function(snapshot, value)
                    assert(snapshot.value == second_value)
                    second_value = value
                    return true, value
                end,
            }, function() return true end)
            busy = true
        "#,
        )
        .exec()?;
        let recreated = create_scoped(&lua, "first")?;
        assert_eq!(
            recreated.to_pointer(),
            lua.globals().get::<mlua::Table>("first")?.to_pointer(),
            "rebuilding the same mod environment must retain its original tracker registry"
        );
        assert!(cleanup(&lua, "first")
            .unwrap_err()
            .contains("Scene gate is busy"));
        assert_eq!(lua.globals().get::<i64>("first_value")?, 2);
        lua.globals().set("busy", false)?;
        cleanup(&lua, "first").unwrap();
        assert_eq!(lua.globals().get::<i64>("first_value")?, 1);
        assert!(lua.globals().get::<bool>("second_value")?);
        cleanup(&lua, "second").unwrap();
        assert!(!lua.globals().get::<bool>("second_value")?);
        cleanup(&lua, "unregistered").unwrap();
        Ok(())
    }
}
