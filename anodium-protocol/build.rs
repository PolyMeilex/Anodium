use std::env::var;
use std::path::Path;
use wayland_scanner::*;

fn main() {
    let protocol_file = "./anodium.xml";

    let out_dir_str = var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    println!("cargo:rerun-if-changed={}", protocol_file);
    generate_code_with_destructor_events(
        protocol_file,
        out_dir.join("server_api.rs"),
        Side::Server,
        &[],
    );

    generate_code_with_destructor_events(
        protocol_file,
        out_dir.join("client_api.rs"),
        Side::Client,
        &[],
    );
}
