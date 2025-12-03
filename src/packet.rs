//! Symbols related to sending and receiving packets

/// Maximum length of the `S0`, `LENGTH`, `S1`, and `PAYLOAD` fields combined
pub const MAX_IN_MEMORY_PACKET_LENGTH: usize = 254;

/// Maximum length of the `LENGTH` field
pub const MAX_LENGTH_FIELD_BITS: u32 = 16;
/// Maximum length of the `S0` field
pub const MAX_S0_LENGTH_BITS: u32 = 8;
/// Maximum length of the `S1` field
pub const MAX_S1_LENGTH_BITS: u32 = 16;

/// The buffer that holds a packet
pub type PacketBuffer = [u8; MAX_IN_MEMORY_PACKET_LENGTH];

/// A packet that can be serialized and transmitted over the network, or deserialized into and then
/// processed on this microcontroller
///
/// For transmission, use [`Packet::new`] and then write data to it.
/// For reception, use [`Packet::new`] and then pass the buffer pointer ([`Packet::buf_ptr`]) to
/// the [`crate::Radio`]
// FIXME: the docs here reference functionality not yet implemented
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Packet {
    buffer: PacketBuffer,

    lf_len: LengthFieldLength,
    s0_len: S0FieldLength,
    s1_len: S1FieldLength,
}

macro_rules! create_field_len_struct {
    ($name:ident, $upper_limit:ident) => {
        /// Struct containing the validated length of a field
        #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $name(u32);

        impl $name {
            /// Convert `len` to this value, checking if `len` is in bounds of the type
            pub fn from_bits(len: u32) -> Option<Self> {
                match len > $upper_limit {
                    true => None,
                    false => Some(Self(len)),
                }
            }
        }
    };
}

create_field_len_struct!(LengthFieldLength, MAX_LENGTH_FIELD_BITS);
create_field_len_struct!(S0FieldLength, MAX_S0_LENGTH_BITS);
create_field_len_struct!(S1FieldLength, MAX_S1_LENGTH_BITS);

impl Packet {
    /// Constructs a new, zeroed out packet
    pub(crate) fn new_zeroed() -> Self {
        let buf: PacketBuffer = [0; _];

        Packet {
            buffer: buf,

            lf_len: LengthFieldLength::default(),
            s0_len: S0FieldLength::default(),
            s1_len: S1FieldLength::default(),
        }
    }

    /// Set the length of the `LENGTH` field
    pub(crate) fn set_lf_len(&mut self, len: LengthFieldLength) -> &mut Self {
        self.lf_len = len;

        self
    }

    /// Set the length of the `S0` field
    pub(crate) fn set_s0_len(&mut self, len: S0FieldLength) -> &mut Self {
        self.s0_len = len;

        self
    }

    /// Set the length of the `S1` field
    pub(crate) fn set_s1_len(&mut self, len: S1FieldLength) -> &mut Self {
        self.s1_len = len;

        self
    }

    /// Returns a pointer to the inner buffer. Use this as something you can pass to the radio
    ///
    /// # Safety
    ///
    /// Make sure you send the packet away or utilize
    // FIXME: incomplete docs
    pub(crate) fn buf_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }

    pub(crate) fn buf_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }

    /// Serialize this packet into a buffer that can be handed over to the radio
    pub(crate) fn serialize(&self) -> crate::Result<SerializedPacketBuffer> {
        todo!()
    }
}

/// A serialized packet
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct SerializedPacketBuffer {
    /// The data buffer
    buffer: PacketBuffer,
    /// How many bytes the data occupies in the buffer (the buffer may be bigger than the data)
    len: usize,
}

impl SerializedPacketBuffer {
    /// Returns a slice of the inner buffer
    pub(crate) fn buf(&self) -> &[u8] {
        &self.buffer[0..self.len]
    }

    /// Returns a mutable slice of the inner buffer
    ///
    /// Be careful with this, you can break the transmission if you modify, for example, the
    /// serialized `LENGTH` field
    pub(crate) fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[0..self.len]
    }
}
