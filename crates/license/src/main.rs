//! ERG's own tool, not shipped with the installer:
//!   erg-license keygen <private-key-file>                       once; prints the public key for erg-license.pub
//!   erg-license sign <private-key-file> <seats> <days> <school>  prints a licence key (days 0 = never expires)
use base64::{engine::general_purpose::STANDARD as B64, Engine};

fn run(a: &[String]) -> Result<String, String> {
    match a {
        [cmd, file] if cmd == "keygen" => {
            let (private, public) = erg_license::keygen()?;
            // create_new: an existing private key is never overwritten (every key sold was signed with it)
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(file).map_err(|e| format!("{file}: {e}"))?;
            std::io::Write::write_all(&mut f, B64.encode(private).as_bytes()).map_err(|e| e.to_string())?;
            Ok(format!("Private key saved to {file}. Keep it secret and backed up, outside the repository.\nPublic key (put this one line in crates/license/erg-license.pub):\n{public}"))
        }
        [cmd, file, seats, days, school @ ..] if cmd == "sign" && !school.is_empty() => {
            let private = B64.decode(std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?.trim()).map_err(|e| e.to_string())?;
            let seats: u32 = seats.parse().ok().filter(|n| *n > 0).ok_or("seats: a number above 0")?;
            let days: u64 = days.parse().map_err(|_| "days: a number, 0 = never expires")?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
            let expires = if days == 0 { 0 } else { now + days * 86_400 };
            erg_license::sign(&erg_license::Licence { school: school.join(" "), seats, expires }, &private)
        }
        _ => Err("usage:\n  erg-license keygen <private-key-file>\n  erg-license sign <private-key-file> <seats> <days> <school name>".into()),
    }
}

fn main() {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(s) => println!("{s}"),
        Err(e) => { eprintln!("{e}"); std::process::exit(1) }
    }
}
