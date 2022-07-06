use std::{fmt::Display, net::{SocketAddr, IpAddr, SocketAddrV4}, io::{Cursor, Write, Read}};

use endian_codec::{PackedSize, EncodeBE, DecodeBE};

const MAGIC_HEADER: u8 = 0x9D;

fn vec_to_sized_array<T, const N: usize>(vec: Vec<T>) -> Result<[T; N], FormatError> {
    vec.try_into()
        .map_err(|_| FormatError { error: String::from("Vec to sized array failed") })
}

#[derive(Clone, Debug)]
pub struct FormatError {
    pub error: String,
}

impl Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Format err: {}", self.error)
    }
}

/// Market trait for types that wrap the content of a message
pub trait MessageContent: EncodeBE + DecodeBE + Clone {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
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

    pub fn msg_type(&self) -> MessageType {
        MessageType::from(self.v_type & 0x0F)
    }

    pub fn msg_size(&self) -> u16 {
        self.size
    }
}

impl Into<Vec<u8>> for Header {
    fn into(self) -> Vec<u8> {
        let mut buf = [0; 4];
        self.encode_as_be_bytes(&mut buf);
        buf.to_vec()
    }
}

impl From<Vec<u8>> for Header {
    fn from(mut vec: Vec<u8>) -> Self {
        vec.resize(4, 0);
        Header::decode_from_be_bytes(&vec)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Alive = 0x01,
    MemberReq = 0x02,
    MemberRes = 0x04,
    Chat = 0x08,
}

impl From<u8> for MessageType {
    fn from(val: u8) -> Self {
        match val {
            0x01 => MessageType::Alive,
            0x02 => MessageType::MemberReq,
            0x04 => MessageType::MemberRes,
            0x08 => MessageType::Chat,
            _ => panic!("Wrong message type supplied")
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct MemberRequest {
    group: [u8; 32],
}

impl MessageContent for MemberRequest {}

impl MemberRequest {
    pub fn new(group: &str) -> Result<MemberRequest, FormatError> {
        if group.len() > 32 {
            return Err(FormatError{error: String::from("Group name exceeds 32.")});
        }
        
        let mut buf: [u8; 32]= [0; 32];
        buf[0..group.len()].copy_from_slice(group.as_bytes());
        Ok(MemberRequest { group: buf })
    }

    pub fn group_name(&self) -> Result<String, FormatError> {
        String::from_utf8(self.group.into_iter().filter(|&p| p != 0).collect())
            .map(|res| res.trim().to_string())
            .map_err(|err| FormatError { error: err.to_string() })
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
        self.group_name()
    }
}

impl From<Vec<u8>> for MemberRequest {
    fn from(mut vec: Vec<u8>) -> Self {
        vec.resize(32, 0);
        MemberRequest::decode_from_be_bytes(&vec)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct MemberResponse {
    group: [u8; 32],
    member_number: u32,
    data: [u8; 30],
}

impl MessageContent for MemberResponse {}

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

        let mut addr_buf = Cursor::new(vec![0; 30]);

        let member_count = addrs.len().try_into().expect("Failed to get member count");
        
        for addr in addrs {
            let ip_bytes = match addr.ip() {
                IpAddr::V4(ip) => ip.octets(),
                _ => panic!("Only IPv4 supported"),
            };

            addr_buf.write(&ip_bytes).unwrap();

            let port = addr.port();

            let mut port_bytes: [u8; 2] = [0; 2];
            port_bytes[0] = ((port & 0xFF00) >> 8) as u8;
            port_bytes[1] = (port & 0x00FF) as u8;

            addr_buf.write(&port_bytes).unwrap();
        };

        Ok(MemberResponse { group: buf, member_number: member_count, data: vec_to_sized_array(addr_buf.get_ref().to_vec()).unwrap() })
    }

    pub fn group_name(&self) -> Result<String, FormatError> {
        String::from_utf8(self.group.into_iter().filter(|&p| p != 0).collect())
            .map(|res| res.trim().to_string())
            .map_err(|err| FormatError { error: err.to_string() })
    }

    pub fn peers(&self) -> Vec<SocketAddr> {
        let mut peer_list =  Vec::<SocketAddr>::new();
        let mut addr_buf = Cursor::new(self.data);

        let member_count = self.member_number;
        
        for i in 0..member_count {
            let mut ip_buf = [0; 4];
            addr_buf.read_exact(&mut ip_buf).unwrap();
            let ip_addr = IpAddr::from(ip_buf);

            let mut port_buf: [u8; 2] = [0; 2];
            addr_buf.read_exact(&mut port_buf).unwrap();
            let port = ((port_buf[0] as u16) << 8)  + port_buf[1] as u16;

            peer_list.push(SocketAddr::new(ip_addr, port));
        };

        peer_list
    }

}

impl Into<Vec<u8>> for MemberResponse {
    fn into(self) -> Vec<u8> {
        let mut buf = [0; 66];
        self.encode_as_be_bytes(&mut buf);
        buf.to_vec()
    }
}

impl From<Vec<u8>> for MemberResponse {
    fn from(mut vec: Vec<u8>) -> Self {
        vec.resize(6632, 0);
        MemberResponse::decode_from_be_bytes(&vec)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct Chat {
    group: [u8; 32],
    member_number: u32,
    // TODO: see how this will get serialized
    // data: [u8; 512],
}

impl MessageContent for Chat {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct Empty {}

impl MessageContent for Empty {}

impl Empty {
    pub fn new() -> Empty {
        Empty {  }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PackedSize, EncodeBE, DecodeBE)]
pub struct Message<T> 
    where T: MessageContent {
    header: Header,
    content: T,
}

impl<T> Message<T> where T: MessageContent {
    pub fn new(header: Header, content: T) -> Message<T> {
        Message { header, content }
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn content(&self) -> &T {
        &self.content
    }
}

impl<T> Into<Vec<u8>> for Message<T> where T: MessageContent {
    fn into(self) -> Vec<u8> {
        let mut buf = [0; 576];
        self.encode_as_be_bytes(&mut buf);
        buf.to_vec()
    }
}

impl<T> From<Vec<u8>> for Message<T> where T: MessageContent {
    fn from(mut vec: Vec<u8>) -> Self {
        vec.resize(576, 0);
        Message::decode_from_be_bytes(&vec)
    }
}

mod tests {
    use std::net::SocketAddr;

    use super::{Header, MAGIC_HEADER, MessageType, MemberRequest, MemberResponse};

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
    fn member_request_deserialization() {
        let req = MemberRequest::from("my-group".as_bytes().to_vec());
        assert_eq!(req.group[0..8], *"my-group".as_bytes());
    }

    #[test]
    fn member_response_serialization() {
        let addrs = vec![
            "11.22.33.44:1234".parse().unwrap(),
            "255.0.0.1:65511".parse().unwrap(),
        ];
        let res = MemberResponse::new("my-group", addrs).unwrap();
        let buf: Vec<u8> = res.into();

        assert_eq!(buf[0..8], *"my-group".as_bytes());

        // Member count
        assert_eq!(buf[32..36], vec![0, 0, 0, 2]);

        // First IP
        assert_eq!(buf[36..40], vec![11, 22, 33, 44]);

        // First port
        assert_eq!(buf[40..42], vec![0x04, 0xD2]);

        // Second IP
        assert_eq!(buf[42..46], vec![255, 0, 0, 1]);

        // Second port
        assert_eq!(buf[46..48], vec![0xFF, 0xE7]);
    }

    #[test]
    fn member_response_deserialization() {
        let data = [
            // group name
            'g' as u8, 'r' as u8, 'p' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            // member count
            0, 0, 0, 2,
            // First IP and port
            11, 22, 255, 0, 0x04, 0xD2,
            // Second IP and port
            255, 0, 1, 1, 0xFD, 0xFE,
        ];
        let res = MemberResponse::from(data.to_vec());

        assert_eq!(res.group[0..3], *"grp".as_bytes());

        // Member count
        assert_eq!(res.member_number, 2);

        // First IP
        assert_eq!(res.data[0..4], vec![11, 22, 255, 0]);

        // First port
        assert_eq!(res.data[4..6], vec![0x04, 0xD2]);

        // Second IP
        assert_eq!(res.data[6..10], vec![255, 0, 1, 1]);

        // Second port
        assert_eq!(res.data[10..12], vec![0xFD, 0xFE]);

        let mut peers = res.peers();
        assert_eq!(peers.pop().unwrap(), SocketAddr::new("255.0.1.1".parse().unwrap(), 65022));
        assert_eq!(peers.pop().unwrap(), SocketAddr::new("11.22.255.0".parse().unwrap(), 1234));
    }
}