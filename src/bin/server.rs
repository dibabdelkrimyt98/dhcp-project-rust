use std::net::{Ipv4Addr, UdpSocket};
use std::str;
use dhcp_demo::ip_pool::IpPool;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:6767")?;
    println!("🟢 [DHCP Server] En écoute sur 127.0.0.1:6767");

    let mut pool = IpPool::new(100, 110);
    let mut buf = [0u8; 1024];

    loop {
        let (len, src) = socket.recv_from(&mut buf)?;
        let msg = str::from_utf8(&buf[..len]).unwrap_or("");
        println!("📩 Reçu de {} : {}", src, msg);

        if msg == "DISCOVER" {
            if let Some(ip) = pool.lease_ip() {
                let offer = format!("OFFER:{}", ip);
                socket.send_to(offer.as_bytes(), src)?;
                println!("📤 OFFER envoyé à {} : {}\n", src, ip);
            } else {
                println!("❌ Plus d'IP disponibles à offrir !");
            }
        } else if msg.starts_with("REQUEST:") {
            let ip_str = msg.trim_start_matches("REQUEST:");
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                if pool.confirm_lease(ip) {
                    let ack = format!("ACK:{}", ip);
                    socket.send_to(ack.as_bytes(), src)?;
                    println!("✅ IP {} attribuée à {}\n", ip, src);
                } else {
                    println!("⚠️ IP {} déjà louée !", ip);
                }
            } else {
                println!("❌ Format d’IP invalide reçu : {}", ip_str);
            }
        } else if msg.starts_with("RELEASE:") {
            let ip_str = msg.trim_start_matches("RELEASE:");
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                pool.release_ip(&ip);
                println!("🔓 IP {} libérée par {}", ip, src);
            } else {
                println!("❌ Format d’IP invalide reçu pour RELEASE : {}", ip_str);
            }
        } else {
            println!("⚠️ Message inconnu reçu : {}", msg);
        }
    }
}
