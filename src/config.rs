use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// 应用配置
#[derive(Debug, Deserialize, Default, Clone)]
pub struct AppConfig {
    /// 扫描相关配置
    #[serde(default)]
    pub scan: ScanConfig,
    /// UI 相关配置
    #[serde(default)]
    pub ui: UiConfig,
    /// 安全相关配置
    #[serde(default)]
    pub safety: SafetyConfig,
}

/// 扫描配置
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ScanConfig {
    /// 额外扫描目标路径（支持 ~ 表示主目录）
    #[serde(default)]
    pub extra_targets: Vec<String>,
}

/// UI 配置
#[derive(Debug, Deserialize, Default, Clone)]
pub struct UiConfig {
    /// 默认排序方式: "name" / "size" / "time"
    #[serde(default)]
    pub default_sort: Option<String>,
}

/// 安全相关配置
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SafetyConfig {
    /// 是否移至系统回收站而非永久删除（默认 false）
    #[serde(default)]
    pub move_to_trash: bool,
}

impl AppConfig {
    /// 从 ~/.config/vac/config.toml 加载配置，失败时返回默认配置
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if !config_path.exists() {
            return Self::default();
        }
        match fs::read_to_string(&config_path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// 配置文件路径
    fn config_path() -> PathBuf {
        directories::UserDirs::new()
            .map(|dirs| {
                dirs.home_dir()
                    .join(".config")
                    .join("vac")
                    .join("config.toml")
            })
            .unwrap_or_else(|| PathBuf::from(".config/vac/config.toml"))
    }

    /// 获取展开后的额外扫描目标路径（~ 展开为主目录，过滤不存在的路径）
    pub fn expanded_extra_targets(&self) -> Vec<PathBuf> {
        let home_dir = directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf());

        self.scan
            .extra_targets
            .iter()
            .filter_map(|raw_path| {
                let expanded = if raw_path.starts_with('~') {
                    if let Some(ref home) = home_dir {
                        let home_str = home.display().to_string();
                        PathBuf::from(raw_path.replacen('~', &home_str, 1))
                    } else {
                        PathBuf::from(raw_path)
                    }
                } else {
                    PathBuf::from(raw_path)
                };
                if expanded.exists() {
                    Some(expanded)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_empty_values() {
        let config = AppConfig::default();
        assert!(config.scan.extra_targets.is_empty());
        assert!(config.ui.default_sort.is_none());
    }

    #[test]
    fn parse_valid_toml() {
        let toml_str = r#"
[scan]
extra_targets = ["~/Projects/node_modules", "/tmp/test"]

[ui]
default_sort = "size"
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse toml");
        assert_eq!(config.scan.extra_targets.len(), 2);
        assert_eq!(config.ui.default_sort.as_deref(), Some("size"));
    }

    #[test]
    fn parse_partial_toml_uses_defaults() {
        let toml_str = r#"
[ui]
default_sort = "time"
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse toml");
        assert!(config.scan.extra_targets.is_empty());
        assert_eq!(config.ui.default_sort.as_deref(), Some("time"));
    }

    #[test]
    fn parse_empty_toml_returns_defaults() {
        let config: AppConfig = toml::from_str("").expect("parse empty toml");
        assert!(config.scan.extra_targets.is_empty());
        assert!(config.ui.default_sort.is_none());
    }

    #[test]
    fn expanded_extra_targets_filters_nonexistent() {
        let config = AppConfig {
            scan: ScanConfig {
                extra_targets: vec![
                    "/tmp".to_string(),
                    "/nonexistent_vac_path_12345".to_string(),
                ],
            },
            ui: UiConfig::default(),
            safety: SafetyConfig::default(),
        };
        let expanded = config.expanded_extra_targets();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0], PathBuf::from("/tmp"));
    }

    #[test]
    fn default_safety_config_has_move_to_trash_false() {
        let config = SafetyConfig::default();
        assert!(!config.move_to_trash);
    }

    #[test]
    fn parse_safety_config_move_to_trash() {
        let toml_str = r#"
[safety]
move_to_trash = true
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse toml");
        assert!(config.safety.move_to_trash);
    }

    #[test]
    fn parse_full_config_with_safety() {
        let toml_str = r#"
[scan]
extra_targets = ["/tmp"]

[ui]
default_sort = "size"

[safety]
move_to_trash = true
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse toml");
        assert_eq!(config.scan.extra_targets.len(), 1);
        assert_eq!(config.ui.default_sort.as_deref(), Some("size"));
        assert!(config.safety.move_to_trash);
    }

    #[test]
    fn parse_toml_without_safety_uses_default() {
        let toml_str = r#"
[scan]
extra_targets = []
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse toml");
        assert!(!config.safety.move_to_trash);
    }
}
