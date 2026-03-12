//! Command-line interface for the Sage language.

use clap::{Parser, Subcommand};
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use miette::{Diagnostic, IntoDiagnostic, Result, Severity, WrapErr};
use sage_checker::check;
use sage_codegen::generate;
use sage_interpreter::{LlmConfig, Runtime, RuntimeConfig};
use sage_lexer::lex;
use sage_parser::parse;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

// Emojis for different stages
static SPARKLES: Emoji<'_, '_> = Emoji("✨ ", "* ");
static GEAR: Emoji<'_, '_> = Emoji("⚙️  ", "> ");
static CHECK: Emoji<'_, '_> = Emoji("✓ ", "v ");
static BRAIN: Emoji<'_, '_> = Emoji("🧠 ", "@ ");

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
    /// Run a Sage program (interpreted)
    Run {
        /// Path to the .sg file to run
        file: PathBuf,

        /// Use mock LLM (for testing without API key)
        #[arg(long)]
        mock: bool,

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

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present (ignore errors if not found)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, mock, quiet } => run_file(&file, mock, quiet).await,
        Commands::Build {
            file,
            release,
            output,
            emit_rust,
        } => build_file(&file, release, &output, emit_rust),
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

/// Run a Sage program file.
async fn run_file(path: &PathBuf, mock: bool, quiet: bool) -> Result<()> {
    let start_time = Instant::now();

    if !quiet {
        print_banner();
    }

    let source = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read file: {}", path.display()))?;

    let filename = path
        .file_name()
        .map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().into_owned());

    // Get LLM info for display
    let llm_config = if mock {
        None
    } else {
        LlmConfig::from_env()
    };

    if !quiet {
        println!(
            "{}Running {}",
            GEAR,
            style(&filename).yellow().bold()
        );
        if let Some(ref cfg) = llm_config {
            let model_display = style(&cfg.model).magenta();
            let url_short = if cfg.api_url.contains("openai.com") {
                "OpenAI".to_string()
            } else {
                // Extract host from URL
                cfg.api_url
                    .replace("http://", "")
                    .replace("https://", "")
                    .split('/')
                    .next()
                    .unwrap_or("local")
                    .to_string()
            };
            println!(
                "  {} {} @ {}",
                BRAIN,
                model_display,
                style(url_short).dim()
            );
        } else if mock {
            println!("  {} {}", BRAIN, style("mock mode").dim());
        }
        println!();
    }

    // Create a spinner for the compilation phase
    let spinner = if !quiet {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        sp.set_message("Parsing...");
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(sp)
    } else {
        None
    };

    // Lex
    let lex_result = match lex(&source) {
        Ok(result) => result,
        Err(err) => {
            if let Some(sp) = spinner {
                sp.finish_and_clear();
            }
            let report = miette::Report::new(err).with_source_code(source);
            return Err(report);
        }
    };

    // Parse
    let source_arc: Arc<str> = Arc::from(source.as_str());
    let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

    if !parse_errors.is_empty() {
        if let Some(sp) = spinner {
            sp.finish_and_clear();
        }
        for err in &parse_errors {
            eprintln!("Parse error: {err}");
        }
        miette::bail!("Parse errors in {filename}");
    }

    let program = program.ok_or_else(|| miette::miette!("Failed to parse program"))?;

    if let Some(ref sp) = spinner {
        sp.set_message("Type checking...");
    }

    // Type check
    let check_result = check(&program);
    let mut has_errors = false;
    for err in &check_result.errors {
        if let Some(ref sp) = spinner {
            sp.finish_and_clear();
        }
        let report = miette::Report::new(err.clone()).with_source_code(source.clone());
        eprintln!("{report:?}");
        // Only count actual errors, not warnings
        if err.severity().unwrap_or(Severity::Error) == Severity::Error {
            has_errors = true;
        }
    }
    if has_errors {
        miette::bail!("Type errors in {filename}");
    }

    if let Some(ref sp) = spinner {
        sp.set_message(format!("{}Running agents...", BRAIN));
    }

    let run_start = Instant::now();

    // Run
    let runtime = if mock {
        Runtime::mock()
    } else {
        Runtime::new(RuntimeConfig::default())
    };

    let result = runtime
        .run(program)
        .await
        .map_err(|e| miette::Report::new(e).with_source_code(source))?;

    let run_duration = run_start.elapsed();
    let total_duration = start_time.elapsed();

    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    // Print result if it's not Unit
    if !result.is_unit() {
        println!("{result}");
    }

    if !quiet {
        println!();
        println!(
            "{}{} Done in {:.2}s (LLM: {:.2}s)",
            CHECK,
            style("Sage").green().bold(),
            total_duration.as_secs_f64(),
            run_duration.as_secs_f64(),
        );
    }

    Ok(())
}

