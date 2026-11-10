//! Static providers: a JSON file in the provider's folder, published as it
//! is. No code runs. An edit to the file changes the folder, and a reload
//! starts the provider again; a message reads it and publishes it again, so
//! a mod's Refresh hears back.

use {
    crate::{Context, Kind, json},
    std::{fs, io::Read, path::Path},
};

pub(crate) fn run(context: &Context) {
    let Kind::Static { file } = &context.manifest.kind else {
        context.fail("not a static provider");
        return;
    };
    let path = context.dir.join(file);
    loop {
        let read = read(&path)
            .map_err(|error| format!("cannot read {file}: {error}"))
            .and_then(|text| json::data(&text).map_err(|problem| format!("{file}: {problem}")));
        match read {
            Ok(data) => {
                context.status(moonframe_core::entries::EntryState::Ok, None);
                context.publish(data);
            },
            Err(problem) => {
                context.fail(problem);
                return;
            },
        }
        // Until the app stops it.
        if context.messages.recv().is_err() || context.stopped() {
            return;
        }
    }
}

/// The file's text, refusing a link, which the manifest refused too: the
/// file may have been swapped for one since. What was opened must be the
/// very file the path names now, not a link's target, so a link swapped in
/// and out around the open is refused too.
pub(crate) fn read(path: &Path) -> std::io::Result<String> {
    let file = fs::File::open(path)?;
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !same_file(&file.metadata()?, &named) {
        return Err(std::io::Error::other("it is a link"));
    }
    // One byte past the limit is enough to refuse it, without reading more.
    let mut text = String::new();
    file.take(json::MAX_JSON as u64 + 1)
        .read_to_string(&mut text)?;
    Ok(text)
}

#[cfg(unix)]
fn same_file(opened: &fs::Metadata, named: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    (opened.dev(), opened.ino()) == (named.dev(), named.ino())
}

/// Without inode numbers, the link check above is what there is.
#[cfg(not(unix))]
fn same_file(opened: &fs::Metadata, named: &fs::Metadata) -> bool {
    opened.is_file() && named.is_file()
}
