// Fill gaps in how the Rust `sdl2` crate links SDL2.
//
// macOS / Linux: it links against system SDL2 but doesn't add Homebrew's
// library path to the linker search. Without this the build fails with
// `ld: library 'SDL2' not found` even when `brew install sdl2` has
// already been run.
//
// Windows: SDL2 is compiled in statically (see Cargo.toml), and sdl2-sys
// leaves advapi32 off its list of system libraries. SDL's audio code calls
// the registry functions in it (`RegOpenKeyExW` etc.), so without this the
// link fails with LNK1120 "unresolved externals".

fn main() {
    if cfg!(target_os = "macos") {
        // Apple silicon Homebrew lives under /opt/homebrew; Intel
        // Homebrew uses /usr/local. Add both. The linker silently
        // ignores ones that don't exist.
        println!("cargo:rustc-link-search=/opt/homebrew/lib");
        println!("cargo:rustc-link-search=/usr/local/lib");
    }
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
