//! Module tree construction and loading.

use crate::error::LoadError;
use crate::manifest::ProjectManifest;
use sage_parser::ast::Program;
use sage_parser::parse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A module path like `["agents", "researcher"]`.
pub type ModulePath = Vec<String>;

/// A complete module tree for a Sage project.
#[derive(Debug)]
pub struct ModuleTree {
    /// All parsed modules, keyed by their module path.
    pub modules: HashMap<ModulePath, ParsedModule>,
    /// The root module path (usually empty for the entry module).
    pub root: ModulePath,
    /// The project root directory.
    pub project_root: PathBuf,
    /// External package roots, keyed by package name.
    /// Maps package name to its cached path on disk.
    pub external_roots: HashMap<String, PathBuf>,
}

/// A discovered test file with its parsed contents.
#[derive(Debug)]
pub struct TestFile {
    /// The file path on disk.
    pub file_path: PathBuf,
    /// The source code.
    pub source: Arc<str>,
    /// The parsed AST.
    pub program: Program,
}

/// A parsed module with its source and AST.
#[derive(Debug)]
pub struct ParsedModule {
    /// The module's path (e.g., `["agents", "researcher"]`).
    pub path: ModulePath,
    /// The file path on disk.
    pub file_path: PathBuf,
    /// The source code.
    pub source: Arc<str>,
    /// The parsed AST.
    pub program: Program,
}

/// Load a single .sg file (no project structure).
pub fn load_single_file(path: &Path) -> Result<ModuleTree, Vec<LoadError>> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        vec![LoadError::IoError {
            path: path.to_path_buf(),
            source: e,
        }]
    })?;

    let source_arc: Arc<str> = Arc::from(source.as_str());
    let lex_result = sage_parser::lex(&source).map_err(|e| {
        vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: vec![format!("{e}")],
        }]
    })?;

    let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

    if !parse_errors.is_empty() {
        return Err(vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: parse_errors.iter().map(|e| format!("{e}")).collect(),
        }]);
    }

    let program = program.ok_or_else(|| {
        vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: vec!["failed to parse program".to_string()],
        }]
    })?;

    let root_path = vec![];
    let mut modules = HashMap::new();
    modules.insert(
        root_path.clone(),
        ParsedModule {
            path: root_path.clone(),
            file_path: path.to_path_buf(),
            source: source_arc,
            program,
        },
    );

    Ok(ModuleTree {
        modules,
        root: root_path,
        project_root: path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        external_roots: HashMap::new(),
    })
}

/// Load a project from a grove.toml or project directory.
///
/// This does NOT resolve external dependencies. For that, use `load_project_with_packages`.
pub fn load_project(project_path: &Path) -> Result<ModuleTree, Vec<LoadError>> {
    // Find the manifest
    let manifest_path = if project_path.is_file() && project_path.ends_with("grove.toml") {
        project_path.to_path_buf()
    } else if project_path.is_dir() {
        // Try grove.toml first, fall back to sage.toml with deprecation warning
        let grove_path = project_path.join("grove.toml");
        let sage_path = project_path.join("sage.toml");
        if grove_path.exists() {
            grove_path
        } else if sage_path.exists() {
            eprintln!("warning: sage.toml is deprecated, rename to grove.toml");
            sage_path
        } else {
            project_path.join("grove.toml") // Will fail with proper error
        }
    } else {
        // It's a .sg file - treat as single file
        return load_single_file(project_path);
    };

    if !manifest_path.exists() {
        // No manifest - treat as single file if it's a .sg
        if project_path.extension().is_some_and(|e| e == "sg") {
            return load_single_file(project_path);
        }
        return Err(vec![LoadError::NoManifest {
            dir: project_path.to_path_buf(),
        }]);
    }

    let manifest = ProjectManifest::load(&manifest_path).map_err(|e| vec![e])?;
    let project_root = manifest_path.parent().unwrap().to_path_buf();
    let entry_path = project_root.join(&manifest.project.entry);

    if !entry_path.exists() {
        return Err(vec![LoadError::MissingEntry { path: entry_path }]);
    }

    // Load the module tree starting from the entry point
    let mut loader = ModuleLoader::new(project_root.clone());
    let root_path: ModulePath = vec![];
    loader.load_module(&root_path, &entry_path)?;

    Ok(ModuleTree {
        modules: loader.modules,
        root: vec![],
        project_root,
        external_roots: HashMap::new(),
    })
}

