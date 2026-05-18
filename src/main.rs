use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
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
    /// Manages DNS master zones (forward or reverse)
    DnsMasterZone(DnsMasterZoneArgs),
    /// Configures Apache HTTPD VirtualHost
    HttpdVhost(HttpdVhostArgs),
    /// Manages DNS records in an existing master zone file
    DnsRecord(DnsRecordArgs),
    /// Delete domain (DNS + Apache)
    DeleteDomain(DeleteDomainArgs),
    /// Add domain to blacklist
    BlacklistAdd(BlacklistArgs),
    /// Remove domain from blacklist
    BlacklistRemove(BlacklistArgs),
    // Manage NFS
    Nfs(NfsArgs),
    // Backup users' system config files
    Backup(BackupArgs),
    // creates a level 5 RAID
    Raid(RaidArgs),
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum ZoneType {
    Forward,
    Reverse,
}

#[derive(clap::Args)]
struct DnsMasterZoneArgs {
    /// The domain name or reverse zone name to create (e.g., example.com or 0.168.192.example.com)
    #[arg(value_name = "DOMAIN")]
    domain: String,

    /// The IP address for the DNS/Web server for forward zones, or the network for reverse zones (e.g., 192.168.0.0/24)
    #[arg(long, short)]
    ip: String,

    /// The type of zone to create (forward or reverse)
    #[arg(value_enum, long, short, default_value_t = ZoneType::Forward)]
    zone_type: ZoneType,

    /// The FQDN of the primary DNS server (e.g., dns.example.com) - required for all zones
    #[arg(long, short, value_name = "NS_FQDN")]
    ns_fqdn: String,

    /// The email for the SOA record (e.g., hostmaster.example.com)
    #[arg(long, short, default_value = "hostmaster.example.com")]
    soa_email: String,
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

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum DnsRecordType {
    A,
    AAAA,
    CNAME,
    MX,
    NS,
    PTR,
    // SOA is managed internally, not directly by user via this command
}

#[derive(clap::Args)]
struct DnsRecordArgs {
    /// The domain name (forward or reverse) for which to add or modify a record
    #[arg(value_name = "DOMAIN")]
    domain: String,

    /// The name of the host/subdomain (e.g., 'www', '@', 'mail', or '1' for PTR records)
    #[arg(value_name = "HOST")]
    host: String,

    /// The type of DNS record to create
    #[arg(value_enum)]
    record_type: DnsRecordType,

    /// The value for the DNS record (e.g., IP address, canonical name, mail host, FQDN for PTR)
    #[arg(value_name = "VALUE")]
    value: String,
}

#[derive(clap::Args)]
struct DeleteDomainArgs {
    /// Domain name (example.com)
    #[arg(short, long)]
    domain: String,

    /// Delete reverse zone too
    #[arg(long, default_value_t = false)]
    reverse: bool,

    /// IP (needed if reverse=true)
    #[arg(long)]
    ip: Option<String>,
}

#[derive(clap::Args)]
struct BlacklistArgs {
    #[arg(short, long)]
    domain: String,

    /// Redirect IP (default = sinkhole)
    #[arg(short, long, default_value = "0.0.0.0")]
    ip: String,

    /// The FQDN of the primary DNS server (e.g., dns.example.com) - required for all zones
    #[arg(long, short, value_name = "NS_FQDN")]
    ns_fqdn: String,

    /// The email for the SOA record (e.g., hostmaster.example.com)
    #[arg(long, short, default_value = "hostmaster.example.com")]
    soa_email: String,
}

#[derive(Subcommand)]
enum NfsAction {
    Add,
    Remove,
    Update,
    Disable,
}

#[derive(clap::Args)]
struct NfsArgs {
    #[command(subcommand)]
    action: NfsAction,

    /// Directory to share
    #[arg(short, long)]
    path: String,

    /// Allowed client (IP or subnet)
    #[arg(short, long)]
    client: String,

    /// Options (rw,sync,no_root_squash,...)
    #[arg(short, long, default_value = "ro,hide")]
    options: String,
}

#[derive(Subcommand)]
enum BackupAction {
    Tar,
    Rsync,
}

#[derive(clap::Args)]
struct BackupArgs {
    #[command(subcommand)]
    action: BackupAction,

    /// Source directory
    #[arg(short, long)]
    source: String,

    /// Destination path
    #[arg(short, long)]
    destination: String,
}

#[derive(clap::Args)]
struct RaidArgs {
    /// Mount point (directory)
    #[arg(short, long)]
    mount_point: String,

