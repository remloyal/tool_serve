cargo build --release --target=x86_64-apple-darwin
cargo build --release --target=aarch64-apple-darwin

cargo build --release  --target=x86_64-pc-windows-msvc

cargo install cross
cross build --target aarch64-apple-darwin
cross build --target x86_64-apple-darwin


http://127.0.0.1:54322/file?path=