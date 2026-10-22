use std::fmt;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Os::Mac => "macOS",
            Os::Linux => "Linux",
            Os::Windows => "Windows",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PackageManager {
    Brew,
    Apt,
    Dnf,
    Pacman,
    Winget,
}

impl PackageManager {
    pub fn candidates(os: Os) -> &'static [PackageManager] {
        match os {
            Os::Mac => &[PackageManager::Brew],
            Os::Linux => &[
                PackageManager::Apt,
                PackageManager::Dnf,
                PackageManager::Pacman,
            ],
            Os::Windows => &[PackageManager::Winget],
        }
    }

    pub fn bin(self) -> &'static str {
        match self {
            PackageManager::Brew => "brew",
            PackageManager::Apt => "apt-get",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
            PackageManager::Winget => "winget",
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            PackageManager::Brew => "brew",
            PackageManager::Apt => "apt",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
            PackageManager::Winget => "winget",
        })
    }
}
