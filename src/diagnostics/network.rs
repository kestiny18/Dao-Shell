//! Explicit, one-use local endpoints. No model URL/host/port is accepted by the probe tool.
use crate::core::{Cancellation, now};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Endpoints {
    entries: HashMap<String, Endpoint>,
}
struct Endpoint {
    host: String,
    port: u16,
    local: bool,
    checks: Vec<String>,
    created: Instant,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Register {
    host: String,
    port: u16,
    scope: String,
    checks: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    mode: String,
    #[serde(default)]
    endpoint_refs: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
}
impl Endpoints {
    pub fn register(&mut self, args: Value) -> Result<Value> {
        let r: Register =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        ensure!(
            matches!(r.scope.as_str(), "public" | "local")
                && r.port > 0
                && valid_host(&r.host)
                && !r.checks.is_empty()
                && r.checks.len() <= 3
                && r.checks
                    .iter()
                    .all(|c| matches!(c.as_str(), "dns" | "tcp" | "https")),
            "invalid_arguments"
        );
        self.entries
            .retain(|_, e| e.created.elapsed() < Duration::from_secs(180));
        ensure!(self.entries.len() < 3, "endpoint_budget");
        let id = uuid::Uuid::new_v4().to_string();
        self.entries.insert(
            id.clone(),
            Endpoint {
                host: r.host.clone(),
                port: r.port,
                local: r.scope == "local",
                checks: r.checks.clone(),
                created: Instant::now(),
            },
        );
        Ok(
            json!({"endpoint_ref":id,"host":r.host,"port":r.port,"scope":r.scope,"checks":r.checks,"expires_seconds":180,"one_use":true,"network_path":"system DNS; direct TCP/HTTPS with proxy disabled; no redirect/auth/credentials"}),
        )
    }
    pub fn query(&mut self, args: Value, cancel: &Cancellation) -> Result<Value> {
        let r: Request =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        ensure!(
            matches!(r.mode.as_str(), "local" | "probe"),
            "invalid_arguments"
        );
        if r.mode == "local" {
            ensure!(
                r.endpoint_refs.is_empty() && r.checks.is_empty(),
                "invalid_arguments"
            );
            let mut local = super::coverage::network_config(cancel)?;
            local["approved_endpoints"] = json!(
                self.entries
                    .iter()
                    .filter(|(_, e)| e.created.elapsed() < Duration::from_secs(180))
                    .map(|(id, e)| json!({"endpoint_ref":id,"checks":e.checks}))
                    .collect::<Vec<_>>()
            );
            return Ok(envelope(local));
        }
        ensure!(
            !r.endpoint_refs.is_empty()
                && r.endpoint_refs.len() <= 3
                && !r.checks.is_empty()
                && r.checks.len() <= 3
                && r.checks
                    .iter()
                    .all(|c| matches!(c.as_str(), "dns" | "tcp" | "https")),
            "invalid_arguments"
        );
        ensure!(
            r.endpoint_refs
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == r.endpoint_refs.len(),
            "invalid_arguments"
        );
        // Validate the entire batch before consuming any reference or performing DNS.
        for id in &r.endpoint_refs {
            let e = self
                .entries
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("endpoint_confirmation_required"))?;
            ensure!(
                e.created.elapsed() < Duration::from_secs(180)
                    && r.checks.iter().all(|c| e.checks.contains(c)),
                "endpoint_confirmation_required"
            );
        }
        let began = Instant::now();
        let mut results = Vec::new();
        for (i, id) in r.endpoint_refs.iter().enumerate() {
            let e = self.entries.remove(id).unwrap();
            if cancel.is_cancelled() {
                break;
            }
            if began.elapsed() > Duration::from_secs(20) {
                results.push(
                    json!({"label":format!("Target {}",i+1),"error":"total_budget_exceeded"}),
                );
                continue;
            }
            results.push(probe(&e, i + 1, &r.checks, cancel, began));
        }
        Ok(envelope(
            json!({"mode":"probe","targets":results,"elapsed_ms":began.elapsed().as_millis(),"stopped":cancel.is_cancelled(),"network_path":"system DNS; direct TCP and HTTPS; proxy explicitly disabled","limits":"Endpoint-only evidence; DNS does not prove TCP; TCP does not prove HTTPS or service health. No redirects or authentication."}),
        ))
    }
}
fn envelope(data: Value) -> Value {
    json!({"schema_version":2,"observation_id":uuid::Uuid::new_v4().to_string(),"kind":"network_diagnose","status":"complete","observed_at":now(),"data":data,"limits":"User-selected endpoint only. Direct path is not a system-proxy or VPN diagnosis; failures do not prove machine-wide disconnection."})
}
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.is_ascii()
        && !host.contains(['/', ':', '@', '?', '#', '\\', ' ', '\n', '\r'])
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}
/// Conservative globally routable scope; mapped IPv4 gets IPv4 rules too.
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let b = a.octets();
            !a.is_private()
                && !a.is_loopback()
                && !a.is_link_local()
                && !a.is_multicast()
                && !a.is_unspecified()
                && b[0] != 0
                && b[0] < 224
                && b[0] != 127
                && !(b[0] == 100 && (64..=127).contains(&b[1]))
                && !(b[0] == 192 && (b[1] == 0 || b[1] == 168))
                && !(b[0] == 198 && (b[1] == 18 || b[1] == 19 || b[1] == 51))
                && !(b[0] == 203 && b[1] == 0)
                && b != [255, 255, 255, 255]
        }
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return public_ip(v4.into());
            }
            let s = a.segments();
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2001
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && (s[1] & 0xf000) == 0)
        }
    }
}
fn allowed(ip: IpAddr, local: bool) -> bool {
    if let IpAddr::V6(ip) = ip
        && let Some(v4) = ip.to_ipv4_mapped()
    {
        return allowed(v4.into(), local);
    }
    let metadata = ip == "169.254.169.254".parse::<IpAddr>().unwrap()
        || ip == "100.100.100.200".parse::<IpAddr>().unwrap();
    !metadata
        && (if local {
            match ip {
                IpAddr::V4(a) => !a.is_unspecified() && !a.is_multicast() && !a.is_broadcast(),
                IpAddr::V6(a) => !a.is_unspecified() && !a.is_multicast(),
            }
        } else {
            public_ip(ip)
        })
}
static DNS_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>> {
    ensure!(
        DNS_IN_FLIGHT
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 3)
                .then_some(n + 1))
            .is_ok(),
        "dns_worker_budget"
    );
    let host = host.to_owned();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (host.as_str(), port)
            .to_socket_addrs()
            .map(|it| it.take(32).collect::<Vec<_>>());
        DNS_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|_| anyhow::anyhow!("dns_timeout"))?
        .map_err(|_| anyhow::anyhow!("dns_failed"))
}
fn probe(
    e: &Endpoint,
    index: usize,
    checks: &[String],
    cancel: &Cancellation,
    total: Instant,
) -> Value {
    let mut out = json!({"label":format!("Target {index}"),"local_host":e.host,"local_port":e.port,"scope":if e.local{"local"}else{"public"},"layers":[],"network_path":"direct_no_proxy"});
    let began = Instant::now();
    let resolved = resolve(&e.host, e.port);
    let dns_ms = began.elapsed().as_millis();
    let mut layers = Vec::new();
    let ips = match resolved {
        Ok(ips) if !ips.is_empty() => ips,
        _ => {
            layers.push(json!({"layer":"dns","status":"failed","code":"system_dns_failed_or_timeout","elapsed_ms":dns_ms}));
            out["layers"] = json!(layers);
            return out;
        }
    };
    if ips.iter().any(|s| !allowed(s.ip(), e.local)) {
        layers.push(json!({"layer":"dns","status":"blocked","code":"resolved_scope_change","elapsed_ms":dns_ms}));
        out["layers"] = json!(layers);
        return out;
    }
    layers.push(json!({"layer":"dns","status":"ok","elapsed_ms":dns_ms,"address_count":ips.len()}));
    out["local_addresses"] = json!(ips);
    for layer in ["tcp", "https"] {
        if !checks.iter().any(|c| c == layer) {
            continue;
        }
        if cancel.is_cancelled() || total.elapsed() > Duration::from_secs(20) {
            layers.push(json!({"layer":layer,"status":"not_run","code":"cancelled_or_budget"}));
            break;
        }
        // Repeat system resolution immediately before connecting; pin that checked set in HTTPS.
        let checked = match resolve(&e.host, e.port) {
            Ok(v) if !v.is_empty() && v.iter().all(|a| allowed(a.ip(), e.local)) => v,
            _ => {
                layers.push(
                    json!({"layer":layer,"status":"blocked","code":"resolution_scope_changed"}),
                );
                continue;
            }
        };
        if cancel.is_cancelled() || total.elapsed() > Duration::from_secs(20) {
            layers.push(json!({"layer":layer,"status":"not_run","code":"cancelled_or_budget"}));
            break;
        }
        let start = Instant::now();
        let result = if layer == "tcp" {
            match TcpStream::connect_timeout(&checked[0], Duration::from_secs(3)) {
                Ok(_) => json!({"layer":layer,"status":"ok"}),
                Err(error) => {
                    json!({"layer":layer,"status":"failed","code":match error.kind(){std::io::ErrorKind::ConnectionRefused=>"connection_refused",std::io::ErrorKind::TimedOut=>"tcp_timeout",_=>"tcp_failed"}})
                }
            }
        } else {
            let host = e.host.clone();
            let port = e.port;
            let handle = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(https_head(host, port, checked, reqwest::Client::builder()))
            });
            handle.join().unwrap_or_else(
                |_| json!({"layer":"https","status":"failed","code":"provider_failed"}),
            )
        };
        if cancel.is_cancelled() {
            layers
                .push(json!({"layer":layer,"status":"unknown","code":"cancelled_during_request"}));
            break;
        }
        let mut result = result;
        result["elapsed_ms"] = json!(start.elapsed().as_millis());
        layers.push(result);
    }
    out["layers"] = json!(layers);
    out
}
async fn https_head(
    host: String,
    port: u16,
    checked: Vec<SocketAddr>,
    builder: reqwest::ClientBuilder,
) -> Value {
    let builder = builder
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(3))
        .resolve_to_addrs(&host, &checked);
    let client = match builder.build() {
        Ok(c) => c,
        Err(_) => {
            return json!({"layer":"https","status":"failed","code":"https_client_unavailable"});
        }
    };
    match client.head(format!("https://{host}:{port}/")).send().await {
        Ok(response) => {
            json!({"layer":"https","status":"response","http_status":response.status().as_u16(),"redirect_followed":false,"redirect_requires_new_confirmation":response.status().is_redirection(),"proxy_used":false})
        }
        Err(error) => {
            json!({"layer":"https","status":"failed","code":if error.is_timeout(){"https_timeout"}else if error.is_connect(){"tls_or_connect_failed"}else{"https_failed"},"proxy_used":false})
        }
    }
}
pub fn projection(data: &Value) -> Value {
    if data["mode"] == "probe" {
        json!({"mode":"probe","elapsed_ms":data["elapsed_ms"],"stopped":data["stopped"],"network_path":data["network_path"],"targets":data["targets"].as_array().into_iter().flatten().map(|t|json!({"label":t["label"],"scope":t["scope"],"layers":t["layers"],"error":t["error"]})).collect::<Vec<_>>()})
    } else {
        json!({"mode":"local","interface_count":data["interface_count"],"has_default_route":data["has_default_route"],"has_dns_configuration":data["has_dns_configuration"],"proxy_config_present":data["proxy_config_present"],"coverage":data["coverage"],"approved_endpoints":data["approved_endpoints"]})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_endpoint_and_reserved_ranges() {
        for host in [
            "https://example.com/",
            "user@example.com",
            "localhost?x",
            "a/b",
            "::1",
            "a\n",
        ] {
            assert!(!valid_host(host));
        }
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "ff02::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        assert!(!allowed("169.254.169.254".parse().unwrap(), true));
    }
    #[test]
    fn mixed_dns_scopes_mapped_metadata_and_projection() {
        let addresses: Vec<SocketAddr> = vec![
            "8.8.8.8:443".parse().unwrap(),
            "10.0.0.1:443".parse().unwrap(),
        ];
        assert!(!addresses.iter().all(|a| allowed(a.ip(), false)));
        assert!(!allowed("::ffff:169.254.169.254".parse().unwrap(), true));
        let p = projection(
            &json!({"mode":"probe","targets":[{"label":"Target 1","local_host":"secret.internal","local_addresses":["10.0.0.1"],"layers":[{"layer":"dns","status":"ok"}]}]}),
        );
        assert!(!p.to_string().contains("secret"));
        assert!(!p.to_string().contains("10.0.0.1"));
    }
    #[test]
    fn controlled_tcp_and_tls_failure_and_one_use() {
        use std::io::Write;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            drop(socket);
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let mut endpoints = Endpoints::default();
        let value=endpoints.register(json!({"host":"127.0.0.1","port":port,"scope":"local","checks":["dns","tcp","https"]})).unwrap();
        let args = json!({"mode":"probe","endpoint_refs":[value["endpoint_ref"]],"checks":["dns","tcp","https"]});
        let v = endpoints
            .query(args.clone(), &Cancellation::default())
            .unwrap();
        thread.join().unwrap();
        let layers = v["data"]["targets"][0]["layers"].as_array().unwrap();
        assert_eq!(layers[1]["status"], "ok");
        assert_eq!(layers[2]["status"], "failed");
        assert_eq!(layers[2]["proxy_used"], false);
        assert!(endpoints.query(args, &Cancellation::default()).is_err());
    }
    #[test]
    fn pinned_https_head_no_redirect_no_proxy_and_no_dns_fallback() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let tls = native_tls::Identity::from_pkcs12(
            include_bytes!("../../tests/fixtures/diagnostic-tls/identity.pfx"),
            "synthetic-fixture-only",
        )
        .unwrap();
        let acceptor = native_tls::TlsAcceptor::new(tls).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut stream = acceptor.accept(socket).unwrap();
            let mut bytes = [0; 8192];
            let n = stream.read(&mut bytes).unwrap();
            let text = String::from_utf8_lossy(&bytes[..n]);
            assert!(text.starts_with("HEAD / HTTP/1.1"));
            assert!(!text.to_lowercase().contains("authorization:"));
            assert!(!text.to_lowercase().contains("cookie:"));
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://169.254.169.254/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let root = reqwest::Certificate::from_pem(include_bytes!(
            "../../tests/fixtures/diagnostic-tls/cert.pem"
        ))
        .unwrap();
        let builder = reqwest::Client::builder()
            .add_root_certificate(root)
            .proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap());
        let value = tokio::runtime::Runtime::new().unwrap().block_on(https_head(
            "probe.dao.invalid".into(),
            addr.port(),
            vec![addr],
            builder,
        ));
        server.join().unwrap();
        assert_eq!(value["http_status"], 302);
        assert_eq!(value["redirect_followed"], false);
        assert_eq!(value["redirect_requires_new_confirmation"], true);
        assert_eq!(value["proxy_used"], false);
    }
    #[test]
    fn forged_ref_or_added_check_makes_no_request() {
        let mut e = Endpoints::default();
        let c = Cancellation::default();
        assert!(
            e.query(
                json!({"mode":"probe","endpoint_refs":["forged"],"checks":["tcp"]}),
                &c
            )
            .is_err()
        );
        let v = e
            .register(json!({"host":"localhost","port":12345,"scope":"local","checks":["dns"]}))
            .unwrap();
        assert!(
            e.query(
                json!({"mode":"probe","endpoint_refs":[v["endpoint_ref"]],"checks":["tcp"]}),
                &c
            )
            .is_err()
        );
    }
}
