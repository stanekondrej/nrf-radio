#![no_std]
#![no_main]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used)]

//! An abstraction over the nRFxxxx SoCs' radio peripheral
//!
//! This library tries not to be opinionated where it doesn't have to be, but in cases where a
//! choice has to be made between a theoretically attainable performance improvement and safety, I
//! usually chose safety.
//!
//! (To give a concrete example of this, the radio mode conversion functions block the thread while
//! they wait for the radio to switch tx/rx modes. Expressing the in-the-middle state in the type
//! system would be very complicated, so just blocking while the transition is en-course is
//! something that would probably save me from shooting myself in the foot)
//!
//! # Performance
//!
//! Speed isn't the main focus of this interface - interrupts generally aren't used; everything is
//! awaited in a spinlock.

pub mod packet;
mod reg_access;

use core::marker::PhantomData;

/// The error type of the library
#[derive(PartialEq, Debug, thiserror::Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    ///
    /// If are constructing [`crate::Radio`] for the first time in your program, **you most
    /// definitely want to use [`Self::new_zeroed`] instead.** This function will not overwrite any
    /// config registers, so things will look weird if you haven't written to said registers before
    /// this function.
    ///
    /// You will probably want to use this function if you've already constructed [`crate::Radio`]
    /// before, but need to construct a new struct for some reason.
    pub fn new(radio: nrf51_pac::RADIO) -> crate::Radio<Disabled> {
        reg_access::disable(&radio);

        let radio = convert_radio!(radio, Disabled);
        radio.wait_for_state(State::DISABLED);

        radio
    }

    /// Like [`Self::new`], but sets many registers to zero, so that packets don't exhibit possibly
    /// unexpected behaviour
    pub fn new_zeroed(radio: nrf51_pac::RADIO) -> crate::Radio<Disabled> {
        let radio = Self::new(radio);

        let r = &radio.radio;

        reg_access::write_tx_address(r, 0);
        reg_access::write_rx_address(r, 0);

        radio
    }
}

/// Implement the `Self::into_receiver()` associated function
macro_rules! impl_into_rx {
    () => {
        /// Switch the radio into receiver mode
        pub fn into_receiver(self) -> $crate::Radio<Enabled<Receiver>> {
            reg_access::disable(&self.radio);
            self.wait_for_state(State::DISABLED);

            reg_access::enable_rx(&self.radio);
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
            reg_access::disable(&self.radio);
            self.wait_for_state(State::DISABLED);

            reg_access::enable_tx(&self.radio);
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
            reg_access::disable(&self.radio);
            self.wait_for_state(State::DISABLED);

            convert_radio!(self.radio, Disabled)
        }
    };
}

impl crate::Radio<Disabled> {
    impl_into_rx!();
    impl_into_tx!();
}

/// The offset (in MHz) from which the frequency is calculated
pub const FREQUENCY_OFFSET: u32 = 2400;

/// The frequency, specified as `2400 MHz + f [MHz]`
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Frequency(u32);

impl Frequency {
    fn from_reg_value(reg_value: u32) -> Option<Self> {
        match reg_value {
            0..=100 => Some(Self(reg_value)),
            _ => None,
        }
    }

    /// Validates that the frequency is in range, returning `None` if not
    pub fn from_mhz(mhz: u32) -> Option<Self> {
        let t = mhz - FREQUENCY_OFFSET;

        match t {
            0..=100 => Some(Self(t)),
            _ => None,
        }
    }

    /// Convert the inner frequency representation to MHz
    pub fn as_mhz(&self) -> u32 {
        self.0 + FREQUENCY_OFFSET
    }
}

pub use nrf51_pac::radio::mode::MODE_A as Mode;

pub use nrf51_pac::radio::pcnf1::ENDIAN_A as Endianness;

/// Interrupts that can be invoked by the radio
#[repr(u32)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Interrupt {
    /// RADIO has ramped up and is ready to be started
    Ready = 1 << 0,
    /// Address sent or received
    Address = 1 << 2,
    /// Packet payload sent or received
    Payload = 1 << 3,
    /// Packet sent or received
    End = 1 << 4,
    /// RADIO has been disabled
    Disabled = 1 << 5,
    /// A device address match occurred on the last received packet
    DevMatch = 1 << 6,
    /// No device address match occurred on the last received packet
    DevMiss = 1 << 7,
    /// Sampling of receive signal strength complete. A new RSSI sample is ready for readout from
    /// the RSSISAMPLE register
    RSSIEnd = 1 << 8,
    /// Bit counter reached bit count value specified in the BCC register
    BCMatch = 1 << 9,
}

/// A value that can be XOR'ed in a certain way in order to get more information
pub type BitMask<T> = T;

