use std::io::Write;

use ssh_key::{HashAlg, PrivateKey, SshSig};

const NAMESPACE: &str = "bk";

/// Sign `data` with the SSH private key at `key_path`.
/// Returns a PEM-encoded OpenSSH signature string, or None if signing fails.
pub fn ssh_sign(data: &[u8], key_path: &str) -> Option<String> {
    let private_key = PrivateKey::read_openssh_file(std::path::Path::new(key_path))
        .map_err(|e| log::warn!("failed to read ssh private key at {key_path}: {e}"))
        .ok()?;
    let sig = SshSig::sign(&private_key, NAMESPACE, HashAlg::Sha256, data)
        .map_err(|e| log::warn!("failed to sign data: {e}"))
        .ok()?;
    sig.to_pem(Default::default())
        .map_err(|e| log::warn!("failed to encode signature to pem: {e}"))
        .ok()
}

fn write_exclusive(path: &str, data: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(data)
}

/// Verify that `sig_pem` is a valid OpenSSH signature over `data` made by
/// the key whose public half is `pub_key_str` (OpenSSH wire format, e.g. `ssh-ed25519 AAAA...`).
///
/// Uses `ssh-keygen -Y verify` under the hood so it matches the standard reference behaviour.
pub fn ssh_verify(pub_key_str: &str, data: &[u8], sig_pem: &str) -> bool {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();

    let allowed_signers_path = format!("/tmp/bk_allowed_{pid}_{nanos}");
    let sig_path = format!("/tmp/bk_sig_{pid}_{nanos}");

    // allowed_signers format: "<principal> <keytype> <base64key>"
    let allowed_signers = format!("bk-host {pub_key_str}\n");

    // Use write_exclusive (O_CREAT | O_EXCL) to prevent symlink race attacks.
    if write_exclusive(&allowed_signers_path, allowed_signers.as_bytes()).is_err()
        || write_exclusive(&sig_path, sig_pem.as_bytes()).is_err()
    {
        let _ = std::fs::remove_file(&allowed_signers_path);
        let _ = std::fs::remove_file(&sig_path);
        return false;
    }

    let result = std::process::Command::new("ssh-keygen")
        .args([
            "-Y",
            "verify",
            "-f",
            &allowed_signers_path,
            "-I",
            "bk-host",
            "-n",
            NAMESPACE,
            "-s",
            &sig_path,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(data);
            }
            child.wait()
        });

    let _ = std::fs::remove_file(&allowed_signers_path);
    let _ = std::fs::remove_file(&sig_path);

    result.map(|s| s.success()).unwrap_or(false)
}
