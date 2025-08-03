use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/crds.rs");
    generate_crds().expect("Failed to generate CRDs");
}

// Include CRD definitions
include!("src/crd.rs");

fn generate_crds() -> Result<(), Box<dyn std::error::Error>> {
    use kube::CustomResourceExt;
    use serde_yaml;

    // Create the output directory
    let out_dir = Path::new("crds");
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    // List CRDs here:
    let crds: Vec<(&str, _)> = vec![
        ("restic-repository", ResticRepository::crd()),
        ("node-backups", NodeBackup::crd()),
    ];

    for (name, crd) in crds {
        let yaml = serde_yaml::to_string(&crd)?;
        let path = out_dir.join(format!("{}.yaml", name));
        fs::write(path, yaml)?;
    }

    Ok(())
}
