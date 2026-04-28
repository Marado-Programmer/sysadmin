# System Administration

## Installation

On a fresh AlmaLinux 8 installation:

```
dnf update -y
dnf groupinstall -y "Development Tools"
dnf install -y git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cd /opt/
git clone https://github.com/Marado-Programmer/sysadmin.git
cd sysadmin/
cargo build
cargo install --path .
```

From there, try to `sysadmin --help` and the rest you'll figure it out.

# Usage


