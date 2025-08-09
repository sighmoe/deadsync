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

    // Compile Vulkan shaders with optimization based on the build profile.
    compile_vulkan_shaders(&mut compiler, &out_dir)?;

    // Copy assets into target/<profile>
    let target_dir = compute_target_dir()?;
    copy_assets(&target_dir)?;

    Ok(())
}

fn compile_vulkan_shaders(compiler: &mut Compiler, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    use std::fmt::Write as _;

    fn kind_for_ext(ext: &str) -> Option<ShaderKind> {
        match ext {
            "vert" => Some(ShaderKind::Vertex),
            "frag" => Some(ShaderKind::Fragment),
            "comp" => Some(ShaderKind::Compute),
            "geom" => Some(ShaderKind::Geometry),
            "tesc" => Some(ShaderKind::TessControl),
            "tese" => Some(ShaderKind::TessEvaluation),
            _ => None,
        }
    }

    let mut opts = shaderc::CompileOptions::new()?;
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    if profile == "release" {
        opts.set_optimization_level(shaderc::OptimizationLevel::Performance);
    } else {
        opts.set_optimization_level(shaderc::OptimizationLevel::Zero);
        opts.set_generate_debug_info();
    }

    // Compile any file named: src/shaders/vulkan_*.<ext> with supported stage
    for entry in glob::glob("src/shaders/vulkan_*.*")? {
        let path = entry?;
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        let Some(kind) = kind_for_ext(ext) else { continue };

        let source = fs::read_to_string(&path)?;
        let src_name = path.to_string_lossy();

        // Better error messages with file/line mapping
        let result = compiler.compile_into_spirv(&source, kind, &src_name, "main", Some(&opts));
        let spirv = match result {
            Ok(ok) => ok,
            Err(e) => {
                // Expand shaderc error with context so it's easy to fix
                let mut msg = String::new();
                writeln!(&mut msg, "Shader compile failed: {}", src_name)?;
                for (i, line) in source.lines().enumerate() {
                    writeln!(&mut msg, "{:4} | {}", i + 1, line)?;
                }
                writeln!(&mut msg, "\nError: {e}")?;
                return Err(msg.into());
            }
        };

        // Emit .spv right under OUT_DIR with the same file name: <file>.<ext>.spv
        let file_name = path.file_name().unwrap().to_string_lossy();
        let dest_path = out_dir.join(format!("{file_name}.spv"));

        let new_bytes = spirv.as_binary_u8();
        let needs_write = match fs::read(&dest_path) {
            Ok(old_bytes) => old_bytes != new_bytes,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&dest_path, new_bytes)?;
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