use binary_layout::prelude::*;

const MAGIC_HEADER: u8 = 0x9D;

define_layout!(header, LittleEndian, {
    magic_bytes: u8,
    v_type: u8,
    size: u16,
  });

pub fn new_header(version: u8, r#type: u8, size: u16) -> header::View<Vec<u8>> {
    header::View::new(
        vec![MAGIC_HEADER, 
        (version & 0x0F) << 4 | r#type & 0x0F, 
        (size >> 8 & 0xFF).try_into().unwrap(),
        (size & 0xFF).try_into().unwrap()])
}


mod tests {
    use crate::message::format::{new_header, MAGIC_HEADER};

    #[test]
    fn header_serialization() {
        let mut header = new_header(12, 0xF, 501);
        let mut expected: Vec<u8> = vec![MAGIC_HEADER, 0xCF, 0x01, 0xF5];
        assert_eq!(header.into_storage(), expected);

        header = new_header(5, 4, 113);
        expected = vec![MAGIC_HEADER, 0x54, 0x00, 0x71];
        assert_eq!(header.into_storage(), expected);
    }
}