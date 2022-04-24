use chrono::prelude::*;
use local_ip_address::list_afinet_netifas;
use log::*;
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use std::process::Command;
use std::str;

pub fn generate_startup_message(port: u16) -> Vec<String> {
    let mut message_buffer = vec![];
    message_buffer.push(String::from("Good morning, my name is Joy!"));
    message_buffer.push(format!("It's {}. ", human_current_date_time()));
    // TODO(David): Extract this
    // probably use some templating engine too
    if let Ok(network_interfaces) = list_afinet_netifas() {
        if network_interfaces.is_empty() {
            error!("No NICs found");
            message_buffer.push(String::from(
                "Huh, It looks like this device has no network interfaces?",
            ))
        } else {
            let mut network_message = String::new();
            network_message.push_str("My network interfaces are ");
            let interface_message: String = network_interfaces
                .iter()
                .filter(|(_, ip)| ip.is_ipv4() && !ip.is_loopback())
                .map(|(name, ip)| format!("{} at {}, ", name, ip))
                .collect();
            info!("local interfaces are: {:?}", interface_message);
            network_message.push_str(interface_message.trim_end_matches(", "));
            network_message.push('.');
            message_buffer.push(network_message);
        }
    } else {
        error!("Failed to query local network interfaces");
        message_buffer.push(String::from("I can't tell you how to reach me because it looks like I failed to query the local interfaces for some reason."));
    }

    if let Some(hostname) = hostname() {
        message_buffer.push(format!("My hostname is {}. ", hostname));
    } else {
        message_buffer.push(String::from(
            "I can't detect my hostname. Maybe this platform isn't supported?",
        ));
    }
    message_buffer.push(format!("My server is running on port {}. ", port));

    message_buffer
}

// This isn't a particularly great solution
#[cfg(target_os = "linux")]
fn hostname() -> Option<String> {
    if let Ok(output) = Command::new("hostname").output() {
        if let Ok(hostname) = str::from_utf8(&output.stdout) {
            Some(hostname.to_owned())
        } else {
            error!("Failed to convert output of hostname command");
            None
        }
    } else {
        error!("Failed to run hostname command");
        None
    }
}

#[cfg(not(target_os = "linux"))]
fn hostname() -> Option<String> {
    None
}

// This could use a humanizer library.
// But this is okay for now I think

pub fn human_current_date_time() -> String {
    let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
    human_date_time(now)
}

pub fn human_current_time() -> String {
    let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
    human_time(now)
}

pub fn human_time(date_time: chrono::DateTime<chrono::Local>) -> String {
    format!("{}:{:02}, ", date_time.hour(), date_time.minute())
}

pub fn human_date_time(date_time: chrono::DateTime<chrono::Local>) -> String {
    format!(
        "{}, {} of {:?}, {} at {}:{:02}, ",
        date_time.weekday(),
        Ordinal(date_time.day()),
        Month::from_u32(date_time.month()).unwrap(),
        date_time.year(),
        date_time.hour(),
        date_time.minute()
    )
}
