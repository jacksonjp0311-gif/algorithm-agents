#!/usr/bin/env sh
set -eu

repo="jacksonjp0311-gif/algorithm-agents"
version="${1:-latest}"
install_root="${ALCHETRON_INSTALL_ROOT:-$HOME/.local/bin}"
case "$(uname -s)" in
  Linux) asset="alchetron-linux-x86_64.tar.gz" ;;
  Darwin) asset="alchetron-macos-host.tar.gz" ;;
  *) echo "Unsupported operating system" >&2; exit 1 ;;
esac
if [ "$version" = "latest" ]; then
  url="https://github.com/$repo/releases/latest/download/$asset"
else
  url="https://github.com/$repo/releases/download/$version/$asset"
fi
runtime_root="${ALCHETRON_RUNTIME_ROOT:-$HOME/.local/share/alchetron}"
mkdir -p "$install_root" "$runtime_root"
tmp="$runtime_root/.alchetron-download.tar.gz"
curl --fail --location "$url" --output "$tmp"
tar -xzf "$tmp" -C "$runtime_root"
rm "$tmp"
chmod 755 "$runtime_root/algo"
ln -sf "$runtime_root/algo" "$install_root/algo"
echo "Alchetron installed at $runtime_root"
echo "Run: algo doctor"
