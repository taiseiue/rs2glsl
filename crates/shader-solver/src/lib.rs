use quote::ToTokens;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Item, UseTree};

type ModulePath = Vec<String>;

pub fn read_sources<P: AsRef<Path>>(paths: &[P]) -> Result<String, ResolveError> {
    let mut source = String::new();

    for path in paths {
        let config = ResolveConfig::discover(path.as_ref())?;
        source.push_str(&resolve_entry(path.as_ref(), &config)?);
    }

    Ok(source)
}

fn resolve_entry(entry_path: &Path, config: &ResolveConfig) -> Result<String, ResolveError> {
    let entry_path = entry_path
        .canonicalize()
        .map_err(|e| ResolveError::FileRead(entry_path.to_path_buf(), e.to_string()))?;
    let mut loader = ModuleLoader::new(config.clone());
    let root_id = ModuleId::current(Vec::new());
    loader.load_root(&root_id, &entry_path)?;

    let mut included_modules = HashSet::new();
    let mut ordered_modules = Vec::new();
    loader.collect_imports(&root_id, &mut included_modules, &mut ordered_modules)?;

    let mut emitter = FlatEmitter::default();
    for module_id in ordered_modules {
        let module = loader
            .modules
            .get(&module_id)
            .expect("included module must exist");
        emitter.push_module(module)?;
    }

    let root = loader
        .modules
        .get(&root_id)
        .expect("root module must exist");
    emitter.push_root_items(root)?;

    Ok(emitter.finish())
}

#[derive(Clone)]
struct ResolveConfig {
    cargo_home: PathBuf,
    lockfile_path: Option<PathBuf>,
}

