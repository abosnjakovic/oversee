class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.9"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.9/oversee-0.3.9-aarch64-apple-darwin.tar.gz"
      sha256 "b4129c26a5e1f1b371c611f1f4b127a7e113544d77e5c1d2c5ecd850d3b1af09"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.9/oversee-0.3.9-x86_64-apple-darwin.tar.gz"
      sha256 "4598b8ea23306011cc0c6717274aeed2257d21370fda04d66cc347b78fe3c391"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
