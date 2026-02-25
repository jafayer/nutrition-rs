pub const DEFAULT_FILE_ENV_VAR: &str = "NUTRITION_DEFAULT_FILE";

pub fn get_default_file_from_env() -> Option<String> {
    std::env::var(DEFAULT_FILE_ENV_VAR).ok()
}
