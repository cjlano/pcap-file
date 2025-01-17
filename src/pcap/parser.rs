use byteorder_slice::{BigEndian, LittleEndian};

use super::RawPcapPacket;
use crate::errors::*;
use crate::pcap::{PcapHeader, PcapPacket};
use crate::Endianness;


/// Parses a Pcap from a slice of bytes.
///
/// You can match on [PcapError::IncompleteBuffer](enum.PcapError.html) to known if the parser need more data.
///
/// # Examples
///
/// ```no_run
/// use pcap_file::pcap::PcapParser;
/// use pcap_file::PcapError;
///
/// let pcap = vec![0_u8; 0];
/// let mut src = &pcap[..];
///
/// // Creates a new parser and parse the pcap header
/// let (rem, pcap_parser) = PcapParser::new(&pcap[..]).unwrap();
/// src = rem;
///
/// loop {
///     match pcap_parser.next_packet(src) {
///         Ok((rem, packet)) => {
///             // Do something
///
///             // Don't forget to update src
///             src = rem;
///
///             // No more data, if no more incoming either then this is the end of the file
///             if rem.is_empty() {
///                 break;
///             }
///         },
///         Err(PcapError::IncompleteBuffer) => {}, // Load more data into src
///         Err(_) => {},                           // Parsing error
///     }
/// }
/// ```
#[derive(Debug)]
pub struct PcapParser {
    header: PcapHeader,
}

impl PcapParser {
    /// Creates a new `PcapParser`.
    /// Returns the parser and the remainder.
    pub fn new(slice: &[u8]) -> PcapResult<(&[u8], PcapParser)> {
        let (slice, header) = PcapHeader::from_slice(slice)?;

        let parser = PcapParser { header };

        Ok((slice, parser))
    }

    /// Returns the next [`PcapPacket`] and the remainder.
    pub fn next_packet<'a>(&self, slice: &'a [u8]) -> PcapResult<(&'a [u8], PcapPacket<'a>)> {
        match self.header.endianness {
            Endianness::Big => PcapPacket::from_slice::<BigEndian>(slice, self.header.ts_resolution, self.header.snaplen),
            Endianness::Little => PcapPacket::from_slice::<LittleEndian>(slice, self.header.ts_resolution, self.header.snaplen),
        }
    }

    /// Returns the next packet and the remainder.
    pub fn next_raw_packet<'a>(&self, slice: &'a [u8]) -> PcapResult<(&'a [u8], RawPcapPacket<'a>)> {
        match self.header.endianness {
            Endianness::Big => RawPcapPacket::from_slice::<BigEndian>(slice),
            Endianness::Little => RawPcapPacket::from_slice::<LittleEndian>(slice),
        }
    }

    /// Returns the header of the pcap file
    pub fn header(&self) -> PcapHeader {
        self.header
    }
}
