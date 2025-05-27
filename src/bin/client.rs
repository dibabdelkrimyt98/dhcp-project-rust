use std::net::UdpSocket;
use std::str;
use std::time::Duration;
use std::thread;
use dhcp_demo::ip_pool::IpPool;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(15)))?;
    let server_addr = "127.0.0.1:6767";

    println!("🔎 Envoi DISCOVER...");
    socket.send_to(b"DISCOVER", server_addr)?;
    let mut buf = [0u8; 1024];

    match socket.recv_from(&mut buf) {
        Ok((len, _)) => {
            let msg = {
                let temp = str::from_utf8(&buf[..len]).unwrap_or("");
                println!("📨 Réponse serveur : {}", temp);
                temp.to_string() // on le transforme en String pour éviter l'emprunt
            };

            if msg.starts_with("OFFER:") {
                let ip = msg.trim_start_matches("OFFER:");
                let request = format!("REQUEST:{}", ip);
                println!("📥 Envoi de la requête de demande IP {}...", ip);
                socket.send_to(request.as_bytes(), server_addr)?;

                // Nouvelle réception -> nouvelle portée propre
                match socket.recv_from(&mut buf) {
                    Ok((len, _)) => {
                        let ack_msg = {
                            let temp = str::from_utf8(&buf[..len]).unwrap_or("");
                            println!("✅ Réponse finale : {}", temp);
                            temp.to_string()
                        };

                        if ack_msg.starts_with("ACK:") {
                            println!("🎉 IP {} assignée avec succès!", ip);
                            
                            println!("⏳ Appuyez sur Entrée pour relâcher l'IP et quitter...");
                            let mut input = String::new();
                            let _ = std::io::stdin().read_line(&mut input); // attend l'utilisateur
                        
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
