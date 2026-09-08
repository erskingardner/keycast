//! Connections to metadata-supplied destinations never resolve DNS twice.
//! Validate every answer, connect to a vetted IP, then use the original hostname
//! for TLS verification and the HTTP upgrade. Redirects and proxies are disallowed.
use async_wsocket::futures_util::{Sink, StreamExt};
use nostr_sdk::{
    error::Error,
    transport::websocket::{WebSocketSink, WebSocketStream},
};
use std::{net::IpAddr, time::Duration};
use url::Url;

pub fn normalize(input: &str) -> Option<String> {
    if input.len() > 512 {
        return None;
    }
    let url = Url::parse(input).ok()?;
    if url.scheme() != "wss"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return None;
    }
    let host = url.host_str()?.trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") {
        return None;
    }
    if let Ok(ip) = host.parse() {
        if !public_ip(ip) {
            return None;
        }
    }
    Some(
        nostr::types::RelayUrl::parse(url.as_str())
            .ok()?
            .to_string()
            .trim_end_matches('/')
            .to_owned(),
    )
}
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            // Only ordinary global unicast. Excludes mapped IPv4, NAT64, ULA,
            // link-local, multicast, documentation, Teredo and 6to4 tunnels.
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}
pub async fn connect(url: &Url) -> Result<(WebSocketSink, WebSocketStream), Error> {
    if normalize(url.as_str()).is_none() {
        return Err(Error::policy("discovered relay destination forbidden"));
    }
    tokio::time::timeout(Duration::from_secs(8), async {
        let host = url
            .host_str()
            .ok_or_else(|| Error::policy("relay host missing"))?
            .trim_matches(['[', ']']);
        let addresses: Vec<_> = tokio::net::lookup_host((host, 443))
            .await?
            .take(17)
            .collect();
        if addresses.is_empty()
            || addresses.len() > 16
            || addresses.iter().any(|a| !public_ip(a.ip()))
        {
            return Err(Error::policy("discovered relay DNS destination forbidden"));
        }
        let mut socket = None;
        for address in addresses {
            if let Ok(Ok(stream)) = tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::TcpStream::connect(address),
            )
            .await
            {
                socket = Some(stream);
                break;
            }
        }
        let socket = socket.ok_or_else(|| Error::transport("relay TCP connection failed"))?;
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(512 * 1024))
            .max_frame_size(Some(512 * 1024));
        let (ws, _) = tokio_tungstenite::client_async_tls_with_config(
            url.as_str(),
            socket,
            Some(config),
            None,
        )
        .await
        .map_err(Error::transport)?;
        let (sink, stream) = ws.split();
        Ok((
            Box::pin(PublicSink(sink)) as WebSocketSink,
            Box::pin(stream.map(|r| {
                use async_wsocket::Message as A;
                use tokio_tungstenite::tungstenite::Message as M;
                Ok(match r.map_err(Error::transport)? {
                    M::Text(v) => A::Text(v.to_string()),
                    M::Binary(v) => A::Binary(v.to_vec()),
                    M::Ping(v) => A::Ping(v.to_vec()),
                    M::Pong(v) => A::Pong(v.to_vec()),
                    M::Close(v) => A::Close(v.map(Into::into)),
                    M::Frame(_) => return Err(Error::transport("unexpected raw frame")),
                })
            })) as WebSocketStream,
        ))
    })
    .await
    .map_err(|_| Error::transport("relay connection timed out"))?
}
// Explicit conversion permits shutdown after transport errors. SinkMapErr consumes
// its conversion closure on the first error and may panic if the sink is reused.
struct PublicSink<S>(S);
impl<S> Sink<async_wsocket::Message> for PublicSink<S>
where
    S: Sink<tokio_tungstenite::tungstenite::Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    type Error = Error;
    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
        std::pin::Pin::new(&mut self.0)
            .poll_ready(cx)
            .map_err(Error::transport)
    }
    fn start_send(
        mut self: std::pin::Pin<&mut Self>,
        item: async_wsocket::Message,
    ) -> Result<(), Error> {
        std::pin::Pin::new(&mut self.0)
            .start_send(item.into())
            .map_err(Error::transport)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
        std::pin::Pin::new(&mut self.0)
            .poll_flush(cx)
            .map_err(Error::transport)
    }
    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Error>> {
        std::pin::Pin::new(&mut self.0)
            .poll_close(cx)
            .map_err(Error::transport)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_private_special_and_tunneled_destinations() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "100.100.100.200",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.0.1",
            "192.0.0.9",
            "198.18.0.1",
            "198.51.100.2",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:0808:0808::1",
            "3fff::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(public_ip(ip.parse().unwrap()));
        }
        for url in [
            "ws://relay.example",
            "wss://127.0.0.1",
            "wss://relay.example:444",
            "wss://user:password@relay.example",
            "wss://relay.example?token=x",
            "wss://relay.example#x",
        ] {
            assert!(normalize(url).is_none());
        }
    }
    #[tokio::test]
    async fn private_dns_resolution_is_rejected_before_connecting() {
        assert!(connect(&Url::parse("wss://localhost.").unwrap())
            .await
            .is_err());
    }
}
