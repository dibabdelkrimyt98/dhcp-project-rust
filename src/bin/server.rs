use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::str;
use std::time::Duration;
use dhcp_demo::ip_pool::IpPool;

#[derive(Debug)]
struct ClientInfo {
    ip: Ipv4Addr,
    active: bool,
}

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:6767")?;
    socket.set_nonblocking(true)?;
    println!("🟢 [DHCP Server] En écoute sur 127.0.0.1:6767");

    let mut pool = IpPool::new(100, 110);
    let mut buf = [0u8; 1024];
    let mut clients: HashMap<SocketAddr, ClientInfo> = HashMap::new();
    let mut running = true;

    while running {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let msg = str::from_utf8(&buf[..len]).unwrap_or("");
                println!("📩 Reçu de {} : {}", src, msg);

                if msg == "DISCOVER" {
                    if let Some(ip) = pool.lease_ip() {
                        let offer = format!("OFFER:{}", ip);
                        socket.send_to(offer.as_bytes(), src)?;
                        clients.insert(src, ClientInfo { ip, active: true });
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
                            if let Some(client) = clients.get_mut(&src) {
                                client.active = true;
                                client.ip = ip;
                            } else {
                                clients.insert(src, ClientInfo { ip, active: true });
                            }
                            println!("✅ IP {} attribuée à {}\n", ip, src);
                        } else {
                            println!("⚠️ IP {} déjà louée ou invalide !", ip);
                        }
                    } else {
                        println!("❌ Format d’IP invalide reçu : {}", ip_str);
                    }
                } else {
                    println!("⚠️ Message inconnu reçu : {}", msg);
                }
            }

            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                show_menu(&mut clients, &mut pool, &mut running);
            }

            Err(e) => {
                eprintln!("❌ Erreur de socket : {}", e);
            }
        }
    }

    println!("⛔ Serveur arrêté.");
    Ok(())
}

fn show_menu(
    clients: &mut HashMap<SocketAddr, ClientInfo>,
    pool: &mut IpPool,
    running: &mut bool,
) {
    println!("\n===== MENU DHCP =====");
    println!("1️⃣  Afficher les clients connectés");
    println!("2️⃣  Supprimer un client (libérer une IP)");
    println!("3️⃣  Éteindre le serveur");
    print!("👉 Choix : ");
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    if let Ok(_) = io::stdin().read_line(&mut choice) {
        match choice.trim() {
            "1" => {
                if clients.is_empty() {
                    println!("📭 Aucun client connecté.");
                } else {
                    println!("📋 Liste des clients :");
                    for (addr, info) in clients.iter() {
                        println!(
                            "🔹 {} => {} [{}]",
                            addr,
                            info.ip,
                            if info.active { "actif" } else { "inactif" }
                        );
                    }
                }
            }
            "2" => {
                println!("🔧 IP à libérer (ex: 192.168.1.105) :");
                let mut ip_input = String::new();
                if io::stdin().read_line(&mut ip_input).is_ok() {
                    let ip_str = ip_input.trim();
                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        let target = clients.iter()
                            .find(|(_, c)| c.ip == ip)
                            .map(|(addr, _)| *addr);

                        if let Some(addr) = target {
                            clients.remove(&addr);
                            pool.release_ip(&ip);
                            println!("✅ IP {} libérée (client supprimé).", ip);
                        } else {
                            println!("❌ Aucun client avec cette IP.");
                        }
                    } else {
                        println!("❌ IP invalide !");
                    }
                }
            }
            "3" => {
                *running = false;
            }
            _ => println!("❌ Choix invalide."),
        }
    }
}
