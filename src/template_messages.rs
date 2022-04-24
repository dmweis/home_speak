use chrono::prelude::*;
use local_ip_address::list_afinet_netifas;
use log::*;
use num_traits::FromPrimitive;
use std::process::Command;
use std::str;

pub fn generate_startup_message() -> String {
    let mut message_buffer = String::new();
    message_buffer.push_str("Good morning! I have just woken up. ");
    message_buffer.push_str(&human_current_time());
    // TODO(David): Extract this
    // probably use some templateing engine too
    if let Ok(network_interfaces) = list_afinet_netifas() {
        if network_interfaces.is_empty() {
            error!("No NICs found");
            message_buffer.push_str("Huh. It looks like this device has no network interfaces? ")
        } else {
            message_buffer.push_str("I am detecting the following network interfaces. ");
            let interface_message: String = network_interfaces
                .iter()
                .filter(|(_, ip)| ip.is_ipv4() && !ip.is_loopback())
                .map(|(name, ip)| format!("{} at {}. ", name, ip))
                .collect();
            info!("local interfaces are: {:?}", interface_message);
            message_buffer.push_str(&interface_message);
        }
    } else {
        error!("Failed to query local network interfaces");
        message_buffer.push_str("I can't tell you how to reach me because it looks like I failed to query the local interfaces for some reason. ");
    }

    if let Some(hostname) = hostname() {
        message_buffer.push_str(&format!("My hostname is {}. ", hostname));
    } else {
        message_buffer
            .push_str("I can't detect my hostname. Maybe this platform isn't supported? ");
    }

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

fn human_current_time() -> String {
    // more janky.
    // TODO(David): Use a real humanizer library
    let local: chrono::DateTime<chrono::Local> = chrono::Local::now();

    format!(
        "Currently it is {}th of {:?}, {} at {}:{}. ",
        local.day(),
        Month::from_u32(local.month()),
        local.year(),
        local.hour(),
        local.minute()
    )
}
