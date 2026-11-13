use std::{env, path::PathBuf, process::Command};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("terminal.o");
    let compiler = env::var_os("CC").unwrap_or_else(|| "cc".into());
    assert!(
        Command::new(compiler)
            .args([
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
                "src/terminal.c",
                "-o"
            ])
            .arg(&object)
            .status()
            .expect("a C compiler is required")
            .success()
    );
    let archiver = env::var_os("AR").unwrap_or_else(|| "ar".into());
    assert!(
        Command::new(archiver)
            .arg("crs")
            .arg(out.join("libterminal.a"))
            .arg(object)
            .status()
            .expect("ar is required")
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    for library in ["static=terminal", "EGL", "GL", "z"] {
        println!("cargo:rustc-link-lib={library}");
    }
    println!("cargo:rerun-if-changed=src/terminal.c");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=AR");
}
