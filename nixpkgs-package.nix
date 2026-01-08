# Copy this file to nixpkgs at: pkgs/by-name/ov/oversee/package.nix
#
# Before submitting:
# 1. Create a release tag (v0.2.3) on GitHub
# 2. Run: nix run nixpkgs#nix-prefetch-github -- abosnjakovic oversee --rev v0.2.3
# 3. Fill in the hash value below
# 4. Build to get cargoHash (will fail and show correct hash)

{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:

rustPlatform.buildRustPackage rec {
  pname = "oversee";
  version = "0.2.3";

  src = fetchFromGitHub {
    owner = "abosnjakovic";
    repo = "oversee";
    rev = "v${version}";
    hash = ""; # TODO: Run nix-prefetch-github abosnjakovic oversee --rev v0.2.3
  };

  cargoHash = ""; # TODO: Build once, copy hash from error message

  meta = with lib; {
    description = "A modern system monitor for macOS with Apple Silicon GPU support";
    homepage = "https://github.com/abosnjakovic/oversee";
    changelog = "https://github.com/abosnjakovic/oversee/releases/tag/v${version}";
    license = licenses.mit;
    maintainers = with maintainers; [ ]; # TODO: Add your maintainer name
    platforms = platforms.darwin;
    mainProgram = "oversee";
  };
}
