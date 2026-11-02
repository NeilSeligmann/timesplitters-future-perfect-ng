// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable controller profiles, with a one-time import of the former temporary file.
use std::path::{Path, PathBuf};
use tsfp_input::controller::ControllerEventPath;

const LEGACY: &str = "tmp/controller-profiles.toml";

fn profile_path(home: Option<&Path>, data: Option<&Path>) -> Result<PathBuf, String> {
    let base = match data.filter(|p| p.is_absolute()) {
        Some(path) => path.to_path_buf(),
        None => home
            .filter(|p| p.is_absolute())
            .ok_or("no absolute HOME or XDG_DATA_HOME for controller profiles")?
            .join(".local/share"),
    };
    Ok(base.join("tsfp/controller-profiles.toml"))
}

fn user_path() -> Result<PathBuf, String> {
    profile_path(
        std::env::var_os("HOME").as_deref().map(Path::new),
        std::env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
    )
}

fn load_from(
    path: &Path,
    legacy: &Path,
    controllers: &mut ControllerEventPath,
) -> Result<(), String> {
    if path.try_exists().map_err(|e| e.to_string())? {
        return controllers.load_profile_file(path);
    }
    if legacy.try_exists().map_err(|e| e.to_string())? {
        controllers.load_profile_file(legacy)?;
        controllers.persist_profile_file(path)?;
    }
    Ok(())
}

pub fn load(controllers: &mut ControllerEventPath) -> Result<(), String> {
    load_from(&user_path()?, Path::new(LEGACY), controllers)
}

pub fn persist(controllers: &ControllerEventPath) -> Result<(), String> {
    controllers.persist_profile_file(&user_path()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_location_requires_absolute_xdg_or_home() {
        assert_eq!(
            profile_path(Some(Path::new("<private>")), Some(Path::new("relative"))).unwrap(),
            Path::new("<private>")
        );
        assert_eq!(
            profile_path(None, Some(Path::new("/data"))).unwrap(),
            Path::new("/data/tsfp/controller-profiles.toml")
        );
        assert!(profile_path(None, Some(Path::new("relative"))).is_err());
    }

    #[test]
    fn imports_legacy_once_and_never_overwrites_durable_profiles() {
        use tsfp_input::controller::ControllerProfile;
        use tsfp_input::router::{Router, RouterConfig};
        let root = (0..1000)
            .find_map(|n| {
                let path = std::env::temp_dir().join(format!(
                    "tsfp-controller-storage-{}-{n}",
                    std::process::id()
                ));
                std::fs::create_dir(&path).ok().map(|()| path)
            })
            .expect("isolated test directory");
        let legacy = root.join("old.toml");
        let path = root.join("data/profiles.toml");
        let mut original = ControllerEventPath::default();
        let mut router = Router::new(RouterConfig::default());
        let id = original
            .connect(
                &mut router,
                1,
                "0123456789abcdef0123456789abcdef",
                Some("serial"),
                None,
                "Pad",
            )
            .unwrap();
        let mut profile = ControllerProfile::default();
        profile.button_map[0] = Some(1);
        original.set_profile(&mut router, id, profile).unwrap();
        std::fs::write(&legacy, original.profile_text()).unwrap();
        let mut loaded = ControllerEventPath::default();
        load_from(&path, &legacy, &mut loaded).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original.profile_text()
        );
        std::fs::write(&legacy, "invalid legacy").unwrap();
        load_from(&path, &legacy, &mut loaded).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
