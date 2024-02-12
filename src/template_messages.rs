use chrono::prelude::*;
use local_ip_address::list_afinet_netifas;
use num_traits::FromPrimitive;
use ordinal::Ordinal;
use std::net::IpAddr;
use std::{fmt::Write, process::Command, str};
use tracing::*;

use crate::configuration::AssistantConfig;

#[derive(Debug, Clone)]
pub struct TemplateEngine {
    assistant_config: AssistantConfig,
    hostname: Option<String>,
    network_interfaces: Option<Vec<(String, IpAddr)>>,
}

impl TemplateEngine {
    pub fn new(assistant_config: AssistantConfig) -> Self {
        let hostname = hostname();
        let network_interfaces = network_interfaces();
        Self {
            assistant_config,
            hostname,
            network_interfaces,
        }
    }

    pub fn startup_message(&self) -> Vec<String> {
        let mut message_buffer = vec![];
        message_buffer.push(format!(
            "Good morning, my name is {}!",
            self.assistant_config.name
        ));
        message_buffer.push(format!("It's {}. ", get_human_current_date_time()));
        if let Some(ref network_interfaces) = self.network_interfaces {
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
                    .fold(String::new(), |mut output, (name, ip)| {
                        let _ = write!(output, "{} at {}, ", name, ip);
                        output
                    });
                info!("local interfaces are: {:?}", interface_message);
                network_message.push_str(interface_message.trim_end_matches(", "));
                network_message.push('.');
                message_buffer.push(network_message);
            }
        } else {
            error!("Failed to query local network interfaces");
            message_buffer.push(String::from("I can't tell you how to reach me because it looks like I failed to query the local interfaces for some reason."));
        }

        if let Some(ref hostname) = self.hostname {
            message_buffer.push(format!("My hostname is {}. ", hostname));
        } else {
            message_buffer.push(String::from(
                "I can't detect my hostname. Maybe this platform isn't supported?",
            ));
        }

        message_buffer
    }

    pub fn template_substitute(message: &str) -> String {
        let current_time = get_human_current_time();
        let current_date_time = get_human_current_date_time();
        message
            .replace("/time", &current_time)
            .replace("/date", &current_date_time)
    }
}

fn network_interfaces() -> Option<Vec<(String, IpAddr)>> {
    list_afinet_netifas().ok()
}

// This isn't a particularly great solution
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

// This could use a humanizer library.
// But this is okay for now I think

pub fn get_human_current_date_time() -> String {
    let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
    humanize_date_time(now)
}

pub fn get_human_current_time() -> String {
    let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
    humanize_time(now)
}

fn humanize_time(date_time: chrono::DateTime<chrono::Local>) -> String {
    format!("{}:{:02}, ", date_time.hour(), date_time.minute())
}

fn humanize_date_time(date_time: chrono::DateTime<chrono::Local>) -> String {
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
