use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=sdl_keyboard.c");
    println!("cargo:rerun-if-changed=SDL/src/events/scancodes_windows.h");
    println!("cargo:rerun-if-changed=SDL");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_FEATURE");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let directory = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = directory.join("SDL");
    assert!(
        source.join("CMakeLists.txt").is_file(),
        "The pinned SDL3 submodule is missing. Run git submodule update --init modules/hotkeys/SDL"
    );
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static");

    let mut configuration = cmake::Config::new(&source);
    configuration
        .define("SDL_SHARED", "OFF")
        .define("SDL_STATIC", "ON")
        .define("SDL_INSTALL", "ON")
        .define("SDL_INSTALL_CPACK", "OFF")
        .define("SDL_INSTALL_DOCS", "OFF")
        .define("SDL_UNINSTALL", "OFF")
        .define("SDL_TEST_LIBRARY", "OFF")
        .define("SDL_TESTS", "OFF")
        .define("SDL_EXAMPLES", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("SDL_VIDEO", "ON");

    // SDL's event sources are always included. The Windows video backend owns
    // its Raw Keyboard implementation, independently of joystick and HIDAPI.
    for subsystem in [
        "AUDIO",
        "GPU",
        "RENDER",
        "CAMERA",
        "JOYSTICK",
        "HAPTIC",
        "HIDAPI",
        "POWER",
        "SENSOR",
        "DIALOG",
        "TRAY",
        "OPENGL",
        "OPENGLES",
        "VULKAN",
        "DIRECTX",
        "XINPUT",
        "DUMMYVIDEO",
        "OFFSCREEN",
    ] {
        configuration.define(format!("SDL_{subsystem}"), "OFF");
    }
    if msvc {
        // Match Cargo's CRT choice instead of SDL selecting a different CRT.
        configuration.static_crt(static_crt);
    }
    let installed = configuration.build();
    let profile = env::var("PROFILE").unwrap();
    let build_config = installed
        .join("build")
        .join(format!("include-config-{profile}"))
        .join("build_config");

    // Emit the adapter first, then SDL and its system dependencies. This order
    // also keeps single-pass static linkers correct for the adapter's SDL calls.
    let mut adapter = cc::Build::new();
    adapter
        .file(directory.join("sdl_keyboard.c"))
        .include(source.join("include"))
        // The legacy-key migration shares SDL's own private Windows scan-code
        // table instead of maintaining a second copy of that mapping.
        .include(source.join("src"))
        .include(build_config)
        .define("SDL_MAIN_HANDLED", None)
        .define("SDL_STATIC_LIB", None);
    if msvc {
        adapter.flag("/utf-8").static_crt(static_crt);
    }
    adapter.compile("shroudforge_sdl_keyboard");

    println!(
        "cargo:rustc-link-search=native={}",
        installed.join("lib").display()
    );
    println!(
        "cargo:rustc-link-lib=static={}",
        if msvc { "SDL3-static" } else { "SDL3" }
    );

    // SDL release-3.4.18's Windows base dependency declaration, with joystick,
    // audio, rendering and GPU backends disabled above. SDL adds no dinput8
    // dependency when its joystick subsystem is disabled.
    for library in [
        "kernel32", "user32", "gdi32", "winmm", "imm32", "ole32", "oleaut32", "version", "uuid",
        "advapi32", "setupapi", "shell32",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
}
