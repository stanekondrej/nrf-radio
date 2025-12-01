#![no_std]
#![no_main]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]

//! An abstraction over the nRFxxxx SoCs' radio peripheral

use core::marker::PhantomData;

/// The error type of the library
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    /// The radio is in an unknown state. Tread lightly - you are on very thin ice.
    #[error("the radio is in an unknown state")]
    UnknownState,

    /// Something that you tried to do couldn't be done fast enough.
    #[error("the operation could not be completed in the specified timeframe")]
    TimedOut,
}

/// Result type returned by functions
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "nrf51")]
#[allow(clippy::upper_case_acronyms)]
type RADIO = nrf51_pac::RADIO;

/// The main RADIO abstraction
pub struct Radio<T> {
    #[cfg(feature = "nrf51")]
    radio: RADIO,
    _marker: PhantomData<T>,
}

/// TX mode
pub struct Transmitter;
/// RX mode
pub struct Receiver;
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
    pub fn new(radio: RADIO) -> crate::Radio<Disabled> {
        radio.tasks_disable.write(|w| unsafe { w.bits(1) });

        let radio = convert_radio!(radio, Disabled);
        radio.wait_for_state(State::DISABLED);

        radio
    }
}

macro_rules! impl_into_rx {
    () => {
        /// Switch the radio into receiver mode
        pub fn into_receiver(self) -> crate::Radio<Receiver> {
            self.radio.tasks_rxen.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::RX);

            convert_radio!(self.radio, Receiver)
        }
    };
}

macro_rules! impl_into_tx {
    () => {
        /// Switch the radio into transmitter mode
        pub fn into_transmitter(self) -> crate::Radio<Transmitter> {
            self.radio.tasks_txen.write(|w| unsafe { w.bits(1) });
            self.wait_for_state(State::TX);

            convert_radio!(self.radio, Transmitter)
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

impl crate::Radio<Transmitter> {
    impl_disable!();
    impl_into_rx!();
}

impl crate::Radio<Receiver> {
    impl_disable!();
    impl_into_tx!();
}

#[cfg(feature = "nrf51")]
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
