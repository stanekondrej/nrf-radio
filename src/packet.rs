//! Module to interface with incoming and outgoing packets.

use nrf51_hal::pac::RADIO;

/// Something that can handle sending and receiving packets.
pub trait PacketHandler<'p> {
    /// Constructs a packet from given payload and sends it.
    fn send_payload<T: Into<&'p [u8]>>(
        &self,
        radio: &RADIO,
        payload: T,
    ) -> Result<(), &'static str>;

    /// Processes a packet from the radio and returns the payload within.
    fn receive_payload(&self, radio: &RADIO) -> &'p [u8];
}

const MAX_PACKET_SIZE: u8 = 254;

/// [`PacketHandler`] that uses a fixed-size buffer to process incoming and outgoing
/// packets from the radio.
pub struct FixedBufHandler<'p> {
    _buf: &'p [u8],
}

impl Default for FixedBufHandler<'_> {
    /// Creates a new handler.
    fn default() -> Self {
        Self {
            _buf: &[0; MAX_PACKET_SIZE as usize],
        }
    }
}

impl<'p> PacketHandler<'p> for FixedBufHandler<'p> {
    fn send_payload<T: Into<&'p [u8]>>(
        &self,
        radio: &RADIO,
        payload: T,
    ) -> Result<(), &'static str> {
        todo!("tried to send payload")
    }

    fn receive_payload(&self, radio: &RADIO) -> &'p [u8] {
        todo!("tried to receive payload")
    }
}
