fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=windows/app.rc");
        println!("cargo:rerun-if-changed=windows/app.manifest");
        embed_resource::compile_for_everything("windows/app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("embed Windows application manifest");
    }
}
