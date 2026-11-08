//! Lines from a stream that arrives in chunks, each at most a set length:
//! a longer one is dropped whole, its tail included, never read as a line.

pub(crate) struct Lines {
    limit: usize,
    pending: Vec<u8>,
    /// Inside a line already too long: dropped through its newline.
    skipping: bool,
}

impl Lines {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            pending: Vec::new(),
            skipping: false,
        }
    }

    /// The whole lines `chunk` completes, without their newlines.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        let mut lines = Vec::new();
        self.pending.extend_from_slice(chunk);
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop();
            if std::mem::take(&mut self.skipping) || line.len() > self.limit {
                continue;
            }
            lines.push(line);
        }
        if self.pending.len() > self.limit {
            self.pending.clear();
            self.skipping = true;
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed(limit: usize, chunks: &[&str]) -> Vec<String> {
        let mut lines = Lines::new(limit);
        chunks
            .iter()
            .flat_map(|chunk| lines.feed(chunk.as_bytes()))
            .map(|line| String::from_utf8_lossy(&line).into_owned())
            .collect()
    }

    #[test]
    fn lines_split_across_chunks_come_whole() {
        assert_eq!(fed(8, &["ab", "c\nde", "\n"]), ["abc", "de"]);
    }

    #[test]
    fn a_long_line_ending_in_the_chunk_that_crosses_the_limit_is_dropped() {
        assert_eq!(fed(4, &["ok\nabc", "defg\nok2\n"]), ["ok", "ok2"]);
    }

    #[test]
    fn the_tail_of_a_long_line_cleared_earlier_is_not_a_line() {
        assert_eq!(fed(4, &["abcdef", "gh\nok\n"]), ["ok"]);
    }

    #[test]
    fn a_line_at_the_limit_is_kept() {
        assert_eq!(fed(4, &["abcd\n"]), ["abcd"]);
    }
}
