use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Item, UseTree};

type ModuleKey = Vec<String>;

pub fn read_sources<P: AsRef<Path>>(paths: &[P]) -> Result<String, ResolveError> {
    let mut source = String::new();

    for path in paths {
        source.push_str(&resolve_entry(path.as_ref())?);
    }

    Ok(source)
}

fn resolve_entry(entry_path: &Path) -> Result<String, ResolveError> {
    let entry_path = entry_path
        .canonicalize()
        .map_err(|e| ResolveError::FileRead(entry_path.to_path_buf(), e.to_string()))?;
    let mut loader = ModuleLoader::default();
    loader.load_root(&entry_path)?;

    let mut included_modules = HashSet::new();
    let mut ordered_modules = Vec::new();
    loader.collect_imports(&Vec::new(), &mut included_modules, &mut ordered_modules)?;

    let mut emitter = FlatEmitter::default();
    for module_key in ordered_modules {
        let module = loader
            .modules
            .get(&module_key)
            .expect("included module must exist");
        emitter.push_module(module)?;
    }

    let root = loader
        .modules
        .get(&Vec::new())
        .expect("root module must exist");
    emitter.push_root_items(root)?;

    Ok(emitter.finish())
}

#[derive(Default)]
struct ModuleLoader {
    modules: HashMap<ModuleKey, ModuleData>,
}

struct ModuleData {
    key: ModuleKey,
    file_path: PathBuf,
    items: Vec<Item>,
}

impl ModuleLoader {
    fn load_root(&mut self, entry_path: &Path) -> Result<(), ResolveError> {
        let resolve_dir = entry_path
            .parent()
            .ok_or_else(|| ResolveError::MissingParent(entry_path.to_path_buf()))?
            .to_path_buf();
        let items = parse_file(entry_path)?;
        self.load_module(Vec::new(), entry_path.to_path_buf(), resolve_dir, items)
    }

    fn load_module(
        &mut self,
        key: ModuleKey,
        file_path: PathBuf,
        resolve_dir: PathBuf,
        items: Vec<Item>,
    ) -> Result<(), ResolveError> {
        if self.modules.contains_key(&key) {
            return Ok(());
        }

        for item in &items {
            if let Item::Mod(item_mod) = item {
                self.load_child_module(&key, &resolve_dir, item_mod)?;
            }
        }

        self.modules.insert(
            key.clone(),
            ModuleData {
                key,
                file_path,
                items,
            },
        );
        Ok(())
    }

    fn load_child_module(
        &mut self,
        parent_key: &ModuleKey,
        parent_resolve_dir: &Path,
        item_mod: &syn::ItemMod,
    ) -> Result<(), ResolveError> {
        let ident = item_mod.ident.to_string();
        let mut child_key = parent_key.clone();
        child_key.push(ident.clone());
        let child_resolve_dir = parent_resolve_dir.join(&ident);

        if let Some((_, items)) = &item_mod.content {
            return self.load_module(
                child_key,
                parent_resolve_dir.join(format!("{ident}.inline.rs")),
                child_resolve_dir,
                items.clone(),
            );
        }

        let child_path = resolve_module_path(parent_resolve_dir, &ident).ok_or_else(|| {
            ResolveError::ModuleNotFound {
                module: display_module_path(&child_key),
                searched_from: parent_resolve_dir.to_path_buf(),
            }
        })?;
        let items = parse_file(&child_path)?;
        self.load_module(child_key, child_path, child_resolve_dir, items)
    }

    fn collect_imports(
        &self,
        module_key: &ModuleKey,
        included_modules: &mut HashSet<ModuleKey>,
        ordered_modules: &mut Vec<ModuleKey>,
    ) -> Result<(), ResolveError> {
        let module = self
            .modules
            .get(module_key)
            .ok_or_else(|| ResolveError::UnknownModule(display_module_path(module_key)))?;

        for item in &module.items {
            if let Item::Use(item_use) = item {
                for target in collect_use_targets(&item_use.tree) {
                    if target.is_empty() {
                        continue;
                    }
                    self.include_module(&target, included_modules, ordered_modules)?;
                }
            }
        }

        Ok(())
    }

