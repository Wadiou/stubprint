//! The pictures a printer keeps in its memory, and what it says about that memory.

use std::collections::BTreeMap;

use super::reader::CAN;
use crate::bitmap::Bitmap;

/// The two places `GS ( L` keeps graphics under a key of two characters. What stays when the
/// printer is reset is the first, and what is lost with `ESC @` on some models is the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Area {
    Nv,
    Download,
}

impl Area {
    /// How many bytes the area holds, as the printer says. These are what an Epson TM-T88 series
    /// printer has by default, and the model of the profile does not change them.
    fn capacity(self) -> usize {
        match self {
            Area::Nv => 256 * 1024,
            Area::Download => 12 * 1024,
        }
    }

    /// The first byte of the answer that tells the size of an area, after the header 0x37.
    fn size_identifier(self, remaining: bool) -> u8 {
        match (self, remaining) {
            (Area::Nv, false) => 0x30,
            (Area::Nv, true) => 0x31,
            (Area::Download, _) => 0x32,
        }
    }

    /// The identifier of the answer that lists the keys of an area.
    fn list_identifier(self) -> u8 {
        match self {
            Area::Nv => 0x72,
            Area::Download => 0x73,
        }
    }
}

/// How many keys one answer lists.
const KEYS_PER_BLOCK: usize = 40;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;

/// A list of keys being sent to the program a block at a time.
#[derive(Debug)]
struct Listing {
    area: Area,
    keys: Vec<(u8, u8)>,
    /// The block that was sent last.
    block: usize,
}

impl Listing {
    /// `37 identifier status keys... 00`, where the status is 0x41 if more blocks follow.
    fn answer(&self) -> Vec<u8> {
        let start = self.block * KEYS_PER_BLOCK;
        let end = (start + KEYS_PER_BLOCK).min(self.keys.len());
        let mut answer = vec![
            0x37,
            self.area.list_identifier(),
            if end < self.keys.len() { 0x41 } else { 0x40 },
        ];
        for (first, second) in &self.keys[start..end] {
            answer.extend_from_slice(&[*first, *second]);
        }
        answer.push(0);
        answer
    }

    fn has_more(&self) -> bool {
        (self.block + 1) * KEYS_PER_BLOCK < self.keys.len()
    }
}

/// What the printer remembers: the pictures of `FS q`, and the graphics of `GS ( L` in both areas.
#[derive(Debug, Default)]
pub(super) struct Memory {
    bit_images: Vec<Bitmap>,
    nv: BTreeMap<(u8, u8), Bitmap>,
    download: BTreeMap<(u8, u8), Bitmap>,
    listing: Option<Listing>,
}

impl Memory {
    fn graphics(&self, area: Area) -> &BTreeMap<(u8, u8), Bitmap> {
        match area {
            Area::Nv => &self.nv,
            Area::Download => &self.download,
        }
    }

    fn graphics_mut(&mut self, area: Area) -> &mut BTreeMap<(u8, u8), Bitmap> {
        match area {
            Area::Nv => &mut self.nv,
            Area::Download => &mut self.download,
        }
    }

    /// How many bytes of pictures are kept in all.
    pub fn bytes(&self) -> usize {
        self.used(Area::Nv) + self.used(Area::Download)
    }

    /// How many bytes of an area are taken. The pictures of `FS q` share the first area.
    fn used(&self, area: Area) -> usize {
        let bit_images = match area {
            Area::Nv => self.bit_images.as_slice(),
            Area::Download => &[],
        };
        bit_images
            .iter()
            .chain(self.graphics(area).values())
            .map(|bitmap| bitmap.data().len())
            .sum()
    }

    /// The answer to the requests for the size of an area: `37 identifier digits 00`, with the
    /// whole size or what is left of it.
    pub fn size_answer(&self, area: Area, remaining: bool) -> Vec<u8> {
        let size = if remaining {
            area.capacity().saturating_sub(self.used(area))
        } else {
            area.capacity()
        };
        let mut answer = vec![0x37, area.size_identifier(remaining)];
        answer.extend_from_slice(size.to_string().as_bytes());
        answer.push(0);
        answer
    }

    pub fn set_bit_images(&mut self, bit_images: Vec<Bitmap>) {
        self.bit_images = bit_images;
    }

    pub fn bit_image(&self, number: usize) -> Option<&Bitmap> {
        self.bit_images.get(number)
    }

