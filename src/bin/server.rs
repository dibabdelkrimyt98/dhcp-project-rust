use std::collections::HashMap;
use std::io::{self, Write};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::Local;
use colored::*;
use dhcp_demo::ip_pool::IpPool;

#[derive(Debug, Clone)]
struct ClientInfo {
    ip: String,
    historique: Vec<String>,
    actif: bool,
}

fn afficher_clients(clients: &HashMap<String, ClientInfo>) {
    println!("\n📋 Clients connectés :");
    for (addr, client) in clients {
        let status = if client.actif { "connecté".green() } else { "déconnecté".red() };
        println!("🔹 {} => {} [{}]", addr, client.ip, status);
    }
}

fn afficher_historique(clients: &HashMap<String, ClientInfo>) {
    println!("\n📜 Historique des clients :");
    for (addr, client) in clients {
        println!(
            "🧾 {} → {} [{}]",
            addr,
            client.ip,
            if client.actif { "connecté".green() } else { "déconnecté".red() }
        );
        for event in &client.historique {
            println!("   ➜ {}", event);
        }
    }
}

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:6767")?;
    println!("🚀 Serveur DHCP démarré sur {}", socket.local_addr()?);

    let clients: Arc<Mutex<HashMap<String, ClientInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let clients_clone = Arc::clone(&clients);
    let pool = Arc::new(Mutex::new(IpPool::new(100, 200))); // 192.168.1.100 à 192.168.1.200
    let pool_clone = Arc::clone(&pool);

    thread::spawn(move || loop {
        let mut buf = [0; 512];
        if let Ok((amt, src)) = socket.recv_from(&mut buf) {
            let msg = String::from_utf8_lossy(&buf[..amt]);
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            println!("📩 [{}] Reçu de {} : {}", now, src, msg);

            let mut clients = clients_clone.lock().unwrap();
            let mut pool = pool_clone.lock().unwrap();

            let entry = clients.entry(src.to_string()).or_insert(ClientInfo {
                ip: "0.0.0.0".into(),
                historique: Vec::new(),
                actif: false,
            });

            let event = format!("{} ➜ {}", msg, now);
            entry.historique.push(event.clone());

            if msg.starts_with("DISCOVER") {
                if let Some(ip) = pool.lease_ip() {
                    entry.ip = ip.to_string();
                    entry.actif = true;
                    let offer = format!("OFFER:{}", ip);
                    let _ = socket.send_to(offer.as_bytes(), src);
                } else {
                    let _ = socket.send_to(b"OFFER:0.0.0.0", src);
                }
            } else if msg.starts_with("REQUEST:") {
                let ip_requested = msg.trim_start_matches("REQUEST:").trim();
                if ip_requested == entry.ip {
                    entry.actif = true;
                    let ack = format!("ACK:{}", ip_requested);
                    let _ = socket.send_to(ack.as_bytes(), src);
                } else {
                    let _ = socket.send_to(b"NAK", src);
                }
            } else if msg.starts_with("RELEASE:") {
                let ip_to_release = msg.trim_start_matches("RELEASE:").trim();
                if ip_to_release == entry.ip {
                    pool.release_ip(&entry.ip.parse().unwrap());
                    entry.actif = false;
                    let _ = socket.send_to(b"RELEASED", src);
                }
            } else {
                println!("⚠️ Message inconnu reçu : {}", msg);
            }
        }
    });

    // Interface Console
    loop {
        println!(
            "\n===== MENU DHCP =====\n\
            1️⃣  Afficher les clients connectés\n\
            2️⃣  Supprimer un client (libérer une IP)\n\
            3️⃣  Historique des clients\n\
            4️⃣  Éteindre le serveur\n\
            👉 Choix : "
        );
        io::stdout().flush()?;
        let mut choix = String::new();
        io::stdin().read_line(&mut choix)?;

        match choix.trim() {
            "1" => {
                let clients = clients.lock().unwrap();
                afficher_clients(&clients);
            }
            "2" => {
                print!("🔧 Entrez l'adresse du client à supprimer : ");
                io::stdout().flush()?;
                let mut addr = String::new();
                io::stdin().read_line(&mut addr)?;
                let addr = addr.trim();
                
                let mut clients = clients.lock().unwrap();
                let mut pool = pool.lock().unwrap();
                
                if let Some((key, client)) = clients.iter_mut().find(|(_, c)| c.ip == addr) {
                    if client.actif {
                        pool.release_ip(&client.ip.parse().unwrap());
                    }
                    client.actif = false;
                    println!("✅ Client avec l'IP {} libéré.", addr);
                } else if let Some(client) = clients.get_mut(addr) {
                    // Ancienne logique si c’est une IP:PORT
                    if client.actif {
                        pool.release_ip(&client.ip.parse().unwrap());
                    }
                    client.actif = false;
                    println!("✅ Client {} libéré.", addr);
                } else {
                    println!("❌ Client introuvable.");
                }
            }
            "3" => {
                let clients = clients.lock().unwrap();
                afficher_historique(&clients);
            }
            "4" => {
                println!("🛑 Arrêt du serveur...");
                break;
            }
            _ => println!("❌ Choix invalide"),
        }
    }

    Ok(())
}
