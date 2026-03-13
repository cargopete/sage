//! Command-line interface for the Sage language.

use clap::{Parser, Subcommand};
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use miette::{Diagnostic, IntoDiagnostic, Result, Severity, WrapErr};
use sage_checker::check_module_tree;
use sage_codegen::generate_module_tree;
use sage_loader::{load_project, ModuleTree};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

// Emojis for different stages
static SPARKLES: Emoji<'_, '_> = Emoji("✨ ", "* ");
static GEAR: Emoji<'_, '_> = Emoji("⚙️  ", "> ");
static CHECK: Emoji<'_, '_> = Emoji("✓ ", "v ");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", ">> ");

/// Ward the owl - Sage's mascot
const WARD_ASCII: &str = r#"
       ___
      (o,o)
      {`"'}
      -"-"-
"#;

/// Sage - A programming language where agents are first-class citizens.
#[derive(Parser)]
#[command(name = "sage")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile and run a Sage program
    Run {
        /// Path to the .sg file to run
        file: PathBuf,

        /// Build in release mode
        #[arg(long)]
        release: bool,

        /// Quiet mode - minimal output
        #[arg(short, long)]
        quiet: bool,
    },

    /// Compile a Sage program to a native binary
    Build {
        /// Path to the .sg file to compile
        file: PathBuf,

        /// Build in release mode
        #[arg(long)]
        release: bool,

        /// Output directory for generated files
        #[arg(short, long, default_value = "target/sage")]
        output: PathBuf,

        /// Only generate Rust code, don't compile
        #[arg(long)]
        emit_rust: bool,
    },

    /// Check a Sage program for errors without running it
    Check {
        /// Path to the .sg file to check
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    // Load .env file if present (ignore errors if not found)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            file,
            release,
            quiet,
        } => run_file(&file, release, quiet),
        Commands::Build {
            file,
            release,
            output,
            emit_rust,
        } => {
            build_file(&file, release, &output, emit_rust, false)?;
            Ok(())
        }
        Commands::Check { file } => check_file(&file),
    }
}

/// Print the Ward owl banner
fn print_banner() {
    let owl = style(WARD_ASCII).cyan().bold();
    println!("{owl}");
    println!(
        "  {} {}",
        style("SAGE").cyan().bold(),
        style("- Where agents come alive").dim()
    );
    println!();
}

/// Run a Sage program (compile + execute).
fn run_file(path: &PathBuf, release: bool, quiet: bool) -> Result<()> {
    // Build the program
    let output_dir = PathBuf::from("target/sage");
    let binary_path = build_file(path, release, &output_dir, false, quiet)?;

    let binary_path = binary_path.ok_or_else(|| miette::miette!("Build did not produce binary"))?;

    // Run the compiled binary
    if !quiet {
        println!();
        println!("{}Running...", ROCKET);
        println!();
    }

    let status = Command::new(&binary_path)
        .status()
        .into_diagnostic()
        .wrap_err("Failed to run compiled program")?;

    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
        miette::bail!("Program exited with error");
    }

    Ok(())
}

/// Check a Sage program file or project without running it.
fn check_file(path: &PathBuf) -> Result<()> {
    let display_name = get_display_name(path);

    // Load the project/file
    let module_tree = match load_project(path) {
        Ok(tree) => tree,
        Err(errors) => {
            for err in errors {
                eprintln!("Load error: {err}");
            }
            miette::bail!("Failed to load {}", display_name);
        }
    };

    // Check the module tree
    let check_result = check_module_tree(&module_tree);
    let mut has_errors = false;

    for err in &check_result.errors {
        // Try to find the source for this error
        let source_code = get_source_for_error(&module_tree, err);
        let report = miette::Report::new(err.clone()).with_source_code(source_code);
        eprintln!("{report:?}");
        if err.severity().unwrap_or(Severity::Error) == Severity::Error {
            has_errors = true;
        }
    }

    if has_errors {
        miette::bail!("Errors found in {}", display_name);
    }

    println!(
        "{}{} {} {}",
        SPARKLES,
        style("No errors").green().bold(),
        style("in").dim(),
        style(&display_name).yellow()
    );
    Ok(())
}

/// Get a display-friendly name for a path.
fn get_display_name(path: &Path) -> String {
    if path.is_dir() {
        // Project directory
        path.file_name().map_or_else(
            || "project".to_string(),
            |s| s.to_string_lossy().into_owned(),
        )
    } else {
        // Single file
        path.file_name().map_or_else(
            || "unknown".to_string(),
            |s| s.to_string_lossy().into_owned(),
        )
    }
}

/// Get source code for an error (used for error reporting).
fn get_source_for_error(tree: &ModuleTree, _err: &sage_checker::CheckError) -> String {
    // For now, return the root module's source. A more sophisticated implementation
    // would track which module the error came from.
    tree.modules
        .get(&tree.root)
        .map_or_else(String::new, |m| (*m.source).to_string())
}

/// Find the Sage toolchain directory.
/// Returns None if no pre-compiled toolchain is available.
fn find_toolchain() -> Option<PathBuf> {
    // 1. Check SAGE_TOOLCHAIN env var
    if let Ok(path) = std::env::var("SAGE_TOOLCHAIN") {
        let path = PathBuf::from(path);
        if path.join("libs").exists() && path.join("bin/rustc").exists() {
            return Some(path);
        }
    }

    // 2. Check relative to sage binary (for distribution)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            // Try ../toolchain (sage is in bin/)
            let toolchain = parent.parent().map(|p| p.join("toolchain"));
            if let Some(ref tc) = toolchain {
                if tc.join("libs").exists() {
                    return toolchain;
                }
            }
            // Try ./toolchain (sage is in root)
            let toolchain = parent.join("toolchain");
            if toolchain.join("libs").exists() {
                return Some(toolchain);
            }
        }
    }

    None
}

/// Compile using pre-compiled toolchain (fast path).
fn compile_with_toolchain(
    toolchain: &PathBuf,
    main_rs: &PathBuf,
    output: &PathBuf,
    _release: bool, // Unused: pre-compiled libs are always release-optimized
) -> Result<()> {
    let rustc = toolchain.join("bin/rustc");
    let libs_dir = toolchain.join("libs");

    // Set library path for rustc's own dylibs
    let lib_dir = toolchain.join("lib");

    let mut cmd = Command::new(&rustc);

    // Add library path for rustc's runtime libraries
    #[cfg(target_os = "macos")]
    cmd.env("DYLD_LIBRARY_PATH", &lib_dir);
    #[cfg(target_os = "linux")]
    cmd.env("LD_LIBRARY_PATH", &lib_dir);

    cmd.arg(main_rs)
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("bin")
        .arg("-L")
        .arg(format!("dependency={}", libs_dir.display()))
        .arg("-L")
        .arg(&libs_dir)
        .arg("-o")
        .arg(output);

    // Pre-compiled libs are always release, so always use -O
    // Note: LTO is not used because pre-compiled libs don't have bitcode
    cmd.arg("-O");

    // Add --extern for each dependency (rlib for libraries, dylib for proc-macros)
    // Track seen crates to avoid duplicates (some crates have multiple versions)
    let mut seen_crates = std::collections::HashSet::new();
    for entry in std::fs::read_dir(&libs_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "rlib" || ext == "dylib" || ext == "so" {
                if let Some(name) = parse_lib_name(&path) {
                    if seen_crates.insert(name.clone()) {
                        cmd.arg("--extern")
                            .arg(format!("{}={}", name, path.display()));
                    }
                }
            }
        }
    }

    let output_result = cmd.output().into_diagnostic()?;

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        miette::bail!("rustc failed:\n{}", stderr);
    }

    Ok(())
}

/// Parse library filename to crate name.
/// libfoo_bar-abc123.rlib -> foo_bar
/// libfoo_bar-abc123.dylib -> foo_bar
fn parse_lib_name(path: &PathBuf) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let name = stem.strip_prefix("lib")?;
    // Split on hash separator
    let name = name.split('-').next()?;
    Some(name.to_string())
}

/// Compile using cargo (slow path, requires Rust installed).
fn compile_with_cargo(project_dir: &PathBuf, release: bool) -> Result<()> {
    let mut cargo_args = vec!["build", "--quiet"];
    if release {
        cargo_args.push("--release");
    }

    let status = Command::new("cargo")
        .args(&cargo_args)
        .current_dir(project_dir)
        .status()
        .into_diagnostic()
        .wrap_err("Failed to run cargo build. Is Rust installed?")?;

    if !status.success() {
        miette::bail!("Cargo build failed");
    }

    Ok(())
}

/// Build a Sage program or project to a native binary.
/// Returns the path to the binary if compilation succeeded.
fn build_file(
    path: &PathBuf,
    release: bool,
    output_dir: &PathBuf,
    emit_rust_only: bool,
    quiet: bool,
) -> Result<Option<PathBuf>> {
    let start_time = Instant::now();

    if !quiet {
        print_banner();
    }

    let display_name = get_display_name(path);

    let project_name = if path.is_dir() {
        // Project directory name
        path.file_name()
            .map_or_else(
                || "sage_program".to_string(),
                |s| s.to_string_lossy().into_owned(),
            )
            .replace('-', "_")
    } else {
        // Single file name (without extension)
        path.file_stem()
            .map_or_else(
                || "sage_program".to_string(),
                |s| s.to_string_lossy().into_owned(),
            )
            .replace('-', "_")
    };

    if !quiet {
        println!("{}Compiling {}", GEAR, style(&display_name).yellow().bold());
        println!();
    }

    // Create a spinner
    let spinner = if !quiet {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        sp.set_message("Loading...");
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(sp)
    } else {
        None
    };

    // Load the project/file
    let module_tree = match load_project(path) {
        Ok(tree) => tree,
        Err(errors) => {
            if let Some(sp) = spinner {
                sp.finish_and_clear();
            }
            for err in errors {
                eprintln!("Load error: {err}");
            }
            miette::bail!("Failed to load {}", display_name);
        }
    };

    if let Some(ref sp) = spinner {
        sp.set_message("Type checking...");
    }

    // Type check the module tree
    let check_result = check_module_tree(&module_tree);
    let mut has_errors = false;

    for err in &check_result.errors {
        if let Some(ref sp) = spinner {
            sp.finish_and_clear();
        }
        let source_code = get_source_for_error(&module_tree, err);
        let report = miette::Report::new(err.clone()).with_source_code(source_code);
        eprintln!("{report:?}");
        if err.severity().unwrap_or(Severity::Error) == Severity::Error {
            has_errors = true;
        }
    }
    if has_errors {
        miette::bail!("Type errors in {}", display_name);
    }

    if let Some(ref sp) = spinner {
        sp.set_message("Generating Rust...");
    }

    // Generate Rust code from module tree
    let generated = generate_module_tree(&module_tree, &project_name);

    // Determine compilation mode
    let toolchain = find_toolchain();
    let use_toolchain = toolchain.is_some();

    // Create output directory
    let project_dir = output_dir.join(&project_name);
    std::fs::create_dir_all(&project_dir)
        .into_diagnostic()
        .wrap_err("Failed to create output directory")?;

    // For toolchain mode, just write main.rs directly
    // For cargo mode, write main.rs in src/ and Cargo.toml
    let (main_rs_path, binary_path) = if use_toolchain {
        let main_rs = project_dir.join("main.rs");
        let binary = project_dir.join(&project_name);
        (main_rs, binary)
    } else {
        let src_dir = project_dir.join("src");
        std::fs::create_dir_all(&src_dir).into_diagnostic()?;
        let main_rs = src_dir.join("main.rs");
        let binary_dir = if release { "release" } else { "debug" };
        let binary = project_dir
            .join("target")
            .join(binary_dir)
            .join(&project_name);
        (main_rs, binary)
    };

    std::fs::write(&main_rs_path, &generated.main_rs)
        .into_diagnostic()
        .wrap_err("Failed to write main.rs")?;

    // Write Cargo.toml only for cargo mode
    if !use_toolchain {
        let cargo_toml_path = project_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, &generated.cargo_toml)
            .into_diagnostic()
            .wrap_err("Failed to write Cargo.toml")?;
    }

    if emit_rust_only {
        if let Some(sp) = spinner {
            sp.finish_and_clear();
        }
        println!(
            "  {} Generated {}",
            CHECK,
            style(main_rs_path.display()).dim()
        );
        println!();
        println!(
            "{}{} Rust code generated in {}",
            SPARKLES,
            style("Done").green().bold(),
            style(project_dir.display()).yellow()
        );
        return Ok(None);
    }

    if let Some(ref sp) = spinner {
        if use_toolchain {
            sp.set_message("Compiling...");
        } else {
            sp.set_message("Building with cargo...");
        }
    }

    // Compile
    if let Some(ref tc) = toolchain {
        compile_with_toolchain(tc, &main_rs_path, &binary_path, release)?;
    } else {
        compile_with_cargo(&project_dir, release)?;
    }

    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    let total_duration = start_time.elapsed();

    if !quiet {
        let mode = if use_toolchain { "" } else { " (cargo)" };
        println!(
            "{}{} Compiled {}{} in {:.2}s",
            SPARKLES,
            style("Done").green().bold(),
            style(&display_name).yellow(),
            style(mode).dim(),
            total_duration.as_secs_f64()
        );
    }

    Ok(Some(binary_path))
}
