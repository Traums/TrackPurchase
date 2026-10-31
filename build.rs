fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("cupertino".to_string());
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
    #[cfg(windows)]
    embed_icon();
}

#[cfg(windows)]
fn embed_icon() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let rc_path = std::path::Path::new(&out_dir).join("icon.rc");
    let res_path = std::path::Path::new(&out_dir).join("icon.res");
    let icon = std::fs::canonicalize("assets/icon.ico").expect("assets/icon.ico");
    let icon = icon.to_string_lossy().replace(r"\\?\", "").replace('\\', "\\\\");
    std::fs::write(&rc_path, format!("1 ICON \"{icon}\"\n")).expect("icon.rc");

    let rc = find_rc();
    let status = std::process::Command::new(&rc)
        .arg("/nologo")
        .arg(format!("/fo{}", res_path.display()))
        .arg(&rc_path)
        .status()
        .expect("rc.exe");
    if !status.success() {
        panic!("не удалось собрать значок приложения");
    }
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
    println!("cargo:rerun-if-changed=assets/icon.ico");
}

#[cfg(windows)]
fn find_rc() -> std::path::PathBuf {
    let root = std::path::Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions = std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.join("x64").join("rc.exe").is_file())
        .collect::<Vec<_>>();
    versions.sort();
    versions
        .pop()
        .map(|dir| dir.join("x64").join("rc.exe"))
        .expect("не найден rc.exe из Windows SDK")
}
