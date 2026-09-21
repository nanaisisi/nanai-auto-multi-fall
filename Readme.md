cargo coupling --ai --no-git
cargo llvm-cov
cargo clippy  --fix  --all --allow-dirty --all-targets --all-features