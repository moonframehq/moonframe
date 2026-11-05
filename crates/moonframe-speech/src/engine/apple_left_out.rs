//! Apple's recognizer exists only on macOS.

use {super::Cancel, std::path::Path};

const ONLY_MACOS: &str = "Apple dictation is only on macOS";

pub(super) fn check(_: Option<&str>) -> Option<String> {
    Some(ONLY_MACOS.to_owned())
}

pub(super) fn transcribe(_: &Path, _: Option<&str>, _: &Cancel) -> Result<String, String> {
    Err(ONLY_MACOS.to_owned())
}
