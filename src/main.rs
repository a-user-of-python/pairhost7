// PairHost7
//
// A jailbreak command-line tool that lets an older jailbroken iPhone act
// as the "computer" for iOS 27 device-initiated wireless pairing.
//
// Background: starting with iOS 27, a device can initiate pairing to a
// computer instead of the other way around. The computer advertises an
// `_remotepairing-pairable-host._tcp` mDNS service; the device connects
// to the advertised port and drives the rppairing conversation, with
// this side acting as the SRP server/accessory. We generate and display
// a PIN that the user types into the device. On success we hold an
// RpPairingFile for the device, which tools such as StikDebug can
// import (StikDebug: Settings > Pairing File > Import Pairing File).
//
// This program is based on the `pair_host` tool from the `idevice`
// project (https://github.com/jkcoxson/idevice, MIT License, by
// Jackson Coxson). Changes from the original: a persistent host
// identity (the original regenerates one every run), a retry loop
// instead of exiting on the first failed attempt, and default output
// paths that are easy to reach on a jailbroken device.

use std::io::Write;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, Command};
use idevice::remote_pairing::{
    PAIRABLE_HOST_SERVICE_TYPE, PairableHost, PairableHostInfo, RpPairingFile, RpPairingSocket,
};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::net::TcpListener;

const DEFAULT_NAME: &str = "PairHost7";
const DEFAULT_MODEL: &str = "Mac17,7";

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/mobile"))
}

fn state_dir() -> PathBuf {
    home_dir().join(".pairhost7")
}

/// The pairing file contains a private key; keep it readable only by
/// the owner, like SSH keys.
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

/// Load the persisted host identity, or create and persist a fresh one.
///
/// A real pairing host keeps the same identity across runs: the
/// pairing file (Ed25519 keys + identifier) and the host `alt_irk`
/// are stored under ~/.pairhost7/ so a device that already paired
/// keeps recognizing this host. The identity file stored here is the
/// pristine generated one; `PairableHost::accept` writes the *paired
/// device's* altIRK into the pairing file it completes, and that
/// completed file is written to the output paths, never over this one.
async fn load_or_create_identity(name: &str, model: &str) -> (RpPairingFile, [u8; 16]) {
    let dir = state_dir();
    let identity_path = dir.join("host_identity.plist");
    let irk_path = dir.join("host_alt_irk.bin");

    if let Ok(file) = RpPairingFile::read_from_file(&identity_path).await {
        if let Ok(bytes) = tokio::fs::read(&irk_path).await {
            if bytes.len() == 16 {
                let mut irk = [0u8; 16];
                irk.copy_from_slice(&bytes);
                println!("Reusing saved host identity {}", file.identifier);
                return (file, irk);
            }
        }
    }

    let file = RpPairingFile::generate(name);
    // PairableHostInfo::generate is the crate's way to mint a random
    // alt_irk; take the value and persist it ourselves.
    let minted = PairableHostInfo::generate(name, model);
    let irk = minted.alt_irk;

    match tokio::fs::create_dir_all(&dir).await {
        Ok(()) => {
            if let Err(e) = file.write_to_file(&identity_path).await {
                eprintln!("warning: could not save host identity: {e}");
            } else {
                restrict_permissions(&identity_path);
            }
            if let Err(e) = tokio::fs::write(&irk_path, irk).await {
                eprintln!("warning: could not save host alt_irk: {e}");
            } else {
                restrict_permissions(&irk_path);
            }
        }
        Err(e) => eprintln!("warning: could not create {}: {e}", dir.display()),
    }
    (file, irk)
}