    fn include_module(
        &self,
        module_key: &ModuleKey,
        included_modules: &mut HashSet<ModuleKey>,
        ordered_modules: &mut Vec<ModuleKey>,
    ) -> Result<(), ResolveError> {
        if !included_modules.insert(module_key.clone()) {
            return Ok(());
        }

        let module = self
            .modules
            .get(module_key)
            .ok_or_else(|| ResolveError::UnknownModule(display_module_path(module_key)))?;

        self.collect_imports(&module.key, included_modules, ordered_modules)?;
        ordered_modules.push(module.key.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FlatEmitter {
    names: HashMap<String, PathBuf>,
    source: String,
}

impl FlatEmitter {
    fn push_module(&mut self, module: &ModuleData) -> Result<(), ResolveError> {
        self.push_items(module)
    }

    fn push_root_items(&mut self, module: &ModuleData) -> Result<(), ResolveError> {
        self.push_items(module)
    }

    fn push_items(&mut self, module: &ModuleData) -> Result<(), ResolveError> {
        for item in &module.items {
            if matches!(item, Item::Use(_) | Item::Mod(_)) {
                continue;
            }
            self.record_name(item, &module.file_path)?;
            self.source.push_str(&item.to_token_stream().to_string());
            self.source.push('\n');
        }
        Ok(())
    }

    fn record_name(&mut self, item: &Item, file_path: &Path) -> Result<(), ResolveError> {
        let Some(name) = item_name(item) else {
            return Ok(());
        };

        if let Some(previous) = self.names.insert(name.clone(), file_path.to_path_buf()) {
            return Err(ResolveError::DuplicateItem {
                name,
                first: previous,
                second: file_path.to_path_buf(),
            });
        }

        Ok(())
    }

    fn finish(self) -> String {
        self.source
    }
}

fn parse_file(path: &Path) -> Result<Vec<Item>, ResolveError> {
    let source = fs::read_to_string(path)
        .map_err(|e| ResolveError::FileRead(path.to_path_buf(), e.to_string()))?;
    let file = syn::parse_file(&source)
        .map_err(|e| ResolveError::Parse(path.to_path_buf(), e.to_string()))?;
    Ok(file.items)
}

fn resolve_module_path(parent_resolve_dir: &Path, ident: &str) -> Option<PathBuf> {
    let direct = parent_resolve_dir.join(format!("{ident}.rs"));
    if direct.is_file() {
        return Some(direct);
    }

    let nested = parent_resolve_dir.join(ident).join("mod.rs");
    if nested.is_file() {
        return Some(nested);
    }

    None
}

fn collect_use_targets(tree: &UseTree) -> Vec<ModuleKey> {
    let mut targets = Vec::new();
    collect_use_targets_inner(tree, None, &mut targets);
    targets
}

fn collect_use_targets_inner(tree: &UseTree, prefix: Option<ModuleKey>, out: &mut Vec<ModuleKey>) {
    match tree {
        UseTree::Path(path) if prefix.is_none() && path.ident == "crate" => {
            collect_use_targets_inner(&path.tree, Some(Vec::new()), out);
        }
        UseTree::Path(path) => {
            let Some(mut prefix) = prefix else {
                return;
            };
            prefix.push(path.ident.to_string());
            collect_use_targets_inner(&path.tree, Some(prefix), out);
        }
        UseTree::Group(group) => {
            let Some(prefix) = prefix else {
                return;
            };
            for tree in &group.items {
                collect_use_targets_inner(tree, Some(prefix.clone()), out);
            }
        }
        UseTree::Glob(_) => {
            if let Some(prefix) = prefix {
                out.push(prefix);
            }
        }
        UseTree::Name(name) => {
            let Some(prefix) = prefix else {
                return;
            };
            if name.ident == "self" {
                out.push(prefix);
            } else if prefix.is_empty() {
                out.push(vec![name.ident.to_string()]);
            } else {
                out.push(prefix);
            }
        }
        UseTree::Rename(rename) => {
            let Some(prefix) = prefix else {
                return;
            };
            if rename.ident == "self" {
                out.push(prefix);
            } else if prefix.is_empty() {
                out.push(vec![rename.ident.to_string()]);
            } else {
                out.push(prefix);
            }
        }
    }
}

fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Const(item) => Some(item.ident.to_string()),
        Item::Enum(item) => Some(item.ident.to_string()),
        Item::Fn(item) => Some(item.sig.ident.to_string()),
        Item::Static(item) => Some(item.ident.to_string()),
        Item::Struct(item) => Some(item.ident.to_string()),
        Item::Trait(item) => Some(item.ident.to_string()),
        Item::Type(item) => Some(item.ident.to_string()),
        Item::Union(item) => Some(item.ident.to_string()),
        _ => None,
    }
}

fn display_module_path(key: &[String]) -> String {
    if key.is_empty() {
        "crate".to_string()
    } else {
        format!("crate::{}", key.join("::"))
    }
}

#[derive(Debug)]
pub enum ResolveError {
    DuplicateItem {
        name: String,
        first: PathBuf,
        second: PathBuf,
    },
    FileRead(PathBuf, String),
    MissingParent(PathBuf),
    ModuleNotFound {
        module: String,
        searched_from: PathBuf,
    },
    Parse(PathBuf, String),
    UnknownModule(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateItem {
                name,
                first,
                second,
            } => write!(
                f,
                "Duplicate item `{name}` while flattening modules: {} and {}",
                first.display(),
                second.display()
            ),
            Self::FileRead(path, err) => write!(f, "File read error ({}): {err}", path.display()),
            Self::MissingParent(path) => {
                write!(f, "Cannot resolve parent directory for {}", path.display())
            }
            Self::ModuleNotFound {
                module,
                searched_from,
            } => write!(
                f,
                "Module not found for `{module}` under {}",
                searched_from.display()
            ),
            Self::Parse(path, err) => write!(f, "Parse error ({}): {err}", path.display()),
            Self::UnknownModule(module) => write!(f, "Unknown module `{module}`"),
        }
    }
}

