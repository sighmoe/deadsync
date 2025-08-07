use fs_extra::dir::{copy, CopyOptions};
use shaderc::{Compiler, ShaderKind};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    // This tells cargo to re-run the script if anything in src/shaders changes.
    println!("cargo:rerun-if-changed=src/shaders/");
    // Also re-run if assets change.
    println!("cargo:rerun-if-changed=assets/*");

    // Initialize the shader compiler.
    // FIX: `Compiler::new()` returns a Result, so we can use `?` directly.
    let compiler = Compiler::new()?;

    // Find the output directory for the compiled shaders.
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    // Find and compile all Vulkan shaders.
    for entry in glob::glob("src/shaders/vulkan_*")? {
        let path = entry?;
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let kind = match extension {
            "vert" => ShaderKind::Vertex,
            "frag" => ShaderKind::Fragment,
            _ => continue, // Skip any other files
        };

        let source = fs::read_to_string(&path)?;
        let spirv = compiler.compile_into_spirv(
            &source,
            kind,
            path.to_str().unwrap(),
            "main",
            None,
        )?;

        // Create the destination path in the `OUT_DIR`.
        let dest_path =
            out_dir.join(path.file_name().unwrap().to_str().unwrap().to_owned() + ".spv");
        fs::write(dest_path, spirv.as_binary_u8())?;
    }

    // --- Asset Copying Logic ---
    // This path points to the directory where the final executable will be, e.g., `target/release`.
    let target_dir = out_dir.join("../../../");

    // Copy the entire assets directory if it exists.
    if fs::metadata("assets").is_ok() {
        let mut options = CopyOptions::new();
        options.overwrite = true;
        copy("assets", &target_dir, &options)?;
        println!("cargo:info=Copied assets to {}", target_dir.display());
    }

    Ok(())
}