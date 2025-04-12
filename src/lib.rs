#![no_std]
#![deny(clippy::missing_crate_level_docs)]

// FIXME: fix dependency on singular crate
use nrf51_hal::pac;

/// RADIO peripheral
pub struct Radio {
    radio: pac::RADIO,
}

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

pub enum Mode {
    Nrf1Mbit = 0,
    Nrf2Mbit = 1,
    Nrf250Kbit = 2,
    Ble1Mbit = 3,
}

#[allow(clippy::enum_variant_names)]
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

impl Radio {
    /// Creates a new interface to the RADIO peripheral
    pub fn new(radio: pac::RADIO) -> Self {
        return Self { radio };
    }

    /// Switches the radio to receive mode. This function calls [`disable`] internally.
    pub fn switch_rx(&self) {
        self.disable();
        self.radio.tasks_rxen.write(|w| unsafe { w.bits(0b1) });
    }

    /// Switches the radio to transmission mode. This function calls [`disable`]
    /// internally.
    pub fn switch_tx(&self) {
        self.disable();
        self.radio.tasks_txen.write(|w| unsafe { w.bits(0b1) });
    }

    /// Disables the radio.
    ///
    /// You may use this to (for example) save power, but if you just want to switch
    /// the mode of the radio, this is pointless, as the appropriate functions do this
    /// anyways. See [`switch_tx`] and [`switch_rx`].
    pub fn disable(&self) {
        self.radio.tasks_disable.write(|w| unsafe { w.bits(0b1) });
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
