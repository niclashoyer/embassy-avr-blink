use core::cell::RefCell;

use arduino_hal::clock::Clock;
use atomic_polyfill::AtomicU64;
use critical_section::{CriticalSection, Mutex};

use super::AlarmState;

// Possible Values:
//
// ╔═══════════╦══════════════╦═══════════════════╗
// ║ PRESCALER ║ TIMER_COUNTS ║ Overflow Interval ║
// ╠═══════════╬══════════════╬═══════════════════╣
// ║        64 ║          250 ║              1 ms ║
// ║       256 ║          125 ║              2 ms ║
// ║       256 ║          250 ║              4 ms ║
// ║      1024 ║          125 ║              8 ms ║
// ║      1024 ║          250 ║             16 ms ║
// ╚═══════════╩══════════════╩═══════════════════╝
//const PRESCALER: u32 = 64;
//const TIMER_COUNTS: u32 = 249;
const PRESCALER: u32 = 64;
const TIMER_COUNTS: u32 = 24;

pub const ALARM_COUNT: usize = 1;

pub struct EmbassyTimer {
    pub(crate) alarms: Mutex<[AlarmState; ALARM_COUNT]>,
    pub(crate) timer: AtomicU64,
}

const ALARM_STATE_NONE: AlarmState = AlarmState::new();

embassy_time::time_driver_impl!(static DRIVER: EmbassyTimer = EmbassyTimer {
    alarms: Mutex::new([ALARM_STATE_NONE; ALARM_COUNT]),
    timer: AtomicU64::new(0),
});

#[avr_device::interrupt(atmega328p)]
#[allow(non_snake_case)]
fn TIMER0_COMPA() {
    DRIVER.on_interrupt();
}

impl EmbassyTimer {
    pub(crate) fn now() -> u64 {
        DRIVER.timer.load(atomic_polyfill::Ordering::Relaxed)
    }

    pub(crate) fn trigger_alarm(&self, n: usize, cs: CriticalSection) {
        let alarm = &self.alarms.borrow(cs)[n];
        // safety:
        // - we can ignore the possiblity of `f` being unset (null) because of the
        //   safety contract of `allocate_alarm`.
        // - other than that we only store valid function pointers into alarm.callback
        let f: fn(*mut ()) = unsafe { core::mem::transmute(alarm.callback.get()) };
        f(alarm.ctx.get());
    }

    pub fn on_interrupt(&self) {
        let ts = self.timer.fetch_add(1, atomic_polyfill::Ordering::Relaxed);

        for n in 0..ALARM_COUNT {
            critical_section::with(|cs| {
                let alarm_state = unsafe { self.alarms.borrow(cs).get_unchecked(n) };
                if ts < alarm_state.timestamp.get() {
                    self.trigger_alarm(n, cs);
                }
            });
        }
    }

    pub fn init(tc0: arduino_hal::pac::TC0) {
        tc0.tccr0a.write(|w| w.wgm0().ctc());
        tc0.ocr0a.write(|w| w.bits(TIMER_COUNTS as u8));
        tc0.tccr0b.write(|w| match PRESCALER {
            8 => w.cs0().prescale_8(),
            64 => w.cs0().prescale_64(),
            256 => w.cs0().prescale_256(),
            1024 => w.cs0().prescale_1024(),
            _ => panic!(),
        });
        tc0.timsk0.write(|w| w.ocie0a().set_bit());
    }

    pub(crate) fn set_alarm(
        &self,
        alarm: embassy_time::driver::AlarmHandle,
        timestamp: u64,
    ) -> bool {
        critical_section::with(|cs| {
            let now = Self::now();
            let alarm_state = unsafe { self.alarms.borrow(cs).get_unchecked(alarm.id() as usize) };
            if timestamp < now {
                alarm_state.timestamp.set(u64::MAX);
                return false;
            }
            alarm_state.timestamp.set(timestamp);

            true
        })
    }
}
