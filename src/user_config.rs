use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Shell {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            program: String::from("powershell.exe"),
            args: vec![],
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub shell: Shell,
}

pub fn load_config() -> Result<Option<Config>, toml::de::Error> {
    crate::winapi::get_known_folder_path(&windows::Win32::UI::Shell::FOLDERID_Profile).map_or(
        Ok(None),
        |p| {
            std::fs::read_to_string(p.join(".winch.toml"))
                .map(|s| toml::from_str::<Config>(s.as_str()).map(Some))
                .unwrap_or(Ok(None))
        },
    )
}
