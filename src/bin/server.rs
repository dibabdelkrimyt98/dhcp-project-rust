use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{self, Write};

pub struct DHCPState {
    pub leases: HashMap<SocketAddr, String>,
    pub history: Vec<(SocketAddr, String)>,
    pub available_ips: Vec<String>,
    pub clients_status: HashMap<SocketAddr, bool>,
    pub socket: UdpSocket,
}

impl DHCPState {
    pub fn new(socket: UdpSocket, ip_pool: Vec<String>) -> Self {
        DHCPState {
            leases: HashMap::new(),
            history: Vec::new(),
            available_ips: ip_pool,
            clients_status: HashMap::new(),
            socket,
        }
    }

    pub fn handle_message(&mut self, msg: &str, src: SocketAddr) {
        if msg.starts_with("DISCOVER") {
            print!("\n\n ******** DORA ******** ");
            println!("\n\n⬅️ DISCOVER reçu de {}", src);
            if let Some(ip) = self.available_ips.pop() {
                println!("➡️ Envoi OFFER {} à {}", ip, src);
                self.leases.insert(src, ip.clone());
                self.clients_status.insert(src, true);
                self.history.push((src, ip.clone()));
                let offer = format!("OFFER:{}", ip);
                let _ = self.socket.send_to(offer.as_bytes(), src);
            } else {
                println!("⚠️ Pas d'IP disponible pour {}", src);
                let _ = self.socket.send_to(b"NO_AVAILABLE_IP", src);
            }
        } else if msg.starts_with("REQUEST:") {
            let requested_ip = msg.trim_start_matches("REQUEST:");
            println!("⬅️ REQUEST {} reçu de {}", requested_ip, src);
    
            if self.leases.values().any(|ip| ip == requested_ip) && self.leases.get(&src) != Some(&requested_ip.to_string()) {
                println!("❌ IP {} déjà utilisée, envoi DECLINE à {}", requested_ip, src);
                let _ = self.socket.send_to(b"DECLINE:IP_IN_USE", src);
            } else {
                println!("➡️ Envoi ACK {} à {}", requested_ip, src);
                self.leases.insert(src, requested_ip.to_string());
                self.clients_status.insert(src, true);
                self.history.push((src, requested_ip.to_string()));
                let ack = format!("ACK:{}", requested_ip);
                let _ = self.socket.send_to(ack.as_bytes(), src);
            }
        } else if msg.starts_with("RELEASE") {
            println!("⬅️ RELEASE reçu de {}", src);
            if let Some(ip) = self.leases.remove(&src) {
                self.available_ips.push(ip.clone());
                self.clients_status.remove(&src);
                println!("🔁 IP {} libérée par {}", ip, src);
            } else {
                println!("⚠️ Aucune IP à libérer pour {}", src);
            }
        }
    }

    pub fn afficher_clients(&self) {
        println!("📋 Clients connectés :");
        for (addr, ip) in &self.leases {
            let statut = if self.clients_status.get(addr).copied().unwrap_or(false) {
                "[connecté]"
            } else {
                "[déconnecté]"
            };
            println!("🔹 {} => {} {}", addr, ip, statut);
        }
    }

    pub fn afficher_historique(&self) {
        println!("📜 Historique des baux :");
        for (addr, ip) in &self.history {
            println!("📍 {} => {}", addr, ip);
        }
    }

    pub fn supprimer_client(&mut self, client_input: &str) {
        let maybe_addr: Option<SocketAddr> = client_input.parse().ok();

        match maybe_addr {
            Some(addr) => {
                if let Some(ip) = self.leases.remove(&addr) {
                    self.available_ips.push(ip.clone());
                    self.clients_status.remove(&addr);

                    // Historique
                    self.history.push((addr, ip.clone()));

                    // Notifier le client
                    let msg = format!("RELEASED_BY_ADMIN:{}", ip);
                    let _ = self.socket.send_to(msg.as_bytes(), addr);

                    println!("✅ Client {} supprimé. IP {} libérée.", addr, ip);
                } else {
                    println!("⚠️ Aucun client trouvé à cette adresse.");
                }
            }
            None => println!("❌ Format d’adresse invalide."),
        }
    }
}

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:8080").expect("Erreur de liaison du socket");
    socket.set_nonblocking(true).unwrap();

    let ip_pool = (100..200)
        .map(|i| format!("192.168.1.{}", i))
        .collect::<Vec<_>>();

    let state = Arc::new(Mutex::new(DHCPState::new(socket.try_clone().unwrap(), ip_pool)));

    let thread_state = Arc::clone(&state);
    thread::spawn(move || {
        let mut buf = [0; 1024];
        loop {
            if let Ok((len, src)) = socket.recv_from(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..len]);
                let mut st = thread_state.lock().unwrap();
                st.handle_message(&msg, src);
            }
        }
    });

    loop {
        println!("\n===== MENU DHCP =====");
        println!("1️⃣  Afficher les clients connectés");
        println!("2️⃣  Supprimer un client (libérer une IP)");
        println!("3️⃣  Historique des clients");
        println!("4️⃣  Éteindre le serveur");
        print!("👉 Choix : ");
        io::stdout().flush().unwrap();

        let mut choix = String::new();
        io::stdin().read_line(&mut choix).unwrap();

        match choix.trim() {
            "1" => state.lock().unwrap().afficher_clients(),
            "2" => {
                print!("🔧 Entrez l'adresse du client à supprimer (IP:PORT) : ");
                io::stdout().flush().unwrap();
                let mut addr = String::new();
                io::stdin().read_line(&mut addr).unwrap();
                state.lock().unwrap().supprimer_client(addr.trim());
            }
            "3" => state.lock().unwrap().afficher_historique(),
            "4" => {
                println!("👋 Arrêt du serveur...");
                break;
            }
            _ => println!("❌ Choix invalide."),
        }
    }
}