impl ResolveConfig {
    fn discover(entry_path: &Path) -> Result<Self, ResolveError> {
        Ok(Self {
            cargo_home: detect_cargo_home()?,
            lockfile_path: find_ancestor_file(entry_path, "Cargo.lock"),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CrateId {
    Current,
    External(String),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ModuleId {
    crate_id: CrateId,
    path: ModulePath,
}

impl ModuleId {
    fn current(path: ModulePath) -> Self {
        Self {
            crate_id: CrateId::Current,
            path,
        }
    }

    fn external(crate_name: impl Into<String>, path: ModulePath) -> Self {
        Self {
            crate_id: CrateId::External(crate_name.into()),
            path,
        }
    }
}

struct ModuleLoader {
    config: ResolveConfig,
    lockfile: Option<Lockfile>,
    external_libs: HashMap<String, PathBuf>,
    modules: HashMap<ModuleId, ModuleData>,
}

impl ModuleLoader {
    fn new(config: ResolveConfig) -> Self {
        Self {
            config,
            lockfile: None,
            external_libs: HashMap::new(),
            modules: HashMap::new(),
        }
    }

    fn load_root(&mut self, root_id: &ModuleId, entry_path: &Path) -> Result<(), ResolveError> {
        let resolve_dir = entry_path
            .parent()
            .ok_or_else(|| ResolveError::MissingParent(entry_path.to_path_buf()))?
            .to_path_buf();
        let items = parse_file(entry_path)?;
        self.load_module(
            root_id.clone(),
            entry_path.to_path_buf(),
            resolve_dir,
            items,
        )
    }

    fn load_module(
        &mut self,
        module_id: ModuleId,
        file_path: PathBuf,
        resolve_dir: PathBuf,
        items: Vec<Item>,
    ) -> Result<(), ResolveError> {
        if self.modules.contains_key(&module_id) {
            return Ok(());
        }

        for item in &items {
            if let Item::Mod(item_mod) = item {
                self.load_child_module(&module_id, &resolve_dir, item_mod)?;
            }
        }

        self.modules.insert(
            module_id.clone(),
            ModuleData {
                module_id,
                file_path,
                items,
            },
        );
        Ok(())
    }

    fn load_child_module(
        &mut self,
        parent_id: &ModuleId,
        parent_resolve_dir: &Path,
        item_mod: &syn::ItemMod,
    ) -> Result<(), ResolveError> {
        let ident = item_mod.ident.to_string();
        let mut child_path = parent_id.path.clone();
        child_path.push(ident.clone());
        let child_id = ModuleId {
            crate_id: parent_id.crate_id.clone(),
            path: child_path,
        };
        let child_resolve_dir = parent_resolve_dir.join(&ident);

        if let Some((_, items)) = &item_mod.content {
            return self.load_module(
                child_id,
                parent_resolve_dir.join(format!("{ident}.inline.rs")),
                child_resolve_dir,
                items.clone(),
            );
        }

        let file_path = resolve_module_path(parent_resolve_dir, &ident).ok_or_else(|| {
            ResolveError::ModuleNotFound {
                module: display_module_id(&child_id),
                searched_from: parent_resolve_dir.to_path_buf(),
            }
        })?;
        let items = parse_file(&file_path)?;
        self.load_module(child_id, file_path, child_resolve_dir, items)
    }

    fn collect_imports(
        &mut self,
        module_id: &ModuleId,
        included_modules: &mut HashSet<ModuleId>,
        ordered_modules: &mut Vec<ModuleId>,
    ) -> Result<(), ResolveError> {
        self.ensure_module_loaded(module_id)?;

        let module = self
            .modules
            .get(module_id)
            .ok_or_else(|| ResolveError::UnknownModule(display_module_id(module_id)))?;
        let targets = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Use(item_use) => Some(collect_use_targets(
                    &item_use.tree,
                    &module.module_id.crate_id,
                )),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();

        for target in targets {
            self.include_module(&target, included_modules, ordered_modules)?;
        }

        Ok(())
    }

    fn include_module(
        &mut self,
        module_id: &ModuleId,
        included_modules: &mut HashSet<ModuleId>,
        ordered_modules: &mut Vec<ModuleId>,
    ) -> Result<(), ResolveError> {
        self.ensure_module_loaded(module_id)?;

        if !included_modules.insert(module_id.clone()) {
            return Ok(());
        }

        let actual_id = self
            .modules
            .get_key_value(module_id)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| ResolveError::UnknownModule(display_module_id(module_id)))?;

        self.collect_imports(&actual_id, included_modules, ordered_modules)?;
        ordered_modules.push(actual_id);
        Ok(())
    }

    fn ensure_module_loaded(&mut self, module_id: &ModuleId) -> Result<(), ResolveError> {
        if self.modules.contains_key(module_id) {
            return Ok(());
        }

        match &module_id.crate_id {
            CrateId::Current => Err(ResolveError::UnknownModule(display_module_id(module_id))),
            CrateId::External(crate_name) => {
                self.load_external_crate(crate_name)?;
                if self.modules.contains_key(module_id) {
                    Ok(())
                } else {
                    Err(ResolveError::UnknownModule(display_module_id(module_id)))
                }
            }
        }
    }

    fn load_external_crate(&mut self, crate_name: &str) -> Result<(), ResolveError> {
        let root_id = ModuleId::external(crate_name, Vec::new());
        if self.modules.contains_key(&root_id) {
            return Ok(());
        }

        let lib_path = self.resolve_external_crate_lib(crate_name)?;
        let resolve_dir = lib_path
            .parent()
            .ok_or_else(|| ResolveError::MissingParent(lib_path.clone()))?
            .to_path_buf();
        let items = parse_file(&lib_path)?;
        self.load_module(root_id, lib_path, resolve_dir, items)
    }

    fn resolve_external_crate_lib(&mut self, crate_name: &str) -> Result<PathBuf, ResolveError> {
        if let Some(path) = self.external_libs.get(crate_name) {
            return Ok(path.clone());
        }

        let package = self.lockfile()?.find_package(crate_name)?;
        let source =
            package
                .source
                .as_deref()
                .ok_or_else(|| ResolveError::UnsupportedPackageSource {
                    crate_name: crate_name.to_string(),
                    source: "workspace/path dependency without source".to_string(),
                })?;

        let lib_path = match parse_package_source(source)? {
            PackageSource::Git { rev } => {
                resolve_git_package_lib(&self.config.cargo_home, &package.name, &rev)?
            }
        };

        self.external_libs
            .insert(crate_name.to_string(), lib_path.clone());
        Ok(lib_path)
    }

    fn lockfile(&mut self) -> Result<&Lockfile, ResolveError> {
        if self.lockfile.is_none() {
            let path = self.config.lockfile_path.clone().ok_or_else(|| {
                ResolveError::LockfileNotFound {
                    searched_from: self.config.cargo_home.clone(),
                }
            })?;
            self.lockfile = Some(Lockfile::from_path(&path)?);
        }

        Ok(self.lockfile.as_ref().expect("lockfile must be loaded"))
    }
}

struct ModuleData {
    module_id: ModuleId,
    file_path: PathBuf,
    items: Vec<Item>,
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

fn collect_use_targets(tree: &UseTree, current_crate: &CrateId) -> Vec<ModuleId> {
    let mut targets = Vec::new();
    collect_use_targets_inner(tree, current_crate, None, Vec::new(), &mut targets);
    targets
}

fn collect_use_targets_inner(
    tree: &UseTree,
    current_crate: &CrateId,
    root: Option<CrateId>,
    prefix: ModulePath,
    out: &mut Vec<ModuleId>,
) {
    match tree {
        UseTree::Path(path) if root.is_none() && path.ident == "crate" => {
            collect_use_targets_inner(
                &path.tree,
                current_crate,
                Some(current_crate.clone()),
                prefix,
                out,
            );
        }
        UseTree::Path(path) if root.is_none() => {
            if path.ident == "self" || path.ident == "super" {
                return;
            }
            collect_use_targets_inner(
                &path.tree,
                current_crate,
                Some(CrateId::External(path.ident.to_string())),
                prefix,
                out,
            );
        }
        UseTree::Path(path) => {
            let mut next_prefix = prefix;
            next_prefix.push(path.ident.to_string());
            collect_use_targets_inner(&path.tree, current_crate, root, next_prefix, out);
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_use_targets_inner(tree, current_crate, root.clone(), prefix.clone(), out);
            }
        }
        UseTree::Glob(_) => {
            if let Some(crate_id) = root {
                out.push(ModuleId {
                    crate_id,
                    path: prefix,
                });
            }
        }
        UseTree::Name(name) => {
            push_named_target(root, prefix, name.ident.to_string(), out);
        }
        UseTree::Rename(rename) => {
            push_named_target(root, prefix, rename.ident.to_string(), out);
        }
    }
}

fn push_named_target(
    root: Option<CrateId>,
    prefix: ModulePath,
    name: String,
    out: &mut Vec<ModuleId>,
) {
    let Some(crate_id) = root else {
        return;
    };

    match crate_id {
        CrateId::Current => {
            if name == "self" {
                out.push(ModuleId::current(prefix));
            } else if prefix.is_empty() {
                out.push(ModuleId::current(vec![name]));
            } else {
                out.push(ModuleId::current(prefix));
            }
        }
        CrateId::External(crate_name) => {
            if name == "self" {
                out.push(ModuleId::external(crate_name, prefix));
            } else if !prefix.is_empty() {
                out.push(ModuleId::external(crate_name, prefix));
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

fn display_module_id(module_id: &ModuleId) -> String {
    match &module_id.crate_id {
        CrateId::Current => {
            if module_id.path.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", module_id.path.join("::"))
            }
        }
        CrateId::External(crate_name) => {
            if module_id.path.is_empty() {
                crate_name.clone()
            } else {
                format!("{crate_name}::{}", module_id.path.join("::"))
            }
        }
    }
}

fn detect_cargo_home() -> Result<PathBuf, ResolveError> {
    if let Some(path) = env::var_os("CARGO_HOME") {
        return Ok(PathBuf::from(path));
    }

    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".cargo"));
    }

    Err(ResolveError::MissingCargoHome)
}

fn find_ancestor_file(path: &Path, file_name: &str) -> Option<PathBuf> {
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    loop {
        let candidate = current.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[derive(Deserialize)]
struct ParsedLockfile {
    package: Vec<LockPackageEntry>,
}

#[derive(Clone, Deserialize)]
struct LockPackageEntry {
    name: String,
    source: Option<String>,
}

struct Lockfile {
    packages: Vec<LockPackageEntry>,
}

impl Lockfile {
    fn from_path(path: &Path) -> Result<Self, ResolveError> {
        let source = fs::read_to_string(path)
            .map_err(|e| ResolveError::FileRead(path.to_path_buf(), e.to_string()))?;
        let parsed: ParsedLockfile = toml::from_str(&source)
            .map_err(|e| ResolveError::Parse(path.to_path_buf(), e.to_string()))?;
        Ok(Self {
            packages: parsed.package,
        })
    }

    fn find_package(&self, crate_name: &str) -> Result<LockPackageEntry, ResolveError> {
        let matches = self
            .packages
            .iter()
            .filter(|package| {
                normalize_package_name(&package.name) == crate_name || package.name == crate_name
            })
            .cloned()
            .collect::<Vec<_>>();

        match matches.len() {
            0 => Err(ResolveError::ExternalCrateNotFound(crate_name.to_string())),
            1 => Ok(matches.into_iter().next().expect("single match must exist")),
            _ => Err(ResolveError::AmbiguousExternalCrate(crate_name.to_string())),
        }
    }
}

fn normalize_package_name(name: &str) -> String {
    name.replace('-', "_")
}

enum PackageSource {
    Git { rev: String },
}

fn parse_package_source(source: &str) -> Result<PackageSource, ResolveError> {
    if let Some(rest) = source.strip_prefix("git+") {
        let Some((_, rev)) = rest.rsplit_once('#') else {
            return Err(ResolveError::UnsupportedPackageSource {
                crate_name: "<unknown>".to_string(),
                source: source.to_string(),
            });
        };
        return Ok(PackageSource::Git {
            rev: rev.to_string(),
        });
    }

    Err(ResolveError::UnsupportedPackageSource {
        crate_name: "<unknown>".to_string(),
        source: source.to_string(),
    })
}

fn resolve_git_package_lib(
    cargo_home: &Path,
    package_name: &str,
    rev: &str,
) -> Result<PathBuf, ResolveError> {
    let checkouts_dir = cargo_home.join("git").join("checkouts");
    let checkout_roots =
        fs::read_dir(&checkouts_dir).map_err(|e| ResolveError::GitCheckoutNotFound {
            package_name: package_name.to_string(),
            rev: rev.to_string(),
            searched_in: checkouts_dir.clone(),
            reason: e.to_string(),
        })?;

    let mut candidates = Vec::new();
    for checkout_root in checkout_roots {
        let checkout_root = checkout_root
            .map_err(|e| ResolveError::FileRead(checkouts_dir.clone(), e.to_string()))?
            .path();
        if !checkout_root.is_dir() {
            continue;
        }

        for repo_checkout in fs::read_dir(&checkout_root)
            .map_err(|e| ResolveError::FileRead(checkout_root.clone(), e.to_string()))?
        {
            let repo_checkout = repo_checkout
                .map_err(|e| ResolveError::FileRead(checkout_root.clone(), e.to_string()))?
                .path();
            if !repo_checkout.is_dir() {
                continue;
            }

            let Some(short_rev) = repo_checkout.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !rev.starts_with(short_rev) {
                continue;
            }

            if let Some(lib_path) = find_package_lib_in_repo(&repo_checkout, package_name)? {
                candidates.push(lib_path);
            }
        }
    }

    match candidates.len() {
        0 => Err(ResolveError::GitCheckoutNotFound {
            package_name: package_name.to_string(),
            rev: rev.to_string(),
            searched_in: checkouts_dir,
            reason: "no matching package manifest found in git checkouts".to_string(),
        }),
        1 => Ok(candidates
            .into_iter()
            .next()
            .expect("single candidate must exist")),
        _ => Err(ResolveError::AmbiguousGitCheckout {
            package_name: package_name.to_string(),
            rev: rev.to_string(),
        }),
    }
}

fn find_package_lib_in_repo(
    repo_root: &Path,
    package_name: &str,
) -> Result<Option<PathBuf>, ResolveError> {
    let manifests = find_manifest_paths(repo_root)?;

    for manifest_path in manifests {
        let manifest = parse_manifest(&manifest_path)?;
        let Some(package) = manifest.package else {
            continue;
        };
        if package.name != package_name {
            continue;
        }

        let lib_path = manifest
            .lib
            .and_then(|lib| lib.path)
            .map(|path| {
                manifest_path
                    .parent()
                    .expect("manifest must have parent")
                    .join(path)
            })
            .unwrap_or_else(|| {
                manifest_path
                    .parent()
                    .expect("manifest must have parent")
                    .join("src/lib.rs")
            });

        if lib_path.is_file() {
            return Ok(Some(lib_path));
        }

        return Err(ResolveError::LibraryTargetNotFound {
            package_name: package_name.to_string(),
            manifest_path,
            lib_path,
        });
    }

    Ok(None)
}

fn find_manifest_paths(root: &Path) -> Result<Vec<PathBuf>, ResolveError> {
    let mut stack = vec![root.to_path_buf()];
    let mut manifests = Vec::new();

    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).map_err(|e| ResolveError::FileRead(dir.clone(), e.to_string()))?
        {
            let entry = entry.map_err(|e| ResolveError::FileRead(dir.clone(), e.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                if name == ".git" || name == "target" {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if entry.file_name() == "Cargo.toml" {
                manifests.push(path);
            }
        }
    }

    Ok(manifests)
}

#[derive(Deserialize)]
struct ParsedManifest {
    package: Option<ManifestPackage>,
    lib: Option<ManifestLib>,
}

#[derive(Deserialize)]
struct ManifestPackage {
    name: String,
}

#[derive(Deserialize)]
struct ManifestLib {
    path: Option<String>,
}

fn parse_manifest(path: &Path) -> Result<ParsedManifest, ResolveError> {
    let source = fs::read_to_string(path)
        .map_err(|e| ResolveError::FileRead(path.to_path_buf(), e.to_string()))?;
    toml::from_str(&source).map_err(|e| ResolveError::Parse(path.to_path_buf(), e.to_string()))
}

#[derive(Debug)]
pub enum ResolveError {
    AmbiguousExternalCrate(String),
    AmbiguousGitCheckout {
        package_name: String,
        rev: String,
    },
    DuplicateItem {
        name: String,
        first: PathBuf,
        second: PathBuf,
    },
    ExternalCrateNotFound(String),
    FileRead(PathBuf, String),
    GitCheckoutNotFound {
        package_name: String,
        rev: String,
        searched_in: PathBuf,
        reason: String,
    },
    LibraryTargetNotFound {
        package_name: String,
        manifest_path: PathBuf,
        lib_path: PathBuf,
    },
    LockfileNotFound {
        searched_from: PathBuf,
    },
    MissingCargoHome,
    MissingParent(PathBuf),
    ModuleNotFound {
        module: String,
        searched_from: PathBuf,
    },
    Parse(PathBuf, String),
    UnknownModule(String),
    UnsupportedPackageSource {
        crate_name: String,
        source: String,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmbiguousExternalCrate(crate_name) => {
                write!(f, "Multiple Cargo.lock packages matched external crate `{crate_name}`")
            }
            Self::AmbiguousGitCheckout { package_name, rev } => write!(
                f,
                "Multiple git checkouts matched package `{package_name}` at revision `{rev}`"
            ),
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
            Self::ExternalCrateNotFound(crate_name) => {
                write!(f, "External crate `{crate_name}` was not found in Cargo.lock")
            }
            Self::FileRead(path, err) => write!(f, "File read error ({}): {err}", path.display()),
            Self::GitCheckoutNotFound {
                package_name,
                rev,
                searched_in,
                reason,
            } => write!(
                f,
                "Git checkout not found for package `{package_name}` at revision `{rev}` under {}: {reason}",
                searched_in.display()
            ),
            Self::LibraryTargetNotFound {
                package_name,
                manifest_path,
                lib_path,
            } => write!(
                f,
                "Library target for package `{package_name}` was not found: manifest {} points to {}",
                manifest_path.display(),
                lib_path.display()
            ),
            Self::LockfileNotFound { searched_from } => write!(
                f,
                "Cargo.lock not found while resolving external crates from {}",
                searched_from.display()
            ),
            Self::MissingCargoHome => write!(f, "CARGO_HOME and HOME are both unavailable"),
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
            Self::UnsupportedPackageSource { crate_name, source } => write!(
                f,
                "Unsupported Cargo.lock source for crate `{crate_name}`: {source}"
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

#[cfg(test)]
mod tests {
    use super::{read_sources, resolve_entry, ResolveConfig};
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

    #[test]
    fn resolves_external_git_crate_from_cargo_lock() {
        let dir = temp_dir_path("git-lock");
        let cargo_home = dir.join("cargo-home");
        let project_root = dir.join("project");
        let entry = project_root.join("shader.rs");
        let lockfile = project_root.join("Cargo.lock");
        let checkout = cargo_home
            .join("git")
            .join("checkouts")
            .join("rs2glsl-deadbeef")
            .join("ed449fe");
        let external_manifest = checkout.join("Cargo.toml");
        let external_lib = checkout.join("src").join("lib.rs");
        let external_math = checkout.join("src").join("math.rs");

        write_file(
            &entry,
            "\
use shader_prelude::*;

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(double(time), 0.0, 0.0, 1.0)
}
",
        );
        write_file(
            &lockfile,
            "\
version = 4

[[package]]
name = \"shader-prelude\"
version = \"0.1.0\"
source = \"git+https://github.com/taiseiue/rs2glsl#ed449fe7aac09a8df92cf4950d6f4231047269fa\"
",
        );
        write_file(
            &external_manifest,
            "\
[package]
name = \"shader-prelude\"
version = \"0.1.0\"
edition = \"2024\"
",
        );
        write_file(
            &external_lib,
            "\
mod math;
use crate::math::*;

fn double(x: f32) -> f32 {
    square(x) + square(x)
}
",
        );
        write_file(
            &external_math,
            "\
fn square(x: f32) -> f32 {
    x * x
}
",
        );

        let config = ResolveConfig {
            cargo_home,
            lockfile_path: Some(lockfile),
        };
        let source = resolve_entry(&entry, &config).expect("resolve failed");

        assert!(source.contains("fn double"));
        assert!(source.contains("fn square"));
        assert!(!source.contains("use shader_prelude::*;"));

        let glsl = transpile_to_glsl(&format!("{TEST_PRELUDE}{source}")).expect("transpile failed");
        assert!(glsl.contains("float double(float x);"));
        assert!(glsl.contains("float square(float x);"));
        assert!(glsl.contains("return vec4(double(time), 0.0, 0.0, 1.0);"));

        fs::remove_dir_all(dir).expect("failed to clean temp dir");
    }
}
