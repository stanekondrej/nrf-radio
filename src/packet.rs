//! Module to interface with incoming and outgoing packets.

use nrf51_hal::pac::RADIO;

/// Something that can handle sending and receiving packets.
///
/// These methods do not check which mode the radio is in. The caller should check this.
pub trait PacketHandler<'p> {
    /// Constructs a packet from given payload and sends it.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn send_payload(&self, radio: &RADIO, payload: &'p [u8]) -> Result<(), &'static str>;

    /// Processes a packet from the radio and returns the payload within.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn receive_payload(&self, radio: &RADIO) -> &'p [u8];
}

const MAX_PACKET_LEN: u8 = 254;

/// [`PacketHandler`] that uses a fixed-size buffer to process incoming and outgoing
/// packets from the radio.
pub struct FixedBufHandler<'p> {
    _buf: &'p [u8],
}

impl Default for FixedBufHandler<'_> {
    /// Creates a new handler.
    fn default() -> Self {
        Self {
            _buf: &[0; MAX_PACKET_LEN as usize],
        }
    }
}

impl<'p> PacketHandler<'p> for FixedBufHandler<'p> {
    unsafe fn send_payload(&self, radio: &RADIO, payload: &'p [u8]) -> Result<(), &'static str> {
        // length of the length field
        // FIXME: configurable
        let len_len = 1;
        let packet_len = payload.len();
        let len = len_len + packet_len;

        if len > MAX_PACKET_LEN as usize || len > (u8::MAX) as usize {
            return Err("Packet is too long");
        }

        radio.pcnf0.write(|w| unsafe { w.lflen().bits(len as u8) });

        // FIXME: configurable
        radio.pcnf0.write(|w| w.s0len().bit(false));
        radio.pcnf0.write(|w| unsafe { w.s1len().bits(0) });

        radio
            .packetptr
            .write(|w| unsafe { w.bits(self._buf.as_ptr().addr() as u32) });

        radio.tasks_start.write(|w| unsafe { w.bits(0b1) });

        Ok(())
    }

    unsafe fn receive_payload(&self, radio: &RADIO) -> &'p [u8] {
        todo!("tried to receive payload")
    }
}