// TODO: some of these functions should maybe be moved to the `crate::Radio<T>` impl, as they aren't
// specific to the enabled state
impl<T> crate::Radio<Enabled<T>> {
    impl_disable!();

    /// Set the frequency on which the radio operates
    pub fn set_frequency(&self, freq: Frequency) -> &Self {
        reg_access::write_frequency(&self.radio, freq.0);

        self
    }

    /// Get the frequency that the radio is set to
    pub fn frequency(&self) -> Frequency {
        let f = reg_access::read_frequency(&self.radio);

        #[cfg(feature = "defmt")]
        defmt::println!("{}", f);
        Frequency::from_reg_value(f)
            .expect("invalid frequency in register; if you see this it is probably a bug")
    }

    /// Sets the mode for the radio
    pub fn set_mode(&self, mode: Mode) -> &Self {
        reg_access::write_mode(&self.radio, mode);

        self
    }

    /// Get the mode that the radio is set to
    pub fn mode(&self) -> Mode {
        reg_access::read_mode(&self.radio)
    }

    /// Set the order of bits of the `S0`, `LENGTH`, `S1`, and `PAYLOAD` fields.
    pub fn set_endianness(&self, endian: Endianness) -> &Self {
        reg_access::set_endianness(&self.radio, endian);

        self
    }

    /// Get the endianness that the radio is set to
    pub fn endianness(&self) -> Endianness {
        reg_access::get_endianness(&self.radio)
    }

    /// Get the `LENGTH` field length
    pub fn lf_len(&self) -> LengthFieldLength {
        reg_access::read_lf_len(&self.radio)
    }

    pub fn set_lf_len(&self, len: LengthFieldLength) {
        reg_access::write_lf_len(&self.radio, len);
    }

    /// Get the `S0` field length
    pub fn s0_len(&self) -> S0FieldLength {
        reg_access::read_s0_len(&self.radio)
    }

    /// Get the `S1` field length
    pub fn s1_len(&self) -> S1FieldLength {
        reg_access::read_s1_len(&self.radio)
    }

    /// Returns a mask on which you can try bit ANDing to check the raised interrupts
    pub fn read_interrupts(&self) -> BitMask<u32> {
        reg_access::read_interrupts(&self.radio)
    }

    /// Set the pointer to a packet which should be sent, or set the pointer to a packet buffer to
    /// which a received packet should be written
    ///
    /// # Safety
    ///
    /// The pointee MUST be a buffer or otherwise writable memory location.
    ///
    /// The pointer MUST be aligned and MUST NOT be dangling, otherwise **UB will be invoked.**
    unsafe fn set_packet_ptr<P>(&self, ptr: *mut P) -> &Self {
        reg_access::set_packet_ptr(&self.radio, ptr);

        self
    }
}

pub use nrf51_pac::radio::txpower::TXPOWER_A as TxPower;

/// Logical address. Can be used for reception or transmission
///
/// # Safety
///
/// In case it wasn't obvious, it isn't really possible to [`core::mem::transmute`] from the bit
/// representation of an address to this one. TX and RX logical addresses are represented
/// differently.
#[allow(missing_docs)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, strum::FromRepr)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Address {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
    F = 5,
    G = 6,
    H = 7,
}

/// Calculates the shift needed to reach the first `true` bit in the bitmask
fn count_mask_shift(mut mask: BitMask<u32>) -> u8 {
    let mut index = 0;
    while (mask & 1) == 0 {
        mask >>= 1;
        index += 1;
    }

    index
}

impl Address {
    fn into_rx_address(self) -> u8 {
        1 << self as u8
    }

    fn into_tx_address(self) -> u8 {
        self as u8
    }

    fn from_tx_address(addr: u8) -> Option<Self> {
        Self::from_repr(addr)
    }
}

impl crate::Radio<Enabled<Transmitter>> {
    impl_into_rx!();

    /// Set the transmission power
    pub fn set_tx_power(&self, tx_power: TxPower) -> &Self {
        reg_access::write_tx_power(&self.radio, tx_power);

        self
    }

    /// Get the transmission power the radio is set to. Returns `None` if the register value is
    /// invalid
    pub fn tx_power(&self) -> Option<TxPower> {
        reg_access::read_tx_power(&self.radio)
    }

    /// Sets the logical address to transmit from
    pub fn set_tx_address(&self, address: Address) -> &Self {
        reg_access::write_tx_address(&self.radio, address as u32);

        self
    }

    /// Disable all addresses for sending
    pub fn disable_all_tx_addresses(&self) -> &Self {
        reg_access::write_tx_address(&self.radio, 0);

        self
    }

