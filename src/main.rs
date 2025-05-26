mod ip_pool;

use ip_pool::IpPool;

fn main() {
    println!("🚀 Test du module IpPool...");

    let mut pool = IpPool::new(100, 102);

    println!("🔍 Attribution de 3 IP :");
    for _ in 0..3 {
        match pool.lease_ip() {
            Some(ip) => println!("✅ IP attribuée : {}", ip),
            None => println!("❌ Plus d’IP disponibles."),
        }
    }

    println!("🔁 Libération d'une IP et réattribution :");
    let ip_to_release = "192.168.1.101".parse().unwrap();
    pool.release_ip(ip_to_release);

    match pool.lease_ip() {
        Some(ip) => println!("🔁 IP réattribuée : {}", ip),
        None => println!("❌ Toujours aucune IP disponible."),
    }

    println!("✅ Test terminé !");
}
