use std::net::UdpSocket;
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
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.connect("127.0.0.1:8080")?;

    let mac_address = get_local_mac().unwrap_or_else(|| {
        println!("⚠️ Impossible de récupérer l'adresse MAC locale. Envoi sans MAC.");
        "UNKNOWN".to_string()
    });

    println!("➡️ Envoi DISCOVER avec MAC {}", mac_address);
    let discover_msg = format!("DISCOVER:{}", mac_address);
    socket.send(discover_msg.as_bytes())?;

    let mut buf = [0u8; 1024];
    let len = socket.recv(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..len]).to_string();
    println!("⬅️ Réception OFFER : {}", response);

    if response.starts_with("OFFER:") {
        let offered_ip = response.trim_start_matches("OFFER:");

        println!("➡️ Envoi REQUEST pour l'IP {}", offered_ip);
        let request_msg = format!("REQUEST:{}", offered_ip);
        socket.send(request_msg.as_bytes())?;

        let len = socket.recv(&mut buf)?;
        let ack_response = String::from_utf8_lossy(&buf[..len]).to_string();
        println!("⬅️ Réception ACK ou DECLINE : {}", ack_response);

        if ack_response.starts_with("ACK:") {
            println!("✅ Bail DHCP accepté pour l'IP {}", offered_ip);

            println!("Appuyez sur Entrée pour libérer l'adresse IP...");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            println!("➡️ Envoi RELEASE");
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
