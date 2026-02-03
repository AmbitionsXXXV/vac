use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use walkdir::WalkDir;

use crate::app::{CleanableEntry, EntryKind, ItemCategory};

/// 扫描类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    /// 扫描预设可清理目录
    Root,
    /// 列出目录内容
    ListDir,
    /// 磁盘扫描（指定路径）
    DiskScan,
}

/// 扫描进度消息
#[derive(Debug, Clone)]
pub enum ScanMessage {
    /// 进度更新 (进度百分比, 当前扫描路径)
    Progress {
        job_id: u64,
        progress: u8,
        path: String,
    },
    /// 根目录扫描单项完成
    RootItem { job_id: u64, entry: CleanableEntry },
    /// 目录条目
    DirEntry { job_id: u64, entry: CleanableEntry },
    /// 目录大小回填
    DirEntrySize { job_id: u64, path: PathBuf, size: u64 },
    /// 全部扫描完成
    Done { job_id: u64 },
    /// 扫描出错
    Error { job_id: u64, message: String },
}

impl ScanMessage {
    pub fn job_id(&self) -> u64 {
        match self {
            ScanMessage::Progress { job_id, .. }
            | ScanMessage::RootItem { job_id, .. }
            | ScanMessage::DirEntry { job_id, .. }
            | ScanMessage::DirEntrySize { job_id, .. }
            | ScanMessage::Done { job_id }
            | ScanMessage::Error { job_id, .. } => *job_id,
        }
    }
}

/// 磁盘扫描器
pub struct Scanner {
    home_dir: PathBuf,
}

impl Scanner {
    pub fn new() -> Option<Self> {
        directories::UserDirs::new().map(|dirs| Self {
            home_dir: dirs.home_dir().to_path_buf(),
        })
    }

    /// 获取所有扫描目标
    pub fn get_scan_targets(&self) -> Vec<(ItemCategory, PathBuf)> {
        let mut targets = vec![
            // 系统缓存
            (
                ItemCategory::SystemCache,
                self.home_dir.join("Library/Caches"),
            ),
            // 日志文件
            (ItemCategory::Logs, self.home_dir.join("Library/Logs")),
            // 临时文件
            (ItemCategory::Temp, PathBuf::from("/tmp")),
            (ItemCategory::Temp, PathBuf::from("/var/tmp")),
            // 下载文件夹
            (ItemCategory::Downloads, self.home_dir.join("Downloads")),
            // 垃圾桶
            (ItemCategory::Trash, self.home_dir.join(".Trash")),
        ];

        // Xcode 派生数据（条件添加）
        let xcode_derived = self.home_dir.join("Library/Developer/Xcode/DerivedData");
        if xcode_derived.exists() {
            targets.push((ItemCategory::XcodeDerivedData, xcode_derived));
        }

        // Homebrew 缓存（条件添加）
        let brew_cache = self.home_dir.join("Library/Caches/Homebrew");
        if brew_cache.exists() {
            targets.push((ItemCategory::HomebrewCache, brew_cache));
        }

        targets
    }

