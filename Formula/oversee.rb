class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.2/oversee-0.3.2-aarch64-apple-darwin.tar.gz"
      sha256 "2d179b0089f41988730b86a288a73d6ad3acd2a04cf96b31077204e81e9714fc"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.2/oversee-0.3.2-x86_64-apple-darwin.tar.gz"
      sha256 "8463125a31266ea9c487fa553fa499655719a049a20618c57a96f6305cabfb45"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
