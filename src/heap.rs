use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

static SOFT: AtomicUsize = AtomicUsize::new(usize::MAX);

static HARD: AtomicUsize = AtomicUsize::new(usize::MAX);

static LIMIT_MB: AtomicUsize = AtomicUsize::new(0);

static ENDING: AtomicBool = AtomicBool::new(false);

static SOFT_HIT: AtomicBool = AtomicBool::new(false);

pub(crate) const SOFT_SHARE: f64 = 0.75;

pub const EXIT_MEMORY: i32 = 3;

pub(crate) fn set_limit_mb(mb: u64) {
    if mb == 0 {
        SOFT.store(usize::MAX, Relaxed);
        HARD.store(usize::MAX, Relaxed);
    } else {
        let hard = (mb as usize).saturating_mul(1 << 20);
        SOFT.store((hard as f64 * SOFT_SHARE) as usize, Relaxed);
        HARD.store(hard, Relaxed);
    }
    LIMIT_MB.store(mb as usize, Relaxed);
    SOFT_HIT.store(false, Relaxed);
}

pub(crate) fn live_mb() -> usize {
    LIVE.load(Relaxed) >> 20
}

#[inline]
pub(crate) fn over_soft() -> bool {
    LIVE.load(Relaxed) > SOFT.load(Relaxed) || SOFT_HIT.load(Relaxed)
}

pub(crate) fn fits(bytes: usize) -> bool {
    let ok = LIVE.load(Relaxed).saturating_add(bytes) <= SOFT.load(Relaxed);
    if !ok {
        SOFT_HIT.store(true, Relaxed);
    }
    ok
}

#[cold]
fn over_hard() -> ! {
    if !ENDING.swap(true, Relaxed) {
        let mut buf = [0u8; 160];
        let mut n = 0;
        let mut put = |s: &[u8]| {
            let k = s.len().min(buf.len() - n);
            buf[n..n + k].copy_from_slice(&s[..k]);
            n += k;
        };
        put(b"keplearn: the memory limit (--max-memory ");
        let mut digits = [0u8; 20];
        let mut d = digits.len();
        let mut v = LIMIT_MB.load(Relaxed);
        loop {
            d -= 1;
            digits[d] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        put(&digits[d..]);
        put(
            b" MB) was reached before a result could be written; raise it or use fewer columns or rows\n",
        );
        let _ = std::io::stderr().write_all(&buf[..n]);
    }
    std::process::exit(EXIT_MEMORY)
}

#[inline]
fn grow(by: usize) {
    let now = LIVE.fetch_add(by, Relaxed) + by;
    if now > HARD.load(Relaxed) && !ENDING.load(Relaxed) {
        over_hard();
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        System.alloc(l)
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        System.alloc_zeroed(l)
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        System.dealloc(p, l)
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        if new_size > l.size() {
            grow(new_size - l.size());
        } else {
            LIVE.fetch_sub(l.size() - new_size, Relaxed);
        }
        System.realloc(p, l, new_size)
    }
}
