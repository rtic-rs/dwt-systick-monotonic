//! # `Monotonic` implementation based on DWT and SysTick

#![no_std]

use core::marker::PhantomData;
use cortex_m::peripheral::{syst::SystClkSource, DCB, DWT, SYST};
use rtic_monotonic::{
    embedded_time::{clock::Error, fraction::Fraction},
    Clock, Instant, Monotonic,
};
use typenum::Unsigned;
pub use typenum::consts;

/// DWT and Systick combination implementing `embedded_time::Clock` and `rtic_monotonic::Monotonic`
///
/// As `typenum` only has continuous numbers up to about 1000, this implementation is split into 3
/// generic parts, MHz, KHz and Hz, where each can be in the span [0, 1000]. The final speed of
/// this monotonic will be `MHz * 1_000_000 + KHz * 1_000 + Hz`.
pub struct DwtSystick<MHZ, KHZ, HZ> {
    dwt: DWT,
    systick: SYST,
    _mhz: PhantomData<MHZ>,
    _khz: PhantomData<KHZ>,
    _hz: PhantomData<HZ>,
}

impl<MHZ, KHZ, HZ> DwtSystick<MHZ, KHZ, HZ>
where
    MHZ: Unsigned,
    KHZ: Unsigned,
    HZ: Unsigned,
{
    /// Enable the DWT and provide a new `Monotonic` based on DWT and SysTick.
    ///
    /// Note that the `sysclk` parameter should come from e.g. the HAL's clock generation function
    /// so the real speed and the declared speed can be compared.
    pub fn new(dcb: &mut DCB, mut dwt: DWT, mut systick: SYST, sysclk: u32) -> Self {
        assert!(MHZ::U32 * 1_000_000 + KHZ::U32 * 1_000 + HZ::U32 == sysclk);
        assert!(KHZ::U32 < 1000);
        assert!(HZ::U32 < 1000);

        dcb.enable_trace();
        DWT::unlock();
        dwt.enable_cycle_counter();

        systick.set_clock_source(SystClkSource::Core);
        systick.enable_counter();

        unsafe { dwt.cyccnt.write(0) };

        DwtSystick {
            dwt,
            systick,
            _mhz: PhantomData,
            _khz: PhantomData,
            _hz: PhantomData,
        }
    }
}

impl<MHZ, KHZ, HZ> Clock for DwtSystick<MHZ, KHZ, HZ>
where
    MHZ: Unsigned,
    KHZ: Unsigned,
    HZ: Unsigned,
{
    type T = u32;

    const SCALING_FACTOR: Fraction =
        Fraction::new(1, MHZ::U32 * 1_000_000 + KHZ::U32 * 1_000 + HZ::U32);

    fn try_now(&self) -> Result<Instant<Self>, Error> {
        // The instant is always valid when the DWT is not reset
        Ok(Instant::new(self.dwt.cyccnt.read()))
    }
}

impl<MHZ, KHZ, HZ> Monotonic for DwtSystick<MHZ, KHZ, HZ>
where
    MHZ: Unsigned,
    KHZ: Unsigned,
    HZ: Unsigned,
{
    const DISABLE_INTERRUPT_ON_EMPTY_QUEUE: bool = true;

    fn reset(&mut self) {
        // Do not reset, as it is optional
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
