class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.10"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.10/oversee-0.3.10-aarch64-apple-darwin.tar.gz"
      sha256 "be513b18f33d30133387a787aabb7724f3a3846013c44eac0339517767d16dd8"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.10/oversee-0.3.10-x86_64-apple-darwin.tar.gz"
      sha256 "4aea2cb0a91c97605eb809f28dfd1046cc9a9b842d44e884fc27c4c3da4a2996"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
