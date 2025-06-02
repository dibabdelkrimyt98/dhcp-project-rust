use std::net::{UdpSocket, SocketAddr};
use std::process::Command;
use std::time::Duration;
use std::io;

#[cfg(target_os = "windows")]
fn get_local_mac() -> Option<String> {
    let output = Command::new("cmd")
        .args(["/C", "getmac"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(mac) = line.split_whitespace().next() {
            if mac.contains('-') {
                return Some(mac.replace("-", "").to_uppercase());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn get_local_mac() -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("ip link | grep -m 1 ether")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(pos) = line.find("ether") {
            return Some(line[pos + 6..].split_whitespace().next()?.replace(":", "").to_uppercase());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_local_mac() -> Option<String> {
    let output = Command::new("ifconfig")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("ether") {
            return Some(line.trim().split_whitespace().last()?.replace(":", "").to_uppercase());
        }
    }
    None
}

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_broadcast(true)?; // Activation du broadcast
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mac_address = get_local_mac().unwrap_or_else(|| {
        println!("⚠️ Impossible de récupérer l'adresse MAC locale. Envoi sans MAC.");
        "UNKNOWN".to_string()
    });

    let server_addr: SocketAddr = "255.255.255.255:67".parse().expect("Adresse broadcast invalide");

    // Étape 1: Envoi du DISCOVER
    println!("➡️ Envoi DISCOVER avec MAC {}", mac_address);
    let discover_msg = format!("DISCOVER:{}", mac_address);
    socket.send_to(discover_msg.as_bytes(), server_addr)?;

    let mut buf = [0u8; 1024];
    let len = match socket.recv(&mut buf) {
        Ok(len) => len,
        Err(e) => {
            println!("❌ Erreur de réception: {}", e);
            println!("🔄 Nouvelle tentative dans 3 secondes...");
            std::thread::sleep(Duration::from_secs(3));
            socket.send_to(discover_msg.as_bytes(), server_addr)?;
            socket.recv(&mut buf)?
        }
    };
    
    let response = String::from_utf8_lossy(&buf[..len]).to_string();
    println!("⬅️ Réception OFFER : {}", response);

    if response.starts_with("OFFER:") {
        // Extraction de l'IP offerte (format: "OFFER:IP:MAC")
        let parts: Vec<&str> = response.split(':').collect();
        if parts.len() < 3 {
            println!("❌ Format OFFER invalide: {}", response);
            return Ok(());
        }
        let offered_ip = parts[1];

        // Étape 2: Envoi du REQUEST avec IP + MAC
        println!("➡️ Envoi REQUEST pour l'IP {}", offered_ip);
        let request_msg = format!("REQUEST:{}:{}", offered_ip, mac_address);
        socket.send_to(request_msg.as_bytes(), server_addr)?;

        let len = socket.recv(&mut buf)?;
        let ack_response = String::from_utf8_lossy(&buf[..len]).to_string();
        println!("⬅️ Réponse du serveur : {}", ack_response);

        if ack_response.starts_with("ACK:") {
            println!("✅ Bail DHCP accepté pour l'IP {}", offered_ip);

            println!("Appuyez sur Entrée pour libérer l'adresse IP...");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            println!("➡️ Envoi RELEASE");
            socket.send_to(b"RELEASE", server_addr)?;
            println!("🔁 Bail DHCP libéré.");
        } else {
            println!("❌ Demande rejetée par le serveur : {}", ack_response);
            if ack_response.contains("NO_AVAILABLE_IP") {
                println!("💡 Le serveur n'a plus d'IP disponibles");
            }
        }
    } else if response == "NO_AVAILABLE_IP" {
        println!("❌ Le serveur n'a plus d'adresses IP disponibles");
    } else {
        println!("❌ Réponse inattendue du serveur : {}", response);
    }

    Ok(())
}