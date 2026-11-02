//! Self-update (T4333): fetch the release list, download the platform binary and its minisign signature,
//! verify against the embedded public key, then replace the running executable. Restart to finish.
use tsfp_launcher_core::config::Channel;
use tsfp_launcher_core::update::{self, Offer};

/// Release signing public key. The secret key is owner-held (`.secrets/minisign-tsfp-ng.key`, GitHub secret
/// `MINISIGN_SECRET_KEY` in the public repo). Unsigned or wrongly signed downloads are never installed.
pub const PUBLIC_KEY: &str = "RWQvJv52Ih8oGINpp9v/f7iSFCtpZF4GOij50+r9qD1B4oKbWM1il22U";

const USER_AGENT: &str = concat!("tsfp-launcher/", env!("CARGO_PKG_VERSION"));

fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut resp = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("{url}: {e}"))?;
    resp.body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))
}

pub fn check(channel: Channel) -> Result<Option<Offer>, String> {
    let asset = update::platform_asset().ok_or("no launcher builds for this platform")?;
    let json = get(update::RELEASES_API)?;
    let json = String::from_utf8(json).map_err(|e| e.to_string())?;
    update::newest(&json, channel, env!("CARGO_PKG_VERSION"), &asset)
}

/// Check `data` against a minisign signature made by the release key.
pub fn verify(data: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::from_base64(public_key).map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(signature).map_err(|e| e.to_string())?;
    key.verify(data, &sig, false)
        .map_err(|e| format!("signature check failed: {e}"))
}

pub fn apply(offer: &Offer) -> Result<String, String> {
    let data = get(&offer.asset_url)?;
    let sig = String::from_utf8(get(&offer.signature_url)?).map_err(|e| e.to_string())?;
    verify(&data, &sig, PUBLIC_KEY)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let staged = exe.with_extension("update");
    std::fs::write(&staged, &data).map_err(|e| format!("{}: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    let result = self_replace::self_replace(&staged).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&staged);
    result.map(|()| {
        format!(
            "Updated to {}. Restart the launcher to finish.",
            offer.version
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_key_parses_and_garbage_is_rejected() {
        assert!(minisign_verify::PublicKey::from_base64(PUBLIC_KEY).is_ok());
        assert!(verify(b"x", "not a signature", PUBLIC_KEY).is_err());
    }

    /// Throwaway key pair (secret discarded); proves a genuine signature passes and tampering fails.
    const TEST_KEY: &str = "RWRauAqsTsaxqkteEvxjb7IBrG3X5tg0V5YPohADDmVjRCaZ5VISbtPW";
    const PAYLOAD: &[u8] = b"tsfp launcher update test payload\n";
    const SIGNATURE: &str = concat!(
        "untrusted comment: signature from minisign secret key\n",
        "RURauAqsTsaxqruvQA/GQcWTTWOSD7x3Jlqt+9zs2/3DWE3DKWBgcvoMpKyo/Fh7DrZctHtGvUdkRcGzSaY885pS9Y6YDPTdrgo=\n",
        "trusted comment: test\n",
        "7RNRD9tpTsCNWGBRku42+1NNJKE90vfn6qFgy53TA+vpuqFYPn64c2HDQi8Hxun8RjHwfepI1L4TVIOr2BiFBw==\n",
    );

    #[test]
    fn genuine_signature_passes_tampered_data_and_wrong_key_fail() {
        verify(PAYLOAD, SIGNATURE, TEST_KEY).unwrap();
        assert!(verify(b"tsfp launcher update test payloaX\n", SIGNATURE, TEST_KEY).is_err());
        assert!(verify(PAYLOAD, SIGNATURE, PUBLIC_KEY).is_err());
    }
}
