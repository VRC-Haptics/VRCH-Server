use std::env;
use std::io;
use winreg::enums::*;
use winreg::RegKey;
use dunce;

const BHAPTICS_KEY_PATH: &str = r"\bhaptics-app\shell\open\command";
const PROXY_PATH: &str = r".\sidecars\bHapticsPlayer\BhapticsPlayer.exe";

fn main() {

    println!("In main");
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: registry_modifier <set|reset>");
        std::process::exit(1);
    }

    let full_path = dunce::canonicalize(PROXY_PATH)
        .map_err(|e| io::Error::new(e.kind(), format!("Error canonicalizing PROXY_PATH: {}", e))).unwrap();
    let full_path_str = full_path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "Unable to convert full path to string")
    }).unwrap();
    println!("Canonical proxy path: {:?}", full_path_str);

    let command = args[1].as_str();
    let result = match command {
        "set" => set_registry(&full_path_str),
        "reset" => reset_registry(&full_path_str),
        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}

/// Opens (or creates) the registry key with KEY_WRITE and sets our proxy as the default.
/// It also stores a backup of the current default in the "actual" value.
fn set_registry(full_path: &str) -> io::Result<()> {
    println!("Starting set_registry");

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    println!("Got root");
    // Try opening the key with write access.
    let subkey_result = hkcr.open_subkey_with_flags(BHAPTICS_KEY_PATH, KEY_ALL_ACCESS);
    match subkey_result {
        Ok(key) => {
            // Try reading the current default value.
            let current_path: Result<String, _> = key.get_value("");
            println!("Matching Path");
            match current_path {
                Ok(ref current) if current == full_path => {
                    // Already set to our proxy – nothing to do.
                    println!("Proxy already set as default.");
                    return Ok(());
                },
                Ok(current) => {
                    // Save backup before replacing.
                    key.set_value("actual", &current)?;
                    key.set_value("", &full_path)?;
                    println!("Registry updated: proxy set as default.");
                },
                Err(err) => {
                    println!("Error in here: {:?}", err);
                    // No default value exists; simply set it.
                    key.set_value("", &full_path)?;
                    println!("Registry updated: default value set.");
                },
            }
        },
        Err(err) => {
            println!("In error {:?}", err);
            // If the key does not exist, create it with write access.
            let (key, _disp) =
                hkcr.create_subkey_with_flags(BHAPTICS_KEY_PATH, KEY_WRITE)?;
            key.set_value("", &full_path)?;
            println!("Registry key created and default value set.");
        },
    }
    Ok(())
}

/// Resets the registry default to the backed-up value stored in "actual".
fn reset_registry(full_path: &str) -> io::Result<()> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let key = hkcr.open_subkey_with_flags(BHAPTICS_KEY_PATH, KEY_ALL_ACCESS)?;
    let current_path: String = key.get_value("")?;
    if full_path == current_path {
        let backup: String = key.get_value("actual")?;
        key.set_value("", &backup)?;
        println!("Registry updated: default value reset from backup.");
    } else {
        println!("Registry default value is not set to our proxy; no reset needed.");
    }
    Ok(())
}
