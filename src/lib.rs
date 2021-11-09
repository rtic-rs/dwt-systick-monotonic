//! # `Monotonic` implementation based on DWT and SysTick

#![no_std]

use cortex_m::peripheral::{syst::SystClkSource, DCB, DWT, SYST};
pub use fugit::{self, ExtU32};
use rtic_monotonic::Monotonic;

/// DWT and Systick combination implementing `embedded_time::Clock` and `rtic_monotonic::Monotonic`
///
/// The frequency of the DWT and SysTick is encoded using the parameter `TIMER_HZ`.
pub struct DwtSystick<const TIMER_HZ: u32> {
    dwt: DWT,
    systick: SYST,
}

impl<const TIMER_HZ: u32> DwtSystick<TIMER_HZ> {
    /// Enable the DWT and provide a new `Monotonic` based on DWT and SysTick.
    ///
    /// Note that the `sysclk` parameter should come from e.g. the HAL's clock generation function
    /// so the real speed and the declared speed can be compared.
    #[inline(always)]
    pub fn new(dcb: &mut DCB, dwt: DWT, systick: SYST, sysclk: u32) -> Self {
        assert!(TIMER_HZ == sysclk);

        dcb.enable_trace();
        DWT::unlock();

        unsafe { dwt.cyccnt.write(0) };

        // We do not start the counter here, it is started in `reset`.

        DwtSystick { dwt, systick }
    }
}

impl<const TIMER_HZ: u32> Monotonic for DwtSystick<TIMER_HZ> {
    const DISABLE_INTERRUPT_ON_EMPTY_QUEUE: bool = true;

    type Instant = fugit::TimerInstantU32<TIMER_HZ>;
    type Duration = fugit::TimerDurationU32<TIMER_HZ>;

    #[inline(always)]
    fn now(&mut self) -> Self::Instant {
        Self::Instant::from_ticks(self.dwt.cyccnt.read())
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

        let max = 0x00ff_ffff;

        let dur = match val.checked_duration_since(now) {
            None => 1, // In the past

            // ARM Architecture Reference Manual says:
            // "Setting SYST_RVR to zero has the effect of
            // disabling the SysTick counter independently
            // of the counter enable bit.", so the min is 1
            Some(x) => max.min(x.ticks()).max(1),
        };

        self.systick.set_reload(dur);
        self.systick.clear_current();
    }

    #[inline(always)]
    fn zero() -> Self::Instant {
        Self::Instant::from_ticks(0)
    }

    #[inline(always)]
    fn clear_compare_flag(&mut self) {
        // NOOP with SysTick interrupt
    }
}
