//! Bussola core: heading smoothing + cardinal labels.
//!
//! Size-oriented design (the whole shared library is ~7 KB per ABI):
//! * `no_std`: `std`'s panic machinery (backtrace, addr2line, gimli,
//!   rustc_demangle) is what makes a trivial Rust `cdylib` weigh ~300 KB.
//!   Dropping it cuts the library by ~98%.
//! * No heap: instances live in a fixed static pool, so no allocator is
//!   required. The pool is tiny (16 slots) and handles are bounds-checked on
//!   every call, so a stale or forged handle can never dereference garbage.
//! * Zero dependencies except `libm` for `floorf`/`fmodf`.
//! * Angles are `f32` degrees in `[0, 360)`, north = 0, clockwise.

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

use core::cell::UnsafeCell;
use core::f32::consts::PI;
use core::sync::atomic::{AtomicBool, Ordering};
use libm::{floorf, fmodf};

/// 16-point compass rose as NUL-terminated ASCII, so the pointer handed out by
/// `bussola_cardinal` is directly readable as a C string by Swift.
const CARDINALS_C: [&[u8]; 16] = [
    b"N\0",
    b"NNE\0",
    b"NE\0",
    b"ENE\0",
    b"E\0",
    b"ESE\0",
    b"SE\0",
    b"SSE\0",
    b"S\0",
    b"SSW\0",
    b"SW\0",
    b"WSW\0",
    b"W\0",
    b"WNW\0",
    b"NW\0",
    b"NNW\0",
];

/// Smoothing engine for a magnetic/true heading in degrees.
pub struct Compass {
    /// Slot reservation flag; also the first field so a `*mut Compass` points
    /// exactly at the slot inside the pool.
    used: AtomicBool,
    /// Smoothed heading in `[0, 360)`.
    heading: f32,
    /// Exponential smoothing factor in `[0.02, 1.0]`; 1.0 = no smoothing.
    alpha: f32,
    /// Number of samples folded in so far.
    samples: u32,
    /// Constant correction added to every sample (magnetic declination).
    offset: f32,
}

/// Number of simultaneously alive instances. The apps use one; the tests use a
/// few in parallel.
const POOL_SIZE: usize = 16;

/// One pool bucket. `UnsafeCell` is what makes interior mutability legal here:
/// the `used` flag reserves the contents for a single owner.
struct Slot(UnsafeCell<Compass>);

// SAFETY: mutation is only reachable through `acquire`, which wins the
// `used` compare-and-swap first, so at most one thread can hold a given slot.
unsafe impl Sync for Slot {}

const EMPTY_SLOT: Compass = Compass {
    used: AtomicBool::new(false),
    heading: 0.0,
    alpha: 0.18,
    samples: 0,
    offset: 0.0,
};

static POOL: [Slot; POOL_SIZE] = [
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
    Slot(UnsafeCell::new(EMPTY_SLOT)),
];

impl Compass {
    /// `alpha` is clamped to `[0.02, 1.0]`; `0.18` works well for a 30-60 Hz
    /// sensor stream (responsive but steady needle). NaN falls back to 0.18.
    fn init(&mut self, alpha: f32) {
        self.alpha = if alpha.is_finite() {
            alpha.clamp(0.02, 1.0)
        } else {
            0.18
        };
        self.heading = 0.0;
        self.samples = 0;
        self.offset = 0.0;
    }

    /// Folds a raw reading (degrees) into the smoothed heading.
    pub fn update(&mut self, raw: f32) -> f32 {
        if !raw.is_finite() {
            return self.heading;
        }
        let sample = wrap360(raw + self.offset);
        if self.samples == 0 {
            self.heading = sample;
        } else {
            // Shortest path across the 0/360 seam.
            let delta = shortest_delta(sample, self.heading);
            self.heading = wrap360(self.heading + self.alpha * delta);
        }
        self.samples = self.samples.saturating_add(1);
        self.heading
    }

