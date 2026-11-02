//! RFC 8484 DNS wire codec, using Hickory for DNS name decompression.
use super::super::model::pipeline::ResolveError;
use super::{canonical, public};
use hickory_proto::serialize::binary::{BinDecodable, BinDecoder};
use hickory_proto::{
    op::{Message, MessageType, OpCode, Query, ResponseCode},
    rr::{DNSClass, Name, RData, RecordType},
};
use std::{collections::HashSet, net::IpAddr};

#[derive(Clone, Debug)]
pub(super) struct Answer {
    pub addresses: Vec<IpAddr>,
    pub unsafe_answers: usize,
    pub ttl: u32,
    pub negative: bool,
    pub nxdomain: bool,
    pub authenticated: bool,
}

pub(super) fn query(host: &str, kind: u16) -> Result<Vec<u8>, ResolveError> {
    let name = Name::from_ascii(format!("{host}.")).map_err(|_| ResolveError::InvalidResponse)?;
    let mut message = Message::new(0, MessageType::Query, OpCode::Query);
    message.metadata.recursion_desired = true;
    // RFC 6840: signal interest in the resolver's AD assertion without requesting DNSSEC records.
    message.metadata.authentic_data = true;
    message.add_query(Query::query(name, RecordType::from(kind)));
    message.to_vec().map_err(|_| ResolveError::InvalidResponse)
}

