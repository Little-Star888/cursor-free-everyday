use colored::*;
use directories::{BaseDirs, UserDirs};
use is_elevated::is_elevated;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sysinfo::{System};
use uuid::Uuid;
use chrono::Local;
use rand::{thread_rng, Rng, distributions::Alphanumeric};

// Color definitions (approximated from PowerShell)
const RED: &str = "red";
const GREEN: &str = "green";
const YELLOW: &str = "yellow";
const BLUE: &str = "blue";
// const NC: &str = "clear"; // `colored` crate handles reset implicitly or via `.normal()`

// Max retries and wait time for process termination
const MAX_RETRIES: u32 = 5;
const WAIT_TIME_SECONDS: u64 = 1;

// Configuration file paths
fn get_storage_file_path() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        let app_data_dir = base_dirs.config_dir(); // Typically %APPDATA% or ~/.config
        Some(app_data_dir.join("Cursor").join("User").join("globalStorage").join("storage.json"))
    } else {
        None
    }
}

fn get_backup_dir_path() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        let app_data_dir = base_dirs.config_dir();
        Some(app_data_dir.join("Cursor").join("User").join("globalStorage").join("backups"))
    } else {
        None
    }
}

fn get_cursor_package_path() -> Option<PathBuf> {
    if let Some(user_dirs) = BaseDirs::new() {
        let local_app_data_dir = user_dirs.data_local_dir();
        let primary_path = local_app_data_dir.join("Programs").join("cursor").join("resources").join("app").join("package.json");
        if primary_path.exists() {
            return Some(primary_path);
        }
        let alt_path = local_app_data_dir.join("cursor").join("resources").join("app").join("package.json");
        if alt_path.exists() {
            return Some(alt_path);
        }
    }
    None
}

fn get_cursor_updater_path() -> Option<PathBuf> {
    if let Some(user_dirs) = BaseDirs::new() {
        let local_app_data_dir = user_dirs.data_local_dir();
        Some(local_app_data_dir.join("cursor-updater"))
    } else {
        None
    }
}


fn press_enter_to_exit(exit_code: i32) {
    print!("Press Enter to exit...");
    io::stdout().flush().unwrap();
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    std::process::exit(exit_code);
}

