//! Integration tests for the package manager.

use sage_package::{parse_dependencies, DependencySpec, LockFile, LockedPackage, PackageCache};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn dependency_spec_serialization_roundtrip() {
    let spec = DependencySpec::with_tag("https://github.com/example/foo", "v1.0.0");
    let toml = toml::to_string(&spec).unwrap();
    let parsed: DependencySpec = toml::from_str(&toml).unwrap();
    assert_eq!(spec, parsed);
}

#[test]
fn path_dependency_spec_serialization_roundtrip() {
    let spec = DependencySpec::with_path("../my-local-lib");
    let toml = toml::to_string(&spec).unwrap();
    let parsed: DependencySpec = toml::from_str(&toml).unwrap();
    assert_eq!(spec, parsed);
}

#[test]
fn lock_file_roundtrip() {
    let lock = LockFile {
        version: 1,
        packages: vec![
            LockedPackage::git(
                "foo".to_string(),
                "1.0.0".to_string(),
                "https://github.com/example/foo".to_string(),
                "abc123def456789".to_string(),
                vec![],
            ),
            LockedPackage::git(
                "bar".to_string(),
                "2.0.0".to_string(),
                "https://github.com/example/bar".to_string(),
                "def456abc789xyz".to_string(),
                vec!["foo".to_string()],
            ),
        ],
    };

    let dir = TempDir::new().unwrap();
    let lock_path = dir.path().join("grove.lock");

    lock.save(&lock_path).unwrap();
    let loaded = LockFile::load(&lock_path).unwrap();

    assert_eq!(loaded.version, lock.version);
    assert_eq!(loaded.packages.len(), lock.packages.len());
    assert_eq!(loaded.packages[0].name, "foo");
    assert_eq!(loaded.packages[1].dependencies, vec!["foo"]);
}

#[test]
fn lock_file_path_dependency_roundtrip() {
    let lock = LockFile {
        version: 1,
        packages: vec![LockedPackage::path(
            "local-lib".to_string(),
            "0.1.0".to_string(),
            "../local-lib".to_string(),
            vec![],
        )],
    };

    let dir = TempDir::new().unwrap();
    let lock_path = dir.path().join("grove.lock");

    lock.save(&lock_path).unwrap();
    let loaded = LockFile::load(&lock_path).unwrap();

    assert_eq!(loaded.packages.len(), 1);
    assert!(loaded.packages[0].is_path());
    assert_eq!(loaded.packages[0].path, Some("../local-lib".to_string()));
}

#[test]
fn parse_dependencies_table() {
    let table: toml::Table = toml::from_str(
        r#"
[http_client]
git = "https://github.com/sage-packages/http"
tag = "v1.0.0"

[json_parser]
git = "https://github.com/sage-packages/json"
branch = "main"

[utils]
git = "https://github.com/sage-packages/utils"
rev = "abc123"

[local]
path = "../local-lib"
"#,
    )
    .unwrap();

    let deps = parse_dependencies(&table).unwrap();
    assert_eq!(deps.len(), 4);

    assert!(deps.contains_key("http_client"));
    assert!(deps["http_client"].is_git());

    assert!(deps.contains_key("json_parser"));
    assert!(deps["json_parser"].is_git());

    assert!(deps.contains_key("utils"));
    assert!(deps["utils"].is_git());

    assert!(deps.contains_key("local"));
    assert!(deps["local"].is_path());
    assert_eq!(deps["local"].path(), Some("../local-lib"));
}

#[test]
fn lock_file_matches_deps() {
    let mut deps = HashMap::new();
    deps.insert(
        "foo".to_string(),
        DependencySpec::with_tag("https://github.com/example/foo", "v1.0.0"),
    );

    let lock = LockFile {
        version: 1,
        packages: vec![LockedPackage::git(
            "foo".to_string(),
            "1.0.0".to_string(),
            "https://github.com/example/foo".to_string(),
            "abc123".to_string(),
            vec![],
        )],
    };

    assert!(lock.matches_dependencies(&deps));
}

#[test]
fn lock_file_matches_path_deps() {
    let mut deps = HashMap::new();
    deps.insert("local".to_string(), DependencySpec::with_path("../lib"));

    let lock = LockFile {
        version: 1,
        packages: vec![LockedPackage::path(
            "local".to_string(),
            "0.1.0".to_string(),
            "../lib".to_string(),
            vec![],
        )],
    };

    assert!(lock.matches_dependencies(&deps));
}

#[test]
fn lock_file_does_not_match_missing_dep() {
    let mut deps = HashMap::new();
    deps.insert(
        "foo".to_string(),
        DependencySpec::with_tag("https://github.com/example/foo", "v1.0.0"),
    );
    deps.insert(
        "bar".to_string(),
        DependencySpec::with_tag("https://github.com/example/bar", "v2.0.0"),
    );

    let lock = LockFile {
        version: 1,
        packages: vec![LockedPackage::git(
            "foo".to_string(),
            "1.0.0".to_string(),
            "https://github.com/example/foo".to_string(),
            "abc123".to_string(),
            vec![],
        )],
    };

    assert!(!lock.matches_dependencies(&deps));
}

#[test]
fn package_cache_can_be_created() {
    // Just verify the cache can be created without errors
    let cache = PackageCache::new().unwrap();
    assert!(cache.root().exists() || cache.root().parent().is_some());
}

#[test]
fn lock_file_dependency_ordering() {
    let lock = LockFile {
        version: 1,
        packages: vec![
            // Define in reverse order to test sorting
            LockedPackage::git(
                "app".to_string(),
                "1.0.0".to_string(),
                "https://example.com/app".to_string(),
                "app123".to_string(),
                vec!["core".to_string(), "utils".to_string()],
            ),
            LockedPackage::git(
                "utils".to_string(),
                "1.0.0".to_string(),
                "https://example.com/utils".to_string(),
                "utils123".to_string(),
                vec!["core".to_string()],
            ),
            LockedPackage::git(
                "core".to_string(),
                "1.0.0".to_string(),
                "https://example.com/core".to_string(),
                "core123".to_string(),
                vec![],
            ),
        ],
    };

    let ordered = lock.in_dependency_order();
    let names: Vec<&str> = ordered.iter().map(|p| p.name.as_str()).collect();

    // core must come before utils and app
    // utils must come before app
    let core_pos = names.iter().position(|&n| n == "core").unwrap();
    let utils_pos = names.iter().position(|&n| n == "utils").unwrap();
    let app_pos = names.iter().position(|&n| n == "app").unwrap();

    assert!(core_pos < utils_pos, "core should come before utils");
    assert!(core_pos < app_pos, "core should come before app");
    assert!(utils_pos < app_pos, "utils should come before app");
}