/// Load a project with external package resolution.
///
/// This function will:
/// 1. Load the project manifest
/// 2. Check for dependencies
/// 3. If lock file exists and is fresh, use it; otherwise resolve dependencies
/// 4. Load all external packages into the module tree
pub fn load_project_with_packages(
    project_path: &Path,
) -> Result<(ModuleTree, bool), Vec<LoadError>> {
    use sage_package::{check_lock_freshness, install_from_lock, resolve_dependencies, LockFile};

    // First, do the basic project loading to check if it's a valid project
    let manifest_path = if project_path.is_file() && project_path.ends_with("grove.toml") {
        project_path.to_path_buf()
    } else if project_path.is_dir() {
        // Try grove.toml first, fall back to sage.toml with deprecation warning
        let grove_path = project_path.join("grove.toml");
        let sage_path = project_path.join("sage.toml");
        if grove_path.exists() {
            grove_path
        } else if sage_path.exists() {
            eprintln!("warning: sage.toml is deprecated, rename to grove.toml");
            sage_path
        } else {
            project_path.join("grove.toml") // Will fail with proper error
        }
    } else {
        // Single file - no packages
        let tree = load_single_file(project_path)?;
        return Ok((tree, false));
    };

    if !manifest_path.exists() {
        if project_path.extension().is_some_and(|e| e == "sg") {
            let tree = load_single_file(project_path)?;
            return Ok((tree, false));
        }
        return Err(vec![LoadError::NoManifest {
            dir: project_path.to_path_buf(),
        }]);
    }

    let manifest = ProjectManifest::load(&manifest_path).map_err(|e| vec![e])?;
    let project_root = manifest_path.parent().unwrap().to_path_buf();

    // Parse dependencies
    let deps = manifest.parse_dependencies().map_err(|e| vec![e])?;

    // Resolve external packages
    let external_roots = if deps.is_empty() {
        HashMap::new()
    } else {
        let lock_path = project_root.join("grove.lock");
        let packages = if lock_path.exists() {
            let lock = LockFile::load(&lock_path)
                .map_err(|e| vec![LoadError::PackageError { source: e }])?;
            if check_lock_freshness(&deps, &lock) {
                // Lock file is fresh - install from lock
                install_from_lock(&project_root, &lock)
                    .map_err(|e| vec![LoadError::PackageError { source: e }])?
            } else {
                // Lock file is stale - re-resolve
                let resolved = resolve_dependencies(&project_root, &deps, Some(&lock))
                    .map_err(|e| vec![LoadError::PackageError { source: e }])?;
                resolved.packages
            }
        } else {
            // No lock file - resolve fresh
            let resolved = resolve_dependencies(&project_root, &deps, None)
                .map_err(|e| vec![LoadError::PackageError { source: e }])?;
            resolved.packages
        };

        packages
            .into_iter()
            .map(|(name, pkg)| (name, pkg.path))
            .collect()
    };

    // Load the main project
    let entry_path = project_root.join(&manifest.project.entry);
    if !entry_path.exists() {
        return Err(vec![LoadError::MissingEntry { path: entry_path }]);
    }

    let mut loader = ModuleLoader::new(project_root.clone());
    let root_path: ModulePath = vec![];
    loader.load_module(&root_path, &entry_path)?;

    let installed = !external_roots.is_empty();

    Ok((
        ModuleTree {
            modules: loader.modules,
            root: vec![],
            project_root,
            external_roots,
        },
        installed,
    ))
}

/// Discover all `*_test.sg` files in a project.
///
/// Walks the source directory and collects all files ending in `_test.sg`.
/// Files in `hearth/` (build output) are excluded.
pub fn discover_test_files(project_path: &Path) -> Result<Vec<PathBuf>, Vec<LoadError>> {
    let project_root = if project_path.is_file() {
        project_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    } else {
        project_path.to_path_buf()
    };

    let src_dir = project_root.join("src");
    let search_dir = if src_dir.exists() {
        src_dir
    } else {
        project_root
    };

    let mut test_files = Vec::new();
    collect_test_files(&search_dir, &mut test_files)?;

    // Sort for deterministic ordering
    test_files.sort();

    Ok(test_files)
}

fn collect_test_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Vec<LoadError>> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        vec![LoadError::IoError {
            path: dir.to_path_buf(),
            source: e,
        }]
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            vec![LoadError::IoError {
                path: dir.to_path_buf(),
                source: e,
            }]
        })?;

        let path = entry.path();

        // Skip hearth (build output directory)
        if path.file_name().is_some_and(|n| n == "hearth") {
            continue;
        }

        if path.is_dir() {
            collect_test_files(&path, out)?;
        } else if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with("_test.sg") {
                    out.push(path);
                }
            }
        }
    }

    Ok(())
}

