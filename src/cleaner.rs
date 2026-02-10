use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::app::CleanableEntry;

/// 清理结果
#[derive(Debug)]
pub struct CleanResult {
    pub success: bool,
    pub freed_space: u64,
    pub errors: Vec<String>,
}

/// Dry-run 单项详情
#[derive(Debug, Clone)]
pub struct DryRunItem {
    pub path: std::path::PathBuf,
    pub file_count: usize,
    pub dir_count: usize,
    pub size: u64,
}

/// Dry-run 结果（不执行实际删除）
#[derive(Debug, Clone)]
pub struct DryRunResult {
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_size: u64,
    pub items: Vec<DryRunItem>,
}

/// 磁盘清理器
pub struct Cleaner;

const FORBIDDEN_PATHS: &[&str] = &[
    "/",
    "/System",
    "/Library",
    "/Applications",
    "/Users",
    "/bin",
    "/sbin",
    "/usr",
    "/var",
    "/etc",
    "/private",
];

impl Cleaner {
    /// 清理选中的项目（永久删除）
    pub fn clean(items: &[CleanableEntry]) -> CleanResult {
        Self::process_items(items, |item| {
            Self::remove_path(&item.path).map_err(|error| error.to_string())?;
            Ok(true)
        })
    }

    /// 将选中的项目移至系统回收站
    pub fn trash_items(items: &[CleanableEntry]) -> CleanResult {
        Self::process_items(items, |item| {
            if !item.path.exists() {
                return Ok(false);
            }
            if item.path.is_dir() {
                Self::trash_dir_contents(&item.path)?;
                return Ok(true);
            }

            trash::delete(&item.path).map_err(|error| error.to_string())?;
            Ok(true)
        })
    }

    fn process_items<F>(items: &[CleanableEntry], mut action: F) -> CleanResult
    where
        F: FnMut(&CleanableEntry) -> Result<bool, String>,
    {
        let mut freed_space = 0u64;
        let mut errors = Vec::new();

        for item in items {
            match action(item) {
                Ok(should_add_freed_space) => {
                    if should_add_freed_space {
                        freed_space += item.size.unwrap_or(0);
                    }
                }
                Err(error_message) => {
                    errors.push(Self::format_item_error(&item.path, &error_message))
                }
            }
        }

        CleanResult {
            success: errors.is_empty(),
            freed_space,
            errors,
        }
    }

    fn format_item_error(path: &Path, error_message: &str) -> String {
        format!("{}: {}", path.display(), error_message)
    }

