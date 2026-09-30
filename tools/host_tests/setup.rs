extern crate alloc;

#[path = "../../src/credentials.rs"]
mod credentials;
#[path = "../../src/setup/dns.rs"]
mod dns;
#[path = "../../src/setup/page.rs"]
mod page;

use credentials::*;
use page::*;

#[test]
fn credentials_round_trip() {
    for (ssid, pw) in [("Home", "hunter22"), ("Café ☕ 5G", ""), ("x", &"p".repeat(63)), (&*"s".repeat(32), "12345678")] {
        let c = Credentials::new(ssid, pw).unwrap();
        let rec = encode(&c);
        assert_eq!(decode(&rec), Some(c), "{ssid}");
    }
}

#[test]
fn credentials_reject_bad_records() {
    assert_eq!(decode(&[0xFF; RECORD_LEN]), None, "erased flash");
    assert_eq!(decode(&[0; RECORD_LEN]), None, "zeroed");
    let mut rec = encode(&Credentials::new("Home", "hunter22").unwrap());
    rec[10] ^= 1;
    assert_eq!(decode(&rec), None, "bit flip");
}

#[test]
fn credentials_validation() {
    assert!(Credentials::new("", "hunter22").is_err());
    assert!(Credentials::new(&"s".repeat(33), "hunter22").is_err());
    assert!(Credentials::new("Home", "short").is_err());
    assert!(Credentials::new("Home", &"p".repeat(64)).is_err());
    assert!(Credentials::new("Home", "").unwrap().is_open());
}

fn query(name: &str, qtype: u16, edns: bool) -> Vec<u8> {
    let mut q = vec![0xAB, 0xCD, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, u8::from(edns)];
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes());
    if edns {
        q.extend_from_slice(&[0, 0, 41, 0x10, 0, 0, 0, 0, 0, 0, 0]);
    }
    q
}

#[test]
fn dns_answers_a_with_our_ip() {
    let q = query("connectivitycheck.gstatic.com", 1, true);
    let mut out = [0u8; 512];
    let n = dns::answer(&q, [192, 168, 4, 1], &mut out).unwrap();
    let r = &out[..n];
    assert_eq!(&r[..2], &[0xAB, 0xCD], "same id");
    assert_eq!(r[2] & 0x80, 0x80, "is a response");
    assert_eq!(r[2] & 0x01, 0x01, "RD echoed");
    assert_eq!(r[3] & 0x0F, 0, "no error");
    assert_eq!(&r[4..12], &[0, 1, 0, 1, 0, 0, 0, 0], "1 question, 1 answer, no extras");
    let question_end = q.len() - 11; // minus the EDNS record
    assert_eq!(&r[12..question_end], &q[12..question_end], "question echoed");
    assert_eq!(&r[question_end..], &[0xC0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 192, 168, 4, 1]);
}

#[test]
fn dns_aaaa_gets_empty_answer_and_junk_is_ignored() {
    let q = query("captive.apple.com", 28, false);
    let mut out = [0u8; 512];
    let n = dns::answer(&q, [192, 168, 4, 1], &mut out).unwrap();
    assert_eq!(n, q.len());
    assert_eq!(&out[6..8], &[0, 0], "no answers");
    assert!(dns::answer(&[1, 2, 3], [0; 4], &mut out).is_none());
    let mut response = q.clone();
    response[2] |= 0x80;
    assert!(dns::answer(&response, [0; 4], &mut out).is_none(), "ignore responses");
    let truncated = &q[..q.len() - 3];
    assert!(dns::answer(truncated, [0; 4], &mut out).is_none());
}

#[test]
fn form_decoding() {
    let body = "ssid=My+Home%21&other=&password=p%40ss+w%C3%B6rd%25";
    assert_eq!(form_field(body, "ssid").as_deref(), Some("My Home!"));
    assert_eq!(form_field(body, "other").as_deref(), Some(""));
    assert_eq!(form_field(body, "password").as_deref(), Some("p@ss wörd%"));
    assert_eq!(form_field(body, "missing"), None);
    assert_eq!(form_field("a=%2", "a").as_deref(), Some("%2"), "dangling percent kept");
    assert_eq!(form_field("a=%zz", "a").as_deref(), Some("%zz"));
}

#[test]
fn page_escapes_ssids_and_sorts() {
    let nets = tidy_networks(vec![
        Network { ssid: "weak".into(), rssi: -85, open: true },
        Network { ssid: "<script>\"x\"".into(), rssi: -40, open: false },
        Network { ssid: "weak".into(), rssi: -60, open: true },
        Network { ssid: "".into(), rssi: -30, open: true },
    ]);
    assert_eq!(nets.len(), 2);
    assert_eq!(nets[0].ssid, "<script>\"x\"");
    assert_eq!(nets[1].rssi, -60, "kept the stronger duplicate");
    let html = setup_page(&nets, Some("Oops <b>"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;&quot;x&quot;"));
    assert!(html.contains("Oops &lt;b&gt;"));
    let _ = std::fs::write(std::env::temp_dir().join("setup_page.html"), &html);
    let _ = std::fs::write(std::env::temp_dir().join("saved_page.html"), saved_page("Home & Co"));
}

#[test]
fn empty_list_page_reloads_itself_until_networks_show_up() {
    let empty = setup_page(&[], None);
    assert!(empty.contains("Looking for networks"));
    assert!(empty.contains("location.reload()"));
    // The typed name is only a fallback, tucked away.
    assert!(empty.contains("<details><summary>Network not listed?"));

    let nets = [Network { ssid: "Home".into(), rssi: -50, open: false }];
    let listed = setup_page(&nets, None);
    assert!(!listed.contains("location.reload()"), "don't reload while choosing");
    assert!(!listed.contains("Looking for networks"));
    let _ = std::fs::write(std::env::temp_dir().join("setup_page_empty.html"), &empty);
}
