//! A tiny DNS responder that answers every name with our own address.
//!
//! That's what makes phones notice the captive portal and open the setup page.

const HEADER_LEN: usize = 12;
const TYPE_A: u16 = 1;
const CLASS_IN: u16 = 1;
const TTL_SECS: u32 = 60;
const ANSWER_LEN: usize = 16;

/// Writes a reply to `query` into `out` and returns its length.
///
/// A-record questions get `ip`; anything else (e.g. AAAA) gets an empty, successful reply.
/// Returns `None` for packets that aren't a single well-formed question.
pub fn answer(query: &[u8], ip: [u8; 4], out: &mut [u8]) -> Option<usize> {
    if query.len() < HEADER_LEN || query[2] & 0x80 != 0 {
        return None; // too short, or already a response
    }
    let questions = u16::from_be_bytes([query[4], query[5]]);
    if questions != 1 {
        return None;
    }

    // Walk the question's name: length-prefixed labels, ending with 0.
    let mut at = HEADER_LEN;
    loop {
        let len = usize::from(*query.get(at)?);
        at += 1;
        if len == 0 {
            break;
        }
        if len & 0xC0 != 0 {
            return None; // compression isn't used in questions
        }
        at += len;
    }
    let qtype = u16::from_be_bytes([*query.get(at)?, *query.get(at + 1)?]);
    let qclass = u16::from_be_bytes([*query.get(at + 2)?, *query.get(at + 3)?]);
    let question_end = at + 4;

    let answer_a = qtype == TYPE_A && qclass == CLASS_IN;
    let len = question_end + if answer_a { ANSWER_LEN } else { 0 };
    let out = out.get_mut(..len)?;

    // Header + question copied from the query (drops any trailing EDNS records).
    out[..question_end].copy_from_slice(&query[..question_end]);
    out[2] = 0x80 | 0x04 | (query[2] & 0x01); // response, authoritative, echo "recursion desired"
    out[3] = 0x80; // recursion available, no error
    out[6..8].copy_from_slice(&u16::from(answer_a).to_be_bytes()); // answers
    out[8..12].fill(0); // no authority / additional records

    if answer_a {
        let a = &mut out[question_end..];
        a[0..2].copy_from_slice(&[0xC0, HEADER_LEN as u8]); // name: pointer to the question
        a[2..4].copy_from_slice(&TYPE_A.to_be_bytes());
        a[4..6].copy_from_slice(&CLASS_IN.to_be_bytes());
        a[6..10].copy_from_slice(&TTL_SECS.to_be_bytes());
        a[10..12].copy_from_slice(&4u16.to_be_bytes());
        a[12..16].copy_from_slice(&ip);
    }
    Some(len)
}
