//! Reading a child's output a line at a time without letting one runaway
//! line fill memory.

use std::io::{BufRead, BufReader, Read};

/// The longest stderr line kept for the log; the rest of a longer one is
/// dropped.
pub const MAX_LOG_LINE: usize = 8 << 10;

/// Hands each line of `output` to `line`, cut to [`MAX_LOG_LINE`] bytes,
/// until it ends or fails. For a log: nothing printed, however long, costs
/// more than one line's worth of memory.
pub fn log_lines(output: impl Read, line: impl FnMut(String)) {
    bounded_lines(output, MAX_LOG_LINE, line);
}

/// [`log_lines`] with a limit of `max` bytes a line.
pub fn bounded_lines(output: impl Read, max: usize, mut line: impl FnMut(String)) {
    let mut reader = BufReader::new(output);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = (&mut reader)
            .take(max as u64)
            .read_until(b'\n', &mut buffer);
        match read {
            Ok(0) | Err(_) => return,
            Ok(_) => {},
        }
        let whole = buffer.last() == Some(&b'\n');
        if whole {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }
        line(String::from_utf8_lossy(&buffer).into_owned());
        if !whole && !skip_line(&mut reader) {
            return;
        }
    }
}

/// Drops the rest of an overlong line, through its newline. False once the
/// output ends or fails.
fn skip_line(reader: &mut impl BufRead) -> bool {
    loop {
        let (found, used) = match reader.fill_buf() {
            Ok([]) | Err(_) => return false,
            Ok(chunk) => match chunk.iter().position(|byte| *byte == b'\n') {
                Some(at) => (true, at + 1),
                None => (false, chunk.len()),
            },
        };
        reader.consume(used);
        if found {
            return true;
        }
    }
}
