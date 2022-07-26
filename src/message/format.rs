use std::{fmt::Display, net::{SocketAddr, IpAddr}, io::{Cursor, Read}};

use byteorder::{WriteBytesExt, BigEndian, ReadBytesExt};

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
pub trait MessageContent: Clone + Into<Vec<u8>> + TryFrom<Vec<u8>> {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    magic_bytes: u8,
    version: u8,
    msg_type: MessageType,
    size: u16,
}

impl Header {
    pub fn new(version: u8, r#type: MessageType, size: u16) -> Header {
        Header {
            magic_bytes: MAGIC_HEADER,
            version,
            msg_type: r#type,
            size,
        }
    }

    pub fn msg_type(&self) -> MessageType {
        self.msg_type
    }

    pub fn msg_size(&self) -> u16 {
        self.size
    }
}

impl Into<Vec<u8>> for Header {
    fn into(self) -> Vec<u8> {
        let mut buf = vec![];
        buf.write_u8(self.magic_bytes).unwrap();
        buf.write_u8((self.version << 4) | self.msg_type as u8).unwrap();
        buf.write_u16::<BigEndian>(self.size).unwrap();
        buf
    }
}

impl TryFrom<Vec<u8>> for Header {
    type Error = FormatError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(value);
        let magic_bytes = reader.read_u8().unwrap();
        let version_type = reader.read_u8().unwrap();
        let size = reader.read_u16::<BigEndian>().unwrap();
        Ok(Header {
            magic_bytes,
            version: version_type & 0xF0,
            msg_type: MessageType::from(version_type & 0x0F),
            size,
        })
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRequest {
    peer_id: String,
    group: String,
}

impl MessageContent for MemberRequest {}

impl MemberRequest {
    pub fn new(peer_id: &str, group: &str) -> Result<MemberRequest, FormatError> {
        if group.len() > 32 {
            return Err(FormatError{error: String::from("Group name exceeds 32.")});
        }

        if peer_id.len() > 32 {
            return Err(FormatError{error: String::from("Peer id exceeds 32.")});
        }
        
        Ok(MemberRequest { group: group.to_string(),  peer_id: peer_id.to_string() })
    }

    pub fn group_name(&self) -> String {
        self.group.clone()
    }

    pub fn peer_id(&self) -> String {
        self.peer_id.clone()
    }
}

impl Into<Vec<u8>> for MemberRequest {
    fn into(self) -> Vec<u8> {
        let mut buf = vec![0u8; 64];
        buf[0..self.group.len()].copy_from_slice(&self.group.as_bytes());
        buf[32..32+self.peer_id.len()].copy_from_slice(&self.peer_id.as_bytes());
        buf
    }
}

impl TryFrom<Vec<u8>> for MemberRequest {
    type Error = FormatError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(&value);

        let mut grp_buf = vec![0; 32];
        let mut peer_id_buf = vec![0; 32];
        reader.read_exact(&mut grp_buf).unwrap();
        reader.read_exact(&mut peer_id_buf).unwrap();