    /// 扫描指定目录并返回大小
    pub fn scan_directory(&self, path: &PathBuf) -> u64 {
        if !path.exists() {
            return 0;
        }

        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    fn scan_directory_with_cancel(
        &self,
        path: &PathBuf,
        job_id: u64,
        cancel_gen: &AtomicU64,
    ) -> u64 {
        if !path.exists() {
            return 0;
        }

        let mut total = 0u64;
        for entry in WalkDir::new(path).into_iter() {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return total;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }

        total
    }

    /// 带进度回调的根目录扫描
    pub fn scan_root_with_progress(
        &self,
        job_id: u64,
        tx: Sender<ScanMessage>,
        cancel_gen: Arc<AtomicU64>,
    ) {
        if cancel_gen.load(Ordering::Relaxed) != job_id {
            return;
        }

        let targets = self.get_scan_targets();
        let total = targets.len().max(1);

        for (index, (category, path)) in targets.into_iter().enumerate() {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let progress = ((index as f32 / total as f32) * 100.0) as u8;
            let path_str = path.display().to_string();
            let _ = tx.send(ScanMessage::Progress {
                job_id,
                progress,
                path: path_str,
            });

            if path.exists() {
                let size = self.scan_directory_with_cancel(&path, job_id, &cancel_gen);
                if cancel_gen.load(Ordering::Relaxed) != job_id {
                    return;
                }
                if size > 0 {
                    let name = category.as_str().to_string();
                    let entry = CleanableEntry {
                        kind: EntryKind::Directory,
                        category: Some(category),
                        path,
                        name,
                        size: Some(size),
                    };
                    let _ = tx.send(ScanMessage::RootItem { job_id, entry });
                }
            }
        }

        let _ = tx.send(ScanMessage::Done { job_id });
    }

    /// 扫描目录列表（仅当前层级）
    pub fn scan_dir_listing(
        &self,
        job_id: u64,
        path: PathBuf,
        tx: Sender<ScanMessage>,
        cancel_gen: Arc<AtomicU64>,
    ) {
        if cancel_gen.load(Ordering::Relaxed) != job_id {
            return;
        }

        let read_dir = match fs::read_dir(&path) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                let _ = tx.send(ScanMessage::Error {
                    job_id,
                    message: format!("无法读取目录 {}: {}", path.display(), err),
                });
                return;
            }
        };

        let mut dir_paths = Vec::new();

        for entry in read_dir {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let entry_path = entry.path();
            let name = entry
                .file_name()
                .to_string_lossy()
                .to_string();

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                dir_paths.push(entry_path.clone());
                let entry = CleanableEntry {
                    kind: EntryKind::Directory,
                    category: None,
                    path: entry_path,
                    name,
                    size: None,
                };
                let _ = tx.send(ScanMessage::DirEntry { job_id, entry });
            } else if file_type.is_file() {
                let size = entry.metadata().ok().map(|m| m.len());
                let entry = CleanableEntry {
                    kind: EntryKind::File,
                    category: None,
                    path: entry_path,
                    name,
                    size,
                };
                let _ = tx.send(ScanMessage::DirEntry { job_id, entry });
            }
        }

        for dir_path in dir_paths {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let size = self.scan_directory_with_cancel(&dir_path, job_id, &cancel_gen);
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let _ = tx.send(ScanMessage::DirEntrySize {
                job_id,
                path: dir_path,
                size,
            });
        }

        let _ = tx.send(ScanMessage::Done { job_id });
    }

    /// 磁盘扫描（扫描指定路径的顶层目录/文件）
    pub fn scan_disk_with_progress(
        &self,
        job_id: u64,
        path: PathBuf,
        tx: Sender<ScanMessage>,
        cancel_gen: Arc<AtomicU64>,
    ) {
        if cancel_gen.load(Ordering::Relaxed) != job_id {
            return;
        }

        if !path.exists() {
            let _ = tx.send(ScanMessage::Error {
                job_id,
                message: format!("路径不存在: {}", path.display()),
            });
            return;
        }

        if !path.is_dir() {
            let _ = tx.send(ScanMessage::Error {
                job_id,
                message: format!("不是目录: {}", path.display()),
            });
            return;
        }

        let _ = tx.send(ScanMessage::Progress {
            job_id,
            progress: 0,
            path: path.display().to_string(),
        });

        let read_dir = match fs::read_dir(&path) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                let _ = tx.send(ScanMessage::Error {
                    job_id,
                    message: format!("无法读取目录 {}: {}", path.display(), err),
                });
                return;
            }
        };

        // 收集所有条目
        let entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
        let total = entries.len().max(1);
        let mut dir_paths = Vec::new();

        for (index, entry) in entries.into_iter().enumerate() {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let progress = ((index as f32 / total as f32) * 50.0) as u8;
            let entry_path = entry.path();
            let _ = tx.send(ScanMessage::Progress {
                job_id,
                progress,
                path: entry_path.display().to_string(),
            });

            let name = entry.file_name().to_string_lossy().to_string();

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                dir_paths.push(entry_path.clone());
                let entry = CleanableEntry {
                    kind: EntryKind::Directory,
                    category: None,
                    path: entry_path,
                    name,
                    size: None,
                };
                let _ = tx.send(ScanMessage::RootItem { job_id, entry });
            } else if file_type.is_file() {
                let size = entry.metadata().ok().map(|m| m.len());
                let entry = CleanableEntry {
                    kind: EntryKind::File,
                    category: None,
                    path: entry_path,
                    name,
                    size,
                };
                let _ = tx.send(ScanMessage::RootItem { job_id, entry });
            }
        }

        // 计算目录大小
        let dir_total = dir_paths.len().max(1);
        for (index, dir_path) in dir_paths.into_iter().enumerate() {
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let progress = 50 + ((index as f32 / dir_total as f32) * 50.0) as u8;
            let _ = tx.send(ScanMessage::Progress {
                job_id,
                progress,
                path: format!("计算大小: {}", dir_path.display()),
            });

            let size = self.scan_directory_with_cancel(&dir_path, job_id, &cancel_gen);
            if cancel_gen.load(Ordering::Relaxed) != job_id {
                return;
            }

            let _ = tx.send(ScanMessage::DirEntrySize {
                job_id,
                path: dir_path,
                size,
            });
        }

        let _ = tx.send(ScanMessage::Done { job_id });
    }

    /// 获取用户主目录
    pub fn home_dir(&self) -> &PathBuf {
        &self.home_dir
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new().expect("无法获取用户目录")
    }
}

