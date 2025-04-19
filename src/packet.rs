//! Module to interface with incoming and outgoing packets.

use core::marker::PhantomData;

use nrf51_hal::pac::RADIO;

use crate::println;

/// Something that can handle sending and receiving packets.
///
/// These methods do not check which mode the radio is in. The caller should check this.
pub trait PacketHandler<'p> {
    /// Constructs a packet from given payload and sends it.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn send_payload(&mut self, radio: &RADIO, payload: &'p [u8])
    -> Result<(), &'static str>;

    /// Processes a packet from the radio and returns the payload within.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn receive_payload(&self, radio: &RADIO) -> &'p [u8];
}

/// The maximum possible length of a packet
pub const MAX_PACKET_LEN: u8 = 254;

/// [`PacketHandler`] that uses a fixed-size buffer to process incoming and outgoing
/// packets from the radio.
pub struct FixedBufHandler<'p> {
    _buf: [u8; MAX_PACKET_LEN as usize],
    _marker: PhantomData<&'p ()>,
}

impl FixedBufHandler<'_> {
    /// Creates a new handler.
    pub fn new(buf: [u8; MAX_PACKET_LEN as usize]) -> Self {
        Self {
            _buf: buf,
            _marker: PhantomData,
        }
    }
}

impl<'p> PacketHandler<'p> for FixedBufHandler<'p> {
    unsafe fn send_payload(
        &mut self,
        radio: &RADIO,
        payload: &'p [u8],
    ) -> Result<(), &'static str> {
        // length of the length field
        let payload_len = payload.len();
        println!("Payload length: {}", payload_len);
        let len = 1 + payload_len;

        if len > MAX_PACKET_LEN as usize || len > (u8::MAX) as usize {
            return Err("Packet is too long");
        }

        // LENGTH field length (in bits)
        radio.pcnf0.write(|w| unsafe { w.lflen().bits(8) });
        // s0 length in bits
        radio.pcnf0.write(|w| w.s0len().bit(false));
        // s1 length in bytes
        radio.pcnf0.write(|w| unsafe { w.s1len().bits(0) });

        self._buf[0] = payload_len as u8;
        for (b, p) in self._buf[1..].iter_mut().zip(payload.iter()) {
            *b = *p
        }

        radio
            .packetptr
            .write(|w| unsafe { w.bits(self._buf.as_ptr().addr() as u32) });

        radio.tasks_start.write(|w| unsafe { w.bits(0b1) });
        println!("Started radio");

        Ok(())
    }

    unsafe fn receive_payload(&self, radio: &RADIO) -> &'p [u8] {
        todo!("tried to receive payload")
    }
}