    /// Devices (e.g. /dev/sdb /dev/sdc /dev/sdd)
    #[arg(required = true)]
    devices: Vec<String>,

    /// Spare Devices (e.g. /dev/sdb /dev/sdc /dev/sdd)
    #[arg(required = false)]
    spare: Vec<String>,
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
            println!("Creating DNS Master Zone ({:?}):", args.zone_type);
            println!("\tDomain:\t{}", args.domain);
            println!("\tIP/Network:\t{}", args.ip);
            println!("\tNS FQDN:\t{}", args.ns_fqdn);
            println!("\tSOA Email:\t{}", args.soa_email);
            handle_dns_master_zone_command(args)?;
        }
        Some(Commands::HttpdVhost(args)) => {
            println!("Configuring Apache HTTPD VirtualHost:");
            println!("\tDomain:\t{}", args.domain);
            println!("\tIP:\t{}", args.ip);
            handle_httpd_vhost_command(&args.domain, &args.ip)?;
        }
        Some(Commands::DnsRecord(args)) => {
            println!("Adding/Modifying DNS Record:");
            println!("\tDomain:\t{}", args.domain);
            println!("\tHost:\t{}", args.host);
            println!("\tType:\t{:?}", args.record_type);
            println!("\tValue:\t{}", args.value);
            handle_dns_record_command(args)?;
        }
        Some(Commands::DeleteDomain(args)) => {
            println!("Deleting Domain:");
            println!("\tDomain:\t{}", args.domain);
            if let Some(ip) = &args.ip {
                println!("\tIP/Network:\t{}", ip);
            }
            handle_delete_domain(args)?;
        }
        Some(Commands::BlacklistAdd(args)) => {
            println!("Blacklisting Domain:");
            println!("\tDomain:\t{}", args.domain);
            handle_blacklist_add(args)?;
        }
        Some(Commands::BlacklistRemove(args)) => {
            println!("Whitelisting Domain:");
            println!("\tDomain:\t{}", args.domain);
            handle_delete_domain(&DeleteDomainArgs {
                domain: String::from(&args.domain),
                reverse: false,
                ip: Some(String::from(&args.ip)),
            })?;
        }
        Some(Commands::Nfs(args)) => {
            handle_nfs(args)?;
        }
        Some(Commands::Backup(args)) => {
            handle_backup(args)?;
        }
        Some(Commands::Raid(args)) => {
            handle_raid(args)?;
        }
        None => {}
    }

    Ok(())
}

fn handle_dns_master_zone_command(args: &DnsMasterZoneArgs) -> io::Result<()> {
    let zone_filename = format!("{}.hosts", args.domain);
    let named_conf_path = PathBuf::from("/etc/named.conf");
    let zone_file_path = PathBuf::from(format!("/var/named/{zone_filename}"));

    edit_named_conf(&named_conf_path, &args.domain)?;

    create_zone_file(
        &zone_file_path,
        &args.ip,
        &args.zone_type,
        &args.ns_fqdn,
        &args.soa_email,
    )?;

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

    let zone_filename = format!("{}.hosts", domain);

    let zone_definition = format!(
        r#"
    zone "{domain}" IN {{
        type master;
        file "/var/named/{zone_filename}";
    }};
    "#
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
    }

    fs::rename(temp_path, named_conf_path)?;
    Ok(())
}

