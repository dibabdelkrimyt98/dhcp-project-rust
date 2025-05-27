use std::net::UdpSocket;
use std::time::Duration;
use std::io;

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.connect("127.0.0.1:8080")?;

    let mut buf = [0u8; 1024];

    println!("➡️  Envoi DISCOVER");
    socket.send(b"DISCOVER")?;

    let len = socket.recv(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..len]).to_string();
    println!("⬅️  Réception OFFER : {}", response);

    if response.starts_with("OFFER:") {
        let offered_ip = response.trim_start_matches("OFFER:");

        println!("➡️  Envoi REQUEST pour l'IP {}", offered_ip);
        let request_msg = format!("REQUEST:{}", offered_ip);
        socket.send(request_msg.as_bytes())?;

        let len = socket.recv(&mut buf)?;
        let ack_response = String::from_utf8_lossy(&buf[..len]).to_string();
        println!("⬅️  Réception ACK ou DECLINE : {}", ack_response);

        if ack_response.starts_with("ACK:") {
            println!("✅ Bail DHCP accepté pour l'IP {}", offered_ip);

            println!("Appuyez sur Entrée pour libérer l'adresse IP...");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            println!("➡️  Envoi RELEASE");
            socket.send(b"RELEASE")?;
            println!("🔁 Bail DHCP libéré.");
        } else {
            println!("❌ Demande rejetée par le serveur : {}", ack_response);
        }
    } else {
        println!("❌ Aucune offre reçue, fin du processus.");
    }

    Ok(())
}
