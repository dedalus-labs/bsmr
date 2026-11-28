fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::copy("value.txt", out.join("value.txt")).unwrap();
    let code = format!(
        "#[macro_export] macro_rules! value {{ () => {{ include_str!({:?}).trim() }} }}",
        out.join("value.txt"),
    );
    std::fs::write(out.join("bindings.rs"), code).unwrap();
}