fn main() {
    // Set output encoding to UTF-8 (Rust strings are UTF-8 by default, console might need setup on Windows)
    // On Windows, `chcp 65001` might be needed in the terminal before running for full UTF-8 display.
    // The script itself cannot reliably change the parent console's encoding.

    // Check administrator privileges
    if !is_elevated() {
        println!("{}", "[ERROR] Please run this script as administrator".color(RED));
        println!("Right-click the executable and select 'Run as administrator'");
        press_enter_to_exit(1);
    }

    // Display Logo
    // Using simple print for now, can be enhanced
    Command::new("cmd").args(&["/c", "cls"]).status().unwrap(); // Clear screen on Windows

    println!("{}", r#"
    ██████╗██╗   ██╗██████╗ ███████╗ ██████╗ ██████╗ 
   ██╔════╝██║   ██║██╔══██╗██╔════╝██╔═══██╗██╔══██╗
   ██║     ██║   ██║██████╔╝███████╗██║   ██║██████╔╝
   ██║     ██║   ██║██╔══██╗╚════██║██║   ██║██╔══██╗
   ╚██████╗╚██████╔╝██║  ██║███████║╚██████╔╝██║  ██║
    ╚═════╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝

"#.bright_cyan());
    println!("{}", "================================".color(BLUE));
    println!("   {}", "Cursor Device ID Modifier Tool".color(GREEN));
    println!("  {}", "Cursor ID Reset Tool - Community Edition".color(YELLOW));
    println!("  {}", "Free tool for Cursor device ID management".color(YELLOW));
    println!("  {}", "[IMPORTANT] This is a free community tool".color(YELLOW));
    println!("{}", "================================".color(BLUE));
    println!("  {}", "QQ群: 951642519 (交流/下载纯免费自动账号切换工具)".color(YELLOW));
    println!("");

    // Get and display Cursor version
    let cursor_version = get_cursor_version();
    match &cursor_version {
        Some(version) => println!("{} Current Cursor version: v{}", "[INFO]".color(GREEN), version),
        None => {
            println!("{} Unable to detect Cursor version", "[WARNING]".color(YELLOW));
            println!("{} Please ensure Cursor is properly installed", "[TIP]".color(YELLOW));
        }
    }
    println!("");

    println!("{} Latest 0.45.x (supported)", "[IMPORTANT NOTE]".color(YELLOW));
    println!("");

    // Check and close Cursor processes
    println!("{} Checking Cursor processes...", "[INFO]".color(GREEN));
    close_cursor_process("Cursor");
    close_cursor_process("cursor");
    println!("");

    let storage_file_path = match get_storage_file_path() {
        Some(path) => path,
        None => {
            println!("{}", "[ERROR] Could not determine APPDATA path for storage file.".color(RED));
            press_enter_to_exit(1);
            unreachable!(); // press_enter_to_exit exits
        }
    };
    // println!("Storage file path: {:?}", storage_file_path);

    let backup_dir_path = match get_backup_dir_path() {
        Some(path) => path,
        None => {
            println!("{}", "[ERROR] Could not determine APPDATA path for backup directory.".color(RED));
            press_enter_to_exit(1);
            unreachable!();
        }
    };
    // println!("Backup dir path: {:?}", backup_dir_path);

    // Create backup directory
    if !backup_dir_path.exists() {
        match fs::create_dir_all(&backup_dir_path) {
            Ok(_) => println!("{} Created backup directory at {:?}", "[INFO]".color(GREEN), backup_dir_path),
            Err(e) => {
                println!("{} Failed to create backup directory at {:?}: {}", "[ERROR]".color(RED), backup_dir_path, e);
                press_enter_to_exit(1);
            }
        }
    }

    // Backup existing configuration
    if storage_file_path.exists() {
        println!("{} Backing up configuration file...", "[INFO]".color(GREEN));
        let backup_name = format!("storage.json.backup_{}", Local::now().format("%Y%m%d_%H%M%S"));
        let backup_file_path = backup_dir_path.join(backup_name);
        match fs::copy(&storage_file_path, &backup_file_path) {
            Ok(_) => println!("{} Configuration backed up to {:?}", "[INFO]".color(GREEN), backup_file_path),
            Err(e) => {
                println!("{} Failed to backup configuration file to {:?}: {}", "[ERROR]".color(RED), backup_file_path, e);
                // Decide if this is a fatal error or a warning
            }
        }
    } else {
        println!("{} No existing configuration file found at {:?} to back up.", "[INFO]".color(GREEN), storage_file_path);
    }
    println!("");

    // Generate new IDs
    println!("{} Generating new IDs...", "[INFO]".color(GREEN));
    let mac_machine_id = new_standard_machine_id();
    let uuid_str = Uuid::new_v4().to_string();
    let prefix_hex = "auth0|user_".as_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let random_part = get_random_hex(32);
    let machine_id = format!("{}{}", prefix_hex, random_part);
    let sqm_id = format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase());

    // println!("Generated MAC_MACHINE_ID: {}", mac_machine_id);
    // println!("Generated UUID_STR: {}", uuid_str);
    // println!("Generated MACHINE_ID: {}", machine_id);
    // println!("Generated SQM_ID: {}", sqm_id);
    // println!("");

    // Create or update configuration file
    println!("{} Updating configuration...", "[INFO]".color(GREEN));
    let storage_update_successful = update_storage_file(
        &storage_file_path,
        &machine_id,
        &mac_machine_id,
        &uuid_str, // This was $UUID in PowerShell, which corresponds to devDeviceId
        &sqm_id
    );

    if storage_update_successful {
        println!("{} Configuration updated successfully.", "[INFO]".color(GREEN));
        // Display results
        println!("");
        println!("{} Configuration updated details:", "[INFO]".color(GREEN));
        println!("{} machineId: {}", "[DEBUG]".color(BLUE), machine_id);
        println!("{} macMachineId: {}", "[DEBUG]".color(BLUE), mac_machine_id);
        println!("{} devDeviceId: {}", "[DEBUG]".color(BLUE), uuid_str);
        println!("{} sqmId: {}", "[DEBUG]".color(BLUE), sqm_id);
    } else {
        println!("{} Main operation failed to update storage file.", "[ERROR]".color(RED));
        // The PS script has an alternative method here, which is complex.
        // For now, we'll just indicate failure.
        press_enter_to_exit(1);
    }
    println!("");

    // Display file tree structure
    println!("{} File structure:", "[INFO]".color(GREEN));
    if let Some(user_dirs) = UserDirs::new() {
        // %APPDATA%\Cursor\User is not directly available via UserDirs or BaseDirs in a cross-platform way for this specific structure.
        // We'll construct it based on APPDATA which UserDirs doesn't directly give, BaseDirs::config_dir() is the closest.
        if let Some(base_dirs) = BaseDirs::new() {
             let app_data_dir_equivalent = base_dirs.config_dir(); // This is platform specific, e.g. %APPDATA% on Windows
             println!("{}", app_data_dir_equivalent.join("Cursor").join("User").display().to_string().color(BLUE));
        }
    } else {
        println!("{} Could not determine APPDATA path for display.", "[WARNING]".color(YELLOW));
    }
    println!("├── globalStorage");
    println!("│   ├── storage.json (modified)");
    println!("│   └── backups");

    // List backup files
    match fs::read_dir(&backup_dir_path) {
        Ok(entries) => {
            let mut backup_files_found = false;
            for entry in entries {
                if let Ok(entry) = entry {
                    if entry.path().is_file() {
                        println!("│       └── {}", entry.file_name().to_string_lossy());
                        backup_files_found = true;
                    }
                }
            }
            if !backup_files_found {
                println!("│       └── (empty)");
            }
        }
        Err(e) => {
            println!("│       └── (Error reading backups: {})", e);
        }
    }
    println!("");

    // Display completion message
    println!("{}", "================================".color(GREEN));
    println!("  {}", "Cursor ID Reset Tool - Community Edition".color(YELLOW));
    println!("{}", "================================".color(GREEN));
    println!("");
    println!("{} Please restart Cursor to apply new configuration", "[INFO]".color(GREEN));
    println!("");

    press_enter_to_exit(0);
}

