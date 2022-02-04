//! # `Monotonic` implementation based on DWT and SysTick

#![no_std]

use cortex_m::peripheral::{syst::SystClkSource, DCB, DWT, SYST};
pub use fugit::{self, ExtU32, ExtU64};
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
    pub fn new(dcb: &mut DCB, dwt: DWT, systick: SYST, sysclk: u32) -> Self {
        assert!(TIMER_HZ == sysclk);

        dcb.enable_trace();
        DWT::unlock();

        unsafe { dwt.cyccnt.write(0) };

        // We do not start the counter here, it is started in `reset`.

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
                Self::Instant::from_ticks(self.dwt.cyccnt.read())
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
                let now = self.dwt.cyccnt.read();

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
        self.dwt.enable_cycle_counter();

        self.systick.set_clock_source(SystClkSource::Core);
        self.systick.enable_counter();

        self.dwt.cyccnt.write(0);
    }

    fn set_compare(&mut self, val: Self::Instant) {
        // The input `val` is in the timer, but the SysTick is a down-counter.
        // We need to convert into its domain.
        let now = self.now();

        let reload = val
            .checked_duration_since(now)
            // Minimum reload value if `val` is in the past
            .map_or(0, |t| t.ticks())
            // ARM Architecture Reference Manual says:
            // "Setting SYST_RVR to zero has the effect of
            // disabling the SysTick counter independently
            // of the counter enable bit.", so the min is 1
            .max(1)
            // SysTick is a 24 bit counter.
            .min(0xff_ffff) as u32;

        self.systick.set_reload(reload);
        self.systick.clear_current();
    }

    #[inline(always)]
    fn zero() -> Self::Instant {
        Self::Instant::from_ticks(0)
    }

    #[inline(always)]
    fn clear_compare_flag(&mut self) {
        // Set a long reload in case `set_compare()` is not called again.
        #[cfg(feature = "extend")]
        self.systick.set_reload(0xff_ffff);
        #[cfg(feature = "extend")]
        self.systick.clear_current();
    }

    #[cfg(feature = "extend")]
    fn on_interrupt(&mut self) {
        // Ensure `now()` is called regularly to track overflows.
        // Since SysTick is narrower than CYCCNT, this is sufficient.
        self.now();
    }
}
