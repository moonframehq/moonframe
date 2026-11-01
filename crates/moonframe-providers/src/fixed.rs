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
            Ok(data) => context.publish(data),
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
/// file may have been swapped for one since.
fn read(path: &Path) -> std::io::Result<String> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::other("it is a link"));
    }
    // One byte past the limit is enough to refuse it, without reading more.
    let mut text = String::new();
    fs::File::open(path)?
        .take(json::MAX_JSON as u64 + 1)
        .read_to_string(&mut text)?;
    Ok(text)
}
