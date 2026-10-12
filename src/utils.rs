use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;
pub const EPOCH_YEAR: i32 = 1970;

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_year(year: i32) -> i64 {
    if is_leap_year(year) { 366 } else { 365 }
}

/// Apple Silicon 与 Intel 两种 Homebrew 安装前缀。
const HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];

/// Homebrew 下载队列解压 bottle/cask 的暂存目录（`HOMEBREW_TEMP_CELLAR` /
/// `HOMEBREW_TEMP_CASKROOM`）。升级中断后内容会一直留下；`brew cleanup` 的
/// `cleanup_temp_staging` 也是直接逐项删除这两个目录的子项。
const HOMEBREW_STAGING_SUBDIRS: &[&str] =
    &["var/homebrew/tmp/.cellar", "var/homebrew/tmp/.caskroom"];

/// 返回本机存在的 Homebrew 暂存目录（已规范化，可直接用于路径前缀比较）。
pub fn homebrew_staging_dirs() -> Vec<PathBuf> {
    HOMEBREW_PREFIXES
        .iter()
        .flat_map(|prefix| {
            HOMEBREW_STAGING_SUBDIRS
                .iter()
                .map(move |subdir| Path::new(prefix).join(subdir))
        })
        .filter_map(|dir| dir.canonicalize().ok())
        .filter(|dir| dir.is_dir())
        .collect()
}

/// 将路径中的 `~` 展开为主目录绝对路径。
pub fn expand_tilde(raw_path: &str) -> String {
    if raw_path.starts_with('~')
        && let Some(user_dirs) = directories::UserDirs::new()
    {
        let home_path = user_dirs.home_dir().display().to_string();
        return raw_path.replacen('~', &home_path, 1);
    }
    raw_path.to_string()
}

/// 格式化 SystemTime。
///
/// - `include_time = false` => `YYYY-MM-DD`
/// - `include_time = true` => `YYYY-MM-DD HH:MM:SS`
pub fn format_time(time: &SystemTime, include_time: bool) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp_seconds = duration.as_secs() as i64;

    let total_days = timestamp_seconds / SECONDS_PER_DAY;
    let seconds_within_day = timestamp_seconds % SECONDS_PER_DAY;
    let hours = seconds_within_day / SECONDS_PER_HOUR;
    let minutes = (seconds_within_day % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let seconds = seconds_within_day % SECONDS_PER_MINUTE;

    let mut remaining_days = total_days;
    let mut year = EPOCH_YEAR;
    loop {
        let current_year_days = days_in_year(year);
        if remaining_days < current_year_days {
            break;
        }
        remaining_days -= current_year_days;
        year += 1;
    }

    let month_day_table: [i64; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month_index = 0usize;
    for (index, &days_in_month) in month_day_table.iter().enumerate() {
        if remaining_days < days_in_month {
            month_index = index;
            break;
        }
        remaining_days -= days_in_month;
    }

    let day_of_month = remaining_days + 1;
    if include_time {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year,
            month_index + 1,
            day_of_month,
            hours,
            minutes,
            seconds
        )
    } else {
        format!("{:04}-{:02}-{:02}", year, month_index + 1, day_of_month)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn format_time_formats_date_without_clock() {
        let time = UNIX_EPOCH + Duration::from_secs(SECONDS_PER_DAY as u64);
        assert_eq!(format_time(&time, false), "1970-01-02");
    }

    #[test]
    fn format_time_formats_date_with_clock() {
        let time = UNIX_EPOCH + Duration::from_secs(SECONDS_PER_DAY as u64 + 3_661);
        assert_eq!(format_time(&time, true), "1970-01-02 01:01:01");
    }

    #[test]
    fn expand_tilde_keeps_plain_path() {
        assert_eq!(expand_tilde("/tmp"), "/tmp");
    }
}
