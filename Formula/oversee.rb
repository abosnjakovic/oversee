class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.3"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.3/oversee-0.3.3-aarch64-apple-darwin.tar.gz"
      sha256 "27af05b80a2cf71b5eaef1b7f0c357a3b049982c7efe2e57126ffe144ad15eaa"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.3/oversee-0.3.3-x86_64-apple-darwin.tar.gz"
      sha256 "25b58fcc2778176f9e0b79f04dc1e551b2c10abe9155d4de16b244d00de9c8c1"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
