/// Based on information from
/// https://cse537-2011.blogspot.com/2011/02/accessing-internal-eeprom-on-atmega328p.html
use arduino_uno::pac::eeprom::{eecr::EEPM_A, RegisterBlock};

use super::{Counter, Counters, State};

pub trait Storage {
    fn write_byte(&self, addr: u16, data: u8);
    fn write_bytes(&self, addr: u16, data: &[u8]) {
        for (addr_offset, byte) in data.iter().enumerate() {
            self.write_byte(addr + addr_offset as u16, *byte);
        }
    }

    fn read_byte(&self, addr: u16) -> u8;
    fn read_bytes(&self, addr: u16, len: usize, buf: &mut [u8]) {
        for i in 0..len {
            buf[i] = self.read_byte(addr + i as u16);
        }
    }
}

impl Storage for *const RegisterBlock {
    fn write_byte(&self, addr: u16, data: u8) {
        if self.read_byte(addr) == data {
            return;
        }

        let block = unsafe { &**self };
        while block.eecr.read().eepe().bit_is_set() {
            // Wait out any previous write
        }

        block.eear.write(|w| unsafe { w.bits(addr) });
        block.eedr.write(|w| unsafe { w.bits(data) });
        block.eecr.write(|w| {
            w.eepm()
                .variant(EEPM_A::VAL_0X00) // Erase and write
                .eempe()
                .set_bit()
        });
        block.eecr.write(|w| w.eepe().set_bit());
    }

    fn read_byte(&self, addr: u16) -> u8 {
        let block = unsafe { &**self };
        while block.eecr.read().eepe().bit_is_set() {
            // Wait out any previous write
        }

        block.eear.write(|w| unsafe { w.bits(addr) });
        block.eecr.write(|w| w.eere().set_bit());
        block.eedr.read().bits()
    }
}

pub trait Storable {
    fn store<S: Storage>(&self, storage: &S, addr: u16);
    fn load<S: Storage>(storage: &S, addr: u16) -> Self;
}

impl Storable for Counter {
    fn store<S: Storage>(&self, storage: &S, addr: u16) {
        storage.write_bytes(addr, &self.val.to_le_bytes());
    }
    fn load<S: Storage>(storage: &S, addr: u16) -> Self {
        let mut buf = [0; 2];
        storage.read_bytes(addr, 2, &mut buf);
        Counter::new(u16::from_le_bytes(buf))
    }
}

impl Storable for Counters {
    fn store<S: Storage>(&self, storage: &S, addr: u16) {
        self.a.store(storage, addr);
        self.b.store(storage, addr + 2);
        self.c.store(storage, addr + 4);
        self.d.store(storage, addr + 6);
    }

    fn load<S: Storage>(storage: &S, addr: u16) -> Self {
        Counters {
            a: Counter::load(storage, addr),
            b: Counter::load(storage, addr + 2),
            c: Counter::load(storage, addr + 4),
            d: Counter::load(storage, addr + 6),
        }
    }
}

impl Storable for State {
    fn store<S: Storage>(&self, storage: &S, addr: u16) {
        self.counters.store(storage, addr);
    }

    fn load<S: Storage>(storage: &S, addr: u16) -> Self {
        State {
            counters: Counters::load(storage, addr),
            ..Default::default()
        }
    }
}