impl std::error::Error for ResolveError {}

#[cfg(test)]
mod tests {
    use super::read_sources;
    use shader_transpiler::transpile_to_glsl;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const TEST_PRELUDE: &str =
        concat!("#[builtin(\"vec4\")] fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {}\n",);

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("shader-solver-{name}-{nanos}.rs"))
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("shader-solver-{name}-{nanos}"))
    }

    fn write_file(path: &Path, source: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }
        fs::write(path, source).expect("failed to write file");
    }

    #[test]
    fn reads_multiple_files_in_order() {
        let first = temp_file_path("first");
        let second = temp_file_path("second");

        fs::write(&first, "fn a() {}\n").expect("failed to write first file");
        fs::write(&second, "fn b() {}\n").expect("failed to write second file");

        let paths = vec![first.clone(), second.clone()];
        let source = read_sources(&paths).expect("failed to read sources");

        assert!(source.contains("fn a"));
        assert!(source.contains("fn b"));

        fs::remove_file(first).expect("failed to remove first file");
        fs::remove_file(second).expect("failed to remove second file");
    }

    #[test]
    fn resolves_local_modules_and_flattens_imports() {
        let dir = temp_dir_path("module-flatten");
        let root = dir.join("shader.rs");
        let helper = dir.join("helper.rs");
        let math = dir.join("helper").join("math.rs");

        write_file(
            &root,
            "\
mod helper;
use crate::helper::*;

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(helper(time), 0.0, 0.0, 1.0)
}
",
        );
        write_file(
            &helper,
            "\
mod math;
use crate::helper::math::*;

fn helper(x: f32) -> f32 {
    double(x)
}
",
        );
        write_file(
            &math,
            "\
fn double(x: f32) -> f32 {
    x * 2.0
}
",
        );

        let source = read_sources(&[root]).expect("resolve failed");
        assert!(source.contains("fn helper"));
        assert!(source.contains("fn double"));
        assert!(!source.contains("mod helper;"));
        assert!(!source.contains("use crate::helper::*;"));

        let glsl = transpile_to_glsl(&format!("{TEST_PRELUDE}{source}")).expect("transpile failed");
        assert!(glsl.contains("float double(float x);"));
        assert!(glsl.contains("float helper(float x);"));
        assert!(glsl.contains("return vec4(helper(time), 0.0, 0.0, 1.0);"));

        fs::remove_dir_all(dir).expect("failed to clean temp dir");
    }
}