    /// All the pictures of `FS q`, in order.
    pub fn bit_images(&self) -> &[Bitmap] {
        &self.bit_images
    }

    pub fn define(&mut self, area: Area, key: (u8, u8), bitmap: Bitmap) {
        self.graphics_mut(area).insert(key, bitmap);
    }

    pub fn get(&self, area: Area, key: (u8, u8)) -> Option<&Bitmap> {
        self.graphics(area).get(&key)
    }

    pub fn remove(&mut self, area: Area, key: (u8, u8)) {
        self.graphics_mut(area).remove(&key);
    }

    pub fn clear(&mut self, area: Area) {
        self.graphics_mut(area).clear();
    }

    /// Starts to send the keys of an area, and gives the first block. The program asks for the
    /// next ones with [`Memory::answer`].
    pub fn list(&mut self, area: Area) -> Vec<u8> {
        let listing = Listing {
            area,
            keys: self.graphics(area).keys().copied().collect(),
            block: 0,
        };
        let answer = listing.answer();
        self.listing = Some(listing);
        answer
    }

    /// While keys are being listed, a byte from the program answers the list: ACK asks for the next
    /// block, NAK for the same one again, and CAN ends it. Any other byte ends it as well, and is
    /// then read as it would be at any other time.
    ///
    /// Gives the bytes to send back when the byte was an answer and is used up, and nothing when
    /// the byte is for the decoder.
    pub fn answer(&mut self, byte: u8) -> Option<Vec<u8>> {
        let listing = self.listing.as_mut()?;
        match byte {
            ACK if listing.has_more() => {
                listing.block += 1;
                Some(listing.answer())
            }
            NAK => Some(listing.answer()),
            ACK | CAN => {
                self.listing = None;
                Some(Vec::new())
            }
            _ => {
                self.listing = None;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> Bitmap {
        Bitmap::from_rows(8, 2, vec![0xaa, 0x55]).unwrap()
    }

    #[test]
    fn a_size_is_digits_between_the_header_and_a_nul() {
        let memory = Memory::default();
        let mut whole = vec![0x37, 0x30];
        whole.extend_from_slice(b"262144");
        whole.push(0);
        assert_eq!(memory.size_answer(Area::Nv, false), whole);
        assert_eq!(
            memory.size_answer(Area::Download, true),
            b"\x372\x31\x32\x32\x38\x38\0"
        );
    }

    #[test]
    fn what_is_stored_takes_from_what_is_left() {
        let mut memory = Memory::default();
        memory.define(Area::Nv, (b'A', b'B'), picture());
        assert_eq!(memory.bytes(), 2);
        assert_eq!(memory.size_answer(Area::Nv, true), b"\x371262142\0");
        assert_eq!(memory.size_answer(Area::Download, true), b"\x3721228\x38\0");
        memory.clear(Area::Nv);
        assert_eq!(memory.bytes(), 0);
    }

    #[test]
    fn nothing_is_an_answer_when_no_list_is_being_sent() {
        let mut memory = Memory::default();
        for byte in [ACK, NAK, CAN, b'x'] {
            assert_eq!(memory.answer(byte), None);
        }
    }

    #[test]
    fn a_list_goes_on_with_ack_repeats_with_nak_and_ends_with_can() {
        let mut memory = Memory::default();
        for key in 0..41 {
            memory.define(Area::Nv, (b'0', b'0' + key), picture());
        }
        let first = memory.list(Area::Nv);
        assert_eq!(first[..3], [0x37, 0x72, 0x41]);
        assert_eq!(first.len(), 3 + 80 + 1);
        let last = memory.answer(ACK).unwrap();
        assert_eq!(last, [0x37, 0x72, 0x40, b'0', b'0' + 40, 0]);
        assert_eq!(memory.answer(NAK), Some(last));
        // After the last block, ACK ends the list without an answer.
        assert_eq!(memory.answer(ACK), Some(Vec::new()));
        assert_eq!(memory.answer(ACK), None);

        memory.list(Area::Nv);
        assert_eq!(memory.answer(CAN), Some(Vec::new()));
        assert_eq!(memory.answer(NAK), None);
    }

    #[test]
    fn another_byte_ends_a_list_and_is_not_used_up() {
        let mut memory = Memory::default();
        memory.list(Area::Download);
        assert_eq!(memory.answer(b'x'), None);
        assert_eq!(memory.answer(ACK), None);
    }
}