/// Check a Sage program file without running it.
fn check_file(path: &PathBuf) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read file: {}", path.display()))?;

    let filename = path
        .file_name()
        .map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().into_owned());

    // Lex
    let lex_result = match lex(&source) {
        Ok(result) => result,
        Err(err) => {
            let report = miette::Report::new(err).with_source_code(source);
            return Err(report);
        }
    };

    // Parse
    let source_arc: Arc<str> = Arc::from(source.as_str());
    let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

    let mut has_errors = false;

    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("Parse error: {err}");
        }
        has_errors = true;
    }

    if let Some(program) = program {
        // Type check
        let check_result = check(&program);
        for err in &check_result.errors {
            let report = miette::Report::new(err.clone()).with_source_code(source.clone());
            eprintln!("{report:?}");
            // Only count actual errors, not warnings
            if err.severity().unwrap_or(Severity::Error) == Severity::Error {
                has_errors = true;
            }
        }
    }

    if has_errors {
        miette::bail!("Errors found in {filename}");
    }

    println!(
        "{}{} {} {}",
        SPARKLES,
        style("No errors").green().bold(),
        style("in").dim(),
        style(&filename).yellow()
    );
    Ok(())
}

/// Build a Sage program to a native binary.
fn build_file(path: &PathBuf, release: bool, output_dir: &PathBuf, emit_rust_only: bool) -> Result<()> {
    let start_time = Instant::now();

    print_banner();

    let source = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read file: {}", path.display()))?;

    let filename = path
        .file_name()
        .map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().into_owned());

    let project_name = path
        .file_stem()
        .map_or_else(|| "sage_program".to_string(), |s| s.to_string_lossy().into_owned())
        .replace('-', "_");

    println!(
        "{}Compiling {}",
        GEAR,
        style(&filename).yellow().bold()
    );
    println!();

    // Lex
    let lex_result = match lex(&source) {
        Ok(result) => result,
        Err(err) => {
            let report = miette::Report::new(err).with_source_code(source);
            return Err(report);
        }
    };

    // Parse
    let source_arc: Arc<str> = Arc::from(source.as_str());
    let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("Parse error: {err}");
        }
        miette::bail!("Parse errors in {filename}");
    }

    let program = program.ok_or_else(|| miette::miette!("Failed to parse program"))?;

    // Type check
    let check_result = check(&program);
    let mut has_errors = false;
    for err in &check_result.errors {
        let report = miette::Report::new(err.clone()).with_source_code(source.clone());
        eprintln!("{report:?}");
        if err.severity().unwrap_or(Severity::Error) == Severity::Error {
            has_errors = true;
        }
    }
    if has_errors {
        miette::bail!("Type errors in {filename}");
    }

    // Generate Rust code
    let generated = generate(&program, &project_name);

    // Create output directory
    let project_dir = output_dir.join(&project_name);
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)
        .into_diagnostic()
        .wrap_err("Failed to create output directory")?;

    // Write generated files
    let main_rs_path = src_dir.join("main.rs");
    let cargo_toml_path = project_dir.join("Cargo.toml");

    std::fs::write(&main_rs_path, &generated.main_rs)
        .into_diagnostic()
        .wrap_err("Failed to write main.rs")?;

    std::fs::write(&cargo_toml_path, &generated.cargo_toml)
        .into_diagnostic()
        .wrap_err("Failed to write Cargo.toml")?;

    println!(
        "  {} Generated {}",
        CHECK,
        style(main_rs_path.display()).dim()
    );
    println!(
        "  {} Generated {}",
        CHECK,
        style(cargo_toml_path.display()).dim()
    );

    if emit_rust_only {
        println!();
        println!(
            "{}{} Rust code generated in {}",
            SPARKLES,
            style("Done").green().bold(),
            style(project_dir.display()).yellow()
        );
        return Ok(());
    }

    // Compile with cargo
    println!();
    println!("  {} Building with cargo...", GEAR);

    let mut cargo_args = vec!["build"];
    if release {
        cargo_args.push("--release");
    }

    let cargo_status = Command::new("cargo")
        .args(&cargo_args)
        .current_dir(&project_dir)
        .status()
        .into_diagnostic()
        .wrap_err("Failed to run cargo build")?;

    if !cargo_status.success() {
        miette::bail!("Cargo build failed");
    }

    let binary_dir = if release { "release" } else { "debug" };
    let binary_path = project_dir.join("target").join(binary_dir).join(&project_name);

    let total_duration = start_time.elapsed();

    println!();
    println!(
        "{}{} Built {} in {:.2}s",
        SPARKLES,
        style("Done").green().bold(),
        style(binary_path.display()).yellow(),
        total_duration.as_secs_f64()
    );

    Ok(())
}