fn create_zone_file(
    zone_file_path: &Path,
    ip_or_network: &str,
    zone_type: &ZoneType,
    ns_fqdn: &str,
    soa_email: &str,
) -> io::Result<()> {
    if let Some(parent) = zone_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let serial = Utc::now().format("%Y%m%d%H").to_string();

    let content = match zone_type {
        ZoneType::Forward => format!(
            r#"$ttl 38400
        @   IN  SOA {ns_fqdn}.  {soa_email}.    (
                            {serial} ; serial
                            10800 ; refresh
                            3600 ; retry
                            604800 ; expire
                            38400 ; minimum
                            )
            IN  NS  {ns_fqdn}.
            IN  A   {ip_or_network}
        "#
        ),
        ZoneType::Reverse => {
            let ptr_ip_fragment = ip_or_network.split('.').last().unwrap_or("1");

            format!(
                r#"$ttl 38400
            @   IN  SOA {ns_fqdn}.  {soa_email}.    (
                                {serial} ; serial
                                10800 ; refresh
                                3600 ; retry
                                604800 ; expire
                                38400 ; minimum
                                )
                IN  NS  {ns_fqdn}.
                IN  A   {ip_or_network}
            {ptr_ip_fragment}   IN  PTR {ns_fqdn}.
            "#
            )
        }
    };

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

        if trimmed_line == listen_directive_to_add {
            found_listen = true;
            writeln!(writer, "{}", line)?;
        } else if trimmed_line == "Listen 80" || trimmed_line == "Listen *:80" {
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

fn handle_dns_record_command(args: &DnsRecordArgs) -> io::Result<()> {
    let zone_file_path = PathBuf::from(format!("/var/named/{}.hosts", args.domain));

    if !zone_file_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Zone file '{}' not found. Please create the master zone first using `dns-master-zone`.",
                zone_file_path.display()
            ),
        ));
    }

    let file = File::open(&zone_file_path)?;
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = reader.lines().filter_map(Result::ok).collect();

    let mut updated_serial = None;
    let soa_regex = Regex::new(r"(\s+\d+)\s*;\s*serial").unwrap();

    for line in &mut lines {
        if line.contains("IN SOA") {
            if let Some(caps) = soa_regex.captures(line) {
                if let Some(serial_match) = caps.get(1) {
                    let current_serial_str = serial_match.as_str().trim();
                    if let Ok(current_serial) = current_serial_str.parse::<u64>() {
                        updated_serial = Some(current_serial + 1);
                        *line = line
                            .replace(current_serial_str, &format!("{}", updated_serial.unwrap()));
                        break;
                    }
                }
            }
        }
    }

    if updated_serial.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Could not find or parse SOA serial in zone file.",
        ));
    }

    let new_record_line = match args.record_type {
        DnsRecordType::A => format!("{}\tIN\tA\t{}", args.host, args.value),
        DnsRecordType::AAAA => format!("{}\tIN\tAAAA\t{}", args.host, args.value),
        DnsRecordType::CNAME => format!("{}\tIN\tCNAME\t{}", args.host, args.value),
        DnsRecordType::MX => format!("{}\tIN\tMX\t10\t{}", args.host, args.value),
        DnsRecordType::NS => format!("{}\tIN\tNS\t{}", args.host, args.value),
        DnsRecordType::PTR => format!("{}\tIN\tPTR\t{}", args.host, args.value),
    };

    lines.push(new_record_line.clone());

    let temp_file =
        NamedTempFile::new_in(zone_file_path.parent().unwrap_or_else(|| Path::new("/tmp")))?;
    let temp_path = temp_file.path();
    let mut writer = BufWriter::new(File::create(temp_path)?);

    for line in lines {
        writeln!(writer, "{}", line)?;
    }
    writer.flush()?;

    fs::rename(temp_path, &zone_file_path)?;

    Ok(())
}

fn handle_delete_domain(args: &DeleteDomainArgs) -> Result<(), Box<dyn std::error::Error>> {
    delete_dns_zone(&args.domain)?;
    delete_virtual_host(&args.domain)?;

    if args.reverse
        && let Some(ip) = &args.ip
    {
        delete_reverse_zone(&ip, &args.domain)?;
    }

    println!("Domain {} removed successfully", args.domain);
    Ok(())
}

fn delete_dns_zone(domain: &str) -> Result<(), Box<dyn std::error::Error>> {
    let named_conf_path = "/etc/named.conf";
    let zone_file = format!("/var/named/{}.hosts", domain);

    let content = std::fs::read_to_string(named_conf_path)?;
    let new_content = content
        .lines()
        .filter(|line| !line.contains(&format!("\"{}\"", domain)))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(named_conf_path, new_content)?;

    if std::path::Path::new(&zone_file).exists() {
        std::fs::remove_file(zone_file)?;
    }

    println!("DNS zone removed: {}", domain);
    Ok(())
}

fn delete_virtual_host(domain: &str) -> Result<(), Box<dyn std::error::Error>> {
    let vhost_file = format!("/etc/httpd/conf.d/{}.conf", domain);
    let web_root = format!("/var/www/html/{}", domain);

    if std::path::Path::new(&vhost_file).exists() {
        std::fs::remove_file(&vhost_file)?;
    }

    if std::path::Path::new(&web_root).exists() {
        std::fs::remove_dir_all(&web_root)?;
    }

    println!("VirtualHost removed: {}", domain);
    Ok(())
}

