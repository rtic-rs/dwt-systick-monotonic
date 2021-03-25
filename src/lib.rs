//! # `Monotonic` implementation based on DWT and SysTick

#![no_std]

use cortex_m::peripheral::{syst::SystClkSource, DCB, DWT, SYST};
use rtic_monotonic::{
    embedded_time::{clock::Error, fraction::Fraction},
    Clock, Instant, Monotonic,
};

/// DWT and Systick combination implementing `embedded_time::Clock` and `rtic_monotonic::Monotonic`
///
/// The frequency of the DWT and SysTick is encoded using the parameter `FREQ`.
pub struct DwtSystick<const FREQ: u32> {
    dwt: DWT,
    systick: SYST,
}

impl<const FREQ: u32> DwtSystick<FREQ> {
    /// Enable the DWT and provide a new `Monotonic` based on DWT and SysTick.
    ///
    /// Note that the `sysclk` parameter should come from e.g. the HAL's clock generation function
    /// so the real speed and the declared speed can be compared.
    pub fn new(dcb: &mut DCB, dwt: DWT, systick: SYST, sysclk: u32) -> Self {
        assert!(FREQ == sysclk);

        dcb.enable_trace();
        DWT::unlock();

        unsafe { dwt.cyccnt.write(0) };

        // We do not start the counter here, it is started in `reset`.

        DwtSystick { dwt, systick }
    }
}

impl<const FREQ: u32> Clock for DwtSystick<FREQ> {
    type T = u32;

    const SCALING_FACTOR: Fraction = Fraction::new(1, FREQ);

    fn try_now(&self) -> Result<Instant<Self>, Error> {
        // The instant is always valid when the DWT is not reset
        Ok(Instant::new(self.dwt.cyccnt.read()))
    }
}

impl<const FREQ: u32> Monotonic for DwtSystick<FREQ> {
    const DISABLE_INTERRUPT_ON_EMPTY_QUEUE: bool = true;

    unsafe fn reset(&mut self) {
        self.dwt.enable_cycle_counter();

        self.systick.set_clock_source(SystClkSource::Core);
        self.systick.enable_counter();

        self.dwt.cyccnt.write(0);
    }

    fn set_compare(&mut self, val: &Instant<Self>) {
        // The input `val` is in the timer, but the SysTick is a down-counter.
        // We need to convert into its domain.
        let now: Instant<Self> = Instant::new(self.dwt.cyccnt.read());

        let max = 0x00ff_ffff;

        let dur = match val.checked_duration_since(&now) {
            None => 1, // In the past

            // ARM Architecture Reference Manual says:
            // "Setting SYST_RVR to zero has the effect of
            // disabling the SysTick counter independently
            // of the counter enable bit.", so the min is 1
            Some(x) => max.min(*x.integer()).max(1),
        };

        self.systick.set_reload(dur);
        self.systick.clear_current();
    }

    fn clear_compare_flag(&mut self) {
        // NOOP with SysTick interrupt
    }
}