    /// Current smoothed heading in `[0, 360)`.
    pub fn heading(&self) -> f32 {
        self.heading
    }

    /// Configured smoothing factor.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Sets a constant correction applied to incoming samples.
    pub fn set_offset(&mut self, deg: f32) {
        if deg.is_finite() {
            self.offset = deg;
        }
    }

    /// Clears the smoothing state, keeping alpha and offset.
    pub fn reset(&mut self) {
        self.heading = 0.0;
        self.samples = 0;
    }
}

/// Reserves a pool slot and initializes it, or `None` when the pool is full.
fn acquire(alpha: f32) -> Option<&'static mut Compass> {
    for slot in POOL.iter() {
        let inner = slot.0.get();
        // SAFETY: `inner` points into the static pool, so reading the flag is
        // valid. Only the thread that wins the CAS below mutates the contents.
        let used = unsafe { &(*inner).used };
        if used
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // SAFETY: this thread now owns the slot exclusively until the
            // matching `used.store(false)` in `bussola_free`.
            let compass = unsafe { &mut *inner };
            compass.init(alpha);
            return Some(compass);
        }
    }
    None
}

/// Validates a handle and returns the reserved slot.
///
/// Bounds and alignment are checked against the pool, so any pointer that did
/// not come from `acquire` (stale, forged, misaligned) is rejected instead of
/// being dereferenced.
fn resolve<'a>(ptr: *const Compass) -> Option<&'a mut Compass> {
    if ptr.is_null() {
        return None;
    }
    let base = POOL.as_ptr() as usize;
    let stride = core::mem::size_of::<Compass>();
    let end = base + stride * POOL_SIZE;
    let addr = ptr as usize;
    if !(base..end).contains(&addr) {
        return None;
    }
    let offset = addr - base;
    if offset % stride != 0 {
        return None;
    }
    let index = offset / stride;
    let inner = POOL[index].0.get();
    // SAFETY: `inner` belongs to a pool slot; owners are serialized by `used`.
    let compass = unsafe { &mut *inner };
    if !compass.used.load(Ordering::Acquire) {
        return None;
    }
    Some(compass)
}

/// Normalizes any angle in degrees into `[0, 360)`.
pub fn wrap360(deg: f32) -> f32 {
    if !deg.is_finite() {
        return 0.0;
    }
    let mut d = fmodf(deg, 360.0);
    if d < 0.0 {
        d += 360.0;
    }
    // Rounding can push the addition back onto 360.0.
    if d >= 360.0 {
        d -= 360.0;
    }
    d
}

/// Signed shortest rotation from `from` to `to`, in `(-180, 180]`.
pub fn shortest_delta(to: f32, from: f32) -> f32 {
    let mut d = fmodf(to - from, 360.0);
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    d
}

/// Index `0..=15` of the 16-point rose for a heading in degrees.
pub fn cardinal_index(deg: f32) -> usize {
    // 22.5° per sector, shifted half a sector so "N" is centred on 0°.
    let shifted = wrap360(deg) + 11.25;
    let idx = floorf(shifted / 22.5) as i32;
    idx.rem_euclid(16) as usize
}

/// Cardinal label ("N", "NNE", ...) for a heading in degrees.
pub fn cardinal(deg: f32) -> &'static str {
    let bytes = &CARDINALS_C[cardinal_index(deg)];
    core::str::from_utf8(&bytes[..bytes.len() - 1]).unwrap_or("N")
}

/// Converts an `atan2`-style heading in radians to compass degrees.
///
/// Android's rotation vector reports 0 rad when the device points south, so the
/// convention is `degrees = 180 + rad * 180 / PI`.
pub fn from_radians(rad: f32) -> f32 {
    if !rad.is_finite() {
        return 0.0;
    }
    wrap360(180.0 + rad * 180.0 / PI)
}

// ---------------------------------------------------------------------------
// C ABI (Swift via the bridging header, plus the tests)
// ---------------------------------------------------------------------------

