use fs_extra::dir::{copy, CopyOptions};
use shaderc::{Compiler, ShaderKind};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    // Rerun on shader or asset changes
    println!("cargo:rerun-if-changed=src/shaders");
    println!("cargo:rerun-if-changed=assets");

    // FIX: Compiler::new() returns a Result, so we can use `?` directly.
    let mut compiler = Compiler::new()?;

    // OUT_DIR used by include_bytes! in Vulkan source
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    // Compile Vulkan shaders with optimization
    compile_vulkan_shaders(&mut compiler, &out_dir)?;

    // Copy assets into target/<profile>
    let target_dir = compute_target_dir()?;
    copy_assets(&target_dir)?;

    Ok(())
}

fn compile_vulkan_shaders(compiler: &mut Compiler, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    // FIX: CompileOptions::new() also returns a Result.
    let mut opts = shaderc::CompileOptions::new()?;
    opts.set_optimization_level(shaderc::OptimizationLevel::Performance);

    for ext in ["vert", "frag"] {
        for entry in glob::glob(&format!("src/shaders/vulkan_*.{}", ext))? {
            let path = entry?;
            let kind = match ext {
                "vert" => ShaderKind::Vertex,
                _ => ShaderKind::Fragment,
            };

            let source = fs::read_to_string(&path)?;
            let spirv = compiler.compile_into_spirv(
                &source,
                kind,
                path.to_str().unwrap(),
                "main",
                Some(&opts),
            )?;

            // Produce: <file_name>.<ext>.spv (matches include_bytes! paths in vulkan.rs)
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
            let dest_path = out_dir.join(format!("{}.spv", file_name));
            fs::write(dest_path, spirv.as_binary_u8())?;
        }
    }
    Ok(())
}

fn compute_target_dir() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let profile = std::env::var("PROFILE")?;
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("target"));
    Ok(base.join(profile))
}

fn copy_assets(target_dir: &Path) -> Result<(), Box<dyn Error>> {
    if fs::metadata("assets").is_ok() {
        let mut options = CopyOptions::new();
        options.overwrite = true;
        // The default behavior (copy_inside=false) is correct.
        // It copies the `assets` directory itself into `target_dir`.
        copy("assets", target_dir, &options)?;
        // Use `cargo:warning=` so the message is always visible.
        println!(
            "cargo:warning=Copied assets to {}",
            target_dir.join("assets").display()
        );
    }
    Ok(())
}