use crate::{model::Domain, state::AppState};
use hickory_resolver::{
    error::{ResolveError, ResolveErrorKind},
    proto::op::ResponseCode,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Serialize, Clone)]
pub struct DnsRecord {
    pub kind: &'static str,
    pub name: String,
    pub value: String,
}
#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct Checks {
    pub spf: bool,
    pub dkim: bool,
    pub dmarc: bool,
    pub ptr: bool,
    pub forward: bool,
}
impl Checks {
    pub fn verified(&self) -> bool {
        self.spf && self.dkim && self.dmarc && self.forward
    }
}

pub fn records(state: &AppState, domain: &Domain) -> Vec<DnsRecord> {
    let ip = state.config.public_ip;
    let mut records = vec![
        DnsRecord {
            kind: "TXT",
            name: format!("{}._domainkey.{}", domain.selector, domain.name),
            value: format!("v=DKIM1; k=rsa; p={}", domain.public_key),
        },
        DnsRecord {
            kind: "TXT",
            name: domain.name.clone(),
            value: match ip {
                Some(ip) => format!(
                    "v=spf1 {}:{ip} -all",
                    if ip.is_ipv4() { "ip4" } else { "ip6" }
                ),
                None => "PUBLIC_IP is not configured".into(),
            },
        },
        DnsRecord {
            kind: "TXT",
            name: format!("_dmarc.{}", domain.name),
            value: "v=DMARC1; p=none".into(),
        },
    ];
    if let Some(ip) = ip {
        records.push(DnsRecord {
            kind: if ip.is_ipv4() { "A" } else { "AAAA" },
            name: state.config.mail_hostname.clone(),
            value: ip.to_string(),
        });
        records.push(DnsRecord {
            kind: "PTR",
            name: ip.to_string(),
            value: state.config.mail_hostname.clone(),
        });
    }
    records
}

fn absent(error: &ResolveError) -> bool {
    matches!(
        error.kind(),
        ResolveErrorKind::NoRecordsFound {
            response_code: ResponseCode::NoError | ResponseCode::NXDomain,
            ..
        }
    )
}

async fn txt(state: &AppState, name: &str) -> anyhow::Result<Vec<String>> {
    match state.dns.txt_lookup(name).await {
        Ok(rows) => Ok(rows
            .iter()
            .map(|row| {
                row.txt_data()
                    .iter()
                    .map(|chunk| String::from_utf8_lossy(chunk))
                    .collect()
            })
            .collect()),
        Err(e) if absent(&e) => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

pub async fn identity(state: &AppState) -> anyhow::Result<(bool, bool)> {
    let Some(ip) = state.config.public_ip else {
        return Ok((false, false));
    };
    let ptr = match state.dns.reverse_lookup(ip).await {
        Ok(names) => names.iter().any(|name| {
            name.to_utf8()
                .trim_end_matches('.')
                .eq_ignore_ascii_case(&state.config.mail_hostname)
        }),
        Err(e) if absent(&e) => false,
        Err(_) => false,
    };
    let forward = match state.dns.lookup_ip(&state.config.mail_hostname).await {
        Ok(addresses) => addresses.iter().any(|address| address == ip),
        Err(e) if absent(&e) => false,
        Err(e) => return Err(e.into()),
    };
    Ok((ptr, forward))
}

pub fn tag<'a>(record: &'a str, name: &str) -> Option<&'a str> {
    record.split(';').find_map(|item| {
        let (key, value) = item.trim().split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then_some(value.trim())
    })
}

pub fn spf_matches(records: &[String], ip: IpAddr) -> bool {
    let spf: Vec<_> = records
        .iter()
        .filter(|s| s.starts_with("v=spf1 "))
        .collect();
    if spf.len() != 1 {
        return false;
    }
    let tokens: Vec<_> = spf[0].split_whitespace().collect();
    // Deliberately limited to the explicit IP policy shown in the dashboard; no unsafe include approximation.
    tokens.len() == 3
        && tokens[1] == format!("{}:{ip}", if ip.is_ipv4() { "ip4" } else { "ip6" })
        && matches!(tokens[2], "-all" | "~all")
}

pub async fn verify(state: &AppState, domain: &Domain) -> anyhow::Result<Checks> {
    let dkim_name = format!("{}._domainkey.{}", domain.selector, domain.name);
    let dmarc_name = format!("_dmarc.{}", domain.name);
    let (spf, dkim, dmarc, (ptr, forward)) = tokio::try_join!(
        txt(state, &domain.name),
        txt(state, &dkim_name),
        txt(state, &dmarc_name),
        identity(state)
    )?;
    let dmarc: Vec<_> = dmarc
        .iter()
        .filter(|r| tag(r, "v") == Some("DMARC1"))
        .collect();
    Ok(Checks {
        spf: state
            .config
            .public_ip
            .is_some_and(|ip| spf_matches(&spf, ip)),
        dkim: dkim.len() == 1
            && tag(&dkim[0], "p")
                .is_some_and(|p| p.split_whitespace().collect::<String>() == domain.public_key)
            && tag(&dkim[0], "k").is_none_or(|k| k == "rsa")
            && tag(&dkim[0], "t").is_none_or(|flags| !flags.split(':').any(|f| f == "y")),
        dmarc: dmarc.len() == 1
            && matches!(tag(dmarc[0], "p"), Some("none" | "quarantine" | "reject")),
        ptr,
        forward,
    })
}

/// Connect only to validated public addresses, never re-resolve the MX hostname in the SMTP client.
pub fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            // Global unicast only; excludes ULA, link-local, mapped IPv4 and transition prefixes.
            (segments[0] & 0xe000) == 0x2000
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] < 0x0200 || segments[1] == 0x0db8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ptr_is_advisory_but_other_checks_remain_required() {
        let mut checks = Checks {
            spf: true,
            dkim: true,
            dmarc: true,
            forward: true,
            ptr: false,
        };
        assert!(checks.verified());
        checks.forward = false;
        assert!(!checks.verified());
        checks.forward = true;
        checks.spf = false;
        assert!(!checks.verified());
    }
    #[test]
    fn blocks_private_mx_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:8.8.8.8",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_address("8.8.8.8".parse().unwrap()));
        assert!(public_address("2606:4700:4700::1111".parse().unwrap()));
    }
    #[test]
    fn spf_does_not_accept_unrestricted_or_ambiguous_records() {
        let ip = "8.8.8.8".parse().unwrap();
        assert!(spf_matches(&["v=spf1 ip4:8.8.8.8 -all".into()], ip));
        for record in [
            "v=spf1 +all",
            "v=spf1 ip4:8.8.8.8 +all",
            "v=spf1 -all ip4:8.8.8.8",
            "v=spf1 include:unknown.test -all",
        ] {
            assert!(!spf_matches(&[record.into()], ip));
        }
    }
}
