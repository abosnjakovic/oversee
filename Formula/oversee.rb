class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.12"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.12/oversee-0.3.12-aarch64-apple-darwin.tar.gz"
      sha256 "cf3b9237ed9803df79c74d04a797c104d6fc5ea7c780f73c24df4f9e532e3869"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.12/oversee-0.3.12-x86_64-apple-darwin.tar.gz"
      sha256 "3555ed4b35f949c5044c8aa2bc9e9f525a5a51114fabc6b21ec886e79eedc79a"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
