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
    /// Configures Apache HTTPD VirtualHost
    HttpdVhost(HttpdVhostArgs),
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

#[derive(clap::Args)]
struct HttpdVhostArgs {
    /// The domain name for the VirtualHost
    #[arg(value_name = "DOMAIN")]
    domain: String,

    /// The IP address Apache should listen on for this VirtualHost
    /// Default to 0.0.0.0 (all interfaces)
    #[arg(long, short, default_value = "0.0.0.0")]
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
        Some(Commands::HttpdVhost(args)) => {
            println!("Configuring Apache HTTPD VirtualHost:");
            println!("\tDomain:\t{}", args.domain);
            println!("\tIP:\t{}", args.ip);
            handle_httpd_vhost_command(&args.domain, &args.ip)?;
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

    for line_result in reader.lines() {
        let mut line = line_result?;
        let trimmed_line = line.trim();

        if !found_listen_on && trimmed_line.starts_with("listen-on port 53") {
            let re = Regex::new(r"(listen-on port 53\s*\{[^}]*)(\};)").unwrap();
            if let Some(caps) = re.captures(&line.clone()) {
                let current_content = caps.get(1).unwrap().as_str();
                let closing_bracket = caps.get(2).unwrap().as_str();
                if !current_content.contains("any;") {
                    if current_content.trim().ends_with('{') {
                        line = format!(
                            "{} 127.0.0.1; any; {}",
                            current_content.trim_end_matches('{').trim(),
                            closing_bracket
                        );
                    } else if current_content.trim().ends_with(';') {
                        line = format!("{current_content} any;{closing_bracket}");
                    } else {
                        line = format!("{current_content}; any;{closing_bracket}");
                    }
                }
            }
            found_listen_on = true;
        } else if !found_allow_query && trimmed_line.starts_with("allow-query") {
            let re = Regex::new(r"(Allow-query\s*\{[^}]*)(\};)").unwrap();
            if let Some(caps) = re.captures(&line.clone()) {
                let current_content = caps.get(1).unwrap().as_str();
                let closing_bracket = caps.get(2).unwrap().as_str();
                if !current_content.contains("any;") {
                    if current_content.trim().ends_with('{') {
                        line = format!(
                            "{} localhost; any; {}",
                            current_content.trim_end_matches('{').trim(),
                            closing_bracket
                        );
                    } else if current_content.trim().ends_with(';') {
                        line = format!("{current_content} any;{closing_bracket}");
                    } else {
                        line = format!("{current_content}; any;{closing_bracket}");
                    }
                }
            }
            found_allow_query = true;
        }

        writeln!(writer, "{}", line)?;
    }
    writer.flush()?;

    let zone_definition = format!(
        r#"
    zone "{domain}" IN {{
        type master;
        file "/var/named/{domain}.hosts";
    }}"#
    );

    let reader = BufReader::new(File::open(temp_path)?);
    let mut exists = false;
    for line_result in reader.lines() {
        let line = line_result?;
        if line.contains(&format!("zone \"{domain}\" IN")) {
            exists = true;
            break;
        }
    }

    if !exists {
        let mut writer_append = OpenOptions::new()
            .write(true)
            .append(true)
            .open(temp_path)?;
        writeln!(writer_append, "{}", zone_definition)?;
    } else {
    }

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
                    {serial} ; serial
                    10800 ; refresh
                    3600 ; retry
                    604800 ; expire
                    38400 ; minimum
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

fn handle_httpd_vhost_command(domain: &str, ip: &str) -> io::Result<()> {
    let vhost_conf_dir = PathBuf::from("/etc/httpd/conf.d");
    let document_root_dir = PathBuf::from(format!("/var/www/html/{domain}"));
    let vhost_conf_path = vhost_conf_dir.join(format!("{domain}.conf"));
    let index_html_path = document_root_dir.join("index.html");

    fs::create_dir_all(&document_root_dir)?;

    let welcome_content = format!(
        r#"<!DOCTYPE html>
        <html lang="en">
        <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Welcome to {domain}</title>
        </head>
        <body>
        <p>boas-vindas</p>
        </body>
        </html>"#,
    );

    let mut index_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&index_html_path)?;
    index_file.write_all(welcome_content.as_bytes())?;

    let vhost_content = format!(
        r#"<VirtualHost {ip}:80>
            ServerName {domain}
            ServerAlias www.{domain}
            DocumentRoot "{}"
            <Directory "{}" >
                Options Indexes FollowSymLinks
                AllowOverride All
                Order allow,deny
                Allow from all
                Require method GET POST OPTIONS
            </Directory>
        </VirtualHost>"#,
        document_root_dir.display(),
        document_root_dir.display()
    );

    let mut vhost_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&vhost_conf_path)?;
    vhost_file.write_all(vhost_content.as_bytes())?;

    let httpd_conf_path = PathBuf::from("/etc/httpd/conf/httpd.conf");
    ensure_httpd_listen_directive(&httpd_conf_path, ip)?;

    println!(
        "Apache HTTPD VirtualHost setup complete (manual `systemctl restart httpd` still needed)"
    );
    Ok(())
}

fn ensure_httpd_listen_directive(httpd_conf_path: &Path, ip: &str) -> io::Result<()> {
    let file = File::open(httpd_conf_path)?;
    let reader = BufReader::new(file);

    let temp_file = NamedTempFile::new_in(
        httpd_conf_path
            .parent()
            .unwrap_or_else(|| Path::new("/tmp")),
    )?;
    let temp_path = temp_file.path();
    let mut writer = BufWriter::new(File::create(temp_path)?);

    let mut found_listen = false;
    let listen_directive_to_add = format!("Listen {}:80", ip);

    for line_result in reader.lines() {
        let line = line_result?;
        let trimmed_line = line.trim();

        if trimmed_line.starts_with("Listen ") && trimmed_line.contains(":80") {
            found_listen = true;
            writeln!(writer, "{}", line)?;
        } else if trimmed_line.starts_with("Listen 80") {
            found_listen = true;
            writeln!(writer, "{}", line)?;
        } else {
            writeln!(writer, "{}", line)?;
        }
    }
    writer.flush()?;

    if !found_listen {
        let mut writer_append = OpenOptions::new()
            .write(true)
            .append(true)
            .open(temp_path)?;
        writeln!(writer_append, "\n{}", listen_directive_to_add)?;
    }

    fs::rename(temp_path, httpd_conf_path)?;
    Ok(())
}
