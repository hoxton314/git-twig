fn main() {
  // Baked into the binary by `option_env!` (hosting/github_api.rs).
  println!("cargo:rerun-if-env-changed=TWIG_GITHUB_OAUTH_CLIENT_ID");
  tauri_build::build()
}