/// Load all test files in a project.
///
/// Returns a vector of parsed test files. Each test file is parsed independently.
pub fn load_test_files(project_path: &Path) -> Result<Vec<TestFile>, Vec<LoadError>> {
    let test_paths = discover_test_files(project_path)?;
    let mut test_files = Vec::new();
    let mut errors = Vec::new();

    for path in test_paths {
        match load_test_file(&path) {
            Ok(tf) => test_files.push(tf),
            Err(mut errs) => errors.append(&mut errs),
        }
    }

    if errors.is_empty() {
        Ok(test_files)
    } else {
        Err(errors)
    }
}

/// Load a single test file.
fn load_test_file(path: &Path) -> Result<TestFile, Vec<LoadError>> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        vec![LoadError::IoError {
            path: path.to_path_buf(),
            source: e,
        }]
    })?;

    let source_arc: Arc<str> = Arc::from(source.as_str());
    let lex_result = sage_parser::lex(&source).map_err(|e| {
        vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: vec![format!("{e}")],
        }]
    })?;

    let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

    if !parse_errors.is_empty() {
        return Err(vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: parse_errors.iter().map(|e| format!("{e}")).collect(),
        }]);
    }

    let program = program.ok_or_else(|| {
        vec![LoadError::ParseError {
            file: path.to_path_buf(),
            errors: vec!["failed to parse program".to_string()],
        }]
    })?;

    Ok(TestFile {
        file_path: path.to_path_buf(),
        source: source_arc,
        program,
    })
}

/// Internal loader that tracks state during recursive loading.
struct ModuleLoader {
    #[allow(dead_code)]
    project_root: PathBuf,
    modules: HashMap<ModulePath, ParsedModule>,
    loading: HashSet<PathBuf>, // Currently loading (for cycle detection)
}