fn get_random_hex(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect::<String>()
        .to_lowercase() // PowerShell version produces lowercase hex
}

fn new_standard_machine_id() -> String {
    // Template: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    // y is one of 8, 9, a, b
    let mut rng = thread_rng();
    let mut id = String::with_capacity(36);
    for (i, char_template) in "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".chars().enumerate() {
        if char_template == '-' || char_template == '4' {
            id.push(char_template);
        } else if char_template == 'x' {
            id.push_str(&format!("{:x}", rng.gen_range(0..16)));
        } else if char_template == 'y' {
            id.push_str(&format!("{:x}", rng.gen_range(8..12))); // 8, 9, a, b
        }
    }
    id
}

#[derive(Deserialize)]
struct PackageJson {
    version: String,
}

fn get_cursor_version() -> Option<String> {
    if let Some(package_path) = get_cursor_package_path() {
        if package_path.exists() {
            match fs::read_to_string(&package_path) {
                Ok(contents) => match serde_json::from_str::<PackageJson>(&contents) {
                    Ok(json) => Some(json.version),
                    Err(e) => {
                        println!("{} Failed to parse package.json: {}", "[ERROR]".color(RED), e);
                        None
                    }
                },
                Err(e) => {
                    println!("{} Failed to read package.json at {:?}: {}", "[ERROR]".color(RED), package_path, e);
                    None
                }
            }
        } else {
            println!("{} package.json not found at {:?}", "[WARNING]".color(YELLOW), package_path);
            None
        }
    } else {
        println!("{} Could not determine path to Cursor's package.json", "[WARNING]".color(YELLOW));
        None
    }
}

fn close_cursor_process(process_name: &str) {
    let mut sys = System::new_all();
    sys.refresh_processes();

    let processes_to_kill: Vec<_> = sys
        .processes()
        .values()
        .filter(|p| p.name().eq_ignore_ascii_case(process_name))
        .collect();

    if !processes_to_kill.is_empty() {
        println!("{} Found {} running", "[WARNING]".color(YELLOW), process_name);
        for p in &processes_to_kill {
            println!("  PID: {}, Name: {}, Path: {:?}", p.pid(), p.name(), p.exe());
        }

        println!("{} Attempting to close {}...", "[WARNING]".color(YELLOW), process_name);
        for p in processes_to_kill {
            if !p.kill() { // kill() sends SIGKILL by default on Unix, TerminateProcess on Windows
                println!("{} Failed to send termination signal to {} (PID: {}). Trying to wait...", "[ERROR]".color(RED), process_name, p.pid());
            }
        }

        let mut retry_count = 0;
        loop {
            sys.refresh_processes();
            let still_running: Vec<_> = sys
                .processes()
                .values()
                .filter(|p| p.name().eq_ignore_ascii_case(process_name))
                .collect();

            if still_running.is_empty() {
                break;
            }

            retry_count += 1;
            if retry_count >= MAX_RETRIES {
                println!("{} Unable to close {} after {} attempts", "[ERROR]".color(RED), process_name, MAX_RETRIES);
                for p in still_running {
                     println!("  Still running - PID: {}, Name: {}, Path: {:?}", p.pid(), p.name(), p.exe());
                }
                println!("{} Please close the process manually and try again", "[ERROR]".color(RED));
                press_enter_to_exit(1);
            }

            println!("{} Waiting for process to close, attempt {}/{}...", "[WARNING]".color(YELLOW), retry_count, MAX_RETRIES);
            std::thread::sleep(std::time::Duration::from_secs(WAIT_TIME_SECONDS));
        }
        println!("{} {} successfully closed", "[INFO]".color(GREEN), process_name);
    }
}