    /// Get the transmission address the radio is currently set to
    pub fn tx_address(&self) -> Address {
        let a = reg_access::read_tx_address(&self.radio);

        Address::from_tx_address(a).expect("invalid tx address; if you're seeing this it's a bug")
    }
}

impl crate::Radio<Enabled<Receiver>> {
    impl_into_tx!();

    /// Enable a logical address for receiving. Multiple can be enabled at once by making use of
    /// [`Self::enable_rx_addresses`], which acts as a replacement for bitwise OR.
    pub fn enable_rx_address(&self, address: Address) -> &Self {
        let reg_value = reg_access::read_rx_address(&self.radio);
        let new_reg_value = reg_value | address as u8;

        reg_access::write_rx_address(&self.radio, new_reg_value);

        self
    }

    /// Disable a logical address for receiving. Multiple can be disabled at once by making use of
    /// [`Self::disable_rx_addresses`].
    pub fn disable_rx_address(&self, address: Address) -> &Self {
        let reg_value = reg_access::read_rx_address(&self.radio);
        let mask = !(address as u8);
        let new_reg_value = reg_value & mask;

        reg_access::write_rx_address(&self.radio, new_reg_value);

        self
    }

    /// Sets the logical addresses on which the radio should listen for packets
    pub fn enable_rx_addresses(&self, addresses: &[Address]) -> &Self {
        if addresses.is_empty() {
            return self;
        }

        // calculate the resulting register value so that only one write is needed
        let mask = addresses.iter().fold(0, |acc, x| acc | *x as u8);

        let reg_value = reg_access::read_rx_address(&self.radio);
        let new_reg_value = reg_value | mask;

        reg_access::write_rx_address(&self.radio, new_reg_value);

        self
    }

    /// Unsets the logical addresses on which the radio should listen for packets
    pub fn disable_rx_addresses(&self, addresses: &[Address]) -> &Self {
        if addresses.is_empty() {
            return self;
        }

        // calculate the resulting register value so that only one write is needed
        let mask = addresses.iter().fold(0, |acc, x| acc | *x as u8);

        let reg_value = reg_access::read_rx_address(&self.radio);
        let new_reg_value = reg_value & !(mask);
        reg_access::write_rx_address(&self.radio, new_reg_value);

        self
    }

    /// Disable all addresses for receiving
    pub fn disable_all_rx_addresses(&self) -> &Self {
        reg_access::write_rx_address(&self.radio, 0);

        self
    }

    /// Get the receive addresses that are enabled on the radio
    pub fn rx_addresses(&self) -> BitMask<u8> {
        reg_access::read_rx_address(&self.radio)
    }

    /// Receives a packet, waiting for `cycles` CPU cycles until returning [`crate::Error::TimedOut`]
    pub fn receive_packet_with_timeout(&self, cycles: u32) -> crate::Result<packet::Packet> {
        let mut p = packet::Packet::new_zeroed();

        let lf_len = reg_access::read_lf_len(&self.radio);
        let s0_len = reg_access::read_s0_len(&self.radio);
        let s1_len = reg_access::read_s1_len(&self.radio);

        p.set_lf_len(lf_len).set_s0_len(s0_len).set_s1_len(s1_len);

        let buf_ptr = p.buf_mut_ptr();
        reg_access::set_packet_ptr(&self.radio, buf_ptr);

        reg_access::tasks::start(&self.radio);
        self.wait_for_state_cycles(State::RX_IDLE, cycles)?;

        Ok(p)
    }

    /// Receives a packet, waiting indefinitely if needed
    pub fn receive_packet(&self) -> crate::Result<packet::Packet> {
        loop {
            let r = self.receive_packet_with_timeout(u32::MAX);
            if Err(crate::Error::TimedOut) == r {
                continue;
            }

            r?;
        }
    }
}

pub use nrf51_pac::radio::state::STATE_A as State;

use crate::packet::{LengthFieldLength, S0FieldLength, S1FieldLength};

impl<T> crate::Radio<T> {
    /// Get the state which the radio is currently in
    pub fn get_state(&self) -> crate::Result<State> {
        reg_access::get_state(&self.radio).ok_or(crate::Error::UnknownState)
    }

    /// Wait until radio goes into state. Break out of the function after `cycles`, returning [`crate::Error::TimedOut`].
    ///
    /// # Safety
    ///
    /// This function panics if the radio is in an invalid state
    pub fn wait_for_state_cycles(&self, state: State, cycles: u32) -> crate::Result<()> {
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
        while self.wait_for_state_cycles(state, u32::MAX) == Err(crate::Error::TimedOut) {
            core::hint::spin_loop();
        }
    }
}
