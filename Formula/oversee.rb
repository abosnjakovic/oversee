class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.4/oversee-0.3.4-aarch64-apple-darwin.tar.gz"
      sha256 "72d8dcfac646e05286a952a16216dbb08ec0c36bdf97707999c5ed86e114bb8a"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.4/oversee-0.3.4-x86_64-apple-darwin.tar.gz"
      sha256 "a224cee6e4fe3f479ba7dc0ec198e1bc4fd3c1f6dbbc07e46f53177ecc0ca7cd"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
