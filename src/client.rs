use std::net::UdpSocket;
use std::str;

fn main() -> std::io::Result<()> {
    let server_addr = "127.0.0.1:6767";
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    println!("🔵 [DHCP Client] Démarré");

    // Étape 1: Envoi DISCOVER
    println!("\n➡️  Étape 1: Envoi de DISCOVER au serveur {}", server_addr);
    socket.send_to(b"DISCOVER", server_addr)?;

    let mut buf = [0u8; 1024];

    // Étape 2: Attente de l'OFFER
    let (len, _) = socket.recv_from(&mut buf)?;
    let offer_msg = String::from_utf8_lossy(&buf[..len]);
    println!("⬅️  Étape 2: Réception de l’offre du serveur : {}", offer_msg);

    if !offer_msg.starts_with("OFFER:") {
        eprintln!("Erreur : Offre attendue, reçu autre chose : {}", offer_msg);
        return Ok(());
    }

    let ip = offer_msg.trim_start_matches("OFFER:").trim();

    // Étape 3: Envoi REQUEST
    println!("\n➡️  Étape 3: Envoi de REQUEST:{} au serveur", ip);
    let request_msg = format!("REQUEST:{}", ip);
    socket.send_to(request_msg.as_bytes(), server_addr)?;

    // Étape 4: Attente ACK ou NACK
    let (len, _) = socket.recv_from(&mut buf)?;
    let resp = String::from_utf8_lossy(&buf[..len]);

    if resp.starts_with("ACK:") {
        let assigned_ip = resp.trim_start_matches("ACK:").trim();
        println!("✅ Adresse IP attribuée : {}\n", assigned_ip);
    } else if resp.starts_with("NACK:") {
        eprintln!("❌ Le serveur a refusé la demande : {}", resp);
    } else if resp.starts_with("ERROR:") {
        eprintln!("❌ Erreur reçue du serveur : {}", resp);
    } else {
        eprintln!("❌ Réponse inattendue du serveur : {}", resp);
    }

    Ok(())
}
