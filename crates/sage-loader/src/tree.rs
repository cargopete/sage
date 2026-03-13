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
    let lex_result = sage_lexer::lex(&source).map_err(|e| {
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
    })
}

/// Load a project from a sage.toml or project directory.
pub fn load_project(project_path: &Path) -> Result<ModuleTree, Vec<LoadError>> {
    // Find the manifest
    let manifest_path = if project_path.is_file() && project_path.ends_with("sage.toml") {
        project_path.to_path_buf()
    } else if project_path.is_dir() {
        project_path.join("sage.toml")
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
        let lex_result = sage_lexer::lex(&source).map_err(|e| {
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

        // Create sage.toml
        fs::write(
            dir.path().join("sage.toml"),
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

        // Create sage.toml
        fs::write(
            dir.path().join("sage.toml"),
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
}
