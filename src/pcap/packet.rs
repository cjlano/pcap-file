use std::borrow::Cow;
use std::io::Write;
use std::time::Duration;

use byteorder_slice::byteorder::WriteBytesExt;
use byteorder_slice::result::ReadSlice;
use byteorder_slice::ByteOrder;
use derive_into_owned::IntoOwned;

use crate::errors::*;
use crate::TsResolution;


/// Pcap packet.
///
/// The payload can be owned or borrowed.
#[derive(Clone, Debug, IntoOwned)]
pub struct PcapPacket<'a> {
    /// Timestamp EPOCH of the packet with a nanosecond resolution
    pub timestamp: Duration,
    /// Original length of the packet when captured on the wire
    pub orig_len: u32,
    /// Payload, owned or borrowed, of the packet
    pub data: Cow<'a, [u8]>,
}

impl<'a> PcapPacket<'a> {
    /// Create a new borrowed `Packet` with the given parameters.
    pub fn new(timestamp: Duration, orig_len: u32, data: &'a [u8]) -> PcapPacket<'a> {
        PcapPacket { timestamp, orig_len, data: Cow::Borrowed(data) }
    }

    /// Create a new owned `Packet` with the given parameters.
    pub fn new_owned(timestamp: Duration, orig_len: u32, data: Vec<u8>) -> PcapPacket<'static> {
        PcapPacket { timestamp, orig_len, data: Cow::Owned(data) }
    }

    /// Parse a new borrowed [`PcapPacket`] from a slice.
    pub fn from_slice<B: ByteOrder>(slice: &'a [u8], ts_resolution: TsResolution, snap_len: u32) -> PcapResult<(&'a [u8], PcapPacket<'a>)> {
        let (rem, raw_packet) = RawPcapPacket::from_slice::<B>(slice)?;
        let s = Self::try_from_raw_packet(raw_packet, ts_resolution, snap_len)?;

        Ok((rem, s))
    }

    /// Write a [`PcapPacket`] to a writer.
    pub fn write_to<W: Write, B: ByteOrder>(&self, writer: &mut W, ts_resolution: TsResolution, snap_len: u32) -> PcapResult<usize> {
        let ts_sec = self.timestamp.as_secs().try_into().map_err(|_| PcapError::InvalidField("PcapPacket: timestamp_secs > u32::MAX"))?;
        let mut ts_frac = self.timestamp.subsec_nanos();
        if ts_resolution == TsResolution::MicroSecond {
            ts_frac /= 1000;
        }

        let incl_len = self.data.len().try_into().map_err(|_| PcapError::InvalidField("PcapPacket: incl_len > u32::MAX"))?;
        let orig_len = self.orig_len;

        if incl_len > snap_len {
            return Err(PcapError::InvalidField("PcapPacket: incl_len > snap_len"));
        }

        if incl_len > orig_len {
            return Err(PcapError::InvalidField("PcapPacket: incl_len > orig_len"));
        }

        let raw_packet = RawPcapPacket {
            ts_sec,
            ts_frac,
            incl_len,
            orig_len,
            data: Cow::Borrowed(&self.data[..]),
        };

        raw_packet.write_to::<_, B>(writer)
    }

    /// Try to create a [`PcapPacket`] from a [`RawPcapPacket`]
    pub fn try_from_raw_packet(raw: RawPcapPacket<'a>, ts_resolution: TsResolution, snap_len: u32) -> PcapResult<Self> {
        // Validate timestamps //
        let ts_sec = raw.ts_sec;
        let mut ts_nsec = raw.ts_frac;
        if ts_resolution == TsResolution::MicroSecond {
            ts_nsec = ts_nsec.checked_mul(1000).ok_or(PcapError::InvalidField("PacketHeader ts_nanosecond is invalid"))?;
        }
        if ts_nsec >= 1_000_000_000 {
            return Err(PcapError::InvalidField("PacketHeader ts_nanosecond >= 1_000_000_000"));
        }

        let incl_len = raw.incl_len;
        let orig_len = raw.orig_len;

        if incl_len > snap_len {
            return Err(PcapError::InvalidField("PacketHeader incl_len > snap_len"));
        }

        if orig_len > snap_len {
            return Err(PcapError::InvalidField("PacketHeader orig_len > snap_len"));
        }

        if incl_len > orig_len {
            return Err(PcapError::InvalidField("PacketHeader incl_len > orig_len"));
        }

        Ok(PcapPacket { timestamp: Duration::new(ts_sec as u64, ts_nsec), orig_len, data: raw.data })
    }
}


/// Raw Pcap packet with its header and data.
/// The fields of the packet are not validated.
///
/// The payload can be owned or borrowed.
#[derive(Clone, Debug, IntoOwned)]
pub struct RawPcapPacket<'a> {
    /// Timestamp in seconds
    pub ts_sec: u32,
    /// Nanosecond or microsecond part of the timestamp
    pub ts_frac: u32,
    /// Number of octets of the packet saved in file
    pub incl_len: u32,
    /// Original length of the packet on the wire
    pub orig_len: u32,
    /// Payload, owned or borrowed, of the packet
    pub data: Cow<'a, [u8]>,
}

impl<'a> RawPcapPacket<'a> {
    /// Parse a new borrowed [`RawPcapPacket`] from a slice.
    pub fn from_slice<B: ByteOrder>(mut slice: &'a [u8]) -> PcapResult<(&'a [u8], Self)> {
        // Check header length
        if slice.len() < 16 {
            return Err(PcapError::IncompleteBuffer);
        }

        // Read packet header //
        // Length checks done before //
        let ts_sec = slice.read_u32::<B>().unwrap();
        let ts_frac = slice.read_u32::<B>().unwrap();
        let incl_len = slice.read_u32::<B>().unwrap();
        let orig_len = slice.read_u32::<B>().unwrap();

        let pkt_len = incl_len as usize;
        if slice.len() < pkt_len {
            return Err(PcapError::IncompleteBuffer);
        }

        let packet = RawPcapPacket { ts_sec, ts_frac, incl_len, orig_len, data: Cow::Borrowed(&slice[..pkt_len]) };
        let rem = &slice[pkt_len..];

        Ok((rem, packet))
    }

    /// Write a [`RawPcapPacket`] to a writer.
    /// The fields of the packet are not validated.
    pub fn write_to<W: Write, B: ByteOrder>(&self, writer: &mut W) -> PcapResult<usize> {
        writer.write_u32::<B>(self.ts_sec).map_err(|e| PcapError::IoError(e))?;
        writer.write_u32::<B>(self.ts_frac).map_err(|e| PcapError::IoError(e))?;
        writer.write_u32::<B>(self.incl_len).map_err(|e| PcapError::IoError(e))?;
        writer.write_u32::<B>(self.orig_len).map_err(|e| PcapError::IoError(e))?;
        writer.write_all(&self.data).map_err(|e| PcapError::IoError(e))?;

        Ok(16 + self.data.len())
    }

    /// Try to convert a [`RawPcapPacket`] into a [`PcapPacket`]
    pub fn try_into_pcap_packet(self, ts_resolution: TsResolution, snap_len: u32) -> PcapResult<PcapPacket<'a>> {
        PcapPacket::try_from_raw_packet(self, ts_resolution, snap_len)
    }
}
