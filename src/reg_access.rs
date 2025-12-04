//! Raw register access functions, working directly on the PAC RADIO peripheral

use nrf51_pac::RADIO;

use crate::{
    BitMask,
    packet::{LengthFieldLength, S0FieldLength, S1FieldLength},
};

pub(crate) fn disable(radio: &RADIO) {
    radio.tasks_disable.write(|w| unsafe { w.bits(1) });
}

pub(crate) fn enable_rx(radio: &RADIO) {
    radio.tasks_rxen.write(|w| unsafe { w.bits(1) });
}

pub(crate) fn enable_tx(radio: &RADIO) {
    radio.tasks_txen.write(|w| unsafe { w.bits(1) });
}

pub(crate) fn read_frequency(radio: &RADIO) -> u32 {
    radio.frequency.read().bits()
}

pub(crate) fn write_frequency(radio: &RADIO, freq: u32) {
    radio.frequency.write(|w| unsafe { w.bits(freq) });
}

pub(crate) fn read_mode(radio: &RADIO) -> crate::Mode {
    radio.mode.read().mode().variant()
}

pub(crate) fn write_mode(radio: &RADIO, mode: crate::Mode) {
    radio.mode.write(|w| w.mode().variant(mode));
}

pub(crate) fn get_endianness(radio: &RADIO) -> crate::Endianness {
    radio.pcnf1.read().endian().variant()
}

pub(crate) fn set_endianness(radio: &RADIO, endian: crate::Endianness) {
    radio.pcnf1.write(|w| w.endian().variant(endian));
}

pub(crate) fn read_interrupts(radio: &RADIO) -> crate::BitMask<u32> {
    radio.intenset.read().bits()
}

pub(crate) fn enable_interrupt(radio: &RADIO, int: crate::Interrupt) {
    radio.intenset.write(|w| unsafe { w.bits(int as u32) });
}

pub(crate) fn disable_interrupt(radio: &RADIO, int: crate::Interrupt) {
    radio.intenclr.write(|w| unsafe { w.bits(int as u32) });
}

pub(crate) fn set_packet_ptr<T>(radio: &RADIO, ptr: *mut T) {
    radio.packetptr.write(|w| unsafe { w.bits(ptr as u32) });
}

pub(crate) fn read_tx_power(radio: &RADIO) -> Option<crate::TxPower> {
    radio.txpower.read().txpower().variant()
}

pub(crate) fn set_tx_power(radio: &RADIO, tx_power: crate::TxPower) {
    radio.txpower.write(|w| w.txpower().variant(tx_power));
}

pub(crate) fn read_tx_address(radio: &RADIO) -> crate::BitMask<u8> {
    radio.txaddress.read().txaddress().bits()
}

pub(crate) fn set_tx_address(radio: &RADIO, addr: u32) {
    radio.txaddress.write(|w| unsafe { w.bits(addr) });
}

pub(crate) fn read_rx_addresses(radio: &RADIO) -> BitMask<u8> {
    radio
        .rxaddresses
        .read()
        .bits()
        .try_into()
        .expect("unable to fit a u32 into a u8") // this should never fail
}

pub(crate) fn write_rx_addresses(radio: &RADIO, mask: BitMask<u8>) {
    radio.rxaddresses.write(|w| unsafe { w.bits(mask.into()) });
}

pub(crate) fn clear_rx_addresses(radio: &RADIO) {
    radio.rxaddresses.write(|w| unsafe { w.bits(0) });
}

pub(crate) fn read_lf_len(radio: &RADIO) -> LengthFieldLength {
    LengthFieldLength::from_bits(radio.pcnf0.read().lflen().bits().into())
        .expect("invalid LENGTH field length in register")
}

pub(crate) fn read_s0_len(radio: &RADIO) -> S0FieldLength {
    S0FieldLength::from_bits(radio.pcnf0.read().s0len().bit().into())
        .expect("invalid S0 field length in register")
}

pub(crate) fn read_s1_len(radio: &RADIO) -> S1FieldLength {
    S1FieldLength::from_bits(radio.pcnf0.read().s1len().bits().into())
        .expect("invalid S1 field length in register")
}

pub(crate) fn get_state(radio: &RADIO) -> Option<crate::State> {
    radio.state.read().state().variant()
}

pub(crate) mod tasks {
    use super::RADIO;

    pub(crate) fn start(radio: &RADIO) {
        radio.tasks_start.write(|w| unsafe { w.bits(1) });
    }
}
