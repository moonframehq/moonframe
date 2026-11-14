//! Following a `scroll_at` without echo, for each element that tells and
//! takes how far down its text it is scrolled (`scrolled`, `scroll_at`).
//!
//! A request scrolls the text from where it was (`from`) to where it asked
//! (`to`). Until the text shows at `to`, an offset still at `from` is the
//! request not yet carried out, and so is nothing to tell. One anywhere else
//! is the reader, who moved before or after it landed, and is told. There is
//! no count of looks to run out, so a slow layout is never taken for the
//! reader, and a reader's move while a request waits is never lost.

use crate::ScrollAt;

/// Points of a move too small to tell.
pub const SCROLL_JITTER: f32 = 0.5;

/// What an element keeps of its scrolling between frames, in points down
/// its text.
#[derive(Clone, Debug, Default)]
pub struct ScrollFollow {
    /// The last `scroll_at` taken, and its fraction until it is carried out.
    taken: Option<ScrollAt>,
    pending: Option<f32>,
    /// A request carried out that has not shown yet.
    expect: Option<Expect>,
    /// The offset last seen.
    seen: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Expect {
    from: f32,
    to: f32,
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < SCROLL_JITTER
}

impl ScrollFollow {
    /// Takes this frame's `scroll_at`: a new one waits to be carried out.
    pub fn take(&mut self, at: Option<ScrollAt>) {
        if at != self.taken {
            self.taken = at;
            self.pending = at.map(|at| at.fraction);
        }
    }

    /// Whether a `scroll_at` waits to be carried out.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Carries out a waiting `scroll_at` on text at `offset`, which can be
    /// scrolled `max`: where to scroll it, unless it is there already.
    pub fn scroll(&mut self, offset: f32, max: f32) -> Option<f32> {
        let fraction = self.pending.take()?;
        let to = (max.max(0.0) * fraction).clamp(0.0, max.max(0.0));
        if near(offset, to) {
            self.landed(offset);
            return None;
        }
        self.expect = Some(Expect { from: offset, to });
        Some(to)
    }

    /// The text shows at `offset` where a request put it, as an engine that
    /// scrolls at once says: nothing to tell of it.
    pub fn landed(&mut self, offset: f32) {
        self.expect = None;
        self.seen = Some(offset);
    }

    /// The fraction to tell of text now at `offset` of `max`, if the reader
    /// moved it since it was last seen. A move that is not the reader's (its
    /// text changing length, say: `reader` false) is only noted.
    pub fn moved(&mut self, offset: f32, max: f32, reader: bool) -> Option<f32> {
        if let Some(expect) = self.expect.take() {
            if near(offset, expect.from) && !near(offset, expect.to) {
                // Not carried out yet.
                self.expect = Some(expect);
                return None;
            }
            self.seen = Some(offset);
            let elsewhere = !near(offset, expect.to);
            return (reader && elsewhere).then(|| ScrollAt::fraction(offset, max));
        }
        let before = self.seen.replace(offset)?;
        (reader && !near(offset, before)).then(|| ScrollAt::fraction(offset, max))
    }

    /// Forgets where the text was seen, for an element that tells nothing
    /// now: when it tells again, it starts from where it is then.
    pub fn forget(&mut self) {
        self.expect = None;
        self.seen = None;
    }
}
