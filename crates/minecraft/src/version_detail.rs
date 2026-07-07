use std::collections::HashMap;

use serde::Deserialize;

use crate::os_match::{rules_allow, FeatureFlags, Rule};

/// Mirrors the structure of `<version>.json` as published by Mojang. Field
/// names follow the JSON exactly (via `#[serde(rename)]` where Rust
/// keywords/casing differ) — see `docs/ARCHITECTURE.md` for links to the
/// reference material this was built against.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionDetail {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub assets: String,
    pub downloads: Downloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersionInfo>,
    /// Present on 1.13+. When absent, fall back to `legacy_minecraft_arguments`.
    pub arguments: Option<Arguments>,
    /// Present on versions older than 1.13: one space-separated string
    /// instead of the structured `arguments` object.
    #[serde(rename = "minecraftArguments")]
    pub legacy_minecraft_arguments: Option<String>,
    pub logging: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: DownloadArtifact,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadArtifact {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersionInfo {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Maps the OS name to a classifier key in `downloads.classifiers`
    /// (e.g. `{"linux": "natives-linux"}`) for libraries that ship native
    /// code, almost always LWJGL components.
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    /// Fabric's loader meta and a few other third-party manifests give a
    /// Maven repo base URL plus a bare coordinate instead of a resolved
    /// `downloads.artifact` — used by `crate::libraries` to build the
    /// download URL itself when this is present and `downloads` is not.
    #[serde(default)]
    pub url: Option<String>,
    /// Which paths inside a natives jar to skip when extracting (almost
    /// always just `["META-INF/"]`). Absent entirely on most libraries,
    /// meaning "extract everything".
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, LibraryArtifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryArtifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub client: LoggingClientConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClientConfig {
    /// A JVM argument template, e.g. `-Dlog4j.configurationFile=${path}`.
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgumentEntry>,
    #[serde(default)]
    pub jvm: Vec<ArgumentEntry>,
}

/// One element of a `game`/`jvm` argument array — either a bare string or a
/// rule-gated entry that may expand to zero, one, or several arguments.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgumentEntry {
    Plain(String),
    Conditional(ConditionalArgument),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConditionalArgument {
    #[serde(default)]
    pub rules: Vec<Rule>,
    pub value: StringOrList,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    Single(String),
    Multiple(Vec<String>),
}

/// Expands a `game`/`jvm` argument array into the final list of process
/// arguments: conditional entries are evaluated against `features` and
/// dropped if their rules don't match, and every `${placeholder}` token is
/// substituted using `placeholders`.
pub fn resolve_arguments(
    entries: &[ArgumentEntry],
    features: &FeatureFlags,
    placeholders: &HashMap<&str, String>,
) -> Vec<String> {
    let mut resolved = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry {
            ArgumentEntry::Plain(template) => resolved.push(substitute(template, placeholders)),
            ArgumentEntry::Conditional(cond) => {
                if !rules_allow(&cond.rules, features) {
                    continue;
                }
                match &cond.value {
                    StringOrList::Single(template) => resolved.push(substitute(template, placeholders)),
                    StringOrList::Multiple(templates) => {
                        resolved.extend(templates.iter().map(|t| substitute(t, placeholders)));
                    }
                }
            }
        }
    }
    resolved
}

/// Pre-1.13 versions ship one flat, space-separated argument string instead
/// of the structured `arguments.game` array. No conditionals exist in this
/// format — just placeholder substitution per whitespace-separated token.
pub fn resolve_legacy_game_arguments(raw: &str, placeholders: &HashMap<&str, String>) -> Vec<String> {
    raw.split_whitespace()
        .map(|token| substitute(token, placeholders))
        .collect()
}

fn substitute(template: &str, placeholders: &HashMap<&str, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in placeholders {
        let needle = format!("${{{key}}}");
        if result.contains(&needle) {
            result = result.replace(&needle, value);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os_match::RuleAction;

    fn placeholders() -> HashMap<&'static str, String> {
        let mut map = HashMap::new();
        map.insert("auth_player_name", "Steve".to_string());
        map.insert("version_name", "1.21.11".to_string());
        map
    }

    #[test]
    fn plain_entries_get_substituted() {
        let entries = vec![
            ArgumentEntry::Plain("--username".into()),
            ArgumentEntry::Plain("${auth_player_name}".into()),
        ];
        let resolved = resolve_arguments(&entries, &FeatureFlags::new(), &placeholders());
        assert_eq!(resolved, vec!["--username".to_string(), "Steve".to_string()]);
    }

    #[test]
    fn conditional_entry_dropped_when_feature_inactive() {
        let mut required = HashMap::new();
        required.insert("is_demo_user".to_string(), true);
        let entries = vec![ArgumentEntry::Conditional(ConditionalArgument {
            rules: vec![Rule {
                action: RuleAction::Allow,
                os: None,
                features: Some(required),
            }],
            value: StringOrList::Single("--demo".into()),
        })];

        let resolved = resolve_arguments(&entries, &FeatureFlags::new(), &placeholders());
        assert!(resolved.is_empty());
    }

    #[test]
    fn conditional_entry_with_multiple_values_expands_all() {
        let entries = vec![ArgumentEntry::Conditional(ConditionalArgument {
            rules: vec![],
            value: StringOrList::Multiple(vec!["--width".into(), "${resolution_width}".into()]),
        })];
        let mut placeholders = placeholders();
        placeholders.insert("resolution_width", "1280".to_string());

        let resolved = resolve_arguments(&entries, &FeatureFlags::new(), &placeholders);
        assert_eq!(resolved, vec!["--width".to_string(), "1280".to_string()]);
    }

    #[test]
    fn legacy_arguments_split_and_substitute() {
        let resolved = resolve_legacy_game_arguments(
            "--username ${auth_player_name} --version ${version_name}",
            &placeholders(),
        );
        assert_eq!(
            resolved,
            vec!["--username", "Steve", "--version", "1.21.11"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn parses_a_realistic_modern_version_json_fragment() {
        let json = r#"
        {
            "id": "1.21.11",
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "assets": "21",
            "assetIndex": {"id": "21", "sha1": "abc", "size": 1, "totalSize": 1, "url": "https://example.test/assetindex.json"},
            "downloads": {"client": {"sha1": "def", "size": 2, "url": "https://example.test/client.jar"}},
            "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 21},
            "libraries": [
                {
                    "name": "org.lwjgl:lwjgl:3.3.3",
                    "downloads": {
                        "artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar", "sha1": "111", "size": 3, "url": "https://example.test/lwjgl.jar"},
                        "classifiers": {
                            "natives-linux": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar", "sha1": "222", "size": 4, "url": "https://example.test/lwjgl-natives.jar"}
                        }
                    },
                    "natives": {"linux": "natives-linux"}
                }
            ],
            "arguments": {
                "game": ["--username", "${auth_player_name}"],
                "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
            }
        }
        "#;

        let parsed: VersionDetail = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed.id, "1.21.11");
        assert_eq!(parsed.libraries.len(), 1);
        assert_eq!(
            parsed.java_version.unwrap().component,
            "java-runtime-gamma"
        );
        assert!(parsed.legacy_minecraft_arguments.is_none());
    }
}