pub(super) fn parse(
    bytes: &[u8],
    host: &str,
    kind: u16,
    require_ad: bool,
) -> Result<Answer, ResolveError> {
    if bytes.len() < 12 || bytes[3] & 0x40 != 0 {
        return Err(ResolveError::InvalidResponse);
    }
    let mut decoder = BinDecoder::new(bytes);
    let message = Message::read(&mut decoder).map_err(|_| ResolveError::InvalidResponse)?;
    if !decoder.is_empty() {
        return Err(ResolveError::InvalidResponse);
    }
    let header = message.metadata;
    if header.id != 0
        || header.message_type != MessageType::Response
        || header.op_code != OpCode::Query
        || header.truncation
        || header.checking_disabled
        || message.queries.len() != 1
    {
        return Err(ResolveError::InvalidResponse);
    }
    let question = &message.queries[0];
    if question.query_class != DNSClass::IN
        || question.query_type != RecordType::from(kind)
        || canonical(&question.name.to_ascii()).as_deref() != Some(host)
    {
        return Err(ResolveError::InvalidResponse);
    }
    if require_ad && !header.authentic_data {
        return Err(ResolveError::Dnssec);
    }
    if !matches!(
        header.response_code,
        ResponseCode::NoError | ResponseCode::NXDomain
    ) {
        return Err(ResolveError::Unavailable);
    }
    let mut seen = HashSet::new();
    let mut current = host.to_owned();
    let mut ttl = u32::MAX;
    loop {
        if !seen.insert(current.clone()) {
            return Err(ResolveError::InvalidResponse);
        }
        let mut found = None;
        for record in &message.answers {
            if record.dns_class != DNSClass::IN {
                return Err(ResolveError::InvalidResponse);
            }
            if record.record_type() == RecordType::CNAME
                && canonical(&record.name.to_ascii()).as_deref() == Some(current.as_str())
            {
                if found.is_some() {
                    return Err(ResolveError::InvalidResponse);
                }
                let RData::CNAME(name) = &record.data else {
                    return Err(ResolveError::InvalidResponse);
                };
                found = Some(canonical(&name.0.to_ascii()).ok_or(ResolveError::InvalidResponse)?);
                ttl = ttl.min(record.ttl);
            }
        }
        match found {
            Some(next) => current = next,
            None => break,
        }
    }
    let mut addresses = Vec::new();
    let mut unsafe_answers = 0;
    for record in &message.answers {
        let owner = canonical(&record.name.to_ascii()).ok_or(ResolveError::InvalidResponse)?;
        if record.record_type() == RecordType::RRSIG {
            if !seen.contains(&owner) {
                return Err(ResolveError::InvalidResponse);
            }
            continue;
        }
        if record.record_type() == RecordType::CNAME {
            if !seen.contains(&owner) {
                return Err(ResolveError::InvalidResponse);
            }
            continue;
        }
        if owner != current || record.record_type() != RecordType::from(kind) {
            return Err(ResolveError::InvalidResponse);
        }
        let ip = match &record.data {
            RData::A(value) if kind == 1 => IpAddr::V4(value.0),
            RData::AAAA(value) if kind == 28 => IpAddr::V6(value.0),
            _ => return Err(ResolveError::InvalidResponse),
        };
        ttl = ttl.min(record.ttl);
        if public(ip) {
            addresses.push(ip);
        } else {
            unsafe_answers += 1;
        }
    }
    if header.response_code == ResponseCode::NXDomain
        && (!addresses.is_empty() || unsafe_answers > 0)
    {
        return Err(ResolveError::InvalidResponse);
    }
    let negative = addresses.is_empty() && unsafe_answers == 0;
    if negative {
        let target =
            Name::from_ascii(format!("{current}.")).map_err(|_| ResolveError::InvalidResponse)?;
        let mut soa_ttl = None;
        for record in &message.authorities {
            if record.dns_class == DNSClass::IN
                && record.name.zone_of(&target)
                && let RData::SOA(soa) = &record.data
            {
                let value = record.ttl.min(soa.minimum);
                soa_ttl = Some(soa_ttl.map_or(value, |old: u32| old.min(value)));
            }
        }
        ttl = soa_ttl.map_or(0, |value| value.min(ttl));
    }
    Ok(Answer {
        addresses,
        unsafe_answers,
        ttl: if ttl == u32::MAX { 0 } else { ttl },
        negative,
        nxdomain: header.response_code == ResponseCode::NXDomain,
        authenticated: header.authentic_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::{
        Record,
        rdata::{A, AAAA, CNAME, SOA},
    };
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn response() -> Message {
        let mut message = Message::response(0, OpCode::Query);
        message.add_query(Query::query(
            Name::from_ascii("example.com.").unwrap(),
            RecordType::A,
        ));
        message
    }

    #[test]
    fn query_has_zero_id_and_recursion_without_cd() {
        let packet = query("example.com", 1).unwrap();
        let message = Message::from_vec(&packet).unwrap();
        assert_eq!(message.metadata.id, 0);
        assert!(message.metadata.recursion_desired);
        assert!(message.metadata.authentic_data);
        assert!(!message.metadata.checking_disabled);
        assert!(message.edns.is_none());
    }

    #[test]
    fn response_validates_question_flags_and_ad() {
        let mut message = response();
        message.answers.push(Record::from_rdata(
            Name::from_ascii("example.com.").unwrap(),
            50,
            RData::A(A(Ipv4Addr::new(1, 1, 1, 1))),
        ));
        let bytes = message.to_vec().unwrap();
        assert_eq!(
            parse(&bytes, "example.com", 1, false).unwrap().addresses,
            vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))]
        );
        assert!(matches!(
            parse(&bytes, "example.com", 1, true),
            Err(ResolveError::Dnssec)
        ));
        assert!(matches!(
            parse(&bytes, "other.example", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.checking_disabled = true;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
    }

    #[test]
    fn rejects_bad_header_question_and_response_codes() {
        let mut message = response();
        message.metadata.id = 1;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.id = 0;
        message.metadata.message_type = MessageType::Query;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.message_type = MessageType::Response;
        message.metadata.op_code = OpCode::Status;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.op_code = OpCode::Query;
        message.metadata.truncation = true;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.truncation = false;
        message.queries.push(message.queries[0].clone());
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.queries.pop();
        message.queries[0].query_class = DNSClass::CH;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.queries[0].query_class = DNSClass::IN;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 28, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.metadata.response_code = ResponseCode::ServFail;
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::Unavailable)
        ));
        message.metadata.response_code = ResponseCode::NoError;
        let mut bytes = message.to_vec().unwrap();
        bytes[3] |= 0x40;
        assert!(matches!(
            parse(&bytes, "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
    }

    #[test]
    fn rejects_trailing_truncated_and_cyclic_compressed_messages() {
        let bytes = response().to_vec().unwrap();
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            parse(&trailing, "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        assert!(matches!(
            parse(&bytes[..bytes.len() - 1], "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        let cyclic = [
            0, 0, 0x81, 0x80, 0, 1, 0, 0, 0, 0, 0, 0, 0xc0, 0x0c, 0, 1, 0, 1,
        ];
        assert!(matches!(
            parse(&cyclic, "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
    }

    #[test]
    fn cname_chain_rejects_unrelated_addresses_and_cycles() {
        let owner = Name::from_ascii("example.com.").unwrap();
        let target = Name::from_ascii("alias.example.com.").unwrap();
        let mut message = response();
        message.metadata.authentic_data = true;
        message.answers.push(Record::from_rdata(
            owner.clone(),
            20,
            RData::CNAME(CNAME(target.clone())),
        ));
        message.answers.push(Record::from_rdata(
            target.clone(),
            60,
            RData::A(A(Ipv4Addr::new(1, 1, 1, 1))),
        ));
        let answer = parse(&message.to_vec().unwrap(), "example.com", 1, true).unwrap();
        assert_eq!(answer.ttl, 20);
        assert!(answer.authenticated);
        message.answers.push(Record::from_rdata(
            Name::from_ascii("unrelated.example.").unwrap(),
            10,
            RData::A(A(Ipv4Addr::new(8, 8, 8, 8))),
        ));
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
        message.answers.pop();
        message.answers.pop();
        message
            .answers
            .push(Record::from_rdata(target, 20, RData::CNAME(CNAME(owner))));
        assert!(matches!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false),
            Err(ResolveError::InvalidResponse)
        ));
    }

    #[test]
    fn ipv6_rrset_excludes_private_addresses() {
        let mut message = response();
        message.queries[0].query_type = RecordType::AAAA;
        let name = Name::from_ascii("example.com.").unwrap();
        message.answers.push(Record::from_rdata(
            name.clone(),
            40,
            RData::AAAA(AAAA("2606:4700:4700::1111".parse::<Ipv6Addr>().unwrap())),
        ));
        message.answers.push(Record::from_rdata(
            name,
            30,
            RData::AAAA(AAAA("fd00::1".parse::<Ipv6Addr>().unwrap())),
        ));
        let answer = parse(&message.to_vec().unwrap(), "example.com", 28, false).unwrap();
        assert_eq!(answer.addresses.len(), 1);
        assert_eq!(answer.unsafe_answers, 1);
        assert_eq!(answer.ttl, 30);
    }

    #[test]
    fn negative_cache_requires_soa_and_uses_minimum() {
        let mut message = response();
        message.metadata.response_code = ResponseCode::NXDomain;
        assert_eq!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false)
                .unwrap()
                .ttl,
            0
        );
        let soa = SOA::new(
            Name::from_ascii("ns.example.com.").unwrap(),
            Name::from_ascii("host.example.com.").unwrap(),
            1,
            2,
            3,
            4,
            30,
        );
        message.authorities.push(Record::from_rdata(
            Name::from_ascii("example.com.").unwrap(),
            90,
            RData::SOA(soa),
        ));
        assert_eq!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false)
                .unwrap()
                .ttl,
            30
        );
        message.metadata.response_code = ResponseCode::NoError;
        assert_eq!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false)
                .unwrap()
                .ttl,
            30
        );
        message.answers.push(Record::from_rdata(
            Name::from_ascii("example.com.").unwrap(),
            10,
            RData::CNAME(CNAME(Name::from_ascii("alias.example.com.").unwrap())),
        ));
        assert_eq!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false)
                .unwrap()
                .ttl,
            10
        );
        message.authorities.clear();
        let unrelated = SOA::new(
            Name::from_ascii("ns.other.net.").unwrap(),
            Name::from_ascii("host.other.net.").unwrap(),
            1,
            2,
            3,
            4,
            30,
        );
        message.authorities.push(Record::from_rdata(
            Name::from_ascii("other.net.").unwrap(),
            90,
            RData::SOA(unrelated),
        ));
        assert_eq!(
            parse(&message.to_vec().unwrap(), "example.com", 1, false)
                .unwrap()
                .ttl,
            0
        );
    }
}
