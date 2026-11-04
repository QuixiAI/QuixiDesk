//! Declarative bundle/profile manifests and persisted patch reconciliation
//! (`Section 5.2` of *A Programming Paradigm for Spatiotemporal Composability*).

use crate::coeffect::RealmId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// One addressable plugin row inside a bundle (`cordis.patch` equivalent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRowSpec {
    pub id: String,
    pub plugin: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub isolate: BTreeMap<String, RealmId>,
    #[serde(default = "empty_object")]
    pub config: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<Value>,
}

fn empty_object() -> Value {
    json!({})
}

/// Bundle manifest grouping related host + client plugin rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default = "default_true")]
    pub listed_by_default: bool,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme_markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_script: Option<String>,
    #[serde(default)]
    pub rows: Vec<PluginRowSpec>,
    /// Optional declarative contributions for user/Creator-authored bundles on disk:
    /// prompt sections, slash commands, scripted tools, or UI slot cards.
    #[serde(default = "empty_object")]
    pub declarative: Value,
}

fn default_true() -> bool {
    true
}

/// Persisted profile selection (`$DATA_DIR/profiles/default/profile.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSelectionFile {
    /// Explicit bundle enable/disable overrides (`bundle_id -> enabled`).
    /// Omitted bundles use `bundle.default_enabled`.
    #[serde(default)]
    pub bundles: BTreeMap<String, bool>,
    /// Default agent preset for new sessions (`standard`, `ptc`, `minimal`, `creator`).
    #[serde(default = "default_preset_id")]
    pub default_preset: String,
}

fn default_preset_id() -> String {
    "standard".to_string()
}

/// Persisted row-level patch override (`$DATA_DIR/profiles/default/cordis.patch.json`),
/// recording only departures from bundle defaults just like `dsh-plugin-manager`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRowPatch {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatchFile {
    #[serde(default)]
    pub rows: Vec<ProfileRowPatch>,
}

/// Resolved effective row fed into `CordisRuntime::reconcile`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLoaderRow {
    pub bundle_id: String,
    pub id: String,
    pub plugin: String,
    pub title: String,
    pub description: String,
    pub disabled: bool,
    pub locked: bool,
    pub isolate: BTreeMap<String, RealmId>,
    pub config: Value,
    pub config_schema: Option<Value>,
}

impl ResolvedLoaderRow {
    /// Digest of `(plugin, isolate, config)` so changing configuration triggers clean Fiber reload.
    pub fn config_digest(&self) -> String {
        serde_json::to_string(&json!({
            "plugin": self.plugin,
            "isolate": self.isolate,
            "config": self.config,
        }))
        .unwrap_or_default()
    }
}

/// Manages built-in bundles, user/Creator bundles in `$DATA_DIR/plugins`, and profile patches.
pub struct ProfileStore {
    pub profile_dir: PathBuf,
    pub user_plugins_dir: PathBuf,
    pub builtin_bundles: Vec<BundleManifest>,
}

impl ProfileStore {
    pub fn new(data_dir: &Path, builtin_bundles: Vec<BundleManifest>) -> Self {
        let profile_dir = data_dir.join("profiles").join("default");
        let user_plugins_dir = data_dir.join("plugins");
        let _ = fs::create_dir_all(&profile_dir);
        let _ = fs::create_dir_all(&user_plugins_dir);
        Self {
            profile_dir,
            user_plugins_dir,
            builtin_bundles,
        }
    }

    fn selection_path(&self) -> PathBuf {
        self.profile_dir.join("profile.json")
    }

    fn patch_path(&self) -> PathBuf {
        self.profile_dir.join("cordis.patch.json")
    }

    pub fn load_selection(&self) -> ProfileSelectionFile {
        fs::read_to_string(self.selection_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| ProfileSelectionFile {
                bundles: BTreeMap::new(),
                default_preset: default_preset_id(),
            })
    }

