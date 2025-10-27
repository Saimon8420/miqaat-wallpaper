fn main() {
    // App icon and the manifest that turns on modern controls and per-monitor DPI.
    embed_resource::compile("res/app.rc", embed_resource::NONE).manifest_optional().unwrap();
    println!("cargo:rerun-if-changed=res/app.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
