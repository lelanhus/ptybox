# Installation

## From Source

```bash
git clone https://github.com/lelanhus/ptybox
cd ptybox
cargo build --release
./target/release/ptybox --help
```

## From crates.io

```bash
cargo install ptybox-cli
```

## Shell Completions

Generate shell completions for your shell:

```bash
# Bash
ptybox completions bash > ~/.bash_completion.d/ptybox

# Zsh
ptybox completions zsh > ~/.zfunc/_ptybox

# Fish
ptybox completions fish > ~/.config/fish/completions/ptybox.fish
```

## Verify Installation

```bash
ptybox --version
ptybox --help
```
