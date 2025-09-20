use anyhow::Result;
use std::path::Path;
use wit_bindgen_rust::Opts;
use wit_parser::Resolve;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=wit/");

    let opts = Opts {
        generate_all: true,
        ..Default::default()
    };
    let mut generator = opts.build();
    let mut resolve = Resolve::default();
    let (pkg, _files) = resolve.push_path("wit")?;
    let main_packages = vec![pkg];
    let world = resolve.select_world(&main_packages, None)?;
    let mut files = Default::default();
    generator.generate(&resolve, world, &mut files)?;

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("generated.rs");
    let (_name, contents) = files.iter().next().unwrap();
    std::fs::write(&dst, contents)?;
    Ok(())
}
