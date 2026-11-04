fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("app.o");
        let status = std::process::Command::new("x86_64-w64-mingw32-windres")
            .args(["app.rc", "-O", "coff", "-o"])
            .arg(&out)
            .status();
        if let Ok(status) = status {
            if status.success() {
                println!("cargo:rustc-link-arg={}", out.display());
            }
        }
    }
}
