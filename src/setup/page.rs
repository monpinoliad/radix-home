//! The setup web page, and parsing what it posts back.

use alloc::{string::String, vec::Vec};
use core::fmt::Write;

/// A network found by the Wi-Fi scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    /// dBm; closer to 0 is stronger.
    pub rssi: i8,
    pub open: bool,
}

/// Sorts strongest first and drops duplicates (mesh / multi-AP networks share a name).
pub fn tidy_networks(mut networks: Vec<Network>) -> Vec<Network> {
    networks.retain(|n| !n.ssid.is_empty());
    networks.sort_by_key(|n| core::cmp::Reverse(n.rssi));
    let mut seen: Vec<Network> = Vec::new();
    for network in networks {
        if !seen.iter().any(|n| n.ssid == network.ssid) {
            seen.push(network);
        }
    }
    seen
}

// Pixel look, all served by the board (the phone has no internet while on the hotspot):
// `pixel.ttf` is the clock's 5x7 font and `sun.png` the weather sun (tools/gen_sprites.py).
// The font is sized in multiples of 8px so every font pixel lands on whole screen pixels;
// borders are 4px "pixel" steps drawn with box-shadows. Network names and inputs use a plain
// monospace font, because the pixel font only has capitals and SSIDs are case-sensitive.
const STYLE: &str = "@font-face{font-family:Px;src:url(pixel.ttf) format('truetype')}\
*{box-sizing:border-box}\
html{background:#b3dfff}\
body{margin:0;min-height:100vh;color:#1c2b60;font:16px/24px Px,ui-monospace,monospace;\
-webkit-font-smoothing:none;background:linear-gradient(#4aa3f0 0 96px,#62b3f6 0 192px,\
#7cc2fa 0 288px,#97d0fd 0 384px,#b3dfff 0) no-repeat}\
main{max-width:416px;margin:0 auto;padding:16px 16px 96px}\
.sun{width:108px;height:108px;margin:8px auto 0;image-rendering:pixelated;\
background:url(sun.png) 0 0/100% 100%}\
.hop{margin-top:48px;animation:hop 1.2s steps(1) infinite}\
@keyframes hop{0%{transform:none}15%{transform:translateY(-12px)}30%{transform:translateY(-36px)}\
45%{transform:translateY(-48px)}60%{transform:translateY(-24px)}75%{transform:none}}\
h1{margin:8px 0 0;font-size:32px;line-height:40px;font-weight:400;text-align:center;color:#fff;\
text-shadow:4px 4px 0 #1c2b60}\
.sub{margin:8px 0 24px;text-align:center}\
.box{margin:0 4px 24px;padding:16px;background:#fff;\
box-shadow:4px 0 #1c2b60,-4px 0 #1c2b60,0 4px #1c2b60,0 -4px #1c2b60,4px 8px #7cc2fa,-4px 8px #7cc2fa}\
.err{margin:0 4px 24px;padding:8px 12px;background:#ffd6e6;color:#8a1c2b;\
box-shadow:4px 0 #8a1c2b,-4px 0 #8a1c2b,0 4px #8a1c2b,0 -4px #8a1c2b}\
.net{display:flex;align-items:center;gap:10px;padding:10px 6px;border-bottom:4px dotted #cfe6fb;cursor:pointer}\
.net:has(input:checked){background:#ffe3ef}\
.net input{accent-color:#e0609a;width:16px;height:16px;margin:0}\
.ssid{flex:1;overflow-wrap:anywhere;font:15px/20px ui-monospace,Menlo,Consolas,monospace}\
.pick{margin:8px 0 16px;padding:8px 12px;background:#ffe3ef;overflow-wrap:anywhere;\
font:700 18px/24px ui-monospace,Menlo,Consolas,monospace}\
.nb{white-space:nowrap}\
.lock{flex:none}\
.bars{display:inline-flex;align-items:flex-end;gap:2px;height:16px;flex:none}\
.bars i{width:4px;background:#d6e4f2}.bars i:nth-child(1){height:4px}.bars i:nth-child(2){height:8px}\
.bars i:nth-child(3){height:12px}.bars i:nth-child(4){height:16px}\
.b1 i:nth-child(-n+1),.b2 i:nth-child(-n+2),.b3 i:nth-child(-n+3),.b4 i{background:#1c2b60}\
label.f{display:block;margin:20px 0 8px}\
summary{margin-top:20px;cursor:pointer;text-decoration:underline;text-underline-offset:4px}\
input[type=text],input[type=password]{width:100%;padding:10px 12px;border:0;border-radius:0;\
background:#f2f8ff;color:#1c2b60;font:16px/24px ui-monospace,Menlo,Consolas,monospace;\
box-shadow:inset 0 0 0 4px #97d0fd}\
input[type=text]:focus,input[type=password]:focus{outline:0;box-shadow:inset 0 0 0 4px #e0609a}\
.show{display:flex;align-items:center;gap:8px;margin-top:12px}.show input{accent-color:#e0609a}\
button{display:block;width:100%;margin-top:24px;padding:12px;border:0;border-radius:0;cursor:pointer;\
font:16px/24px Px,monospace;color:#fff;background:#e0609a;\
box-shadow:inset -4px -4px 0 #b8407a,4px 0 #1c2b60,-4px 0 #1c2b60,0 4px #1c2b60,0 -4px #1c2b60}\
button:active{transform:translateY(4px);box-shadow:inset 4px 4px 0 #b8407a,4px 0 #1c2b60,\
-4px 0 #1c2b60,0 4px #1c2b60,0 -4px #1c2b60}\
.ghost{margin-top:0;background:none;color:#1c2b60;box-shadow:none;text-decoration:underline;\
text-underline-offset:4px}.ghost:active{transform:none;box-shadow:none}\
.ground{position:fixed;left:0;right:0;bottom:0;height:48px;background:#7cc26b;box-shadow:0 -4px #5c9a4e}";

