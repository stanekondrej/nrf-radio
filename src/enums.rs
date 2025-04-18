//! Constants defined in order to work with the radio more easily. See their respective
//! documentation for more details.

/// Radio state; useful for determining whether it's in `Tx`, `Rx`, or `Disabled` mode
#[derive(Debug, strum::IntoStaticStr)]
pub enum State {
    /// The radio is disabled
    Disabled = 0,
    /// The radio is ramping up into `Rx`
    RxRu = 1,
    /// The radio is waiting to receive something
    RxIdle = 2,
    /// The radio is receiving something
    Rx = 3,
    /// The radio is being disabled while in `Rx`
    RxDisable = 4,
    /// The radio is ramping up to transmit something
    TxRu = 9,
    /// The radio is waiting to transmit something
    TxIdle = 10,
    /// The radio is transmitting something
    Tx = 11,
    /// The radio is being disabled while in `Tx`
    TxDisable = 12,
}

/// Transmission power
#[allow(missing_docs)] // these are self-explanatory
#[derive(Clone, Copy, strum::EnumIter, strum::IntoStaticStr)]
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

/// The different protocols and speeds supported by the radio. The `Nrf` modes are
/// Nordic's proprietary implementations, while `Ble1Mbit is just` Bluetooth low energy.
#[allow(missing_docs)] // better described by the enum doc
#[derive(Clone, Copy, strum::EnumIter, strum::IntoStaticStr)]
pub enum Mode {
    Nrf1Mbit = 0,
    Nrf2Mbit = 1,
    Nrf250Kbit = 2,
    Ble1Mbit = 3,
}

/// **Basically** simpler representations of in-air physical addresses.
#[allow(missing_docs)] // better described by the enum doc
#[derive(Clone, Copy, strum::EnumIter, strum::IntoStaticStr)]
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
#[derive(PartialEq, Eq, Clone, Copy, strum::EnumIter, strum::IntoStaticStr)]
pub enum Event {
    /// RADIO has ramped up and is ready to be started
    Ready = 0,
    /// Address sent or received
    Address = 1,
    /// Packet payload sent or received
    Payload = 2,
    /// Packet sent or received
    End = 3,
    /// RADIO has been disabled
    Disabled = 4,
    /// A device address match occurred on the last received packet
    DevMatch = 5,
    /// No device address match occurred on the last received packet
    DevMiss = 6,
    /// Sampling of receive signal strength complete. A new RSSI sample is ready for readout from the
    /// RSSISAMPLE register.
    RSSIEnd = 7,
    /// Bit counter reached bit count value specified in the BCC register
    BCMatch = 10,
}
