use std::{fmt::Display, net::{SocketAddr, IpAddr}};

use endian_codec::{PackedSize, EncodeBE, DecodeBE};

const MAGIC_HEADER: u8 = 0x9D;

#[derive(Clone, Debug)]
pub struct FormatError {
    pub error: String,
}

impl Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Format err: {}", self.error)
    }
}

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

#[derive(Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct MemberRequest {
    group: [u8; 32],
}

impl MemberRequest {
    pub fn new(group: &str) -> Result<MemberRequest, FormatError> {
        if group.len() > 32 {
            return Err(FormatError{error: String::from("Group name exceeds 32.")});
        }
        
        let mut buf: [u8; 32]= [0; 32];
        buf[0..group.len()].copy_from_slice(group.as_bytes());
        Ok(MemberRequest { group: buf })
    }
}

impl Into<Vec<u8>> for MemberRequest {
    fn into(self) -> Vec<u8> {
        let mut buf = [0; 32];
        self.encode_as_be_bytes(&mut buf);
        buf.to_vec()
    }
}

impl TryInto<String> for MemberRequest {
    type Error = FormatError;

    fn try_into(self) -> Result<String, Self::Error> {
        // Filter out zero bytes
        // TODO: encapsulate length of string
        String::from_utf8(self.group.into_iter().filter(|&p| p != 0).collect())
            .map(|res| res.trim().to_string())
            .map_err(|err| FormatError { error: err.to_string() })
    }
}

#[derive(Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct MemberResponse {
    group: [u8; 32],
    member_number: u32,
    data: [u8; 30],
}

impl MemberResponse {
    pub fn new(group: &str, addrs: Vec<SocketAddr>) -> Result<MemberResponse, FormatError> {
        if addrs.len() > 5 {
            return Err(FormatError{error: String::from("More than 5 peer addresses.")});
        }

        if group.len() > 32 {
            return Err(FormatError{error: String::from("Group name exceeds 32.")});
        }
        
        let mut buf: [u8; 32]= [0; 32];
        buf[0..group.len()].copy_from_slice(group.as_bytes());

        let mut addr_buf: [u8; 30]= [0; 30];

        let member_count = addrs.len().try_into().expect("Failed to get member count");
        
        let mut i = 0;
        addrs.iter().enumerate().for_each(|(i, addr)| 
        {
            let addr = addrs[i];
            let ip_bytes = match addr.ip() {
                IpAddr::V4(ip) => ip.octets(),
                _ => panic!("Only IPv4 supported"),
            };

            // TODO: address insert

            let port = addr.port();

            let mut port_bytes: [u8; 2] = [0; 2];
            port_bytes[0] = (port & 0xFF00) as u8;
            port_bytes[1] = (port & 0x00FF) as u8;

            // TODO: port insert
        });

        Ok(MemberResponse { group: buf, member_number: member_count, data: addr_buf })
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

    #[test]
    fn member_request_serialization() {
        let req = MemberRequest::new("my-group").unwrap();
        let buf: Vec<u8> = req.into();
        assert_eq!(buf[0..8], *"my-group".as_bytes());

        let req2 = MemberRequest::new("my-second-group").unwrap();
        let str: String = req2.try_into().unwrap();
        assert_eq!(str, "my-second-group");
    }

    #[test]
    fn member_response_serialization() {
        let req = MemberRequest::new("my-group").unwrap();
        let buf: Vec<u8> = req.into();
        assert_eq!(buf[0..8], *"my-group".as_bytes());

        let req2 = MemberRequest::new("my-second-group").unwrap();
        let str: String = req2.try_into().unwrap();
        assert_eq!(str, "my-second-group");
    }
}