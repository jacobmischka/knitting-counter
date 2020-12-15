#![no_std]
#![no_main]

use panic_halt as _;

use arduino_uno::{pac::EEPROM, prelude::*, Delay};
use atmega328p_hal::port::{mode, Pin};
use hd44780_driver::{self as lcd_driver, bus::I2CBus, HD44780};

mod display_props {
    pub const DISPLAY_ADDRESS: u8 = 0x27;

    pub const SCREEN_WIDTH: u8 = 16;
    pub const LINE_WIDTH: u8 = 64;
    pub const BOTTOM_RIGHT: u8 = LINE_WIDTH + SCREEN_WIDTH - 1;

    pub const COUNTER_START: u8 = 6;
    pub const SELECTED_COUNTER: u8 = BOTTOM_RIGHT;
    pub const DIRTY_STATE: u8 = LINE_WIDTH;
}

mod eeprom;

const STATE_STORAGE_ADDRESS: u16 = 1337;

use display_props::*;
use eeprom::Storable;

#[arduino_uno::entry]
fn main() -> ! {
    let peripherals = arduino_uno::Peripherals::take().unwrap();
    let mut pins = arduino_uno::Pins::new(peripherals.PORTB, peripherals.PORTC, peripherals.PORTD);

    let mut delay = Delay::new();

    let mut lcd = HD44780::new_i2c(
        arduino_uno::I2cMaster::new(
            peripherals.TWI,
            pins.a4.into_pull_up_input(&mut pins.ddr),
            pins.a5.into_pull_up_input(&mut pins.ddr),
            50000,
        ),
        DISPLAY_ADDRESS,
        &mut delay,
    )
    .unwrap();
    lcd.set_cursor_visibility(lcd_driver::Cursor::Invisible, &mut delay)
        .unwrap();

    let mut rows = [
        pins.d2.into_output(&mut pins.ddr).downgrade(),
        pins.d3.into_output(&mut pins.ddr).downgrade(),
        pins.d4.into_output(&mut pins.ddr).downgrade(),
        pins.d5.into_output(&mut pins.ddr).downgrade(),
    ];
    let cols = [
        pins.d6.into_pull_up_input(&pins.ddr).downgrade(),
        pins.d7.into_pull_up_input(&pins.ddr).downgrade(),
        pins.d8.into_pull_up_input(&pins.ddr).downgrade(),
        pins.d9.into_pull_up_input(&pins.ddr).downgrade(),
    ];

    let mut debouncer = Debouncer::new();

    let mut state = State::new();
    avr_device::interrupt::free(|_| {
        state = State::load(&EEPROM::ptr(), STATE_STORAGE_ADDRESS);
    });

    state.update_display(&mut lcd, &mut delay).unwrap();

    loop {
        if let Some(input) = debouncer.debounce(Input::from_pins(&mut rows, &cols)) {
            state.handle_input(input);
            state.update_display(&mut lcd, &mut delay).unwrap();
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Mode {
    Normal,
    Input,
    ConfirmReset,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Normal
    }
}

#[derive(Debug, Clone, Default)]
struct State {
    mode: Mode,
    counters: Counters,
    selected_counter: CounterSelection,
    digits_input: Option<DigitsInput>,
}

impl State {
    fn new() -> State {
        State {
            mode: Mode::Normal,
            counters: Default::default(),
            selected_counter: CounterSelection::A,
            digits_input: None,
        }
    }

    fn change_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Input => {
                let counter_val = self.get_counter().val();
                self.digits_input = Some(DigitsInput::new(counter_val));
            }
            Mode::Normal => {
                self.digits_input = None;
            }
            Mode::ConfirmReset => {}
        }
        self.mode = mode;
    }

    fn get_counter(&self) -> &Counter {
        self.counters.get(self.selected_counter)
    }

    fn get_counter_mut(&mut self) -> &mut Counter {
        self.counters.get_mut(self.selected_counter)
    }

    fn handle_input(&mut self, input: Input) {
        match self.mode {
            Mode::Normal => match input {
                Input::Num0 => {
                    self.change_mode(Mode::ConfirmReset);
                }
                Input::Num1 => {}
                Input::Num2 => {}
                Input::Num3 => {}
                Input::Num4 => {}
                Input::Num5 => {
                    avr_device::interrupt::free(|_| {
                        self.store(&EEPROM::ptr(), STATE_STORAGE_ADDRESS);
                    });
                }
                Input::Num6 => {}
                Input::Num7 => {}
                Input::Num8 => {}
                Input::Num9 => {}
                Input::Star => {
                    self.get_counter_mut().dec();
                }
                Input::Hash => {
                    self.get_counter_mut().inc();
                }
                Input::A | Input::B | Input::C | Input::D => {
                    let counter = CounterSelection::from_input(&input).unwrap();
                    if self.selected_counter == counter {
                        self.change_mode(Mode::Input);
                    } else {
                        self.selected_counter = counter;
                    }
                }
            },
            Mode::Input => {
                if let Some(digits) = self.digits_input.as_mut() {
                    match input {
                        Input::Star => {
                            if digits.index == 0 {
                                digits.index = BUF_LEN
                            }

                            digits.index -= 1;
                        }
                        Input::Hash => {
                            digits.index = (digits.index + 1) % BUF_LEN;
                        }
                        x => {
                            if let Some(digit) = x.to_digit() {
                                digits.add_digit(digit);
                            } else if let Some(counter_selection) = CounterSelection::from_input(&x)
                            {
                                if self.selected_counter == counter_selection {
                                    let new_val = digits.parse();
                                    self.get_counter_mut().set(new_val);
                                }

                                self.change_mode(Mode::Normal);
                            }
                        }
                    }
                }
            }
            Mode::ConfirmReset => match input {
                Input::Star => {
                    self.change_mode(Mode::Normal);
                }
                Input::Hash => {
                    self.get_counter_mut().reset();
                    self.change_mode(Mode::Normal);
                }
                _ => {}
            },
        }
    }

    fn update_display<I2C, D>(
        &self,
        lcd: &mut HD44780<I2CBus<I2C>>,
        delay: &mut D,
    ) -> lcd_driver::error::Result<()>
    where
        I2C: embedded_hal::blocking::i2c::Write,
        D: embedded_hal::blocking::delay::DelayUs<u16> + embedded_hal::blocking::delay::DelayMs<u8>,
    {
        lcd.clear(delay)?;
        lcd.reset(delay)?;
        match self.mode {
            Mode::Normal => {
                lcd.set_cursor_pos(COUNTER_START, delay)?;
                for c in &self.get_counter().to_digits().to_chars() {
                    if let Some(c) = c {
                        lcd.write_char(*c, delay)?;
                    } else {
                        lcd.shift_cursor(lcd_driver::Direction::Right, delay)?;
                    }
                }
            }
            Mode::Input => {
                lcd.set_cursor_pos(COUNTER_START, delay)?;
                if let Some(digits_input) = &self.digits_input {
                    for c in &digits_input.buf.to_chars() {
                        if let Some(c) = c {
                            lcd.write_char(*c, delay)?;
                        } else {
                            lcd.write_char('0', delay)?;
                        }
                    }
                    lcd.set_cursor_pos(
                        LINE_WIDTH + COUNTER_START + digits_input.index as u8,
                        delay,
                    )?;
                    lcd.write_char('^', delay)?;
                }
            }
            Mode::ConfirmReset => {
                lcd.write_str("Clear counter?", delay)?;
                lcd.set_cursor_pos(LINE_WIDTH, delay)?;
                lcd.write_str("*: No  #: Yes", delay)?;
            }
        }

        if !self.get_counter().is_dirty() {
            lcd.set_cursor_pos(DIRTY_STATE, delay).unwrap();
            lcd.write_str("Saved", delay).unwrap();
        }

        lcd.set_cursor_pos(SELECTED_COUNTER, delay)?;
        lcd.write_char(self.selected_counter.to_char(), delay)?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum CounterSelection {
    A,
    B,
    C,
    D,
}

impl Default for CounterSelection {
    fn default() -> Self {
        CounterSelection::A
    }
}

impl CounterSelection {
    fn to_char(&self) -> char {
        match self {
            CounterSelection::A => 'A',
            CounterSelection::B => 'B',
            CounterSelection::C => 'C',
            CounterSelection::D => 'D',
        }
    }

    fn from_input(input: &Input) -> Option<CounterSelection> {
        match input {
            Input::A => Some(CounterSelection::A),
            Input::B => Some(CounterSelection::B),
            Input::C => Some(CounterSelection::C),
            Input::D => Some(CounterSelection::D),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Counters {
    a: Counter,
    b: Counter,
    c: Counter,
    d: Counter,
}

impl Counters {
    fn get(&self, selection: CounterSelection) -> &Counter {
        match selection {
            CounterSelection::A => &self.a,
            CounterSelection::B => &self.b,
            CounterSelection::C => &self.c,
            CounterSelection::D => &self.d,
        }
    }

    fn get_mut(&mut self, selection: CounterSelection) -> &mut Counter {
        match selection {
            CounterSelection::A => &mut self.a,
            CounterSelection::B => &mut self.b,
            CounterSelection::C => &mut self.c,
            CounterSelection::D => &mut self.d,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Counter {
    val: u16,
    dirty: bool,
}

impl Counter {
    fn new(val: u16) -> Counter {
        Counter { val, dirty: false }
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn clean(&mut self) {
        self.dirty = false;
    }

    fn val(&self) -> u16 {
        self.val
    }

    fn inc(&mut self) {
        self.val = self.val.wrapping_add(1);
        self.dirty = true;
    }

    fn dec(&mut self) {
        self.val = self.val.wrapping_sub(1);
        self.dirty = true;
    }

    fn set(&mut self, val: u16) {
        self.val = val;
        self.dirty = true;
    }

    fn reset(&mut self) {
        self.val = 0;
        self.dirty = true;
    }

    fn to_digits(&self) -> Digits {
        Digits([
            ((self.val / 10000) % 10) as u8,
            ((self.val / 1000) % 10) as u8,
            ((self.val / 100) % 10) as u8,
            ((self.val / 10) % 10) as u8,
            (self.val % 10) as u8,
        ])
    }
}

struct Debouncer {
    last_input: Option<Input>,
}

impl Debouncer {
    fn new() -> Debouncer {
        Debouncer { last_input: None }
    }

    fn debounce(&mut self, input: Option<Input>) -> Option<Input> {
        if input != self.last_input {
            self.last_input = input;
            input
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Input {
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Star,
    Hash,
    A,
    B,
    C,
    D,
}

const NUM_ROWS: usize = 4;
const NUM_COLS: usize = 4;

impl Input {
    fn from_pins(
        rows: &mut [Pin<mode::Output>],
        cols: &[Pin<mode::Input<mode::PullUp>>],
    ) -> Option<Input> {
        for row in rows.iter_mut() {
            row.set_high().void_unwrap();
        }

        for i in 0..NUM_ROWS {
            rows[i].set_low().void_unwrap();
            for j in 0..NUM_COLS {
                if cols[j].is_low().void_unwrap() {
                    rows[i].set_high().void_unwrap();
                    match (i, j) {
                        (2, 3) => return Some(Input::Num1),
                        (2, 2) => return Some(Input::Num4),
                        (2, 1) => return Some(Input::Num7),
                        (2, 0) => return Some(Input::Star),
                        (3, 3) => return Some(Input::Num2),
                        (3, 2) => return Some(Input::Num5),
                        (3, 1) => return Some(Input::Num8),
                        (3, 0) => return Some(Input::Num0),
                        (1, 3) => return Some(Input::Num3),
                        (1, 2) => return Some(Input::Num6),
                        (1, 1) => return Some(Input::Num9),
                        (1, 0) => return Some(Input::Hash),
                        (0, 3) => return Some(Input::A),
                        (0, 2) => return Some(Input::B),
                        (0, 1) => return Some(Input::C),
                        (0, 0) => return Some(Input::D),
                        _ => panic!("Invalid key index ({}, {})", i, j),
                    }
                }
            }
            rows[i].set_high().void_unwrap();
        }

        None
    }

    #[allow(dead_code)]
    fn from_serial(byte: u8) -> Option<Input> {
        use Input::*;

        match byte {
            48 => Some(Num0),
            49 => Some(Num1),
            50 => Some(Num2),
            51 => Some(Num3),
            52 => Some(Num4),
            53 => Some(Num5),
            54 => Some(Num6),
            55 => Some(Num7),
            56 => Some(Num8),
            57 => Some(Num9),
            42 => Some(Star),
            35 => Some(Hash),
            97 => Some(A),
            98 => Some(B),
            99 => Some(C),
            100 => Some(D),
            _ => None,
        }
    }

    fn to_digit(&self) -> Option<u8> {
        match self {
            Input::Num0 => Some(0),
            Input::Num1 => Some(1),
            Input::Num2 => Some(2),
            Input::Num3 => Some(3),
            Input::Num4 => Some(4),
            Input::Num5 => Some(5),
            Input::Num6 => Some(6),
            Input::Num7 => Some(7),
            Input::Num8 => Some(8),
            Input::Num9 => Some(9),
            _ => None,
        }
    }
}

const BUF_LEN: usize = 5;

#[derive(Debug, Clone, Default)]
struct Digits([u8; BUF_LEN]);

impl Digits {
    fn from_u16(val: u16) -> Digits {
        let mut buf = [0u8; BUF_LEN];
        for i in 0..BUF_LEN {
            let index = BUF_LEN - 1 - i;
            buf[index] = ((val / 10u16.pow(i as _)) % 10) as u8;
        }

        Digits(buf)
    }

    fn to_u16(&self) -> u16 {
        self.0.iter().rev().enumerate().fold(0u16, |acc, (i, val)| {
            acc.saturating_add((*val as u16) * 10u16.pow(i as _))
        }) as _
    }

    fn to_chars(&self) -> [Option<char>; BUF_LEN] {
        let mut leading = true;
        let mut chars = [None; BUF_LEN];
        for (i, digit) in self.0.iter().enumerate() {
            if i < chars.len() - 1 && leading && *digit == 0 {
                continue;
            } else {
                leading = false;
                chars[i] = core::char::from_digit(*digit as _, 10);
            }
        }

        chars
    }
}

#[derive(Debug, Clone, Default)]
struct DigitsInput {
    buf: Digits,
    index: usize,
}

impl DigitsInput {
    fn new(val: u16) -> Self {
        DigitsInput {
            buf: Digits::from_u16(val),
            index: 0,
        }
    }

    fn add_digit(&mut self, digit: u8) {
        if self.index == 0 && digit > 6 {
            // Do nothing, out of bounds
        } else {
            self.buf.0[self.index] = digit;
            self.index = (self.index + 1) % BUF_LEN;
        }
    }

    fn parse(&self) -> u16 {
        self.buf.to_u16()
    }
}
