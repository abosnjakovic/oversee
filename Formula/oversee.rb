class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.11"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.11/oversee-0.3.11-aarch64-apple-darwin.tar.gz"
      sha256 "79869a1c1b40ffb59948973ca24716aebd3493d44dd15861f1e5b99ca434ab98"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.11/oversee-0.3.11-x86_64-apple-darwin.tar.gz"
      sha256 "567cd7520e6f7cf51316e6423dec1f9a0a2551bca8aff22d366eb715cfdb5605"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