/// Pixel padlock (7x8 art pixels at 2x).
const LOCK: &str = "<svg class=lock width=14 height=16 viewBox=\"0 0 7 8\" shape-rendering=crispEdges \
aria-label=secured><path fill=#1c2b60 d=\"M2 0h3v1h1v2H5V1H2v2H1V1h1zM0 3h7v5H0z\"/>\
<path fill=#fff d=\"M3 4h1v2H3z\"/></svg>";

fn page_start(out: &mut String, title: &str) {
    out.push_str(
        "<!doctype html><html lang=en><head><meta charset=utf-8>\
         <meta name=viewport content=\"width=device-width,initial-scale=1\"><title>",
    );
    escape_into(out, title);
    out.push_str("</title><style>");
    out.push_str(STYLE);
    out.push_str("</style></head><body><main>");
}

fn page_end(out: &mut String) {
    out.push_str("</main><div class=ground></div></body></html>");
}

/// 1-4 bars from the signal strength.
fn signal_bars(rssi: i8) -> u8 {
    match rssi {
        -55.. => 4,
        -67..=-56 => 3,
        -78..=-68 => 2,
        _ => 1,
    }
}

/// While the scan hasn't found anything, reload every few seconds so the list fills in by
/// itself - unless the user is typing into the form.
const RELOAD_UNTIL_FOUND: &str = "<script>setTimeout(function r(){var a=document.activeElement;\
if((a&&a.tagName=='INPUT')||other.value||pw.value)setTimeout(r,4000);else location.reload()},4000)\
</script>";

/// The form: pick a scanned network and enter its password (a hidden network can be typed in).
pub fn setup_page(networks: &[Network], error: Option<&str>) -> String {
    let mut out = String::with_capacity(6144);
    page_start(&mut out, "Clock Wi-Fi setup");
    out.push_str(
        "<div class=sun role=img aria-label=Sun></div>\
         <h1>Wi-Fi setup</h1><p class=sub>Pick a network for the clock.</p>",
    );
    if let Some(error) = error {
        out.push_str("<p class=err>");
        escape_into(&mut out, error);
        out.push_str("</p>");
    }
    out.push_str("<form class=box method=post action=save>");
    if networks.is_empty() {
        out.push_str("<p>Looking for networks... This page updates by itself.</p>");
    }
    for (i, network) in networks.iter().enumerate() {
        out.push_str("<label class=net><input type=radio name=ssid value=\"");
        escape_into(&mut out, &network.ssid);
        out.push('"');
        if i == 0 {
            out.push_str(" checked");
        }
        out.push_str("><span class=ssid>");
        escape_into(&mut out, &network.ssid);
        out.push_str("</span>");
        if !network.open {
            out.push_str(LOCK);
        }
        let _ = write!(
            out,
            "<span class=\"bars b{}\" aria-label=signal><i></i><i></i><i></i><i></i></span></label>",
            signal_bars(network.rssi)
        );
    }
    out.push_str(
        "<label class=f for=pw>Password</label>\
         <input type=password id=pw name=password maxlength=63 autocomplete=off>\
         <label class=show><input type=checkbox onchange=\"pw.type=this.checked?'text':'password'\">\
         Show password</label>\
         <details><summary>Network not listed?</summary>\
         <label class=f for=other>Type its name</label>\
         <input type=text id=other name=other maxlength=32 autocapitalize=off autocomplete=off \
         spellcheck=false></details>\
         <button>Save &amp; connect</button></form>\
         <form method=post action=rescan><button class=ghost>Scan again</button></form>",
    );
    if networks.is_empty() {
        out.push_str(RELOAD_UNTIL_FOUND);
    }
    page_end(&mut out);
    out
}

pub fn saved_page(ssid: &str) -> String {
    let mut out = String::with_capacity(4096);
    page_start(&mut out, "Saved!");
    out.push_str(
        "<div class=\"sun hop\" role=img aria-label=Sun></div>\
         <h1>Saved!</h1><p class=sub>The clock is restarting.</p>\
         <div class=box><p>It will join</p><p class=pick>",
    );
    escape_into(&mut out, ssid);
    out.push_str(
        "</p><p>You can close this page.</p><p>If the clock still shows <span class=nb>--:--</span> \
         after a minute, the password was probably wrong. Radix-Setup will come back so you can \
         try again.</p>\
         </div>",
    );
    page_end(&mut out);
    out
}

pub fn escape_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

/// Value of `name` in an `application/x-www-form-urlencoded` body.
pub fn form_field(body: &str, name: &str) -> Option<String> {
    body.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (url_decode(key) == name).then(|| url_decode(value))
    })
}

fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi << 4 | lo);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
