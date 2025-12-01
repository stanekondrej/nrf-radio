#![no_std]
#![no_main]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]

//! An abstraction over the nRFxxxx SoCs' radio peripheral
//!
//! This library tries not to be opinionated, but in cases where a choice has to be made between
//! a theoretically attainable performance improvement and safety, I usually chose safety.
//!
//! (To give a concrete example of this, the radio mode conversion functions block the thread while
//! they wait for the radio to switch tx/rx modes. Expressing the in-the-middle state in the type
//! system would be very complicated, so just blocking while the transition is en-course is
//! something that would probably save me from shooting myself in the foot)

use core::marker::PhantomData;

/// The error type of the library
#[derive(PartialEq, Debug, thiserror::Error)]
pub enum Error {
    /// The radio is in an unknown state. Tread lightly - you are on very thin ice.
    #[error("the radio is in an unknown state")]
    UnknownState,

    /// Something that you tried to do couldn't be done fast enough.
    #[error("the operation could not be completed in the specified timeframe")]
    TimedOut,

    /// A value that you tried to convert to another falls out of range of the given container.
    #[error("the value is out of bounds of the requested container")]
    ValueOutOfBounds,
}

/// Result type returned by functions
pub type Result<T> = core::result::Result<T, Error>;

/// The main RADIO abstraction
pub struct Radio<T> {
    radio: nrf51_pac::RADIO,
    _marker: PhantomData<T>,
}

/// TX mode
pub struct Transmitter;
/// RX mode
pub struct Receiver;

/// Enabled mode
pub struct Enabled<T>(PhantomData<T>);
/// Disabled mode
pub struct Disabled;

/// Converts the radio from one state to another
macro_rules! convert_radio {
    ($radio:expr, $into_state:ident) => {
        crate::Radio::<$into_state> {
            radio: $radio,
            _marker: PhantomData,
        }
    };
}

impl crate::Radio<()> {
    /// Constructs a new [`crate::Radio`], setting the radio state to disabled
    pub fn new(radio: nrf51_pac::RADIO) -> crate::Radio<Disabled> {
        radio.tasks_disable.write(|w| unsafe { w.bits(1) });

        let radio = convert_radio!(radio, Disabled);
        radio.wait_for_state(State::DISABLED);

        radio
    }
}

/// Implement the `Self::into_receiver()` associated function
macro_rules! impl_into_rx {
    () => {
        /// Switch the radio into receiver mode
        pub fn into_receiver(self) -> $crate::Radio<Enabled<Receiver>> {
            self.radio.tasks_disable.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::DISABLED);

            self.radio.tasks_rxen.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::RX_IDLE);

            $crate::Radio {
                radio: self.radio,
                _marker: PhantomData,
            }
        }
    };
}

/// Implement the `Self::into_transmitter()` associated function
macro_rules! impl_into_tx {
    () => {
        /// Switch the radio into transmitter mode
        pub fn into_transmitter(self) -> crate::Radio<Enabled<Transmitter>> {
            self.radio.tasks_disable.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::DISABLED);

            self.radio.tasks_txen.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::TX_IDLE);

            crate::Radio {
                radio: self.radio,
                _marker: PhantomData,
            }
        }
    };
}

/// Implement the `Self::disable()` associated function
macro_rules! impl_disable {
    () => {
        /// Disable the radio
        pub fn disable(self) -> crate::Radio<Disabled> {
            self.radio.tasks_disable.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::DISABLED);

            convert_radio!(self.radio, Disabled)
        }
    };
}

impl crate::Radio<Disabled> {
    impl_into_rx!();
    impl_into_tx!();
}

/// The frequency on which the radio operates in MHz. (For example, 2400 means 2,4 GHz here)
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct Frequency(u32);

impl core::convert::TryFrom<u32> for Frequency {
    type Error = crate::Error;

    fn try_from(value: u32) -> core::result::Result<Self, Self::Error> {
        if !(2400..=2500).contains(&value) {
            return Err(crate::Error::ValueOutOfBounds);
        }

        Ok(Self(value - 2400))
    }
}

pub use nrf51_pac::radio::mode::MODE_A as Mode;

pub use nrf51_pac::radio::pcnf1::ENDIAN_A as Endianness;

impl<T> crate::Radio<Enabled<T>> {
    impl_disable!();

    /// Set the frequency on which the radio operates
    pub fn set_frequency(&self, freq: Frequency) -> &Self {
        self.radio.frequency.write(|w| unsafe { w.bits(freq.0) });

        self
    }

    /// Sets the mode for the radio
    pub fn set_mode(&self, mode: Mode) -> &Self {
        self.radio.mode.write(|w| w.mode().variant(mode));

        self
    }

    /// Set the order of bits of the S0, LENGTH, S1, and PAYLOAD fields.
    pub fn set_endianness(&self, endian: Endianness) -> &Self {
        self.radio.pcnf1.write(|w| w.endian().variant(endian));

        self
    }
}

pub use nrf51_pac::radio::txpower::TXPOWER_A as TxPower;

/// Logical address. Can be used for reception or transmission
#[allow(missing_docs)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[repr(u32)]
pub enum Address {
    A = 1 << 0,
    B = 1 << 1,
    C = 1 << 2,
    D = 1 << 3,
    E = 1 << 4,
    F = 1 << 5,
    G = 1 << 6,
    H = 1 << 7,
}

impl crate::Radio<Enabled<Transmitter>> {
    impl_into_rx!();

    /// Set the transmission power
    pub fn set_tx_power(&self, tx_power: TxPower) -> &Self {
        self.radio.txpower.write(|w| w.txpower().variant(tx_power));

        self
    }

    /// Sets the logical address to transmit from
    pub fn set_tx_address(&self, address: Address) -> &Self {
        self.radio
            .txaddress
            .write(|w| unsafe { w.bits(address as u32) });

        self
    }
}

impl crate::Radio<Enabled<Receiver>> {
    impl_into_tx!();

    /// Sets the logical addresses on which the radio should listen for packets
    pub fn set_rx_addresses(&self, addresses: &[Address]) -> &Self {
        // calculate the resulting register value so that only one write is needed
        let a = addresses.iter().fold(0, |acc, x| acc | *x as u32);

        self.radio.rxaddresses.write(|w| unsafe { w.bits(a) });

        self
    }
}

pub use nrf51_pac::radio::state::STATE_A as State;

impl<T> crate::Radio<T> {
    /// Get the state which the radio is currently in
    pub fn get_state(&self) -> crate::Result<State> {
        self.radio
            .state
            .read()
            .state()
            .variant()
            .ok_or(crate::Error::UnknownState)
    }

    /// Wait until radio goes into state. Break out of the function after `cycles`, returning [`crate::Error::TimedOut`].
    ///
    /// # Safety
    ///
    /// This function panics if the radio is in an invalid state
    pub fn wait_for_state_cycles(&self, state: State, cycles: usize) -> crate::Result<()> {
        for _ in 0..cycles {
            if self.get_state().expect("The radio is in an invalid state") == state {
                return Ok(());
            }
        }

        Err(crate::Error::TimedOut)
    }

    /// Wait until radio goes into `state`. Be careful not to deadlock your program this way.
    ///
    /// # Safety
    ///
    /// This function panics if the radio is in an invalid state
    pub fn wait_for_state(&self, state: State) {
        while self.wait_for_state_cycles(state, usize::MAX) == Err(crate::Error::TimedOut) {
            core::hint::spin_loop();
        }
    }
}
