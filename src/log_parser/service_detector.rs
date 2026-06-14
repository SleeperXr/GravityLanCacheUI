/// Detects the gaming/CDN service based on the request hostname.
pub struct ServiceDetector;

impl ServiceDetector {
    /// Map a hostname to a known service name.
    pub fn detect(host: &str) -> String {
        let host_lower = host.to_lowercase();

        for (service, patterns) in SERVICE_PATTERNS {
            if patterns.iter().any(|p| host_matches(&host_lower, p)) {
                return service.to_string();
            }
        }

        "other".to_string()
    }
}

/// Check if a hostname matches a pattern (supports leading wildcard `*.`).
fn host_matches(host: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host.ends_with(suffix) || host == suffix
    } else {
        host == pattern
    }
}

/// Service → hostname patterns mapping.
const SERVICE_PATTERNS: &[(&str, &[&str])] = &[
    (
        "steam",
        &[
            "*.steamcontent.com",
            "*.cs.steampowered.com",
            "*.cm.steampowered.com",
            "*.steampowered.com",
            "*.steamstatic.com",
        ],
    ),
    (
        "epicgames",
        &[
            "*.epicgames.com",
            "*.unrealengine.com",
            "*.ol.epicgames.com",
            "*.on.epicgames.com",
            "epicgames-download1.akamaized.net",
            "*.epicgames.dev",
        ],
    ),
    (
        "gog",
        &[
            "*.gog.com",
            "*.cdn.gog.com",
        ],
    ),
    (
        "origin",
        &[
            "*.ea.com",
            "*.origin.com",
            "*.ea2d.com",
        ],
    ),
    (
        "ubisoft",
        &[
            "*.ubisoft.com",
            "*.ubi.com",
            "*.cdn.ubi.com",
        ],
    ),
    (
        "battlenet",
        &[
            "*.blizzard.com",
            "*.battle.net",
            "blzddist1-a.akamaihd.net",
            "blzddist2-a.akamaihd.net",
            "*.blz-contentstack.com",
        ],
    ),
    (
        "xbox",
        &[
            "*.xboxlive.com",
            "*.xbox.com",
            "*.xboxservices.com",
            "assets1.xboxlive.com",
            "assets2.xboxlive.com",
            "*.delivery.mp.microsoft.com",
        ],
    ),
    (
        "windowsupdate",
        &[
            "*.windowsupdate.com",
            "*.update.microsoft.com",
            "*.dl.delivery.mp.microsoft.com",
            "*.do.dsp.mp.microsoft.com",
            "tsfe.trafficshaping.dsp.mp.microsoft.com",
        ],
    ),
    (
        "riotgames",
        &[
            "*.riotgames.com",
            "*.riotcdn.net",
            "*.leagueoflegends.com",
        ],
    ),
    (
        "nintendo",
        &[
            "*.cdn.nintendo.net",
            "*.nintendo.com",
        ],
    ),
    (
        "playstation",
        &[
            "*.playstation.net",
            "*.sonyentertainmentnetwork.com",
            "*.dl.playstation.net",
        ],
    ),
    (
        "rockstar",
        &[
            "*.rockstargames.com",
            "*.socialclub.rockstargames.com",
        ],
    ),
    (
        "arenanet",
        &[
            "*.arenanet.com",
            "*.ncsoft.com",
        ],
    ),
    (
        "wargaming",
        &[
            "*.wargaming.net",
            "*.wgcdn.co",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_steam() {
        assert_eq!(ServiceDetector::detect("cache1.steamcontent.com"), "steam");
        assert_eq!(ServiceDetector::detect("valve123.cs.steampowered.com"), "steam");
    }

    #[test]
    fn detects_epicgames() {
        assert_eq!(ServiceDetector::detect("download.epicgames.com"), "epicgames");
    }

    #[test]
    fn detects_battlenet() {
        assert_eq!(ServiceDetector::detect("us.cdn.blizzard.com"), "battlenet");
    }

    #[test]
    fn detects_windowsupdate() {
        assert_eq!(ServiceDetector::detect("dl.delivery.mp.microsoft.com"), "windowsupdate");
    }

    #[test]
    fn detects_xbox() {
        assert_eq!(ServiceDetector::detect("assets1.xboxlive.com"), "xbox");
    }

    #[test]
    fn returns_other_for_unknown() {
        assert_eq!(ServiceDetector::detect("cdn.example.com"), "other");
    }
}
