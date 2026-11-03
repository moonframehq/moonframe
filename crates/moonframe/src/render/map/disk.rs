//! The tile cache on disk: kept under a size, oldest fetched first out,
//! and the date a tile was fetched, as HTTP writes it, to ask the server
//! whether it changed since.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The most the tile cache holds on disk, across providers.
const MOST_BYTES: u64 = 512 * 1024 * 1024;

/// Where every provider's tiles are kept.
pub(super) fn folder() -> PathBuf {
    crate::paths::cache().join("tiles")
}

/// Brings the cache under [`MOST_BYTES`], deleting the tiles fetched
/// longest ago first. Run once a run, in the background.
pub(super) fn prune(folder: &Path) {
    prune_to(folder, MOST_BYTES);
}

/// Brings `folder` under `most` bytes, down to four fifths of it so it
/// does not run again at once, oldest fetched first.
fn prune_to(folder: &Path, most: u64) {
    let mut files = Vec::new();
    collect(folder, &mut files, 0);
    let total: u64 = files.iter().map(|(_, _, len)| len).sum();
    if total <= most {
        return;
    }
    let target = most / 5 * 4;
    files.sort_by_key(|(_, modified, _)| *modified);
    let mut left = total;
    for (path, _, len) in files {
        if left <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            left -= len;
        }
    }
    tracing::debug!("map tiles: pruned the cache from {total} to {left} bytes");
}

/// Every file under `folder`, with when it was fetched and its size. The
/// cache is provider/zoom/x/y deep; deeper is not the cache's.
fn collect(folder: &Path, files: &mut Vec<(PathBuf, SystemTime, u64)>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() && depth < 4 {
            collect(&entry.path(), files, depth + 1);
        } else if metadata.is_file() {
            let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
            files.push((entry.path(), modified, metadata.len()));
        }
    }
}

/// `at` as HTTP writes dates: `Sun, 06 Nov 1994 08:49:37 GMT`.
pub(super) fn http_date(at: SystemTime) -> String {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
    // 1 January 1970 was a Thursday.
    let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][(days % 7) as usize];
    let (year, month, day) = civil(days as i64);
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month - 1) as usize];
    format!("{weekday}, {day:02} {month} {year} {hour:02}:{minute:02}:{second:02} GMT")
}

/// The year, month and day `days` after 1 January 1970 (Howard Hinnant's
/// civil-from-days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 {
        mp + 3
    } else {
        mp - 9
    } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_http_writes_them() {
        let at = UNIX_EPOCH + Duration::from_secs(784_111_777);
        assert_eq!(http_date(at), "Sun, 06 Nov 1994 08:49:37 GMT");
        assert_eq!(http_date(UNIX_EPOCH), "Thu, 01 Jan 1970 00:00:00 GMT");
        let leap = UNIX_EPOCH + Duration::from_secs(1_709_208_000);
        assert_eq!(http_date(leap), "Thu, 29 Feb 2024 12:00:00 GMT");
    }

    #[test]
    fn pruning_deletes_the_oldest_until_under_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("osm/16/10485");
        std::fs::create_dir_all(&folder).unwrap();
        // Ten tiles of 100 bytes, fetched a minute apart, oldest first.
        let files: Vec<PathBuf> = (0..10u64)
            .map(|index| {
                let path = folder.join(format!("{index}.png"));
                std::fs::write(&path, [0u8; 100]).unwrap();
                let at = UNIX_EPOCH + Duration::from_secs(1_000_000 + index * 60);
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(at)
                    .unwrap();
                path
            })
            .collect();
        // Under the cap: nothing goes.
        prune_to(dir.path(), 1000);
        assert!(files.iter().all(|file| file.is_file()));
        // Over it: the oldest go until four fifths of the cap is left.
        prune_to(dir.path(), 500);
        let kept: Vec<bool> = files.iter().map(|file| file.is_file()).collect();
        assert_eq!(kept, [
            false, false, false, false, false, false, true, true, true, true
        ]);
    }
}
