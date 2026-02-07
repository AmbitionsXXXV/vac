use std::fs;
use std::path::Path;

use crate::app::CleanableEntry;

/// 清理结果
#[derive(Debug)]
pub struct CleanResult {
    pub success: bool,
    pub freed_space: u64,
    pub errors: Vec<String>,
}

/// 磁盘清理器
pub struct Cleaner;

impl Cleaner {
    /// 清理选中的项目
    pub fn clean(items: &[CleanableEntry]) -> CleanResult {
        let mut freed_space = 0u64;
        let mut errors = Vec::new();

        for item in items {
            match Self::remove_path(&item.path) {
                Ok(()) => {
                    freed_space += item.size.unwrap_or(0);
                }
                Err(e) => {
                    errors.push(format!("{}: {}", item.path.display(), e));
                }
            }
        }

        CleanResult {
            success: errors.is_empty(),
            freed_space,
            errors,
        }
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

        // 不允许删除的路径
        const FORBIDDEN: &[&str] = &[
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

        let path_str = canonical.to_string_lossy();

        // 检查是否为禁止路径
        for f in FORBIDDEN {
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
        }
    }

    #[test]
    fn is_safe_to_delete_rejects_forbidden_paths() {
        let forbidden = [
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

        for path in forbidden {
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
}
