//! Versioned bootstrap calls expose only a previously admitted host plan.
use shroudforge_api::startup_services::{self, PlanV2};
use std::path::PathBuf;
unsafe fn root(value: *const u16) -> Option<PathBuf> {
    if value.is_null() {
        return None;
    }
    let mut length = 0;
    while length < 32768 && unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    if length == 0 || length == 32768 {
        return None;
    }
    Some(PathBuf::from(
        String::from_utf16(unsafe { std::slice::from_raw_parts(value, length) }).ok()?,
    ))
}
/// 0 legacy, 1 admitted selected plan, 2 refusal. No caller-selected service mask.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_take_startup_services_v2(
    game: *const u16,
    output: *mut PlanV2,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(root) = (unsafe { root(game) }) else {
            return 2;
        };
        if output.is_null() {
            return 2;
        }
        match startup_services::take(&root) {
            Ok(Some(plan)) => {
                unsafe { output.write(plan) };
                1
            }
            Ok(None) => 0,
            Err(error) => {
                let _ = shroudforge_logging::append(&root, 'E', "startup-services", &error);
                2
            }
        }
    })
    .unwrap_or(2)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_commit_startup_services_v2(
    game: *const u16,
    completed: u32,
) -> bool {
    std::panic::catch_unwind(|| {
        let Some(root) = (unsafe { root(game) }) else {
            return false;
        };
        match startup_services::commit(&root, completed) {
            Ok(()) => true,
            Err(error) => {
                let _ = shroudforge_logging::append(&root, 'E', "startup-services", &error);
                false
            }
        }
    })
    .unwrap_or(false)
}

/// Trusted provider dispatch only. Held-entry proof material and tokens remain
/// inside the native provider and never cross the plugin C ABI.
#[unsafe(no_mangle)]
pub extern "C" fn shroudforge_stage_selected_native_plugins() -> bool {
    std::panic::catch_unwind(
        || match startup_services::stage_native_plugins_before_entry() {
            Ok(true) => true,
            Ok(false) => false,
            Err(error) => {
                tracing::error!(target: "shroudforge::runtime", %error,
                    "Selected V2 native plugins could not stage before framework preparation");
                false
            }
        },
    )
    .unwrap_or(false)
}

/// Trusted provider dispatch only. Held-entry proof material and tokens remain
/// inside the native provider and never cross the plugin C ABI.
#[unsafe(no_mangle)]
pub extern "C" fn shroudforge_prepare_selected_native_plugins() -> bool {
    std::panic::catch_unwind(
        || match startup_services::prepare_native_plugins_before_entry() {
            Ok(true) => true,
            Ok(false) => false,
            Err(error) => {
                tracing::error!(target: "shroudforge::runtime", %error,
                    "Selected V2 native plugins could not prepare before game entry");
                false
            }
        },
    )
    .unwrap_or(false)
}

/// Serial post-restore dispatch from the original executable entry thread.
/// Callbacks receive only their opaque context, never a held-entry token or host table.
#[unsafe(no_mangle)]
pub extern "C" fn shroudforge_run_selected_native_plugins_entry_restored() -> bool {
    std::panic::catch_unwind(|| {
        shroudforge_api::startup_services::dispatch_native_plugins_entry_restored()
    })
    .unwrap_or(false)
}
