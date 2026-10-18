fn main() {
    // Static imports are resolved before `main` (and before `harden_dll_search`) with the
    // default search order, which starts at the exe's folder; the portable exe may sit (and be
    // elevated) in Downloads. LOAD_LIBRARY_SEARCH_SYSTEM32 (0x800) makes the loader take them
    // from System32 only (Windows 10 1607+; older systems ignore the flag).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    tauri_build::build()
}
