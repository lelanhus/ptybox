# Installation

## From GitHub Releases (Recommended)

`ptybox` release assets include:
- `ptybox-x86_64-apple-darwin.tar.gz`
- `ptybox-aarch64-apple-darwin.tar.gz`
- `ptybox-x86_64-unknown-linux-gnu.tar.gz`
- `ptybox-aarch64-unknown-linux-gnu.tar.gz`
- `checksums.sha256`

```bash
VERSION="v0.1.1"
TARGET="x86_64-unknown-linux-gnu" # choose your platform target
BASE_URL="https://github.com/lelanhus/ptybox/releases/download/${VERSION}"

curl -L -o "ptybox-${TARGET}.tar.gz" "${BASE_URL}/ptybox-${TARGET}.tar.gz"
curl -L -o checksums.sha256 "${BASE_URL}/checksums.sha256"

if command -v sha256sum >/dev/null 2>&1; then
  grep "ptybox-${TARGET}.tar.gz" checksums.sha256 | sha256sum -c -
else
  grep "ptybox-${TARGET}.tar.gz" checksums.sha256 | shasum -a 256 -c -
fi

tar -xzf "ptybox-${TARGET}.tar.gz"
./ptybox --version
```

## From crates.io (after publish)

```bash
cargo install ptybox-cli
ptybox --version
```

## From Source (Fallback)

```bash
git clone https://github.com/lelanhus/ptybox
cd ptybox
cargo build --release
./target/release/ptybox --help
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