fn update_storage_file(
    storage_file_path: &Path,
    machine_id: &str,
    mac_machine_id: &str,
    dev_device_id: &str,
    sqm_id: &str,
) -> bool {
    if !storage_file_path.exists() {
        println!("{} Configuration file not found: {:?}", "[ERROR]".color(RED), storage_file_path);
        println!("{} Please install and run Cursor once before using this script", "[TIP]".color(YELLOW));
        return false;
    }

    let original_content = match fs::read_to_string(storage_file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("{} Failed to read configuration file {:?}: {}", "[ERROR]".color(RED), storage_file_path, e);
            return false;
        }
    };

    let mut config: Value = match serde_json::from_str(&original_content) {
        Ok(json_value) => json_value,
        Err(e) => {
            println!("{} Failed to parse configuration file JSON: {}", "[ERROR]".color(RED), e);
            // Attempt to restore original content is not applicable here as we haven't written yet
            return false;
        }
    };

    // Ensure the path to telemetry values exists or create it
    // serde_json::Value uses `pointer_mut` for this kind of access.
    // Example: /telemetry/machineId
    // We need to ensure `config["telemetry"]` is an object.
    if !config.get("telemetry").map_or(false, |v| v.is_object()) {
        if config.as_object_mut().is_some() { // Check if config itself is an object
            config["telemetry"] = serde_json::json!({});
        } else {
            println!("{} Configuration root is not a JSON object. Cannot set telemetry.", "[ERROR]".color(RED));
            return false;
        }
    }
    
    // Update specific values
    // Using .get_mut("telemetry") and then working with the resulting Option<&mut Value>
    if let Some(telemetry) = config.get_mut("telemetry") {
        if let Some(telemetry_obj) = telemetry.as_object_mut() {
            telemetry_obj.insert("machineId".to_string(), Value::String(machine_id.to_string()));
            telemetry_obj.insert("macMachineId".to_string(), Value::String(mac_machine_id.to_string()));
            telemetry_obj.insert("devDeviceId".to_string(), Value::String(dev_device_id.to_string()));
            telemetry_obj.insert("sqmId".to_string(), Value::String(sqm_id.to_string()));
        } else {
            println!("{} 'telemetry' field is not an object.", "[ERROR]".color(RED));
            return false; // Or attempt to restore original_content
        }
    } else {
        // This case should ideally be covered by the creation logic above.
        println!("{} Failed to access or create 'telemetry' object.", "[ERROR]".color(RED));
        return false; // Or attempt to restore original_content
    }
    
    match serde_json::to_string_pretty(&config) { // Using pretty for readability, PowerShell does compact
        Ok(updated_json) => {
            match fs::write(storage_file_path, updated_json.as_bytes()) { // .as_bytes() for UTF-8
                Ok(_) => {
                    println!("{} Configuration file updated successfully at {:?}", "[INFO]".color(GREEN), storage_file_path);
                    true
                }
                Err(e) => {
                    println!("{} Failed to write updated configuration to {:?}: {}", "[ERROR]".color(RED), storage_file_path, e);
                    // Attempt to restore original content
                    if fs::write(storage_file_path, original_content.as_bytes()).is_err() {
                        println!("{} CRITICAL: Failed to restore original content to {:?} after write error.", "[ERROR]".color(RED), storage_file_path);
                    }
                    false
                }
            }
        }
        Err(e) => {
            println!("{} Failed to serialize updated configuration to JSON: {}", "[ERROR]".color(RED), e);
            // Attempt to restore original content if we had changed it in memory (not the case here with direct write path)
            // No need to restore file if serialization failed before writing.
            false
        }
    }
}
