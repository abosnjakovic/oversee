class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.3.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.1/oversee-0.3.1-aarch64-apple-darwin.tar.gz"
      sha256 "f0c85b45f0e3ce5edcceffd5034460637c8953fb82bb9e465dfd1593ff7afa17"
    else
      url "https://github.com/abosnjakovic/oversee/releases/download/v0.3.1/oversee-0.3.1-x86_64-apple-darwin.tar.gz"
      sha256 "b72f79ae5633ee2b13ceb35a40a9e758cddfc800a2a1a8eb42ca85a1a51c5a3b"
    end
  end

  def install
    bin.install "oversee"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/oversee --version")
  end
end