/// Write the completed pairing file. With no explicit --out, write to
/// every default location that works: the user's home directory (easy
/// over SSH/SCP), /var/mobile/Media (reachable from Filza/Files for
/// AirDrop), and the current directory. Returns true if any write
/// succeeded.
async fn write_result_file(file: &RpPairingFile, explicit_out: Option<&str>) -> bool {
    let mut targets: Vec<PathBuf> = Vec::new();
    match explicit_out {
        Some(p) => targets.push(PathBuf::from(p)),
        None => {
            targets.push(home_dir().join("pairingfile.plist"));
            targets.push(PathBuf::from("/var/mobile/Media/pairingfile.plist"));
            targets.push(PathBuf::from("pairingfile.plist"));
        }
    }
    targets.dedup();

    let mut any = false;
    for target in &targets {
        match file.write_to_file(target).await {
            Ok(()) => {
                restrict_permissions(target);
                println!("Pairing file written to {}", target.display());
                any = true;
            }
            Err(e) => eprintln!("warning: could not write {}: {e}", target.display()),
        }
    }
    any
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches = Command::new("pairhost7")
        .about("Advertise as a pairable host and accept a device-initiated pairing (iOS 27+)")
        .arg(
            Arg::new("name")
                .long("name")
                .value_name("NAME")
                .help("Name shown on the device")
                .default_value(DEFAULT_NAME),
        )
        .arg(
            Arg::new("model")
                .long("model")
                .value_name("MODEL")
                .help("Hardware model identifier shown on the device")
                .default_value(DEFAULT_MODEL),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .help("TCP port to listen on (0 = pick a free port)")
                .default_value("0"),
        )
        .arg(
            Arg::new("out")
                .long("out")
                .value_name("PATH")
                .help("Where to write the resulting pairing file (default: ~/pairingfile.plist, /var/mobile/Media/pairingfile.plist and ./pairingfile.plist)"),
        )
        .arg(
            Arg::new("pinless")
                .long("pinless")
                .help("Advertise pinless pairing and use Apple's all-zero setup code")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let name = matches.get_one::<String>("name").unwrap().clone();
    let model = matches.get_one::<String>("model").unwrap().clone();
    let port: u16 = matches
        .get_one::<String>("port")
        .unwrap()
        .parse()
        .expect("invalid port");
    let out = matches.get_one::<String>("out").cloned();
    let pinless = matches.get_flag("pinless");

    // Bind first so we can advertise the real port.
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .expect("failed to bind TCP listener");
    let port = listener.local_addr().expect("no local addr").port();

    let (mut pairing_file, alt_irk) = load_or_create_identity(&name, &model).await;
    let mut host_info = PairableHostInfo::generate(&name, &model);
    host_info.alt_irk = alt_irk;
    host_info.allows_pinless_pairing = pinless;
    let service_identifier = pairing_file.identifier.clone();

    let mdns = ServiceDaemon::new().expect("failed to create mDNS daemon");
    // mdns-sd rejects service type names longer than 15 characters by
    // default; "remotepairing-pairable-host" is 28, so raise the limit
    // (same value the upstream pair_host tool uses).
    mdns.set_service_name_len_max(30)
        .expect("failed to raise service name length limit");
    let hostname = format!("pairhost7-{}.local.", &service_identifier[..8]);
    let txt = host_info.mdns_txt_records(&service_identifier);
    let properties: Vec<(&str, &str)> = txt.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let service_info = ServiceInfo::new(
        PAIRABLE_HOST_SERVICE_TYPE,
        &service_identifier,
        &hostname,
        "",
        port,
        &properties[..],
    )
    .expect("invalid service info")
    .enable_addr_auto();
    mdns.register(service_info)
        .expect("failed to register mDNS service");

    println!("Advertising {PAIRABLE_HOST_SERVICE_TYPE} as \"{name}\" ({model})");
    println!("  identifier: {service_identifier}");
    println!("  port:       {port}");
    println!();
    println!("On the iPhone or iPad you want to pair (iOS 27+):");
    println!("  Settings > Privacy & Security > Developer Mode");
    println!("  Pick \"{name}\" and enter the code shown here.");
    println!();
    println!("Waiting for a device to connect and start pairing...");
    println!("(Leave this running. Ctrl+C stops it.)");
    std::io::stdout().flush().ok();

    let mut done = false;
    while !done {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("accept failed: {e}");
                break;
            }
        };
        println!("Device connected from {peer_addr}");

        let socket = RpPairingSocket::new_device(stream);
        let mut host = PairableHost::new(socket, host_info.clone());

        match host
            .accept(&mut pairing_file, |pin| async move {
                println!();
                println!("========================================");
                println!("  Enter this code on your device: {pin}");
                println!("========================================");
                println!();
                std::io::stdout().flush().ok();
            })
            .await
        {
            Ok(peer_device) => {
                println!("Paired with device:");
                println!("  name:  {}", peer_device.name);
                println!("  model: {}", peer_device.model);
                println!("  udid:  {}", peer_device.remotepairing_udid);
                if write_result_file(&pairing_file, out.as_deref()).await {
                    println!();
                    println!("Done. AirDrop pairingfile.plist to the paired device and");
                    println!("import it in StikDebug: Settings > Pairing File > Import Pairing File.");
                    done = true;
                } else {
                    eprintln!("Pairing succeeded but writing the pairing file failed everywhere.");
                    eprintln!("Fix the output path problem and run pairhost7 again to re-pair.");
                    break;
                }
            }
            Err(e) => {
                eprintln!("Pairing attempt failed: {e}");
                eprintln!("Still advertising as \"{name}\" — you can try again on the device.");
                std::io::stdout().flush().ok();
            }
        }
    }

    if let Ok(rx) = mdns.shutdown() {
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    }
}
