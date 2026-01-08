class Oversee < Formula
  desc "A modern system monitor for macOS with Apple Silicon GPU support"
  homepage "https://github.com/abosnjakovic/oversee"
  version "0.2.4"

  if Hardware::CPU.arm?
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.4/oversee-0.2.4-aarch64-apple-darwin.tar.gz"
    sha256 "4bdd0d9615b242c8e3a8e83b2e2b2c8a78d4ca6d4e7bf986783f74332defe0dc"
  else
    url "https://github.com/abosnjakovic/oversee/releases/download/v0.2.4/oversee-0.2.4-x86_64-apple-darwin.tar.gz"
    sha256 "8f20c1722b0905d496d4f8d8958d0049a0a516772a1ee94fb69731755cb48b16"
  end

  def install
    bin.install "oversee"
  end

  test do
    system "#{bin}/oversee", "--version"
  end
end
