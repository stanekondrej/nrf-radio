//! `nrf-radio` - use the nRF5xxxx SoCs' radio, easily

#![no_std]
#![deny(missing_docs)]

pub mod enums;
pub mod packet;

use core::{marker::PhantomData, mem};

// FIXME: fix dependency on singular crate
use nrf51_hal::pac;

trait Mode {}

#[allow(missing_docs)]
pub struct Transmitter;
#[allow(missing_docs)]
pub struct Receiver;

impl Mode for Transmitter {}
impl Mode for Receiver {}

/// RADIO peripheral interface.
///
/// Construct new instances using the [`Radio::new_receiver`] or [`Radio::new_transmitter`] associated
/// functions. Then, use `into_receiver` or `into_transmitter` to convert between both
/// freely and safely.
///
/// To disable the radio peripheral, either let the [`Radio`] struct go out of scope,
/// drop it manually, or just call [`Radio::disable()`].
#[allow(private_interfaces)]
pub struct Radio<'r, H, M = Receiver>
where
    H: packet::PacketHandler<'r>,
{
    radio: pac::RADIO,
    handler: H,
    _marker: PhantomData<&'r H>,

    _mode: PhantomData<M>,
}

// impl<M> Drop for Radio<M> {
//     fn drop(&mut self) {
//         unsafe {
//             self.disable();
//         }
//     }
// }

#[allow(private_bounds)]
impl<'r, M, H> Radio<'r, H, M>
where
    M: Mode,
    H: packet::PacketHandler<'r>,
{
    /// See [`Self::new_receiver`]
    #[allow(private_interfaces)]
    pub fn new(radio: pac::RADIO, handler: H) -> Radio<'r, H, Receiver> {
        Self::new_receiver(radio, handler)
    }

    /// Creates a new receiving interface to the RADIO peripheral
    #[allow(private_interfaces)]
    pub fn new_receiver(radio: pac::RADIO, handler: H) -> Radio<'r, H, Receiver> {
        Radio {
            radio,
            handler,

            _marker: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Checks the state that the radio is currently in.
    pub fn check_state(&self) -> enums::State {
        let state = self.radio.state.read().bits();
        unsafe { mem::transmute(state as u8) }
    }

    /// Disables the radio.
    ///
    /// # Safety
    ///
    /// This function disables the radio peripheral. In order to keep the code complexity
    /// as low as possible, you need to create a new receiver or transmitter the next
    /// time you want to use the RADIO.
    pub unsafe fn disable(&self) {
        self.radio.tasks_disable.write(|w| unsafe { w.bits(0b1) });
    }

    /// Enables interrupts to be emitted by the event specified
    pub fn enable_event(&self, event: enums::Event) {
        self.radio
            .intenset
            .write(|w| unsafe { w.bits(0b1 << event as u8) });
    }

    /// Unsets the bit signaling the firing of an event
    pub fn clear_event(&self, event: enums::Event) {
        self.radio
            .intenclr
            .write(|w| unsafe { w.bits(0b1 << event as u8) });
    }

    /// Clear all events
    pub fn clear_events(&self) {
        self.radio
            .intenclr
            .write(|w| unsafe { w.bits(0b10_0111_1111) });
    }

    /// Returns the raw bit representation from the register. Use bitwise AND `((reg ^
    /// variant as u32) != 0)` to check whether a certain event is active. This is
    /// simpler both from an implementation' as well as from a user's perspective.
    ///
    /// It is theoretically possible that two or more events are active at once. It
    /// SHOULD never happen (if the user/programmer handles interrupts properly).
    pub fn get_active_events(&self) -> u32 {
        self.radio.intenset.read().bits()
    }

    /// Gets the event that is currently set as active. Note that this expects only one
    /// event to be set at a time.
    ///
    /// Note that when called outside of a context invoked due to an event, this function
    /// will return [`Event::Ready`], because it has the value 0x0, and therefore there
    /// is literally no way to tell that event and no event apart.
    pub fn get_active_event(&self) -> Option<enums::Event> {
        // TODO: surely this can be rewritten as a macro or something...
        let events = [
            enums::Event::Ready,
            enums::Event::Address,
            enums::Event::Payload,
            enums::Event::End,
            enums::Event::Disabled,
            enums::Event::DevMatch,
            enums::Event::DevMiss,
            enums::Event::RSSIEnd,
            enums::Event::BCMatch,
        ];
        let reg = self.radio.intenset.read().bits();
        events.into_iter().find(|&e| reg & (0b1 << e as u32) != 0)
    }

    /// Sets the radio mode (protocol, tx/rx speed, etc..)
    pub fn set_mode(&self, mode: enums::Mode) {
        self.radio.mode.write(|w| unsafe { w.bits(mode as u32) });
    }

    /// Reads the set radio mode from the register.
    pub fn get_mode(&self) -> Option<enums::Mode> {
        use enums::Mode::*;

        let mode = self.radio.mode.read().bits();
        let modes = [Nrf250Kbit, Nrf1Mbit, Nrf2Mbit, Ble1Mbit];

        modes.iter().find(|&&m| (mode ^ m as u32) != 0).copied()
    }

    /// Starts the radio. Depending on the mode, this means either:
    ///
    /// - sending the packet specified in `packetptr`
    /// - receiving a packet into the memore specified in `packetptr`
    ///
    /// As you can see, `packetptr` plays an important role. See [`set_packet_addr`].
    fn start(&self) {
        self.radio.tasks_start.write(|w| unsafe { w.bits(0b1) });
    }

    /// Stops the radio. This might not seem very useful, but it can probably come in
    /// handy when you're receiving packets and you need to send something once in
    /// a while. Then, you can call this from an interrupt, send a packet, and go back
    /// to receiving for a while (or something).
    fn stop(&self) {
        self.radio.tasks_stop.write(|w| unsafe { w.bits(0b1) });
    }

    /// Switches the radio to receive mode. This function calls [`disable`] internally.
    fn switch_rx(&self) {
        unsafe {
            self.disable();
        }
        self.radio.tasks_rxen.write(|w| unsafe { w.bits(0b1) });
    }

    /// Switches the radio to transmission mode. This function calls [`disable`]
    /// internally.
    fn switch_tx(&self) {
        unsafe {
            self.disable();
        }
        self.radio.tasks_txen.write(|w| unsafe { w.bits(0b1) });
    }

    /// Sets the pointer to a packet buffer. Should be set to a new value after each
    /// transmission and receival.
    fn set_packet_ptr(&self, packet: &[u8]) {
        self.radio
            .packetptr
            .write(|w| unsafe { w.bits(&raw const packet as u32) });
    }

    /// Set the frequency at which the radio should broadcast and listen.
    ///
    /// This should be a number between 2400 and 2500. Otherwise, it will get clamped
    /// to this range anyways.
    pub fn set_frequency(&self, freq: u32) {
        let freq = if freq > 2500 { 2500 } else { freq };
        let f = u32::checked_sub(freq, 2400).unwrap_or(0);

        self.radio
            .frequency
            .write(|w| unsafe { w.frequency().bits(f as u8) });
    }

    /// Reads the set frequency from the register.
    pub fn get_frequency(&self) -> u32 {
        // THEORETICALLY, this number could be so big that after addition, it could
        // overflow. Is that a feasible scenario? I don't think so. If stuff breaks for
        // somebody because of this, I won't be sure what to believe anymore.

        let freq = self.radio.frequency.read().bits();
        //freq.checked_add(2400).expect("Frequency is set to an astronomically large number.")
        freq + 2400
    }
}

