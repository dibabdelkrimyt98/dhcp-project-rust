// server.rs
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{self, Write};
use rusqlite::{Connection, params, OptionalExtension};

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

// Initialise la base de données
fn init_db() -> rusqlite::Result<Connection> {
    let conn = Connection::open("dhcp.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS leases (
            id INTEGER PRIMARY KEY,
            mac TEXT NOT NULL,
            ip TEXT NOT NULL,
            start_time DATETIME DEFAULT CURRENT_TIMESTAMP,
            end_time DATETIME,
            vendor TEXT,
            status TEXT
        )",
        [],
    )?;
    Ok(conn)
}

// Enregistre un nouveau bail dans la base de données
fn log_lease(conn: &Connection, mac: &str, ip: &str, vendor: &str, status: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO leases (mac, ip, vendor, status) VALUES (?1, ?2, ?3, ?4)",
        params![mac, ip, vendor, status],
    )?;
    Ok(())
}

// Met à jour le statut d'un bail
fn update_lease_status(conn: &Connection, mac: &str, ip: &str, status: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE leases SET end_time = CURRENT_TIMESTAMP, status = ?1 
         WHERE mac = ?2 AND ip = ?3 AND end_time IS NULL",
        params![status, mac, ip],
    )?;
    Ok(())
}

pub struct DHCPState {
    pub leases: HashMap<SocketAddr, (String, String)>, // IP + MAC
    pub available_ips: Vec<String>,
    pub clients_status: HashMap<SocketAddr, bool>,
    pub socket: UdpSocket,
    pub db_conn: Arc<Mutex<Connection>>, // Connexion à la base SQLite
}

impl DHCPState {
    pub fn new(socket: UdpSocket, ip_pool: Vec<String>, db_conn: Connection) -> Self {
        DHCPState {
            leases: HashMap::new(),
            available_ips: ip_pool,
            clients_status: HashMap::new(),
            socket,
            db_conn: Arc::new(Mutex::new(db_conn)),
        }
    }

    pub fn handle_message(&mut self, msg: &str, src: SocketAddr) {
        if msg.starts_with("DISCOVER:") {
            let mac = msg.trim_start_matches("DISCOVER:").trim();
            println!("\n\n ******** DORA ******** ");
            println!("⬅️ DISCOVER reçu de {} avec MAC {}", src, mac);
            if let Some(ip) = self.available_ips.pop() {
                let vendor = lookup_oui(mac);
                println!("➡️ Envoi OFFER {} à {} (Marque: {})", ip, src, vendor);
                self.leases.insert(src, (ip.clone(), mac.to_string()));
                self.clients_status.insert(src, true);
                
                // Enregistrement dans la base de données
                let db = self.db_conn.clone();
                let mac_clone = mac.to_string();
                let ip_clone = ip.clone();
                let vendor_clone = vendor.to_string();
                thread::spawn(move || {
                    let conn = db.lock().unwrap();
                    log_lease(&conn, &mac_clone, &ip_clone, &vendor_clone, "OFFERED")
                        .unwrap_or_else(|e| eprintln!("Erreur DB: {}", e));
                });
                
                let offer = format!("OFFER:{}:{}", ip, mac);
                let _ = self.socket.send_to(offer.as_bytes(), src);
            } else {
                println!("⚠️ Pas d'IP disponible pour {}", src);
                let _ = self.socket.send_to(b"NO_AVAILABLE_IP", src);
            }
        } else if msg.starts_with("REQUEST:") {
            // Format attendu: REQUEST:<ip>:<mac>
            let rest = msg.trim_start_matches("REQUEST:").trim();
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() < 2 {
                println!("❌ Format REQUEST invalide de {}", src);
                return;
            }
            let requested_ip = parts[0];
            let mac = parts[1];
            println!("⬅️ REQUEST {} reçu de {} avec MAC {}", requested_ip, src, mac);

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
                
                // Mise à jour du bail dans la base de données
                let db = self.db_conn.clone();
                let mac_clone = mac.to_string();
                let ip_clone = requested_ip.to_string();
                thread::spawn(move || {
                    let conn = db.lock().unwrap();
                    update_lease_status(&conn, &mac_clone, &ip_clone, "ACKNOWLEDGED")
                        .unwrap_or_else(|e| eprintln!("Erreur DB: {}", e));
                });
                
                let ack = format!("ACK:{}:{}", requested_ip, mac);
                let _ = self.socket.send_to(ack.as_bytes(), src);
            }
        } else if msg.starts_with("RELEASE") {
            println!("\n\n⬅️ RELEASE reçu de {}", src);
            if let Some((ip, mac)) = self.leases.remove(&src) {
                self.available_ips.push(ip.clone());
                self.clients_status.remove(&src);
                println!("🔁 IP {} libérée par {} (MAC {})", ip, src, mac);
                
                // Mise à jour du bail dans la base de données
                let db = self.db_conn.clone();
                thread::spawn(move || {
                    let conn = db.lock().unwrap();
                    update_lease_status(&conn, &mac, &ip, "RELEASED")
                        .unwrap_or_else(|e| eprintln!("Erreur DB: {}", e));
                });
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
        let conn = self.db_conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT mac, ip, vendor, start_time, end_time, status 
             FROM leases ORDER BY start_time DESC"
        ).unwrap();
        
        let lease_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        }).unwrap();

        for lease in lease_iter {
            if let Ok((mac, ip, vendor, start, end, status)) = lease {
                let end_time = end.unwrap_or_else(|| "En cours".to_string());
                println!(
                    "📍 {} - {} ({}) | Statut: {} | Début: {} | Fin: {}",
                    mac, ip, vendor, status, start, end_time
                );
            }
        }
    }

    pub fn supprimer_client(&mut self, client_input: &str) {
        let maybe_addr: Option<SocketAddr> = client_input.parse().ok();

        match maybe_addr {
            Some(addr) => {
                if let Some((ip, mac)) = self.leases.remove(&addr) {
                    self.available_ips.push(ip.clone());
                    self.clients_status.remove(&addr);

                    // Mise à jour du bail dans la base de données
                    let db = self.db_conn.clone();
                    let mac_clone = mac.clone();
                    let ip_clone = ip.clone();
                    thread::spawn(move || {
                        let conn = db.lock().unwrap();
                        update_lease_status(&conn, &mac_clone, &ip_clone, "RELEASED_BY_ADMIN")
                            .unwrap_or_else(|e| eprintln!("Erreur DB: {}", e));
                    });

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
    // Initialisation de la base de données
    let db_conn = init_db().expect("Erreur initialisation base de données");
    
    let socket = UdpSocket::bind("0.0.0.0:67").expect("Erreur de liaison du socket");
    socket.set_nonblocking(true).unwrap();

    let ip_pool = (100..200)
        .map(|i| format!("192.168.1.{}", i))
        .collect::<Vec<_>>();

    let state = Arc::new(Mutex::new(DHCPState::new(
        socket.try_clone().unwrap(), 
        ip_pool,
        db_conn
    )));

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