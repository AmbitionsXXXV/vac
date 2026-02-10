use std::path::PathBuf;

use clap::Parser;

use crate::utils::expand_tilde;

/// VAC - macOS 磁盘清理工具
///
/// 无参数启动时进入 TUI 交互界面；使用 --scan 等参数可以非交互模式运行。
#[derive(Parser, Debug)]
#[command(name = "vac", version, about, long_about = None)]
pub struct Cli {
    /// 执行扫描（非交互模式）。可选值: preset（预设目录）、home（主目录）、或指定路径
    #[arg(long, value_name = "MODE_OR_PATH")]
    pub scan: Option<ScanTarget>,

    /// 仅模拟删除，不执行实际清理（需配合 --clean 使用）
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// 执行清理（清理扫描结果中的所有项目）
    #[arg(long, default_value_t = false)]
    pub clean: bool,

    /// 将结果输出到指定文件（支持 .json 格式）
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// 排序方式: name / size / time
    #[arg(long, value_name = "ORDER", default_value = "size")]
    pub sort: String,

    /// 使用回收站而非永久删除（覆盖配置文件设置）
    #[arg(long, default_value_t = false)]
    pub trash: bool,
}

/// 扫描目标类型
#[derive(Debug, Clone)]
pub enum ScanTarget {
    /// 扫描预设可清理目录
    Preset,
    /// 扫描用户主目录
    Home,
    /// 扫描指定路径
    Path(PathBuf),
}

impl std::str::FromStr for ScanTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "preset" => Ok(ScanTarget::Preset),
            "home" => Ok(ScanTarget::Home),
            other => {
                let path = PathBuf::from(expand_tilde(other));
                Ok(ScanTarget::Path(path))
            }
        }
    }
}

impl Cli {
    /// 判断是否为非交互模式（传入了 --scan 参数）
    pub fn is_non_interactive(&self) -> bool {
        self.scan.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_target_parses_preset() {
        let target: ScanTarget = "preset".parse().unwrap();
        assert!(matches!(target, ScanTarget::Preset));
    }

    #[test]
    fn scan_target_parses_home() {
        let target: ScanTarget = "home".parse().unwrap();
        assert!(matches!(target, ScanTarget::Home));
    }

    #[test]
    fn scan_target_parses_absolute_path() {
        let target: ScanTarget = "/tmp/test".parse().unwrap();
        match target {
            ScanTarget::Path(p) => assert_eq!(p, PathBuf::from("/tmp/test")),
            _ => panic!("expected Path variant"),
        }
    }

    #[test]
    fn scan_target_parses_tilde_path() {
        let target: ScanTarget = "~/Documents".parse().unwrap();
        match target {
            ScanTarget::Path(p) => {
                let path_str = p.display().to_string();
                assert!(!path_str.starts_with('~'));
                assert!(path_str.ends_with("Documents"));
            }
            _ => panic!("expected Path variant"),
        }
    }

    #[test]
    fn cli_parse_no_args_is_interactive() {
        let cli = Cli::parse_from(["vac"]);
        assert!(!cli.is_non_interactive());
    }

    #[test]
    fn cli_parse_scan_preset() {
        let cli = Cli::parse_from(["vac", "--scan", "preset"]);
        assert!(cli.is_non_interactive());
        assert!(matches!(cli.scan, Some(ScanTarget::Preset)));
    }

    #[test]
    fn cli_parse_scan_with_dry_run() {
        let cli = Cli::parse_from(["vac", "--scan", "preset", "--dry-run"]);
        assert!(cli.dry_run);
    }

    #[test]
    fn cli_parse_scan_with_output() {
        let cli = Cli::parse_from(["vac", "--scan", "preset", "--output", "report.json"]);
        assert_eq!(cli.output, Some(PathBuf::from("report.json")));
    }

    #[test]
    fn cli_parse_scan_with_sort() {
        let cli = Cli::parse_from(["vac", "--scan", "preset", "--sort", "name"]);
        assert_eq!(cli.sort, "name");
    }

    #[test]
    fn cli_parse_trash_flag() {
        let cli = Cli::parse_from(["vac", "--scan", "preset", "--clean", "--trash"]);
        assert!(cli.trash);
        assert!(cli.clean);
    }

    #[test]
    fn cli_default_sort_is_size() {
        let cli = Cli::parse_from(["vac"]);
        assert_eq!(cli.sort, "size");
    }
}
