use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use std::io::{self, Write};
use std::env;
use std::process::exit;

// Table OUI simplifiée : OUI (uppercase, sans séparateurs) -> marque
fn lookup_oui(mac: &str) -> &'static str {
    let oui_map = [
        ("3C5A37", "Apple"),
        ("FCFBFB", "Samsung"),
        ("A4C138", "Dell"),
        ("00163E", "Cisco"),
        ("001A2B", "Hewlett-Packard"),
        ("F4F5E8", "Sony"),
        ("F0DE61", "Microsoft"),
        ("3C5AB4", "Apple"),
        ("B827EB", "Raspberry Pi Foundation"),
    ];

    let mac = mac.to_uppercase().replace(":", "").replace("-", "");
    if mac.len() < 6 {
        return "Unknown";
    }
    let prefix = &mac[0..6];
    for (oui, vendor) in oui_map.iter() {
        if *oui == prefix {
            return vendor;
        }
    }
    "Unknown"
}

pub struct DHCPState {
    pub leases: HashMap<SocketAddr, (String, String)>, // IP + MAC
    pub history: Vec<(SocketAddr, String, String)>,    // addr, IP, MAC
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
        let now = SystemTime::now();
        if msg.starts_with("DISCOVER:") {
            let mac = msg.trim_start_matches("DISCOVER:").trim();
            println!("\n[{:?}] ******** DORA ********", now);
            println!("⬅️ DISCOVER reçu de {} avec MAC {}", src, mac);
            if let Some(ip) = self.available_ips.pop() {
                let vendor = lookup_oui(mac);
                println!("➡️ Envoi OFFER {} à {} (Marque: {})", ip, src, vendor);
                self.leases.insert(src, (ip.clone(), mac.to_string()));
                self.clients_status.insert(src, true);
                self.history.push((src, ip.clone(), mac.to_string()));
                let offer = format!("OFFER:{}:{}", ip, mac);
                let _ = self.socket.send_to(offer.as_bytes(), src);
            } else {
                println!("⚠️ Pas d'IP disponible pour {}", src);
                let _ = self.socket.send_to(b"NO_AVAILABLE_IP", src);
            }
        } else if msg.starts_with("REQUEST:") {
            let rest = msg.trim_start_matches("REQUEST:").trim();
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() < 2 {
                println!("❌ Format REQUEST invalide de {}", src);
                return;
            }
            let requested_ip = parts[0];
            let mac = parts[1];
            println!("[{:?}] ⬅️ REQUEST {} reçu de {} avec MAC {}", now, requested_ip, src, mac);

            if self.leases.values().any(|(ip, _)| ip == requested_ip)
                && self.leases.get(&src) != Some(&(requested_ip.to_string(), mac.to_string()))
            {
                println!("❌ IP {} déjà utilisée, envoi DECLINE à {}", requested_ip, src);
                let _ = self.socket.send_to(b"DECLINE:IP_IN_USE", src);
            } else {
                let vendor = lookup_oui(mac);
                println!("➡️ Envoi ACK {} à {} (Marque: {})", requested_ip, src, vendor);
                self.leases.insert(src, (requested_ip.to_string(), mac.to_string()));
                self.clients_status.insert(src, true);
                self.history.push((src, requested_ip.to_string(), mac.to_string()));
                let ack = format!("ACK:{}:{}", requested_ip, mac);
                let _ = self.socket.send_to(ack.as_bytes(), src);
            }
        } else if msg.starts_with("RELEASE") {
            println!("\n[{:?}] ⬅️ RELEASE reçu de {}", now, src);
            if let Some((ip, mac)) = self.leases.remove(&src) {
                self.available_ips.push(ip.clone());
                self.clients_status.remove(&src);
                println!("🔁 IP {} libérée par {} (MAC {})", ip, src, mac);
            } else {
                println!("⚠️ Aucune IP à libérer pour {}", src);
            }
        }
    }

    pub fn afficher_clients(&self) {
        println!("📋 Clients connectés :");
        for (addr, (ip, mac)) in &self.leases {
            let statut = if self.clients_status.get(addr).copied().unwrap_or(false) {
                "[connecté]"
            } else {
                "[déconnecté]"
            };
            let vendor = lookup_oui(mac);
            println!("🔹 {} => {} {} (MAC: {}, Marque: {})", addr, ip, statut, mac, vendor);
        }
    }

    pub fn afficher_historique(&self) {
        println!("📜 Historique des baux :");
        for (addr, ip, mac) in &self.history {
            let vendor = lookup_oui(mac);
            println!("📍 {} => {} (MAC: {}, Marque: {})", addr, ip, mac, vendor);
        }
    }

    pub fn supprimer_client(&mut self, client_input: &str) {
        let maybe_addr: Option<SocketAddr> = client_input.parse().ok();

        match maybe_addr {
            Some(addr) => {
                if let Some((ip, mac)) = self.leases.remove(&addr) {
                    self.available_ips.push(ip.clone());
                    self.clients_status.remove(&addr);
                    self.history.push((addr, ip.clone(), mac.clone()));

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
    // Argument optionnel : IP:PORT à binder (ex: 192.168.1.10:8080)
    let bind_addr = env::args().nth(1).unwrap_or_else(|| "0.0.0.0:67".to_string());
    println!("🛰️ Démarrage serveur DHCP sur {}", bind_addr);

    let socket = UdpSocket::bind(&bind_addr).unwrap_or_else(|e| {
        eprintln!("Erreur de liaison sur {} : {}", bind_addr, e);
        exit(1);
    });
    socket.set_nonblocking(true).unwrap();

    // Pool IP flexible, par exemple 192.168.1.100 - 192.168.1.199
    let ip_pool = (100..200)
        .map(|i| {
            // On remplace le dernier octet par i, en prenant le préfixe IP de bind_addr
            // Pour simplicité, on extrait le préfixe IP avant le dernier point :
            let ip_prefix = bind_addr.split(':').next().unwrap_or("192.168.1.0");
            let mut parts = ip_prefix.split('.').collect::<Vec<_>>();
            if parts.len() == 4 {
                let new_val = i.to_string();
                parts[3] = &new_val;
                parts.join(".")
            } else {
                format!("192.168.1.{}", i)
            }
        })
        .collect::<Vec<_>>();

    let state = Arc::new(Mutex::new(DHCPState::new(socket.try_clone().unwrap(), ip_pool)));

    let thread_state = Arc::clone(&state);
    let socket_clone = socket.try_clone().unwrap();

    // Thread écoute messages réseau UDP non bloquant
    thread::spawn(move || {
        let mut buf = [0; 1024];
        loop {
            match socket_clone.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);
                    let mut st = thread_state.lock().unwrap();
                    st.handle_message(&msg, src);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Pas de données reçues, on attend un peu pour ne pas boucler à vide
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    eprintln!("Erreur recv_from: {}", e);
                    break;
                }
            }
        }
    });

    // Gestion Ctrl+C (pour Linux/Unix et Windows)
    ctrlc::set_handler(move || {
        println!("\n🛑 Signal d'arrêt reçu, serveur DHCP termine proprement...");
        exit(0);
    }).expect("Erreur lors de la configuration du gestionnaire Ctrl+C");

    // Menu principal pour contrôle manuel du serveur
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
        let choix = choix.trim();

        match choix {
            "1" => {
                let st = state.lock().unwrap();
                st.afficher_clients();
            }
            "2" => {
                let st = state.lock().unwrap();
                st.afficher_historique();
            }
            "3" => {
                print!("Entrer l'adresse client (IP:port) à supprimer : ");
                io::stdout().flush().unwrap();
                let mut addr = String::new();
                io::stdin().read_line(&mut addr).unwrap();
                let addr = addr.trim();
                let mut st = state.lock().unwrap();
                st.supprimer_client(addr);
            }
            "4" => {
                println!("👋 Arrêt du serveur...");
                break;
            }
            _ => println!("Choix invalide, veuillez réessayer."),
        }
    }
}
