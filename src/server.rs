use std::net::{Ipv4Addr, UdpSocket};

mod ip_pool;
use ip_pool::IpPool;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:6767")?;
    println!("🟢 [DHCP Server] En écoute sur 127.0.0.1:6767");

    let mut pool = IpPool::new(100, 110);
    let mut buf = [0u8; 1024];

    loop {
        let (len, src) = socket.recv_from(&mut buf)?;
        let msg = String::from_utf8_lossy(&buf[..len]);
        println!("📩 Reçu de {} : {}", src, msg);

        if msg == "DISCOVER" {
            if let Some(ip) = pool.lease_ip() {
                let offer = format!("OFFER:{}", ip);
                socket.send_to(offer.as_bytes(), src)?;
                println!("📤 OFFER envoyé à {} : {}\n", src, ip);
            } else {
                println!("❌ Plus d'IP disponibles à offrir !");
                let nack = "NACK:Plus d'IP disponibles";
                socket.send_to(nack.as_bytes(), src)?;
            }
        } else if msg.starts_with("REQUEST:") {
            let ip_str = msg.trim_start_matches("REQUEST:");
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                if pool.confirm_lease(ip) {
                    let ack = format!("ACK:{}", ip);
                    socket.send_to(ack.as_bytes(), src)?;
                    println!("✅ IP {} attribuée à {}\n", ip, src);
                } else {
                    println!("⚠️ IP {} déjà louée ou non disponible !", ip);
                    let nack = format!("NACK:IP {} non disponible", ip);
                    socket.send_to(nack.as_bytes(), src)?;
                }
            } else {
                println!("❌ Format d’IP invalide reçu : {}", ip_str);
                let err = "ERROR:IP invalide";
                socket.send_to(err.as_bytes(), src)?;
            }
        } else {
            println!("⚠️ Message inconnu : {}", msg);
            let err = "ERROR:Message inconnu";
            socket.send_to(err.as_bytes(), src)?;
        }
    }
}
