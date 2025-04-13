#![no_std]
#![deny(clippy::missing_crate_level_docs)]

pub mod packet;

use core::mem;

// FIXME: fix dependency on singular crate
use nrf51_hal::pac;

// TODO: this is definitely true on the nRF51822, but I'm not sure about other ones
pub const MAX_PACKET_LENGTH: usize = 254;

/// RADIO peripheral
pub struct Radio {
    radio: pac::RADIO,

    _buf: [u8; MAX_PACKET_LENGTH],
}

/// Radio state; useful for determining whether it's in `Tx`, `Rx`, or `Disabled` mode
#[derive(Debug)]
pub enum State {
    Disabled = 0,
    RxRu = 1,
    RxIdle = 2,
    Rx = 3,
    RxDisable = 4,
    TxRu = 9,
    TxIdle = 10,
    Tx = 11,
    TxDisable = 12,
}

/// Transmission power
pub enum TxPower {
    Pos4dBm = 0x04,
    _0dBm = 0x00,
    Neg4dBm = 0xFC,
    Neg8dBm = 0xF8,
    Neg12dBm = 0xF4,
    Neg16dBm = 0xF0,
    Neg20dBm = 0xEC,
    Neg30dBm = 0xD8,
}

/// The different protocols and speeds supported by the radio
pub enum Mode {
    Nrf1Mbit = 0,
    Nrf2Mbit = 1,
    Nrf250Kbit = 2,
    Ble1Mbit = 3,
}

/// **Basically** simpler representations of in-air physical addresses
pub enum LogicalAddress {
    _0 = 0,
    _1 = 1,
    _2 = 2,
    _3 = 3,
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
}

/// Events emitted by the radio when certain things happen.
///
/// A good example is the `END` event that is emitted **when a packet is received,**
/// and at the end of the transmission of a packet.
pub enum Event {
    Ready = 0,
    Address = 1,
    Payload = 2,
    End = 3,
    Disabled = 4,
    DevMatch = 5,
    DevMiss = 6,
    RSSIEnd = 7,
    BCMatch = 10,
}

impl Radio {
    /// Creates a new interface to the RADIO peripheral
    pub fn new(radio: pac::RADIO) -> Self {
        return Self {
            radio,
            _buf: [0; MAX_PACKET_LENGTH],
        };
    }

    /// Starts the radio. Depending on the mode, this means either:
    ///
    /// - sending the packet specified in `packetptr`
    /// - receiving a packet into the memore specified in `packetptr`
    ///
    /// As you can see, `packetptr` plays an important role. See [`set_packet_addr`].
    pub fn start(&self) {
        self.radio.tasks_start.write(|w| unsafe { w.bits(0b1) });
    }

    /// Stops the radio. This might not seem very useful, but it can probably come in
    /// handy when you're receiving packets and you need to send something once in
    /// a while. Then, you can call this from an interrupt, send a packet, and go back
    /// to receiving for a while (or something).
    pub fn stop(&self) {
        self.radio.tasks_stop.write(|w| unsafe { w.bits(0b1) });
    }

    /// Switches the radio to receive mode. This function calls [`disable`] internally.
    pub fn switch_rx(&self) {
        self.disable();
        self.radio.tasks_rxen.write(|w| unsafe { w.bits(0b1) });
    }

    /// Call this after the `END` event, when the radio is in `Rx`. You can check
    /// the mode by calling [`check_state`]
    pub fn handle_incoming_packet(&self) {}

    /// Switches the radio to transmission mode. This function calls [`disable`]
    /// internally.
    pub fn switch_tx(&self) {
        self.disable();
        self.radio.tasks_txen.write(|w| unsafe { w.bits(0b1) });
    }

    pub fn check_state(&self) -> State {
        let state = self.radio.state.read().bits();
        unsafe { mem::transmute(state as u8) }
    }

    /// Disables the radio.
    ///
    /// You may use this to (for example) save power, but if you just want to switch
    /// the mode of the radio, this is pointless, as the appropriate functions do this
    /// anyways. See [`switch_tx`] and [`switch_rx`].
    pub fn disable(&self) {
        self.radio.tasks_disable.write(|w| unsafe { w.bits(0b1) });
    }

    /// Enables interrupts to be emitted by the event specified
    pub fn enable_event(&self, event: Event) {
        self.radio
            .intenset
            .write(|w| unsafe { w.bits(0b1 << event as u8) });
    }

    /// Unsets the bit signaling the firing of an event
    pub fn clear_event(&self, event: Event) {
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

    /// Set the logical address to send packets from.
    pub fn set_tx_address(&self, address: LogicalAddress) {
        self.radio
            .txaddress
            .write(|w| unsafe { w.txaddress().bits(address as u8) });
    }

    /// Enable listening on a logical address. This is different from the actual
    /// addresses that the radio uses internally, but is much simpler to use and is
    /// probably what you want.
    pub fn enable_rx_address(&self, address: LogicalAddress) {
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

    /// Disables a logical address for listening. See [`enable_rx_address`].
    pub fn disable_rx_address(&self, address: LogicalAddress) {
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

    /// Sets the power (in dB) with which the radio should broadcast. More power = higher
    /// energy consumption.
    pub fn set_tx_power(&self, power: TxPower) {
        self.radio
            .txpower
            .write(|w| unsafe { w.txpower().bits(power as u8) });
    }

    /// Sets the radio mode - mostly useful for changing the transfer speed
    pub fn set_mode(&self, mode: Mode) {
        self.radio.mode.write(|w| unsafe { w.bits(mode as u32) });
    }
}