fn delete_reverse_zone(ip: &str, domain: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return Err("Invalid IP format".into());
    }

    let reverse_zone = format!("{}.{}.{}.{}", parts[2], parts[1], parts[0], domain);

    let zone_file = format!("/var/named/{}.hosts", reverse_zone);

    let named_conf_path = "/etc/named.conf";
    let content = std::fs::read_to_string(named_conf_path)?;

    let new_content = content
        .lines()
        .filter(|line| !line.contains(&reverse_zone))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(named_conf_path, new_content)?;

    if std::path::Path::new(&zone_file).exists() {
        std::fs::remove_file(zone_file)?;
    }

    println!("Reverse zone removed: {}", reverse_zone);
    Ok(())
}

fn handle_blacklist_add(args: &BlacklistArgs) -> Result<(), Box<dyn std::error::Error>> {
    let zone_dir = PathBuf::from("/var/named/blacklist");
    fs::create_dir_all(&zone_dir)?;

    let zone_file = zone_dir.join(format!("{}.hosts", args.domain));

    create_zone_file(
        &zone_file,
        &args.ip,
        &ZoneType::Forward,
        &args.ns_fqdn,
        &args.soa_email,
    )?;

    handle_dns_record_command(&DnsRecordArgs {
        domain: args.domain.clone(),
        host: String::from("@"),
        record_type: DnsRecordType::A,
        value: args.ip.clone(),
    })?;
    handle_dns_record_command(&DnsRecordArgs {
        domain: args.domain.clone(),
        host: String::from("*"),
        record_type: DnsRecordType::A,
        value: args.ip.clone(),
    })?;

    Ok(())
}

fn handle_nfs(args: &NfsArgs) -> io::Result<()> {
    let exports_path = "/etc/exports";
    let content = fs::read_to_string(exports_path).unwrap_or_default();

    let entry = format!("{} {}({})", args.path, args.client, args.options);

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    match args.action {
        NfsAction::Add => {
            if !lines.iter().any(|l| l.contains(&args.path)) {
                lines.push(entry);
            }
        }
        NfsAction::Remove => {
            lines.retain(|l| !l.contains(&args.path));
        }
        NfsAction::Update => {
            lines.retain(|l| !l.contains(&args.path));
            lines.push(entry);
        }
        NfsAction::Disable => {
            for line in &mut lines {
                if line.contains(&args.path) && !line.starts_with('#') {
                    *line = format!("#{}", line);
                }
            }
        }
    }

    fs::write(exports_path, lines.join("\n"))?;

    std::process::Command::new("systemctl")
        .arg("restart")
        .arg("nfs-server")
        .output()?;

    Ok(())
}

fn handle_backup(args: &BackupArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.action {
        BackupAction::Tar => {
            let output = format!(
                "{}/system_backup_{}.tar.gz",
                args.destination,
                chrono::Local::now().format("%Y%m%d")
            );

            std::process::Command::new("tar")
                .args([
                    "-czf",
                    &output,
                    "/etc/passwd",
                    "/etc/group",
                    "/etc/shadow",
                    "/etc/gshadow",
                ])
                .status()?;
        }
        BackupAction::Rsync => {
            let current_dest = format!(
                "{}/{}",
                args.destination,
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            let latest_link = format!("{}/latest", args.destination);

            let mut rsync_cmd = std::process::Command::new("rsync");
            rsync_cmd.args(["-avh", "--delete"]);

            if std::path::Path::new(&latest_link).exists() {
                rsync_cmd.arg(format!("--link-dest={}", latest_link));
            }

            rsync_cmd.args([&args.source, &current_dest]);

            let status = rsync_cmd.status()?;

            if status.success() {
                let _ = std::fs::remove_file(&latest_link);
                std::os::unix::fs::symlink(&current_dest, &latest_link)?;
            }
        }
    }

    Ok(())
}

fn handle_raid(args: &RaidArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.devices.len() < 3 {
        return Err("RAID5 requires at least 3 devices".into());
    }

    let mut cmd = std::process::Command::new("mdadm");
    cmd.arg("--create")
        .arg("/dev/md0")
        .arg("--level=5")
        .arg("--raid-devices")
        .arg(args.devices.len().to_string());

    for dev in &args.devices {
        cmd.arg(dev);
    }

    if !args.spare.is_empty() {
        cmd.arg("--spare-devices").arg(args.spare.len().to_string());

        for dev in &args.spare {
            cmd.arg(dev);
        }
    }

    cmd.status()?;

    std::process::Command::new("mkfs.ext4")
        .arg("/dev/md0")
        .status()?;

    fs::create_dir_all(&args.mount_point)?;

    std::process::Command::new("mount")
        .args(["/dev/md0", &args.mount_point])
        .status()?;

    Ok(())
}