/// Reserves a compass. Returns NULL when the pool is exhausted.
/// Release it with `bussola_free`.
#[no_mangle]
pub extern "C" fn bussola_new(alpha: f32) -> *mut Compass {
    match acquire(alpha) {
        Some(c) => c as *mut Compass,
        None => core::ptr::null_mut(),
    }
}

/// Releases a compass created by `bussola_new`. NULL, stale and forged handles
/// are ignored.
///
/// # Safety
/// The handle must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn bussola_free(ptr: *mut Compass) {
    if let Some(c) = resolve(ptr as *const Compass) {
        c.used.store(false, Ordering::Release);
    }
}

/// Feeds a raw heading in degrees, returns the smoothed heading.
///
/// # Safety
/// `ptr` must be NULL or a handle from `bussola_new`.
#[no_mangle]
pub unsafe extern "C" fn bussola_update(ptr: *mut Compass, raw_deg: f32) -> f32 {
    match resolve(ptr as *const Compass) {
        Some(c) => c.update(raw_deg),
        None => 0.0,
    }
}

/// Feeds a raw `atan2` heading in radians, returns the smoothed heading.
///
/// # Safety
/// `ptr` must be NULL or a handle from `bussola_new`.
#[no_mangle]
pub unsafe extern "C" fn bussola_update_rad(ptr: *mut Compass, raw_rad: f32) -> f32 {
    match resolve(ptr as *const Compass) {
        Some(c) => c.update(from_radians(raw_rad)),
        None => 0.0,
    }
}

/// Smoothed heading in degrees.
///
/// # Safety
/// `ptr` must be NULL or a handle from `bussola_new`.
#[no_mangle]
pub unsafe extern "C" fn bussola_heading(ptr: *const Compass) -> f32 {
    match resolve(ptr) {
        Some(c) => c.heading(),
        None => 0.0,
    }
}

/// Sets the calibration offset in degrees.
///
/// # Safety
/// `ptr` must be NULL or a handle from `bussola_new`.
#[no_mangle]
pub unsafe extern "C" fn bussola_set_offset(ptr: *mut Compass, deg: f32) {
    if let Some(c) = resolve(ptr as *const Compass) {
        c.set_offset(deg);
    }
}

/// Clears the filter state, keeping alpha and offset.
///
/// # Safety
/// `ptr` must be NULL or a handle from `bussola_new`.
#[no_mangle]
pub unsafe extern "C" fn bussola_reset(ptr: *mut Compass) {
    if let Some(c) = resolve(ptr as *const Compass) {
        c.reset();
    }
}

/// NUL-terminated cardinal label for a heading. Static, never freed.
#[no_mangle]
pub extern "C" fn bussola_cardinal(deg: f32) -> *const u8 {
    CARDINALS_C[cardinal_index(deg)].as_ptr()
}

/// Number of rose sectors (always 16).
#[no_mangle]
pub extern "C" fn bussola_cardinal_count() -> i32 {
    CARDINALS_C.len() as i32
}

/// NUL-terminated library version string.
#[no_mangle]
pub extern "C" fn bussola_version() -> *const u8 {
    b"1.0.0\0".as_ptr()
}

// ---------------------------------------------------------------------------
// Panic handler: with `no_std` we must provide one. Aborting is the right
// behaviour for a library called across an FFI boundary anyway.
//
// Apple platforms need an explicit `-lSystem`: rustc links a `cdylib` with
// `-nodefaultlibs`, so `abort` and `dyld_stub_binder` would stay unresolved.
// ---------------------------------------------------------------------------
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    #[cfg(target_vendor = "apple")]
    #[link(name = "System")]
    extern "C" {
        fn abort() -> !;
    }

    #[cfg(not(target_vendor = "apple"))]
    extern "C" {
        fn abort() -> !;
    }

    // SAFETY: `abort` never returns.
    unsafe { abort() }
}