    /// 将目录内容移至回收站，保留目录结构本身
    fn trash_dir_contents(path: &Path) -> Result<(), String> {
        let entries: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .collect();

        let mut errors = Vec::new();
        for entry in entries {
            if let Err(e) = trash::delete(entry.path()) {
                errors.push(format!("{}: {}", entry.path().display(), e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    /// 模拟删除，统计将要删除的文件数、目录数和大小
    pub fn dry_run(items: &[CleanableEntry]) -> DryRunResult {
        let mut total_files = 0usize;
        let mut total_dirs = 0usize;
        let mut total_size = 0u64;
        let mut dry_run_items = Vec::new();

        for item in items {
            let (file_count, dir_count, size) = Self::count_path_contents(&item.path);
            total_files += file_count;
            total_dirs += dir_count;
            total_size += size;
            dry_run_items.push(DryRunItem {
                path: item.path.clone(),
                file_count,
                dir_count,
                size,
            });
        }

        DryRunResult {
            total_files,
            total_dirs,
            total_size,
            items: dry_run_items,
        }
    }

    /// 统计路径下的文件数、目录数和总大小
    fn count_path_contents(path: &Path) -> (usize, usize, u64) {
        if !path.exists() {
            return (0, 0, 0);
        }

        if path.is_file() {
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            return (1, 0, size);
        }

        let mut file_count = 0usize;
        let mut dir_count = 0usize;
        let mut size = 0u64;

        for entry in WalkDir::new(path).follow_links(false).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            // 跳过根路径本身（目录内容清理保留目录结构）
            if entry.path() == path {
                continue;
            }
            if entry.file_type().is_file() {
                file_count += 1;
                if let Ok(m) = entry.metadata() {
                    size += m.len();
                }
            } else if entry.file_type().is_dir() {
                dir_count += 1;
            }
        }

        (file_count, dir_count, size)
    }

    /// 删除指定路径（文件或目录）
    fn remove_path(path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        if path.is_dir() {
            // 遍历目录内容并删除，保留目录本身
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    fs::remove_dir_all(&entry_path)?;
                } else {
                    fs::remove_file(&entry_path)?;
                }
            }
        } else {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// 清空垃圾桶
    pub fn empty_trash() -> std::io::Result<u64> {
        let home = directories::UserDirs::new()
            .map(|d| d.home_dir().to_path_buf())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "无法获取用户目录"))?;

        let trash_path = home.join(".Trash");
        let mut freed = 0u64;

        if trash_path.exists() {
            for entry in fs::read_dir(&trash_path)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                freed += metadata.len();

                let path = entry.path();
                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
        }

        Ok(freed)
    }

    /// 安全检查：确保路径可以安全删除
    ///
    /// 使用 canonicalize 解析符号链接，防止通过符号链接绕过安全检查。
    /// 禁止删除系统关键目录和用户根目录本身。
    pub fn is_safe_to_delete(path: &Path) -> bool {
        // 规范化路径，解析符号链接
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        let path_str = canonical.to_string_lossy();

        // 检查是否为禁止路径
        for f in FORBIDDEN_PATHS {
            if path_str == *f {
                return false;
            }
        }

        // 确保路径在用户目录下或临时目录下
        if let Some(home) = directories::UserDirs::new() {
            let home_path = home.home_dir();
            // 不允许删除用户根目录本身
            if canonical == home_path {
                return false;
            }
            if canonical.starts_with(home_path) {
                return true;
            }
        }

        // 允许临时目录（含 macOS /private/tmp 实际路径）
        if canonical.starts_with("/tmp")
            || canonical.starts_with("/private/tmp")
            || canonical.starts_with("/var/tmp")
        {
            return true;
        }

        false
    }
}

impl Default for Cleaner {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CleanableEntry, EntryKind};
    use std::fs;
    use std::path::PathBuf;

    fn item(path: PathBuf, size: Option<u64>) -> CleanableEntry {
        CleanableEntry {
            kind: EntryKind::File,
            category: None,
            path,
            name: "item".to_string(),
            size,
            modified_at: None,
        }
    }

    #[test]
    fn is_safe_to_delete_rejects_forbidden_paths() {
        for path in FORBIDDEN_PATHS {
            assert!(!Cleaner::is_safe_to_delete(Path::new(path)));
        }
    }

    #[test]
    fn is_safe_to_delete_rejects_user_home_directory() {
        if let Some(home) = directories::UserDirs::new() {
            assert!(!Cleaner::is_safe_to_delete(home.home_dir()));
        }
    }

    #[test]
    fn is_safe_to_delete_rejects_nonexistent_paths() {
        assert!(!Cleaner::is_safe_to_delete(Path::new(
            "/tmp/vac-nonexistent-path-12345"
        )));
    }

    #[test]
    fn is_safe_to_delete_accepts_tmp_paths() {
        let dir = tempfile::Builder::new()
            .prefix("vac-test-")
            .tempdir_in("/tmp")
            .expect("create temp dir");
        assert!(Cleaner::is_safe_to_delete(dir.path()));
    }

    #[test]
    fn clean_removes_files_and_dir_contents() {
        let dir = tempfile::Builder::new()
            .prefix("vac-clean-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, b"hello").expect("write file");

        let dir_path = dir.path().join("folder");
        fs::create_dir(&dir_path).expect("create dir");
        let nested_file = dir_path.join("nested.txt");
        fs::write(&nested_file, b"world").expect("write nested file");

        let file_item = item(file_path.clone(), Some(5));
        let dir_item = item(dir_path.clone(), Some(5));

        let result = Cleaner::clean(&[file_item, dir_item]);

        assert!(result.success);
        assert!(!file_path.exists());
        assert!(dir_path.exists());
        assert_eq!(fs::read_dir(&dir_path).unwrap().count(), 0);
    }

    #[test]
    fn trash_items_moves_files_to_trash() {
        let dir = tempfile::Builder::new()
            .prefix("vac-trash-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let file_path = dir.path().join("trash_me.txt");
        fs::write(&file_path, b"trash test").expect("write file");

        let file_item = CleanableEntry {
            kind: EntryKind::File,
            category: None,
            path: file_path.clone(),
            name: "trash_me.txt".to_string(),
            size: Some(10),
            modified_at: None,
        };

        let result = Cleaner::trash_items(&[file_item]);
        assert!(result.success);
        assert!(!file_path.exists());
    }

    #[test]
    fn trash_items_moves_dir_contents_to_trash() {
        let dir = tempfile::Builder::new()
            .prefix("vac-trash-dir-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").expect("write file a");

        let dir_item = CleanableEntry {
            kind: EntryKind::Directory,
            category: None,
            path: dir.path().to_path_buf(),
            name: "test-dir".to_string(),
            size: Some(5),
            modified_at: None,
        };

        let result = Cleaner::trash_items(&[dir_item]);
        assert!(result.success);
        // 目录本身保留，但文件已移至回收站
        assert!(dir.path().exists());
        assert!(!file_a.exists());
    }

    #[test]
    fn trash_items_skips_nonexistent_paths() {
        let item = CleanableEntry {
            kind: EntryKind::File,
            category: None,
            path: PathBuf::from("/tmp/vac-nonexistent-trash-12345"),
            name: "nonexistent".to_string(),
            size: Some(0),
            modified_at: None,
        };

        let result = Cleaner::trash_items(&[item]);
        assert!(result.success);
        assert_eq!(result.freed_space, 0);
    }

    #[test]
    fn dry_run_counts_correctly() {
        let dir = tempfile::Builder::new()
            .prefix("vac-dryrun-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        // 创建已知结构: 2 个文件 + 1 个子目录（含 1 个文件）
        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").expect("write file a");

        let file_b = dir.path().join("b.txt");
        fs::write(&file_b, vec![0u8; 10]).expect("write file b");

        let sub_dir = dir.path().join("sub");
        fs::create_dir(&sub_dir).expect("create sub dir");
        let file_c = sub_dir.join("c.txt");
        fs::write(&file_c, b"world").expect("write file c");

        let dir_item = CleanableEntry {
            kind: EntryKind::Directory,
            category: None,
            path: dir.path().to_path_buf(),
            name: "test".to_string(),
            size: Some(20),
            modified_at: None,
        };

        let result = Cleaner::dry_run(&[dir_item]);

        assert_eq!(result.total_files, 3);
        assert_eq!(result.total_dirs, 1);
        assert_eq!(result.total_size, 20); // 5 + 10 + 5
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].file_count, 3);
        assert_eq!(result.items[0].dir_count, 1);
    }
}
