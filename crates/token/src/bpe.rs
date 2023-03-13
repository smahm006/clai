// This check is new and seems buggy (possibly with PyO3 interaction)
//#![allow(clippy::borrow_deref_ref)]

use std::collections::HashSet;
use std::thread;

use fancy_regex::Regex;
use std::collections::HashMap;

fn _byte_pair_merge<T>(
    piece: &[u8],
    ranks: &HashMap<Vec<u8>, usize>,
    f: impl Fn(std::ops::Range<usize>) -> T,
) -> Vec<T> {
    // This is a vector of (start, rank).
    // The rank is of the byte pair starting at position start.
    // The rank of the last item in the vector is not a valid value.
    let mut parts: Vec<(usize, usize)> = (0..piece.len() + 1).map(|i| (i, usize::MAX)).collect();

    // NOTE: using a macro here because a closure fails to get inlined
    // according to optimization remarks.
    // A closure also cannot capture a reference to `piece` without
    // the borrow checker complaining about the mutable borrows during
    // the assignments later in this code.
    macro_rules! get_rank {
        ($start_idx:expr, $skip:expr) => {{
            let start_idx: usize = $start_idx;
            let skip: usize = $skip;
            if (start_idx + skip + 2) < parts.len() {
                ranks
                    .get(&piece[parts[start_idx].0..parts[start_idx + skip + 2].0])
                    .map(|r| *r)
            } else {
                None
            }
        }};
        ($idx:expr) => {{
            get_rank!($idx, 0)
        }};
    }

    // We look up the ranks once in the beggining and iteratively update
    // them during each merge, which reduces the number of rank lookups.
    for i in 0..parts.len() - 2 {
        match get_rank!(i) {
            Some(rank) => {
                // usize::MAX is a sentinel value and cannot be a valid rank
                debug_assert!(rank != usize::MAX);
                parts[i].1 = rank;
            }
            None => {
                continue;
            }
        };
    }

    // If you have n parts and m merges, this does O(mn) work.
    // We could do something with a heap and do O(m log n) work.
    // It is important to consider that n is often small (<100), and as such
    // the cache-locality benefits outweigh the algorithmic complexity downsides
    // of the `parts` vector data structure above.

    // Note that we hash bytes, not token pairs. As long as we train BPE the way we
    // currently do, this is equivalent. An easy way to break this would be to decouple
    // merge priority from token index or to prevent specific token merges.
    loop {
        if parts.len() == 1 {
            break;
        }

        // usize::MAX is a sentinel rank value allowing us to
        // take the min more quickly
        let mut min_rank: (usize, usize) = (usize::MAX, 0);
        for (i, &(_, rank)) in parts[..parts.len() - 1].iter().enumerate() {
            if rank < min_rank.0 {
                min_rank = (rank, i);
            }
        }

        if min_rank.0 != usize::MAX {
            let i = min_rank.1;

            // NOTE: We are about to remove parts[i + 1]. We do not do it
            // yet because there are cache-locality benefits to updating
            // parts[i] and parts[i-1] before removing, which could thrash
            // the cache. Thus, we update the rank calculation by skipping over
            // parts[i + 1], by invoking `get_rank!` with `skip = 1`.
            parts[i].1 = get_rank!(i, 1).unwrap_or(usize::MAX);
            if i > 0 {
                parts[i - 1].1 = get_rank!(i - 1, 1).unwrap_or(usize::MAX);
            }

            parts.remove(i + 1);
        } else {
            break;
        }
    }
    let mut out: Vec<T> = Vec::with_capacity(parts.len() - 1);
    for i in 0..parts.len() - 1 {
        out.push(f(parts[i].0..parts[i + 1].0));
    }
    out
}

fn byte_pair_encode(piece: &[u8], ranks: &HashMap<Vec<u8>, usize>) -> Vec<usize> {
    if piece.len() == 1 {
        return vec![ranks[piece]];
    }
    _byte_pair_merge(piece, ranks, |p| ranks[&piece[p.start..p.end]])
}

fn byte_pair_split<'a>(piece: &'a [u8], ranks: &HashMap<Vec<u8>, usize>) -> Vec<&'a [u8]> {
    if piece.len() == 1 {
        return vec![piece];
    }
    _byte_pair_merge(piece, ranks, |p| &piece[p.start..p.end])
}

