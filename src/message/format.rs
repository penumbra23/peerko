use endian_codec::{PackedSize, EncodeBE, DecodeBE};

const MAGIC_HEADER: u8 = 0x9D;

#[derive(Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct Header {
    magic_bytes: u8,
    v_type: u8,
    size: u16,
}

impl Header {
    pub fn new(version: u8, r#type: MessageType, size: u16) -> Header {
        Header {
            magic_bytes: MAGIC_HEADER,
            v_type: (version & 0x0F) << 4 | (r#type as u8) & 0x0F,
            size
        }
    }
}

impl Into<Vec<u8>> for Header {
    fn into(self) -> Vec<u8> {
        let mut buf = [0; 4];
        self.encode_as_be_bytes(&mut buf);
        buf.to_vec()
    }
}

#[repr(u8)]
pub enum MessageType {
    Alive = 0x01,
    MemberReq = 0x02,
    MemberRes = 0x04,
    Chat = 0x08,
}

pub struct MemberRequest {
    group: String,
}

impl MemberRequest {
    pub fn new(group: &String) -> MemberRequest {
        MemberRequest { group: group.clone() }
    }
}

mod tests {
    use crate::message::format::{Header, MAGIC_HEADER, MessageType, MemberRequest};

    #[test]
    fn header_serialization() {
        let mut header = Header::new(12, MessageType::Alive, 501);
        let mut expected: Vec<u8> = vec![MAGIC_HEADER, 0xC1, 0x01, 0xF5];
        assert_eq!(<Header as Into<Vec<u8>>>::into(header), expected);

        header = Header::new(5, MessageType::Chat, 113);
        expected = vec![MAGIC_HEADER, 0x58, 0x00, 0x71];
        assert_eq!(<Header as Into<Vec<u8>>>::into(header), expected);
    }
}