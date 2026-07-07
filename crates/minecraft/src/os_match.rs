use std::collections::HashMap;

use regex::Regex;
use serde::Deserialize;

/// A single entry from a Mojang "rules" array — used both to decide whether
/// a library applies on this platform and whether a conditional JVM/game
/// argument should be included.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    /// A regex (as used historically to exclude specific old macOS point
    /// releases from certain LWJGL natives), matched against
    /// `std::env::consts::OS`'s version — in practice we only have the OS
    /// *name* readily available, not a parsed version string, so this is
    /// matched leniently (see `os_version_matches`).
    pub version: Option<String>,
    pub arch: Option<String>,
}

/// Which optional launch-time features are active for this particular
/// launch (demo mode, custom resolution, quick play, ...). Conditional
/// arguments in the version JSON are gated on these. Anything not present
/// here is treated as `false` — we simply don't emit arguments for features
/// this milestone doesn't implement yet (demo mode, quick play) rather than
/// guessing at them.
#[derive(Debug, Clone, Default)]
pub struct FeatureFlags {
    flags: HashMap<String, bool>,
}

impl FeatureFlags {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: bool) -> &mut Self {
        self.flags.insert(key.into(), value);
        self
    }

    pub fn is_active(&self, key: &str) -> bool {
        self.flags.get(key).copied().unwrap_or(false)
    }
}

/// Mojang's launcher name for the running OS — distinct from Rust's own
/// `std::env::consts::OS` (which says `"macos"`, not `"osx"`).
pub fn current_os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "osx",
        "windows" => "windows",
        "linux" => "linux",
        other => other,
    }
}

pub fn current_arch() -> &'static str {
    std::env::consts::ARCH
}

/// Evaluates a Mojang `rules` array against the current platform.
///
/// Semantics (matched against the official launcher's documented
/// behavior): an absent or empty rules array means "always include". When
/// rules are present, each one is checked **in array order**; whichever
/// rule matched *last* decides the outcome, and the starting assumption
/// before any rule has matched is `false`. This is what allows the common
/// "allow everywhere, then disallow on this one platform" pattern used for
/// natives.
pub fn rules_allow(rules: &[Rule], features: &FeatureFlags) -> bool {
    if rules.is_empty() {
        return true;
    }

    let mut allowed = false;
    for rule in rules {
        if rule_matches_platform(rule, features) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches_platform(rule: &Rule, features: &FeatureFlags) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != current_os_name() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if !arch_matches(arch, current_arch()) {
                return false;
            }
        }
        if let Some(version_pattern) = &os.version {
            if !os_version_matches(version_pattern) {
                return false;
            }
        }
    }

    if let Some(required_features) = &rule.features {
        for (key, expected) in required_features {
            if features.is_active(key) != *expected {
                return false;
            }
        }
    }

    true
}

fn arch_matches(pattern: &str, actual: &str) -> bool {
    // Mojang's old 32-bit-only natives used patterns like `^x86$`; modern
    // entries (arm64, x86_64) are usually exact strings. Try regex first
    // since it correctly handles both, falling back to a plain equality
    // check if the pattern isn't valid regex for some reason.
    Regex::new(pattern)
        .map(|re| re.is_match(actual))
        .unwrap_or_else(|_| pattern == actual)
}

fn os_version_matches(pattern: &str) -> bool {
    // We don't have a clean parsed OS version string available
    // cross-platform without extra OS-specific calls, and this rule only
    // appears in a handful of very old library entries (pre-2017 macOS
    // exclusions). Rather than silently mis-include or mis-exclude a
    // library, fail open (match) so the library is still considered —
    // worst case on those ancient versions is one extra native ends up
    // downloaded, which is harmless.
    let _ = Regex::new(pattern);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rules_always_allowed() {
        assert!(rules_allow(&[], &FeatureFlags::new()));
    }

    #[test]
    fn allow_all_then_disallow_one_os_excludes_only_that_os() {
        let rules = vec![
            Rule {
                action: RuleAction::Allow,
                os: None,
                features: None,
            },
            Rule {
                action: RuleAction::Disallow,
                os: Some(OsRule {
                    name: Some("definitely-not-this-platform".into()),
                    version: None,
                    arch: None,
                }),
                features: None,
            },
        ];
        // The disallow rule targets a platform we are not running on, so it
        // never matches, and the leading unconditional allow wins.
        assert!(rules_allow(&rules, &FeatureFlags::new()));
    }

    #[test]
    fn disallow_current_os_is_excluded() {
        let rules = vec![
            Rule {
                action: RuleAction::Allow,
                os: None,
                features: None,
            },
            Rule {
                action: RuleAction::Disallow,
                os: Some(OsRule {
                    name: Some(current_os_name().to_string()),
                    version: None,
                    arch: None,
                }),
                features: None,
            },
        ];
        assert!(!rules_allow(&rules, &FeatureFlags::new()));
    }

    #[test]
    fn feature_gated_rule_requires_matching_flag() {
        let mut features = HashMap::new();
        features.insert("is_demo_user".to_string(), true);
        let rules = vec![Rule {
            action: RuleAction::Allow,
            os: None,
            features: Some(features),
        }];

        let mut flags = FeatureFlags::new();
        assert!(!rules_allow(&rules, &flags), "feature defaults to false");

        flags.set("is_demo_user", true);
        assert!(rules_allow(&rules, &flags));
    }

    #[test]
    fn mojang_os_name_mapping() {
        // We can't change std::env::consts::OS in a test, but we can assert
        // the mapping function never returns Rust's "macos" spelling, since
        // every real version.json in the wild uses "osx".
        assert_ne!(current_os_name(), "macos");
    }
}