// Various performance notes:
//
// Regex
// =====
// Most of the time is spent in regex. The easiest way to speed this up is by using less fancy
// regex features. For instance, using a regex parse-able by `regex` crate is 3x faster than
// the usual regex we use.
//
// However, given that we're using a regex parse-able by `regex`, there isn't much difference
// between using the `regex` crate and using the `fancy_regex` crate.
//
// There is an important interaction between threading, `regex` and `fancy_regex`.
// When using `fancy_regex`, we hit `regex.find_at`. It turns out that this causes contention on
// some mutable scratch space inside of `regex`. This absolutely kills performance. When using plain
// old `regex`, we don't hit this, because `find_iter` has a different code path.
// Related: https://github.com/rust-lang/regex/blob/master/PERFORMANCE.md
// Anyway, the way we get around this is with having a (mostly) thread local clone of the regex for
// each thread.
//
// Threading
// =========
// I tried using `rayon`. It wasn't really faster than using Python threads and releasing the GIL.
// So goodbye `rayon`! Let thread count etc be in control of our Python users.
//
// Caching
// =======
// The reference tokeniser has an lru cache over the equivalent of `byte_pair_encode`.
// Originally, we had one too! Without it, we were only vaguely faster than Python.
// I used an RWLock to protect the cache. This didn't seem to hurt single threaded performance
// noticeably, but it did affect multi-threaded performance. Weirdly, it seemed to affect
// multi-threaded performance even when I only had readers (maybed I messed something up?).
// Anyway, I realised that we could get rid of the cache, if we treat the set of tokens as a cache!
// These are exactly the set or merges that are likely to be hot. And now we don't have to think
// about interior mutability, memory use, or cloning.
//
// Hashing
// =======
// We use FxHashMap instead of the standard HashMap. This is maybe like a 5-10% win?
// The current implementation ends up doing a lot of hashing of bytes. In theory, this could be made
// to be hashing of two-tuples of ints, which looks like it may also be a couple percent faster.

use std::num::NonZeroU64;
struct FakeThreadId(NonZeroU64);

fn hash_current_thread() -> usize {
    // It's easier to use unsafe than to use nightly. Rust has this nice u64 thread id counter
    // that works great for our use case of avoiding collisions in our array. Unfortunately,
    // it's private. However, there are only so many ways you can layout a u64, so just transmute
    // https://github.com/rust-lang/rust/issues/67939
    const _: [u8; 8] = [0; std::mem::size_of::<thread::ThreadId>()];
    const _: [u8; 8] = [0; std::mem::size_of::<FakeThreadId>()];
    let x =
        unsafe { std::mem::transmute::<thread::ThreadId, FakeThreadId>(thread::current().id()).0 };
    u64::from(x) as usize
}

const MAX_NUM_THREADS: usize = 128;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BPE {
    pub encoder: HashMap<Vec<u8>, usize>,
    pub special_tokens: HashMap<String, usize>,
    pub pattern: String,
}

impl BPE {
    fn _get_tl_regex(&self) -> Regex {
        let regex = Regex::new(self.pattern.as_str()).unwrap();
        let regex_tls: Vec<Regex> = (0..MAX_NUM_THREADS).map(|_| regex.clone()).collect();
        regex_tls[hash_current_thread() % MAX_NUM_THREADS].clone()
    }

    fn _get_tl_special_regex(&self) -> Regex {
        let special_regex = {
            let _parts = self
                .special_tokens
                .keys()
                .map(|s| fancy_regex::escape(s))
                .collect::<Vec<_>>();
            Regex::new(&_parts.join("|")).unwrap()
        };
        let special_regex_tls: Vec<Regex> = (0..MAX_NUM_THREADS)
            .map(|_| special_regex.clone())
            .collect();
        special_regex_tls[hash_current_thread() % MAX_NUM_THREADS].clone()
    }

    pub fn encode(&self, text: &str, allowed_special: &HashSet<&str>) -> Vec<usize> {
        let special_regex = self._get_tl_special_regex();
        let regex = self._get_tl_regex();
        let mut ret = vec![];

        let mut start = 0;
        let mut last_piece_token_len = 0;
        loop {
            let mut next_special;
            let mut start_find = start;
            loop {
                // Find the next allowed special token, if any
                next_special = special_regex.find_from_pos(text, start_find).unwrap();
                match next_special {
                    Some(m) => {
                        if allowed_special.contains(&text[m.start()..m.end()]) {
                            break;
                        }
                        start_find = m.start() + 1;
                    }
                    None => break,
                }
            }
            let end = next_special.map_or(text.len(), |m| m.start());

            for mat in regex.find_iter(&text[start..end]) {
                let piece = mat.unwrap().as_str().as_bytes();
                if let Some(token) = self.encoder.get(piece) {
                    last_piece_token_len = 1;
                    ret.push(*token);
                    continue;
                }
                let tokens = byte_pair_encode(piece, &self.encoder);
                last_piece_token_len = tokens.len();
                ret.extend(&tokens);
            }

            match next_special {
                // And here we push the special token
                Some(m) => {
                    let piece = m.as_str();
                    let token = self.special_tokens[piece];
                    ret.push(token);
                    start = m.end();
                    last_piece_token_len = 0;
                }
                None => break,
            }
        }
        ret
    }
    fn encode_para(&self, text: &str, allowed_special: HashSet<&str>) -> Vec<usize> {
        self.encode(text, &allowed_special)
    }
}
