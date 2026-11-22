use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_gnu_windres() -> Option<PathBuf> {
    for cand in ["windres", "x86_64-w64-mingw32-windres"] {
        if Command::new(cand).arg("--version").output().is_ok() {
            return Some(PathBuf::from(cand));
        }
    }
    if let Ok(local_app) = env::var("LOCALAPPDATA") {
        let pkg_root = Path::new(&local_app)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages");
        if let Ok(entries) = std::fs::read_dir(pkg_root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("BrechtSanders.WinLibs") {
                    let cand = entry.path().join("mingw64").join("bin").join("windres.exe");
                    if cand.exists() {
                        return Some(cand);
                    }
                }
            }
        }
    }
    None
}

fn find_msvc_rc() -> Option<PathBuf> {
    if Command::new("rc").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc"));
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-changed=assets/brand_icon_48.bgra");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_env == "gnu" {
        let windres = find_gnu_windres().expect("windres not found for x86_64-pc-windows-gnu");
        let out_obj = out_dir.join("app_icon.o");
        let status = Command::new(&windres)
            .args(["-I", "assets", "assets/app.rc", "-O", "coff", "-o"])
            .arg(&out_obj)
            .status()
            .expect("failed to invoke windres");
        assert!(status.success(), "windres failed compiling assets/app.rc");
        println!("cargo:rustc-link-arg={}", out_obj.display());
    } else if target_env == "msvc" {
        let rc = find_msvc_rc().expect("rc.exe not found for MSVC target");
        let out_res = out_dir.join("app_icon.res");
        let status = Command::new(&rc)
            .args(["/nologo", "/fo"])
            .arg(&out_res)
            .arg("assets/app.rc")
            .status()
            .expect("failed to invoke rc.exe");
        assert!(status.success(), "rc.exe failed compiling assets/app.rc");
        println!("cargo:rustc-link-arg={}", out_res.display());
    }
}
