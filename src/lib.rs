//! # `Monotonic` implementation based on DWT cycle counter and SysTick

#![no_std]

use cortex_m::peripheral::{syst::SystClkSource, DCB, DWT, SYST};
pub use fugit;
#[cfg(not(feature = "extend"))]
pub use fugit::ExtU32;
#[cfg(feature = "extend")]
pub use fugit::ExtU64;
use rtic_monotonic::Monotonic;

/// DWT and Systick combination implementing `rtic_monotonic::Monotonic`.
///
/// This implementation is tickless. It does not use periodic interrupts to count
/// "ticks" (like `systick-monotonic`) but only to obtain actual desired compare
/// events and to manage overflows.
///
/// The frequency of the DWT and SysTick is encoded using the parameter `TIMER_HZ`.
/// They must be equal.
///
/// Note that the SysTick interrupt must not be disabled longer than half the
/// cycle counter overflow period (typically a couple seconds).
///
/// When the `extend` feature is enabled, the cycle counter width is extended to
/// `u64` by detecting and counting overflows.
pub struct DwtSystick<const TIMER_HZ: u32> {
    dwt: DWT,
    systick: SYST,
    #[cfg(feature = "extend")]
    last: u64,
}

impl<const TIMER_HZ: u32> DwtSystick<TIMER_HZ> {
    /// Enable the DWT and provide a new `Monotonic` based on DWT and SysTick.
    ///
    /// Note that the `sysclk` parameter should come from e.g. the HAL's clock generation function
    /// so the speed calculated at runtime and the declared speed (generic parameter
    /// `TIMER_HZ`) can be compared.
    #[inline(always)]
    pub fn new(dcb: &mut DCB, mut dwt: DWT, mut systick: SYST, sysclk: u32) -> Self {
        assert!(TIMER_HZ == sysclk);

        dcb.enable_trace();
        DWT::unlock();
        assert!(DWT::has_cycle_counter());

        // Clear the cycle counter here so scheduling (`set_compare()`) before `reset()`
        // works correctly.
        dwt.set_cycle_count(0);

        systick.set_clock_source(SystClkSource::Core);

        // We do not start the counters here but in `reset()`.

        DwtSystick {
            dwt,
            systick,
            #[cfg(feature = "extend")]
            last: 0,
        }
    }
}

impl<const TIMER_HZ: u32> Monotonic for DwtSystick<TIMER_HZ> {
    cfg_if::cfg_if! {
        if #[cfg(not(feature = "extend"))] {
            const DISABLE_INTERRUPT_ON_EMPTY_QUEUE: bool = true;

            type Instant = fugit::TimerInstantU32<TIMER_HZ>;
            type Duration = fugit::TimerDurationU32<TIMER_HZ>;

            #[inline(always)]
            fn now(&mut self) -> Self::Instant {
                Self::Instant::from_ticks(DWT::cycle_count())
            }
        } else {
            // Need to detect and track overflows.
            const DISABLE_INTERRUPT_ON_EMPTY_QUEUE: bool = false;

            type Instant = fugit::TimerInstantU64<TIMER_HZ>;
            type Duration = fugit::TimerDurationU64<TIMER_HZ>;

            #[inline(always)]
            fn now(&mut self) -> Self::Instant {
                let mut high = (self.last >> 32) as u32;
                let low = self.last as u32;
                let now = DWT::cycle_count();

                // Detect CYCCNT overflow
                if now < low {
                    high = high.wrapping_add(1);
                }
                self.last = ((high as u64) << 32) | (now as u64);

                Self::Instant::from_ticks(self.last)
            }
        }
    }

    unsafe fn reset(&mut self) {
        self.systick.enable_counter();

        // Enable and reset the cycle counter to locate the epoch.
        self.dwt.enable_cycle_counter();
        self.dwt.set_cycle_count(0);
    }

    fn set_compare(&mut self, val: Self::Instant) {
        // The input `val` refers to the cycle counter value (up-counter)
        // but the SysTick is a down-counter with interrupt on zero.
        let reload = val
            .checked_duration_since(self.now())
            // Minimum reload value if `val` is in the past
            .map_or(0, |duration| duration.ticks())
            // CYCCNT and SysTick have the same clock, no
            // ticks conversion necessary, only clamping:
            //
            // ARM Architecture Reference Manual says:
            // "Setting SYST_RVR to zero has the effect of
            // disabling the SysTick counter independently
            // of the counter enable bit.", so the min is 1
            .max(1)
            // SysTick is a 24 bit counter.
            .min(0xff_ffff) as u32;

        self.systick.set_reload(reload);
        // Also clear the current counter. That doesn't cause a SysTick
        // interrupt and loads the reload value on the next cycle.
        self.systick.clear_current();
    }

    #[inline(always)]
    fn zero() -> Self::Instant {
        Self::Instant::from_ticks(0)
    }

    #[inline(always)]
    fn clear_compare_flag(&mut self) {
        // SysTick exceptions don't need flag clearing.
        //
        // But when extending the cycle counter range, we need to keep
        // the interrupts enabled to detect overflow.
        // This function is always called in the interrupt handler early.
        // Reset a maximum reload value in case `set_compare()` is not called.
        // Otherwise the interrupt would keep firing at the previous set
        // interval.
        #[cfg(feature = "extend")]
        {
            self.systick.set_reload(0xff_ffff);
            self.systick.clear_current();
        }
    }

    #[cfg(feature = "extend")]
    fn on_interrupt(&mut self) {
        // Ensure `now()` is called regularly to track overflows.
        // Since SysTick is narrower than CYCCNT, this is sufficient.
        self.now();
    }
}
