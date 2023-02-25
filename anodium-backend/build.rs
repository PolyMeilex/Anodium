use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=/usr/share/hwdata/pnp.ids");

    let file = fs::read_to_string("/usr/share/hwdata/pnp.ids").unwrap();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("hwdata_pnp_ids.rs");

    let mut output = Vec::new();

    output.push("#[rustfmt::skip]".into());
    output.push("pub fn find_manufacturer(vendor: &[char; 3]) -> Option<&'static str> {".into());
    output.push("match vendor {".into());

    for line in file.lines() {
        let mut segment = line.split('\t');

        let mut code = segment.next().unwrap().chars();
        let n1 = code.next().unwrap();
        let n2 = code.next().unwrap();
        let n3 = code.next().unwrap();

        let name = segment.next().unwrap();

        output.push(format!("['{n1}', '{n2}', '{n3}'] => Some(\"{name}\"),"));
    }

    output.push("_ => None,".into());

    output.push("}".into());
    output.push("}".into());

    let output = output.join("\n");

    fs::write(dest_path, output).unwrap();
}
