use super::*;

const OPERATION: &str = runtime_provider::first_person_operation_name();

pub(crate) fn create(lua: &mlua::Lua, owner: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    add_function_with_mod(lua, &table, "set_mode", owner, set_mode)?;
    Ok(table)
}

fn set_mode(
    lua: &mlua::Lua,
    args: FunctionArgs,
    owner: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    if args.len() != 2 {
        return Err(LuaError::generic(
            "first_person.set_mode(enabled, cameraDistance) takes exactly two arguments",
        ));
    }
    let enabled = args.get::<bool>(0)?;
    let camera_distance = args.get::<f64>(1)?;
    validate_camera_distance(camera_distance).map_err(LuaError::generic)?;

    let state = lua.app_data_ref::<AppState>().unwrap();
    let generation = state.lua_map_owner(&owner.info().id);
    let result = dispatch_mode(
        enabled,
        || runtime_provider::first_person_release_owner(generation),
        || {
            if let Some(reason) = runtime_denial_reason(&state, owner, OPERATION) {
                return Err(reason);
            }
            if !state.is_client() || !owner.info().target.supports_process(false) {
                return Err(
                    "first-person camera control requires a client-targeted mod in a client process"
                        .into(),
                );
            }
            if !runtime_provider::first_person_available() {
                return Err(runtime_provider::first_person_detail());
            }
            runtime_provider::first_person_set_mode(generation, true, camera_distance)
        },
    );
    match result {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}

fn dispatch_mode(
    enabled: bool,
    release_owner: impl FnOnce() -> Result<(), String>,
    enable_mode: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if enabled {
        enable_mode()
    } else {
        // The token is host-derived and owned by this mod. Cleanup must remain
        // available if profile or provider admission changed after enable.
        release_owner()
    }
}

fn validate_camera_distance(distance: f64) -> Result<(), String> {
    if !distance.is_finite() || !(-0.5..=0.5).contains(&distance) {
        return Err("cameraDistance must be finite and between -0.5 and 0.5".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{dispatch_mode, validate_camera_distance};

    #[test]
    fn accepts_only_finite_distances_in_the_native_range() {
        for distance in [-0.5, -0.25, 0.0, 0.25, 0.5] {
            assert!(validate_camera_distance(distance).is_ok());
        }
        for distance in [
            f64::NEG_INFINITY,
            -0.500_001,
            0.500_001,
            f64::INFINITY,
            f64::NAN,
        ] {
            assert!(validate_camera_distance(distance).is_err());
        }
    }

    #[test]
    fn disabling_releases_the_owner_when_enable_admission_is_unavailable() {
        let mut released = false;
        let mut admission_checked = false;
        let result = dispatch_mode(
            false,
            || {
                released = true;
                Ok(())
            },
            || {
                admission_checked = true;
                Err("native first-person provider is unavailable".into())
            },
        );
        assert!(result.is_ok());
        assert!(released);
        assert!(!admission_checked);
    }
}
