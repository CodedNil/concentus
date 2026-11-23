fn main() {
    println!("cargo:rerun-if-changed=migrations");
    topcoat::tailwind::BuildConfig::new().render().unwrap();
}
