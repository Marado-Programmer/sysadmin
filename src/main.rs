use chrono::Utc;
use clap::{Parser, Subcommand};
use regex::Regex;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
    /// Manages DNS master zones
    DnsMasterZone(DnsMasterZoneArgs),
}

#[derive(clap::Args)]
struct DnsMasterZoneArgs {
    /// The domain name to create the master zone for
    #[arg(value_name = "DOMAIN")]
    domain: String,

    /// The IP address of the DNS/Web server
    #[arg(long, short, default_value = "192.168.0.1")]
    ip: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // You can check the value provided by positional arguments, or option arguments
    if let Some(name) = cli.name.as_deref() {
        println!("Value for name: {name}");
    }

    if let Some(config_path) = cli.config.as_deref() {
        println!("Value for config: {}", config_path.display());
    }

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => println!("Debug mode is off"),
        1 => println!("Debug mode is kind of on"),
        2 => println!("Debug mode is on"),
        _ => println!("Don't be crazy"),
    }

    match &cli.command {
        Some(Commands::Test { list }) => {
            if *list {
                println!("Printing testing lists...");
            } else {
                println!("Not printing testing lists...");
            }
        }
        Some(Commands::DnsMasterZone(args)) => {
            println!("Creating DNS Master Zone:");
            println!("\tDomain:\t{}", args.domain);
            println!("\tIP:\t{}", args.ip);
            handle_dns_master_zone_command(&args.domain, &args.ip)?;
        }
        None => {}
    }

    Ok(())
}

fn handle_dns_master_zone_command(domain: &str, ip: &str) -> io::Result<()> {
    let named_conf_path = PathBuf::from("/etc/named.conf");
    let zone_file_path = PathBuf::from(format!("/var/named/{domain}.hosts"));

    edit_named_conf(&named_conf_path, domain)?;

    create_zone_file(&zone_file_path, domain, ip)?;

    println!("DNS Master Zone setup complete (manual `systemctl restart named` still needed)");
    Ok(())
}

fn edit_named_conf(named_conf_path: &Path, domain: &str) -> io::Result<()> {
    let file = File::open(named_conf_path)?;
    let reader = BufReader::new(file);

    let temp_file = NamedTempFile::new_in(
        named_conf_path
            .parent()
            .unwrap_or_else(|| Path::new("/tmp")),
    )?;
    let temp_path = temp_file.path();
    let mut writer = BufWriter::new(File::create(temp_path)?);

    let mut found_listen_on = false;
    let mut found_allow_query = false;
    let mut named_conf_content: Vec<String> = Vec::new();

    for line_result in reader.lines() {
        let mut line = line_result?;
        let trimmed_line = line.trim();

        if !found_listen_on && trimmed_line.starts_with("listen-on port 53") {
            let re = Regex::new(r"listen-on port 53\s*\{([^}]*)\};").unwrap();
            if let Some(caps) = re.captures(&line) {
                let current_ips = caps.get(1).map_or("", |m| m.as_str());
                if !current_ips.contains("any;") {
                    if current_ips.trim().is_empty() {
                        line = format!("listen-on port 53 {{ 127.0.0.1; any; }};");
                    } else {
                        line = line.replace(current_ips, &format!("{current_ips} any;"));
                    }
                }
            }
            found_listen_on = true;
        } else if !found_allow_query && trimmed_line.starts_with("allow-query") {
            let re = Regex::new(r"Allow-query\s*\{([^}]*)\};").unwrap();
            if let Some(caps) = re.captures(&line) {
                let current_hosts = caps.get(1).map_or("", |m| m.as_str());
                if !current_hosts.contains("any;") {
                    if current_hosts.trim().is_empty() {
                        line = format!("Allow-query {{ localhost; any; }};");
                    } else {
                        line = line.replace(current_hosts, &format!("{current_hosts} any;"));
                    }
                }
            }
            found_allow_query = true;
        }

        named_conf_content.push(line);
    }

    let zone_definition = format!(
        r#"zone "{domain}" IN {{
            type master;
            file "/var/named/{domain}.hosts";
        }}"#
    );

    if !named_conf_content
        .iter()
        .any(|l| l.contains(&format!("zone \"{domain}\" IN")))
    {
        named_conf_content.push(zone_definition);
    }

    for line in named_conf_content {
        writeln!(writer, "{}", line)?;
    }
    writer.flush()?;

    fs::rename(temp_path, named_conf_path)?;
    Ok(())
}

fn create_zone_file(zone_file_path: &Path, domain: &str, ip: &str) -> io::Result<()> {
    if let Some(parent) = zone_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let serial = Utc::now().format("%Y%m%d%H").to_string();

    let content = format!(
        r#"$ttl 38400
    @   IN  SOA {domain}.   (
                {serial}    ; serial
                10800   ; refresh
                3600    ; retry
                604800  ; expire
                38400   ; minimum
                )
        IN  NS  {domain}.
        IN  A   {ip}
    "#
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(zone_file_path)?;

    file.write_all(content.as_bytes())?;
    Ok(())
}