impl<'r, H> Radio<'r, H, Receiver>
where
    H: packet::PacketHandler<'r>,
{
    /// Converts the receiver into a transmitter.
    pub fn into_transmitter(self) -> Radio<'r, H, Transmitter> {
        self.switch_tx();

        Radio {
            radio: self.radio,
            handler: self.handler,

            _marker: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Enable listening on a logical address. This is different from the actual
    /// addresses that the radio uses internally, but is much simpler to use and is
    /// probably what you want.
    pub fn enable_rx_address(&self, address: enums::LogicalAddress) {
        //   1011_0101
        // | 0100_0000
        //   ---------
        //   1111_0101
        //    ^

        let current = self.radio.rxaddresses.read().bits();
        let byte = 0b1 << address as u8;
        let applied = current | byte;

        self.radio.rxaddresses.write(|w| unsafe { w.bits(applied) });
    }

    /// Disables a logical address for listening. See [`Radio::enable_rx_address`].
    pub fn disable_rx_address(&self, address: enums::LogicalAddress) {
        //   0010_0110
        // ^ 0000_0100
        //   ---------
        //   0010_0010
        //         ^

        let current = self.radio.rxaddresses.read().bits();
        let byte = 0b1 << address as u8;
        let applied = current ^ byte;

        self.radio.rxaddresses.write(|w| unsafe { w.bits(applied) });
    }

    /// Returns the raw bit representation from the register. Use bitwise AND `((reg ^
    /// variant as u32) != 0)` to check whether a certain address is enabled. This is
    /// simpler both from an implementation' as well as from a user's perspective.
    pub fn get_enabled_rx_addresses(&self) -> u32 {
        self.radio.rxaddresses.read().bits()
    }
}

impl<'r, H> Radio<'r, H, Transmitter>
where
    H: packet::PacketHandler<'r>,
{
    /// Converts the transmitter into a receiver.
    pub fn into_receiver(self) -> Radio<'r, H, Receiver> {
        self.switch_rx();

        Radio {
            radio: self.radio,
            handler: self.handler,

            _marker: PhantomData,
            _mode: PhantomData,
        }
    }

    /// Set the logical address to send packets from.
    pub fn set_tx_address(&self, address: enums::LogicalAddress) {
        self.radio
            .txaddress
            .write(|w| unsafe { w.txaddress().bits(address as u8) });
    }

    /// Reads the enabled Tx address from the register.
    pub fn get_tx_address(&self) -> Option<enums::LogicalAddress> {
        use enums::LogicalAddress::*;

        let addr = self.radio.txaddress.read().bits();
        // FIXME: rewrite this to be auto-generated from the actual enum (see strum)
        let addresses = [_0, _1, _2, _3, _4, _5, _6, _7];

        addresses.iter().find(|&&a| (addr ^ a as u32) != 0).copied()
    }

    /// Sets the power (in dB) with which the radio should broadcast. More power = higher
    /// energy consumption.
    pub fn set_tx_power(&self, power: enums::TxPower) {
        self.radio
            .txpower
            .write(|w| unsafe { w.txpower().bits(power as u8) });
    }

    /// Reads the set Tx poewr from the register.
    pub fn get_tx_power(&self) -> Option<enums::TxPower> {
        use enums::TxPower::*;

        let power = self.radio.txpower.read().bits();
        let levels = [
            Pos4dBm, _0dBm, Neg4dBm, Neg8dBm, Neg12dBm, Neg16dBm, Neg20dBm, Neg30dBm,
        ];

        levels.iter().find(|&&l| ((l as u32 ^ power) != 0)).copied()
    }
}
