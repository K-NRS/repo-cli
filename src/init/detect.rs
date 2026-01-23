use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Bun,
    Pnpm,
    NextJs,
    NodeJs,
    ReactNative,
    Xcode,
    Go,
    Python,
    Generic,
}

impl ProjectType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Bun => "Bun",
            Self::Pnpm => "Node.js (pnpm)",
            Self::NextJs => "Next.js",
            Self::NodeJs => "Node.js (npm)",
            Self::ReactNative => "React Native",
            Self::Xcode => "Xcode (iOS/macOS)",
            Self::Go => "Go",
            Self::Python => "Python",
            Self::Generic => "Generic",
        }
    }

    pub fn all() -> &'static [ProjectType] {
        &[
            Self::Rust,
            Self::Bun,
            Self::Pnpm,
            Self::NextJs,
            Self::NodeJs,
            Self::ReactNative,
            Self::Xcode,
            Self::Go,
            Self::Python,
            Self::Generic,
        ]
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "bun" => Some(Self::Bun),
            "pnpm" => Some(Self::Pnpm),
            "nextjs" | "next" | "next.js" => Some(Self::NextJs),
            "nodejs" | "node" | "npm" => Some(Self::NodeJs),
            "react-native" | "reactnative" | "rn" => Some(Self::ReactNative),
            "xcode" | "ios" | "macos" | "swift" => Some(Self::Xcode),
            "go" | "golang" => Some(Self::Go),
            "python" | "py" => Some(Self::Python),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }
}

fn file_exists(base: &Path, name: &str) -> bool {
    base.join(name).exists()
}

fn glob_exists(base: &Path, pattern: &str) -> bool {
    let pattern_path = base.join(pattern);
    glob::glob(pattern_path.to_str().unwrap_or(""))
        .map(|paths| paths.filter_map(Result::ok).next().is_some())
        .unwrap_or(false)
}

fn package_json_has_dep(base: &Path, dep: &str) -> bool {
    let pkg_path = base.join("package.json");
    if !pkg_path.exists() {
        return false;
    }

    std::fs::read_to_string(&pkg_path)
        .map(|content| {
            serde_json::from_str::<serde_json::Value>(&content)
                .map(|json| {
                    json.get("dependencies")
                        .and_then(|d| d.get(dep))
                        .is_some()
                        || json.get("devDependencies")
                            .and_then(|d| d.get(dep))
                            .is_some()
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Detect project type from files in the given path
/// Detection priority: more specific types first
pub fn detect_project_type(path: &Path) -> Option<ProjectType> {
    // Rust: Cargo.toml
    if file_exists(path, "Cargo.toml") {
        return Some(ProjectType::Rust);
    }

    // Go: go.mod
    if file_exists(path, "go.mod") {
        return Some(ProjectType::Go);
    }

    // Xcode: *.xcodeproj or *.xcworkspace
    if glob_exists(path, "*.xcodeproj") || glob_exists(path, "*.xcworkspace") {
        return Some(ProjectType::Xcode);
    }

    // Python: pyproject.toml or setup.py
    if file_exists(path, "pyproject.toml") || file_exists(path, "setup.py") {
        return Some(ProjectType::Python);
    }

    // JavaScript/Node ecosystem - check more specific first
    let has_package_json = file_exists(path, "package.json");

    if has_package_json {
        // Next.js: next.config.* files
        if glob_exists(path, "next.config.*") {
            return Some(ProjectType::NextJs);
        }

        // React Native: app.json + react-native dep
        if file_exists(path, "app.json") && package_json_has_dep(path, "react-native") {
            return Some(ProjectType::ReactNative);
        }

        // Bun: bun.lockb
        if file_exists(path, "bun.lockb") {
            return Some(ProjectType::Bun);
        }

        // pnpm: pnpm-lock.yaml
        if file_exists(path, "pnpm-lock.yaml") {
            return Some(ProjectType::Pnpm);
        }

        // Node.js (npm): package-lock.json
        if file_exists(path, "package-lock.json") {
            return Some(ProjectType::NodeJs);
        }

        // Has package.json but no lockfile - default to npm
        return Some(ProjectType::NodeJs);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_rust() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(detect_project_type(dir.path()), Some(ProjectType::Rust));
    }

    #[test]
    fn test_detect_go() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("go.mod"), "module test").unwrap();
        assert_eq!(detect_project_type(dir.path()), Some(ProjectType::Go));
    }

    #[test]
    fn test_detect_bun() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("bun.lockb"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), Some(ProjectType::Bun));
    }

    #[test]
    fn test_detect_none() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_project_type(dir.path()), None);
    }
}
