#![no_std]
#![no_main]

use panic_halt as _;

use arduino_uno::{
    hal::port::{mode::Output, portb::PB5},
    prelude::*,
};
use atmega328p_hal::port::{mode, Pin};
use ufmt::{derive::uDebug, uwriteln};

#[arduino_uno::entry]
fn main() -> ! {
    let peripherals = arduino_uno::Peripherals::take().unwrap();

    let mut pins = arduino_uno::Pins::new(peripherals.PORTB, peripherals.PORTC, peripherals.PORTD);
    let mut led = pins.d13.into_output(&mut pins.ddr);

    let mut serial = arduino_uno::Serial::new(
        peripherals.USART0,
        pins.d0,
        pins.d1.into_output(&mut pins.ddr),
        57600.into_baudrate(),
    );

    uwriteln!(&mut serial, "Hello, Arduino!").void_unwrap();

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

    loop {
        if let Some(input) = debouncer.debounce(Input::from_pins(&mut rows, &cols)) {
            uwriteln!(&mut serial, "Input: {:?}", input).void_unwrap();
            led.set_high().void_unwrap();
        } else {
            led.set_low().void_unwrap();
        }
    }
}

/*
#[arduino_uno::entry]
fn main() -> ! {
    let peripherals = arduino_uno::Peripherals::take().unwrap();

    let mut pins = arduino_uno::Pins::new(peripherals.PORTB, peripherals.PORTC, peripherals.PORTD);

    let mut serial = arduino_uno::Serial::new(
        peripherals.USART0,
        pins.d0,
        pins.d1.into_output(&mut pins.ddr),
        57600.into_baudrate(),
    );

    let mut state = State::new();

    uwriteln!(&mut serial, "State: {:#?}\r", &state).void_unwrap();

    loop {
        let b = nb::block!(serial.read()).void_unwrap();
        let input = Input::from_serial(b);

        if let Some(input) = input {
            state.handle_input(input);

            uwriteln!(&mut serial, "State: {:#?}\r", &state).void_unwrap();
        } else {
            uwriteln!(&mut serial, "Invalid input\r").void_unwrap();
        }
    }
}
*/

fn sutter_blink(led: &mut PB5<Output>, times: usize) {
    for i in (0..times).map(|i| i * 10) {
        led.toggle().void_unwrap();
        arduino_uno::delay_ms(i as u16);
    }
}

#[derive(Debug, Copy, Clone, uDebug)]
enum Mode {
    Normal,
    Input,
}

#[derive(Debug, Clone, uDebug)]
struct State {
    mode: Mode,
    counters: Counters,
    selected_counter: CounterSelection,
    digits: Option<Digits>,
}

impl State {
    fn new() -> State {
        State {
            mode: Mode::Normal,
            counters: Default::default(),
            selected_counter: CounterSelection::A,
            digits: None,
        }
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
                    self.get_counter_mut().reset();
                }
                Input::Num1 => {}
                Input::Num2 => {}
                Input::Num3 => {}
                Input::Num4 => {}
                Input::Num5 => {}
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
                Input::A => {
                    self.selected_counter = CounterSelection::A;
                }
                Input::B => {
                    self.selected_counter = CounterSelection::B;
                }
                Input::C => {
                    self.selected_counter = CounterSelection::C;
                }
                Input::D => {
                    self.selected_counter = CounterSelection::D;
                }
            },
            Mode::Input => {
                // TODO

                if let Some(digits) = &mut self.digits {
                    if let Some(digit) = input.to_digit() {
                        digits.add_digit(digit);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone, uDebug)]
enum CounterSelection {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Default, uDebug)]
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

#[derive(Debug, Clone, Default, uDebug)]
struct Counter {
    val: usize,
}

impl Counter {
    fn new() -> Counter {
        Counter::default()
    }

    fn inc(&mut self) {
        self.val += 1;
    }

    fn dec(&mut self) {
        if self.val > 0 {
            self.val -= 1;
        }
    }

    fn reset(&mut self) {
        self.val = 0;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, uDebug)]
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

const BUF_LEN: usize = 6;

#[derive(Debug, Clone, uDebug)]
struct Digits {
    buf: [u8; BUF_LEN],
    index: usize,
}

impl Digits {
    fn add_digit(&mut self, digit: u8) {
        if self.index < BUF_LEN {
            self.buf[self.index] = digit;
            self.index += 1;
        }
    }

    fn parse(&self) -> usize {
        self.buf
            .iter()
            .rev()
            .enumerate()
            .fold(0u16, |acc, (i, val)| {
                acc + (*val as u16) * 10u16.pow(i as _)
            }) as _
    }
}