        // TODO: handle this
        let group = String::from_utf8(grp_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();
        let peer_id = String::from_utf8(peer_id_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();

        Ok(MemberRequest {
            group,
            peer_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberResponse {
    group: String,
    member_number: u8,
    peers: Vec<(String, SocketAddr)>,
}

impl MessageContent for MemberResponse {}

impl MemberResponse {
    pub fn new(group: &str, peers: Vec<(String, SocketAddr)>) -> Result<MemberResponse, FormatError> {
        if peers.len() > 5 {
            return Err(FormatError{error: String::from("More than 5 peer addresses.")});
        }

        if group.len() > 32 {
            return Err(FormatError{error: String::from("Group name exceeds 32.")});
        }
        
        let member_number = peers.len().try_into().expect("Failed to get member count");

        Ok(MemberResponse { group: group.to_string(), member_number, peers })
    }

    pub fn group_name(&self) -> String {
        self.group.clone()
    }

    pub fn peers(&self) -> &Vec<(String, SocketAddr)> {
        &self.peers
    }

}

impl Into<Vec<u8>> for MemberResponse {
    fn into(self) -> Vec<u8> {
        // group_name + member_count + peer_id + IP(4) + Port(2)
        let msg_size = 32 + 1 + (32 + 4 + 2) * self.peers.len();
        let mut buf = vec![0; msg_size];

        let grp_name_len = self.group.len();
        buf[0..grp_name_len].copy_from_slice(self.group.as_bytes());

        buf[32] = self.peers.len() as u8;

        for i in 0..self.peers.len() {
            let (peer_id, peer_addr) = &self.peers[i];
            let offset = 33 + 38*i;
            buf[offset..offset+peer_id.len()].copy_from_slice(peer_id.as_bytes());

            let ip_bytes = match peer_addr.ip() {
                IpAddr::V4(ip) => ip.octets(),
                _ => panic!("Only IPv4 supported"),
            };

            buf[offset+32..offset+36].copy_from_slice(&ip_bytes);
            buf[offset+36..offset+38].copy_from_slice(&peer_addr.port().to_be_bytes());
        }

        buf
    }
}

impl TryFrom<Vec<u8>> for MemberResponse {
    type Error = FormatError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(value);
        let mut grp_buf = vec![0; 32];
        
        reader.read_exact(&mut grp_buf).unwrap();

        let group = String::from_utf8(grp_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();

        let member_number = reader.read_u8().unwrap();

        let mut peers: Vec<(String, SocketAddr)> = Vec::new();

        for i in 0..member_number {
            let mut peer_id_buf = vec![0; 32];
            reader.read_exact(&mut peer_id_buf).unwrap();
            let peer_id = String::from_utf8(peer_id_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();

            let mut ip_buf = [0; 4];
            reader.read_exact(&mut ip_buf).unwrap();

            let port = reader.read_u16::<BigEndian>().unwrap();

            peers.push((peer_id, SocketAddr::new(IpAddr::from(ip_buf), port)));
        }

        Ok(MemberResponse{
            group,
            member_number,
            peers,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    peer_id: String,
    msg: String,
}


impl Into<Vec<u8>> for Chat {
    fn into(self) -> Vec<u8> {
        let msg_size = 33 + self.msg.len();
        let mut buf = vec![0; msg_size];

        buf[0..self.peer_id.len()].copy_from_slice(self.peer_id.as_bytes());
        buf[32] = self.msg.len() as u8;
        buf[33..msg_size].copy_from_slice(self.msg.as_bytes());
        buf
    }
}

impl TryFrom<Vec<u8>> for Chat {
    type Error = FormatError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(value);
        let mut peer_id_buf = vec![0; 32];
        
        reader.read_exact(&mut peer_id_buf).unwrap();

        let peer_id = String::from_utf8(peer_id_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();

        let msg_len = reader.read_u8().unwrap() as usize;
        let mut msg_buf = vec![0; msg_len];

        reader.read_exact(&mut msg_buf).unwrap();
        let msg = String::from_utf8(msg_buf.into_iter().filter(|s| *s != 0).collect()).unwrap();

        Ok(Chat{
            peer_id,
            msg,
        })
    }
}

impl MessageContent for Chat {}

impl Chat {
    pub fn new(peer_id: String, msg: &String) -> Chat {
        Chat{
            peer_id,
            msg: msg.clone(),
        }
    }

    pub fn msg(&self) -> &str {
        &self.msg
    }

    pub fn peer_id(&self) -> String {
        self.peer_id.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<T> 
    where T: MessageContent {
    header: Header,
    content: Option<T>,
}

impl<T> Message<T> where T: MessageContent {
    pub fn new(header: Header, content: Option<T>) -> Message<T> {
        Message { header, content }
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn content(&self) -> Option<&T> {
        self.content.as_ref()
    }
}

impl<T> Into<Vec<u8>> for Message<T> where T: MessageContent {
    fn into(self) -> Vec<u8> {
        // let mut buf = [0; 576];
        // self.encode_as_be_bytes(&mut buf);
        // buf.to_vec()
        let mut msg_bytes: Vec<u8> = self.header.into();
        if let Some(content) = self.content {
            msg_bytes.extend(content.into());
        }
        msg_bytes
    }
}

impl<T> TryFrom<Vec<u8>> for Message<T> where T: MessageContent {
    type Error = FormatError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(&value);

        let mut header_bytes: [u8; 4] = [0; 4];
        reader.read_exact(&mut header_bytes).unwrap();

        // TODO: handle error
        let header = Header::try_from(header_bytes.to_vec()).unwrap();

        let content_size = value.len() - 4;

        let mut content_bytes: Vec<u8> = vec![0; content_size];
        reader.read_exact(&mut content_bytes).unwrap();

        let content = T::try_from(content_bytes)
            .map_err(|_err| FormatError { error: "Error converting message content".to_string() })?;

        Ok(Message {
            header,
            content: Some(content),
        })
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
        let req = MemberRequest::new("peer-A", "my-group").unwrap();
        let buf: Vec<u8> = req.into();
        assert_eq!(buf[0..8], *"my-group".as_bytes());
    }

    #[test]
    fn member_request_deserialization() {
        let req = MemberRequest::new("peer1", "my-group").unwrap();

        let bytes: Vec<u8> = req.into();

        let req2 = MemberRequest::try_from(bytes).unwrap();
        assert_eq!(req2.group, "my-group");
        assert_eq!(req2.peer_id, "peer1");
    }

    #[test]
    fn member_response_serialization() {
        let peers = vec![
            ("peer-A".to_string(), "11.22.33.44:1234".parse().unwrap()),
            ("peer-B".to_string(), "255.0.0.1:65511".parse().unwrap()),
        ];
        let res = MemberResponse::new("my-group", peers).unwrap();
        let buf: Vec<u8> = res.into();

        let res2 = MemberResponse::try_from(buf).unwrap();

        assert_eq!(res2.group, "my-group".to_string());

        assert_eq!(res2.member_number, 2);

        assert_eq!(res2.peers[0], ("peer-A".to_string(), "11.22.33.44:1234".parse().unwrap()));
        assert_eq!(res2.peers[1],  ("peer-B".to_string(), "255.0.0.1:65511".parse().unwrap()));
    }

    #[test]
    fn member_response_deserialization() {
        let data = [
            // group name
            'g' as u8, 'r' as u8, 'p' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            // member count
            2,
            // First peer name
            'p' as u8, 'e' as u8, 'e' as u8, 'r' as u8, 'A' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            // First IP and port
            11, 22, 255, 0, 0x04, 0xD2,
            // Second peer name
            'p' as u8, 'e' as u8, 'e' as u8, 'r' as u8, 'B' as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            // Second IP and port
            255, 0, 1, 1, 0xFD, 0xFE,
        ];
        let res = MemberResponse::try_from(data.to_vec()).unwrap();

        assert_eq!(res.group, "grp".to_string());

        // Member count
        assert_eq!(res.member_number, 2);

        let peers = res.peers();
        assert_eq!(peers[0], ("peerA".to_string(), SocketAddr::new("11.22.255.0".parse().unwrap(), 1234)));
        assert_eq!(peers[1], ("peerB".to_string(), SocketAddr::new("255.0.1.1".parse().unwrap(), 65022)));
    }
}