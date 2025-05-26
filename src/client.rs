use std::net::UdpSocket;
use std::str;
use std::time::Duration;
use std::thread;
use std::io;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let server_addr = "127.0.0.1:6767";

    println!("🔎 Envoi DISCOVER...");
    socket.send_to(b"DISCOVER", server_addr)?;

    let mut buf = [0u8; 1024];
    match socket.recv_from(&mut buf) {
        Ok((len, _)) => {
            let msg = String::from_utf8_lossy(&buf[..len]).to_string(); // éviter l'emprunt prolongé
            println!("📨 Réponse serveur : {}", msg);

            if msg.starts_with("OFFER:") {
                let ip = msg.trim_start_matches("OFFER:");
                let request = format!("REQUEST:{}", ip);
                println!("📥 Envoi de la requête de demande IP {}...", ip); // Pas d'échappement inutile
                socket.send_to(request.as_bytes(), server_addr)?;

                let mut ack_buf = [0u8; 1024]; // buffer séparé si tu veux éviter tout conflit
                match socket.recv_from(&mut ack_buf) {
                    Ok((ack_len, _)) => {
                        let ack_msg = String::from_utf8_lossy(&ack_buf[..ack_len]).to_string();
                        println!("✅ Réponse finale : {}", ack_msg);

                        if ack_msg.starts_with("ACK:") {
                            println!("🎉 IP {} assignée avec succès!", ip);

                            // Simuler une utilisation puis RELEASE
                            thread::sleep(Duration::from_secs(5));
                            let release = format!("RELEASE:{}", ip);
                            socket.send_to(release.as_bytes(), server_addr)?;
                            println!("🔓 IP {} relâchée.", ip);
                        }
                    }
                    Err(e) => println!("❌ Timeout ou erreur lors du ACK: {}", e),
                }
            }
        }
        Err(e) => println!("❌ Timeout ou erreur lors du OFFER: {}", e),
    }

    Ok(())
}