impl ModuleLoader {
    fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            modules: HashMap::new(),
            loading: HashSet::new(),
        }
    }

    fn load_module(&mut self, path: &ModulePath, file_path: &Path) -> Result<(), Vec<LoadError>> {
        let canonical = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());

        // Check for cycles
        if self.loading.contains(&canonical) {
            let cycle: Vec<String> = self
                .loading
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            return Err(vec![LoadError::CircularDependency { cycle }]);
        }

        // Already loaded?
        if self.modules.contains_key(path) {
            return Ok(());
        }

        self.loading.insert(canonical.clone());

        // Read and parse
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            vec![LoadError::IoError {
                path: file_path.to_path_buf(),
                source: e,
            }]
        })?;

        let source_arc: Arc<str> = Arc::from(source.as_str());
        let lex_result = sage_parser::lex(&source).map_err(|e| {
            vec![LoadError::ParseError {
                file: file_path.to_path_buf(),
                errors: vec![format!("{e}")],
            }]
        })?;

        let (program, parse_errors) = parse(lex_result.tokens(), Arc::clone(&source_arc));

        if !parse_errors.is_empty() {
            return Err(vec![LoadError::ParseError {
                file: file_path.to_path_buf(),
                errors: parse_errors.iter().map(|e| format!("{e}")).collect(),
            }]);
        }

        let program = program.ok_or_else(|| {
            vec![LoadError::ParseError {
                file: file_path.to_path_buf(),
                errors: vec!["failed to parse program".to_string()],
            }]
        })?;

        // Process mod declarations to find child modules
        let parent_dir = file_path.parent().unwrap();
        let file_stem = file_path.file_stem().unwrap().to_str().unwrap();
        let is_mod_file = file_stem == "mod";

        for mod_decl in &program.mod_decls {
            let child_name = &mod_decl.name.name;
            let mut child_path = path.clone();
            child_path.push(child_name.clone());

            // Find the child module file
            let child_file = self.find_module_file(parent_dir, child_name, is_mod_file)?;

            // Recursively load
            self.load_module(&child_path, &child_file)?;
        }

        self.loading.remove(&canonical);

        // Store the module
        self.modules.insert(
            path.clone(),
            ParsedModule {
                path: path.clone(),
                file_path: file_path.to_path_buf(),
                source: source_arc,
                program,
            },
        );

        Ok(())
    }

    fn find_module_file(
        &self,
        parent_dir: &Path,
        mod_name: &str,
        _parent_is_mod_file: bool,
    ) -> Result<PathBuf, Vec<LoadError>> {
        // Try two locations:
        // 1. mod_name.sg (sibling file)
        // 2. mod_name/mod.sg (directory with mod.sg)
        let sibling = parent_dir.join(format!("{mod_name}.sg"));
        let nested = parent_dir.join(mod_name).join("mod.sg");

        let sibling_exists = sibling.exists();
        let nested_exists = nested.exists();

        match (sibling_exists, nested_exists) {
            (true, true) => Err(vec![LoadError::AmbiguousModule {
                mod_name: mod_name.to_string(),
                candidates: vec![sibling, nested],
            }]),
            (true, false) => Ok(sibling),
            (false, true) => Ok(nested),
            (false, false) => Err(vec![LoadError::FileNotFound {
                mod_name: mod_name.to_string(),
                searched: vec![sibling, nested],
                span: (0, 0).into(),
                source_code: String::new(),
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_single_file_works() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.sg");
        fs::write(
            &file,
            r#"
agent Main {
    on start {
        emit(42);
    }
}
run Main;
"#,
        )
        .unwrap();

        let tree = load_single_file(&file).unwrap();
        assert_eq!(tree.modules.len(), 1);
        assert!(tree.modules.contains_key(&vec![]));
    }

    #[test]
    fn load_project_with_manifest() {
        let dir = TempDir::new().unwrap();

        // Create grove.toml
        fs::write(
            dir.path().join("grove.toml"),
            r#"
[project]
name = "test"
entry = "src/main.sg"
"#,
        )
        .unwrap();

        // Create src/main.sg
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/main.sg"),
            r#"
agent Main {
    on start {
        emit(0);
    }
}
run Main;
"#,
        )
        .unwrap();

        let tree = load_project(dir.path()).unwrap();
        assert_eq!(tree.modules.len(), 1);
    }

    #[test]
    fn load_project_with_submodule() {
        let dir = TempDir::new().unwrap();

        // Create grove.toml
        fs::write(
            dir.path().join("grove.toml"),
            r#"
[project]
name = "test"
entry = "src/main.sg"
"#,
        )
        .unwrap();

        // Create src/main.sg with mod declaration
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/main.sg"),
            r#"
mod agents;

agent Main {
    on start {
        emit(0);
    }
}
run Main;
"#,
        )
        .unwrap();

        // Create src/agents.sg
        fs::write(
            dir.path().join("src/agents.sg"),
            r#"
pub agent Worker {
    on start {
        emit(1);
    }
}
"#,
        )
        .unwrap();

        let tree = load_project(dir.path()).unwrap();
        assert_eq!(tree.modules.len(), 2);
        assert!(tree.modules.contains_key(&vec![]));
        assert!(tree.modules.contains_key(&vec!["agents".to_string()]));
    }

    #[test]
    fn discover_test_files_finds_all() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();

        // Create main file and test files
        fs::write(
            dir.path().join("src/main.sg"),
            "agent Main { on start { emit(0); } } run Main;",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/counter_test.sg"),
            "test \"counter works\" { assert(true); }",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/worker_test.sg"),
            "test \"worker works\" { assert(true); }",
        )
        .unwrap();

        let test_files = discover_test_files(dir.path()).unwrap();
        assert_eq!(test_files.len(), 2);
        assert!(test_files.iter().any(|p| p.ends_with("counter_test.sg")));
        assert!(test_files.iter().any(|p| p.ends_with("worker_test.sg")));
    }

    #[test]
    fn discover_test_files_skips_hearth() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("hearth")).unwrap();

        fs::write(
            dir.path().join("src/main.sg"),
            "agent Main { on start { emit(0); } } run Main;",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/counter_test.sg"),
            "test \"counter\" { assert(true); }",
        )
        .unwrap();
        // This should be skipped
        fs::write(
            dir.path().join("hearth/generated_test.sg"),
            "test \"gen\" { assert(true); }",
        )
        .unwrap();

        let test_files = discover_test_files(dir.path()).unwrap();
        assert_eq!(test_files.len(), 1);
        assert!(test_files[0].ends_with("counter_test.sg"));
    }

    #[test]
    fn load_test_files_parses_all() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();

        fs::write(
            dir.path().join("src/main.sg"),
            "agent Main { on start { emit(0); } } run Main;",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/math_test.sg"),
            r#"
test "addition works" {
    let x = 1 + 2;
    assert(x == 3);
}

test "subtraction works" {
    let y = 5 - 3;
    assert(y == 2);
}
"#,
        )
        .unwrap();

        let test_files = load_test_files(dir.path()).unwrap();
        assert_eq!(test_files.len(), 1);
        assert_eq!(test_files[0].program.tests.len(), 2);
    }
}
