// Embeds the Windows executable icon and version metadata.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/codelines.rc");
        embed_resource::compile("assets/codelines.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