// ---------------------------------------------------------------------------
// JNI entry points (Android). Only primitives cross the boundary: the Kotlin
// side keeps the handle as a `long` and maps the sector index to a label.
// ---------------------------------------------------------------------------
#[cfg(target_os = "android")]
mod jni {
    use super::{
        bussola_free, bussola_heading, bussola_new, bussola_reset, bussola_set_offset,
        bussola_update, bussola_update_rad, cardinal_index, Compass,
    };

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeNew(
        _env: *mut u8,
        _class: *mut u8,
        alpha: f32,
    ) -> i64 {
        bussola_new(alpha) as i64
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeFree(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
    ) {
        // SAFETY: handle comes from nativeNew, or is 0 (rejected by resolve).
        unsafe { bussola_free(handle as *mut Compass) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeUpdate(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
        raw_deg: f32,
    ) -> f32 {
        unsafe { bussola_update(handle as *mut Compass, raw_deg) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeUpdateRad(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
        raw_rad: f32,
    ) -> f32 {
        unsafe { bussola_update_rad(handle as *mut Compass, raw_rad) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeHeading(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
    ) -> f32 {
        unsafe { bussola_heading(handle as *const Compass) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeSetOffset(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
        deg: f32,
    ) {
        unsafe { bussola_set_offset(handle as *mut Compass, deg) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeReset(
        _env: *mut u8,
        _class: *mut u8,
        handle: i64,
    ) {
        unsafe { bussola_reset(handle as *mut Compass) }
    }

    /// Sector index `0..=15` for a heading, mapped to a label in Kotlin.
    #[no_mangle]
    pub extern "system" fn Java_com_bussola_core_Core_nativeCardinalIndex(
        _env: *mut u8,
        _class: *mut u8,
        raw_deg: f32,
    ) -> i32 {
        cardinal_index(raw_deg) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ops::{Deref, DerefMut};
    use std::sync::{Mutex, MutexGuard};

    /// The pool is a small process-wide static, so tests that allocate from it
    /// must not step on each other. `pool_exhaustion_and_reuse` deliberately
    /// drains it, which would starve every other test running in parallel.
    static POOL_LOCK: Mutex<()> = Mutex::new(());

    fn pool_lock() -> MutexGuard<'static, ()> {
        // A panicking test would poison the mutex; the data (the pool) is still
        // consistent because each Guard frees its slot on unwind.
        POOL_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Owns one pool slot and releases it on drop, so the tests can run in
    /// parallel without exhausting the (deliberately small) static pool.
    struct Guard(*mut Compass);

    impl Deref for Guard {
        type Target = Compass;
        fn deref(&self) -> &Compass {
            // SAFETY: the slot stays reserved for the guard's lifetime.
            unsafe { &*self.0 }
        }
    }

    impl DerefMut for Guard {
        fn deref_mut(&mut self) -> &mut Compass {
            // SAFETY: the guard has exclusive ownership of the slot.
            unsafe { &mut *self.0 }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            // SAFETY: the pointer came from bussola_new and is freed once.
            unsafe { bussola_free(self.0) }
        }
    }

    fn compass(alpha: f32) -> Guard {
        let ptr = bussola_new(alpha);
        assert!(!ptr.is_null(), "pool exhausted");
        Guard(ptr)
    }

    #[test]
    fn wrap_normalizes_into_range() {
        assert_eq!(wrap360(0.0), 0.0);
        assert_eq!(wrap360(360.0), 0.0);
        assert_eq!(wrap360(-90.0), 270.0);
        assert_eq!(wrap360(720.5), 0.5);
        assert!((wrap360(-0.5) - 359.5).abs() < 1e-4);
        assert_eq!(wrap360(f32::NAN), 0.0);
        assert_eq!(wrap360(f32::INFINITY), 0.0);
    }

    #[test]
    fn shortest_delta_crosses_north() {
        assert!((shortest_delta(10.0, 350.0) - 20.0).abs() < 1e-4);
        assert!((shortest_delta(350.0, 10.0) + 20.0).abs() < 1e-4);
        assert!((shortest_delta(180.0, 0.0) - 180.0).abs() < 1e-4);
        // Exactly 180° apart is a tie: either sign is correct.
        assert!((shortest_delta(0.0, 180.0).abs() - 180.0).abs() < 1e-4);
        assert!(shortest_delta(90.0, 90.0).abs() < 1e-6);
    }

    #[test]
    fn cardinal_labels_are_correct_at_boundaries() {
        assert_eq!(cardinal(0.0), "N");
        assert_eq!(cardinal(11.24), "N");
        assert_eq!(cardinal(11.26), "NNE");
        assert_eq!(cardinal(45.0), "NE");
        assert_eq!(cardinal(90.0), "E");
        assert_eq!(cardinal(180.0), "S");
        assert_eq!(cardinal(270.0), "W");
        assert_eq!(cardinal(348.75), "N");
        assert_eq!(cardinal(359.99), "N");
        assert_eq!(cardinal(-45.0), "NW");
        assert_eq!(cardinal(720.0), "N");
    }

    #[test]
    fn every_label_is_covered_once() {
        let mut seen: Vec<&str> = (0..16).map(|i| cardinal(i as f32 * 22.5)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 16);
        assert!(seen.contains(&"N"));
        assert!(seen.contains(&"NNW"));
    }

    #[test]
    fn cardinal_index_stays_in_bounds() {
        for i in -2000..2000 {
            let idx = cardinal_index(i as f32 * 0.37);
            assert!(idx < 16, "index {idx} out of range for {i}");
        }
    }

    #[test]
    fn first_sample_is_used_directly() {
        let _pool = pool_lock();
        let mut c = compass(0.2);
        assert!((c.update(123.0) - 123.0).abs() < 1e-4);
        assert!((c.heading() - 123.0).abs() < 1e-4);
    }

    #[test]
    fn smoothing_converges_without_overshoot() {
        let _pool = pool_lock();
        let mut c = compass(0.2);
        c.update(0.0);
        let mut last = 0.0;
        for _ in 0..200 {
            last = c.update(90.0);
        }
        assert!((last - 90.0).abs() < 0.5, "converged to {last}");
        assert!(last <= 90.0 + 1e-3, "overshot to {last}");
    }

    #[test]
    fn smoothing_takes_short_path_across_north() {
        let _pool = pool_lock();
        let mut c = compass(0.5);
        c.update(350.0);
        let after = c.update(10.0);
        assert!(
            shortest_delta(after, 350.0).abs() < 30.0,
            "went the long way: {after}"
        );
    }

    #[test]
    fn noise_is_attenuated() {
        let _pool = pool_lock();
        let mut c = compass(0.1);
        c.update(100.0);
        for _ in 0..10 {
            c.update(120.0);
            c.update(80.0);
        }
        let h = c.heading();
        assert!((h - 100.0).abs() < 5.0, "not attenuated: {h}");
    }

    #[test]
    fn offset_shifts_heading() {
        let _pool = pool_lock();
        let mut c = compass(1.0);
        c.set_offset(10.0);
        assert!((c.update(0.0) - 10.0).abs() < 1e-4);
        c.set_offset(f32::NAN); // ignored
        assert!(c.update(350.0).abs() < 1e-4);
    }

    #[test]
    fn reset_keeps_configuration() {
        let _pool = pool_lock();
        let mut c = compass(1.0);
        c.set_offset(5.0);
        c.update(100.0);
        c.reset();
        assert_eq!(c.heading(), 0.0);
        assert!((c.update(20.0) - 25.0).abs() < 1e-4);
    }

    #[test]
    fn alpha_is_clamped() {
        let _pool = pool_lock();
        assert!(compass(0.0).alpha() >= 0.02);
        assert_eq!(compass(50.0).alpha(), 1.0);
        assert!((compass(f32::NAN).alpha() - 0.18).abs() < 1e-6);
    }

    #[test]
    fn radians_conversion_handles_atan2_output() {
        assert!((from_radians(0.0) - 180.0).abs() < 1e-3);
        assert!((from_radians(PI / 2.0) - 270.0).abs() < 1e-3);
        assert!(from_radians(-PI).abs() < 1e-3);
        assert_eq!(from_radians(f32::NAN), 0.0);
    }

    #[test]
    fn non_finite_samples_are_ignored() {
        let _pool = pool_lock();
        let mut c = compass(1.0);
        c.update(30.0);
        assert!((c.update(f32::NAN) - 30.0).abs() < 1e-4);
        assert!((c.update(f32::INFINITY) - 30.0).abs() < 1e-4);
    }

    #[test]
    fn pool_handles_are_validated() {
        let _pool = pool_lock();
        unsafe {
            let p = bussola_new(0.5);
            assert!(!p.is_null());
            assert!((bussola_update(p, 45.0) - 45.0).abs() < 1e-4);
            // Radian entry point: 0 rad maps to 180°, but the filter starts at
            // the previous sample, so only check the direction of travel.
            let after_rad = bussola_update_rad(p, 0.0);
            assert!(
                (after_rad - 45.0).abs() > 1.0 && after_rad > 45.0 && after_rad < 180.0,
                "radian sample not folded in: {after_rad}"
            );
            bussola_set_offset(p, 0.0);
            bussola_reset(p);
            assert_eq!(bussola_heading(p), 0.0);
            // With the filter reset the next sample is taken verbatim.
            assert!((bussola_update_rad(p, 0.0) - 180.0).abs() < 1e-3);

            // Forged / misaligned pointers must be rejected, not dereferenced.
            assert_eq!(bussola_heading(std::ptr::null()), 0.0);
            assert_eq!(bussola_update(std::ptr::null_mut(), 10.0), 0.0);
            let bogus = (p as usize).wrapping_add(1) as *mut Compass;
            assert_eq!(bussola_update(bogus, 10.0), 0.0);
            let far = 0xdead_beef_usize as *mut Compass;
            assert_eq!(bussola_heading(far), 0.0);
            bussola_free(far);
            bussola_free(std::ptr::null_mut());

            // Freed handle is no longer usable.
            bussola_free(p);
            assert_eq!(bussola_heading(p), 0.0);
        }
    }

    #[test]
    fn pool_exhaustion_and_reuse() {
        let _pool = pool_lock();
        // Other tests may hold slots in parallel, so drain the pool instead of
        // assuming it starts empty: keep allocating until it refuses.
        let mut handles = Vec::new();
        loop {
            let p = bussola_new(0.5);
            if p.is_null() {
                break;
            }
            handles.push(p);
            assert!(handles.len() <= POOL_SIZE, "pool handed out too many slots");
        }
        assert!(!handles.is_empty(), "pool was already empty at start");
        // Full now: freeing one slot makes it reusable.
        let freed = handles.pop().expect("one handle");
        // SAFETY: `freed` came from bussola_new and is not used afterwards.
        unsafe { bussola_free(freed) };
        let again = bussola_new(0.5);
        assert!(!again.is_null(), "freed slot was not reused");
        // SAFETY: `again` came from bussola_new and is freed once.
        unsafe { bussola_free(again) };
        for p in handles {
            // SAFETY: every handle came from bussola_new and is freed once.
            unsafe { bussola_free(p) };
        }
    }

    #[test]
    fn ffi_strings_are_nul_terminated() {
        let _pool = pool_lock();
        unsafe {
            let ptr = bussola_cardinal(90.0);
            let s = std::ffi::CStr::from_ptr(ptr as *const i8)
                .to_str()
                .expect("utf8");
            assert_eq!(s, "E");
            let v = std::ffi::CStr::from_ptr(bussola_version() as *const i8)
                .to_str()
                .expect("utf8");
            assert_eq!(v, "1.0.0");
            assert_eq!(bussola_cardinal_count(), 16);
        }
    }
}