    pub fn save_selection(&self, sel: &ProfileSelectionFile) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(sel).map_err(|e| e.to_string())?;
        fs::write(self.selection_path(), raw).map_err(|e| e.to_string())
    }

    pub fn load_patch(&self) -> ProfilePatchFile {
        fs::read_to_string(self.patch_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_patch(&self, patch: &ProfilePatchFile) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(patch).map_err(|e| e.to_string())?;
        fs::write(self.patch_path(), raw).map_err(|e| e.to_string())
    }

    /// Reads user/Creator-installed bundles from `$DATA_DIR/plugins/<dir>/manifest.json`.
    pub fn load_user_bundles(&self) -> Vec<BundleManifest> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.user_plugins_dir) else {
            return out;
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            let manifest_path = dir.join("manifest.json");
            let Ok(raw) = fs::read_to_string(&manifest_path) else {
                continue;
            };
            let Ok(mut bundle) = serde_json::from_str::<BundleManifest>(&raw) else {
                continue;
            };
            bundle.official = false;
            bundle
                .source
                .get_or_insert_with(|| format!("local:{}", dir.display()));
            if bundle.readme_markdown.is_none() {
                if let Ok(md) = fs::read_to_string(dir.join("README.md")) {
                    bundle.readme_markdown = Some(md);
                }
            }
            if bundle.client_script.is_none() {
                if let Ok(js) = fs::read_to_string(dir.join("client.js")) {
                    bundle.client_script = Some(js);
                }
            }
            out.push(bundle);
        }
        out
    }

    pub fn all_bundles(&self) -> Vec<BundleManifest> {
        let mut all = self.builtin_bundles.clone();
        let known: std::collections::HashSet<String> = all.iter().map(|b| b.id.clone()).collect();
        for user in self.load_user_bundles() {
            if !known.contains(&user.id) {
                all.push(user);
            }
        }
        all
    }

    /// Resolves effective enabled bundles and merged row definitions.
    pub fn resolve_rows(&self) -> Vec<ResolvedLoaderRow> {
        let selection = self.load_selection();
        let patch = self.load_patch();
        let patch_by_id: HashMap<&str, &ProfileRowPatch> =
            patch.rows.iter().map(|r| (r.id.as_str(), r)).collect();

        let mut resolved = Vec::new();
        for bundle in self.all_bundles() {
            let bundle_enabled = if bundle.locked {
                true
            } else {
                selection
                    .bundles
                    .get(&bundle.id)
                    .copied()
                    .unwrap_or(bundle.default_enabled)
            };
            for row in bundle.rows {
                let row_override = patch_by_id.get(row.id.as_str()).copied();
                let disabled = if !bundle_enabled {
                    true
                } else if row.locked {
                    false
                } else {
                    row_override
                        .and_then(|o| o.disabled)
                        .unwrap_or(row.disabled)
                };
                let config = row_override
                    .and_then(|o| o.config.clone())
                    .unwrap_or(row.config);
                resolved.push(ResolvedLoaderRow {
                    bundle_id: bundle.id.clone(),
                    id: row.id,
                    plugin: row.plugin,
                    title: row.title,
                    description: row.description,
                    disabled,
                    locked: bundle.locked || row.locked,
                    isolate: row.isolate,
                    config,
                    config_schema: row.config_schema,
                });
            }
        }
        resolved
    }

    pub fn set_bundle_enabled(&self, bundle_id: &str, enabled: bool) -> Result<(), String> {
        let all = self.all_bundles();
        let Some(bundle) = all.iter().find(|b| b.id == bundle_id) else {
            return Err(format!("Unknown bundle \"{bundle_id}\""));
        };
        if bundle.locked && !enabled {
            return Err(format!("Core bundle \"{bundle_id}\" is required and cannot be disabled"));
        }
        let mut selection = self.load_selection();
        if enabled == bundle.default_enabled {
            selection.bundles.remove(bundle_id);
        } else {
            selection.bundles.insert(bundle_id.to_owned(), enabled);
        }
        self.save_selection(&selection)
    }

    /// Updates only departures from bundle defaults (`dsh-plugin-manager` patch minimization rule).
    pub fn set_row_enabled(&self, row_id: &str, enabled: bool) -> Result<(), String> {
        let all = self.all_bundles();
        let Some((bundle, row)) = all
            .iter()
            .flat_map(|b| b.rows.iter().map(move |r| (b, r)))
            .find(|(_, r)| r.id == row_id)
        else {
            return Err(format!("Unknown plugin row \"{row_id}\""));
        };
        if (bundle.locked || row.locked) && !enabled {
            return Err(format!("Plugin \"{row_id}\" is locked by a required core bundle"));
        }
        let desired_disabled = !enabled;
        let mut patch = self.load_patch();
        if let Some(existing) = patch.rows.iter_mut().find(|r| r.id == row_id) {
            if desired_disabled == row.disabled {
                existing.disabled = None;
            } else {
                existing.disabled = Some(desired_disabled);
            }
        } else if desired_disabled != row.disabled {
            patch.rows.push(ProfileRowPatch {
                id: row_id.to_owned(),
                disabled: Some(desired_disabled),
                config: None,
            });
        }
        patch
            .rows
            .retain(|r| r.disabled.is_some() || r.config.is_some());
        self.save_patch(&patch)
    }

    pub fn set_row_config(&self, row_id: &str, config: Value) -> Result<(), String> {
        if !config.is_object() {
            return Err("Plugin configuration must be a JSON object".into());
        }
        let all = self.all_bundles();
        let Some(row) = all.iter().flat_map(|b| b.rows.iter()).find(|r| r.id == row_id) else {
            return Err(format!("Unknown plugin row \"{row_id}\""));
        };
        let mut patch = self.load_patch();
        if let Some(existing) = patch.rows.iter_mut().find(|r| r.id == row_id) {
            if config == row.config {
                existing.config = None;
            } else {
                existing.config = Some(config);
            }
        } else if config != row.config {
            patch.rows.push(ProfileRowPatch {
                id: row_id.to_owned(),
                disabled: None,
                config: Some(config),
            });
        }
        patch
            .rows
            .retain(|r| r.disabled.is_some() || r.config.is_some());
        self.save_patch(&patch)
    }

    pub fn install_user_bundle(
        &self,
        mut manifest: BundleManifest,
        client_js: Option<&str>,
        readme_md: Option<&str>,
    ) -> Result<PathBuf, String> {
        let id = manifest.id.trim().to_owned();
        if id.is_empty()
            || id.len() > 80
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(
                "Bundle id must be 1..80 ASCII letters, digits, hyphens, underscores, or dots"
                    .into(),
            );
        }
        if self.builtin_bundles.iter().any(|b| b.id == id) {
            return Err(format!("Bundle id \"{id}\" collides with a built-in bundle"));
        }
        manifest.official = false;
        let dir = self.user_plugins_dir.join(&id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if let Some(js) = client_js {
            fs::write(dir.join("client.js"), js).map_err(|e| e.to_string())?;
            manifest.client_script = None;
        }
        if let Some(md) = readme_md {
            fs::write(dir.join("README.md"), md).map_err(|e| e.to_string())?;
            manifest.readme_markdown = None;
        }
        let raw = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        fs::write(dir.join("manifest.json"), raw).map_err(|e| e.to_string())?;
        let mut selection = self.load_selection();
        selection.bundles.insert(id, true);
        self.save_selection(&selection)?;
        Ok(dir)
    }

    pub fn remove_user_bundle(&self, bundle_id: &str) -> Result<(), String> {
        if self.builtin_bundles.iter().any(|b| b.id == bundle_id) {
            return Err(format!("Built-in bundle \"{bundle_id}\" cannot be uninstalled (disable it instead)"));
        }
        let dir = self.user_plugins_dir.join(bundle_id);
        if !dir.exists() {
            return Err(format!("User bundle \"{bundle_id}\" is not installed"));
        }
        let mut selection = self.load_selection();
        selection.bundles.remove(bundle_id);
        self.save_selection(&selection)?;
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())
    }
}
