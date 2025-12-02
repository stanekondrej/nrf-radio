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
pub type BitMask = u32;

// TODO: some of these functions should maybe be moved to the `crate::Radio<T>` impl, as they aren't
// specific to the enabled state
impl<T> crate::Radio<Enabled<T>> {
    impl_disable!();

    /// Set the frequency on which the radio operates
    pub fn set_frequency(&self, freq: Frequency) -> &Self {
        self.radio.frequency.write(|w| unsafe { w.bits(freq.0) });

        self
    }

    /// Get the frequency that the radio is set to
    pub fn frequency(&self) -> Frequency {
        let f = self.radio.frequency.read().bits();

        #[cfg(feature = "defmt")]
        defmt::println!("{}", f);
        Frequency::from_reg_value(f)
            .expect("invalid frequency in register; if you see this it is probably a bug")
    }

    /// Sets the mode for the radio
    pub fn set_mode(&self, mode: Mode) -> &Self {
        self.radio.mode.write(|w| w.mode().variant(mode));

        self
    }

    /// Get the mode that the radio is set to
    pub fn mode(&self) -> Mode {
        self.radio.mode.read().mode().variant()
    }

    /// Set the order of bits of the S0, LENGTH, S1, and PAYLOAD fields.
    pub fn set_endianness(&self, endian: Endianness) -> &Self {
        self.radio.pcnf1.write(|w| w.endian().variant(endian));

        self
    }

    /// Get the endianness that the radio is set to
    pub fn endianness(&self) -> Endianness {
        self.radio.pcnf1.read().endian().variant()
    }

    /// Enable the given interrupt on the radio. In order to actually receive the interrupt firing,
    /// it needs to be enabled in the [`NVIC`](nrf51_pac::NVIC) as well.
    ///
    /// # Safety
    ///
    /// If used incorrectly, this can break the behaviour of the abstraction. Try to use the
    /// provided functions unless absolutely necessary.
    unsafe fn enable_interrupt(&self, int: Interrupt) {
        self.radio.intenset.write(|w| unsafe { w.bits(int as u32) });
    }

    /// Disable the given interrupt on the radio
    ///
    /// # Safety
    ///
    /// If used incorrectly, this can break the behaviour of the abstraction. Try to use the
    /// provided functions unless absolutely necessary.
    unsafe fn disable_interrupt(&self, int: Interrupt) {
        self.radio.intenclr.write(|w| unsafe { w.bits(int as u32) });
    }

    /// Clear the interrupt on the given event.
    ///
    /// # Safety
    ///
    /// This can break the behaviour of the abstraction. Use with caution.
    unsafe fn clear_interrupt(&self, int: Interrupt) {
        macro_rules! impl_write {
            ($( ($variant:path, $reg_name:ident) ),*) => {
                match int {
                    $(
                        $variant => self.radio.$reg_name.write(|w| unsafe { w.bits(0) }),
                    )*
                }
            };
        }

        impl_write!(
            (Interrupt::Ready, events_ready),
            (Interrupt::Address, events_address),
            (Interrupt::Payload, events_payload),
            (Interrupt::End, events_end),
            (Interrupt::Disabled, events_disabled),
            (Interrupt::DevMatch, events_devmatch),
            (Interrupt::DevMiss, events_devmiss),
            (Interrupt::RSSIEnd, events_rssiend),
            (Interrupt::BCMatch, events_bcmatch)
        );
    }

    /// Returns a mask on which you can try bit ANDing to check the raised interrupts
    pub fn read_interrupts(&self) -> BitMask {
        self.radio.intenset.read().bits()
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
        self.radio
            .packetptr
            .write(|w| unsafe { w.bits(ptr as u32) });

        self
    }
}

pub use nrf51_pac::radio::txpower::TXPOWER_A as TxPower;

/// Logical address. Can be used for reception or transmission
#[allow(missing_docs)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, strum::FromRepr)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

    /// Get the transmission power the radio is set to. Returns `None` if the register value is
    /// invalid
    pub fn tx_power(&self) -> Option<TxPower> {
        self.radio.txpower.read().txpower().variant()
    }

    /// Sets the logical address to transmit from
    pub fn set_tx_address(&self, address: Address) -> &Self {
        self.radio
            .txaddress
            .write(|w| unsafe { w.bits(address as u32) });

        self
    }

    /// Disable all addresses for sending
    pub fn disable_all_tx_addresses(&self) -> &Self {
        self.radio.txaddress.write(|w| unsafe { w.bits(0) });

        self
    }

    /// Get the transmission address the radio is currently set to
    pub fn tx_address(&self) -> Address {
        let a = self.radio.txaddress.read().txaddress().bits();

        Address::from_repr(a as u32).expect("invalid tx address; if you're seeing this it's a bug")
    }
}

impl crate::Radio<Enabled<Receiver>> {
    impl_into_tx!();

    /// Enable a logical address for receiving. Multiple can be enabled at once by making use of
    /// [`Self::enable_rx_addresses`], which acts as a replacement for bitwise OR.
    pub fn enable_rx_address(&self, address: Address) -> &Self {
        self.radio.rxaddresses.modify(|r, w| {
            let e = r.bits();
            let mask = address as u32;
            unsafe { w.bits(e | mask) }
        });

        self
    }

    /// Disable a logical address for receiving. Multiple can be disabled at once by making use of
    /// [`Self::disable_rx_addresses`].
    pub fn disable_rx_address(&self, address: Address) -> &Self {
        self.radio.rxaddresses.modify(|r, w| {
            let e = r.bits();
            let mask = !(address as u32);

            unsafe { w.bits(e & mask) }
        });

        self
    }

    /// Sets the logical addresses on which the radio should listen for packets
    pub fn enable_rx_addresses(&self, addresses: &[Address]) -> &Self {
        if addresses.is_empty() {
            return self;
        }

        // calculate the resulting register value so that only one write is needed
        let mask = addresses.iter().fold(0_u32, |acc, x| acc | *x as u32);

        self.radio
            .rxaddresses
            .modify(|r, w| unsafe { w.bits(r.bits() | mask) });

        self
    }

    /// Unsets the logical addresses on which the radio should listen for packets
    pub fn disable_rx_addresses(&self, addresses: &[Address]) -> &Self {
        if addresses.is_empty() {
            return self;
        }

        // calculate the resulting register value so that only one write is needed
        let mask = addresses.iter().fold(0_u32, |acc, x| acc | *x as u32);

        self.radio.rxaddresses.modify(|r, w| {
            let orig = r.bits();
            let mask = orig & !(mask);

            unsafe { w.bits(mask) }
        });

        self
    }

    /// Disable all addresses for receiving
    pub fn disable_all_rx_addresses(&self) -> &Self {
        self.radio.rxaddresses.write(|w| unsafe { w.bits(0) });

        self
    }

    /// Get the receive addresses that are enabled on the radio
    pub fn rx_addresses(&self) -> BitMask {
        self.radio.rxaddresses.read().bits()
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