/// 格式化字节大小为人类可读格式
pub fn format_size(bytes: u64) -> String {
    bytesize::ByteSize::b(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::{Arc, atomic::AtomicU64};

    #[test]
    fn scan_directory_returns_zero_for_missing_path() {
        let scanner = Scanner::new().expect("user dirs");
        let size = scanner.scan_directory(&PathBuf::from("/tmp/path-does-not-exist"));
        assert_eq!(size, 0);
    }

    #[test]
    fn scan_directory_sums_file_sizes() {
        let scanner = Scanner::new().expect("user dirs");
        let dir = tempfile::Builder::new()
            .prefix("vac-scan-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let file_a = dir.path().join("a.txt");
        fs::write(&file_a, b"hello").expect("write file a");

        let sub = dir.path().join("sub");
        fs::create_dir(&sub).expect("create sub dir");
        let file_b = sub.join("b.bin");
        fs::write(&file_b, vec![0u8; 10]).expect("write file b");

        let size = scanner.scan_directory(&dir.path().to_path_buf());
        assert_eq!(size, 15);
    }

    #[test]
    fn scan_dir_listing_emits_entries_and_sizes() {
        let scanner = Scanner::new().expect("user dirs");
        let dir = tempfile::Builder::new()
            .prefix("vac-list-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, b"hello").expect("write file");

        let sub_dir = dir.path().join("folder");
        fs::create_dir(&sub_dir).expect("create dir");
        let nested = sub_dir.join("nested.txt");
        fs::write(&nested, b"world").expect("write nested");

        let (tx, rx) = mpsc::channel();
        let cancel_gen = Arc::new(AtomicU64::new(1));

        scanner.scan_dir_listing(1, dir.path().to_path_buf(), tx, cancel_gen);

        let mut saw_dir = false;
        let mut saw_dir_size = false;
        for msg in rx {
            match msg {
                ScanMessage::DirEntry { entry, .. } => {
                    if entry.kind == EntryKind::Directory {
                        saw_dir = true;
                    }
                }
                ScanMessage::DirEntrySize { path, size, .. } => {
                    if path == sub_dir && size > 0 {
                        saw_dir_size = true;
                    }
                }
                ScanMessage::Done { .. } => break,
                _ => {}
            }
        }

        assert!(saw_dir);
        assert!(saw_dir_size);
    }

    #[test]
    fn scan_dir_listing_respects_cancel_generation() {
        let scanner = Scanner::new().expect("user dirs");
        let dir = tempfile::Builder::new()
            .prefix("vac-cancel-")
            .tempdir_in("/tmp")
            .expect("create temp dir");

        let (tx, rx) = mpsc::channel();
        let cancel_gen = Arc::new(AtomicU64::new(2));

        scanner.scan_dir_listing(1, dir.path().to_path_buf(), tx, cancel_gen);

        assert!(rx.try_recv().is_err());
    }
}
